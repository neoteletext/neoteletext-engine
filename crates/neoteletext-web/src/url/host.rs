//! The host grammar of the strict address parser (REQ-WEB-016): DNS names
//! of ASCII labels, IPv4 in strict dotted decimal, and IPv6 as RFC 4291
//! section 2.2 writes it, with the forms other parsers read as numbers
//! refused.

use super::{IpAddress, UrlError};
use crate::bytes::{Overflow, Writer, hex_value, parse_decimal};
use crate::consts::{
    IPV4_OCTETS, IPV6_GROUPS, MAX_HOST_NAME_LENGTH, MAX_IPV4_OCTET_DIGITS, MAX_IPV6_GROUP_DIGITS, MAX_LABEL_LENGTH,
};

/// `text` as an IP address when it is one, as a host is written without
/// brackets: strict dotted decimal, or RFC 4291's text form.
#[must_use]
pub fn ip_literal(text: &str) -> Option<IpAddress> {
    if text.contains(':') {
        parse_ipv6(text.as_bytes()).ok().map(IpAddress::V6)
    } else {
        parse_ipv4(text.as_bytes()).ok().map(IpAddress::V4)
    }
}

/// Parses an IPv4 address in strict dotted decimal: four parts of one to
/// three digits, no leading zeros, each at most 255.
pub(super) fn parse_ipv4(text: &[u8]) -> Result<[u8; IPV4_OCTETS], UrlError> {
    let mut octets = [0u8; IPV4_OCTETS];
    let mut count = 0usize;
    for part in text.split(|&b| b == b'.') {
        let slot = octets.get_mut(count).ok_or(UrlError::Ipv4)?;
        if part.len() > 1 && part.first() == Some(&b'0') {
            return Err(UrlError::Ipv4);
        }
        let value = parse_decimal(part, MAX_IPV4_OCTET_DIGITS).ok_or(UrlError::Ipv4)?;
        *slot = u8::try_from(value).map_err(|_| UrlError::Ipv4)?;
        count = count.saturating_add(1);
    }
    if count == IPV4_OCTETS { Ok(octets) } else { Err(UrlError::Ipv4) }
}

/// Parses an IPv6 address (RFC 4291 section 2.2): eight groups of one to
/// four hexadecimal digits, one `::` standing for one or more zero groups,
/// and an optional dotted IPv4 tail. Zone identifiers are rejected.
pub(super) fn parse_ipv6(text: &[u8]) -> Result<[u16; IPV6_GROUPS], UrlError> {
    let mut head = [0u16; IPV6_GROUPS];
    let mut tail = [0u16; IPV6_GROUPS];
    let (head_text, tail_text) = match find_double_colon(text) {
        Some(index) => (text.get(..index).unwrap_or(&[]), Some(text.get(index.saturating_add(2)..).unwrap_or(&[]))),
        None => (text, None),
    };
    let head_count = parse_groups(head_text, &mut head, tail_text.is_none())?;
    let tail_count = match tail_text {
        Some(rest) => parse_groups(rest, &mut tail, true)?,
        None => 0,
    };
    let total = head_count.saturating_add(tail_count);
    let valid = match tail_text {
        None => total == IPV6_GROUPS,
        Some(_) => total < IPV6_GROUPS,
    };
    if !valid {
        return Err(UrlError::Ipv6);
    }
    let mut groups = head;
    let offset = IPV6_GROUPS.saturating_sub(tail_count);
    for (index, &value) in tail.iter().take(tail_count).enumerate() {
        if let Some(slot) = groups.get_mut(offset.saturating_add(index)) {
            *slot = value;
        }
    }
    Ok(groups)
}

fn find_double_colon(text: &[u8]) -> Option<usize> {
    text.windows(2).position(|pair| pair == b"::")
}

/// Parses colon-separated groups into `groups`, returning how many were
/// written. `ipv4_allowed` permits a dotted IPv4 tail as the last part.
fn parse_groups(text: &[u8], groups: &mut [u16; IPV6_GROUPS], ipv4_allowed: bool) -> Result<usize, UrlError> {
    if text.is_empty() {
        return Ok(0);
    }
    let mut count = 0usize;
    let parts = text.split(|&b| b == b':');
    let total_parts = text.split(|&b| b == b':').count();
    for (index, part) in parts.enumerate() {
        let last = index.saturating_add(1) == total_parts;
        if last && ipv4_allowed && part.contains(&b'.') {
            let [a, b, c, d] = parse_ipv4(part).map_err(|_| UrlError::Ipv6)?;
            for value in [u16::from_be_bytes([a, b]), u16::from_be_bytes([c, d])] {
                *groups.get_mut(count).ok_or(UrlError::Ipv6)? = value;
                count = count.saturating_add(1);
            }
            continue;
        }
        if part.is_empty() || part.len() > MAX_IPV6_GROUP_DIGITS {
            return Err(UrlError::Ipv6);
        }
        let value = part.iter().try_fold(0u16, |value, &d| Some((value << 4) | u16::from(hex_value(d)?)));
        *groups.get_mut(count).ok_or(UrlError::Ipv6)? = value.ok_or(UrlError::Ipv6)?;
        count = count.saturating_add(1);
    }
    Ok(count)
}

/// True for a label that a URL parser elsewhere would read as a number:
/// all decimal digits, or `0x` followed by hexadecimal digits.
fn is_numeric_label(label: &[u8]) -> bool {
    if !label.is_empty() && label.iter().all(u8::is_ascii_digit) {
        return true;
    }
    match label {
        [b'0', b'x' | b'X', rest @ ..] => rest.iter().all(u8::is_ascii_hexdigit),
        _ => false,
    }
}

/// Classifies and checks a host that is not in brackets.
pub(super) fn check_host(host: &[u8]) -> Result<Option<IpAddress>, UrlError> {
    if host.is_empty() {
        return Err(UrlError::Host);
    }
    let last_label = host.rsplit(|&b| b == b'.').next().unwrap_or(&[]);
    // A host ending in a numeric label is an IPv4 address to other
    // parsers (the WHATWG URL Standard's "ends in a number"), so it must be
    // strict dotted decimal here or be rejected: this closes the octal,
    // hexadecimal and shortened forms.
    if is_numeric_label(last_label) {
        return parse_ipv4(host).map(|octets| Some(IpAddress::V4(octets)));
    }
    if host.len() > MAX_HOST_NAME_LENGTH {
        return Err(UrlError::Host);
    }
    for label in host.split(|&b| b == b'.') {
        let valid = !label.is_empty()
            && label.len() <= MAX_LABEL_LENGTH
            && label.iter().all(|&b| b.is_ascii_alphanumeric() || b == b'-')
            && label.first() != Some(&b'-')
            && label.last() != Some(&b'-');
        if !valid {
            return Err(UrlError::Host);
        }
    }
    Ok(None)
}

/// The groups of an IPv4-mapped address's prefix, `::ffff:0:0/96` (RFC
/// 4291 section 2.5.5.2): five zero groups, then `ffff`.
const IPV4_MAPPED_PREFIX: [u16; IPV6_GROUPS - IPV4_GROUPS] = [0, 0, 0, 0, 0, 0xFFFF];
/// The IPv6 groups an IPv4 address fills.
const IPV4_GROUPS: usize = 2;
/// The bits of one hexadecimal digit, and of one IPv6 group.
const NIBBLE_BITS: u32 = 4;
const NIBBLE_MASK: u16 = 0xF;
const GROUP_NIBBLES: u32 = u16::BITS / NIBBLE_BITS;

/// Writes `groups` in RFC 5952's text form (section 4): lower-case digits
/// without leading zeros, the longest run of two or more zero groups (the
/// first, when runs tie) as `::`, and an IPv4-mapped address in its mixed
/// form `::ffff:a.b.c.d` (section 5). One address then has one spelling,
/// so hosts compare as text (decision E.114 item 9, finding F-96).
pub(super) fn write_ipv6(groups: &[u16; IPV6_GROUPS], writer: &mut Writer<'_>) -> Result<(), Overflow> {
    if groups.starts_with(&IPV4_MAPPED_PREFIX) {
        writer.put(b"::ffff:")?;
        let [high, low] = groups
            .get(IPV4_MAPPED_PREFIX.len()..)
            .map_or([0, 0], |tail| [tail.first().copied().unwrap_or(0), tail.get(1).copied().unwrap_or(0)]);
        let [a, b] = high.to_be_bytes();
        let [c, d] = low.to_be_bytes();
        for (index, octet) in [a, b, c, d].into_iter().enumerate() {
            if index > 0 {
                writer.byte(b'.')?;
            }
            writer.decimal(u64::from(octet))?;
        }
        return Ok(());
    }
    // The longest run of zero groups, if it is two or more long.
    let (mut best_start, mut best_len, mut run_start, mut run_len) = (0usize, 0usize, 0usize, 0usize);
    for (index, &group) in groups.iter().enumerate() {
        if group == 0 {
            if run_len == 0 {
                run_start = index;
            }
            run_len = run_len.saturating_add(1);
            if run_len > best_len {
                (best_start, best_len) = (run_start, run_len);
            }
        } else {
            run_len = 0;
        }
    }
    let compressed = (best_len >= IPV4_GROUPS).then_some(best_start..best_start.saturating_add(best_len));
    let mut index = 0usize;
    while let Some(&group) = groups.get(index) {
        if let Some(run) = compressed.as_ref().filter(|run| run.start == index) {
            writer.put(b"::")?;
            index = run.end;
            continue;
        }
        if index > 0 && compressed.as_ref().is_none_or(|run| run.end != index) {
            writer.byte(b':')?;
        }
        write_group(group, writer)?;
        index = index.saturating_add(1);
    }
    Ok(())
}

/// One group in lower-case hexadecimal without leading zeros.
fn write_group(group: u16, writer: &mut Writer<'_>) -> Result<(), Overflow> {
    let mut started = false;
    for nibble in (0..GROUP_NIBBLES).rev() {
        let digit = u8::try_from((group >> nibble.saturating_mul(NIBBLE_BITS)) & NIBBLE_MASK).unwrap_or(0);
        started |= digit != 0 || nibble == 0;
        if started {
            writer.byte(neoteletext_core::types::hex_digit(digit).to_ascii_lowercase())?;
        }
    }
    Ok(())
}
