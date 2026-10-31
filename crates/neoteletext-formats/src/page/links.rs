//! The records of the TTI Link Extension (Appendix E, decision E.88) and of
//! the web-native concept's page-number and text links.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use neoteletext_core::limits::MAX_URL_LENGTH;
use neoteletext_core::types::{PageAddress, SubCode};

use crate::text::Text;

/// Bytes kept of a link label. Labels may hold 9 characters (Appendix E.1);
/// more are kept so that the page checker can report them.
pub const LINK_LABEL_BYTES: usize = 40;
/// Bytes of a service name in a link target.
pub const SERVICE_NAME_BYTES: usize = 32;
/// Link Extension links per page: four coloured keys and Index
/// (Appendix E.1).
pub const LINK_LINES: usize = 5;
/// What marks a link to a web page, to open outside teletext, rather than
/// a teletext channel (Appendix E.1, decision E.88).
pub const WEB_MARK: &[u8] = b"web:";
/// The longest address a link's target may hold: the longest address of
/// all (REQ-1661, DEC-1668), with its `web:` mark, so that any address can
/// be a web link. Longer makes the link invalid.
pub const LINK_TARGET_BYTES: usize = MAX_URL_LENGTH + WEB_MARK.len();
/// `LK` lines per page: page numbers linked elsewhere (concept section 5
/// item 3).
pub const NUMBER_LINKS: usize = 8;
/// `LT` lines per page: runs of text linked elsewhere (concept section 5
/// item 2).
pub const TEXT_LINKS: usize = 8;

/// The links of one sub-page at the most: `LN`, `LK` and `LT` lines.
pub const MAX_LINKS: usize = LINK_LINES + NUMBER_LINKS + TEXT_LINKS;
/// The bytes of all one sub-page's link addresses together (DEC-1708):
/// every link at its longest on the heap, and one link at its longest in a
/// build that allocates nothing, so that every build follows any one link.
#[cfg(feature = "alloc")]
pub const LINK_TEXT_BYTES: usize = MAX_LINKS * LINK_TARGET_BYTES;
/// The bytes of all one sub-page's link addresses together (DEC-1708):
/// every link at its longest on the heap, and one link at its longest in a
/// build that allocates nothing, so that every build follows any one link.
#[cfg(not(feature = "alloc"))]
pub const LINK_TEXT_BYTES: usize = LINK_TARGET_BYTES;

/// Where a link's address lies: in its sub-page's [`LinkText`], or in the
/// body of the LINKS record it came in. Empty for a link with no address.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct AddressSpan {
    start: u32,
    length: u16,
    web: bool,
}

impl AddressSpan {
    /// No address.
    pub const NONE: Self = Self { start: 0, length: 0, web: false };

    /// The address `address` at `start` of whatever holds it, or `None`
    /// when it could not be one.
    #[must_use]
    pub fn at(start: usize, address: &[u8]) -> Option<Self> {
        if address.len() > LINK_TARGET_BYTES {
            return None;
        }
        Some(Self {
            start: u32::try_from(start).ok()?,
            length: u16::try_from(address.len()).ok()?,
            web: address.starts_with(WEB_MARK),
        })
    }

    /// True when there is no address.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }

    /// True when the address is a web page's, with its `web:` mark.
    #[must_use]
    pub const fn is_web(self) -> bool {
        self.web
    }

    /// The address in `text`, the bytes that hold it.
    #[must_use]
    pub fn of(self, text: &[u8]) -> &[u8] {
        let start = usize::try_from(self.start).unwrap_or(usize::MAX);
        text.get(start..start.saturating_add(usize::from(self.length))).unwrap_or(&[])
    }
}

/// One sub-page's link addresses, held out of line (DEC-1708): a link
/// holds an [`AddressSpan`] into these, so that a page pays for the
/// addresses it has, not for the longest it could have.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LinkText {
    #[cfg(feature = "alloc")]
    bytes: Vec<u8>,
    #[cfg(not(feature = "alloc"))]
    bytes: Text<LINK_TEXT_BYTES>,
}

impl LinkText {
    /// No addresses.
    #[must_use]
    pub const fn new() -> Self {
        #[cfg(feature = "alloc")]
        let bytes = Vec::new();
        #[cfg(not(feature = "alloc"))]
        let bytes = Text::new();
        Self { bytes }
    }

    /// Adds an address and says where it lies, or `None`, adding nothing,
    /// when it is too long or does not fit.
    pub fn add(&mut self, address: &[u8]) -> Option<AddressSpan> {
        let start = self.as_bytes().len();
        let span = AddressSpan::at(start, address)?;
        if start.saturating_add(address.len()) > LINK_TEXT_BYTES {
            return None;
        }
        #[cfg(feature = "alloc")]
        self.bytes.extend_from_slice(address);
        #[cfg(not(feature = "alloc"))]
        for &byte in address {
            self.bytes.push(byte);
        }
        Some(span)
    }

    /// The address at `span`.
    #[must_use]
    pub fn address(&self, span: AddressSpan) -> &[u8] {
        span.of(self.as_bytes())
    }

    /// Every address held, one after another.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        #[cfg(feature = "alloc")]
        return self.bytes.as_slice();
        #[cfg(not(feature = "alloc"))]
        return self.bytes.as_bytes();
    }
}

/// The keys of the TTI Link Extension (Appendix E.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum LinkKey {
    /// Red.
    Red,
    /// Green.
    Green,
    /// Yellow.
    Yellow,
    /// Cyan.
    Cyan,
    /// Index.
    Index,
}

impl LinkKey {
    /// The key for a letter of an `LN` line.
    #[must_use]
    pub const fn from_letter(letter: u8) -> Option<Self> {
        match letter.to_ascii_uppercase() {
            b'R' => Some(Self::Red),
            b'G' => Some(Self::Green),
            b'Y' => Some(Self::Yellow),
            b'C' => Some(Self::Cyan),
            b'I' => Some(Self::Index),
            _ => None,
        }
    }

    /// The letter written in `LN` lines.
    #[must_use]
    pub const fn letter(self) -> u8 {
        match self {
            Self::Red => b'R',
            Self::Green => b'G',
            Self::Yellow => b'Y',
            Self::Cyan => b'C',
            Self::Index => b'I',
        }
    }

    /// The X/27/0 link this key uses: links 1 to 4 for the colours and link
    /// 6 for Index (REQ-NAV-010), counted from 0 here.
    #[must_use]
    pub const fn fastext_index(self) -> usize {
        match self {
            Self::Red => 0,
            Self::Green => 1,
            Self::Yellow => 2,
            Self::Cyan => 3,
            Self::Index => 5,
        }
    }

    /// True for the four coloured keys.
    #[must_use]
    pub const fn is_coloured(self) -> bool {
        !matches!(self, Self::Index)
    }
}

/// A link of the TTI Link Extension (Appendix E.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkLine {
    /// The key.
    pub key: LinkKey,
    /// The other service named, empty for this service.
    pub service: Text<SERVICE_NAME_BYTES>,
    /// The page.
    pub address: PageAddress,
    /// The sub-page, `None` for any sub-page.
    pub subcode: Option<SubCode>,
    /// The address of another site's page file, when the target is an
    /// address (the web-native concept section 3); empty otherwise.
    pub url: AddressSpan,
    /// The prompt text, as written.
    pub label: Text<LINK_LABEL_BYTES>,
    /// True when the label in the file was longer than this record keeps.
    pub label_truncated: bool,
    /// The line of the file.
    pub line: u32,
}

impl LinkLine {
    /// True when the target is in another service or at an address: both
    /// lead to another site.
    #[must_use]
    pub const fn is_other_service(&self) -> bool {
        !self.service.is_empty() || !self.url.is_empty()
    }

    /// True when the target is a web page, to open outside teletext
    /// (decision E.88).
    #[must_use]
    pub const fn is_web(&self) -> bool {
        self.url.is_web()
    }
}

/// Where a page-number or text link leads: a page of this service, a page
/// of a named service, or an address (concept section 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkDestination {
    /// The other service named, empty for this service or an address.
    pub service: Text<SERVICE_NAME_BYTES>,
    /// The page; for an address, the page its file holds is found when it
    /// is fetched.
    pub address: PageAddress,
    /// The sub-page, `None` for any sub-page.
    pub subcode: Option<SubCode>,
    /// The address, when the target is one, as written: a web page's keeps
    /// its `web:` mark (decision E.88).
    pub url: AddressSpan,
}

impl LinkDestination {
    /// True when the target is a web page, to open outside teletext
    /// (decision E.88).
    #[must_use]
    pub const fn is_web(&self) -> bool {
        self.url.is_web()
    }
}

/// An `LK` line: typing or choosing `number` on this page leads to
/// `destination` (concept section 5 item 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NumberLink {
    /// The page number linked.
    pub number: PageAddress,
    /// Where it leads.
    pub destination: LinkDestination,
    /// The line of the file.
    pub line: u32,
}

/// An `LT` line: the run of `length` characters at `row` and `column` is
/// a link to `destination` (concept section 5 item 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextLink {
    /// The row, 1 to 23.
    pub row: u8,
    /// The first column.
    pub column: u8,
    /// The characters linked.
    pub length: u8,
    /// Where it leads.
    pub destination: LinkDestination,
    /// The line of the file.
    pub line: u32,
}
