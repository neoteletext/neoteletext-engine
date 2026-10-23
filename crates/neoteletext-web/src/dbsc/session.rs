//! The sessions an agent keeps (sections 8.5, 8.6 and 8.9): a session's
//! scope and the cookies it binds, and the bounded store of them.

use super::instructions::ScopeRule;
use super::{
    KeyHandle, MAX_CHALLENGE, MAX_CREDENTIAL_NAME, MAX_CREDENTIALS, MAX_SCOPE_RULES, MAX_SESSION_ID, MAX_SESSION_URL,
    MAX_SESSIONS,
};
use crate::bytes::TextBuf;
use crate::url::{Origin, Url};

/// One bound session.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Session {
    /// Its identifier.
    pub id: TextBuf<MAX_SESSION_ID>,
    /// The key that proves it.
    pub key: KeyHandle,
    /// Where it is refreshed.
    pub refresh_url: TextBuf<MAX_SESSION_URL>,
    /// The origin whose requests it covers, or whose site's.
    pub origin: Origin,
    /// Site scope: the origin's host and every host below it.
    pub include_site: bool,
    /// The scope rules, applied last first.
    pub rules: [Option<ScopeRule>; MAX_SCOPE_RULES],
    /// The cookies it keeps.
    pub credentials: [Option<TextBuf<MAX_CREDENTIAL_NAME>>; MAX_CREDENTIALS],
    /// The last challenge the server gave it.
    pub challenge: Option<TextBuf<MAX_CHALLENGE>>,
    /// When it lapses, in the agent's seconds.
    pub expires: u64,
}

/// True when `host` matches a rule's `domain`: `*`, `*.example.com` (any
/// host below `example.com`), or the host itself, without regard to case.
fn domain_matches(pattern: &[u8], host: &[u8]) -> bool {
    if pattern == b"*" {
        return true;
    }
    if let Some(parent) = pattern.strip_prefix(b"*.") {
        return host.len() > parent.len()
            && host
                .get(host.len().saturating_sub(parent.len())..)
                .is_some_and(|tail| tail.eq_ignore_ascii_case(parent))
            && host.get(host.len().saturating_sub(parent.len()).saturating_sub(1)) == Some(&b'.');
    }
    pattern.eq_ignore_ascii_case(host)
}

impl Session {
    /// Whether `url` is in this session's scope (section 8.5): its origin,
    /// or with site scope its host and those below, over HTTPS; then the
    /// last scope rule that matches decides, and none means in scope. The
    /// refresh address itself never is, so that a refresh is never
    /// deferred for itself.
    #[must_use]
    pub fn covers(&self, url: &Url) -> bool {
        let host = url.host_text().as_bytes();
        let within = if self.include_site {
            url.is_secure() && self.origin.secure && site_member(host, self.origin.host.as_bytes())
        } else {
            url.origin() == self.origin
        };
        if !within || url.as_str().as_bytes() == self.refresh_url.as_bytes() {
            return false;
        }
        let path = url.path().as_bytes();
        self.rules
            .iter()
            .rev()
            .flatten()
            .find(|rule| domain_matches(rule.domain.as_bytes(), host) && path.starts_with(rule.path.as_bytes()))
            .is_none_or(|rule| rule.include)
    }

    /// Whether a request carrying the cookie header `cookies` (its value,
    /// `name=value; ...`) lacks one of the session's cookies (section 8.6),
    /// so that the session must be refreshed first.
    #[must_use]
    pub fn missing(&self, cookies: &[u8]) -> bool {
        self.credentials.iter().flatten().any(|name| {
            !cookies.split(|&byte| byte == b';').any(|pair| {
                let pair = crate::bytes::trim_ows(pair);
                pair.split(|&byte| byte == b'=').next() == Some(name.as_bytes())
            })
        })
    }
}

/// True when `host` is `site` or a host below it.
fn site_member(host: &[u8], site: &[u8]) -> bool {
    if host.eq_ignore_ascii_case(site) {
        return true;
    }
    host.len() > site.len()
        && host.get(host.len().saturating_sub(site.len())..).is_some_and(|tail| tail.eq_ignore_ascii_case(site))
        && host.get(host.len().saturating_sub(site.len()).saturating_sub(1)) == Some(&b'.')
}

/// The sessions an agent keeps.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sessions {
    slots: [Option<Session>; MAX_SESSIONS],
}

impl Default for Sessions {
    fn default() -> Self {
        Self::new()
    }
}

impl Sessions {
    /// None.
    #[must_use]
    pub const fn new() -> Self {
        Self { slots: [None; MAX_SESSIONS] }
    }

    /// The sessions, by slot.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &Session)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| slot.as_ref().map(|session| (index, session)))
    }

    /// The session in `slot`.
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<&Session> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    /// The session in `slot`, to change.
    pub fn get_mut(&mut self, slot: usize) -> Option<&mut Session> {
        self.slots.get_mut(slot).and_then(Option::as_mut)
    }

    /// The slot of the session `id` of `origin`.
    #[must_use]
    pub fn find(&self, origin: &Origin, id: &[u8]) -> Option<usize> {
        self.iter().find(|(_, session)| session.origin == *origin && session.id.as_bytes() == id).map(|(slot, _)| slot)
    }

    /// True when a new session could be kept.
    #[must_use]
    pub fn has_room(&self) -> bool {
        self.slots.iter().any(Option::is_none)
    }

    /// Keeps `session`, replacing one of the same origin and identifier;
    /// gives back the key of the session it replaced, or as the error the
    /// new session's own key when there is no room, for the caller to
    /// delete.
    pub fn insert(&mut self, session: Session) -> Result<Option<KeyHandle>, KeyHandle> {
        if let Some(slot) = self.find(&session.origin, session.id.as_bytes()) {
            let old = self.slots.get_mut(slot).and_then(|slot| slot.replace(session));
            return Ok(old.map(|old| old.key).filter(|&key| key != session.key));
        }
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) else { return Err(session.key) };
        *slot = Some(session);
        Ok(None)
    }

    /// Ends the session in `slot`, giving it back so its key can be
    /// deleted.
    pub fn remove(&mut self, slot: usize) -> Option<Session> {
        self.slots.get_mut(slot).and_then(Option::take)
    }

    /// Ends every session past its time, calling `ended` with each.
    pub fn expire(&mut self, now: u64, mut ended: impl FnMut(Session)) {
        for slot in &mut self.slots {
            if slot.is_some_and(|session| session.expires <= now)
                && let Some(session) = slot.take()
            {
                ended(session);
            }
        }
    }
}
