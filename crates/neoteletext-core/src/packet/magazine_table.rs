//! The Magazine Organization Table (clause 10.6, Figure 20, Tables 20 to
//! 23; REQ-PKT-018), read from the 40 stored data bytes (packet bytes 6 to
//! 45) of a packet, which the page store keeps as received (decision E.56).

use super::triplet_fields::ObjectKind;
use crate::consts::{
    DATA_BYTES, DATA_START, MAGAZINE_MASK, MOT_DECIMAL_DIGITS, MOT_DECIMAL_FIRST_PACKET, MOT_DEFAULT_SECOND_SHIFT,
    MOT_FALLBACK_NONE, MOT_FALLBACK_PANELS_MASK, MOT_FALLBACK_PANELS_SHIFT, MOT_FALLBACK_SUBSTITUTION, MOT_GLOBAL_FLAG,
    MOT_HEX_DIGITS, MOT_HEX_TENS_FIRST_PACKET, MOT_HEX_UNITS_FIRST_PACKET, MOT_LINK_NUMBER_MASK, MOT_LINK_X27_FLAG,
    MOT_OBJECT_LINK_BYTES, MOT_OBJECT_LINK_FIRST_BYTE, MOT_OBJECT_LINKS_PER_PACKET, MOT_PAIRS_DECIMAL_UNITS,
    MOT_PAIRS_HEX_UNITS, MOT_TOTALS_BYTE, PACKET_FIRST_BYTE_NUMBER, SIDE_PANEL_HALF_COLUMNS,
};
use crate::ecc::hamming84;
use crate::types::{FallbackFields, Magazine, PageNumber, SidePanelFields};

/// The packet byte number of the first stored data byte, byte 6: tables
/// give positions as packet byte numbers.
const DATA_FIRST_BYTE_NUMBER: usize = PACKET_FIRST_BYTE_NUMBER + DATA_START;

/// Where the byte pair of a page lies in its Magazine Organization Table:
/// the packet and the index of the first byte within the 40 data bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct MotPosition {
    /// The packet, 1 to 14.
    pub packet: u8,
    /// The index of the pair's first byte in the data bytes (packet byte
    /// number minus 6).
    pub index: usize,
}

/// The position of the byte pair for page `page` (Figure 20; REQ-PKT-018).
#[must_use]
pub fn mot_position(page: PageNumber) -> MotPosition {
    let (tens, units) = (page.tens(), page.units());
    let (first_packet, ordinal, per_packet) = if units < MOT_DECIMAL_DIGITS {
        if tens < MOT_DECIMAL_DIGITS {
            (
                MOT_DECIMAL_FIRST_PACKET,
                tens.saturating_mul(MOT_DECIMAL_DIGITS).saturating_add(units),
                MOT_PAIRS_DECIMAL_UNITS,
            )
        } else {
            let ordinal =
                tens.saturating_sub(MOT_DECIMAL_DIGITS).saturating_mul(MOT_DECIMAL_DIGITS).saturating_add(units);
            (MOT_HEX_TENS_FIRST_PACKET, ordinal, MOT_PAIRS_DECIMAL_UNITS)
        }
    } else {
        let ordinal = tens.saturating_mul(MOT_HEX_DIGITS).saturating_add(units.saturating_sub(MOT_DECIMAL_DIGITS));
        (MOT_HEX_UNITS_FIRST_PACKET, ordinal, MOT_PAIRS_HEX_UNITS)
    };
    let ordinal = usize::from(ordinal);
    MotPosition {
        packet: first_packet.saturating_add(ordinal.checked_div(per_packet).unwrap_or(0) as u8),
        index: ordinal.checked_rem(per_packet).unwrap_or(0).saturating_mul(2),
    }
}

/// The object half of a page's byte pair in the Magazine Organization
/// Table (clauses 10.6.2 and 10.6.3). The second byte names DRCS pages,
/// which are not read (decision E.95).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct MotPageEntry {
    /// First byte bit 4: the page needs objects from the GPOP.
    pub global_objects: bool,
    /// First byte bits 3 to 1: the POP link 1 to 7, or 0 for none.
    pub pop_link: u8,
}

fn nibble_at(row: &[u8; DATA_BYTES], index: usize) -> Option<u8> {
    hamming84(row.get(index).copied()?)
}

/// `N` Hamming 8/4 values from `index`; `None` when any is uncorrectable.
fn nibbles<const N: usize>(row: &[u8; DATA_BYTES], index: usize) -> Option<[u8; N]> {
    let mut out = [0u8; N];
    for (offset, slot) in out.iter_mut().enumerate() {
        *slot = nibble_at(row, index.saturating_add(offset))?;
    }
    Some(out)
}

/// The byte pair at `index` of a table packet; `None` when its first byte
/// is uncorrectable (decision E.7).
#[must_use]
pub fn mot_page_entry(row: &[u8; DATA_BYTES], index: usize) -> Option<MotPageEntry> {
    let [objects] = nibbles::<1>(row, index)?;
    Some(MotPageEntry { global_objects: objects & MOT_GLOBAL_FLAG != 0, pop_link: objects & MOT_LINK_NUMBER_MASK })
}

/// A default object of an object link (clause 10.6.4.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct DefaultObject {
    /// Its type.
    pub kind: ObjectKind,
    /// The 8-bit object number N7 to N0; N8 is 0, so only pointer packets 1
    /// and 2 can be reached.
    pub number: u8,
}

/// An object link of the Magazine Organization Table (Table 21).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct MotObjectLink {
    /// Byte N bits 1 to 3: the magazine of the object page, absolute, with
    /// 0 meaning magazine 8 (decision E.52).
    pub magazine: Magazine,
    /// Byte N bit 4: the link may be superseded by X/27/4 or X/27/5.
    pub x27_flag: bool,
    /// Bytes N+1 and N+2: the page; FF means the link is not used.
    pub page: PageNumber,
    /// Byte N+3: the highest S1 transmitted, so 0 means one sub-page.
    pub subpage_count: u8,
    /// Byte N+4: the side-panel and substitution fallback of a POP link.
    /// It is reserved in the GPOP link.
    pub fallback: FallbackFields,
    /// Bytes N+5 to N+9: up to two default objects.
    pub default_objects: [Option<DefaultObject>; 2],
}

/// Byte offsets within an object link (Table 21).
const LINK_TENS: usize = 1;
const LINK_UNITS: usize = 2;
const LINK_SUBPAGES: usize = 3;
const LINK_FALLBACK: usize = 4;
const LINK_DEFAULT_TYPES: usize = 5;
const LINK_DEFAULT_FIRST: usize = 6;
const LINK_DEFAULT_SECOND: usize = 8;

/// The side panels a fallback value selects (Table 21, byte N+4 bits 3 and
/// 2): none, left 16, right 16, or both 8, in the X/28 field form.
const fn fallback_panels(bits: u8) -> SidePanelFields {
    let (left, right, left_columns) = match bits & MOT_FALLBACK_PANELS_MASK {
        0b00 => (false, false, 0),
        0b01 => (true, false, 0),
        0b10 => (false, true, 0),
        _ => (true, true, SIDE_PANEL_HALF_COLUMNS),
    };
    SidePanelFields { left, right, status_flag: true, left_columns }
}

/// The fallback byte of a POP link.
#[must_use]
pub const fn mot_fallback(byte: u8) -> FallbackFields {
    if byte & MOT_FALLBACK_NONE != 0 {
        return FallbackFields { side_panels: fallback_panels(0), black_background_substitution: false };
    }
    FallbackFields {
        side_panels: fallback_panels(byte >> MOT_FALLBACK_PANELS_SHIFT),
        black_background_substitution: byte & MOT_FALLBACK_SUBSTITUTION != 0,
    }
}

/// Object link `link` (0 to 3) of a table packet X/19, X/20, X/22 or X/23:
/// in X/19 and X/22 link 0 is the GPOP link and links 1 to 3 are POP links
/// 1 to 3; in X/20 and X/23 they are POP links 4 to 7 (Table 20). `None`
/// when any byte is uncorrectable.
#[must_use]
pub fn mot_object_link(row: &[u8; DATA_BYTES], link: usize) -> Option<MotObjectLink> {
    if link >= MOT_OBJECT_LINKS_PER_PACKET {
        return None;
    }
    let start = MOT_OBJECT_LINK_FIRST_BYTE
        .saturating_add(link.saturating_mul(MOT_OBJECT_LINK_BYTES))
        .saturating_sub(DATA_FIRST_BYTE_NUMBER);
    let bytes = nibbles::<MOT_OBJECT_LINK_BYTES>(row, start)?;
    let byte = |offset: usize| bytes.get(offset).copied().unwrap_or(0);
    let types = byte(LINK_DEFAULT_TYPES);
    let default = |kind_bits: u8, number_offset: usize| {
        ObjectKind::from_bits(kind_bits).map(|kind| DefaultObject {
            kind,
            number: byte(number_offset) | byte(number_offset.saturating_add(1)) << 4,
        })
    };
    Some(MotObjectLink {
        magazine: Magazine::from_bits(byte(0) & MAGAZINE_MASK),
        x27_flag: byte(0) & MOT_LINK_X27_FLAG != 0,
        page: PageNumber(byte(LINK_TENS) << 4 | byte(LINK_UNITS)),
        subpage_count: byte(LINK_SUBPAGES),
        fallback: mot_fallback(byte(LINK_FALLBACK)),
        default_objects: [
            default(types, LINK_DEFAULT_FIRST),
            default(types >> MOT_DEFAULT_SECOND_SHIFT, LINK_DEFAULT_SECOND),
        ],
    })
}

/// The totals of enhancement pages in bytes 40 to 43 of X/21 and X/24
/// (clause 10.6.6). Bytes 44 and 45 count DRCS pages, which are not read
/// (decision E.95).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct MotTotals {
    /// Bit `Magazine::index()` set for each magazine included in the totals.
    pub magazines: u8,
    /// The total number of object pages, public and global.
    pub object_pages: u8,
}

/// The totals of a table packet X/21 or X/24; `None` when any byte is
/// uncorrectable.
#[must_use]
pub fn mot_totals(row: &[u8; DATA_BYTES]) -> Option<MotTotals> {
    let start = MOT_TOTALS_BYTE.saturating_sub(DATA_FIRST_BYTE_NUMBER);
    // Byte 40 holds M3 M2 M1 M8 and byte 41 M7 M6 M5 M4, most significant
    // first, which is the order of `Magazine::index()`.
    let [low_magazines, high_magazines, low_objects, high_objects] = nibbles::<4>(row, start)?;
    Some(MotTotals { magazines: low_magazines | high_magazines << 4, object_pages: low_objects | high_objects << 4 })
}

/// Encoders for the table's byte groups, as Hamming 8/4 values before
/// coding: the inverse of the decoders above.
pub(super) mod encode {
    use super::{
        LINK_DEFAULT_FIRST, LINK_DEFAULT_SECOND, LINK_DEFAULT_TYPES, LINK_FALLBACK, LINK_SUBPAGES, LINK_TENS,
        LINK_UNITS, MotObjectLink, MotPageEntry, MotTotals,
    };
    use crate::consts::{
        MAGAZINE_MASK, MOT_DEFAULT_SECOND_SHIFT, MOT_FALLBACK_PANELS_SHIFT, MOT_FALLBACK_SUBSTITUTION, MOT_GLOBAL_FLAG,
        MOT_LINK_NUMBER_MASK, MOT_LINK_X27_FLAG, MOT_OBJECT_LINK_BYTES, NIBBLE_MASK,
    };

    /// The two values of a byte pair; the second, for DRCS pages, is 0.
    #[must_use]
    pub const fn mot_page_entry(entry: MotPageEntry) -> [u8; 2] {
        let objects = if entry.global_objects { MOT_GLOBAL_FLAG } else { 0 } | entry.pop_link & MOT_LINK_NUMBER_MASK;
        [objects, 0]
    }

    /// The ten values of an object link.
    #[must_use]
    pub fn mot_object_link(link: &MotObjectLink) -> [u8; MOT_OBJECT_LINK_BYTES] {
        let mut out = [0u8; MOT_OBJECT_LINK_BYTES];
        let mut set = |offset: usize, value: u8| {
            if let Some(slot) = out.get_mut(offset) {
                *slot = value & NIBBLE_MASK;
            }
        };
        set(0, link.magazine.bits() & MAGAZINE_MASK | if link.x27_flag { MOT_LINK_X27_FLAG } else { 0 });
        set(LINK_TENS, link.page.tens());
        set(LINK_UNITS, link.page.units());
        set(LINK_SUBPAGES, link.subpage_count);
        let panels = link.fallback.side_panels;
        let panel_bits = match (panels.left, panels.right) {
            (false, false) => 0b00,
            (true, false) => 0b01,
            (false, true) => 0b10,
            // Table 21 has only the 8 + 8 split for two panels.
            (true, true) => 0b11,
        };
        let substitution = if link.fallback.black_background_substitution { MOT_FALLBACK_SUBSTITUTION } else { 0 };
        set(LINK_FALLBACK, panel_bits << MOT_FALLBACK_PANELS_SHIFT | substitution);
        let kind_bits =
            |index: usize| link.default_objects.get(index).copied().flatten().map_or(0, |object| object.kind.bits());
        set(LINK_DEFAULT_TYPES, kind_bits(0) | kind_bits(1) << MOT_DEFAULT_SECOND_SHIFT);
        for (index, offset) in [(0, LINK_DEFAULT_FIRST), (1, LINK_DEFAULT_SECOND)] {
            let number = link.default_objects.get(index).copied().flatten().map_or(0, |object| object.number);
            set(offset, number);
            set(offset.saturating_add(1), number >> 4);
        }
        out
    }

    /// The six values of the totals; the DRCS total is 0.
    #[must_use]
    pub const fn mot_totals(totals: &MotTotals) -> [u8; 6] {
        [
            totals.magazines & NIBBLE_MASK,
            totals.magazines >> 4,
            totals.object_pages & NIBBLE_MASK,
            totals.object_pages >> 4,
            0,
            0,
        ]
    }
}
