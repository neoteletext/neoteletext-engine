//! The cookie store (REQ-WEB-006, REQ-SEC-013), following the RFC 6265bis
//! draft of 30 September 2026 as browsers implement it (decision E.35),
//! made stricter where the single-origin model allows:
//!
//! - every cookie is host-only: `Domain` is ignored (REQ-WEB-015);
//! - `Secure` decisions are asked of the security policy;
//! - there is no third-party context at all.
//!
//! Cookies live in memory the caller provides: `SLOTS` cookie records and
//! an arena of `BYTES` bytes for their host, name, value and path. Values
//! are opaque and never interpreted.

mod date;
mod header;
mod persist;
mod store;

pub use date::parse_cookie_date;

use crate::bytes::Writer;
use crate::consts::{DEFAULT_COOKIES_PER_HOST, DEFAULT_COOKIES_TOTAL, MAX_COOKIE_AGE_DAYS, SECONDS_PER_DAY};

/// The longest lifetime in seconds (REQ-WEB-006).
const MAX_AGE_SECONDS: u64 = MAX_COOKIE_AGE_DAYS * SECONDS_PER_DAY;

/// The `SameSite` attribute as stored (REQ-WEB-006).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SameSite {
    /// `Strict`.
    Strict,
    /// `Lax`.
    Lax,
    /// `None`.
    None,
    /// Absent or unrecognised: enforced as `Lax`.
    Default,
}

/// The kind of request cookies are being chosen for (REQ-WEB-006).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Site {
    /// A request within the service being viewed: all matching cookies.
    SameService,
    /// A top-level GET to a different service, made by following a link:
    /// `Lax`, default and `None` cookies, not `Strict`.
    CrossServiceNavigation,
}

/// Why a `Set-Cookie` header was not stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CookieRejected {
    /// A control character other than horizontal tab.
    ControlCharacter,
    /// Name and value both empty.
    Empty,
    /// Name and value together over 4096 bytes.
    TooLarge,
    /// `Secure` over plain HTTP.
    SecureOverPlain,
    /// Plain HTTP may not replace or delete a `Secure` cookie.
    WouldReplaceSecure,
    /// The name's prefix rules were broken.
    Prefix,
    /// `SameSite=None` without `Secure` (RFC 6265bis section 5.7).
    SameSiteNoneNotSecure,
    /// The cookie was already expired; any stored copy was deleted.
    Expired,
    /// There is no room even after evicting every other cookie.
    NoRoom,
    /// This build keeps no cookies (the `cookies` feature, decision E.61).
    NotCompiledIn,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(clippy::struct_excessive_bools)] // Each flag is one cookie attribute of REQ-WEB-006.
struct Record {
    start: u32,
    host_len: u16,
    name_len: u16,
    value_len: u16,
    path_len: u16,
    /// Expiry in seconds of the jar's clock; `None` for a session cookie.
    expires: Option<u64>,
    secure: bool,
    http_only: bool,
    partitioned: bool,
    same_site: SameSite,
    /// Set over an encrypted connection: the host has been reached over
    /// HTTPS (REQ-SEC-007).
    via_secure: bool,
    created: u64,
    last_access: u64,
    /// When it was first stored and last sent, in seconds of the jar's
    /// clock (decision E.104): for a port that lists cookies or clears
    /// those of a time range.
    created_at: u64,
    used_at: u64,
}

impl Record {
    fn total_len(&self) -> usize {
        usize::from(self.host_len)
            .saturating_add(usize::from(self.name_len))
            .saturating_add(usize::from(self.value_len))
            .saturating_add(usize::from(self.path_len))
    }

    fn expired(&self, now: u64) -> bool {
        self.expires.is_some_and(|expires| expires <= now)
    }
}

/// The parts of one stored cookie.
struct Parts<'a> {
    host: &'a [u8],
    name: &'a [u8],
    value: &'a [u8],
    path: &'a [u8],
}

/// One stored cookie as a port may see it (decision E.104): which host it
/// belongs to, its size, its times and whether it lasts beyond the session.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CookieInfo<'a> {
    /// The host that set it.
    pub host: &'a [u8],
    /// The bytes it holds: host, name, value and path.
    pub bytes: usize,
    /// When it was first stored, in seconds of the jar's clock.
    pub created_at: u64,
    /// When it was last sent, or stored when never sent.
    pub used_at: u64,
    /// When it expires; `None` for a session cookie.
    pub expires: Option<u64>,
    /// It was set over an encrypted connection.
    pub via_secure: bool,
}

/// Where a user agent keeps its cookies (decision E.104). The engine's
/// [`CookieJar`] is one; a port may keep them elsewhere, as a browser that
/// holds them in a more trusted process than the one that speaks to the
/// network does, and give the agent this interface instead
/// (`UserAgent::with_cookies`). Each method is the jar's method of the
/// same name. It has no `Debug` supertrait: a trait object's table would
/// carry `core::fmt` into every build (REQ-GEN-016).
pub trait Cookies {
    /// Writes the `Cookie` header value for a request: see
    /// [`CookieJar::header_for`].
    fn header_for(
        &mut self,
        host: &str,
        path: &str,
        secure: bool,
        site: Site,
        now: u64,
        out: &mut Writer<'_>,
    ) -> (usize, bool);
    /// Processes one `Set-Cookie` header value: see
    /// [`CookieJar::store_within`].
    #[allow(clippy::too_many_arguments, reason = "the arguments of `CookieJar::store_within`")]
    fn store_within(
        &mut self,
        header: &[u8],
        host: &str,
        request_path: &str,
        scope: Option<&str>,
        secure: bool,
        now: u64,
        wall_clock: bool,
    ) -> Result<(), CookieRejected>;
    /// See [`CookieJar::reached_securely`].
    fn reached_securely(&self, host: &str, now: u64) -> bool;
    /// A response from `host` over an encrypted connection said
    /// `Clear-Site-Data: "cookies"`: see [`CookieJar::clear_host`].
    fn clear_host(&mut self, host: &str);
    /// Erases every cookie: see [`CookieJar::wipe`].
    fn wipe(&mut self);
}

impl<const SLOTS: usize, const BYTES: usize> Cookies for CookieJar<SLOTS, BYTES> {
    fn header_for(
        &mut self,
        host: &str,
        path: &str,
        secure: bool,
        site: Site,
        now: u64,
        out: &mut Writer<'_>,
    ) -> (usize, bool) {
        Self::header_for(self, host, path, secure, site, now, out)
    }

    fn store_within(
        &mut self,
        header: &[u8],
        host: &str,
        request_path: &str,
        scope: Option<&str>,
        secure: bool,
        now: u64,
        wall_clock: bool,
    ) -> Result<(), CookieRejected> {
        Self::store_within(self, header, host, request_path, scope, secure, now, wall_clock)
    }

    fn reached_securely(&self, host: &str, now: u64) -> bool {
        Self::reached_securely(self, host, now)
    }

    fn clear_host(&mut self, host: &str) {
        Self::clear_host(self, host);
    }

    fn wipe(&mut self) {
        Self::wipe(self);
    }
}

/// The cookie store, with `SLOTS` records and `BYTES` bytes of text.
#[derive(Clone, Debug)]
pub struct CookieJar<const SLOTS: usize, const BYTES: usize> {
    records: [Option<Record>; SLOTS],
    arena: [u8; BYTES],
    used: usize,
    tick: u64,
    per_host: usize,
    total: usize,
}

impl<const SLOTS: usize, const BYTES: usize> Default for CookieJar<SLOTS, BYTES> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SLOTS: usize, const BYTES: usize> CookieJar<SLOTS, BYTES> {
    /// An empty jar with the default limits of REQ-WEB-006, reduced to
    /// `SLOTS` when that is smaller.
    #[must_use]
    pub const fn new() -> Self {
        let total = if DEFAULT_COOKIES_TOTAL < SLOTS { DEFAULT_COOKIES_TOTAL } else { SLOTS };
        let per_host = if DEFAULT_COOKIES_PER_HOST < total { DEFAULT_COOKIES_PER_HOST } else { total };
        Self { records: [None; SLOTS], arena: [0; BYTES], used: 0, tick: 0, per_host, total }
    }

    /// Sets the per-host and total limits (REQ-WEB-006), each at most
    /// `SLOTS`.
    pub fn set_limits(&mut self, per_host: usize, total: usize) {
        self.total = total.min(SLOTS);
        self.per_host = per_host.min(self.total);
    }

    /// Erases every cookie (REQ-SEC-013).
    pub fn wipe(&mut self) {
        self.records = [None; SLOTS];
        self.arena = [0; BYTES];
        self.used = 0;
    }

    /// Each cookie held, expired ones included until evicted (decision
    /// E.104).
    pub fn cookies(&self) -> impl Iterator<Item = CookieInfo<'_>> + '_ {
        self.records.iter().flatten().map(|record| self.info(record))
    }

    fn info<'j>(&'j self, record: &Record) -> CookieInfo<'j> {
        CookieInfo {
            host: self.parts(record).host,
            bytes: record.total_len(),
            created_at: record.created_at,
            used_at: record.used_at,
            expires: record.expires,
            via_secure: record.via_secure,
        }
    }

    /// Removes every cookie `selected` chooses (decision E.104). Returns
    /// how many went.
    pub fn remove_where(&mut self, selected: &dyn Fn(&CookieInfo<'_>) -> bool) -> usize {
        let mut removed = 0usize;
        let mut index = 0usize;
        while index < SLOTS {
            let chosen = self.records.get(index).copied().flatten().is_some_and(|record| selected(&self.info(&record)));
            if chosen {
                self.remove(index);
                removed = removed.saturating_add(1);
            }
            index = index.saturating_add(1);
        }
        removed
    }

    /// Removes every cookie of `host`: `Clear-Site-Data` naming
    /// `"cookies"` (W3C Clear Site Data section 4.2), for the host the jar
    /// is the store of (decision E.104).
    pub fn clear_host(&mut self, host: &str) {
        self.remove_where(&|cookie| cookie.host == host.as_bytes());
    }

    /// The number of cookies held, expired ones included until evicted.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.iter().flatten().count()
    }

    /// True when no cookies are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.iter().all(Option::is_none)
    }

    fn next_tick(&mut self) -> u64 {
        self.tick = self.tick.wrapping_add(1);
        self.tick
    }

    fn parts(&self, record: &Record) -> Parts<'_> {
        let start = record.start as usize;
        let host_end = start.saturating_add(usize::from(record.host_len));
        let name_end = host_end.saturating_add(usize::from(record.name_len));
        let value_end = name_end.saturating_add(usize::from(record.value_len));
        let path_end = value_end.saturating_add(usize::from(record.path_len));
        Parts {
            host: self.arena.get(start..host_end).unwrap_or(&[]),
            name: self.arena.get(host_end..name_end).unwrap_or(&[]),
            value: self.arena.get(name_end..value_end).unwrap_or(&[]),
            path: self.arena.get(value_end..path_end).unwrap_or(&[]),
        }
    }

    /// Removes a record and compacts the arena behind it.
    fn remove(&mut self, index: usize) {
        let Some(record) = self.records.get_mut(index).and_then(Option::take) else {
            return;
        };
        let start = record.start as usize;
        let len = record.total_len();
        let end = start.saturating_add(len);
        if let Some(region) = self.arena.get_mut(start..self.used) {
            region.copy_within(len.min(region.len()).., 0);
        }
        self.used = self.used.saturating_sub(len);
        let shift = u32::try_from(len).unwrap_or(0);
        for other in self.records.iter_mut().flatten() {
            if other.start as usize >= end {
                other.start = other.start.saturating_sub(shift);
            }
        }
    }

    /// Finds the record with the same name, host and path.
    fn find(&self, host: &[u8], name: &[u8], path: &[u8]) -> Option<usize> {
        self.records.iter().position(|slot| {
            slot.as_ref().is_some_and(|record| {
                let parts = self.parts(record);
                parts.host == host && parts.name == name && parts.path == path
            })
        })
    }

    /// The index of the cookie to evict among those `selected`: an expired
    /// one first, then the least recently used.
    fn victim(&self, now: u64, selected: &dyn Fn(&Parts<'_>) -> bool) -> Option<usize> {
        let mut best: Option<(bool, u64, usize)> = None;
        for (index, slot) in self.records.iter().enumerate() {
            let Some(record) = slot else { continue };
            if !selected(&self.parts(record)) {
                continue;
            }
            // Expired cookies sort before live ones, then by last access.
            let key = (!record.expired(now), record.last_access, index);
            if best.is_none_or(|current| key < current) {
                best = Some(key);
            }
        }
        best.map(|(_, _, index)| index)
    }

    fn count_where(&self, selected: &dyn Fn(&Parts<'_>) -> bool) -> usize {
        self.records.iter().flatten().filter(|record| selected(&self.parts(record))).count()
    }

    /// Evicts until a cookie of `needed` bytes for `host` fits within the
    /// per-host limit, the total limit, the slots and the arena.
    fn make_room(&mut self, host: &[u8], needed: usize, now: u64) -> Result<(), CookieRejected> {
        if needed > BYTES {
            return Err(CookieRejected::NoRoom);
        }
        while self.count_where(&|parts| parts.host == host) >= self.per_host {
            let index = self.victim(now, &|parts| parts.host == host).ok_or(CookieRejected::NoRoom)?;
            self.remove(index);
        }
        while self.len() >= self.total || BYTES.saturating_sub(self.used) < needed {
            let index = self.victim(now, &|_| true).ok_or(CookieRejected::NoRoom)?;
            self.remove(index);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
