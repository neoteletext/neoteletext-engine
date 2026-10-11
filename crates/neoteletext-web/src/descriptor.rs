//! The service descriptor (REQ-WEB-002) and the page address template
//! (REQ-WEB-003).
//!
//! The descriptor is a JSON object. REQUIREMENTS.md names its members but
//! not their spelling; this reader uses:
//!
//! ```json
//! {
//!   "name": "Example Teletext",
//!   "initial_page": "100",
//!   "page_template": "https://example.org/ttx/pages/{mpp}",
//!   "push": { "address": "wss://example.org/ttx/push", "method": "websocket" },
//!   "form_origins": ["https://forms.example.org"],
//!   "region": 0,
//!   "services": [
//!     { "name": "weather", "display_name": "Weather", "descriptor": "https://weather.example/ttx.json" }
//!   ]
//! }
//! ```
//!
//! `name`, `initial_page` and `page_template` are required. `method` is
//! `websocket` or `sse`. Addresses may be relative to the descriptor's own
//! address. A missing required member, a duplicated or unknown member, an
//! unknown template placeholder or a malformed address rejects the whole
//! descriptor (REQ-WEB-014: never partly interpreted). A descriptor fetched
//! over HTTPS that names a plain `http` or `ws` address is rejected
//! (REQ-SEC-007).

use neoteletext_core::types::{PageAddress, SubCode};

use crate::bytes::{TextBuf, Writer};
use crate::consts::{
    MAX_FORM_ORIGINS, MAX_LINKED_SERVICES, MAX_REGION, MAX_SERVICE_NAME, MAX_SHORT_NAME, MAX_URL_LENGTH,
};
use crate::json::{JsonError, JsonReader, JsonStr, Token, integer};
use crate::url::{Origin, Url, UrlError};

/// Room for the addresses of all linked services' descriptors together.
const SERVICE_ADDRESS_BYTES: usize = 4096;
/// Aliases a descriptor may declare (`docs/web-native-concept.md`
/// section 5 item 4).
pub const MAX_ALIASES: usize = 16;
/// Room for every alias's target together.
const ALIAS_TARGET_BYTES: usize = 4096;
/// The shortest refresh interval a descriptor may ask for, in seconds:
/// more often would be a load the browser need not place on a server.
pub const MIN_REFRESH_SECONDS: u32 = 5;
/// The longest refresh interval, a day.
pub const MAX_REFRESH_SECONDS: u32 = 24 * 60 * 60;
/// Characters of `{sub}`: four hexadecimal digits.
const SUBCODE_DIGITS: u32 = 4;

/// The members of the descriptor, for diagnostics.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Member {
    /// The whole document.
    Document,
    /// `name`
    Name,
    /// `initial_page`
    InitialPage,
    /// `page_template`
    PageTemplate,
    /// `push`
    Push,
    /// `push.address`
    PushAddress,
    /// `push.method`
    PushMethod,
    /// `form_origins`
    FormOrigins,
    /// `region`
    Region,
    /// `services`
    Services,
    /// `services[].name`
    ServiceName,
    /// `services[].display_name`
    ServiceDisplayName,
    /// `services[].descriptor`
    ServiceDescriptor,
    /// `aliases`
    Aliases,
    /// `refresh`
    Refresh,
}

/// Why a descriptor was rejected, with the member concerned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DescriptorError {
    /// Not valid JSON.
    Json(JsonError),
    /// A member has the wrong type or form.
    Invalid(Member),
    /// A required member is missing.
    Missing(Member),
    /// A member appears twice.
    Duplicate(Member),
    /// A member this version does not know.
    Unknown,
    /// The template has an unknown placeholder or stray brace.
    Placeholder,
    /// An address failed the strict parser.
    Address(Member, UrlError),
    /// A plain `http` or `ws` address in a descriptor fetched over HTTPS.
    Downgrade(Member),
    /// More entries than this build keeps.
    TooMany(Member),
}

impl From<JsonError> for DescriptorError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

/// How the push connection is made (REQ-WEB-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PushMethod {
    /// RFC 6455 WebSocket.
    WebSocket,
    /// Server-sent events.
    ServerSentEvents,
}

/// Another service that links may lead to (Appendix E).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkedService {
    /// The short name used in `LN` targets.
    pub short_name: TextBuf<MAX_SHORT_NAME>,
    /// The name shown to the viewer.
    pub display_name: TextBuf<MAX_SERVICE_NAME>,
    address_start: u16,
    address_end: u16,
}

/// A parsed, checked service descriptor.
#[derive(Clone, Debug)]
pub struct Descriptor {
    /// The service name.
    pub name: TextBuf<MAX_SERVICE_NAME>,
    /// The initial page.
    pub initial_page: PageAddress,
    template: TextBuf<MAX_URL_LENGTH>,
    base: Url,
    push: Option<(Url, PushMethod)>,
    form_origins: [Option<Origin>; MAX_FORM_ORIGINS],
    /// The default character set region (decision E.4), 0 to 15.
    pub region: Option<u8>,
    services: [Option<LinkedService>; MAX_LINKED_SERVICES],
    service_addresses: TextBuf<SERVICE_ADDRESS_BYTES>,
    aliases: [Option<Alias>; MAX_ALIASES],
    alias_targets: TextBuf<ALIAS_TARGET_BYTES>,
    /// How often, in seconds, a page on screen is asked for again when the
    /// service has no push (concept decision 13).
    pub refresh_seconds: Option<u32>,
}

/// A page number that leads to another site on every page of this one
/// (concept section 5 item 4): to a named service's page, `name:mpp`, or
/// to an address, resolved against the descriptor's own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Alias {
    /// The page number.
    pub page: PageAddress,
    target_start: u16,
    target_end: u16,
}

/// Decodes a JSON string into a fixed buffer, as UTF-8 text.
fn text<const N: usize>(value: JsonStr<'_>, member: Member) -> Result<TextBuf<N>, DescriptorError> {
    let mut buffer = [0u8; N];
    let len = value.decode_into(&mut buffer).map_err(|_| DescriptorError::Invalid(member))?;
    let decoded = buffer.get(..len).unwrap_or(&[]);
    if decoded.is_empty() || decoded.iter().any(|&b| b < 0x20 || b == 0x7F) {
        return Err(DescriptorError::Invalid(member));
    }
    TextBuf::from_bytes(decoded).ok_or(DescriptorError::Invalid(member))
}

fn expect_string(token: Option<Token<'_>>, member: Member) -> Result<JsonStr<'_>, DescriptorError> {
    match token {
        Some(Token::Str(value)) => Ok(value),
        _ => Err(DescriptorError::Invalid(member)),
    }
}

/// Parses `mpp` written as three hexadecimal digits, magazine 1 to 8.
fn parse_mpp(text: &[u8]) -> Option<PageAddress> {
    if text.len() != 3 {
        return None;
    }
    let value = text.iter().try_fold(0u16, |value, &d| Some((value << 4) | u16::from(crate::bytes::hex_value(d)?)))?;
    PageAddress::from_mpp(value)
}

/// Checks a template's placeholders: only `{m}`, `{pp}`, `{mpp}` and
/// `{sub}`, no other brace, and none before the path. Placeholders become
/// hexadecimal digits, which are valid anywhere in a path or query, so one
/// expansion that parses proves they all do; in the host they would not
/// (`1.2.3.{pp}` is an address for some pages and a name for others).
fn check_template(template: &[u8]) -> Result<(), DescriptorError> {
    let authority_start = match template.windows(3).position(|w| w == b"://") {
        Some(index) => Some(index.saturating_add(3)),
        None => template.starts_with(b"//").then_some(2),
    };
    if let Some(start) = authority_start {
        let after = template.get(start..).unwrap_or(&[]);
        let end = after.iter().position(|&b| b == b'/' || b == b'?').unwrap_or(after.len());
        if template.get(..start.saturating_add(end)).unwrap_or(&[]).contains(&b'{') {
            return Err(DescriptorError::Placeholder);
        }
    }
    let mut rest = template;
    while let Some(open) = rest.iter().position(|&b| b == b'{' || b == b'}') {
        if rest.get(open) == Some(&b'}') {
            return Err(DescriptorError::Placeholder);
        }
        let after = rest.get(open.saturating_add(1)..).unwrap_or(&[]);
        let close = after.iter().position(|&b| b == b'}').ok_or(DescriptorError::Placeholder)?;
        let name = after.get(..close).unwrap_or(&[]);
        if !matches!(name, b"m" | b"pp" | b"mpp" | b"sub") {
            return Err(DescriptorError::Placeholder);
        }
        rest = after.get(close.saturating_add(1)..).unwrap_or(&[]);
    }
    Ok(())
}

/// Expands a template for a page and optional sub-code (REQ-WEB-003). With
/// no sub-code, `{sub}` becomes `0000`.
pub fn expand_template(
    template: &[u8],
    page: PageAddress,
    subcode: Option<SubCode>,
    out: &mut Writer<'_>,
) -> Result<(), UrlError> {
    let digits = page.ascii();
    let too_long = |_| UrlError::TooLong;
    let mut rest = template;
    while let Some(open) = rest.iter().position(|&b| b == b'{') {
        out.put(rest.get(..open).unwrap_or(&[])).map_err(too_long)?;
        let after = rest.get(open.saturating_add(1)..).unwrap_or(&[]);
        let close = after.iter().position(|&b| b == b'}').ok_or(UrlError::Path)?;
        match after.get(..close).unwrap_or(&[]) {
            b"m" => out.put(digits.get(..1).unwrap_or(&[])),
            b"pp" => out.put(digits.get(1..).unwrap_or(&[])),
            b"mpp" => out.put(&digits),
            b"sub" => out.hex(u32::from(subcode.map_or(0, SubCode::value)), SUBCODE_DIGITS),
            _ => return Err(UrlError::Path),
        }
        .map_err(too_long)?;
        rest = after.get(close.saturating_add(1)..).unwrap_or(&[]);
    }
    out.put(rest).map_err(too_long)
}

/// Resolves an address member against the descriptor's address and applies
/// REQ-SEC-007.
fn address(base: &Url, value: &str, member: Member) -> Result<Url, DescriptorError> {
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
    /// Parses a descriptor fetched from `base` (its final address, after
    /// redirects).
    pub fn parse(json: &[u8], base: &Url) -> Result<Self, DescriptorError> {
        let mut reader = JsonReader::new(json)?;
        if reader.next_token()? != Some(Token::BeginObject) {
            return Err(DescriptorError::Invalid(Member::Document));
        }
        let mut descriptor = Self {
            name: TextBuf::new(),
            initial_page: PageAddress::from_mpp(neoteletext_core::consts::DEFAULT_INITIAL_PAGE)
                .ok_or(DescriptorError::Invalid(Member::InitialPage))?,
            template: TextBuf::new(),
            base: *base,
            push: None,
            form_origins: [None; MAX_FORM_ORIGINS],
            region: None,
            services: [None; MAX_LINKED_SERVICES],
            service_addresses: TextBuf::new(),
            aliases: [None; MAX_ALIASES],
            alias_targets: TextBuf::new(),
            refresh_seconds: None,
        };
        let mut seen = [false; 9];
        let [
            seen_name,
            seen_page,
            seen_template,
            seen_push,
            seen_origins,
            seen_region,
            seen_services,
            seen_aliases,
            seen_refresh,
        ] = &mut seen;
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
                first(seen_template, Member::PageTemplate)?;
                let value: TextBuf<MAX_URL_LENGTH> =
                    text(expect_string(reader.next_token()?, Member::PageTemplate)?, Member::PageTemplate)?;
                check_template(value.as_bytes())?;
                descriptor.template = value;
                // One expansion that parses proves every expansion does (see
                // `check_template`).
                let sample = descriptor
                    .page_url(descriptor.initial_page, None)
                    .map_err(|error| DescriptorError::Address(Member::PageTemplate, error))?;
                if base.is_secure() && !sample.is_secure() {
                    return Err(DescriptorError::Downgrade(Member::PageTemplate));
                }
            } else if key.equals(b"push") {
                first(seen_push, Member::Push)?;
                descriptor.push = Some(Self::parse_push(&mut reader, base)?);
            } else if key.equals(b"form_origins") {
                first(seen_origins, Member::FormOrigins)?;
                descriptor.parse_origins(&mut reader, base)?;
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

    fn parse_push(reader: &mut JsonReader<'_>, base: &Url) -> Result<(Url, PushMethod), DescriptorError> {
        if reader.next_token()? != Some(Token::BeginObject) {
            return Err(DescriptorError::Invalid(Member::Push));
        }
        let mut url = None;
        let mut method = None;
        while let Some(token) = reader.next_token()? {
            let key = match token {
                Token::Key(key) => key,
                Token::EndObject => break,
                _ => return Err(DescriptorError::Invalid(Member::Push)),
            };
            if key.equals(b"address") {
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
            } else {
                return Err(DescriptorError::Unknown);
            }
        }
        let url = url.ok_or(DescriptorError::Missing(Member::PushAddress))?;
        let method = method.ok_or(DescriptorError::Missing(Member::PushMethod))?;
        // The scheme must suit the method.
        if url.scheme().is_websocket() != (method == PushMethod::WebSocket) {
            return Err(DescriptorError::Invalid(Member::PushAddress));
        }
        Ok((url, method))
    }

    fn parse_origins(&mut self, reader: &mut JsonReader<'_>, base: &Url) -> Result<(), DescriptorError> {
        if reader.next_token()? != Some(Token::BeginArray) {
            return Err(DescriptorError::Invalid(Member::FormOrigins));
        }
        let mut count = 0usize;
        loop {
            let value = match reader.next_token()? {
                Some(Token::EndArray) => return Ok(()),
                Some(Token::Str(value)) => value,
                _ => return Err(DescriptorError::Invalid(Member::FormOrigins)),
            };
            let value: TextBuf<MAX_URL_LENGTH> = text(value, Member::FormOrigins)?;
            let url = address(base, value.as_str(), Member::FormOrigins)?;
            let slot = self.form_origins.get_mut(count).ok_or(DescriptorError::TooMany(Member::FormOrigins))?;
            *slot = Some(url.origin());
            count = count.saturating_add(1);
        }
    }

    /// `aliases`: an object of page numbers and their targets, each a
    /// named service's page (`name:mpp`) or an address.
    fn parse_aliases(&mut self, reader: &mut JsonReader<'_>, base: &Url) -> Result<(), DescriptorError> {
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

    /// The alias for a page number, if the descriptor declares one.
    #[must_use]
    pub fn alias(&self, page: PageAddress) -> Option<&Alias> {
        self.aliases.iter().flatten().find(|alias| alias.page == page)
    }

    /// Every alias.
    pub fn aliases(&self) -> impl Iterator<Item = &Alias> {
        self.aliases.iter().flatten()
    }

    /// An alias's target: `name:mpp`, or an absolute address.
    #[must_use]
    pub fn alias_target(&self, alias: &Alias) -> &str {
        self.alias_targets.as_str().get(usize::from(alias.target_start)..usize::from(alias.target_end)).unwrap_or("")
    }

    fn parse_services(&mut self, reader: &mut JsonReader<'_>, base: &Url) -> Result<(), DescriptorError> {
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
                } else {
                    return Err(DescriptorError::Unknown);
                }
            }
            let short_name = short_name.ok_or(DescriptorError::Missing(Member::ServiceName))?;
            let display_name = display_name.ok_or(DescriptorError::Missing(Member::ServiceDisplayName))?;
            let url = url.ok_or(DescriptorError::Missing(Member::ServiceDescriptor))?;
            if self.service(short_name.as_str()).is_some() {
                return Err(DescriptorError::Duplicate(Member::ServiceName));
            }
            let start = self.service_addresses.len();
            self.service_addresses
                .push(url.as_str().as_bytes())
                .map_err(|_| DescriptorError::TooMany(Member::Services))?;
            let end = self.service_addresses.len();
            let narrow = |v: usize| u16::try_from(v).map_err(|_| DescriptorError::TooMany(Member::Services));
            let slot = self.services.get_mut(count).ok_or(DescriptorError::TooMany(Member::Services))?;
            *slot = Some(LinkedService {
                short_name,
                display_name,
                address_start: narrow(start)?,
                address_end: narrow(end)?,
            });
            count = count.saturating_add(1);
        }
    }

    /// The descriptor's own address: the service's origin (REQ-WEB-015).
    #[must_use]
    pub const fn address(&self) -> &Url {
        &self.base
    }

    /// The page address template as written.
    #[must_use]
    pub fn template(&self) -> &str {
        self.template.as_str()
    }

    /// The address of a page (REQ-WEB-003): the template expanded and
    /// resolved against the descriptor's address.
    pub fn page_url(&self, page: PageAddress, subcode: Option<SubCode>) -> Result<Url, UrlError> {
        let mut buffer = [0u8; MAX_URL_LENGTH];
        let mut writer = Writer::new(&mut buffer);
        expand_template(self.template.as_bytes(), page, subcode, &mut writer)?;
        let expanded = core::str::from_utf8(writer.written()).map_err(|_| UrlError::BadCharacter)?;
        self.base.join(expanded)
    }

    /// The push address and method, if the service pushes.
    #[must_use]
    pub const fn push(&self) -> Option<&(Url, PushMethod)> {
        self.push.as_ref()
    }

    /// The other origins forms may submit to (REQ-WEB-008).
    pub fn form_origins(&self) -> impl Iterator<Item = &Origin> {
        self.form_origins.iter().flatten()
    }

    /// A linked service by its short name (Appendix E.2 rule 2: an unknown
    /// name makes the link null).
    #[must_use]
    pub fn service(&self, short_name: &str) -> Option<&LinkedService> {
        self.services.iter().flatten().find(|service| service.short_name.as_str() == short_name)
    }

    /// The descriptor address of a linked service.
    #[must_use]
    pub fn service_address(&self, service: &LinkedService) -> &str {
        self.service_addresses
            .as_str()
            .get(usize::from(service.address_start)..usize::from(service.address_end))
            .unwrap_or("")
    }

    /// Every linked service.
    pub fn services(&self) -> impl Iterator<Item = &LinkedService> {
        self.services.iter().flatten()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;
    use std::string::ToString;

    const VALID: &str = r#"{
        "name": "Example Teletext",
        "initial_page": "100",
        "page_template": "/ttx/pages/{mpp}/{sub}?m={m}&p={pp}",
        "push": { "address": "wss://example.org/ttx/push", "method": "websocket" },
        "form_origins": ["https://forms.example.org", "https://example.org:8443"],
        "region": 2,
        "services": [
            { "name": "weather", "display_name": "Weather é", "descriptor": "https://weather.example/ttx.json" },
            { "name": "news-2", "display_name": "News", "descriptor": "/other/ttx.json" }
        ]
    }"#;

    fn base() -> Url {
        Url::parse("https://example.org/ttx/service.json").unwrap()
    }

    #[test]
    fn valid_descriptor() {
        let descriptor = Descriptor::parse(VALID.as_bytes(), &base()).unwrap();
        assert_eq!(descriptor.name.as_str(), "Example Teletext");
        assert_eq!(descriptor.initial_page.mpp(), 0x100);
        assert_eq!(descriptor.region, Some(2));
        let page = PageAddress::from_mpp(0x8A5).unwrap();
        assert_eq!(
            descriptor.page_url(page, Some(SubCode::new(0x12))).unwrap().as_str(),
            "https://example.org/ttx/pages/8A5/0012?m=8&p=A5"
        );
        assert_eq!(
            descriptor.page_url(page, None).unwrap().as_str(),
            "https://example.org/ttx/pages/8A5/0000?m=8&p=A5"
        );
        let (push, method) = descriptor.push().unwrap();
        assert_eq!((push.as_str(), *method), ("wss://example.org/ttx/push", PushMethod::WebSocket));
        assert_eq!(descriptor.form_origins().count(), 2);
        let weather = descriptor.service("weather").unwrap();
        assert_eq!(weather.display_name.as_str(), "Weather \u{e9}");
        assert_eq!(descriptor.service_address(weather), "https://weather.example/ttx.json");
        let news = descriptor.service("news-2").unwrap();
        assert_eq!(descriptor.service_address(news), "https://example.org/other/ttx.json");
        assert!(descriptor.service("sport").is_none());
    }

    #[test]
    fn minimal_descriptor() {
        let json = br#"{"name":"S","initial_page":"1A0","page_template":"http://s.example/{mpp}"}"#;
        let base = Url::parse("http://s.example/d.json").unwrap();
        let descriptor = Descriptor::parse(json, &base).unwrap();
        assert!(descriptor.push().is_none());
        assert_eq!(descriptor.region, None);
    }

    #[test]
    fn malformed_descriptors() {
        let plain = Url::parse("http://s.example/d.json").unwrap();
        let cases: [(&str, DescriptorError); 20] = [
            ("[]", DescriptorError::Invalid(Member::Document)),
            (r#"{"initial_page":"100","page_template":"/{mpp}"}"#, DescriptorError::Missing(Member::Name)),
            (r#"{"name":"S","page_template":"/{mpp}"}"#, DescriptorError::Missing(Member::InitialPage)),
            (r#"{"name":"S","initial_page":"100"}"#, DescriptorError::Missing(Member::PageTemplate)),
            (
                r#"{"name":"S","name":"T","initial_page":"100","page_template":"/{mpp}"}"#,
                DescriptorError::Duplicate(Member::Name),
            ),
            (r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","x":1}"#, DescriptorError::Unknown),
            (
                r#"{"name":"S","initial_page":"900","page_template":"/{mpp}"}"#,
                DescriptorError::Invalid(Member::InitialPage),
            ),
            (
                r#"{"name":"S","initial_page":100,"page_template":"/{mpp}"}"#,
                DescriptorError::Invalid(Member::InitialPage),
            ),
            (r#"{"name":"","initial_page":"100","page_template":"/{mpp}"}"#, DescriptorError::Invalid(Member::Name)),
            (r#"{"name":"S","initial_page":"100","page_template":"/{page}"}"#, DescriptorError::Placeholder),
            (r#"{"name":"S","initial_page":"100","page_template":"/{mpp"}"#, DescriptorError::Placeholder),
            (r#"{"name":"S","initial_page":"100","page_template":"/mpp}"}"#, DescriptorError::Placeholder),
            (
                r#"{"name":"S","initial_page":"100","page_template":"http://1.2.3.{pp}/x"}"#,
                DescriptorError::Placeholder,
            ),
            (r#"{"name":"S","initial_page":"100","page_template":"//p{m}.example/x"}"#, DescriptorError::Placeholder),
            (
                r#"{"name":"S","initial_page":"100","page_template":"ftp://s.example/{mpp}"}"#,
                DescriptorError::Address(Member::PageTemplate, UrlError::Scheme),
            ),
            (
                r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","region":16}"#,
                DescriptorError::Invalid(Member::Region),
            ),
            (
                r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","push":{"address":"/p","method":"websocket"}}"#,
                DescriptorError::Invalid(Member::PushAddress),
            ),
            (
                r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","push":{"address":"ws://s.example/p","method":"poll"}}"#,
                DescriptorError::Invalid(Member::PushMethod),
            ),
            (
                r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","services":[{"name":"a b","display_name":"A","descriptor":"/a"}]}"#,
                DescriptorError::Invalid(Member::ServiceName),
            ),
            (
                r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","services":[{"name":"a","descriptor":"/a"}]}"#,
                DescriptorError::Missing(Member::ServiceDisplayName),
            ),
        ];
        for (json, error) in cases {
            assert_eq!(Descriptor::parse(json.as_bytes(), &plain).map(|d| d.initial_page), Err(error), "{json}");
        }
        assert!(matches!(Descriptor::parse(b"{\"name\":", &plain), Err(DescriptorError::Json(_))));
    }

    #[test]
    fn https_descriptor_cannot_name_plain_addresses() {
        let cases: [(&str, Member); 4] = [
            (r#"{"name":"S","initial_page":"100","page_template":"http://example.org/{mpp}"}"#, Member::PageTemplate),
            (
                r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","push":{"address":"ws://example.org/p","method":"websocket"}}"#,
                Member::PushAddress,
            ),
            (
                r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","form_origins":["http://forms.example"]}"#,
                Member::FormOrigins,
            ),
            (
                r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","services":[{"name":"a","display_name":"A","descriptor":"http://a.example/"}]}"#,
                Member::ServiceDescriptor,
            ),
        ];
        for (json, member) in cases {
            assert_eq!(
                Descriptor::parse(json.as_bytes(), &base()).map(|d| d.initial_page),
                Err(DescriptorError::Downgrade(member)),
                "{json}"
            );
        }
    }

    #[test]
    fn too_many_entries() {
        let mut json =
            std::string::String::from(r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","form_origins":["#);
        for i in 0..=MAX_FORM_ORIGINS {
            if i > 0 {
                json.push(',');
            }
            json.push_str("\"https://f");
            json.push_str(&i.to_string());
            json.push_str(".example\"");
        }
        json.push_str("]}");
        assert_eq!(
            Descriptor::parse(json.as_bytes(), &base()).map(|d| d.initial_page),
            Err(DescriptorError::TooMany(Member::FormOrigins))
        );
    }

    /// `docs/web-native-concept.md` section 5 item 4: aliases lead a page
    /// number to a named service's page or to an address.
    #[test]
    fn aliases() {
        let json = r#"{"name":"A","initial_page":"100","page_template":"/{mpp}.t42",
            "aliases":{"300":"weather:300","400":"https://sport.example/400.t42"}}"#;
        let descriptor = Descriptor::parse(json.as_bytes(), &base()).unwrap();
        let target = |mpp| descriptor.alias(PageAddress::from_mpp(mpp).unwrap()).map(|a| descriptor.alias_target(a));
        assert_eq!(target(0x300), Some("weather:300"));
        assert_eq!(target(0x400), Some("https://sport.example/400.t42"));
        assert_eq!(target(0x500), None);
    }

    #[test]
    fn refresh_interval() {
        let head = r#"{"name":"A","initial_page":"100","page_template":"/{mpp}.t42","refresh":"#;
        let parse = |value: &str| Descriptor::parse(std::format!("{head}{value}}}").as_bytes(), &base());
        assert_eq!(parse("60").unwrap().refresh_seconds, Some(60));
        for bad in ["1", "100000", "\"60\"", "-5", "6.5"] {
            assert_eq!(parse(bad).map(|_| ()), Err(DescriptorError::Invalid(Member::Refresh)), "{bad}");
        }
    }

    #[test]
    fn bad_aliases_reject_the_descriptor() {
        let head = r#"{"name":"A","initial_page":"100","page_template":"/{mpp}.t42","aliases":"#;
        for (aliases, error) in [
            (r#"{"30":"w:300"}"#, DescriptorError::Invalid(Member::Aliases)),
            (r#"{"300":"w:3000"}"#, DescriptorError::Invalid(Member::Aliases)),
            (r#"{"300":"w w:300"}"#, DescriptorError::Invalid(Member::Aliases)),
            (r#"{"300":"http://plain.example/300.t42"}"#, DescriptorError::Downgrade(Member::Aliases)),
            (r#"{"300":"wss://x.example/300.t42"}"#, DescriptorError::Invalid(Member::Aliases)),
            (r#"{"300":"w:300","300":"w:301"}"#, DescriptorError::Duplicate(Member::Aliases)),
            (r#"["300"]"#, DescriptorError::Invalid(Member::Aliases)),
            // Without `://` a target is `name:mpp`; a path alone is neither.
            (r#"{"300":"/local/300.t42"}"#, DescriptorError::Invalid(Member::Aliases)),
        ] {
            let json = std::format!("{head}{aliases}}}");
            assert_eq!(Descriptor::parse(json.as_bytes(), &base()).map(|_| ()), Err(error), "{aliases}");
        }
        let many: std::vec::Vec<std::string::String> =
            (0..=MAX_ALIASES).map(|n| std::format!(r#""{:03X}":"w:100""#, 0x100 + n)).collect();
        let json = std::format!("{head}{{{}}}}}", many.join(","));
        assert_eq!(
            Descriptor::parse(json.as_bytes(), &base()).map(|_| ()),
            Err(DescriptorError::TooMany(Member::Aliases))
        );
    }
}
