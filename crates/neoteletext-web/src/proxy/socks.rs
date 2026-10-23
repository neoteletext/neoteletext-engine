//! The SOCKS5 proxy handshake (RFC 1928, REQ-WEB-032): the greeting, the
//! connect request carrying the host's name or address, and the reply.

use super::ProxyError;
use crate::bytes::Writer;
use crate::url::{IpAddress, ip_literal};

/// The SOCKS protocol's version (RFC 1928 section 3).
const SOCKS_VERSION: u8 = 5;
/// "No authentication required" (RFC 1928 section 3).
const SOCKS_NO_AUTHENTICATION: u8 = 0;
/// The CONNECT command (RFC 1928 section 4).
const SOCKS_CONNECT: u8 = 1;
/// The address types (RFC 1928 section 5).
const SOCKS_IPV4: u8 = 1;
const SOCKS_NAME: u8 = 3;
const SOCKS_IPV6: u8 = 4;
/// The reserved byte (RFC 1928 sections 4 and 6).
const SOCKS_RESERVED: u8 = 0;
/// "Succeeded" (RFC 1928 section 6).
const SOCKS_SUCCEEDED: u8 = 0;
/// The longest name SOCKS5 carries: its length is one byte.
pub(super) const SOCKS_MAX_NAME: usize = 255;
/// The greeting: version 5, one method, no authentication.
pub const SOCKS_GREETING: [u8; 3] = [SOCKS_VERSION, 1, SOCKS_NO_AUTHENTICATION];
/// The longest SOCKS5 reply: the header, a name of 255 bytes, its length
/// and the port.
pub const SOCKS_MAX_REPLY: usize = 4 + 1 + SOCKS_MAX_NAME + 2;

/// Reads the SOCKS5 proxy's choice of method after [`SOCKS_GREETING`]:
/// two bytes.
///
/// # Errors
/// Another version, or a method but no authentication.
pub fn read_socks_choice(reply: [u8; 2]) -> Result<(), ProxyError> {
    match reply {
        [SOCKS_VERSION, SOCKS_NO_AUTHENTICATION] => Ok(()),
        [SOCKS_VERSION, _] => Err(ProxyError::NeedsAuthentication),
        _ => Err(ProxyError::Malformed),
    }
}

/// Writes the SOCKS5 request to connect to `host` and `port`: the address
/// when the host is one, otherwise the name for the proxy to resolve.
///
/// # Errors
/// A name longer than 255 bytes, or `out` too small.
pub fn write_socks_connect(host: &str, port: u16, out: &mut [u8]) -> Result<usize, ProxyError> {
    let mut writer = Writer::new(out);
    writer.put(&[SOCKS_VERSION, SOCKS_CONNECT, SOCKS_RESERVED]).map_err(|_| ProxyError::TooLong)?;
    match ip_literal(host) {
        Some(IpAddress::V4(octets)) => {
            writer.put(&[SOCKS_IPV4]).map_err(|_| ProxyError::TooLong)?;
            writer.put(&octets).map_err(|_| ProxyError::TooLong)?;
        }
        Some(IpAddress::V6(groups)) => {
            writer.put(&[SOCKS_IPV6]).map_err(|_| ProxyError::TooLong)?;
            for group in groups {
                writer.put(&group.to_be_bytes()).map_err(|_| ProxyError::TooLong)?;
            }
        }
        None => {
            let length = u8::try_from(host.len()).map_err(|_| ProxyError::TooLong)?;
            if host.is_empty() {
                return Err(ProxyError::TooLong);
            }
            writer.put(&[SOCKS_NAME, length]).map_err(|_| ProxyError::TooLong)?;
            writer.put(host.as_bytes()).map_err(|_| ProxyError::TooLong)?;
        }
    }
    writer.put(&port.to_be_bytes()).map_err(|_| ProxyError::TooLong)?;
    Ok(writer.written().len())
}

/// The length of the SOCKS5 reply that begins with `start`, its first five
/// bytes, so a transport knows how many to read.
///
/// # Errors
/// Another version, a reserved byte not zero, or an unknown address type.
pub fn socks_reply_length(start: [u8; 5]) -> Result<usize, ProxyError> {
    let [version, _, reserved, kind, first] = start;
    if version != SOCKS_VERSION || reserved != SOCKS_RESERVED {
        return Err(ProxyError::Malformed);
    }
    let address = match kind {
        SOCKS_IPV4 => 4,
        SOCKS_IPV6 => 16,
        SOCKS_NAME => 1 + usize::from(first),
        _ => return Err(ProxyError::Malformed),
    };
    // The version, reply, reserved and type bytes, then the address, then
    // the port's two.
    Ok(address.saturating_add(4 + 2))
}

/// Reads a whole SOCKS5 reply, of the length [`socks_reply_length`] gave.
///
/// # Errors
/// As [`socks_reply_length`], a reply of another length, or any reply code
/// but success.
pub fn read_socks_reply(reply: &[u8]) -> Result<(), ProxyError> {
    let start: [u8; 5] = reply.get(..5).and_then(|start| start.try_into().ok()).ok_or(ProxyError::Malformed)?;
    if socks_reply_length(start)? != reply.len() {
        return Err(ProxyError::Malformed);
    }
    match start.get(1).copied() {
        Some(SOCKS_SUCCEEDED) => Ok(()),
        Some(code) => Err(ProxyError::Refused(u16::from(code))),
        None => Err(ProxyError::Malformed),
    }
}
