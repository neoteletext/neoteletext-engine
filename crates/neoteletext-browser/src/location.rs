//! Locations and what the address bar accepts (`docs/desktop-browser.md`
//! section 7, `docs/web-native-concept.md` sections 3 and 4).
//!
//! A location is where a tab's pages come from: a web-native site, a
//! service named by its descriptor, a local file or a live feed. Its
//! **site key** decides when the tuning screen is shown: the browser treats
//! every origin (scheme, host and port) as its own site, which is stricter
//! than the registrable domain and needs no public suffix list (decision
//! recorded in `docs/macos-build-decisions.md`).

use neoteletext_core::types::PageAddress;
use neoteletext_web::url::{Scheme, Url, UrlError};

/// The longest live feed name accepted.
const MAX_FEED_NAME: usize = 32;
/// The longest file path accepted.
const MAX_PATH: usize = 4096;
/// The prefix of a live feed's address.
pub const LIVE_PREFIX: &str = "zxnet:";
/// The prefix of a file's address.
pub const FILE_PREFIX: &str = "file://";
/// The prefix the fetcher reads as "a web-native site"; the same as
/// `neoteletext_app::fetch::SITE_PREFIX`.
pub const FETCHER_SITE_PREFIX: &str = "site:";
/// The extensions of a page's own file (`docs/web-native-concept.md`
/// section 2 item 7).
const PAGE_FILE_EXTENSIONS: [&str; 2] = [".t42", ".tti"];
/// The extension of a service descriptor.
const DESCRIPTOR_EXTENSION: &str = ".json";
/// Where a web-native site keeps its descriptor
/// (`docs/web-native-concept.md` section 2 item 2): that address names the
/// site itself, which opens with its defaults if the descriptor is gone.
const WELL_KNOWN_DESCRIPTOR: &str = "/.well-known/teletext.json";
/// Digits in a page number.
const PAGE_DIGITS: usize = 3;

/// Where a tab's pages come from.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum Location {
    /// A web-native site, by its origin: `https://example.org`, with the
    /// port when it is not the scheme's own.
    Site {
        /// The origin, without a trailing slash.
        origin: String,
        /// The origin is `https`.
        secure: bool,
    },
    /// A service named by the address of its descriptor (REQ-WEB-002).
    Service {
        /// The descriptor's address.
        descriptor: String,
        /// Its origin, which is the service's site.
        origin: String,
        /// The descriptor is fetched over `https`.
        secure: bool,
    },
    /// A local file of pages: TTI, or T42 packets.
    File {
        /// Its path.
        path: String,
    },
    /// A zxnet live feed (REQ-INP-022), by its short name.
    Live {
        /// The feed's short name.
        feed: String,
    },
}

/// A location and, optionally, the page to show there first.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Destination {
    /// Where.
    pub location: Location,
    /// The page, `0xMPP`; the location's initial page when absent.
    pub page: Option<u16>,
}

/// Why the address bar's text was not accepted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AddressError {
    /// Nothing was typed.
    Empty,
    /// The address failed the strict parser (REQ-WEB-016).
    Address(UrlError),
    /// Only `http` and `https` lead to sites.
    Scheme,
    /// The page number is not three digits of a real page.
    Page,
    /// A live feed name must be letters, digits and hyphens.
    Feed,
    /// A file path that is too long or holds control characters.
    Path,
}

/// A page number as typed: three digits, magazine 1 to 8.
#[must_use]
pub fn page_number(text: &str) -> Option<u16> {
    if text.len() != PAGE_DIGITS || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mpp = u16::from_str_radix(text, 16).ok()?;
    PageAddress::from_mpp(mpp).map(PageAddress::mpp)
}

/// The origin of a parsed address, as text: the scheme, the host, and the
/// port when it is not the scheme's own.
fn origin_text(url: &Url) -> String {
    let scheme = if url.is_secure() { "https" } else { "http" };
    let default = if url.is_secure() { Scheme::Https.default_port() } else { Scheme::Http.default_port() };
    let host = url.host_text();
    // An IPv6 host is written in brackets in an address.
    let host = if host.contains(':') { format!("[{host}]") } else { host.to_string() };
    if url.port() == default { format!("{scheme}://{host}") } else { format!("{scheme}://{host}:{}", url.port()) }
}

/// The page a page file's path names: `/140.t42` is page 140.
fn page_of_path(path: &str) -> Option<u16> {
    let name = path.strip_prefix('/')?;
    PAGE_FILE_EXTENSIONS.iter().find_map(|extension| name.strip_suffix(extension)).and_then(page_number)
}

/// Reads what was typed in the address bar.
///
/// - `https://example.org/` or `example.org`: a site, at its initial page;
///   an address without a scheme is `https`.
/// - `https://example.org/140.t42`: that page of that site.
/// - `example.org 140`: the same, the convenience of section 4.
/// - `https://example.org/service.json`: the service that descriptor names.
/// - `zxnet:teefax`: a live feed. `file:///path` or `/path`: a file.
pub fn parse_input(text: &str) -> Result<Destination, AddressError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AddressError::Empty);
    }
    if let Some(feed) = text.strip_prefix(LIVE_PREFIX) {
        let valid = !feed.is_empty()
            && feed.len() <= MAX_FEED_NAME
            && feed.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
        return if valid {
            Ok(Destination { location: Location::Live { feed: feed.to_ascii_lowercase() }, page: None })
        } else {
            Err(AddressError::Feed)
        };
    }
    if let Some(path) = text.strip_prefix(FILE_PREFIX).or_else(|| text.starts_with('/').then_some(text)) {
        if path.is_empty() || path.len() > MAX_PATH || path.chars().any(char::is_control) {
            return Err(AddressError::Path);
        }
        return Ok(Destination { location: Location::File { path: path.to_string() }, page: None });
    }
    let (address, typed_page) = match text.rsplit_once(char::is_whitespace) {
        Some((address, page)) => (address.trim_end(), Some(page_number(page).ok_or(AddressError::Page)?)),
        None => (text, None),
    };
    let full = if address.contains("://") { address.to_string() } else { format!("https://{address}") };
    let url = Url::parse(&full).map_err(AddressError::Address)?;
    if !matches!(url.scheme(), Scheme::Http | Scheme::Https) {
        return Err(AddressError::Scheme);
    }
    let origin = origin_text(&url);
    let secure = url.is_secure();
    let location = if url.path().ends_with(DESCRIPTOR_EXTENSION) && url.path() != WELL_KNOWN_DESCRIPTOR {
        Location::Service { descriptor: url.as_str().to_string(), origin, secure }
    } else {
        Location::Site { origin, secure }
    };
    let page = typed_page.or_else(|| page_of_path(url.path()));
    Ok(Destination { location, page })
}

impl Location {
    /// The identity that decides whether two locations are the same site.
    #[must_use]
    pub fn site_key(&self) -> String {
        match self {
            Self::Site { origin, .. } | Self::Service { origin, .. } => origin.clone(),
            Self::File { path } => format!("{FILE_PREFIX}{path}"),
            Self::Live { feed } => format!("{LIVE_PREFIX}{feed}"),
        }
    }

    /// The address the fetcher process is started with, or `None` for a
    /// file, which needs no fetcher.
    #[must_use]
    pub fn fetcher_address(&self) -> Option<String> {
        match self {
            Self::Site { origin, .. } => Some(format!("{FETCHER_SITE_PREFIX}{origin}/")),
            Self::Service { descriptor, .. } => Some(descriptor.clone()),
            Self::Live { feed } => Some(format!("{LIVE_PREFIX}{feed}")),
            Self::File { .. } => None,
        }
    }

    /// The short name for tabs and row 0: the host (with a port that is
    /// not the scheme's own), the file's name, or the feed.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Site { origin, .. } | Self::Service { origin, .. } => {
                origin.split_once("://").map_or(origin.as_str(), |(_, host)| host).to_string()
            }
            Self::File { path } => path.rsplit('/').next().unwrap_or(path).to_string(),
            Self::Live { feed } => feed.clone(),
        }
    }

    /// The connection is encrypted, or there is none: files are local and
    /// zxnet feeds are `wss`.
    #[must_use]
    pub const fn is_secure(&self) -> bool {
        match self {
            Self::Site { secure, .. } | Self::Service { secure, .. } => *secure,
            Self::File { .. } | Self::Live { .. } => true,
        }
    }

    /// The address bar's text for a page here: a site's page as the
    /// address of its file (`docs/web-native-concept.md` section 3), which
    /// typed back leads to the same page.
    #[must_use]
    pub fn address(&self, page: Option<u16>) -> String {
        match (self, page) {
            (Self::Site { origin, .. }, Some(page)) => format!("{origin}/{page:03X}.t42"),
            (Self::Site { origin, .. }, None) => format!("{origin}/"),
            (Self::Service { descriptor, .. }, Some(page)) => format!("{descriptor} {page:03X}"),
            (Self::Service { descriptor, .. }, None) => descriptor.clone(),
            (Self::File { path }, _) => format!("{FILE_PREFIX}{path}"),
            (Self::Live { feed }, _) => format!("{LIVE_PREFIX}{feed}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(origin: &str, secure: bool) -> Location {
        Location::Site { origin: origin.to_string(), secure }
    }

    #[test]
    fn addresses_name_sites_and_pages() {
        let cases = [
            ("https://example.org/", site("https://example.org", true), None),
            ("example.org", site("https://example.org", true), None),
            ("  example.org 140 ", site("https://example.org", true), Some(0x140)),
            ("http://127.0.0.1:8001/300.t42", site("http://127.0.0.1:8001", false), Some(0x300)),
            ("https://Example.ORG:443/1A0.tti", site("https://example.org", true), Some(0x1A0)),
            ("https://example.org/other/path", site("https://example.org", true), None),
            ("http://[::1]:81/", site("http://[::1]:81", false), None),
        ];
        for (typed, location, page) in cases {
            assert_eq!(parse_input(typed), Ok(Destination { location, page }), "{typed}");
        }
    }

    #[test]
    fn descriptors_feeds_and_files() {
        let service = parse_input("http://127.0.0.1:9/service.json 200").unwrap();
        assert_eq!(
            service.location,
            Location::Service {
                descriptor: "http://127.0.0.1:9/service.json".into(),
                origin: "http://127.0.0.1:9".into(),
                secure: false
            }
        );
        assert_eq!(service.page, Some(0x200));
        assert_eq!(
            parse_input("http://127.0.0.1:9/.well-known/teletext.json").unwrap().location,
            Location::Site { origin: "http://127.0.0.1:9".into(), secure: false },
            "the well-known descriptor names the site"
        );
        assert_eq!(parse_input("zxnet:Teefax").unwrap().location, Location::Live { feed: "teefax".into() });
        assert_eq!(parse_input("/tmp/a.t42").unwrap().location, Location::File { path: "/tmp/a.t42".into() });
        assert_eq!(parse_input("file:///tmp/a.tti").unwrap().location, Location::File { path: "/tmp/a.tti".into() });
    }

    #[test]
    fn bad_input_is_refused() {
        assert_eq!(parse_input("   "), Err(AddressError::Empty));
        assert_eq!(parse_input("example.org 999"), Err(AddressError::Page), "magazine 9 does not exist");
        assert_eq!(parse_input("example.org 14"), Err(AddressError::Page));
        assert_eq!(parse_input("example.org 0FF"), Err(AddressError::Page), "magazine 0 is not typed");
        assert_eq!(parse_input("wss://zxnet.co.uk/ws"), Err(AddressError::Scheme));
        assert_eq!(parse_input("zxnet:a b"), Err(AddressError::Feed));
        assert_eq!(parse_input("zxnet:"), Err(AddressError::Feed));
        assert_eq!(parse_input("/tmp/a\nb"), Err(AddressError::Path));
        assert!(matches!(parse_input("https://user@example.org/"), Err(AddressError::Address(UrlError::UserInfo))));
        assert!(matches!(parse_input("javascript:alert(1)"), Err(AddressError::Address(_))));
        assert!(matches!(parse_input("https://bücher.example/"), Err(AddressError::Address(UrlError::BadCharacter))));
    }

    #[test]
    fn sites_are_origins() {
        let a = parse_input("https://example.org/100.t42").unwrap().location;
        let b = parse_input("example.org 200").unwrap().location;
        let other_port = parse_input("https://example.org:8443/").unwrap().location;
        let plain = parse_input("http://example.org/").unwrap().location;
        let sub = parse_input("https://news.example.org/").unwrap().location;
        assert_eq!(a.site_key(), b.site_key());
        for other in [other_port, plain, sub] {
            assert_ne!(a.site_key(), other.site_key(), "{other:?}");
        }
    }

    #[test]
    fn locations_round_trip_through_the_address_bar() {
        for typed in ["https://example.org/140.t42", "http://127.0.0.1:8001/", "zxnet:teefax", "file:///tmp/x.tti"] {
            let destination = parse_input(typed).unwrap();
            let shown = destination.location.address(destination.page);
            assert_eq!(parse_input(&shown).unwrap(), destination, "{typed} shown as {shown}");
        }
        let site = site("http://127.0.0.1:8001", false);
        assert_eq!(site.fetcher_address().as_deref(), Some("site:http://127.0.0.1:8001/"));
        assert_eq!(site.label(), "127.0.0.1:8001");
        assert!(!site.is_secure());
    }
}
