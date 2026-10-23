//! Storing a cookie: the `Set-Cookie` algorithm (RFC 6265bis sections 5.6
//! and 5.7, REQ-WEB-006), its attributes, the prefix rules and the path
//! rules.

use super::{CookieJar, CookieRejected, MAX_AGE_SECONDS, Record, SameSite, parse_cookie_date};
use crate::bytes::{copy_prefix, parse_decimal, starts_with_ignore_case, trim_ows};
use crate::consts::{
    MAX_COOKIE_ATTRIBUTE_VALUE, MAX_COOKIE_NAME_VALUE, PREFIX_HOST, PREFIX_HOST_HTTP, PREFIX_HTTP, PREFIX_SECURE,
    cookie_attribute as attribute,
};
use neoteletext_core::policy;

/// Where a stored cookie's path comes from.
#[derive(Clone, Copy)]
pub(super) enum PathRule<'a> {
    /// RFC 6265bis section 5.6.4: the `Path` attribute, or the default
    /// path of `request_path`, kept inside `scope` when it is given.
    Request { request_path: &'a str, scope: Option<&'a str> },
    /// The path a saved record names, used as it was saved.
    Saved(&'a [u8]),
}

impl<'a> PathRule<'a> {
    /// The path the cookie is stored with, given its `Path` attribute.
    fn path(self, attribute: Option<&'a [u8]>) -> &'a [u8] {
        match self {
            PathRule::Request { request_path, scope } => {
                let path = attribute.unwrap_or_else(|| default_cookie_path(request_path.as_bytes()));
                match scope {
                    Some(scope) if !path.starts_with(scope.as_bytes()) => scope.as_bytes(),
                    _ => path,
                }
            }
            PathRule::Saved(saved) => saved,
        }
    }
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

impl<const SLOTS: usize, const BYTES: usize> CookieJar<SLOTS, BYTES> {
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
        self.store_within(header, host, request_path, None, secure, now, wall_clock)
    }

    /// [`Self::store`], keeping the cookie inside `scope` when it is given:
    /// a path that is not inside it, including the default path, becomes
    /// `scope` itself. A channel's cookies are scoped to its folder
    /// (REQ-WEB-026).
    #[allow(clippy::too_many_arguments, reason = "the arguments of `store`, and the scope")]
    pub fn store_within(
        &mut self,
        header: &[u8],
        host: &str,
        request_path: &str,
        scope: Option<&str>,
        secure: bool,
        now: u64,
        wall_clock: bool,
    ) -> Result<(), CookieRejected> {
        self.store_with_path(header, host, PathRule::Request { request_path, scope }, secure, now, wall_clock)
    }

    /// The `Set-Cookie` algorithm, with the cookie's path chosen by
    /// `rule`: from the header and the request, or the path a saved record
    /// names, which an import passes here rather than rebuilding it into a
    /// `Path=` attribute that the attribute limit would drop (finding F-13,
    /// decision E.114 item 1).
    pub(super) fn store_with_path(
        &mut self,
        header: &[u8],
        host: &str,
        rule: PathRule<'_>,
        secure: bool,
        now: u64,
        wall_clock: bool,
    ) -> Result<(), CookieRejected> {
        // Section 5.6 step 1: control characters other than tab, in a saved
        // path as in the header it once came from.
        let saved_path = match rule {
            PathRule::Saved(saved) => saved,
            PathRule::Request { .. } => &[],
        };
        if header.iter().chain(saved_path).any(|&b| (b < 0x20 && b != b'\t') || b == 0x7F) {
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
        let mut attributes = parse_attributes(attributes_text, now, wall_clock);
        if let PathRule::Saved(saved) = rule {
            // The saved path stands as the attribute, so the prefix rules
            // see it as they did when the cookie was first stored.
            attributes.path = Some(saved);
        }

        if !policy::may_send_cookie(attributes.secure, secure) {
            return Err(CookieRejected::SecureOverPlain);
        }
        check_prefixes(name, value, &attributes, secure)?;
        if attributes.same_site == SameSite::None && !attributes.secure {
            return Err(CookieRejected::SameSiteNoneNotSecure);
        }

        let path = rule.path(attributes.path);
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
                let created =
                    self.records.get(index).copied().flatten().map(|record| (record.created, record.created_at));
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
            copy_prefix(self.arena.get_mut(self.used..end).ok_or(CookieRejected::NoRoom)?, part);
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
            created: created.map_or(tick, |(created, _)| created),
            last_access: tick,
            created_at: created.map_or(now, |(_, created_at)| created_at),
            used_at: now,
        };
        let slot = self.records.iter_mut().find(|slot| slot.is_none()).ok_or(CookieRejected::NoRoom)?;
        *slot = Some(record);
        Ok(())
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
pub(super) fn default_cookie_path(request_path: &[u8]) -> &[u8] {
    if request_path.first() != Some(&b'/') {
        return b"/";
    }
    match request_path.iter().rposition(|&b| b == b'/') {
        Some(0) | None => b"/",
        Some(last) => request_path.get(..last).unwrap_or(b"/"),
    }
}

/// RFC 6265bis section 5.1.4: path-match.
pub(super) fn path_matches(request_path: &[u8], cookie_path: &[u8]) -> bool {
    if request_path == cookie_path {
        return true;
    }
    request_path.starts_with(cookie_path)
        && (cookie_path.last() == Some(&b'/') || request_path.get(cookie_path.len()) == Some(&b'/'))
}
