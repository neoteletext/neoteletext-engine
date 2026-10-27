//! The page file representation shared by the TTI, EP1, edit.tf and screen
//! dump readers: one sub-page of fixed size, with the extension data of the
//! TTI Link Extension (Appendix E) kept beside it (implementation decision
//! 7).

use neoteletext_core::consts::{
    C4_ERASE, C12, C13, C14, COLUMNS, CONTROL_MASK, DEFAULT_INITIAL_PAGE, DESIGNATION_CODES, NATIONAL_INDEX_MASK,
    NATIONAL_SHIFT, SPACE, STORED_ROWS, TRIPLETS_PER_PACKET,
};
use neoteletext_core::packet::Triplet;
use neoteletext_core::types::{ControlBits, FixedVec, Magazine, PageAddress, PageNumber, SubCode};

use crate::consts::{TTI_PS_C4, TTI_PS_C5_SHIFT, TTI_PS_C5_TO_C11, TTI_PS_C12_BIT};
use crate::text::Text;

mod interaction;
mod issues;
mod links;
mod packets;

pub use interaction::CursorLine;
pub use issues::{IssueKind, PAGE_ISSUES, PageIssue, Warning};
pub use links::{
    LINK_ADDRESS_BYTES, LINK_LABEL_BYTES, LINK_LINES, LinkDestination, LinkKey, LinkLine, NUMBER_LINKS, NumberLink,
    SERVICE_NAME_BYTES, TEXT_LINKS, TextLink, WEB_MARK,
};
pub use packets::{EnhancementPacket, FASTEXT_LINKS, FastextLink, RAW_PACKETS, RawBody, RawPacket};

/// Bytes kept of a description (`DE`) or source (`DS`); the MRG format
/// allows 24 characters.
pub const DESCRIPTION_BYTES: usize = 64;

/// The page function and page coding of `PF` or X/28/0 (Tables 3 and 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct PageFunction {
    /// Page function, 4 bits.
    pub function: u8,
    /// Page coding, 3 bits.
    pub coding: u8,
}

/// One sub-page read from a page file.
#[derive(Clone, Copy, Debug)]
pub struct PageFile {
    /// Magazine and page number.
    pub address: PageAddress,
    /// The sub-page number of `PN`.
    pub subpage: u8,
    /// The sub-code.
    pub subcode: SubCode,
    /// The page status word in TTI `PS` form; see [`PageFile::control`].
    pub status: u16,
    /// `RE`: bits 3 to 6 of the default character set designation.
    pub region: Option<u8>,
    /// `PF`: page function and coding.
    pub function: Option<PageFunction>,
    /// `DE`.
    pub description: Text<DESCRIPTION_BYTES>,
    /// `DS`.
    pub source: Text<DESCRIPTION_BYTES>,
    /// Rows 0 to 25 as 7-bit codes; row 0 is the header row.
    pub rows: [[u8; COLUMNS]; STORED_ROWS],
    /// Bit r set when row r was given.
    pub rows_present: u32,
    /// Packets X/26, at most one per designation code.
    pub enhancements: FixedVec<EnhancementPacket, DESIGNATION_CODES>,
    /// Other packed enhancement lines.
    pub raw_packets: FixedVec<RawPacket, RAW_PACKETS>,
    /// X/27/0 links from `FL` or `OL,27`; `None` for a null link.
    pub fastext: [Option<FastextLink>; FASTEXT_LINKS],
    /// The X/27/0 "display row 24" flag.
    pub fastext_row_24: bool,
    /// `LN` lines.
    pub links: FixedVec<LinkLine, LINK_LINES>,
    /// False when the `LN` lines broke Appendix E.2 rule 1, so the page has
    /// no links.
    pub links_valid: bool,
    /// `LK` lines.
    pub number_links: FixedVec<NumberLink, NUMBER_LINKS>,
    /// `LT` lines.
    pub text_links: FixedVec<TextLink, TEXT_LINKS>,
    /// The last `CU` line (REQ-INT-006).
    pub cursor: Option<CursorLine>,
    /// Problems found while reading.
    pub issues: FixedVec<PageIssue, PAGE_ISSUES>,
    /// Bit w set when warning w was marked `CK,considered`.
    pub considered: u16,
    /// The line of `PN`, or 0.
    pub line: u32,
}

/// The default page address of files that carry none.
pub(crate) const DEFAULT_ADDRESS: PageAddress = PageAddress {
    magazine: Magazine::from_bits((DEFAULT_INITIAL_PAGE >> 8) as u8),
    page: PageNumber(DEFAULT_INITIAL_PAGE as u8),
};

impl PageFile {
    /// An empty page at the default address 100, all rows absent.
    #[must_use]
    pub const fn new() -> Self {
        let triplets = [Triplet { address: 0, mode: 0, data: 0 }; TRIPLETS_PER_PACKET];
        let link = LinkLine {
            key: LinkKey::Index,
            service: Text::new(),
            address: DEFAULT_ADDRESS,
            subcode: None,
            url: Text::new(),
            label: Text::new(),
            label_truncated: false,
            line: 0,
        };
        let destination =
            LinkDestination { service: Text::new(), address: DEFAULT_ADDRESS, subcode: None, url: Text::new() };
        Self {
            address: DEFAULT_ADDRESS,
            subpage: 0,
            subcode: SubCode::new(0),
            status: 0,
            region: None,
            function: None,
            description: Text::new(),
            source: Text::new(),
            rows: [[SPACE; COLUMNS]; STORED_ROWS],
            rows_present: 0,
            enhancements: FixedVec::new(EnhancementPacket { designation: 0, triplets }),
            raw_packets: FixedVec::new(RawPacket { number: 0, designation: 0, body: RawBody::Triplets(triplets) }),
            fastext: [None; FASTEXT_LINKS],
            fastext_row_24: false,
            links: FixedVec::new(link),
            links_valid: true,
            number_links: FixedVec::new(NumberLink { number: DEFAULT_ADDRESS, destination, line: 0 }),
            text_links: FixedVec::new(TextLink { row: 0, column: 0, length: 0, destination, line: 0 }),
            cursor: None,
            issues: FixedVec::new(PageIssue { kind: IssueKind::BadCursorLine, line: 0 }),
            considered: 0,
            line: 0,
        }
    }

    /// Returns the page to the empty state of [`PageFile::new`] without
    /// building a new value on the stack.
    pub fn clear(&mut self) {
        self.address = DEFAULT_ADDRESS;
        self.subpage = 0;
        self.subcode = SubCode::new(0);
        self.status = 0;
        self.region = None;
        self.function = None;
        self.description.clear();
        self.source.clear();
        self.rows = [[SPACE; COLUMNS]; STORED_ROWS];
        self.rows_present = 0;
        self.enhancements.clear();
        self.raw_packets.clear();
        self.fastext = [None; FASTEXT_LINKS];
        self.fastext_row_24 = false;
        self.links.clear();
        self.links_valid = true;
        self.number_links.clear();
        self.text_links.clear();
        self.cursor = None;
        self.issues.clear();
        self.considered = 0;
        self.line = 0;
    }

    /// The control bits C4 to C14 of the page status word (decision E.21):
    /// 0x4000 is C4, bits 0 to 6 are C5 to C11 and bits 7 to 9 are C12 to
    /// C14.
    #[must_use]
    pub fn control(&self) -> ControlBits {
        let national = u16::from(NATIONAL_INDEX_MASK);
        let mut bits = 0u16;
        if self.status & TTI_PS_C4 != 0 {
            bits |= C4_ERASE;
        }
        bits |= (self.status & TTI_PS_C5_TO_C11) << TTI_PS_C5_SHIFT;
        bits |= ((self.status >> TTI_PS_C12_BIT) & national) << NATIONAL_SHIFT;
        ControlBits(bits & CONTROL_MASK)
    }

    /// Sets the control bits, keeping the status bits that are not control
    /// bits, such as 0x8000 "transmit page".
    pub fn set_control(&mut self, control: ControlBits) {
        let national = u16::from(NATIONAL_INDEX_MASK);
        let mask = TTI_PS_C4 | TTI_PS_C5_TO_C11 | (national << TTI_PS_C12_BIT);
        let mut status = self.status & !mask;
        if control.0 & C4_ERASE != 0 {
            status |= TTI_PS_C4;
        }
        status |= (control.0 >> TTI_PS_C5_SHIFT) & TTI_PS_C5_TO_C11;
        status |= ((control.0 >> NATIONAL_SHIFT) & national) << TTI_PS_C12_BIT;
        self.status = status;
    }

    /// Sets C12 to C14 so that the national option index 4×C12 + 2×C13 +
    /// C14 is `index` (decision E.21).
    pub fn set_national_index(&mut self, index: u8) {
        let mut bits = self.control().0 & !(C12 | C13 | C14);
        for (bit, weight) in [(C12, 0b100), (C13, 0b010), (C14, 0b001)] {
            if index & weight != 0 {
                bits |= bit;
            }
        }
        self.set_control(ControlBits(bits));
    }

    /// True when row `row` was given.
    #[must_use]
    pub fn has_row(&self, row: usize) -> bool {
        u32::try_from(row).ok().and_then(|row| self.rows_present.checked_shr(row)).is_some_and(|bits| bits & 1 == 1)
    }

    /// Row `row`'s codes, whether or not it was given (absent rows are
    /// spaces).
    #[must_use]
    pub fn row(&self, row: usize) -> Option<&[u8; COLUMNS]> {
        self.rows.get(row)
    }

    /// Sets row `row` from 7-bit codes, padding with spaces.
    pub fn set_row(&mut self, row: usize, codes: &[u8]) {
        if let Some(target) = self.rows.get_mut(row) {
            *target = [SPACE; COLUMNS];
            for (slot, &code) in target.iter_mut().zip(codes) {
                *slot = code;
            }
            if let Some(bit) = u32::try_from(row).ok().and_then(|row| 1u32.checked_shl(row)) {
                self.rows_present |= bit;
            }
        }
    }

    /// Records a problem. Returns false when the issue list was full.
    pub fn add_issue(&mut self, kind: IssueKind, line: u32) -> bool {
        self.issues.push(PageIssue { kind, line })
    }

    /// True when a problem of this kind was recorded.
    #[must_use]
    pub fn has_issue(&self, kind: IssueKind) -> bool {
        self.issues.as_slice().iter().any(|issue| issue.kind == kind)
    }

    /// True when the page's links come from `LN` lines (Appendix E.2 rule 3:
    /// `LN` wins over `FL`).
    #[must_use]
    pub fn uses_link_lines(&self) -> bool {
        !self.links.is_empty()
    }
}

// `FixedVec`'s derived equality also compares its unused slots, which hold
// whatever an earlier page left; pages compare by what they hold.
impl PartialEq for PageFile {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
            && self.subpage == other.subpage
            && self.subcode == other.subcode
            && self.status == other.status
            && self.region == other.region
            && self.function == other.function
            && self.description == other.description
            && self.source == other.source
            && self.rows == other.rows
            && self.rows_present == other.rows_present
            && self.enhancements.as_slice() == other.enhancements.as_slice()
            && self.raw_packets.as_slice() == other.raw_packets.as_slice()
            && self.fastext == other.fastext
            && self.fastext_row_24 == other.fastext_row_24
            && self.links.as_slice() == other.links.as_slice()
            && self.links_valid == other.links_valid
            && self.number_links.as_slice() == other.number_links.as_slice()
            && self.text_links.as_slice() == other.text_links.as_slice()
            && self.cursor == other.cursor
            && self.issues.as_slice() == other.issues.as_slice()
            && self.considered == other.considered
            && self.line == other.line
    }
}

impl Eq for PageFile {}

impl Default for PageFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use neoteletext_core::consts::{C5_NEWSFLASH, C8_UPDATE, C11_SERIAL, C12, C14};

    #[test]
    fn status_maps_to_control_bits_as_e21() {
        let mut page = PageFile::new();
        page.status = 0x8000 | 0x4000 | 0x0001 | 0x0040 | 0x0080;
        let control = page.control();
        assert_eq!(control.0, C4_ERASE | C5_NEWSFLASH | C11_SERIAL | C12);
        assert_eq!(control.national_index(), 4);
        page.set_control(ControlBits(C8_UPDATE | C14));
        assert_eq!(page.status, 0x8000 | 0x0008 | 0x0200);
        for bit in 4..=14 {
            page.set_control(ControlBits(1 << bit));
            assert_eq!(page.control(), ControlBits(1 << bit), "C{bit}");
        }
    }

    #[test]
    fn rows_are_tracked() {
        let mut page = PageFile::new();
        assert!(!page.has_row(3));
        page.set_row(3, b"AB");
        assert!(page.has_row(3));
        assert_eq!(&page.row(3).unwrap()[..3], b"AB ");
        page.clear();
        assert!(!page.has_row(3));
        assert_eq!(page.address.mpp(), 0x100);
    }
}
