//! Constants of the web standards this crate implements, each defined once
//! with the clause it comes from (implementation decision 6). Teletext
//! values come only from `neoteletext_core::consts`.

// ---------------------------------------------------------------------------
// Time: the caller's clock counts microseconds (REQ-GEN-005).
// ---------------------------------------------------------------------------

/// Microseconds in one second.
pub const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
/// Seconds in one day, for cookie lifetimes.
pub const SECONDS_PER_DAY: u64 = 86_400;

// ---------------------------------------------------------------------------
// Addresses (REQ-WEB-015, REQ-WEB-016, RFC 3986)
// ---------------------------------------------------------------------------

/// The longest address accepted, in characters (REQ-WEB-016).
pub const MAX_URL_LENGTH: usize = 2048;
/// The longest DNS name accepted (REQ-WEB-016; RFC 1035 section 2.3.4).
pub const MAX_HOST_NAME_LENGTH: usize = 253;
/// The longest DNS label (RFC 1035 section 2.3.4).
pub const MAX_LABEL_LENGTH: usize = 63;
/// Room for a host as written in an address: a DNS name, or the longest
/// IPv6 text form `[ffff:...:255.255.255.255]` in brackets.
pub const MAX_HOST_TEXT_LENGTH: usize = MAX_HOST_NAME_LENGTH + 2;
/// Default port of `http` and `ws` (RFC 9110 section 4.2.1, RFC 6455
/// section 3).
pub const PORT_HTTP: u16 = 80;
/// Default port of `https` and `wss` (RFC 9110 section 4.2.2, RFC 6455
/// section 3).
pub const PORT_HTTPS: u16 = 443;
/// The most decimal digits a port may have (65535).
pub const MAX_PORT_DIGITS: usize = 5;
/// Octets of an IPv4 address.
pub const IPV4_OCTETS: usize = 4;
/// The most decimal digits of one IPv4 octet.
pub const MAX_IPV4_OCTET_DIGITS: usize = 3;
/// 16-bit groups of an IPv6 address (RFC 4291 section 2.2).
pub const IPV6_GROUPS: usize = 8;
/// The most hexadecimal digits of one IPv6 group (RFC 4291 section 2.2).
pub const MAX_IPV6_GROUP_DIGITS: usize = 4;

// ---------------------------------------------------------------------------
// HTTP/1.1 (RFC 9110, RFC 9112) and the profile of REQ-WEB-014
// ---------------------------------------------------------------------------

/// The HTTP version written in requests (RFC 9112 section 2.3).
pub const HTTP_VERSION: &[u8] = b"HTTP/1.1";
/// Line ending (RFC 9112 section 2.1).
pub const CRLF: &[u8] = b"\r\n";
/// The name that starts every HTTP version, case-sensitive (RFC 9112
/// section 2.3: `HTTP-name = %s"HTTP"`, then "/").
pub const HTTP_NAME: &[u8] = b"HTTP/";
/// The major version of the protocol this agent speaks (RFC 9112 section
/// 2.3).
pub const HTTP_MAJOR: u8 = 1;
/// The minor version of an HTTP/1.0 response.
pub const HTTP_MINOR_0: u8 = 0;
/// The minor version of an HTTP/1.1 response.
pub const HTTP_MINOR_1: u8 = 1;
/// Digits of a status code (RFC 9112 section 4: `status-code = 3DIGIT`).
pub const STATUS_CODE_DIGITS: usize = 3;
/// Status 101 Switching Protocols (RFC 9110 section 15.2.2).
pub const STATUS_SWITCHING_PROTOCOLS: u16 = 101;
/// Status 200 OK.
pub const STATUS_OK: u16 = 200;
/// Status 204 No Content: never has a body (RFC 9112 section 6.3 rule 1).
pub const STATUS_NO_CONTENT: u16 = 204;
/// Status 301 Moved Permanently.
pub const STATUS_MOVED_PERMANENTLY: u16 = 301;
/// Status 302 Found.
pub const STATUS_FOUND: u16 = 302;
/// Status 303 See Other.
pub const STATUS_SEE_OTHER: u16 = 303;
/// Status 304 Not Modified: never has a body (RFC 9112 section 6.3 rule 1).
pub const STATUS_NOT_MODIFIED: u16 = 304;
/// Status 307 Temporary Redirect: the method is kept.
pub const STATUS_TEMPORARY_REDIRECT: u16 = 307;
/// Status 308 Permanent Redirect: the method is kept.
pub const STATUS_PERMANENT_REDIRECT: u16 = 308;
/// Status 404 Not Found: "page not in this service" (REQ-WEB-003).
pub const STATUS_NOT_FOUND: u16 = 404;
/// 410 Gone: a site that removed its descriptor has none (RFC 9110 section 15.5.11).
pub const STATUS_GONE: u16 = 410;
/// Statuses from 100 to 199 are informational and carry no body.
pub const STATUS_INFORMATIONAL_FIRST: u16 = 100;
/// The first successful status.
pub const STATUS_SUCCESS_FIRST: u16 = 200;
/// The first redirection status.
pub const STATUS_REDIRECT_FIRST: u16 = 300;
/// The first client error status.
pub const STATUS_CLIENT_ERROR_FIRST: u16 = 400;
/// One past the last valid status code (RFC 9110 section 15).
pub const STATUS_LIMIT: u16 = 600;
/// At most this many redirects are followed (REQ-WEB-003).
pub const MAX_REDIRECTS: u8 = 5;
/// The most headers one response may carry (REQ-SEC-011).
pub const MAX_RESPONSE_HEADERS: usize = 64;
/// The most hexadecimal digits of a chunk size: chunks are bounded by the
/// body limit, which is far below 16^8 bytes (REQ-SEC-011).
pub const MAX_CHUNK_SIZE_DIGITS: usize = 8;
/// The longest chunk-size line, extensions included (REQ-SEC-011).
pub const MAX_CHUNK_LINE: usize = 256;
/// The most decimal digits of a Content-Length that can fit any limit.
pub const MAX_CONTENT_LENGTH_DIGITS: usize = 10;
/// The longest validator (`ETag` or `Last-Modified`) kept for a page.
pub const MAX_VALIDATOR_LENGTH: usize = 128;

/// Default limit of one page response body (REQ-WEB-008).
pub const DEFAULT_PAGE_LIMIT: usize = 256 * 1024;
/// Default limit of one push message (REQ-WEB-008).
pub const DEFAULT_PUSH_LIMIT: usize = 64 * 1024;
/// Default limit of the service descriptor (REQ-WEB-008).
pub const DEFAULT_DESCRIPTOR_LIMIT: usize = 16 * 1024;
/// Default limit of the headers of one response (REQ-WEB-008).
pub const DEFAULT_HEADER_LIMIT: usize = 16 * 1024;
/// Default room for one outgoing request: request line, headers and
/// cookies.
pub const DEFAULT_REQUEST_LIMIT: usize = 16 * 1024;
/// Default room for an encoded form body.
pub const DEFAULT_FORM_LIMIT: usize = 4 * 1024;

/// Header names this client sends or reads (REQ-WEB-014). Compared without
/// regard to letter case (RFC 9110 section 5.1).
pub mod header {
    /// `Host` (RFC 9110 section 7.2).
    pub const HOST: &[u8] = b"Host";
    /// `User-Agent` (RFC 9110 section 10.1.5).
    pub const USER_AGENT: &[u8] = b"User-Agent";
    /// `Accept` (RFC 9110 section 12.5.1).
    pub const ACCEPT: &[u8] = b"Accept";
    /// `Accept-Encoding` (RFC 9110 section 12.5.3).
    pub const ACCEPT_ENCODING: &[u8] = b"Accept-Encoding";
    /// `Cookie` (RFC 6265bis section 5.7).
    pub const COOKIE: &[u8] = b"Cookie";
    /// RFC 9110 section 11.6.2: sent only with a device-bound session's
    /// registration, when the server gave the value (REQ-WEB-017).
    pub const AUTHORIZATION: &[u8] = b"Authorization";
    /// `If-None-Match` (RFC 9110 section 13.1.2).
    pub const IF_NONE_MATCH: &[u8] = b"If-None-Match";
    /// `If-Modified-Since` (RFC 9110 section 13.1.3).
    pub const IF_MODIFIED_SINCE: &[u8] = b"If-Modified-Since";
    /// `Content-Type` (RFC 9110 section 8.3).
    pub const CONTENT_TYPE: &[u8] = b"Content-Type";
    /// `Content-Length` (RFC 9110 section 8.6).
    pub const CONTENT_LENGTH: &[u8] = b"Content-Length";
    /// `Content-Encoding` (RFC 9110 section 8.4).
    pub const CONTENT_ENCODING: &[u8] = b"Content-Encoding";
    /// `Transfer-Encoding` (RFC 9112 section 6.1).
    pub const TRANSFER_ENCODING: &[u8] = b"Transfer-Encoding";
    /// `Connection` (RFC 9110 section 7.6.1).
    pub const CONNECTION: &[u8] = b"Connection";
    /// `Location` (RFC 9110 section 10.2.2).
    pub const LOCATION: &[u8] = b"Location";
    /// `Set-Cookie` (RFC 6265bis section 4.1).
    pub const SET_COOKIE: &[u8] = b"Set-Cookie";
    /// `Clear-Site-Data` (W3C Clear Site Data section 3.1, decision E.104).
    pub const CLEAR_SITE_DATA: &[u8] = b"Clear-Site-Data";
    /// `ETag` (RFC 9110 section 8.8.3).
    pub const ETAG: &[u8] = b"ETag";
    /// `Last-Modified` (RFC 9110 section 8.8.2).
    pub const LAST_MODIFIED: &[u8] = b"Last-Modified";
    /// `Cache-Control` (RFC 9111 section 5.2).
    pub const CACHE_CONTROL: &[u8] = b"Cache-Control";
    /// `Retry-After` (RFC 9110 section 10.2.3).
    pub const RETRY_AFTER: &[u8] = b"Retry-After";
    /// `Last-Event-ID` (HTML Living Standard section 9.2.4).
    pub const LAST_EVENT_ID: &[u8] = b"Last-Event-ID";
    /// `Upgrade` (RFC 9110 section 7.8).
    pub const UPGRADE: &[u8] = b"Upgrade";
    /// `Sec-WebSocket-Key` (RFC 6455 section 11.3.1).
    pub const SEC_WEBSOCKET_KEY: &[u8] = b"Sec-WebSocket-Key";
    /// `Sec-WebSocket-Version` (RFC 6455 section 11.3.5).
    pub const SEC_WEBSOCKET_VERSION: &[u8] = b"Sec-WebSocket-Version";
    /// `Sec-WebSocket-Accept` (RFC 6455 section 11.3.3).
    pub const SEC_WEBSOCKET_ACCEPT: &[u8] = b"Sec-WebSocket-Accept";
    /// `Sec-WebSocket-Extensions` (RFC 6455 section 11.3.2).
    pub const SEC_WEBSOCKET_EXTENSIONS: &[u8] = b"Sec-WebSocket-Extensions";
    /// `Sec-WebSocket-Protocol` (RFC 6455 section 11.3.4).
    pub const SEC_WEBSOCKET_PROTOCOL: &[u8] = b"Sec-WebSocket-Protocol";
}

/// `Accept` for pages and the descriptor: the two page media types and JSON
/// (REQ-WEB-014).
pub const ACCEPT_PAGES: &[u8] = b"text/x-teletext-tti, application/x-teletext-t42, application/json";
/// `Accept` for a server-sent event stream (HTML Living Standard section
/// 9.2.2).
pub const ACCEPT_EVENT_STREAM: &[u8] = b"text/event-stream";
/// The only content coding asked for and accepted (REQ-SEC-011).
pub const IDENTITY: &[u8] = b"identity";
/// The only transfer coding accepted (RFC 9112 section 7.1).
pub const CHUNKED: &[u8] = b"chunked";
/// The `Connection` option that ends a persistent connection (RFC 9112
/// section 9.6).
pub const CONNECTION_CLOSE: &[u8] = b"close";
/// The `Clear-Site-Data` types that clear cookies, each a quoted string
/// compared exactly (W3C Clear Site Data sections 3.1 and 3.2).
pub const CLEAR_COOKIES: &[u8] = b"\"cookies\"";
/// The wildcard type, which clears every type.
pub const CLEAR_EVERYTHING: &[u8] = b"\"*\"";
/// The `Connection` option of a protocol upgrade (RFC 6455 section 4.1).
pub const CONNECTION_UPGRADE: &[u8] = b"Upgrade";
/// The upgrade token for WebSocket (RFC 6455 section 4.1).
pub const UPGRADE_WEBSOCKET: &[u8] = b"websocket";

/// Media types (REQ-WEB-003, REQ-WEB-002, REQ-WEB-005, REQ-WEB-011).
pub mod media {
    /// A TTI page file, defined by this project.
    pub const TTI: &[u8] = b"text/x-teletext-tti";
    /// A T42 packet stream, defined by this project.
    pub const T42: &[u8] = b"application/x-teletext-t42";
    /// Accepted for interoperability; the format is detected by the caller.
    pub const TEXT_PLAIN: &[u8] = b"text/plain";
    /// Accepted for interoperability; the format is detected by the caller.
    pub const OCTET_STREAM: &[u8] = b"application/octet-stream";
    /// The type common server configurations give `.bdf` files, accepted
    /// for fonts only (REQ-FNT-003).
    pub const FONT_BDF: &[u8] = b"application/x-font-bdf";
    /// The service descriptor (RFC 8259 section 11).
    pub const JSON: &[u8] = b"application/json";
    /// A server-sent event stream.
    pub const EVENT_STREAM: &[u8] = b"text/event-stream";
    /// Form submissions (WHATWG URL Standard section 5).
    pub const FORM_URLENCODED: &[u8] = b"application/x-www-form-urlencoded";
}

/// Cache-Control directives honoured (RFC 9111 section 5.2.2).
pub mod cache {
    /// `max-age`.
    pub const MAX_AGE: &[u8] = b"max-age";
    /// `no-cache`.
    pub const NO_CACHE: &[u8] = b"no-cache";
    /// `no-store`.
    pub const NO_STORE: &[u8] = b"no-store";
}

// ---------------------------------------------------------------------------
// WebSocket (RFC 6455)
// ---------------------------------------------------------------------------

/// The GUID appended to the key to form the accept value (RFC 6455
/// section 1.3).
pub const WEBSOCKET_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
/// The protocol version sent (RFC 6455 section 4.1 item 9).
pub const WEBSOCKET_VERSION: &[u8] = b"13";
/// Bytes of the random nonce behind `Sec-WebSocket-Key` (RFC 6455 section
/// 4.1 item 7).
pub const WEBSOCKET_KEY_BYTES: usize = 16;
/// Characters of the base64 form of the key.
pub const WEBSOCKET_KEY_TEXT: usize = 24;
/// Characters of the base64 form of the accept value (a SHA-1 digest).
pub const WEBSOCKET_ACCEPT_TEXT: usize = 28;
/// The FIN bit (RFC 6455 section 5.2).
pub const WS_FIN: u8 = 0x80;
/// The three reserved bits, which must be zero without extensions.
pub const WS_RSV_MASK: u8 = 0x70;
/// The opcode bits.
pub const WS_OPCODE_MASK: u8 = 0x0F;
/// The MASK bit of the second byte.
pub const WS_MASK_BIT: u8 = 0x80;
/// The payload length bits of the second byte.
pub const WS_LENGTH_MASK: u8 = 0x7F;
/// Payload length value meaning "a 16-bit length follows".
pub const WS_LENGTH_16: u8 = 126;
/// Payload length value meaning "a 64-bit length follows".
pub const WS_LENGTH_64: u8 = 127;
/// The largest payload expressible in the 7-bit length field.
pub const WS_LENGTH_7_MAX: usize = 125;
/// The largest control frame payload (RFC 6455 section 5.5).
pub const WS_CONTROL_MAX: usize = 125;
/// Bytes of the masking key (RFC 6455 section 5.3).
pub const WS_MASK_BYTES: usize = 4;
/// The longest frame header: 2 bytes, an 8-byte length and a mask.
pub const WS_MAX_HEADER: usize = 14;
/// Opcode: continuation frame (RFC 6455 section 5.2).
pub const OPCODE_CONTINUATION: u8 = 0x0;
/// Opcode: text frame.
pub const OPCODE_TEXT: u8 = 0x1;
/// Opcode: binary frame.
pub const OPCODE_BINARY: u8 = 0x2;
/// Opcode: connection close.
pub const OPCODE_CLOSE: u8 = 0x8;
/// Opcode: ping.
pub const OPCODE_PING: u8 = 0x9;
/// Opcode: pong.
pub const OPCODE_PONG: u8 = 0xA;
/// Opcodes from this value up are control frames (RFC 6455 section 5.5).
pub const OPCODE_CONTROL_FIRST: u8 = 0x8;
/// Close status: normal closure (RFC 6455 section 7.4.1).
pub const CLOSE_NORMAL: u16 = 1000;
/// Close status: protocol error.
pub const CLOSE_PROTOCOL_ERROR: u16 = 1002;
/// Close status: data inconsistent with the message type.
pub const CLOSE_INVALID_DATA: u16 = 1007;
/// Close status: message too big.
pub const CLOSE_TOO_BIG: u16 = 1009;
/// Bytes of a close status code.
pub const CLOSE_CODE_BYTES: usize = 2;
/// The WebSocket text message that subscribes to pages (REQ-WEB-005).
pub const SUBSCRIBE: &[u8] = b"subscribe";
/// The subscription to the whole service (REQ-WEB-005).
pub const SUBSCRIBE_ALL: &[u8] = b"all";

// ---------------------------------------------------------------------------
// Server-sent events (HTML Living Standard section 9.2) and push
// ---------------------------------------------------------------------------

/// The event carrying a TTI page (REQ-WEB-005).
pub const EVENT_PAGE: &[u8] = b"page";
/// The event carrying base64 T42 packets (REQ-WEB-005).
pub const EVENT_PACKETS: &[u8] = b"packets";
/// The longest event identifier kept for `Last-Event-ID`.
pub const MAX_EVENT_ID: usize = 256;
/// The longest event type name kept.
pub const MAX_EVENT_TYPE: usize = 32;
/// The query parameter naming the pages subscribed to over server-sent
/// events (REQ-WEB-005).
pub const SUBSCRIBE_QUERY: &[u8] = b"subscribe=";
/// The first reconnection delay (REQ-WEB-005).
pub const PUSH_RETRY_FIRST_US: u64 = MICROSECONDS_PER_SECOND;
/// The longest reconnection delay (REQ-WEB-005).
pub const PUSH_RETRY_MAX_US: u64 = 60 * MICROSECONDS_PER_SECOND;
/// REQ-WEB-005's heartbeat: a push WebSocket silent this long is pinged.
/// 25 seconds is below the idle timeout of about 30 seconds common on NATs
/// and mobile networks, the interval qrkeyboard's relay keeps alive at.
pub const PUSH_PING_AFTER_US: u64 = 25 * MICROSECONDS_PER_SECOND;
/// A ping not answered within this long means the connection is dead.
pub const PUSH_PONG_WITHIN_US: u64 = 10 * MICROSECONDS_PER_SECOND;
/// An event stream silent this long is made again: the client cannot ping
/// it.
pub const PUSH_EVENTS_SILENCE_US: u64 = 60 * MICROSECONDS_PER_SECOND;
/// The longest input message a receiver sends on the push WebSocket:
/// `neoteletext_input::MAX_MESSAGE` (`neoteletext-common/INPUT.md` section
/// 5.1, REQ-INT-101). The command-line program asserts that the two agree.
pub const INPUT_MESSAGE_MAX: usize = 4096;

// ---------------------------------------------------------------------------
// Cookies (RFC 6265bis draft of 30 September 2026, REQ-WEB-006)
// ---------------------------------------------------------------------------

/// The longest name and value together (RFC 6265bis section 5.6 step 4).
pub const MAX_COOKIE_NAME_VALUE: usize = 4096;
/// The longest attribute value (RFC 6265bis section 5.6 step 6).
pub const MAX_COOKIE_ATTRIBUTE_VALUE: usize = 1024;
/// The longest lifetime, in days (RFC 6265bis section 5.6.1 and 5.6.2).
pub const MAX_COOKIE_AGE_DAYS: u64 = 400;
/// Default cookies per host (REQ-WEB-006).
pub const DEFAULT_COOKIES_PER_HOST: usize = 50;
/// Default cookies in total (REQ-WEB-006).
pub const DEFAULT_COOKIES_TOTAL: usize = 100;
/// The `__Secure-` prefix (RFC 6265bis section 4.1.3.1).
pub const PREFIX_SECURE: &[u8] = b"__Secure-";
/// The `__Host-` prefix (RFC 6265bis section 4.1.3.2).
pub const PREFIX_HOST: &[u8] = b"__Host-";
/// The `__Http-` prefix (RFC 6265bis section 4.1.3.3).
pub const PREFIX_HTTP: &[u8] = b"__Http-";
/// The `__Host-Http-` prefix (RFC 6265bis section 4.1.3.4).
pub const PREFIX_HOST_HTTP: &[u8] = b"__Host-Http-";

/// Cookie attribute names (RFC 6265bis section 5.6).
pub mod cookie_attribute {
    /// `Expires`.
    pub const EXPIRES: &[u8] = b"expires";
    /// `Max-Age`.
    pub const MAX_AGE: &[u8] = b"max-age";
    /// `Domain`.
    pub const DOMAIN: &[u8] = b"domain";
    /// `Path`.
    pub const PATH: &[u8] = b"path";
    /// `Secure`.
    pub const SECURE: &[u8] = b"secure";
    /// `HttpOnly`.
    pub const HTTP_ONLY: &[u8] = b"httponly";
    /// `SameSite`.
    pub const SAME_SITE: &[u8] = b"samesite";
    /// `Partitioned`.
    pub const PARTITIONED: &[u8] = b"partitioned";
    /// `SameSite` value `Strict`.
    pub const STRICT: &[u8] = b"strict";
    /// `SameSite` value `Lax`.
    pub const LAX: &[u8] = b"lax";
    /// `SameSite` value `None`.
    pub const NONE: &[u8] = b"none";
}

// ---------------------------------------------------------------------------
// Encodings
// ---------------------------------------------------------------------------

/// The base64 alphabet (RFC 4648 section 4, Table 1).
pub const BASE64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
/// The base64 pad character (RFC 4648 section 4).
pub const BASE64_PAD: u8 = b'=';
/// Bytes per base64 group.
pub const BASE64_GROUP_BYTES: usize = 3;
/// Characters per base64 group.
pub const BASE64_GROUP_CHARS: usize = 4;
/// Bits per base64 character.
pub const BASE64_BITS: u32 = 6;
/// The digest length of SHA-1 (RFC 3174 section 1).
pub const SHA1_DIGEST_BYTES: usize = 20;
/// The block length of SHA-1 (RFC 3174 section 4).
pub const SHA1_BLOCK_BYTES: usize = 64;

/// Bytes of a SHA-256 digest (FIPS 180-4 section 1), which names a font
/// (REQ-FNT-006).
pub const SHA256_DIGEST_BYTES: usize = 32;
/// Bytes of a SHA-256 message block (FIPS 180-4 section 5.2.1).
pub const SHA256_BLOCK_BYTES: usize = 64;
/// Hexadecimal digits that write a SHA-256 digest (REQ-FNT-004).
pub const SHA256_HEX_DIGITS: usize = 2 * SHA256_DIGEST_BYTES;
/// The deepest JSON nesting the reader accepts (RFC 8259 section 9 allows
/// a limit).
pub const MAX_JSON_DEPTH: usize = 16;

// ---------------------------------------------------------------------------
// Service descriptor (REQ-WEB-002)
// ---------------------------------------------------------------------------

/// The most permitted form origins a descriptor may list.
pub const MAX_FORM_ORIGINS: usize = 8;
/// The most other services a descriptor may list.
pub const MAX_LINKED_SERVICES: usize = 16;
/// The longest service name or display name kept.
pub const MAX_SERVICE_NAME: usize = 64;
/// The longest short name of a linked service (Appendix E.1).
pub const MAX_SHORT_NAME: usize = 32;
/// The highest character set region (decision E.4): a 4-bit group.
pub const MAX_REGION: u8 = 15;

// ---------------------------------------------------------------------------
// Forms (REQ-WEB-011, Appendix C)
// ---------------------------------------------------------------------------

/// The field carrying the form token (REQ-WEB-011).
pub const FIELD_TOKEN: &[u8] = b"ttx_token";
/// The field naming the submit button pressed (REQ-WEB-011).
pub const FIELD_ACTION: &[u8] = b"ttx_action";

// ---------------------------------------------------------------------------
// The zxnet live feed (REQ-INP-022)
// ---------------------------------------------------------------------------

/// Keepalive interval (REQ-INP-022).
pub const ZXNET_KEEPALIVE_US: u64 = 5 * MICROSECONDS_PER_SECOND;
/// Reconnection delay (REQ-INP-022).
pub const ZXNET_RECONNECT_US: u64 = 10 * MICROSECONDS_PER_SECOND;
/// Failed attempts before giving up (REQ-INP-022).
pub const ZXNET_MAX_ATTEMPTS: u8 = 5;
/// Silence that counts as a dropped connection (REQ-INP-022).
pub const ZXNET_INACTIVITY_US: u64 = 5 * MICROSECONDS_PER_SECOND;
/// The protocol version understood (REQ-INP-022).
pub const ZXNET_API_VERSION: u8 = 1;
/// The slot of the displayed page (REQ-INP-022).
pub const ZXNET_DISPLAY_SLOT: u8 = 0;
/// The page walk's default hold timeout: twice the longest page repeat
/// seen in the 1 October 2026 probe (about 6 seconds on `ch4`), so a page
/// is held through at least one full cycle (research/answers-teletext-
/// sources.md section 2.1).
pub const ZXNET_WALK_TIMEOUT_US: u64 = 12 * MICROSECONDS_PER_SECOND;
/// After a walked page's last row arrives, the walk waits this long for
/// more rows before moving on, so a page is never left mid-transmission.
pub const ZXNET_WALK_SETTLE_US: u64 = MICROSECONDS_PER_SECOND;
/// The most pages the walk remembers from `pageExists` (8 magazines of
/// 256 page numbers).
pub const ZXNET_MAX_PAGES: usize = 2048;

// ---------------------------------------------------------------------------
// The user agent's defaults (REQ-WEB-004, REQ-SEC-011, REQ-SEC-020)
// ---------------------------------------------------------------------------

/// Default shortest interval between page fetches: five per second, so
/// next-page and pre-fetching cannot flood a server (REQ-WEB-004).
pub const DEFAULT_FETCH_INTERVAL_US: u64 = MICROSECONDS_PER_SECOND / 5;
/// Default time limit for a connection to open (REQ-SEC-011).
pub const DEFAULT_CONNECT_TIMEOUT_US: u64 = 10 * MICROSECONDS_PER_SECOND;
/// Default time limit for a whole request and response (REQ-SEC-011).
pub const DEFAULT_RESPONSE_TIMEOUT_US: u64 = 30 * MICROSECONDS_PER_SECOND;
/// The longest `Retry-After` honoured: a server cannot silence the client
/// for longer than this.
pub const MAX_RETRY_AFTER_US: u64 = 600 * MICROSECONDS_PER_SECOND;
/// Page requests waiting for the fetch connection.
pub const PAGE_QUEUE: usize = 16;
/// Pages whose validators and freshness are remembered (REQ-WEB-004).
pub const CACHE_ENTRIES: usize = 32;
/// Hosts remembered as reached over HTTPS this session (REQ-SEC-007).
pub const SECURE_HOSTS: usize = 32;
/// Origins whose first form submission has been confirmed (REQ-SEC-009).
pub const CONFIRMED_ORIGINS: usize = 16;
/// Local-network addresses the configuration may allow (REQ-SEC-006).
pub const ALLOWED_LOCAL: usize = 4;
/// Pages a push subscription may name (REQ-WEB-005).
pub const MAX_SUBSCRIBED_PAGES: usize = 32;
