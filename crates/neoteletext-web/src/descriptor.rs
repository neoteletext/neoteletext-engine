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
//!
//! A channel's `channel.json` (REQ-WEB-024, decision E.70) is read by
//! [`Descriptor::parse_channel`], with three differences: every member is
//! optional, `page_template` is refused because a channel's files have
//! fixed names, and a linked service may be a channel, given by its
//! folder's address in a member `channel`. Its push address must be on the
//! channel's own origin.

mod links;
mod parse;
mod template;

pub use template::expand_template;

use neoteletext_core::types::PageAddress;

use crate::bytes::TextBuf;
use crate::consts::{MAX_FORM_ORIGINS, MAX_LINKED_SERVICES, MAX_SERVICE_NAME, MAX_SHORT_NAME, MAX_URL_LENGTH};
use crate::json::JsonError;
use crate::url::{Origin, Url, UrlError};

/// Room for the addresses of all linked services' descriptors together.
const SERVICE_ADDRESS_BYTES: usize = 4096;
/// Aliases a descriptor may declare (the web-native concept
/// section 5 item 4).
pub const MAX_ALIASES: usize = 16;
/// Room for every alias's target together.
const ALIAS_TARGET_BYTES: usize = 4096;
/// The shortest refresh interval a descriptor may ask for, in seconds:
/// more often would be a load a receiver need not place on a server.
pub const MIN_REFRESH_SECONDS: u32 = 5;
/// The longest refresh interval, a day.
pub const MAX_REFRESH_SECONDS: u32 = 24 * 60 * 60;
/// The initial page when none is given (REQ-SVC-004).
const DEFAULT_PAGE: PageAddress = match PageAddress::from_mpp(neoteletext_core::consts::DEFAULT_INITIAL_PAGE) {
    Some(page) => page,
    // Page 100 is always valid; this arm only keeps the constant total.
    None => PageAddress {
        magazine: neoteletext_core::types::Magazine::from_bits(1),
        page: neoteletext_core::types::PageNumber(0),
    },
};

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
    /// `push.input`
    PushInput,
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
    /// `services[].channel`
    ServiceChannel,
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
    /// A member a channel's `channel.json` may not have (REQ-WEB-024).
    NotForChannels(Member),
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
    /// The address is a channel's folder (REQ-WEB-024); otherwise it is a
    /// descriptor's.
    pub channel: bool,
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
    /// The channel takes the viewer's keys on its push WebSocket (area
    /// INT, decision E.90).
    input: bool,
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
    /// This is a channel's `channel.json`, or a channel's defaults.
    channel: bool,
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

impl Descriptor {
    /// Parses a descriptor fetched from `base` (its final address, after
    /// redirects).
    pub fn parse(json: &[u8], base: &Url) -> Result<Self, DescriptorError> {
        Self::parse_with(json, base, false)
    }

    /// Parses a channel's `channel.json`, fetched from `base` (REQ-WEB-024).
    pub fn parse_channel(json: &[u8], base: &Url) -> Result<Self, DescriptorError> {
        Self::parse_with(json, base, true)
    }

    /// A channel's settings when it has no `channel.json` (REQ-WEB-018):
    /// initial page 100 and nothing else, named by its host and folder.
    /// `folder` is the channel's address, ending in `/`.
    #[must_use]
    pub fn channel_defaults(folder: &Url) -> Self {
        let mut name = TextBuf::new();
        let path = folder.path().trim_end_matches('/');
        for part in [folder.host_text(), path] {
            for &byte in part.as_bytes() {
                // A long name is cut: it is only a title.
                if name.push(&[byte]).is_err() {
                    break;
                }
            }
        }
        Self::empty(folder, true, name)
    }

    fn empty(base: &Url, channel: bool, name: TextBuf<MAX_SERVICE_NAME>) -> Self {
        Self {
            name,
            initial_page: DEFAULT_PAGE,
            template: TextBuf::new(),
            base: *base,
            push: None,
            input: false,
            form_origins: [None; MAX_FORM_ORIGINS],
            region: None,
            services: [None; MAX_LINKED_SERVICES],
            service_addresses: TextBuf::new(),
            aliases: [None; MAX_ALIASES],
            alias_targets: TextBuf::new(),
            refresh_seconds: None,
            channel,
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

    /// The descriptor's own address: the service's origin (REQ-WEB-015).
    #[must_use]
    pub const fn address(&self) -> &Url {
        &self.base
    }

    /// This is a channel's settings (REQ-WEB-024).
    #[must_use]
    pub const fn is_channel(&self) -> bool {
        self.channel
    }

    /// The page address template as written.
    #[must_use]
    pub fn template(&self) -> &str {
        self.template.as_str()
    }

    /// The push address and method, if the service pushes.
    #[must_use]
    pub const fn push(&self) -> Option<&(Url, PushMethod)> {
        self.push.as_ref()
    }

    /// Whether the channel takes the viewer's keys back on its push
    /// WebSocket (`push.input`, area INT): never without a WebSocket.
    #[must_use]
    pub const fn input(&self) -> bool {
        self.input
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
mod tests;
