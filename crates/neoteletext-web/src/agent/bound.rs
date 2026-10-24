//! The agent's side of Device Bound Session Credentials (REQ-WEB-017,
//! decision E.110): it registers the sessions servers ask for, keeps them
//! in [`Sessions`], and before a request in a session's scope that would
//! go without one of its cookies, refreshes the session first and then
//! sends the request (sections 8.5 to 8.9 of the Editor's Draft of 8
//! September 2026). The protocol's records are [`crate::dbsc`]'s.

use super::{
    BoundChange, Event, Exchange, Method, Purpose, RandomSource, ResponseHead, Sink, Site, UserAgent,
    cookies::cookie_store, event,
};
use crate::bytes::{Overflow, TextBuf, Writer};
use crate::dbsc::{
    self, Answer, CHALLENGE_HEADER, DeviceKeys, Instructions, KeyHandle, MAX_AUTHORIZATION, MAX_CHALLENGE,
    MAX_REGISTRATIONS_PER_HOUR, MAX_SESSION_URL, MAX_SESSIONS, PublicKey, REGISTRATION_HEADER,
    REGISTRATION_WINDOW_SECONDS, RESPONSE_HEADER, SESSION_ID_HEADER, SESSION_LIFETIME_SECONDS, Session, Sessions,
};
use crate::http;
use crate::url::{Origin, Url};
use neoteletext_core::types::Instant;

/// HTTP statuses the refresh table of section 8.8 names.
const STATUS_FORBIDDEN: u16 = 403;
const STATUS_PROXY_AUTHENTICATION: u16 = 407;
const STATUS_TOO_MANY: u16 = 429;
const STATUS_CLIENT_FIRST: u16 = 400;
const STATUS_SERVER_FIRST: u16 = 500;
const STATUS_SUCCESS: core::ops::Range<u16> = 200..300;

/// A registration a response asked for, not yet made.
#[derive(Clone, Copy, Debug)]
struct Asked {
    /// The registration endpoint, absolute, on the asking origin.
    url: TextBuf<MAX_SESSION_URL>,
    challenge: Option<TextBuf<MAX_CHALLENGE>>,
    authorization: Option<TextBuf<MAX_AUTHORIZATION>>,
}

/// A registration in flight, with the key made for it.
#[derive(Clone, Copy, Debug)]
struct Registering {
    asked: Asked,
    key: KeyHandle,
    public: PublicKey,
}

/// A request held until a session's refresh is over.
#[derive(Clone, Copy, Debug)]
struct Deferred {
    purpose: Purpose,
    url: Url,
    method: Method,
    site: Site,
    /// The sessions already refreshed for it, one bit a slot (section 8.5:
    /// each at most once a request).
    handled: u8,
}

/// The registrations one origin has had the agent make in the current
/// window (finding F-91).
#[derive(Clone, Copy, Debug)]
struct Registrations {
    origin: Origin,
    /// When the window began, in seconds of the agent's clock.
    since: u64,
    made: u8,
}

/// The agent's device-bound sessions and the exchanges they wait for.
#[derive(Debug)]
pub(super) struct Bound {
    sessions: Sessions,
    /// The registrations of the origins seen most recently, one a slot.
    registrations: [Option<Registrations>; MAX_SESSIONS],
    asked: Option<Asked>,
    registering: Option<Registering>,
    refresh: Option<(u8, bool)>,
    deferred: Option<Deferred>,
    /// The sessions refreshed for the request being dispatched.
    handled: u8,
}

// The slots fit the bits of `Deferred::handled`.
const _: () = assert!(MAX_SESSIONS <= u8::BITS as usize);

/// A bound session's response, as its completion reads it: the status, the
/// body, the address answered and the time in seconds.
#[derive(Clone, Copy)]
struct Answered<'a> {
    status: u16,
    body: &'a [u8],
    url: &'a Url,
    seconds: u64,
}

impl Bound {
    pub(super) const fn new() -> Self {
        Self {
            sessions: Sessions::new(),
            registrations: [None; MAX_SESSIONS],
            asked: None,
            registering: None,
            refresh: None,
            deferred: None,
            handled: 0,
        }
    }

    /// Counts a registration `origin` asks for at `seconds`, and says
    /// whether it may be made: at most [`MAX_REGISTRATIONS_PER_HOUR`] in
    /// each [`REGISTRATION_WINDOW_SECONDS`] (decision E.114 item 6, finding
    /// F-91). An origin not yet counted takes a free slot, or the slot whose
    /// window began first.
    fn may_register(&mut self, origin: &Origin, seconds: u64) -> bool {
        let slots = &mut self.registrations;
        let index = slots
            .iter()
            .position(|slot| slot.is_some_and(|counted| counted.origin == *origin))
            .or_else(|| slots.iter().position(Option::is_none))
            .or_else(|| {
                slots.iter().enumerate().min_by_key(|(_, slot)| slot.map_or(0, |counted| counted.since)).map(|(i, _)| i)
            });
        let Some(slot) = index.and_then(|index| slots.get_mut(index)) else { return false };
        let current = slot.filter(|counted| {
            counted.origin == *origin && seconds.saturating_sub(counted.since) < REGISTRATION_WINDOW_SECONDS
        });
        let counted = current.unwrap_or(Registrations { origin: *origin, since: seconds, made: 0 });
        if counted.made >= MAX_REGISTRATIONS_PER_HOUR {
            return false;
        }
        *slot = Some(Registrations { made: counted.made.saturating_add(1), ..counted });
        true
    }

    /// Acts on the answer to a registration (section 8.8): the session it
    /// makes is kept, or the registration's key is deleted.
    fn registered(&mut self, answered: &Answered<'_>, delete: &mut dyn FnMut(KeyHandle), sink: &mut Sink<'_>) {
        let Answered { status, body, url, seconds } = *answered;
        let Some(registering) = self.registering.take() else { return };
        let answer = STATUS_SUCCESS.contains(&status).then(|| dbsc::read_instructions(body));
        let made = match answer {
            Some(Ok(Answer::Continue(instructions))) => session_of(&instructions, registering.key, url, seconds),
            _ => None,
        };
        let Some(session) = made else {
            delete(registering.key);
            event(sink, Event::BoundSession { session: "", change: BoundChange::RegistrationFailed });
            return;
        };
        match self.sessions.insert(session) {
            Ok(replaced) => {
                if let Some(old) = replaced {
                    delete(old);
                }
                event(sink, Event::BoundSession { session: session.id.as_str(), change: BoundChange::Registered });
            }
            Err(key) => {
                delete(key);
                event(sink, Event::BoundSession { session: "", change: BoundChange::RegistrationFailed });
            }
        }
    }

    /// Acts on the answer to a refresh of the session in `slot` (section
    /// 8.9): the session is kept, updated, retried once with a challenge,
    /// or ended.
    fn refreshed(
        &mut self,
        slot: u8,
        retried: bool,
        answered: &Answered<'_>,
        delete: &mut dyn FnMut(KeyHandle),
        sink: &mut Sink<'_>,
    ) {
        let Answered { status, body, url, seconds } = *answered;
        let slot = usize::from(slot);
        let Some(session) = self.sessions.get(slot).copied() else { return };
        let id = session.id;
        let end = |bound: &mut Self, delete: &mut dyn FnMut(KeyHandle), sink: &mut Sink<'_>| {
            if let Some(ended) = bound.sessions.remove(slot) {
                delete(ended.key);
            }
            event(sink, Event::BoundSession { session: id.as_str(), change: BoundChange::Ended });
        };
        if STATUS_SUCCESS.contains(&status) {
            if body.is_empty() {
                event(sink, Event::BoundSession { session: id.as_str(), change: BoundChange::Refreshed });
                return;
            }
            match dbsc::read_instructions(body) {
                Ok(Answer::Continue(instructions)) if instructions.id == id => {
                    match session_of(&instructions, session.key, url, seconds) {
                        Some(updated) => {
                            if let Some(kept) = self.sessions.get_mut(slot) {
                                *kept = updated;
                            }
                            event(sink, Event::BoundSession { session: id.as_str(), change: BoundChange::Refreshed });
                        }
                        None => end(self, delete, sink),
                    }
                }
                _ => end(self, delete, sink),
            }
        } else if status == STATUS_FORBIDDEN && !retried && session.challenge.is_some() {
            // Section 8.8: once more, with the challenge this
            // answer gave; a second 403 ends the attempt.
            self.refresh = Some((u8::try_from(slot).unwrap_or(0), true));
        } else if (STATUS_CLIENT_FIRST..STATUS_SERVER_FIRST).contains(&status)
            && !matches!(status, STATUS_FORBIDDEN | STATUS_PROXY_AUTHENTICATION | STATUS_TOO_MANY)
        {
            end(self, delete, sink);
        } else {
            event(sink, Event::BoundSession { session: id.as_str(), change: BoundChange::RefreshFailed });
        }
    }
}

/// The bit of `slot` in a `handled` mask.
fn bit(slot: usize) -> u8 {
    u32::try_from(slot).ok().and_then(|slot| 1_u8.checked_shl(slot)).unwrap_or(0)
}

/// Writes `text` as an sf-string (RFC 9651 section 4.1.6). An sf-string
/// holds printable ASCII alone, `0x20..=0x7E` (section 3.3.3), so any
/// other byte fails rather than reaching the header (finding F-14,
/// decision E.114 item 6).
fn sf_string(text: &[u8], out: &mut Writer<'_>) -> Result<(), Overflow> {
    if !text.iter().all(|&byte| byte == b' ' || byte.is_ascii_graphic()) {
        return Err(Overflow);
    }
    out.byte(b'"')?;
    for &byte in text {
        if byte == b'"' || byte == b'\\' {
            out.byte(b'\\')?;
        }
        out.byte(byte)?;
    }
    out.byte(b'"')
}

/// Writes one header line `name: ` then what `value` writes.
fn header(
    out: &mut Writer<'_>,
    name: &[u8],
    value: impl FnOnce(&mut Writer<'_>) -> Result<(), Overflow>,
) -> Result<(), Overflow> {
    out.put(name)?;
    out.put(b": ")?;
    value(out)?;
    out.put(b"\r\n")
}

/// The headers of a bound session's request (sections 8.8 and 9): the
/// registration's proof, with the public key, and `Authorization`; a
/// refresh's session identifier and, when the server gave a challenge,
/// its proof. A key that cannot sign leaves the proof out, and the server
/// refuses.
pub(super) fn write_headers<'k>(
    bound: &mut Bound,
    keys: Option<&mut (dyn DeviceKeys + 'k)>,
    exchange: &Exchange,
    out: &mut Writer<'_>,
) -> Result<(), Overflow> {
    let Some(keys) = keys else { return Ok(()) };
    match exchange.purpose {
        Purpose::Register => {
            let Some(registering) = &bound.registering else { return Ok(()) };
            let asked = &registering.asked;
            let challenge = asked.challenge.as_ref().map(TextBuf::as_bytes);
            let authorization = asked.authorization.as_ref().map(TextBuf::as_bytes);
            let mark = out.len();
            let proof = header(out, RESPONSE_HEADER, |out| {
                dbsc::write_proof(keys, registering.key, Some(&registering.public), challenge, authorization, out)
                    .map_err(|_| Overflow)
            });
            if proof.is_err() {
                out.truncate(mark);
            }
            if let Some(authorization) = authorization {
                header(out, crate::consts::header::AUTHORIZATION, |out| out.put(authorization))?;
            }
        }
        Purpose::Refresh { slot, .. } => {
            let Some(session) = bound.sessions.get_mut(usize::from(slot)) else { return Ok(()) };
            header(out, SESSION_ID_HEADER, |out| sf_string(session.id.as_bytes(), out))?;
            // A challenge is used once.
            if let Some(challenge) = session.challenge.take() {
                let mark = out.len();
                let proof = header(out, RESPONSE_HEADER, |out| {
                    dbsc::write_proof(keys, session.key, None, Some(challenge.as_bytes()), None, out)
                        .map_err(|_| Overflow)
                });
                if proof.is_err() {
                    out.truncate(mark);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// A session from its instructions, given by the response to `url`: its
/// refresh address, the request's own when none is given, and its scope's
/// origin must be the responding origin's, over HTTPS (a narrowing of
/// section 9.6's same-site rule, which needs the public suffix list).
fn session_of(instructions: &Instructions, key: KeyHandle, url: &Url, seconds: u64) -> Option<Session> {
    // Site scope is refused, so a session covers its own host alone,
    // with no public suffix list (decision E.114 item 6, finding F-91).
    if !url.is_secure() || instructions.include_site {
        return None;
    }
    let origin = url.origin();
    let refresh = match &instructions.refresh_url {
        Some(given) => url.join(given.as_str()).ok()?,
        None => *url,
    };
    if refresh.origin() != origin {
        return None;
    }
    if let Some(given) = &instructions.origin
        && Url::parse(given.as_str()).ok()?.origin() != origin
    {
        return None;
    }
    Some(Session {
        id: instructions.id,
        key,
        refresh_url: TextBuf::from_bytes(refresh.as_str().as_bytes())?,
        origin,
        include_site: instructions.include_site,
        rules: instructions.rules,
        credentials: instructions.credentials,
        challenge: None,
        expires: seconds.saturating_add(SESSION_LIFETIME_SECONDS),
    })
}

impl<'m, R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'m, R, SLOTS, COOKIE_BYTES> {
    /// Gives the agent the port's keys that never leave the device, which
    /// turns on Device Bound Session Credentials (REQ-WEB-017): without
    /// them a server's request for a bound session is ignored, and
    /// sessions work as cookies alone (REQ-WEB-006).
    pub fn set_device_keys(&mut self, keys: &'m mut dyn DeviceKeys) {
        self.device_keys = Some(keys);
    }

    /// The device-bound sessions kept, for a port to list.
    #[must_use]
    pub const fn bound_sessions(&self) -> &Sessions {
        &self.bound.sessions
    }

    /// The next bound session's exchange, before any other request: a
    /// refresh, then a registration, then the request a refresh held.
    pub(super) fn bound_next(&mut self, sink: &mut Sink<'_>) -> Option<(Purpose, Url, Method, Site)> {
        if let Some((slot, retried)) = self.bound.refresh.take() {
            let session = self.bound.sessions.get(usize::from(slot))?;
            if let Ok(url) = Url::parse(session.refresh_url.as_str()) {
                return Some((Purpose::Refresh { slot, retried }, url, Method::Post, Site::SameService));
            }
        }
        if let Some(asked) = self.bound.asked.take() {
            let made = self.device_keys.as_deref_mut().and_then(DeviceKeys::create);
            match (made, Url::parse(asked.url.as_str())) {
                (Some((key, public)), Ok(url)) => {
                    self.bound.registering = Some(Registering { asked, key, public });
                    return Some((Purpose::Register, url, Method::Post, Site::SameService));
                }
                (made, _) => {
                    if let (Some((key, _)), Some(keys)) = (made, self.device_keys.as_deref_mut()) {
                        keys.delete(key);
                    }
                    event(sink, Event::BoundSession { session: "", change: BoundChange::RegistrationFailed });
                }
            }
        }
        let deferred = self.bound.deferred.take()?;
        self.bound.handled = deferred.handled;
        Some((deferred.purpose, deferred.url, deferred.method, deferred.site))
    }

    /// Holds a request back for a refresh when a session not yet refreshed
    /// for it covers its address and it would go without one of the
    /// session's cookies (sections 8.5 and 8.6). Returns true when held.
    pub(super) fn bound_defer(
        &mut self,
        now: Instant,
        purpose: Purpose,
        url: &Url,
        method: Method,
        site: Site,
    ) -> bool {
        let handled = core::mem::take(&mut self.bound.handled);
        if self.device_keys.is_none() || matches!(purpose, Purpose::Register | Purpose::Refresh { .. }) {
            return false;
        }
        let (seconds, _) = self.seconds(now);
        let Self { bound, device_keys, jar, port_cookies, buffers, .. } = self;
        bound.sessions.expire(seconds, |session| {
            if let Some(keys) = device_keys.as_deref_mut() {
                keys.delete(session.key);
            }
        });
        let mut due = None;
        for (slot, session) in bound.sessions.iter() {
            if handled & bit(slot) != 0 || !session.covers(url) {
                continue;
            }
            // The cookies this request would carry, in the request buffer,
            // which is free until the request is written.
            let mut cookies = Writer::new(buffers.fetch_request);
            let store = cookie_store(jar, port_cookies);
            let _ = store.header_for(url.host_text(), url.path(), url.is_secure(), site, seconds, &mut cookies);
            if session.missing(cookies.written()) {
                due = Some(slot);
                break;
            }
        }
        let Some(slot) = due else { return false };
        if let Some(session) = bound.sessions.get_mut(slot) {
            // Section 8.5: a session chosen for refresh lasts on.
            session.expires = seconds.saturating_add(SESSION_LIFETIME_SECONDS);
        }
        bound.deferred = Some(Deferred { purpose, url: *url, method, site, handled: handled | bit(slot) });
        bound.refresh = Some((u8::try_from(slot).unwrap_or(0), false));
        true
    }

    /// Reads a response head over HTTPS for the bound sessions: the
    /// challenges it gives (section 8.3), a registration it asks for
    /// (section 8.2), and `Clear-Site-Data`, which ends its origin's
    /// sessions (section 10.2).
    pub(super) fn bound_head(&mut self, now: Instant, exchange: &Exchange, head: &ResponseHead, sink: &mut Sink<'_>) {
        let url = &exchange.url;
        if self.device_keys.is_none() || !url.is_secure() {
            return;
        }
        let (seconds, _) = self.seconds(now);
        let origin = url.origin();
        let head_bytes = self.buffers.fetch_head.get(..self.fetch.head.len()).unwrap_or(&[]);
        let Self { bound, device_keys, .. } = self;
        http::each_field(head_bytes, CHALLENGE_HEADER, &mut |value| {
            let Some(challenge) = dbsc::read_challenge(value) else { return };
            if let Some(slot) = bound.sessions.find(&origin, challenge.session.as_bytes())
                && let Some(session) = bound.sessions.get_mut(slot)
            {
                session.challenge = Some(challenge.challenge);
            }
        });
        if head.clear_cookies {
            for slot in 0..MAX_SESSIONS {
                if bound.sessions.get(slot).is_some_and(|session| session.origin == origin)
                    && let Some(session) = bound.sessions.remove(slot)
                {
                    if let Some(keys) = device_keys.as_deref_mut() {
                        keys.delete(session.key);
                    }
                    event(sink, Event::BoundSession { session: session.id.as_str(), change: BoundChange::Ended });
                }
            }
        }
        if bound.asked.is_some() || bound.registering.is_some() || !bound.sessions.has_room() {
            return;
        }
        let mut asked = None;
        http::each_field(head_bytes, REGISTRATION_HEADER, &mut |value| {
            if asked.is_some() {
                return;
            }
            let Some(registration) = dbsc::read_registration(value) else { return };
            // The endpoint is the responding origin's own (a narrowing of
            // section 8.2, which allows any address).
            let Ok(endpoint) = url.join(registration.path.as_str()) else { return };
            if endpoint.origin() != origin {
                return;
            }
            let Some(text) = TextBuf::from_bytes(endpoint.as_str().as_bytes()) else { return };
            asked =
                Some(Asked { url: text, challenge: registration.challenge, authorization: registration.authorization });
        });
        if asked.is_some() && bound.may_register(&origin, seconds) {
            bound.asked = asked;
        }
    }

    /// Acts on a bound session's response (sections 8.8 and 8.9).
    pub(super) fn bound_complete(
        &mut self,
        now: Instant,
        exchange: &Exchange,
        head: &ResponseHead,
        sink: &mut Sink<'_>,
    ) {
        let (seconds, _) = self.seconds(now);
        let status = head.status;
        let body = self.buffers.body.get(..self.fetch.body_len).unwrap_or(&[]);
        let Self { bound, device_keys, .. } = self;
        let mut delete = |key: KeyHandle| {
            if let Some(keys) = device_keys.as_deref_mut() {
                keys.delete(key);
            }
        };
        let answered = Answered { status, body, url: &exchange.url, seconds };
        match exchange.purpose {
            Purpose::Register => bound.registered(&answered, &mut delete, sink),
            Purpose::Refresh { slot, retried } => bound.refreshed(slot, retried, &answered, &mut delete, sink),
            _ => {}
        }
    }

    /// A bound session's exchange failed on the network or by policy: a
    /// registration's key is deleted; a refresh lets its request go on.
    pub(super) fn bound_failed(&mut self, purpose: Purpose, sink: &mut Sink<'_>) {
        match purpose {
            Purpose::Register => {
                if let (Some(registering), Some(keys)) =
                    (self.bound.registering.take(), self.device_keys.as_deref_mut())
                {
                    keys.delete(registering.key);
                }
                event(sink, Event::BoundSession { session: "", change: BoundChange::RegistrationFailed });
            }
            Purpose::Refresh { slot, .. } => {
                let id = self.bound.sessions.get(usize::from(slot)).map(|session| session.id);
                let id = id.as_ref().map_or("", TextBuf::as_str);
                event(sink, Event::BoundSession { session: id, change: BoundChange::RefreshFailed });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]
    use super::*;

    /// Finding F-14 (decision E.114 item 6): an sf-string holds only
    /// `0x20..=0x7E` (RFC 9651 section 3.3.3), so a line break or a
    /// control byte can never reach a header the agent writes.
    #[test]
    fn an_sf_string_refuses_bytes_outside_printable_ascii() {
        let mut buffer = [0_u8; 32];
        let mut out = Writer::new(&mut buffer);
        sf_string(br#"a "b"\"#, &mut out).unwrap();
        assert_eq!(out.written(), br#""a \"b\"\\""#);
        for text in [&b"a\nb"[..], b"a\r", b"\0", b"\x7f", b"\xc3\xa9"] {
            let mut buffer = [0_u8; 32];
            assert_eq!(sf_string(text, &mut Writer::new(&mut buffer)), Err(Overflow), "{text:?}");
        }
    }
}
