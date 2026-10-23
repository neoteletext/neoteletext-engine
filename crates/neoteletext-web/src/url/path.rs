//! The path and query grammar of the strict address parser (RFC 3986
//! sections 3.3 and 3.4, REQ-WEB-016) and the removal of dot segments
//! (section 5.2.4).

use super::UrlError;
use crate::bytes::{Writer, hex_value};

/// RFC 3986 section 2: unreserved and sub-delimiter characters, plus `:`
/// and `@` (pchar), `/` in paths, and `?` in queries.
fn is_path_char(byte: u8, query: bool) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'.' | b'_' | b'~' | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
        )
        || matches!(byte, b':' | b'@' | b'/')
        || (query && byte == b'?')
}

/// Checks a path or query: allowed characters and well-formed
/// percent-encoding.
pub(super) fn check_path(text: &[u8], query: bool) -> Result<(), UrlError> {
    let mut index = 0usize;
    while let Some(&byte) = text.get(index) {
        if byte == b'%' {
            let valid = text.get(index.saturating_add(1)).copied().and_then(hex_value).is_some()
                && text.get(index.saturating_add(2)).copied().and_then(hex_value).is_some();
            if !valid {
                return Err(UrlError::Path);
            }
            index = index.saturating_add(3);
        } else if is_path_char(byte, query) {
            index = index.saturating_add(1);
        } else {
            return Err(UrlError::Path);
        }
    }
    Ok(())
}

/// RFC 3986 section 5.2.4, writing the result. Percent-encoded dots are
/// not decoded, as the RFC specifies.
pub(super) fn remove_dot_segments(path: &[u8], writer: &mut Writer<'_>) -> Result<(), crate::bytes::Overflow> {
    let start = writer.len();
    let mut rest = path;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix(b"../").or_else(|| rest.strip_prefix(b"./")) {
            rest = after;
        } else if rest.starts_with(b"/./") {
            rest = rest.get(2..).unwrap_or(&[]);
        } else if rest == b"/." {
            rest = b"/";
        } else if rest.starts_with(b"/../") || rest == b"/.." {
            rest = if rest.len() == 3 { b"/" } else { rest.get(3..).unwrap_or(&[]) };
            // Remove the last segment written, and its preceding slash.
            let written = writer.written().get(start..).unwrap_or(&[]);
            let cut = written.iter().rposition(|&b| b == b'/').unwrap_or(0);
            writer.truncate(start.saturating_add(cut));
        } else if rest == b"." || rest == b".." {
            rest = &[];
        } else {
            let skip = usize::from(rest.first() == Some(&b'/'));
            let end = rest.iter().skip(skip).position(|&b| b == b'/').map_or(rest.len(), |i| i.saturating_add(skip));
            writer.put(rest.get(..end).unwrap_or(&[]))?;
            rest = rest.get(end..).unwrap_or(&[]);
        }
    }
    if writer.len() == start {
        writer.byte(b'/')?;
    }
    Ok(())
}
