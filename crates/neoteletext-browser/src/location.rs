//! Locations and what the address bar accepts (REQ-BRW-002, REQ-WEB-018,
//! REQ-WEB-023, `docs/desktop-browser.md` section 7).
//!
//! A location is where a tab's pages come from: a channel, a service named
//! by its descriptor, a local file or a live feed. Its **channel key**
//! decides when the tuning screen is shown: a channel is a folder, and is
//! identified by its origin (scheme, host and port) and its folder path
//! (decision E.70). A service opened by its descriptor is identified by
//! its origin and the folder that holds the descriptor.

use neoteletext_core::types::PageAddress;
use neoteletext_web::channel::{self, ChannelError, ChannelFile};
use neoteletext_web::url::{Scheme, Url, UrlError};

/// The longest live feed name accepted.
const MAX_FEED_NAME: usize = 32;
/// The longest file path accepted.
const MAX_PATH: usize = 4096;
/// The prefix of a live feed's address.
pub const LIVE_PREFIX: &str = "zxnet:";
/// The prefix of a file's address.
pub const FILE_PREFIX: &str = "file://";
/// The prefix the fetcher reads as "a channel"; the same as
/// `neoteletext_app::fetch::CHANNEL_PREFIX`.
pub const FETCHER_CHANNEL_PREFIX: &str = "channel:";
/// The extension of a service descriptor.
const DESCRIPTOR_EXTENSION: &str = ".json";
/// Digits in a page number.
const PAGE_DIGITS: usize = 3;
/// What separates a scheme from the rest of an address.
const SCHEME_SEPARATOR: &str = "://";

/// Where a tab's pages come from.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum Location {
    /// A channel, by its folder: `https://example.org/` or
    /// `http://127.0.0.1:8001/one/`, with the port only when it is not the
    /// scheme's own (REQ-WEB-018).
    Channel {
        /// The folder's address, ending in `/`.
        folder: String,
        /// The channel is reached over `https`.
        secure: bool,
    },
    /// A service named by the address of its descriptor (REQ-WEB-002).
    Service {
        /// The descriptor's address.
        descriptor: String,
        /// The folder holding the descriptor: the service's channel.
        folder: String,
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
    /// The address breaks a channel's rules (REQ-WEB-023).
    Channel(ChannelError),
    /// Only `http` and `https` lead to channels.
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

/// The path of an address as typed, from the first `/` after the
/// authority; empty when there is none.
fn written_path(full: &str) -> &str {
    let rest = full.split_once(SCHEME_SEPARATOR).map_or(full, |(_, rest)| rest);
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    rest.find('/').and_then(|slash| rest.get(slash..)).unwrap_or("")
}

/// The page a channel's page file holds, and whether the file is T42:
/// `http://h.example/one/140.t42` is page 140 in T42.
#[must_use]
pub fn page_file(address: &str) -> Option<(u16, bool)> {
    match channel::parse_address(address).ok()?.file? {
        ChannelFile::Page { page, t42 } => Some((page.mpp(), t42)),
        _ => None,
    }
}

/// True when the address bar's text asks for a channel's contents view
/// (REQ-BRW-009): the address of its `contents.txt`.
#[must_use]
pub fn wants_contents(text: &str) -> bool {
    written_path(text.trim()).rsplit('/').next() == Some(channel::CONTENTS_FILE)
}

/// Reads what was typed in the address bar (REQ-BRW-002).
///
/// - `https://example.org/`, `example.org`, `example.org/ceefax/`: a
///   channel, at its initial page; an address without a scheme is `https`.
/// - `https://example.org/ceefax`: the folder `ceefax/`, when the last
///   segment has no `.`.
/// - `https://example.org/ceefax/140.tti` or `140.t42`: that page of that
///   channel; `index.t42`, `channel.json` and `contents.txt` name the
///   channel.
/// - `example.org/ceefax 140`: the same page, the convenience of section 4.
/// - `https://example.org/path/service.json`: the service that descriptor
///   names, for any name but `channel.json`.
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
    let full = if address.contains(SCHEME_SEPARATOR) { address.to_string() } else { format!("https://{address}") };
    let url = Url::parse(&full).map_err(AddressError::Address)?;
    if !matches!(url.scheme(), Scheme::Http | Scheme::Https) {
        return Err(AddressError::Scheme);
    }
    let secure = url.is_secure();
    let last = written_path(&full).rsplit('/').next().unwrap_or("");
    if last.ends_with(DESCRIPTOR_EXTENSION) && last != channel::SETTINGS_FILE {
        let directory = url.path().rfind('/').map_or("/", |slash| url.path().get(..=slash).unwrap_or("/"));
        let folder = format!("{}{directory}", origin_text(&url));
        return Ok(Destination {
            location: Location::Service { descriptor: url.as_str().to_string(), folder, secure },
            page: typed_page,
        });
    }
    // A last segment without a dot is a folder typed without its slash.
    let channel_text = if !last.is_empty() && !last.contains('.') { format!("{full}/") } else { full };
    let found = channel::parse_address(&channel_text).map_err(|error| match error {
        ChannelError::Address(error) => AddressError::Address(error),
        ChannelError::Scheme => AddressError::Scheme,
        other => AddressError::Channel(other),
    })?;
    let file_page = match found.file {
        Some(ChannelFile::Page { page, .. }) => Some(page.mpp()),
        _ => None,
    };
    let folder = format!("{}{}", origin_text(&found.folder), found.folder.path());
    Ok(Destination { location: Location::Channel { folder, secure }, page: typed_page.or(file_page) })
}

impl Location {
    /// The identity that decides whether two locations are the same
    /// channel: the origin and the folder (REQ-WEB-018).
    #[must_use]
    pub fn site_key(&self) -> String {
        match self {
            Self::Channel { folder, .. } | Self::Service { folder, .. } => folder.clone(),
            Self::File { path } => format!("{FILE_PREFIX}{path}"),
            Self::Live { feed } => format!("{LIVE_PREFIX}{feed}"),
        }
    }

    /// The address the fetcher process is started with, or `None` for a
    /// file, which needs no fetcher.
    #[must_use]
    pub fn fetcher_address(&self) -> Option<String> {
        match self {
            Self::Channel { folder, .. } => Some(format!("{FETCHER_CHANNEL_PREFIX}{folder}")),
            Self::Service { descriptor, .. } => Some(descriptor.clone()),
            Self::Live { feed } => Some(format!("{LIVE_PREFIX}{feed}")),
            Self::File { .. } => None,
        }
    }

    /// The host and the folder path without its slashes, for a channel or
    /// service: `("user.github.io", "ceefax")`, or an empty folder for the
    /// root. The host includes a port that is not the scheme's own.
    #[must_use]
    pub fn host_and_folder(&self) -> Option<(String, String)> {
        match self {
            Self::Channel { folder, .. } | Self::Service { folder, .. } => {
                let rest = folder.split_once(SCHEME_SEPARATOR).map_or(folder.as_str(), |(_, rest)| rest);
                let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
                Some((host.to_string(), path.trim_end_matches('/').to_string()))
            }
            Self::File { .. } | Self::Live { .. } => None,
        }
    }

    /// The short name for tabs and row 0: the host and folder, such as
    /// `user.github.io/ceefax` (decision E.70 item 13), the file's name,
    /// or the feed.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Channel { .. } | Self::Service { .. } => match self.host_and_folder() {
                Some((host, folder)) if folder.is_empty() => host,
                Some((host, folder)) => format!("{host}/{folder}"),
                None => String::new(),
            },
            Self::File { path } => path.rsplit('/').next().unwrap_or(path).to_string(),
            Self::Live { feed } => feed.clone(),
        }
    }

    /// The connection is encrypted, or there is none: files are local and
    /// zxnet feeds are `wss`.
    #[must_use]
    pub const fn is_secure(&self) -> bool {
        match self {
            Self::Channel { secure, .. } | Self::Service { secure, .. } => *secure,
            Self::File { .. } | Self::Live { .. } => true,
        }
    }

    /// The address bar's text for a page here: a channel's page as the
    /// address of its file, in TTI unless `t42`, which typed back leads to
    /// the same page.
    #[must_use]
    pub fn address(&self, page: Option<u16>, t42: bool) -> String {
        let extension = if t42 { channel::T42_EXTENSION } else { channel::TTI_EXTENSION };
        match (self, page) {
            (Self::Channel { folder, .. }, Some(page)) => format!("{folder}{page:03X}{extension}"),
            (Self::Channel { folder, .. }, None) => folder.clone(),
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

    fn channel(folder: &str, secure: bool) -> Location {
        Location::Channel { folder: folder.to_string(), secure }
    }

    #[test]
    fn addresses_name_channels_and_pages() {
        let cases = [
            ("https://example.org/", channel("https://example.org/", true), None),
            ("example.org", channel("https://example.org/", true), None),
            ("  example.org 140 ", channel("https://example.org/", true), Some(0x140)),
            ("http://127.0.0.1:8001/300.t42", channel("http://127.0.0.1:8001/", false), Some(0x300)),
            ("https://Example.ORG:443/1A0.tti", channel("https://example.org/", true), Some(0x1A0)),
            ("user.github.io/ceefax/", channel("https://user.github.io/ceefax/", true), None),
            ("https://user.github.io/ceefax", channel("https://user.github.io/ceefax/", true), None),
            ("user.github.io/ceefax 300", channel("https://user.github.io/ceefax/", true), Some(0x300)),
            ("http://h.example/a/b/index.t42", channel("http://h.example/a/b/", false), None),
            ("http://h.example/a/channel.json", channel("http://h.example/a/", false), None),
            ("http://h.example/v1.2/", channel("http://h.example/v1.2/", false), None),
            ("http://[::1]:81/", channel("http://[::1]:81/", false), None),
        ];
        for (typed, location, page) in cases {
            assert_eq!(parse_input(typed), Ok(Destination { location, page }), "{typed}");
        }
    }

    #[test]
    fn descriptors_feeds_files_and_contents() {
        let service = parse_input("http://127.0.0.1:9/svc/service.json 200").unwrap();
        assert_eq!(
            service.location,
            Location::Service {
                descriptor: "http://127.0.0.1:9/svc/service.json".into(),
                folder: "http://127.0.0.1:9/svc/".into(),
                secure: false
            }
        );
        assert_eq!(service.page, Some(0x200));
        assert_eq!(parse_input("zxnet:Teefax").unwrap().location, Location::Live { feed: "teefax".into() });
        assert_eq!(parse_input("/tmp/a.t42").unwrap().location, Location::File { path: "/tmp/a.t42".into() });
        assert_eq!(parse_input("file:///tmp/a.tti").unwrap().location, Location::File { path: "/tmp/a.tti".into() });
        let contents = "https://example.org/ceefax/contents.txt";
        assert_eq!(parse_input(contents).unwrap().location, channel("https://example.org/ceefax/", true));
        assert!(wants_contents(contents));
        assert_eq!(page_file("http://h.example/one/140.t42"), Some((0x140, true)));
        assert_eq!(page_file("http://h.example/one/index.t42"), None);
        assert!(!wants_contents("https://example.org/ceefax/"));
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
        // Path tricks are refused, never repaired (REQ-WEB-023).
        assert_eq!(parse_input("https://h.example/a/../b/"), Err(AddressError::Channel(ChannelError::DotSegment)));
        assert_eq!(parse_input("https://h.example/a%2Fb/"), Err(AddressError::Channel(ChannelError::Character)));
        assert_eq!(parse_input("https://h.example/a//b/"), Err(AddressError::Channel(ChannelError::EmptySegment)));
        assert_eq!(parse_input("https://h.example/a/?x=1"), Err(AddressError::Channel(ChannelError::Query)));
        assert_eq!(
            parse_input("https://h.example/a/page.html"),
            Err(AddressError::Channel(ChannelError::NotChannelFile))
        );
    }

    #[test]
    fn channels_are_folders() {
        let a = parse_input("https://example.org/100.tti").unwrap().location;
        let b = parse_input("example.org 200").unwrap().location;
        assert_eq!(a.site_key(), b.site_key());
        let one = parse_input("http://h.example/one/").unwrap().location;
        for other in [
            "https://example.org:8443/",
            "http://example.org/",
            "https://news.example.org/",
            "https://example.org/sub/",
        ] {
            assert_ne!(a.site_key(), parse_input(other).unwrap().location.site_key(), "{other}");
        }
        assert_ne!(one.site_key(), parse_input("http://h.example/one-more/").unwrap().location.site_key());
        assert_ne!(
            one.site_key(),
            parse_input("http://h.example/One/").unwrap().location.site_key(),
            "paths keep case"
        );
        assert_ne!(one.site_key(), parse_input("http://h.example/one/two/").unwrap().location.site_key());
    }

    #[test]
    fn locations_round_trip_through_the_address_bar() {
        for typed in ["https://example.org/140.tti", "http://127.0.0.1:8001/one/", "zxnet:teefax", "file:///tmp/x.tti"]
        {
            let destination = parse_input(typed).unwrap();
            let shown = destination.location.address(destination.page, false);
            assert_eq!(parse_input(&shown).unwrap(), destination, "{typed} shown as {shown}");
        }
        let t42 = parse_input("http://127.0.0.1:8001/one/").unwrap().location;
        assert_eq!(t42.address(Some(0x200), true), "http://127.0.0.1:8001/one/200.t42");
        assert_eq!(t42.fetcher_address().as_deref(), Some("channel:http://127.0.0.1:8001/one/"));
        assert_eq!(t42.label(), "127.0.0.1:8001/one");
        assert_eq!(channel("https://example.org/", true).label(), "example.org");
        assert!(!t42.is_secure());
    }
}
