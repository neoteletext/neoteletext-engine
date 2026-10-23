//! The HTTP proxy's `CONNECT` handshake (RFC 9110 section 9.3.6,
//! REQ-WEB-032): the request, and the proxy's answer read as a response
//! head.

use super::ProxyError;
use crate::bytes::Writer;
use crate::http::{HeadCollector, parse_head};

/// The success status class of an HTTP response.
const HTTP_SUCCESS: core::ops::Range<u16> = 200..300;

/// Writes the `CONNECT` request for `host` and `port` (RFC 9110 section
/// 9.3.6), an IPv6 address in brackets.
///
/// # Errors
/// When it does not fit `out`.
pub fn write_connect(host: &str, port: u16, out: &mut [u8]) -> Result<usize, ProxyError> {
    let mut writer = Writer::new(out);
    let authority = |writer: &mut Writer<'_>| -> Result<(), ProxyError> {
        let bracket = host.contains(':');
        if bracket {
            writer.put(b"[").map_err(|_| ProxyError::TooLong)?;
        }
        writer.put(host.as_bytes()).map_err(|_| ProxyError::TooLong)?;
        if bracket {
            writer.put(b"]").map_err(|_| ProxyError::TooLong)?;
        }
        let mut digits = [0_u8; 5];
        writer.put(b":").map_err(|_| ProxyError::TooLong)?;
        writer.put(decimal(port, &mut digits)).map_err(|_| ProxyError::TooLong)
    };
    writer.put(b"CONNECT ").map_err(|_| ProxyError::TooLong)?;
    authority(&mut writer)?;
    writer.put(b" HTTP/1.1\r\nHost: ").map_err(|_| ProxyError::TooLong)?;
    authority(&mut writer)?;
    writer.put(b"\r\n\r\n").map_err(|_| ProxyError::TooLong)?;
    Ok(writer.written().len())
}

/// `value` in decimal digits, in `digits`.
fn decimal(value: u16, digits: &mut [u8; 5]) -> &[u8] {
    let mut start = digits.len();
    let mut rest = value;
    loop {
        start = start.saturating_sub(1);
        if let Some(digit) = digits.get_mut(start) {
            *digit = b'0'.saturating_add(u8::try_from(rest % 10).unwrap_or(0));
        }
        rest /= 10;
        if rest == 0 || start == 0 {
            break;
        }
    }
    digits.get(start..).unwrap_or(&[])
}

/// Reads an HTTP proxy's answer to `CONNECT`, collecting its head in
/// `buffer` across calls: `Ok(Some(used))` once a success's head is whole,
/// `used` being the bytes of `input` it took (what follows is the
/// tunnel's), `Ok(None)` when more is needed.
///
/// # Errors
/// A malformed head, a head longer than `buffer`, or any status but
/// success.
pub fn read_connect_reply(
    collector: &mut HeadCollector,
    buffer: &mut [u8],
    input: &[u8],
) -> Result<Option<usize>, ProxyError> {
    let (used, complete) = collector.feed(buffer, input).map_err(|_| ProxyError::Malformed)?;
    if !complete {
        return Ok(None);
    }
    let head = buffer.get(..collector.len()).ok_or(ProxyError::Malformed)?;
    let parsed = parse_head(head, &mut |_| {}).map_err(|_| ProxyError::Malformed)?;
    if HTTP_SUCCESS.contains(&parsed.status) { Ok(Some(used)) } else { Err(ProxyError::Refused(parsed.status)) }
}
