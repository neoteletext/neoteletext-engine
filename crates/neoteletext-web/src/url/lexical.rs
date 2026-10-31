//! The first, lexical pass of the strict address parser (REQ-WEB-016,
//! RFC 3986 sections 3.1 and 3.2): the characters every address keeps
//! to, and an absolute address split into its scheme, host, port, path and
//! query.

use super::{FRAGMENT_MARK, Parts, Scheme, UrlError};
use crate::bytes::parse_decimal;
use crate::consts::{MAX_PORT_DIGITS, MAX_URL_LENGTH};

/// Checks the characters every address must keep to.
pub(super) fn check_characters(text: &[u8]) -> Result<(), UrlError> {
    if text.len() > MAX_URL_LENGTH {
        return Err(UrlError::TooLong);
    }
    for &byte in text {
        match byte {
            b'\\' => return Err(UrlError::Backslash),
            0x21..=0x7E => {}
            _ => return Err(UrlError::BadCharacter),
        }
    }
    Ok(())
}

/// The scheme of an absolute reference: letters, digits, `+`, `-` and `.`
/// before a colon, starting with a letter (RFC 3986 section 3.1).
pub(super) fn scheme_end(text: &[u8]) -> Option<usize> {
    let first = text.first()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let end = text.iter().position(|&b| !(b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.')))?;
    (text.get(end) == Some(&b':')).then_some(end)
}

pub(super) fn split_parts(text: &[u8]) -> Result<Parts<'_>, UrlError> {
    let end = scheme_end(text).ok_or(UrlError::Scheme)?;
    let scheme = Scheme::from_name(text.get(..end).unwrap_or(&[])).ok_or(UrlError::Scheme)?;
    let rest = text.get(end.saturating_add(1)..).unwrap_or(&[]);
    let rest = rest.strip_prefix(b"//").ok_or(UrlError::Scheme)?;
    // REQ-WEB-016: the host ends at the first `/`, `?` or `#`, so nothing
    // after a `#` can pose as the host.
    let authority_end = rest.iter().position(|&b| matches!(b, b'/' | b'?' | FRAGMENT_MARK)).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    if authority.contains(&b'@') {
        return Err(UrlError::UserInfo);
    }
    let (host, bracketed, port_text) = if let Some(inner) = authority.strip_prefix(b"[") {
        let close = inner.iter().position(|&b| b == b']').ok_or(UrlError::Ipv6)?;
        let after = inner.get(close.saturating_add(1)..).unwrap_or(&[]);
        let port_text = match after.split_first() {
            None => None,
            Some((b':', digits)) => Some(digits),
            Some(_) => return Err(UrlError::Host),
        };
        (inner.get(..close).unwrap_or(&[]), true, port_text)
    } else {
        match authority.iter().position(|&b| b == b':') {
            Some(colon) => (
                authority.get(..colon).unwrap_or(&[]),
                false,
                Some(authority.get(colon.saturating_add(1)..).unwrap_or(&[])),
            ),
            None => (authority, false, None),
        }
    };
    let port = match port_text {
        None => None,
        Some(digits) => Some(parse_port(digits)?),
    };
    let (tail, fragment) = match tail.iter().position(|&b| b == FRAGMENT_MARK) {
        Some(mark) => (tail.get(..mark).unwrap_or(&[]), tail.get(mark.saturating_add(1)..)),
        None => (tail, None),
    };
    let (path, query) = match tail.iter().position(|&b| b == b'?') {
        Some(mark) => (tail.get(..mark).unwrap_or(&[]), Some(tail.get(mark.saturating_add(1)..).unwrap_or(&[]))),
        None => (tail, None),
    };
    Ok(Parts { scheme, host, bracketed, port, path, query, fragment })
}

fn parse_port(digits: &[u8]) -> Result<u16, UrlError> {
    // A leading zero is not the canonical form of any port; rejecting it
    // keeps every accepted address in one spelling.
    if digits.first() == Some(&b'0') {
        return Err(UrlError::Port);
    }
    let value = parse_decimal(digits, MAX_PORT_DIGITS).ok_or(UrlError::Port)?;
    u16::try_from(value).ok().filter(|&port| port != 0).ok_or(UrlError::Port)
}
