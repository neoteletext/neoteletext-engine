//! The `SET_COOKIE` record (`neoteletext-common/INPUT.md` section 7.7): one
//! `Set-Cookie` header value, as an HTTP response carries it (RFC 6265bis
//! section 4.1), set while the push connection is open. The receiver
//! applies it with the parser and the rules of a response's header
//! (REQ-WEB-006); the server builds it from typed fields with
//! [`SetCookie::encode`], never by hand.

use neoteletext_audio::MAX_BODY;
use neoteletext_core::packet::Packet;

use super::{RecordFault, TYPE_SET_COOKIE, write_packets};
use crate::bytes::Writer;
use crate::cookie::SameSite;
use crate::http::is_token_character;

/// The longest line: the frame's largest body.
pub const MAX_LINE: usize = MAX_BODY;

/// The attributes' names and separators as RFC 6265bis section 4.1.1
/// writes them.
const SEPARATOR: &[u8] = b"; ";
const PATH: &[u8] = b"Path=";
const MAX_AGE: &[u8] = b"Max-Age=";
const SAME_SITE: &[u8] = b"SameSite=";
const SECURE: &[u8] = b"Secure";
const HTTP_ONLY: &[u8] = b"HttpOnly";

/// Whether a byte may be in the line: printable ASCII (section 7.7).
const fn is_line_byte(byte: u8) -> bool {
    matches!(byte, b' '..=b'~')
}

/// RFC 6265bis section 4.1.1's `cookie-octet`: printable ASCII but space,
/// the double quote, the comma, the semicolon and the backslash.
const fn is_cookie_octet(byte: u8) -> bool {
    matches!(byte, b'!' | b'#'..=b'+' | b'-'..=b':' | b'<'..=b'[' | b']'..=b'~')
}

/// Reads a `SET_COOKIE` record's body: the line, unparsed, for the cookie
/// jar's parser.
///
/// # Errors
/// An empty line, or a byte outside printable ASCII.
pub fn read(body: &[u8]) -> Result<&[u8], RecordFault> {
    if body.is_empty() || body.len() > MAX_LINE {
        return Err(RecordFault::Length);
    }
    if !body.iter().all(|&byte| is_line_byte(byte)) {
        return Err(RecordFault::BadValue("cookie line"));
    }
    Ok(body)
}

/// A cookie as a program gives it, typed (`SERVER.md` section 8.2): what
/// the server turns into the line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SetCookie<'a> {
    /// The name: an RFC 9110 token, never empty.
    pub name: &'a str,
    /// The value: RFC 6265bis cookie octets, possibly empty.
    pub value: &'a str,
    /// The path, beginning `/`, or `None` for the default, the channel's
    /// folder.
    pub path: Option<&'a str>,
    /// Seconds until it expires; 0 removes it; `None` for a session cookie.
    pub max_age: Option<u32>,
    /// The same-site rule; [`SameSite::Default`] writes none.
    pub same_site: SameSite,
    /// Sent only over an encrypted connection.
    pub secure: bool,
    /// Not shown to a page's scripts; kept for the line's meaning, since a
    /// teletext page runs none.
    pub http_only: bool,
}

impl SetCookie<'_> {
    /// Writes the line into `out` and returns its length, or `None` when
    /// a field breaks its rule or the line is longer than `out` or
    /// [`MAX_LINE`].
    #[must_use]
    pub fn line(&self, out: &mut [u8]) -> Option<usize> {
        let name_ok = !self.name.is_empty() && self.name.bytes().all(is_token_character);
        let value_ok = self.value.bytes().all(is_cookie_octet);
        let path_ok = self
            .path
            .is_none_or(|path| path.starts_with('/') && path.bytes().all(|byte| is_line_byte(byte) && byte != b';'));
        if !(name_ok && value_ok && path_ok) {
            return None;
        }
        let mut writer = Writer::new(out);
        (|| {
            writer.put(self.name.as_bytes())?;
            writer.byte(b'=')?;
            writer.put(self.value.as_bytes())?;
            if let Some(path) = self.path {
                writer.put(SEPARATOR)?;
                writer.put(PATH)?;
                writer.put(path.as_bytes())?;
            }
            if let Some(seconds) = self.max_age {
                writer.put(SEPARATOR)?;
                writer.put(MAX_AGE)?;
                writer.decimal(u64::from(seconds))?;
            }
            let same_site: Option<&[u8]> = match self.same_site {
                SameSite::Strict => Some(b"Strict"),
                SameSite::Lax => Some(b"Lax"),
                SameSite::None => Some(b"None"),
                SameSite::Default => None,
            };
            if let Some(rule) = same_site {
                writer.put(SEPARATOR)?;
                writer.put(SAME_SITE)?;
                writer.put(rule)?;
            }
            if self.secure {
                writer.put(SEPARATOR)?;
                writer.put(SECURE)?;
            }
            if self.http_only {
                writer.put(SEPARATOR)?;
                writer.put(HTTP_ONLY)?;
            }
            Ok::<(), crate::bytes::Overflow>(())
        })()
        .ok()?;
        let length = writer.len();
        (length <= MAX_LINE).then_some(length)
    }

    /// Writes the cookie as a `SET_COOKIE` record, packets 4/31 of audio
    /// service `service`, record id `record_id`. Returns the packets
    /// written.
    ///
    /// # Errors
    /// A field that breaks its rule, or a line over [`MAX_LINE`].
    pub fn encode(&self, service: u8, record_id: u8, out: &mut dyn FnMut(Packet)) -> Result<usize, RecordFault> {
        let mut line = [0_u8; MAX_LINE];
        let length = self.line(&mut line).ok_or(RecordFault::BadValue("cookie"))?;
        let line = read(line.get(..length).ok_or(RecordFault::Length)?)?;
        write_packets(TYPE_SET_COOKIE, line, service, record_id, out).ok_or(RecordFault::Length)
    }
}
