//! The agent's use of the cookie store (REQ-WEB-006, REQ-SEC-013): where
//! its cookies are, what a response sets and clears, the `Cookie` header
//! of a request, persistence, and the memory of hosts reached over HTTPS
//! (REQ-SEC-007).

use neoteletext_core::types::Instant;

use super::{Event, Sink, UserAgent, event};
use crate::bytes::{Overflow, TextBuf, Writer};
use crate::consts::{CRLF, header};
use crate::cookie::{CookieJar, CookieRejected, Cookies, Site};
use crate::http::{self, HttpError, ResponseHead};
use crate::port::{CookieStorage, RandomSource};
use crate::url::Url;
use neoteletext_core::policy;

/// Where the agent's cookies are: the port's, when it gave some
/// (decision E.104), else the agent's own jar.
pub(super) fn cookie_store<'a, 'm, const SLOTS: usize, const BYTES: usize>(
    jar: &'a mut CookieJar<SLOTS, BYTES>,
    port: &'a mut Option<&'m mut dyn Cookies>,
) -> &'a mut (dyn Cookies + 'm) {
    match port {
        Some(port) => &mut **port,
        None => jar,
    }
}

/// Stores the `Set-Cookie` values of a complete response head, after
/// clearing the host's cookies when the head says `Clear-Site-Data:
/// "cookies"` over an encrypted connection (W3C Clear Site Data section
/// 4.2, decision E.104), so that a response can sign the user out and set
/// a fresh cookie at once. The head is read twice: first for what it
/// clears, then for what it sets.
#[allow(clippy::too_many_arguments, reason = "a cookie's context: where, how, when")]
pub(super) fn store_response_cookies(
    store: &mut dyn Cookies,
    head_bytes: &[u8],
    host: &str,
    path: &str,
    scope: Option<&str>,
    secure: bool,
    seconds: u64,
    calendar: bool,
    sink: &mut Sink<'_>,
) -> Result<ResponseHead, HttpError> {
    let mut setting = false;
    let head = http::parse_head(head_bytes, &mut |_| setting = true)?;
    if !cfg!(feature = "cookies") {
        if setting {
            event(sink, Event::CookieRejected(CookieRejected::NotCompiledIn));
        }
        return Ok(head);
    }
    if head.clear_cookies && secure {
        store.clear_host(host);
    }
    if setting {
        let _ = http::parse_head(head_bytes, &mut |value| {
            if let Err(rejected) = store.store_within(value, host, path, scope, secure, seconds, calendar) {
                event(sink, Event::CookieRejected(rejected));
            }
        });
    }
    Ok(head)
}

/// Writes the `Cookie` header for a request to `url`, if any cookie
/// matches, keeping room for the empty line and `body_len` bytes of body.
/// Returns true when some matching cookies did not fit.
pub(super) fn write_cookie_header(
    jar: &mut dyn Cookies,
    writer: &mut Writer<'_>,
    url: &Url,
    path: &str,
    site: Site,
    seconds: u64,
    body_len: usize,
) -> Result<bool, Overflow> {
    if !cfg!(feature = "cookies") {
        return Ok(false);
    }
    // Room for this header's line end, the empty line and the body.
    writer.set_reserve(CRLF.len().saturating_mul(2).saturating_add(body_len));
    let mark = writer.len();
    let started = writer.put(header::COOKIE).and_then(|()| writer.put(b": "));
    let (count, truncated) = match started {
        Ok(()) => jar.header_for(url.host_text(), path, url.is_secure(), site, seconds, writer),
        Err(Overflow) => (0, true),
    };
    writer.set_reserve(0);
    if count == 0 {
        writer.truncate(mark);
    } else {
        writer.put(CRLF)?;
    }
    Ok(truncated)
}

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    /// Saves the cookies that may be kept (REQ-SEC-013), when the
    /// configuration allows it and calendar time is known. Returns the
    /// number saved. `scratch` holds the encoded data.
    pub fn save_cookies<S: CookieStorage>(
        &self,
        now: Instant,
        storage: &mut S,
        scratch: &mut [u8],
    ) -> Result<usize, Option<S::Error>> {
        let (seconds, calendar) = self.seconds(now);
        if !cfg!(feature = "cookies") || !self.config.persist_cookies || !calendar {
            return Ok(0);
        }
        let mut writer = Writer::new(scratch);
        let count = self.jar.export(seconds, &mut writer).map_err(|_| None)?;
        storage.save(writer.written()).map_err(Some)?;
        Ok(count)
    }

    /// Restores saved cookies, when the configuration allows it and
    /// calendar time is known. Returns the number restored.
    pub fn load_cookies<S: CookieStorage>(
        &mut self,
        now: Instant,
        storage: &mut S,
        scratch: &mut [u8],
    ) -> Result<usize, S::Error> {
        let (seconds, calendar) = self.seconds(now);
        if !cfg!(feature = "cookies") || !self.config.persist_cookies || !calendar {
            return Ok(0);
        }
        let len = storage.load(scratch)?;
        Ok(self.jar.import(scratch.get(..len).unwrap_or(&[]), seconds))
    }

    /// Erases every cookie, the port's when it keeps them (REQ-SEC-013).
    pub(super) fn wipe_cookies(&mut self) {
        cookie_store(&mut self.jar, &mut self.port_cookies).wipe();
    }

    /// REQ-SEC-007: the host was reached over HTTPS this session, or holds
    /// a live cookie set over HTTPS, and `url` is plain. Decided by the
    /// policy.
    pub(super) fn is_downgrade(&self, now: Instant, url: &Url) -> bool {
        let host = url.host_text();
        let (seconds, _) = self.seconds(now);
        let reached = self.secure_hosts.iter().flatten().any(|known| known.as_str() == host)
            || cfg!(feature = "cookies")
                && self.port_cookies.as_deref().map_or_else(
                    || self.jar.reached_securely(host, seconds),
                    |port| port.reached_securely(host, seconds),
                );
        policy::is_downgrade(reached, url.origin().parts(), host)
    }

    /// Remembers that `host` was reached over HTTPS (REQ-SEC-007). The
    /// memory is kept most recently seen first; a host seen again moves to
    /// the front, and a new host when the memory is full takes the place
    /// of the one least recently seen, so the rule keeps holding for the
    /// hosts in use (decision E.114 item 3, finding F-43).
    pub(super) fn remember_secure(&mut self, host: &str) {
        let Some(entry) = TextBuf::from_bytes(host.as_bytes()) else { return };
        let hosts = &mut self.secure_hosts;
        let seen = hosts.iter().position(|slot| slot.is_none_or(|known| known.as_str() == host));
        let last = seen.unwrap_or(hosts.len().saturating_sub(1));
        if let Some(moved) = hosts.get_mut(..=last) {
            moved.rotate_right(1);
        }
        if let Some(first) = hosts.first_mut() {
            *first = Some(entry);
        }
    }
}
