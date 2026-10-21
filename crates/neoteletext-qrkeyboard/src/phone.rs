//! The phone's side, as qrkeyboard's phone page (`controller.ts`) runs
//! it: read the join address, fetch the screen's configuration and check
//! its hash, join, pair, and send keys. For tests of a screen against a
//! relay, and for checking a relay works, never for a real phone.

use std::time::{Duration, Instant};

use crate::config::{self, CONFIG_HASH_BYTES};
use crate::crypto::{Opener, Role, SECRET_BYTES, Sealer, constant_time_equal};
use crate::json::Value;
use crate::message::{self, from_base64url};
use crate::net::{self, Incoming, WebSocket};
use crate::pairing::{Outcome, Pairing, Secret};
use crate::{Error, LEAVE};

/// How long pairing may take (the phone page's `PAIRING_TIMEOUT_MS`).
const PAIRING_TIMEOUT: Duration = Duration::from_secs(15);
/// How long an acknowledgement may take.
const ACK_TIMEOUT: Duration = Duration::from_secs(10);
/// One wait on the relay.
const STEP: Duration = Duration::from_millis(100);

/// The time `wait` from now.
fn later(wait: Duration) -> Instant {
    let now = Instant::now();
    now.checked_add(wait).unwrap_or(now)
}

/// A paired phone.
pub struct Phone {
    socket: WebSocket,
    sealer: Sealer,
    opener: Opener,
}

/// The parts of a join address's fragment.
fn fragment<'a>(url: &'a str, name: &str) -> Option<&'a str> {
    let (_, fragment) = url.split_once('#')?;
    fragment.split('&').find_map(|part| part.strip_prefix(name)?.strip_prefix('='))
}

impl Phone {
    /// Scans `join_url`, with `pin` in PIN mode, and pairs.
    ///
    /// # Errors
    /// A malformed address, a configuration whose hash does not match, a
    /// wrong PIN, or a relay that fails.
    pub fn join(join_url: &str, pin: Option<&str>) -> Result<Self, Error> {
        let (origin, _) = join_url.split_once("/app#").ok_or(Error::Address)?;
        let code = fragment(join_url, "c").ok_or(Error::Address)?;
        let hash = fragment(join_url, "h").and_then(from_base64url).ok_or(Error::Address)?;
        let key = fragment(join_url, "k")
            .map(|key| from_base64url(key).and_then(|key| <[u8; SECRET_BYTES]>::try_from(key).ok()));
        let (status, body) = net::request(&net::parse(&format!("{origin}/j/{code}"))?, "GET", None, None, None)?;
        if status != neoteletext_web::consts::STATUS_OK {
            return Err(Error::Status(status));
        }
        let resolved = Value::parse(&body).map_err(|_| Error::Protocol("the join answer is not JSON"))?;
        let received = resolved.get("config").ok_or(Error::Protocol("no configuration"))?;
        // The relay must give the configuration the QR's hash names.
        let received_hash: [u8; CONFIG_HASH_BYTES] = config::hash(received);
        if !constant_time_equal(&received_hash, &hash) {
            return Err(Error::Protocol("a configuration whose hash does not match the QR"));
        }
        let secret = match (key, pin) {
            (Some(Some(key)), None) => Secret::Key(key),
            (None, Some(pin)) => Secret::Pin(pin.to_owned()),
            _ => return Err(Error::Address),
        };
        let relay =
            net::parse(resolved.get("relay").and_then(Value::as_str).ok_or(Error::Protocol("a relay address"))?)?;
        let mut socket = WebSocket::connect(&relay, None)?;
        socket.send_text(&Value::object(&[("t", Value::string("join"))]).text())?;
        let deadline = later(PAIRING_TIMEOUT);
        let mut pairing: Option<Pairing> = None;
        while Instant::now() < deadline {
            match socket.receive(STEP)? {
                None => {}
                Some(Incoming::Text(text)) => {
                    let message = Value::parse(text.as_bytes()).unwrap_or(Value::Null);
                    if message.get("type").and_then(Value::as_str) == Some("peer")
                        && message.get("up").and_then(Value::as_bool) == Some(true)
                    {
                        let run = Pairing::new(Role::Phone, code, &secret, &received_hash)?;
                        for frame in run.start() {
                            socket.send_binary(&frame)?;
                        }
                        pairing = Some(run);
                    }
                }
                Some(Incoming::Binary(frame)) => {
                    let Some(run) = &mut pairing else { continue };
                    let (out, outcome) = run.receive(&frame);
                    for frame in out {
                        socket.send_binary(&frame)?;
                    }
                    match outcome {
                        Some(Outcome::Paired(channel)) => {
                            let (sealer, opener) = *channel;
                            return Ok(Self { socket, sealer, opener });
                        }
                        Some(Outcome::WrongPin) => return Err(Error::Protocol("the PIN did not match")),
                        None => {}
                    }
                }
                Some(Incoming::Closed(code)) => return Err(Error::Closed(code.unwrap_or(0))),
            }
        }
        Err(Error::Network("pairing took too long".into()))
    }

    /// Sends a message and waits for its acknowledgement: whether the
    /// screen took it.
    fn send(&mut self, members: &[(&str, Value)]) -> Result<bool, Error> {
        let id = message::new_id()?;
        let mut all = vec![("id", Value::string(&id))];
        all.extend(members.iter().cloned());
        let plaintext = message::pack(&Value::object(&all))?;
        let frame = self.sealer.seal(&plaintext).ok_or(Error::Limit("the frame counter"))?;
        self.socket.send_binary(&frame)?;
        let deadline = later(ACK_TIMEOUT);
        while Instant::now() < deadline {
            if let Some(Incoming::Binary(frame)) = self.socket.receive(STEP)?
                && let Some(plaintext) = self.opener.open(&frame)
                && let Some(json) = plaintext.get(1..)
                && let Ok(ack) = Value::parse(json)
                && ack.get("type").and_then(Value::as_str) == Some("ack")
                && ack.get("id").and_then(Value::as_str) == Some(id.as_str())
            {
                return Ok(ack.get("ok").and_then(Value::as_bool) == Some(true));
            }
        }
        Err(Error::Network("no acknowledgement".into()))
    }

    /// A key of the layout down then up.
    ///
    /// # Errors
    /// When the relay fails or the screen does not acknowledge.
    pub fn press(&mut self, key: &str) -> Result<bool, Error> {
        let down = self.send(&[("type", Value::string("down")), ("key", Value::string(key))])?;
        let up = self.send(&[("type", Value::string("up")), ("key", Value::string(key))])?;
        Ok(down && up)
    }

    /// Text typed in the text box.
    ///
    /// # Errors
    /// When the relay fails or the screen does not acknowledge.
    pub fn type_text(&mut self, text: &str) -> Result<bool, Error> {
        self.send(&[("type", Value::string("key")), ("text", Value::string(text))])
    }

    /// Asks the screen to finish the session.
    ///
    /// # Errors
    /// When the relay fails.
    pub fn finish(&mut self) -> Result<(), Error> {
        let id = message::new_id()?;
        let plaintext =
            message::pack(&Value::object(&[("type", Value::string("finish")), ("id", Value::string(&id))]))?;
        let frame = self.sealer.seal(&plaintext).ok_or(Error::Limit("the frame counter"))?;
        self.socket.send_binary(&frame)
    }

    /// Leaves, releasing the screen for another phone.
    pub fn leave(mut self) {
        let _ = self.socket.send_text(LEAVE);
        self.socket.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fragment_is_read() {
        let url = "https://qrkeyboard.com/app#c=ABCD&k=xyz&h=123";
        assert_eq!(fragment(url, "c"), Some("ABCD"));
        assert_eq!(fragment(url, "k"), Some("xyz"));
        assert_eq!(fragment(url, "h"), Some("123"));
        assert_eq!(fragment(url, "x"), None);
    }
}
