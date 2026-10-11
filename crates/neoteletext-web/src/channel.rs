//! Channels: a channel is a folder on a web server (decision E.70,
//! REQ-WEB-018, REQ-WEB-019, REQ-WEB-023, REQ-WEB-027).
//!
//! A channel's address is an `http` or `https` address whose path ends in
//! `/`. Its files have fixed names in that folder. This module holds the
//! rules every part of the program shares: which addresses name a channel,
//! which file names a channel has, whether an address lies inside a
//! channel, and the checks a channel's file must pass for the format its
//! name says.
//!
//! The folder rules are checked on the address as written, before the
//! strict parser of REQ-WEB-016 removes `.` and `..` segments, because an
//! address that needs repairing is refused, never repaired (REQ-WEB-023).

use neoteletext_core::consts::PACKET_BYTES;
use neoteletext_core::ecc::hamming84;
use neoteletext_core::types::PageAddress;

use crate::bytes::Writer;
use crate::consts::MAX_URL_LENGTH;
use crate::url::{Scheme, Url, UrlError};

/// The channel's settings (REQ-WEB-024).
pub const SETTINGS_FILE: &str = "channel.json";
/// The channel's bundle of pages (REQ-WEB-019).
pub const BUNDLE_FILE: &str = "index.t42";
/// The channel's contents (REQ-WEB-025).
pub const CONTENTS_FILE: &str = "contents.txt";
/// The extension of a page file in TTI (REQ-INP-013).
pub const TTI_EXTENSION: &str = ".tti";
/// The extension of a page file in T42 (REQ-INP-002).
pub const T42_EXTENSION: &str = ".t42";
/// The most segments a folder path may have (REQ-WEB-023).
pub const MAX_SEGMENTS: usize = 8;
/// The longest segment of a folder path.
pub const MAX_SEGMENT: usize = 64;
/// The longest folder path, slashes included.
pub const MAX_FOLDER_PATH: usize = 256;
/// Digits of a page number in a file name.
const PAGE_DIGITS: usize = 3;
/// The command that starts a page in a TTI file (REQ-INP-013).
const TTI_PAGE_COMMAND: &[u8] = b"PN,";

/// Why an address is not a channel's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelError {
    /// The strict parser refused it.
    Address(UrlError),
    /// Only `http` and `https` lead to channels.
    Scheme,
    /// A channel's address has no query.
    Query,
    /// A segment holds a character outside the unreserved set of RFC 3986
    /// section 2.3, including `%`.
    Character,
    /// An empty segment: a doubled slash.
    EmptySegment,
    /// A segment that is only dots, such as `.` or `..`.
    DotSegment,
    /// A segment longer than [`MAX_SEGMENT`].
    LongSegment,
    /// More than [`MAX_SEGMENTS`] segments.
    TooDeep,
    /// A path longer than [`MAX_FOLDER_PATH`].
    TooLong,
    /// The last segment is not one of a channel's file names.
    NotChannelFile,
}

/// A file of a channel (REQ-WEB-019).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelFile {
    /// `NNN.tti` or `NNN.t42`.
    Page {
        /// The page.
        page: PageAddress,
        /// The file is T42; otherwise TTI.
        t42: bool,
    },
    /// `index.t42`.
    Bundle,
    /// `channel.json`.
    Settings,
    /// `contents.txt`.
    Contents,
}

impl ChannelFile {
    /// Reads a file name: a page file, written with capital `A` to `F`, or
    /// one of the other fixed names. `None` for any other name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            SETTINGS_FILE => return Some(Self::Settings),
            BUNDLE_FILE => return Some(Self::Bundle),
            CONTENTS_FILE => return Some(Self::Contents),
            _ => {}
        }
        let (digits, t42) = if let Some(digits) = name.strip_suffix(TTI_EXTENSION) {
            (digits, false)
        } else {
            (name.strip_suffix(T42_EXTENSION)?, true)
        };
        let capital_hex = |byte: &u8| byte.is_ascii_digit() || (b'A'..=b'F').contains(byte);
        if digits.len() != PAGE_DIGITS || !digits.bytes().all(|byte| capital_hex(&byte)) {
            return None;
        }
        let mpp = crate::bytes::hex_number(digits.as_bytes())?;
        Some(Self::Page { page: PageAddress::from_mpp(mpp)?, t42 })
    }
}

/// Checks one folder segment (REQ-WEB-023).
fn check_segment(segment: &[u8]) -> Result<(), ChannelError> {
    if segment.is_empty() {
        return Err(ChannelError::EmptySegment);
    }
    if segment.iter().all(|&byte| byte == b'.') {
        return Err(ChannelError::DotSegment);
    }
    if segment.len() > MAX_SEGMENT {
        return Err(ChannelError::LongSegment);
    }
    // RFC 3986 section 2.3: ALPHA / DIGIT / "-" / "." / "_" / "~".
    let unreserved = |byte: &u8| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');
    if !segment.iter().all(unreserved) {
        return Err(ChannelError::Character);
    }
    Ok(())
}

/// Checks a folder path as written: `/`, or `/` followed by segments each
/// ending in `/` (REQ-WEB-023).
pub fn check_folder_path(path: &str) -> Result<(), ChannelError> {
    if path.len() > MAX_FOLDER_PATH {
        return Err(ChannelError::TooLong);
    }
    let inner = path.strip_prefix('/').and_then(|rest| if rest.is_empty() { Some("") } else { rest.strip_suffix('/') });
    let Some(inner) = inner else { return Err(ChannelError::EmptySegment) };
    if inner.is_empty() {
        return Ok(());
    }
    let mut count = 0usize;
    for segment in inner.split('/') {
        check_segment(segment.as_bytes())?;
        count = count.saturating_add(1);
    }
    if count > MAX_SEGMENTS {
        return Err(ChannelError::TooDeep);
    }
    Ok(())
}

/// The path of an absolute address as written: from the first `/` after
/// the authority to the query, the fragment or the end. `None` when there
/// is no path. Scanned byte by byte, as the address parser does, so that
/// no search can reach a bound (REQ-GEN-019).
fn written_path(text: &[u8]) -> Option<&[u8]> {
    let scheme_end = text.windows(3).position(|window| window == b"://")?;
    let after_scheme = text.get(scheme_end.saturating_add(3)..)?;
    let start = after_scheme.iter().position(|&byte| byte == b'/')?;
    let path = after_scheme.get(start..)?;
    let end = path.iter().position(|&byte| byte == b'?' || byte == b'#').unwrap_or(path.len());
    path.get(..end)
}

/// A channel's address and the file it names, if any.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChannelAddress {
    /// The channel's folder: its address ends in `/`.
    pub folder: Url,
    /// The file named, when the address is not the folder itself.
    pub file: Option<ChannelFile>,
}

/// Reads an absolute address as a channel's folder or one of its files
/// (REQ-WEB-023). The rules are checked on the address as written; the
/// strict parser then checks the rest. An address without a path is the
/// root folder.
pub fn parse_address(text: &str) -> Result<ChannelAddress, ChannelError> {
    let url = Url::parse(text).map_err(ChannelError::Address)?;
    if !matches!(url.scheme(), Scheme::Http | Scheme::Https) {
        return Err(ChannelError::Scheme);
    }
    if url.query().is_some() || text.bytes().any(|byte| byte == b'?') {
        return Err(ChannelError::Query);
    }
    let written = written_path(text.as_bytes()).unwrap_or(b"/");
    let (folder_path, last) = match written.iter().rposition(|&byte| byte == b'/') {
        Some(slash) => (written.get(..=slash).unwrap_or(b"/"), written.get(slash.saturating_add(1)..).unwrap_or(b"")),
        None => (&b"/"[..], &b""[..]),
    };
    // The address parser accepted the whole address, so its path is ASCII.
    let folder_path = core::str::from_utf8(folder_path).map_err(|_| ChannelError::Character)?;
    let last = core::str::from_utf8(last).map_err(|_| ChannelError::Character)?;
    check_folder_path(folder_path)?;
    let file =
        if last.is_empty() { None } else { Some(ChannelFile::from_name(last).ok_or(ChannelError::NotChannelFile)?) };
    let folder = url.join(folder_path).map_err(ChannelError::Address)?;
    // The parser must not have changed what was checked.
    if folder.path() != folder_path {
        return Err(ChannelError::DotSegment);
    }
    Ok(ChannelAddress { folder, file })
}

/// True when `url` lies inside the channel whose folder is `folder`: the
/// same origin, and a path that starts with the folder's (REQ-WEB-026).
#[must_use]
pub fn is_inside(folder: &Url, url: &Url) -> bool {
    folder.origin().same(&url.origin()) && url.path().starts_with(folder.path())
}

/// True when `url` is a redirect target that keeps the channel
/// (REQ-WEB-021): inside it, or the same host and folder over `https`
/// when `folder` is plain `http` and both ports are the schemes' defaults.
#[must_use]
pub fn redirect_keeps_channel(folder: &Url, url: &Url) -> bool {
    if is_inside(folder, url) {
        return true;
    }
    !folder.is_secure()
        && url.is_secure()
        && url.host_text() == folder.host_text()
        && folder.port() == Scheme::Http.default_port()
        && url.port() == Scheme::Https.default_port()
        && url.path().starts_with(folder.path())
}

/// The address of a file of the channel.
pub fn file_url(folder: &Url, file: ChannelFile) -> Result<Url, UrlError> {
    let mut buffer = [0u8; MAX_URL_LENGTH];
    let mut writer = Writer::new(&mut buffer);
    let too_long = |_| UrlError::TooLong;
    match file {
        ChannelFile::Page { page, t42 } => {
            writer.put(&page.ascii()).map_err(too_long)?;
            let extension = if t42 { T42_EXTENSION } else { TTI_EXTENSION };
            writer.put(extension.as_bytes()).map_err(too_long)?;
        }
        ChannelFile::Bundle => writer.put(BUNDLE_FILE.as_bytes()).map_err(too_long)?,
        ChannelFile::Settings => writer.put(SETTINGS_FILE.as_bytes()).map_err(too_long)?,
        ChannelFile::Contents => writer.put(CONTENTS_FILE.as_bytes()).map_err(too_long)?,
    }
    let name = core::str::from_utf8(writer.written()).map_err(|_| UrlError::BadCharacter)?;
    folder.join(name)
}

/// A TTI page file must have at least one `PN` line (REQ-WEB-027).
#[must_use]
pub fn is_tti(bytes: &[u8]) -> bool {
    bytes.split(|&byte| byte == b'\n').any(|line| line.starts_with(TTI_PAGE_COMMAND))
}

/// A T42 file must be a whole, non-zero number of 42-byte packets, each
/// with a valid Hamming 8/4 magazine and row address (EN 300 706 clause
/// 8.2, REQ-WEB-027).
#[must_use]
pub fn is_t42(bytes: &[u8]) -> bool {
    let (packets, rest) = bytes.as_chunks::<PACKET_BYTES>();
    !packets.is_empty()
        && rest.is_empty()
        && packets.iter().all(|packet| {
            packet.first().and_then(|&b| hamming84(b)).is_some() && packet.get(1).and_then(|&b| hamming84(b)).is_some()
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    fn folder(text: &str) -> Result<std::string::String, ChannelError> {
        parse_address(text).map(|address| {
            assert!(address.file.is_none(), "{text}");
            std::string::String::from(address.folder.as_str())
        })
    }

    #[test]
    fn folders_are_checked_as_written() {
        assert_eq!(folder("https://example.org"), Ok("https://example.org/".into()));
        assert_eq!(folder("https://example.org/"), Ok("https://example.org/".into()));
        assert_eq!(folder("https://User.GitHub.io/ceefax/"), Ok("https://user.github.io/ceefax/".into()));
        assert_eq!(folder("http://h.example:8080/~ann/v1.2/"), Ok("http://h.example:8080/~ann/v1.2/".into()));
        assert_eq!(folder("https://h.example/a/../b/"), Err(ChannelError::DotSegment));
        assert_eq!(folder("https://h.example/a/./"), Err(ChannelError::DotSegment));
        assert_eq!(folder("https://h.example/a/.../"), Err(ChannelError::DotSegment));
        assert_eq!(folder("https://h.example/a%2Fb/"), Err(ChannelError::Character));
        assert_eq!(folder("https://h.example/%2E%2E/"), Err(ChannelError::Character));
        assert_eq!(folder("https://h.example/a//b/"), Err(ChannelError::EmptySegment));
        assert_eq!(folder("https://h.example/a/?x=1"), Err(ChannelError::Query));
        assert!(matches!(folder("https://h.example/a/#top"), Err(ChannelError::Address(_))));
        assert!(matches!(folder("wss://h.example/a/"), Err(ChannelError::Scheme)));
        let nine = "https://h.example/a/b/c/d/e/f/g/h/i/";
        assert_eq!(folder(nine), Err(ChannelError::TooDeep));
        let long = std::format!("https://h.example/{}/", "x".repeat(MAX_SEGMENT + 1));
        assert_eq!(folder(&long), Err(ChannelError::LongSegment));
    }

    #[test]
    fn file_names_are_fixed() {
        let page = |mpp| PageAddress::from_mpp(mpp).unwrap();
        assert_eq!(ChannelFile::from_name("140.tti"), Some(ChannelFile::Page { page: page(0x140), t42: false }));
        assert_eq!(ChannelFile::from_name("1FE.t42"), Some(ChannelFile::Page { page: page(0x1FE), t42: true }));
        assert_eq!(ChannelFile::from_name("1fe.t42"), None, "capital digits only");
        assert_eq!(ChannelFile::from_name("940.tti"), None);
        assert_eq!(ChannelFile::from_name("P100.tti"), None);
        assert_eq!(ChannelFile::from_name("index.t42"), Some(ChannelFile::Bundle));
        assert_eq!(ChannelFile::from_name("index.tti"), None);
        assert_eq!(ChannelFile::from_name("channel.json"), Some(ChannelFile::Settings));
        assert_eq!(ChannelFile::from_name("contents.txt"), Some(ChannelFile::Contents));
        let address = parse_address("https://h.example/ceefax/140.tti").unwrap();
        assert_eq!(address.folder.as_str(), "https://h.example/ceefax/");
        assert_eq!(address.file, Some(ChannelFile::Page { page: page(0x140), t42: false }));
        assert_eq!(parse_address("https://h.example/ceefax/page.html"), Err(ChannelError::NotChannelFile));
        let url = file_url(&address.folder, ChannelFile::Page { page: page(0x1FE), t42: true }).unwrap();
        assert_eq!(url.as_str(), "https://h.example/ceefax/1FE.t42");
    }

    #[test]
    fn inside_means_the_same_origin_and_below_the_folder() {
        let at = |text| Url::parse(text).unwrap();
        let one = at("http://h.example/one/");
        assert!(is_inside(&one, &at("http://h.example/one/100.tti")));
        assert!(!is_inside(&one, &at("http://h.example/two/100.tti")));
        assert!(!is_inside(&one, &at("http://h.example/one-more/100.tti")), "/one/ is not a prefix of /one-more/");
        assert!(!is_inside(&one, &at("http://h.example:8080/one/100.tti")));
        assert!(redirect_keeps_channel(&one, &at("https://h.example/one/100.tti")));
        assert!(!redirect_keeps_channel(&one, &at("https://h.example:8443/one/100.tti")), "another port");
        assert!(!redirect_keeps_channel(&one, &at("https://h.example/two/")));
    }

    #[test]
    fn content_must_match_the_name() {
        assert!(is_tti(b"DE,x\r\nPN,10000\r\nOL,1,HELLO\r\n"));
        assert!(!is_tti(b"[% INCLUDE header %]\n"), "a Template Toolkit file");
        let mut packet = [0u8; PACKET_BYTES];
        packet[0] = 0x15;
        packet[1] = 0x15;
        assert!(is_t42(&packet));
        assert!(!is_t42(&packet[..41]));
        assert!(!is_t42(b""));
        // Two bits wrong: Hamming 8/4 detects it but cannot correct it.
        packet[0] = 0x15 ^ 0b11;
        assert!(!is_t42(&packet), "an address that is not Hamming 8/4");
    }
}
