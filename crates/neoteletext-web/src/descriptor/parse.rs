//! The JSON member grammar of the descriptor (REQ-WEB-002) and of a
//! channel's `channel.json` (REQ-WEB-024): each member read once, a
//! duplicate or unknown one refusing the whole document.

use super::template::check_template;
use super::{Descriptor, DescriptorError, MAX_REFRESH_SECONDS, MIN_REFRESH_SECONDS, Member, PushMethod};
use crate::bytes::TextBuf;
use crate::consts::{MAX_REGION, MAX_URL_LENGTH};
use crate::json::{JsonReader, JsonStr, Token, integer};
use crate::url::Url;
use neoteletext_core::types::PageAddress;

/// Decodes a JSON string into a fixed buffer, as UTF-8 text.
pub(super) fn text<const N: usize>(value: JsonStr<'_>, member: Member) -> Result<TextBuf<N>, DescriptorError> {
    let mut buffer = [0u8; N];
    let len = value.decode_into(&mut buffer).map_err(|_| DescriptorError::Invalid(member))?;
    let decoded = buffer.get(..len).unwrap_or(&[]);
    if decoded.is_empty() || decoded.iter().any(|&b| b < 0x20 || b == 0x7F) {
        return Err(DescriptorError::Invalid(member));
    }
    TextBuf::from_bytes(decoded).ok_or(DescriptorError::Invalid(member))
}

pub(super) fn expect_string(token: Option<Token<'_>>, member: Member) -> Result<JsonStr<'_>, DescriptorError> {
    match token {
        Some(Token::Str(value)) => Ok(value),
        _ => Err(DescriptorError::Invalid(member)),
    }
}

/// Parses `mpp` written as three hexadecimal digits, magazine 1 to 8.
pub(super) fn parse_mpp(text: &[u8]) -> Option<PageAddress> {
    if text.len() != 3 {
        return None;
    }
    let value = text.iter().try_fold(0u16, |value, &d| Some((value << 4) | u16::from(crate::bytes::hex_value(d)?)))?;
    PageAddress::from_mpp(value)
}

/// Resolves an address member against the descriptor's address and applies
/// REQ-SEC-007.
pub(super) fn address(base: &Url, value: &str, member: Member) -> Result<Url, DescriptorError> {
    let url = base.join(value).map_err(|error| DescriptorError::Address(member, error))?;
    if base.is_secure() && !url.is_secure() {
        return Err(DescriptorError::Downgrade(member));
    }
    Ok(url)
}

/// A member that is a whole number from `least` to `most`.
fn bounded_integer(reader: &mut JsonReader<'_>, member: Member, least: u32, most: u32) -> Result<u32, DescriptorError> {
    let value = match reader.next_token()? {
        Some(Token::Number(number)) => integer(number),
        _ => None,
    };
    value
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| (least..=most).contains(value))
        .ok_or(DescriptorError::Invalid(member))
}

/// Marks a member as seen, refusing a second one.
fn first(seen: &mut bool, member: Member) -> Result<(), DescriptorError> {
    if *seen {
        return Err(DescriptorError::Duplicate(member));
    }
    *seen = true;
    Ok(())
}

impl Descriptor {
    pub(super) fn parse_with(json: &[u8], base: &Url, channel: bool) -> Result<Self, DescriptorError> {
        let mut reader = JsonReader::new(json)?;
        if reader.next_token()? != Some(Token::BeginObject) {
            return Err(DescriptorError::Invalid(Member::Document));
        }
        let mut descriptor = Self::defaults(base, channel)?;
        descriptor.base = *base;
        let mut seen = [false; 8];
        let [seen_name, seen_page, seen_template, seen_push, seen_region, seen_services, seen_aliases, seen_refresh] =
            &mut seen;
        while let Some(token) = reader.next_token()? {
            let key = match token {
                Token::Key(key) => key,
                Token::EndObject => break,
                _ => return Err(DescriptorError::Invalid(Member::Document)),
            };
            if key.equals(b"name") {
                first(seen_name, Member::Name)?;
                descriptor.name = text(expect_string(reader.next_token()?, Member::Name)?, Member::Name)?;
            } else if key.equals(b"initial_page") {
                first(seen_page, Member::InitialPage)?;
                let value = expect_string(reader.next_token()?, Member::InitialPage)?;
                descriptor.initial_page =
                    value.plain().and_then(parse_mpp).ok_or(DescriptorError::Invalid(Member::InitialPage))?;
            } else if key.equals(b"page_template") {
                if channel {
                    return Err(DescriptorError::NotForChannels(Member::PageTemplate));
                }
                first(seen_template, Member::PageTemplate)?;
                descriptor.read_template(&mut reader, base)?;
            } else if key.equals(b"push") {
                first(seen_push, Member::Push)?;
                descriptor.read_push(&mut reader, base, channel)?;
            } else if key.equals(b"region") {
                first(seen_region, Member::Region)?;
                let region = bounded_integer(&mut reader, Member::Region, 0, u32::from(MAX_REGION))?;
                descriptor.region = Some(u8::try_from(region).map_err(|_| DescriptorError::Invalid(Member::Region))?);
            } else if key.equals(b"services") {
                first(seen_services, Member::Services)?;
                descriptor.parse_services(&mut reader, base)?;
            } else if key.equals(b"aliases") {
                first(seen_aliases, Member::Aliases)?;
                descriptor.parse_aliases(&mut reader, base)?;
            } else if key.equals(b"refresh") {
                first(seen_refresh, Member::Refresh)?;
                descriptor.refresh_seconds =
                    Some(bounded_integer(&mut reader, Member::Refresh, MIN_REFRESH_SECONDS, MAX_REFRESH_SECONDS)?);
            } else {
                return Err(DescriptorError::Unknown);
            }
        }
        reader.next_token()?;
        if channel {
            return Ok(descriptor);
        }
        if !*seen_name {
            return Err(DescriptorError::Missing(Member::Name));
        }
        if !*seen_page {
            return Err(DescriptorError::Missing(Member::InitialPage));
        }
        if !*seen_template {
            return Err(DescriptorError::Missing(Member::PageTemplate));
        }
        Ok(descriptor)
    }

    /// The descriptor before any member is read: a channel's defaults for
    /// its folder, or an empty service.
    fn defaults(base: &Url, channel: bool) -> Result<Self, DescriptorError> {
        Ok(if channel {
            let folder = base.join(".").map_err(|error| DescriptorError::Address(Member::Document, error))?;
            Self::channel_defaults(&folder)
        } else {
            Self::empty(base, false, TextBuf::new())
        })
    }

    /// Reads the `page_template` member and checks that it expands to an
    /// address no less secure than the descriptor's.
    fn read_template(&mut self, reader: &mut JsonReader<'_>, base: &Url) -> Result<(), DescriptorError> {
        let value: TextBuf<MAX_URL_LENGTH> =
            text(expect_string(reader.next_token()?, Member::PageTemplate)?, Member::PageTemplate)?;
        check_template(value.as_bytes())?;
        self.template = value;
        // One expansion that parses proves every expansion does (see
        // `check_template`).
        let sample = self
            .page_url(self.initial_page, None)
            .map_err(|error| DescriptorError::Address(Member::PageTemplate, error))?;
        if base.is_secure() && !sample.is_secure() {
            return Err(DescriptorError::Downgrade(Member::PageTemplate));
        }
        Ok(())
    }

    /// Reads the `push` member and checks its origin. A channel's is always
    /// at its folder's `push` (neoteletext-common's REQ-1693 item 2), so it
    /// names no address.
    fn read_push(&mut self, reader: &mut JsonReader<'_>, base: &Url, channel: bool) -> Result<(), DescriptorError> {
        let (url, method, input) = Self::parse_push(reader, base, channel)?;
        // Keys go back only to the descriptor's own origin, for a
        // service as for a channel (INPUT.md section 4, finding F-90).
        if input && !url.origin().same(&base.origin()) {
            return Err(DescriptorError::Invalid(Member::PushInput));
        }
        self.push = Some((url, method));
        self.input = input;
        Ok(())
    }

    fn parse_push(
        reader: &mut JsonReader<'_>,
        base: &Url,
        channel: bool,
    ) -> Result<(Url, PushMethod, bool), DescriptorError> {
        if reader.next_token()? != Some(Token::BeginObject) {
            return Err(DescriptorError::Invalid(Member::Push));
        }
        let mut url = None;
        let mut method = None;
        let mut input = None;
        while let Some(token) = reader.next_token()? {
            let key = match token {
                Token::Key(key) => key,
                Token::EndObject => break,
                _ => return Err(DescriptorError::Invalid(Member::Push)),
            };
            if key.equals(b"address") && !channel {
                if url.is_some() {
                    return Err(DescriptorError::Duplicate(Member::PushAddress));
                }
                let value: TextBuf<MAX_URL_LENGTH> =
                    text(expect_string(reader.next_token()?, Member::PushAddress)?, Member::PushAddress)?;
                url = Some(address(base, value.as_str(), Member::PushAddress)?);
            } else if key.equals(b"method") {
                if method.is_some() {
                    return Err(DescriptorError::Duplicate(Member::PushMethod));
                }
                let value = expect_string(reader.next_token()?, Member::PushMethod)?;
                method = Some(if value.equals(b"websocket") {
                    PushMethod::WebSocket
                } else if value.equals(b"sse") {
                    PushMethod::ServerSentEvents
                } else {
                    return Err(DescriptorError::Invalid(Member::PushMethod));
                });
            } else if key.equals(b"input") {
                if input.is_some() {
                    return Err(DescriptorError::Duplicate(Member::PushInput));
                }
                input = match reader.next_token()? {
                    Some(Token::Bool(value)) => Some(value),
                    _ => return Err(DescriptorError::Invalid(Member::PushInput)),
                };
            } else {
                return Err(DescriptorError::Unknown);
            }
        }
        let method = method.ok_or(DescriptorError::Missing(Member::PushMethod))?;
        let url = if channel {
            crate::channel::push_url(base, method == PushMethod::WebSocket)
                .map_err(|error| DescriptorError::Address(Member::PushAddress, error))?
        } else {
            url.ok_or(DescriptorError::Missing(Member::PushAddress))?
        };
        // The scheme must suit the method.
        if url.scheme().is_websocket() != (method == PushMethod::WebSocket) {
            return Err(DescriptorError::Invalid(Member::PushAddress));
        }
        // Keys go back only on a WebSocket (decision E.90).
        let input = input.unwrap_or(false);
        if input && method != PushMethod::WebSocket {
            return Err(DescriptorError::Invalid(Member::PushInput));
        }
        Ok((url, method, input))
    }
}
