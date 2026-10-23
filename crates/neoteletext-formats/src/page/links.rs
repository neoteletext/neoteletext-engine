//! The records of the TTI Link Extension (Appendix E, decision E.88) and of
//! the web-native concept's page-number and text links.

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
/// The longest address a link may name (the web-native concept
/// section 3): longer addresses make the link invalid.
pub const LINK_ADDRESS_BYTES: usize = 256;
/// What marks a link to a web page, to open outside teletext, rather than
/// a teletext channel (Appendix E.1, decision E.88).
pub const WEB_MARK: &[u8] = b"web:";
/// `LK` lines per page: page numbers linked elsewhere (concept section 5
/// item 3).
pub const NUMBER_LINKS: usize = 8;
/// `LT` lines per page: runs of text linked elsewhere (concept section 5
/// item 2).
pub const TEXT_LINKS: usize = 8;

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
    pub url: Text<LINK_ADDRESS_BYTES>,
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
    pub fn is_web(&self) -> bool {
        self.url.as_bytes().starts_with(WEB_MARK)
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
    pub url: Text<LINK_ADDRESS_BYTES>,
}

impl LinkDestination {
    /// True when the target is a web page, to open outside teletext
    /// (decision E.88).
    #[must_use]
    pub fn is_web(&self) -> bool {
        self.url.as_bytes().starts_with(WEB_MARK)
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
