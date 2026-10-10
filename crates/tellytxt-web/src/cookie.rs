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

use tellytxt_core::policy;

use crate::bytes::{Overflow, Writer, parse_decimal, starts_with_ignore_case, trim_ows};
use crate::consts::{
    DEFAULT_COOKIES_PER_HOST, DEFAULT_COOKIES_TOTAL, MAX_COOKIE_AGE_DAYS, MAX_COOKIE_ATTRIBUTE_VALUE,
    MAX_COOKIE_NAME_VALUE, PREFIX_HOST, PREFIX_HOST_HTTP, PREFIX_HTTP, PREFIX_SECURE, SECONDS_PER_DAY,
    cookie_attribute as attribute,
};

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
    /// A form submission to a different origin: only cookies that are
    /// `SameSite=None` and `Secure`.
    CrossOriginForm,
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

/// The attributes parsed from one `Set-Cookie` header.
#[allow(clippy::struct_excessive_bools)] // Each flag is one cookie attribute of REQ-WEB-006.
struct Attributes<'a> {
    max_age: Option<i64>,
    expires: Option<u64>,
    domain: bool,
    path: Option<&'a [u8]>,
    secure: bool,
    http_only: bool,
    partitioned: bool,
    same_site: SameSite,
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

    /// Processes one `Set-Cookie` header value received from `host` for a
    /// request to `request_path` (RFC 6265bis sections 5.6 and 5.7).
    /// `now` is in seconds; `wall_clock` says whether `now` is calendar
    /// time, without which an `Expires` date cannot be compared and the
    /// attribute is ignored.
    pub fn store(
        &mut self,
        header: &[u8],
        host: &str,
        request_path: &str,
        secure: bool,
        now: u64,
        wall_clock: bool,
    ) -> Result<(), CookieRejected> {
        // Section 5.6 step 1: control characters other than tab.
        if header.iter().any(|&b| (b < 0x20 && b != b'\t') || b == 0x7F) {
            return Err(CookieRejected::ControlCharacter);
        }
        let (pair, attributes_text) = match header.iter().position(|&b| b == b';') {
            Some(index) => (header.get(..index).unwrap_or(&[]), header.get(index..).unwrap_or(&[])),
            None => (header, &[][..]),
        };
        let (name, value) = match pair.iter().position(|&b| b == b'=') {
            Some(index) => (
                trim_ows(pair.get(..index).unwrap_or(&[])),
                trim_ows(pair.get(index.saturating_add(1)..).unwrap_or(&[])),
            ),
            None => (&[][..], trim_ows(pair)),
        };
        if name.is_empty() && value.is_empty() {
            return Err(CookieRejected::Empty);
        }
        if name.len().saturating_add(value.len()) > MAX_COOKIE_NAME_VALUE {
            return Err(CookieRejected::TooLarge);
        }
        let attributes = parse_attributes(attributes_text, now, wall_clock);

        if !policy::may_send_cookie(attributes.secure, secure) {
            return Err(CookieRejected::SecureOverPlain);
        }
        check_prefixes(name, value, &attributes, secure)?;
        if attributes.same_site == SameSite::None && !attributes.secure {
            return Err(CookieRejected::SameSiteNoneNotSecure);
        }

        let mut default_path = [0u8; crate::consts::MAX_URL_LENGTH];
        let path = if let Some(path) = attributes.path {
            path
        } else {
            let derived = default_cookie_path(request_path.as_bytes());
            let target = default_path.get_mut(..derived.len()).unwrap_or(&mut []);
            target.copy_from_slice(derived);
            default_path.get(..derived.len()).unwrap_or(b"/")
        };
        let host = host.as_bytes();

        // Section 5.7 step 16: plain HTTP cannot touch a Secure cookie of
        // the same name whose path covers this one.
        if !secure {
            let shadowed = self.records.iter().flatten().any(|record| {
                let parts = self.parts(record);
                record.secure && parts.name == name && parts.host == host && path_matches(path, parts.path)
            });
            if shadowed {
                return Err(CookieRejected::WouldReplaceSecure);
            }
        }

        // Max-Age takes precedence over Expires; both are capped at 400 days.
        let expires = match (attributes.max_age, attributes.expires) {
            (Some(seconds), _) => Some(if seconds <= 0 {
                0
            } else {
                now.saturating_add(u64::try_from(seconds).unwrap_or(u64::MAX).min(MAX_AGE_SECONDS))
            }),
            (None, Some(date)) => Some(date.min(now.saturating_add(MAX_AGE_SECONDS))),
            (None, None) => None,
        };

        let existing = self.find(host, name, path);
        let created = match existing {
            Some(index) => {
                let created = self.records.get(index).copied().flatten().map(|record| record.created);
                self.remove(index);
                created
            }
            None => None,
        };
        if expires.is_some_and(|expires| expires <= now) {
            return Err(CookieRejected::Expired);
        }

        let needed = host.len().saturating_add(name.len()).saturating_add(value.len()).saturating_add(path.len());
        self.make_room(host, needed, now)?;
        let start = self.used;
        for part in [host, name, value, path] {
            let end = self.used.saturating_add(part.len());
            let target = self.arena.get_mut(self.used..end).ok_or(CookieRejected::NoRoom)?;
            target.copy_from_slice(part);
            self.used = end;
        }
        let tick = self.next_tick();
        let narrow = |len: usize| u16::try_from(len).map_err(|_| CookieRejected::TooLarge);
        let record = Record {
            start: u32::try_from(start).map_err(|_| CookieRejected::NoRoom)?,
            host_len: narrow(host.len())?,
            name_len: narrow(name.len())?,
            value_len: narrow(value.len())?,
            path_len: narrow(path.len())?,
            expires,
            secure: attributes.secure,
            http_only: attributes.http_only,
            partitioned: attributes.partitioned,
            same_site: attributes.same_site,
            via_secure: secure,
            created: created.unwrap_or(tick),
            last_access: tick,
        };
        let slot = self.records.iter_mut().find(|slot| slot.is_none()).ok_or(CookieRejected::NoRoom)?;
        *slot = Some(record);
        Ok(())
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

    /// Writes the `Cookie` header value for a request (RFC 6265bis section
    /// 5.8.3): `name=value` pairs joined by `; `, longest path first, then
    /// oldest first. Returns the number of cookies written, and whether
    /// some did not fit in `out`.
    pub fn header_for(
        &mut self,
        host: &str,
        path: &str,
        secure: bool,
        site: Site,
        now: u64,
        out: &mut Writer<'_>,
    ) -> (usize, bool) {
        let mut order = [0usize; SLOTS];
        let mut count = 0usize;
        for (index, slot) in self.records.iter().enumerate() {
            let Some(record) = slot else { continue };
            let parts = self.parts(record);
            let wanted = parts.host == host.as_bytes()
                && path_matches(path.as_bytes(), parts.path)
                && policy::may_send_cookie(record.secure, secure)
                && !record.expired(now)
                && match site {
                    Site::SameService => true,
                    Site::CrossServiceNavigation => record.same_site != SameSite::Strict,
                    Site::CrossOriginForm => record.same_site == SameSite::None && record.secure,
                };
            if wanted && let Some(entry) = order.get_mut(count) {
                *entry = index;
                count = count.saturating_add(1);
            }
        }
        let selected = order.get_mut(..count).unwrap_or(&mut []);
        // Insertion sort: at most SLOTS entries.
        for i in 1..selected.len() {
            let mut j = i;
            while j > 0 {
                let (Some(&a), Some(&b)) = (selected.get(j.saturating_sub(1)), selected.get(j)) else { break };
                if self.sort_key(b) < self.sort_key(a) {
                    selected.swap(j.saturating_sub(1), j);
                    j = j.saturating_sub(1);
                } else {
                    break;
                }
            }
        }
        let mut written = 0usize;
        let mut truncated = false;
        let tick = self.next_tick();
        for &index in selected.iter() {
            let Some(record) = self.records.get(index).copied().flatten() else { continue };
            let mark = out.len();
            let parts = self.parts(&record);
            let result = (|| -> Result<(), Overflow> {
                if written > 0 {
                    out.put(b"; ")?;
                }
                if !parts.name.is_empty() {
                    out.put(parts.name)?;
                    out.byte(b'=')?;
                }
                out.put(parts.value)
            })();
            if result.is_err() {
                out.truncate(mark);
                truncated = true;
                continue;
            }
            written = written.saturating_add(1);
            if let Some(Some(stored)) = self.records.get_mut(index) {
                stored.last_access = tick;
            }
        }
        (written, truncated)
    }

    fn sort_key(&self, index: usize) -> (core::cmp::Reverse<u16>, u64) {
        self.records
            .get(index)
            .copied()
            .flatten()
            .map_or((core::cmp::Reverse(0), u64::MAX), |record| (core::cmp::Reverse(record.path_len), record.created))
    }

    /// True when a cookie for `host` was set over an encrypted connection
    /// and is still live: the host has been reached over HTTPS "for as long
    /// as its cookies last" (REQ-SEC-007).
    #[must_use]
    pub fn reached_securely(&self, host: &str, now: u64) -> bool {
        self.records
            .iter()
            .flatten()
            .any(|record| record.via_secure && !record.expired(now) && self.parts(record).host == host.as_bytes())
    }

    /// Writes the cookies that may be saved (REQ-SEC-013): only those with
    /// an expiry that has not passed. Each record is one line:
    /// `expires flags host-length name-length value-length path-length`
    /// in decimal, a space, then the four texts back to back.
    pub fn export(&self, now: u64, out: &mut Writer<'_>) -> Result<usize, Overflow> {
        out.put(EXPORT_VERSION)?;
        let mut count = 0usize;
        for record in self.records.iter().flatten() {
            let Some(expires) = record.expires.filter(|&expires| expires > now) else { continue };
            let parts = self.parts(record);
            out.decimal(expires)?;
            out.byte(b' ')?;
            out.decimal(u64::from(encode_flags(record)))?;
            for len in [parts.host.len(), parts.name.len(), parts.value.len(), parts.path.len()] {
                out.byte(b' ')?;
                out.decimal(len as u64)?;
            }
            out.byte(b' ')?;
            for part in [parts.host, parts.name, parts.value, parts.path] {
                out.put(part)?;
            }
            out.byte(b'\n')?;
            count = count.saturating_add(1);
        }
        Ok(count)
    }

    /// Reads cookies written by [`Self::export`]. The saved data is treated
    /// as untrusted: every record is checked, and the first malformed one
    /// ends the import. Returns the number of cookies restored.
    pub fn import(&mut self, data: &[u8], now: u64) -> usize {
        let Some(mut rest) = data.strip_prefix(EXPORT_VERSION) else { return 0 };
        let mut restored = 0usize;
        while !rest.is_empty() {
            let Some((record, after)) = self.import_one(rest, now) else { break };
            rest = after;
            restored = restored.saturating_add(usize::from(record));
        }
        restored
    }

    fn import_one<'d>(&mut self, data: &'d [u8], now: u64) -> Option<(bool, &'d [u8])> {
        let mut fields = [0u64; 6];
        let mut rest = data;
        for field in &mut fields {
            let end = rest.iter().position(|&b| b == b' ')?;
            *field = parse_decimal(rest.get(..end)?, crate::bytes::MAX_U64_DIGITS)?;
            rest = rest.get(end.saturating_add(1)..)?;
        }
        let [expires, flags, host_len, name_len, value_len, path_len] = fields;
        let lens = [host_len, name_len, value_len, path_len].map(|len| usize::try_from(len).unwrap_or(usize::MAX));
        let total = lens.iter().try_fold(0usize, |sum, &len| sum.checked_add(len))?;
        let texts = rest.get(..total)?;
        rest = rest.get(total..)?.strip_prefix(b"\n")?;
        let [host_len, name_len, value_len, _] = lens;
        let host = texts.get(..host_len)?;
        let name = texts.get(host_len..host_len.checked_add(name_len)?)?;
        let value_start = host_len.checked_add(name_len)?;
        let value = texts.get(value_start..value_start.checked_add(value_len)?)?;
        let path = texts.get(value_start.checked_add(value_len)?..)?;
        let host = core::str::from_utf8(host).ok()?;
        if crate::url::Url::parse_host_only(host).is_err() || path.first() != Some(&b'/') {
            return None;
        }
        // Rebuild the header and store it through the normal rules, so a
        // tampered file cannot create a cookie the rules would refuse.
        let mut header = [0u8; MAX_COOKIE_NAME_VALUE + MAX_COOKIE_ATTRIBUTE_VALUE + 128];
        let mut writer = Writer::new(&mut header);
        let flags = u8::try_from(flags).ok()?;
        let lifetime = expires.checked_sub(now)?;
        let built = (|| -> Result<(), Overflow> {
            if !name.is_empty() {
                writer.put(name)?;
            }
            writer.byte(b'=')?;
            writer.put(value)?;
            writer.put(b"; Path=")?;
            writer.put(path)?;
            writer.put(b"; Max-Age=")?;
            writer.decimal(lifetime)?;
            decode_flags(flags, &mut writer)
        })();
        built.ok()?;
        let secure = flags & FLAG_VIA_SECURE != 0;
        let stored = self.store(writer.written(), host, "/", secure, now, false).is_ok();
        Some((stored, rest))
    }
}

/// The first line of exported cookie data.
const EXPORT_VERSION: &[u8] = b"tellytxt-cookies 1\n";
const FLAG_SECURE: u8 = 1;
const FLAG_HTTP_ONLY: u8 = 2;
const FLAG_PARTITIONED: u8 = 4;
const FLAG_VIA_SECURE: u8 = 8;
const FLAG_SAME_SITE_SHIFT: u32 = 4;

fn encode_flags(record: &Record) -> u8 {
    let same_site = match record.same_site {
        SameSite::Default => 0,
        SameSite::Strict => 1,
        SameSite::Lax => 2,
        SameSite::None => 3,
    };
    let flag = |set: bool, bit: u8| if set { bit } else { 0 };
    flag(record.secure, FLAG_SECURE)
        | flag(record.http_only, FLAG_HTTP_ONLY)
        | flag(record.partitioned, FLAG_PARTITIONED)
        | flag(record.via_secure, FLAG_VIA_SECURE)
        | (same_site << FLAG_SAME_SITE_SHIFT)
}

fn decode_flags(flags: u8, writer: &mut Writer<'_>) -> Result<(), Overflow> {
    if flags & FLAG_SECURE != 0 {
        writer.put(b"; Secure")?;
    }
    if flags & FLAG_HTTP_ONLY != 0 {
        writer.put(b"; HttpOnly")?;
    }
    if flags & FLAG_PARTITIONED != 0 {
        writer.put(b"; Partitioned")?;
    }
    match flags >> FLAG_SAME_SITE_SHIFT {
        1 => writer.put(b"; SameSite=Strict"),
        2 => writer.put(b"; SameSite=Lax"),
        3 => writer.put(b"; SameSite=None"),
        _ => Ok(()),
    }
}

/// The prefix rules of REQ-WEB-006 (RFC 6265bis section 4.1.3), checked
/// without regard to letter case. A nameless cookie whose value starts with
/// a prefix is refused too, so it cannot pose as a prefixed cookie (RFC
/// 6265bis section 5.7 step 19).
fn check_prefixes(name: &[u8], value: &[u8], attributes: &Attributes<'_>, secure: bool) -> Result<(), CookieRejected> {
    let prefixes = [PREFIX_SECURE, PREFIX_HOST, PREFIX_HTTP, PREFIX_HOST_HTTP];
    if name.is_empty() && prefixes.iter().any(|prefix| starts_with_ignore_case(value, prefix)) {
        return Err(CookieRejected::Prefix);
    }
    let secure_rule = attributes.secure && secure;
    let host_rule = secure_rule && attributes.path == Some(b"/".as_slice()) && !attributes.domain;
    let http_rule = secure_rule && attributes.http_only;
    let ok = if starts_with_ignore_case(name, PREFIX_HOST_HTTP) {
        host_rule && http_rule
    } else if starts_with_ignore_case(name, PREFIX_HOST) {
        host_rule
    } else if starts_with_ignore_case(name, PREFIX_HTTP) {
        http_rule
    } else if starts_with_ignore_case(name, PREFIX_SECURE) {
        secure_rule
    } else {
        true
    };
    if ok { Ok(()) } else { Err(CookieRejected::Prefix) }
}

/// Parses the attributes after the name-value pair (RFC 6265bis section
/// 5.6 steps 6 to 9). Attributes whose value is over 1024 bytes are
/// ignored.
fn parse_attributes(text: &[u8], now: u64, wall_clock: bool) -> Attributes<'_> {
    let mut attributes = Attributes {
        max_age: None,
        expires: None,
        domain: false,
        path: None,
        secure: false,
        http_only: false,
        partitioned: false,
        same_site: SameSite::Default,
    };
    for item in text.split(|&b| b == b';').skip(1) {
        let (name, value) = match item.iter().position(|&b| b == b'=') {
            Some(index) => (
                trim_ows(item.get(..index).unwrap_or(&[])),
                trim_ows(item.get(index.saturating_add(1)..).unwrap_or(&[])),
            ),
            None => (trim_ows(item), &[][..]),
        };
        if value.len() > MAX_COOKIE_ATTRIBUTE_VALUE {
            continue;
        }
        if name.eq_ignore_ascii_case(attribute::EXPIRES) {
            // Without calendar time an Expires date cannot be compared.
            if wall_clock && let Some(date) = parse_cookie_date(value) {
                attributes.expires = Some(date);
            }
        } else if name.eq_ignore_ascii_case(attribute::MAX_AGE) {
            if let Some(seconds) = parse_max_age(value) {
                attributes.max_age = Some(seconds);
            }
        } else if name.eq_ignore_ascii_case(attribute::DOMAIN) {
            // Ignored: every cookie is host-only (REQ-WEB-006). Its presence
            // still breaks the __Host- rule.
            attributes.domain = true;
        } else if name.eq_ignore_ascii_case(attribute::PATH) {
            attributes.path = (value.first() == Some(&b'/')).then_some(value);
        } else if name.eq_ignore_ascii_case(attribute::SECURE) {
            attributes.secure = true;
        } else if name.eq_ignore_ascii_case(attribute::HTTP_ONLY) {
            attributes.http_only = true;
        } else if name.eq_ignore_ascii_case(attribute::PARTITIONED) {
            attributes.partitioned = true;
        } else if name.eq_ignore_ascii_case(attribute::SAME_SITE) {
            attributes.same_site = if value.eq_ignore_ascii_case(attribute::STRICT) {
                SameSite::Strict
            } else if value.eq_ignore_ascii_case(attribute::LAX) {
                SameSite::Lax
            } else if value.eq_ignore_ascii_case(attribute::NONE) {
                SameSite::None
            } else {
                SameSite::Default
            };
        }
    }
    // `now` is used only through the date comparison done by the caller.
    let _ = now;
    attributes
}

/// RFC 6265bis section 5.6.2: an optional minus sign then digits.
fn parse_max_age(value: &[u8]) -> Option<i64> {
    let (negative, digits) = match value.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, value),
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    // Any value too long to parse is far beyond the 400-day cap.
    let magnitude = parse_decimal(digits, 18).map_or(i64::MAX, |v| i64::try_from(v).unwrap_or(i64::MAX));
    Some(if negative { magnitude.saturating_neg() } else { magnitude })
}

/// RFC 6265bis section 5.1.4: the directory of the request path.
fn default_cookie_path(request_path: &[u8]) -> &[u8] {
    if request_path.first() != Some(&b'/') {
        return b"/";
    }
    match request_path.iter().rposition(|&b| b == b'/') {
        Some(0) | None => b"/",
        Some(last) => request_path.get(..last).unwrap_or(b"/"),
    }
}

/// RFC 6265bis section 5.1.4: path-match.
fn path_matches(request_path: &[u8], cookie_path: &[u8]) -> bool {
    if request_path == cookie_path {
        return true;
    }
    request_path.starts_with(cookie_path)
        && (cookie_path.last() == Some(&b'/') || request_path.get(cookie_path.len()) == Some(&b'/'))
}

/// True for a delimiter of the cookie date grammar (RFC 6265bis section
/// 5.1.1).
const fn is_date_delimiter(byte: u8) -> bool {
    matches!(byte, 0x09 | 0x20..=0x2F | 0x3B..=0x40 | 0x5B..=0x60 | 0x7B..=0x7E)
}

/// Splits leading digits (at most `max`, at least `min`) from a token
/// whose remainder must not start with a digit.
fn leading_digits(token: &[u8], min: usize, max: usize) -> Option<(u64, &[u8])> {
    let count = token.iter().take_while(|b| b.is_ascii_digit()).count();
    if count < min || count > max {
        return None;
    }
    Some((parse_decimal(token.get(..count)?, max)?, token.get(count..)?))
}

const MONTHS: [&[u8; 3]; 12] =
    [b"jan", b"feb", b"mar", b"apr", b"may", b"jun", b"jul", b"aug", b"sep", b"oct", b"nov", b"dec"];

/// The cookie date algorithm (RFC 6265bis section 5.1.1), giving seconds
/// since 1970. Dates before 1970 give 0, which is always in the past.
#[must_use]
pub fn parse_cookie_date(text: &[u8]) -> Option<u64> {
    let mut time: Option<(u64, u64, u64)> = None;
    let mut day: Option<u64> = None;
    let mut month: Option<u64> = None;
    let mut year: Option<u64> = None;
    for token in text.split(|&b| is_date_delimiter(b)).filter(|token| !token.is_empty()) {
        if time.is_none()
            && let Some(parsed) = parse_time_token(token)
        {
            time = Some(parsed);
            continue;
        }
        if day.is_none()
            && let Some((value, _)) = leading_digits(token, 1, 2)
        {
            day = Some(value);
            continue;
        }
        if month.is_none()
            && let Some(index) =
                MONTHS.iter().position(|name| token.get(..3).is_some_and(|head| head.eq_ignore_ascii_case(*name)))
        {
            month = Some((index as u64).saturating_add(1));
            continue;
        }
        if year.is_none()
            && let Some((value, _)) = leading_digits(token, 2, 4)
        {
            year = Some(value);
        }
    }
    let (hour, minute, second) = time?;
    let (day, month, mut year) = (day?, month?, year?);
    // Section 5.1.1 steps 3 and 4: two-digit years.
    if (70..=99).contains(&year) {
        year = year.saturating_add(1900);
    } else if year <= 69 {
        year = year.saturating_add(2000);
    }
    if !(1..=31).contains(&day) || year < 1601 || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    if day > days_in_month(year, month) {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    let time_of_day = hour
        .saturating_mul(SECONDS_PER_HOUR)
        .saturating_add(minute.saturating_mul(SECONDS_PER_MINUTE))
        .saturating_add(second);
    Some(days.saturating_mul(SECONDS_PER_DAY).saturating_add(time_of_day))
}

fn parse_time_token(token: &[u8]) -> Option<(u64, u64, u64)> {
    let (hour, rest) = leading_digits(token, 1, 2)?;
    let rest = rest.strip_prefix(b":")?;
    let (minute, rest) = leading_digits(rest, 1, 2)?;
    let rest = rest.strip_prefix(b":")?;
    let (second, _) = leading_digits(rest, 1, 2)?;
    Some((hour, minute, second))
}

const SECONDS_PER_HOUR: u64 = 3600;
const SECONDS_PER_MINUTE: u64 = 60;
/// Gregorian leap-year cycle lengths.
const LEAP_EVERY: u64 = 4;
const LEAP_SKIPPED_EVERY: u64 = 100;
const LEAP_RESTORED_EVERY: u64 = 400;

const fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(LEAP_EVERY) && !year.is_multiple_of(LEAP_SKIPPED_EVERY))
        || year.is_multiple_of(LEAP_RESTORED_EVERY)
}

const fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to a date, or 0 for earlier dates: the
/// days-from-civil algorithm on the proleptic Gregorian calendar.
fn days_from_civil(year: u64, month: u64, day: u64) -> Option<u64> {
    const DAYS_PER_ERA: u64 = 146_097;
    const UNIX_EPOCH_FROM_0000_03_01: u64 = 719_468;
    let y = if month <= 2 { year.checked_sub(1)? } else { year };
    let era = y / LEAP_RESTORED_EVERY;
    let year_of_era = y.checked_sub(era.checked_mul(LEAP_RESTORED_EVERY)?)?;
    let shifted_month = if month > 2 { month.checked_sub(3)? } else { month.checked_add(9)? };
    // Days before the month in a year starting in March: 153 days per five
    // months, rounded.
    let day_of_year = (153u64.checked_mul(shifted_month)?.checked_add(2)? / 5).checked_add(day.checked_sub(1)?)?;
    let day_of_era = year_of_era
        .checked_mul(365)?
        .checked_add(year_of_era / LEAP_EVERY)?
        .checked_sub(year_of_era / LEAP_SKIPPED_EVERY)?
        .checked_add(day_of_year)?;
    let days = era.checked_mul(DAYS_PER_ERA)?.checked_add(day_of_era)?;
    Some(days.saturating_sub(UNIX_EPOCH_FROM_0000_03_01))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;

    type Jar = CookieJar<120, 65536>;

    const NOW: u64 = 1_791_590_400; // 2026-10-10 00:00:00 UTC

    fn header(jar: &mut Jar, host: &str, path: &str, secure: bool, site: Site) -> std::string::String {
        let mut buffer = [0u8; 8192];
        let mut writer = Writer::new(&mut buffer);
        jar.header_for(host, path, secure, site, NOW, &mut writer);
        std::string::String::from_utf8(writer.written().to_vec()).unwrap()
    }

    fn set(jar: &mut Jar, text: &str, secure: bool) -> Result<(), CookieRejected> {
        jar.store(text.as_bytes(), "a.example", "/dir/page", secure, NOW, true)
    }

    #[test]
    fn control_characters_ignore_the_header() {
        let mut jar = Jar::new();
        assert_eq!(set(&mut jar, "a=b\u{1}c", true), Err(CookieRejected::ControlCharacter));
        assert_eq!(set(&mut jar, "a=b; Path=/\u{7f}", true), Err(CookieRejected::ControlCharacter));
        assert_eq!(set(&mut jar, "a=b\tc", true), Ok(()), "tab is allowed");
    }

    #[test]
    fn empty_name_and_value_is_ignored() {
        let mut jar = Jar::new();
        assert_eq!(set(&mut jar, "=", true), Err(CookieRejected::Empty));
        assert_eq!(set(&mut jar, " ; Secure", true), Err(CookieRejected::Empty));
        assert_eq!(set(&mut jar, "lonely", true), Ok(()));
        assert_eq!(header(&mut jar, "a.example", "/dir", true, Site::SameService), "lonely");
    }

    #[test]
    fn size_limits() {
        let mut jar = Jar::new();
        let mut big = std::string::String::from("n=");
        big.push_str(&"v".repeat(4095));
        assert_eq!(set(&mut jar, &big, true), Ok(()), "exactly 4096 is accepted");
        big.push('v');
        assert_eq!(set(&mut jar, &big, true), Err(CookieRejected::TooLarge));
        jar.wipe();
        let long_path = std::format!("p=1; Path=/{}", "x".repeat(1024));
        assert_eq!(set(&mut jar, &long_path, true), Ok(()));
        // The over-long Path attribute was ignored, so the default path applies.
        assert_eq!(header(&mut jar, "a.example", "/dir/x", true, Site::SameService), "p=1");
    }

    #[test]
    fn domain_is_ignored_so_cookies_are_host_only() {
        let mut jar = Jar::new();
        set(&mut jar, "a=1; Domain=example; Path=/", true).unwrap();
        assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "a=1");
        assert_eq!(header(&mut jar, "b.a.example", "/", true, Site::SameService), "");
        assert_eq!(header(&mut jar, "example", "/", true, Site::SameService), "");
    }

    #[test]
    fn lifetimes() {
        let mut jar = Jar::new();
        set(&mut jar, "a=1; Path=/; Max-Age=10; Expires=Wed, 21 Oct 2015 07:28:00 GMT", true).unwrap();
        assert_eq!(
            header(&mut jar, "a.example", "/", true, Site::SameService),
            "a=1",
            "Max-Age wins over a past Expires"
        );
        set(&mut jar, "b=1; Path=/; Max-Age=999999999", true).unwrap();
        let record = jar.records.iter().flatten().find(|r| jar.parts(r).name == b"b").copied().unwrap();
        assert_eq!(record.expires, Some(NOW + 400 * 86_400), "capped at 400 days");
        set(&mut jar, "c=1; Path=/; Expires=Fri, 01 Jan 2100 00:00:00 GMT", true).unwrap();
        let record = jar.records.iter().flatten().find(|r| jar.parts(r).name == b"c").copied().unwrap();
        assert_eq!(record.expires, Some(NOW + 400 * 86_400), "Expires is capped too");
        assert_eq!(set(&mut jar, "a=1; Path=/; Max-Age=0", true), Err(CookieRejected::Expired));
        assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "b=1; c=1", "Max-Age=0 deletes");
        assert_eq!(
            set(&mut jar, "b=1; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT", true),
            Err(CookieRejected::Expired)
        );
        assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "c=1", "a past Expires deletes");
        set(&mut jar, "s=1; Path=/", true).unwrap();
        let record = jar.records.iter().flatten().find(|r| jar.parts(r).name == b"s").copied().unwrap();
        assert_eq!(record.expires, None, "a session cookie");
    }

    #[test]
    fn secure_cookies_and_plain_http() {
        let mut jar = Jar::new();
        assert_eq!(set(&mut jar, "s=1; Secure; Path=/", false), Err(CookieRejected::SecureOverPlain));
        set(&mut jar, "s=1; Secure; Path=/", true).unwrap();
        assert_eq!(set(&mut jar, "s=2; Path=/", false), Err(CookieRejected::WouldReplaceSecure));
        assert_eq!(set(&mut jar, "s=2; Path=/x", false), Err(CookieRejected::WouldReplaceSecure));
        assert_eq!(set(&mut jar, "s=; Path=/; Max-Age=0", false), Err(CookieRejected::WouldReplaceSecure));
        assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "s=1");
        assert_eq!(header(&mut jar, "a.example", "/", false, Site::SameService), "", "never sent over plain HTTP");
        set(&mut jar, "p=1; Path=/", false).unwrap();
        assert_eq!(header(&mut jar, "a.example", "/", false, Site::SameService), "p=1");
    }

    #[test]
    fn prefixes() {
        let mut jar = Jar::new();
        let cases: [(&str, bool, bool); 14] = [
            ("__Secure-a=1; Secure", true, true),
            ("__Secure-a=1", true, false),
            ("__SECURE-a=1", true, false),
            ("__Secure-a=1; Secure", false, false),
            ("__Host-a=1; Secure; Path=/", true, true),
            ("__Host-a=1; Secure", true, false),
            ("__Host-a=1; Secure; Path=/; Domain=a.example", true, false),
            ("__host-a=1; Path=/", true, false),
            ("__Http-a=1; Secure; HttpOnly", true, true),
            ("__Http-a=1; Secure", true, false),
            ("__Host-Http-a=1; Secure; HttpOnly; Path=/", true, true),
            ("__Host-Http-a=1; Secure; HttpOnly", true, false),
            ("__Host-Http-a=1; Secure; Path=/", true, false),
            ("=__Host-a=1; Secure; Path=/", true, false),
        ];
        for (text, secure, accepted) in cases {
            let result = set(&mut jar, text, secure);
            if accepted {
                assert_eq!(result, Ok(()), "{text}");
            } else {
                assert!(matches!(result, Err(CookieRejected::Prefix | CookieRejected::SecureOverPlain)), "{text}");
            }
        }
    }

    #[test]
    fn same_site_rules() {
        let mut jar = Jar::new();
        set(&mut jar, "strict=1; Path=/; Secure; SameSite=Strict", true).unwrap();
        set(&mut jar, "lax=1; Path=/; Secure; SameSite=Lax", true).unwrap();
        set(&mut jar, "none=1; Path=/; Secure; SameSite=None", true).unwrap();
        set(&mut jar, "default=1; Path=/; Secure; SameSite=bogus", true).unwrap();
        set(&mut jar, "absent=1; Path=/; Secure", true).unwrap();
        assert_eq!(
            set(&mut jar, "insecure=1; Path=/; SameSite=None", true),
            Err(CookieRejected::SameSiteNoneNotSecure)
        );
        let record = jar.records.iter().flatten().find(|r| jar.parts(r).name == b"default").copied().unwrap();
        assert_eq!(record.same_site, SameSite::Default);
        assert_eq!(
            header(&mut jar, "a.example", "/", true, Site::SameService),
            "strict=1; lax=1; none=1; default=1; absent=1"
        );
        assert_eq!(
            header(&mut jar, "a.example", "/", true, Site::CrossServiceNavigation),
            "lax=1; none=1; default=1; absent=1",
            "default is enforced as Lax"
        );
        assert_eq!(header(&mut jar, "a.example", "/", true, Site::CrossOriginForm), "none=1");
    }

    #[test]
    fn http_only_and_partitioned_are_stored() {
        let mut jar = Jar::new();
        set(&mut jar, "a=1; HttpOnly; Partitioned; Secure; Path=/", true).unwrap();
        let record = jar.records.iter().flatten().next().copied().unwrap();
        assert!(record.http_only && record.partitioned);
        assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "a=1");
    }

    #[test]
    fn paths_and_order() {
        let mut jar = Jar::new();
        set(&mut jar, "root=1; Path=/", true).unwrap();
        set(&mut jar, "deep=1; Path=/dir/sub", true).unwrap();
        set(&mut jar, "dir=1", true).unwrap(); // default path /dir
        set(&mut jar, "other=1; Path=/other", true).unwrap();
        set(&mut jar, "relative=1; Path=dir", true).unwrap(); // bad Path: default applies
        assert_eq!(
            header(&mut jar, "a.example", "/dir/sub/page", true, Site::SameService),
            "deep=1; dir=1; relative=1; root=1"
        );
        assert_eq!(header(&mut jar, "a.example", "/dirt", true, Site::SameService), "root=1");
        assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "root=1");
        // Replacing keeps the creation order.
        set(&mut jar, "root=2; Path=/", true).unwrap();
        assert_eq!(header(&mut jar, "a.example", "/x", true, Site::SameService), "root=2");
        assert_eq!(default_cookie_path(b"/a/b/c"), b"/a/b");
        assert_eq!(default_cookie_path(b"/a"), b"/");
        assert_eq!(default_cookie_path(b""), b"/");
    }

    #[test]
    fn values_are_opaque() {
        let mut jar = Jar::new();
        set(&mut jar, "session=v2|eyJ1c2VyIjoxfQ==|sig; Path=/", true).unwrap();
        assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "session=v2|eyJ1c2VyIjoxfQ==|sig");
    }

    #[test]
    fn per_host_and_total_limits_evict_least_recently_used_expired_first() {
        let mut jar = Jar::new();
        jar.set_limits(3, 5);
        for name in ["a", "b", "c"] {
            jar.store(std::format!("{name}=1; Path=/").as_bytes(), "h1.example", "/", true, NOW, true).unwrap();
        }
        // Use "a" so "b" becomes least recently used.
        let mut buffer = [0u8; 64];
        let mut writer = Writer::new(&mut buffer);
        jar.header_for("h1.example", "/", true, Site::SameService, NOW, &mut writer);
        jar.store(b"a=2; Path=/", "h1.example", "/", true, NOW, true).unwrap();
        jar.store(b"d=1; Path=/", "h1.example", "/", true, NOW, true).unwrap();
        assert_eq!(header(&mut jar, "h1.example", "/", true, Site::SameService), "a=2; c=1; d=1");
        // An expired cookie goes first, even if recently used.
        jar.store(b"x=1; Path=/; Max-Age=5", "h2.example", "/", true, NOW, true).unwrap();
        jar.store(b"y=1; Path=/", "h2.example", "/", true, NOW, true).unwrap();
        assert_eq!(jar.len(), 5);
        jar.store(b"z=1; Path=/", "h3.example", "/", true, NOW + 10, true).unwrap();
        assert_eq!(jar.len(), 5);
        let names: std::vec::Vec<_> = jar.records.iter().flatten().map(|r| jar.parts(r).name.to_vec()).collect();
        assert!(!names.contains(&b"x".to_vec()), "{names:?}");
    }

    #[test]
    fn arena_compaction_keeps_other_cookies_intact() {
        let mut jar: CookieJar<8, 64> = CookieJar::new();
        jar.store(b"a=11111111; Path=/", "h.example", "/", true, NOW, true).unwrap();
        jar.store(b"b=22222222; Path=/", "h.example", "/", true, NOW, true).unwrap();
        jar.store(b"c=33333333; Path=/", "h.example", "/", true, NOW, true).unwrap();
        // The arena is full: storing evicts the least recently used.
        jar.store(b"d=44444444; Path=/", "h.example", "/", true, NOW, true).unwrap();
        let mut buffer = [0u8; 128];
        let mut writer = Writer::new(&mut buffer);
        jar.header_for("h.example", "/", true, Site::SameService, NOW, &mut writer);
        assert_eq!(writer.written(), b"b=22222222; c=33333333; d=44444444");
        assert_eq!(jar.store(&[b'x'; 70], "h.example", "/", true, NOW, true), Err(CookieRejected::NoRoom));
    }

    #[test]
    fn wipe_and_persistence() {
        let mut jar = Jar::new();
        set(&mut jar, "keep=1; Path=/; Max-Age=3600; Secure; SameSite=Strict", true).unwrap();
        set(&mut jar, "session=1; Path=/", true).unwrap();
        set(&mut jar, "soon=1; Path=/; Max-Age=1", true).unwrap();
        let mut buffer = [0u8; 1024];
        let mut writer = Writer::new(&mut buffer);
        assert_eq!(jar.export(NOW + 5, &mut writer).unwrap(), 1, "session and expired cookies are never saved");
        let saved = writer.written().to_vec();
        jar.wipe();
        assert!(jar.is_empty());
        assert_eq!(jar.import(&saved, NOW + 5), 1);
        assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "keep=1");
        assert_eq!(header(&mut jar, "a.example", "/", true, Site::CrossServiceNavigation), "", "SameSite survives");
        // Tampered data is refused.
        let mut tampered = saved.clone();
        let len = tampered.len();
        tampered[len - 1] = b'x';
        let mut fresh = Jar::new();
        assert_eq!(fresh.import(&tampered, NOW), 0);
        assert_eq!(fresh.import(b"tellytxt-cookies 1\n9999999999 1 3 1 1 1 a.bx=1/\n", NOW), 0, "a bad host");
        assert_eq!(fresh.import(b"garbage", NOW), 0);
    }

    #[test]
    fn cookie_dates() {
        let cases: [(&str, Option<u64>); 8] = [
            ("Wed, 21 Oct 2015 07:28:00 GMT", Some(1_445_412_480)),
            ("Sunday, 06-Nov-94 08:49:37 GMT", Some(784_111_777)),
            ("Sun Nov  6 08:49:37 1994", Some(784_111_777)),
            ("Thu, 01 Jan 1970 00:00:00 GMT", Some(0)),
            ("Sat, 10 Oct 2026 00:00:00 GMT", Some(NOW)),
            ("Wed, 30 Feb 2015 07:28:00 GMT", None),
            ("Wed, 21 Oct 2015 25:28:00 GMT", None),
            ("21 Oct 2015", None),
        ];
        for (text, expected) in cases {
            assert_eq!(parse_cookie_date(text.as_bytes()), expected, "{text}");
        }
    }

    #[test]
    fn expires_needs_calendar_time() {
        let mut jar = Jar::new();
        jar.store(b"a=1; Path=/; Expires=Fri, 01 Jan 2100 00:00:00 GMT", "h.example", "/", true, 50, false).unwrap();
        let record = jar.records.iter().flatten().next().copied().unwrap();
        assert_eq!(record.expires, None, "kept as a session cookie");
    }

    #[test]
    fn secure_history_for_downgrade() {
        let mut jar = Jar::new();
        set(&mut jar, "a=1; Path=/; Max-Age=60", true).unwrap();
        assert!(jar.reached_securely("a.example", NOW));
        assert!(!jar.reached_securely("a.example", NOW + 61));
        assert!(!jar.reached_securely("b.example", NOW));
    }
}
