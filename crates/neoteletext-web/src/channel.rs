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
use neoteletext_core::types::{PageAddress, SubCode};

use crate::bytes::{Overflow, Writer};
use crate::url::{FRAGMENT_MARK, QUERY_MARK, Scheme, Url, UrlError};

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
/// The extension of a page's catalogue of sub-pages (REQ-WEB-034).
pub const SUBPAGES_EXTENSION: &str = ".sub";
/// Between a page number and a sub-code in a sub-page's file name,
/// `NNN-SSSS.t42` (REQ-WEB-019).
pub const SUBPAGE_SEPARATOR: u8 = b'-';
/// The longest file name of a channel: `NNN-SSSS.t42`, `channel.json` and
/// `contents.txt` are twelve characters.
pub const MAX_FILE_NAME: usize = PAGE_DIGITS + 1 + crate::subpages::SUBCODE_DIGITS + T42_EXTENSION.len();
/// The most segments a folder path may have (REQ-WEB-023).
pub const MAX_SEGMENTS: usize = 8;
/// The longest segment of a folder path.
pub const MAX_SEGMENT: usize = 64;
/// The longest folder path, slashes included.
pub const MAX_FOLDER_PATH: usize = 256;
/// Digits of a page number in a file name.
pub(crate) const PAGE_DIGITS: usize = 3;
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
    /// `NNN-SSSS.t42`: one sub-page of a page whose sub-pages are files
    /// of their own, listed by its `NNN.sub`.
    SubPage {
        /// The page.
        page: PageAddress,
        /// The sub-page.
        subcode: SubCode,
    },
    /// `NNN.sub`: the catalogue of a page's sub-pages (REQ-WEB-034).
    SubPages {
        /// The page.
        page: PageAddress,
    },
    /// `index.t42`.
    Bundle,
    /// `channel.json`.
    Settings,
    /// `contents.txt`.
    Contents,
}

impl ChannelFile {
    /// Reads a file name: a page file, a sub-page's file or a catalogue,
    /// written with capital `A` to `F`, or one of the other fixed names.
    /// `None` for any other name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            SETTINGS_FILE => return Some(Self::Settings),
            BUNDLE_FILE => return Some(Self::Bundle),
            CONTENTS_FILE => return Some(Self::Contents),
            _ => {}
        }
        if let Some(digits) = name.strip_suffix(SUBPAGES_EXTENSION) {
            return Some(Self::SubPages { page: page_of(digits.as_bytes())? });
        }
        let (digits, t42) = if let Some(digits) = name.strip_suffix(TTI_EXTENSION) {
            (digits, false)
        } else {
            (name.strip_suffix(T42_EXTENSION)?, true)
        };
        let digits = digits.as_bytes();
        match digits.split_at_checked(PAGE_DIGITS) {
            Some((page, [])) => Some(Self::Page { page: page_of(page)?, t42 }),
            Some((page, [SUBPAGE_SEPARATOR, code @ ..])) if t42 => {
                let subcode = crate::subpages::parse_subcode(code)?;
                Some(Self::SubPage { page: page_of(page)?, subcode })
            }
            _ => None,
        }
    }
}

/// True for the digits `0` to `9` and the capitals `A` to `F`, the only
/// hexadecimal digits a channel's file names use (REQ-WEB-019).
pub(crate) const fn is_capital_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'A'..=b'F')
}

/// A page number in a file name: three capital hexadecimal digits, the
/// magazine 1 to 8 first.
pub(crate) fn page_of(digits: &[u8]) -> Option<PageAddress> {
    if digits.len() != PAGE_DIGITS || !digits.iter().all(|&byte| is_capital_hex(byte)) {
        return None;
    }
    PageAddress::from_mpp(crate::bytes::hex_number(digits)?)
}

/// Writes a file's name (REQ-WEB-019).
pub fn write_file_name(writer: &mut Writer<'_>, file: ChannelFile) -> Result<(), Overflow> {
    match file {
        ChannelFile::Page { page, t42 } => {
            writer.put(&page.ascii())?;
            writer.put(if t42 { T42_EXTENSION } else { TTI_EXTENSION }.as_bytes())
        }
        ChannelFile::SubPage { page, subcode } => {
            writer.put(&page.ascii())?;
            writer.byte(SUBPAGE_SEPARATOR)?;
            crate::subpages::write_subcode(writer, subcode)?;
            writer.put(T42_EXTENSION.as_bytes())
        }
        ChannelFile::SubPages { page } => {
            writer.put(&page.ascii())?;
            writer.put(SUBPAGES_EXTENSION.as_bytes())
        }
        ChannelFile::Bundle => writer.put(BUNDLE_FILE.as_bytes()),
        ChannelFile::Settings => writer.put(SETTINGS_FILE.as_bytes()),
        ChannelFile::Contents => writer.put(CONTENTS_FILE.as_bytes()),
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
    // A page is chosen after the channel opens: its folder has no fragment.
    if url.fragment().is_some() {
        return Err(ChannelError::Address(UrlError::Fragment));
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

/// Whether an address opens as a channel, a folder or a channel's own file,
/// rather than as a service's descriptor (DEC-1710): the one reader of
/// channels' addresses says.
#[must_use]
pub fn is_channel_address(text: &str) -> bool {
    parse_address(location(text)).is_ok()
}

/// An address without its query and its fragment. A viewer's address may
/// have both (REQ-1693): a channel's folder is the address without them.
#[must_use]
pub fn location(text: &str) -> &str {
    let end = text.as_bytes().iter().position(|&byte| matches!(byte, QUERY_MARK | FRAGMENT_MARK));
    end.and_then(|end| text.get(..end)).unwrap_or(text)
}

/// The percent-encoded separators a server may decode into a path: `.`,
/// `/` and `\` (RFC 3986 section 2.1; hexadecimal digits in either case).
const ENCODED_SEPARATORS: [&[u8; PERCENT_ENCODED_LENGTH]; 3] = [b"%2E", b"%2F", b"%5C"];
/// A percent sign and two hexadecimal digits (RFC 3986 section 2.1).
const PERCENT_ENCODED_LENGTH: usize = 3;

/// True when `url` lies inside the channel whose folder is `folder`: the
/// same origin, and a path below the folder's (REQ-WEB-026).
#[must_use]
pub fn is_inside(folder: &Url, url: &Url) -> bool {
    folder.origin().same(&url.origin()) && below_folder(folder, url)
}

/// True when `url`'s path starts with the folder's and holds no encoded
/// separator after it, which a server that decodes them could follow out
/// of the folder (decision E.114 item 5, finding F-45).
fn below_folder(folder: &Url, url: &Url) -> bool {
    let Some(rest) = url.path().strip_prefix(folder.path()) else { return false };
    !rest
        .as_bytes()
        .windows(PERCENT_ENCODED_LENGTH)
        .any(|window| ENCODED_SEPARATORS.iter().any(|separator| window.eq_ignore_ascii_case(*separator)))
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
        && below_folder(folder, url)
}

/// The address of a file of the channel.
pub fn file_url(folder: &Url, file: ChannelFile) -> Result<Url, UrlError> {
    let mut buffer = [0u8; MAX_FILE_NAME];
    let mut writer = Writer::new(&mut buffer);
    write_file_name(&mut writer, file).map_err(|_| UrlError::TooLong)?;
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
mod tests;
