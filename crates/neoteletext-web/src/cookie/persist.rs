//! The persistence format of the cookie store (REQ-SEC-013, decision
//! E.104, version 3 since each cookie keeps its channel, REQ-515): export,
//! and import of the data as untrusted.

use super::store::PathRule;
use super::{CookieJar, Record, SameSite, SetCookie};
use crate::bytes::{MAX_U64_DIGITS, Overflow, Writer, parse_decimal};

/// The first line of exported cookie data.
const EXPORT_VERSION: &[u8] = b"neoteletext-cookies 3\n";
const FLAG_SECURE: u8 = 1;
const FLAG_HTTP_ONLY: u8 = 2;
const FLAG_PARTITIONED: u8 = 4;
const FLAG_VIA_SECURE: u8 = 8;
const FLAG_SAME_SITE_SHIFT: u32 = 4;
/// The two bits of the `SameSite` value, above the four flags.
const SAME_SITE_MASK: u8 = 0b11;
/// The cookie belongs to a channel, whose folder is the first
/// `scope-length` bytes of its path; without it the length is 0. A flag
/// of its own, since a folder's length alone could not tell an empty
/// folder from none.
const FLAG_CHANNEL: u8 = 1 << (FLAG_SAME_SITE_SHIFT + SAME_SITE_MASK.count_ones());
/// The decimal fields before a record's texts.
const RECORD_FIELDS: usize = 9;

impl<const SLOTS: usize, const BYTES: usize> CookieJar<SLOTS, BYTES> {
    /// Writes the cookies that may be saved (REQ-SEC-013): only those with
    /// an expiry that has not passed. Each record is one line:
    /// `expires created used flags host-length name-length value-length
    /// path-length scope-length` in decimal, a space, then the four texts
    /// back to back (decision E.104: the times a port lists and clears by;
    /// version 3: the channel's folder, the start of the path, REQ-515).
    pub fn export(&self, now: u64, out: &mut Writer<'_>) -> Result<usize, Overflow> {
        out.put(EXPORT_VERSION)?;
        let mut count = 0usize;
        for record in self.records.iter().flatten() {
            let Some(expires) = record.expires.filter(|&expires| expires > now) else { continue };
            let parts = self.parts(record);
            for time in [expires, record.created_at, record.used_at] {
                out.decimal(time)?;
                out.byte(b' ')?;
            }
            out.decimal(u64::from(encode_flags(record)))?;
            let scope_len = parts.scope.map_or(0, <[u8]>::len);
            for len in [parts.host.len(), parts.name.len(), parts.value.len(), parts.path.len(), scope_len] {
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
    /// as untrusted: every record is checked. A record the rules refuse,
    /// such as one that has expired since it was saved, is read past and
    /// not stored; only a malformed one ends the import, since its end
    /// cannot be found (finding F-13, decision E.114 item 1). Returns the
    /// number of cookies restored.
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
        let mut fields = [0u64; RECORD_FIELDS];
        let mut rest = data;
        for field in &mut fields {
            let end = rest.iter().position(|&b| b == b' ')?;
            *field = parse_decimal(rest.get(..end)?, MAX_U64_DIGITS)?;
            rest = rest.get(end.saturating_add(1)..)?;
        }
        let [expires, created_at, used_at, flags, host_len, name_len, value_len, path_len, scope_len] = fields;
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
        let scope_len = usize::try_from(scope_len).unwrap_or(usize::MAX);
        // The record's end is known from here, so a refused record is read
        // past rather than ending the import (finding F-13).
        let Ok(host) = core::str::from_utf8(host) else { return Some((false, rest)) };
        if crate::url::Url::parse_host_only(host).is_err() || path.first() != Some(&b'/') {
            return Some((false, rest));
        }
        // The record goes through the normal rules, so a tampered file
        // cannot create a cookie the rules would refuse. The path is passed
        // as saved, not as an attribute (finding F-13).
        let Ok(flags) = u8::try_from(flags) else { return Some((false, rest)) };
        // The channel's folder is the start of the path, and a cookie of no
        // channel has none.
        let scope = match (flags & FLAG_CHANNEL != 0, path.get(..scope_len)) {
            (true, Some(scope)) => Some(scope),
            (false, _) if scope_len == 0 => None,
            _ => return Some((false, rest)),
        };
        // An expired record has no time left, which the rules refuse.
        let max_age = Some(i64::try_from(expires.saturating_sub(now)).unwrap_or(i64::MAX));
        let cookie = SetCookie {
            name,
            value,
            max_age,
            expires: None,
            domain: false,
            path: None,
            secure: flags & FLAG_SECURE != 0,
            http_only: flags & FLAG_HTTP_ONLY != 0,
            partitioned: flags & FLAG_PARTITIONED != 0,
            same_site: decode_same_site(flags),
        };
        let secure = flags & FLAG_VIA_SECURE != 0;
        let stored = self.store_fields(&cookie, host, PathRule::Saved { path, scope }, secure, now).is_ok();
        // Its times as saved, none later than now.
        if stored
            && let Some(index) = self.find(host.as_bytes(), scope, name, path)
            && let Some(Some(record)) = self.records.get_mut(index)
        {
            record.created_at = created_at.min(now);
            record.used_at = used_at.min(now).max(record.created_at);
        }
        Some((stored, rest))
    }
}

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
        | flag(record.scope_len.is_some(), FLAG_CHANNEL)
        | (same_site << FLAG_SAME_SITE_SHIFT)
}

const fn decode_same_site(flags: u8) -> SameSite {
    match (flags >> FLAG_SAME_SITE_SHIFT) & SAME_SITE_MASK {
        1 => SameSite::Strict,
        2 => SameSite::Lax,
        3 => SameSite::None,
        _ => SameSite::Default,
    }
}
