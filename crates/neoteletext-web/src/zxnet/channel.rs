//! A feed's channel (REQ-INP-022): the identifier the zxnet viewer uses,
//! base64 of a JSON array of the service name, the host and the path, and
//! the `wss` address it names.

use super::{KNOWN_CHANNELS, MAX_IDENTIFIER, MAX_SERVICE};
use crate::base64;
use crate::bytes::{TextBuf, Writer};
use crate::json::{JsonReader, Token};
use crate::url::Url;

/// Why a channel could not be used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelError {
    /// Not base64.
    Base64,
    /// Not a JSON array of three non-empty strings.
    Json,
    /// The host and path do not form a valid `wss` address.
    Address,
    /// Too long to keep.
    TooLong,
    /// No feed has this short name.
    UnknownName,
}

/// A feed: its identifier, service name and address.
#[derive(Clone, Copy, Debug)]
pub struct Channel {
    identifier: TextBuf<MAX_IDENTIFIER>,
    service: TextBuf<MAX_SERVICE>,
    address: Url,
}

impl Channel {
    /// Decodes a channel identifier: base64 of a JSON array of the service
    /// name, the host and the path.
    pub fn from_identifier(identifier: &str) -> Result<Self, ChannelError> {
        let mut decoded = [0u8; MAX_IDENTIFIER];
        let len = base64::decode(identifier.as_bytes(), &mut decoded).map_err(|_| ChannelError::Base64)?;
        let json = decoded.get(..len).unwrap_or(&[]);
        let mut reader = JsonReader::new(json).map_err(|_| ChannelError::Json)?;
        if reader.next_token() != Ok(Some(Token::BeginArray)) {
            return Err(ChannelError::Json);
        }
        let mut parts: [TextBuf<MAX_IDENTIFIER>; 3] = [TextBuf::new(); 3];
        for part in &mut parts {
            let Ok(Some(Token::Str(value))) = reader.next_token() else { return Err(ChannelError::Json) };
            let mut buffer = [0u8; MAX_IDENTIFIER];
            let len = value.decode_into(&mut buffer).map_err(|_| ChannelError::TooLong)?;
            let text = buffer.get(..len).unwrap_or(&[]);
            if text.is_empty() || text.iter().any(|&b| !(0x21..=0x7E).contains(&b) || b == b',') {
                return Err(ChannelError::Json);
            }
            *part = TextBuf::from_bytes(text).ok_or(ChannelError::TooLong)?;
        }
        if reader.next_token() != Ok(Some(Token::EndArray)) || reader.next_token() != Ok(None) {
            return Err(ChannelError::Json);
        }
        let [service, host, path] = parts;
        if host.as_bytes().iter().any(|&b| b == b'/' || b == b'?') || path.as_bytes().first() != Some(&b'/') {
            return Err(ChannelError::Address);
        }
        let mut text = [0u8; crate::consts::MAX_URL_LENGTH];
        let mut writer = Writer::new(&mut text);
        let built =
            writer.put(b"wss://").and_then(|()| writer.put(host.as_bytes())).and_then(|()| writer.put(path.as_bytes()));
        built.map_err(|_| ChannelError::TooLong)?;
        let address = core::str::from_utf8(writer.written()).map_err(|_| ChannelError::Address)?;
        let address = Url::parse(address).map_err(|_| ChannelError::Address)?;
        Ok(Self {
            identifier: TextBuf::from_bytes(identifier.as_bytes()).ok_or(ChannelError::TooLong)?,
            service: TextBuf::from_bytes(service.as_bytes()).ok_or(ChannelError::TooLong)?,
            address,
        })
    }

    /// A known feed by its short name (REQ-INP-022's table). The short
    /// name is only a label: the `ch4` feed's service name is
    /// `channel4-capture`.
    pub fn from_short_name(name: &str) -> Result<Self, ChannelError> {
        let (_, identifier) =
            KNOWN_CHANNELS.iter().find(|(short, _)| *short == name).ok_or(ChannelError::UnknownName)?;
        Self::from_identifier(identifier)
    }

    /// A feed given by its `wss` address and service name; the identifier
    /// is built from them.
    pub fn from_address(address: &str, service: &str) -> Result<Self, ChannelError> {
        let url = Url::parse(address).map_err(|_| ChannelError::Address)?;
        if url.scheme() != crate::url::Scheme::Wss || url.query().is_some() {
            return Err(ChannelError::Address);
        }
        let printable = |text: &str| {
            !text.is_empty() && text.bytes().all(|b| (0x21..=0x7E).contains(&b) && !matches!(b, b'"' | b'\\' | b','))
        };
        if !printable(service) {
            return Err(ChannelError::Json);
        }
        let mut json = [0u8; MAX_IDENTIFIER];
        let mut writer = Writer::new(&mut json);
        let built = (|| {
            writer.put(b"[\"")?;
            writer.put(service.as_bytes())?;
            writer.put(b"\",\"")?;
            writer.put(url.authority())?;
            writer.put(b"\",\"")?;
            writer.put(url.path().as_bytes())?;
            writer.put(b"\"]")
        })();
        built.map_err(|_| ChannelError::TooLong)?;
        let mut identifier = [0u8; MAX_IDENTIFIER];
        let len = base64::encode(writer.written(), &mut identifier).map_err(|_| ChannelError::TooLong)?;
        Self::from_identifier(
            core::str::from_utf8(identifier.get(..len).unwrap_or(&[])).map_err(|_| ChannelError::Base64)?,
        )
    }

    /// The identifier, sent to the server as it is.
    #[must_use]
    pub fn identifier(&self) -> &str {
        self.identifier.as_str()
    }

    /// The service name: the identifier's first string.
    #[must_use]
    pub fn service(&self) -> &str {
        self.service.as_str()
    }

    /// The feed's address.
    #[must_use]
    pub const fn address(&self) -> &Url {
        &self.address
    }
}
