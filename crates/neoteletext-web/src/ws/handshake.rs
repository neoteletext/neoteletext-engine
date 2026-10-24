//! The opening handshake (RFC 6455 section 4.1): the key, its expected
//! accept value, the request's headers and the check of the response.

use super::{HandshakeKey, WsError};
use crate::base64;
use crate::bytes::{Overflow, Writer};
use crate::consts::{
    STATUS_SWITCHING_PROTOCOLS, UPGRADE_WEBSOCKET, WEBSOCKET_ACCEPT_TEXT, WEBSOCKET_GUID, WEBSOCKET_KEY_BYTES,
    WEBSOCKET_KEY_TEXT, WEBSOCKET_VERSION, header,
};
use crate::http::{self, Method, ResponseHead};
use crate::port::RandomSource;
use crate::sha1;
use crate::url::Url;

/// Draws a new handshake key from the random source (RFC 6455 section 4.1
/// item 7).
pub fn new_key(random: &mut dyn RandomSource) -> HandshakeKey {
    let mut nonce = [0u8; WEBSOCKET_KEY_BYTES];
    random.fill(&mut nonce);
    let mut key = [0u8; WEBSOCKET_KEY_TEXT];
    // 16 bytes always encode to exactly 24 characters.
    let _ = base64::encode(&nonce, &mut key);
    key
}

/// The `Sec-WebSocket-Accept` value a server must return for `key` (RFC
/// 6455 section 4.2.2 item 5.4).
#[must_use]
pub fn accept_for(key: &[u8]) -> [u8; WEBSOCKET_ACCEPT_TEXT] {
    let digest = sha1::digest(&[key, WEBSOCKET_GUID]);
    let mut accept = [0u8; WEBSOCKET_ACCEPT_TEXT];
    // 20 bytes always encode to exactly 28 characters.
    let _ = base64::encode(&digest, &mut accept);
    accept
}

/// Writes the opening handshake request line and headers (RFC 6455
/// section 4.1): only the headers of the profile (REQ-WEB-014), with
/// `user_agent` (REQ-WEB-031). The caller adds the `Cookie` header, if any,
/// and the empty line that ends the request.
pub fn write_handshake_headers(
    writer: &mut Writer<'_>,
    url: &Url,
    key: &HandshakeKey,
    user_agent: &[u8],
) -> Result<(), Overflow> {
    http::write_request_line(writer, Method::Get, url.target())?;
    http::write_host(writer, url)?;
    http::write_header(writer, header::USER_AGENT, user_agent)?;
    http::write_header(writer, header::UPGRADE, UPGRADE_WEBSOCKET)?;
    http::write_header(writer, header::CONNECTION, crate::consts::CONNECTION_UPGRADE)?;
    http::write_header(writer, header::SEC_WEBSOCKET_KEY, key)?;
    http::write_header(writer, header::SEC_WEBSOCKET_VERSION, WEBSOCKET_VERSION)
}

/// Checks the handshake response (RFC 6455 section 4.1, the client's
/// checks on the server's response): status 101, `Upgrade: websocket`,
/// `Connection: Upgrade`, the right accept value, and no extension or
/// subprotocol, since none were requested.
pub fn check_handshake(head: &ResponseHead, key: &HandshakeKey) -> Result<(), WsError> {
    let accepted = head.status == STATUS_SWITCHING_PROTOCOLS
        && head.upgrade_websocket
        && head.connection_upgrade
        && head.websocket_accept == Some(accept_for(key))
        && !head.websocket_extras;
    if accepted { Ok(()) } else { Err(WsError::Handshake) }
}
