//! The descriptor's links to other sites: its aliases (the web-native
//! concept section 5 item 4) and its linked services (Appendix E,
//! REQ-WEB-020), each a bounded list over an arena of addresses.

use super::parse::{address, expect_string, parse_mpp, text};
use super::{Alias, Descriptor, DescriptorError, LinkedService, Member};
use crate::bytes::TextBuf;
use crate::consts::{MAX_SERVICE_NAME, MAX_SHORT_NAME, MAX_URL_LENGTH};
use crate::json::{JsonReader, Token};
use crate::url::Url;

impl Descriptor {
    /// `aliases`: an object of page numbers and their targets, each a
    /// named service's page (`name:mpp`) or an address (REQ-WEB-002,
    /// REQ-WEB-020).
    pub(super) fn parse_aliases(&mut self, reader: &mut JsonReader<'_>, base: &Url) -> Result<(), DescriptorError> {
        if reader.next_token()? != Some(Token::BeginObject) {
            return Err(DescriptorError::Invalid(Member::Aliases));
        }
        let mut count = 0usize;
        loop {
            let key = match reader.next_token()? {
                Some(Token::EndObject) => return Ok(()),
                Some(Token::Key(key)) => key,
                _ => return Err(DescriptorError::Invalid(Member::Aliases)),
            };
            let page = key.plain().and_then(parse_mpp).ok_or(DescriptorError::Invalid(Member::Aliases))?;
            if self.alias(page).is_some() {
                return Err(DescriptorError::Duplicate(Member::Aliases));
            }
            let value: TextBuf<MAX_URL_LENGTH> =
                text(expect_string(reader.next_token()?, Member::Aliases)?, Member::Aliases)?;
            let start = self.alias_targets.len();
            if value.as_bytes().windows(3).any(|window| window == b"://") {
                let url = address(base, value.as_str(), Member::Aliases)?;
                if url.scheme().is_websocket() {
                    return Err(DescriptorError::Invalid(Member::Aliases));
                }
                self.alias_targets.push(url.as_str().as_bytes())
            } else {
                // `name:mpp`, the form of Appendix E.
                let (name, target) = value.as_str().split_once(':').ok_or(DescriptorError::Invalid(Member::Aliases))?;
                let valid_name =
                    !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
                if !valid_name || parse_mpp(target.as_bytes()).is_none() {
                    return Err(DescriptorError::Invalid(Member::Aliases));
                }
                self.alias_targets.push(value.as_bytes())
            }
            .map_err(|_| DescriptorError::TooMany(Member::Aliases))?;
            let end = self.alias_targets.len();
            let narrow = |v: usize| u16::try_from(v).map_err(|_| DescriptorError::TooMany(Member::Aliases));
            let slot = self.aliases.get_mut(count).ok_or(DescriptorError::TooMany(Member::Aliases))?;
            *slot = Some(Alias { page, target_start: narrow(start)?, target_end: narrow(end)? });
            count = count.saturating_add(1);
        }
    }

    pub(super) fn parse_services(&mut self, reader: &mut JsonReader<'_>, base: &Url) -> Result<(), DescriptorError> {
        if reader.next_token()? != Some(Token::BeginArray) {
            return Err(DescriptorError::Invalid(Member::Services));
        }
        let mut count = 0usize;
        loop {
            match reader.next_token()? {
                Some(Token::EndArray) => return Ok(()),
                Some(Token::BeginObject) => {}
                _ => return Err(DescriptorError::Invalid(Member::Services)),
            }
            let mut short_name: Option<TextBuf<MAX_SHORT_NAME>> = None;
            let mut display_name: Option<TextBuf<MAX_SERVICE_NAME>> = None;
            let mut url: Option<Url> = None;
            let mut is_channel = false;
            while let Some(token) = reader.next_token()? {
                let key = match token {
                    Token::Key(key) => key,
                    Token::EndObject => break,
                    _ => return Err(DescriptorError::Invalid(Member::Services)),
                };
                if key.equals(b"name") {
                    let value = text(expect_string(reader.next_token()?, Member::ServiceName)?, Member::ServiceName)?;
                    // Appendix E.1: letters, digits and hyphens.
                    if short_name.is_some() || !value.as_bytes().iter().all(|&b| b.is_ascii_alphanumeric() || b == b'-')
                    {
                        return Err(DescriptorError::Invalid(Member::ServiceName));
                    }
                    short_name = Some(value);
                } else if key.equals(b"display_name") {
                    if display_name.is_some() {
                        return Err(DescriptorError::Duplicate(Member::ServiceDisplayName));
                    }
                    display_name = Some(text(
                        expect_string(reader.next_token()?, Member::ServiceDisplayName)?,
                        Member::ServiceDisplayName,
                    )?);
                } else if key.equals(b"descriptor") {
                    if url.is_some() {
                        return Err(DescriptorError::Duplicate(Member::ServiceDescriptor));
                    }
                    let value: TextBuf<MAX_URL_LENGTH> = text(
                        expect_string(reader.next_token()?, Member::ServiceDescriptor)?,
                        Member::ServiceDescriptor,
                    )?;
                    url = Some(address(base, value.as_str(), Member::ServiceDescriptor)?);
                } else if key.equals(b"channel") && self.channel {
                    if url.is_some() {
                        return Err(DescriptorError::Invalid(Member::ServiceChannel));
                    }
                    let value: TextBuf<MAX_URL_LENGTH> =
                        text(expect_string(reader.next_token()?, Member::ServiceChannel)?, Member::ServiceChannel)?;
                    let resolved = address(base, value.as_str(), Member::ServiceChannel)?;
                    // A linked channel is a folder (REQ-WEB-023).
                    match crate::channel::parse_address(resolved.as_str()) {
                        Ok(found) if found.file.is_none() => url = Some(found.folder),
                        _ => return Err(DescriptorError::Invalid(Member::ServiceChannel)),
                    }
                    is_channel = true;
                } else {
                    return Err(DescriptorError::Unknown);
                }
            }
            let short_name = short_name.ok_or(DescriptorError::Missing(Member::ServiceName))?;
            let display_name = display_name.ok_or(DescriptorError::Missing(Member::ServiceDisplayName))?;
            let url = url.ok_or(DescriptorError::Missing(Member::ServiceDescriptor))?;
            self.store_service(count, short_name, display_name, &url, is_channel)?;
            count = count.saturating_add(1);
        }
    }

    /// Stores linked service number `count`, refusing a second service of
    /// the same name, and its address in the arena.
    fn store_service(
        &mut self,
        count: usize,
        short_name: TextBuf<MAX_SHORT_NAME>,
        display_name: TextBuf<MAX_SERVICE_NAME>,
        url: &Url,
        is_channel: bool,
    ) -> Result<(), DescriptorError> {
        if self.service(short_name.as_str()).is_some() {
            return Err(DescriptorError::Duplicate(Member::ServiceName));
        }
        let start = self.service_addresses.len();
        self.service_addresses.push(url.as_str().as_bytes()).map_err(|_| DescriptorError::TooMany(Member::Services))?;
        let end = self.service_addresses.len();
        let narrow = |v: usize| u16::try_from(v).map_err(|_| DescriptorError::TooMany(Member::Services));
        let slot = self.services.get_mut(count).ok_or(DescriptorError::TooMany(Member::Services))?;
        *slot = Some(LinkedService {
            short_name,
            display_name,
            channel: is_channel,
            address_start: narrow(start)?,
            address_end: narrow(end)?,
        });
        Ok(())
    }
}
