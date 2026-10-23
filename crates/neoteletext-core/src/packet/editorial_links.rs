//! Editorial links, packet X/27/0 (clause 9.6.1, REQ-PKT-005), and the
//! 6-byte page reference it shares with packet 8/30 (Figure 12).

use super::{Packet, nibble};
use crate::consts::{LINK_NULL_PAGE, LINK_NULL_SUBCODE, MAGAZINE_MASK};
use crate::types::{Magazine, PageNumber, SubCode};

/// One editorial link of packet X/27/0 (clause 9.6.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Link {
    /// The linked magazine, after applying the relative bits M1 to M3.
    pub magazine: Magazine,
    /// The linked page number.
    pub page: PageNumber,
    /// The linked sub-code; 3F7F means any sub-page.
    pub subcode: SubCode,
}

/// Packet X/27/0: six links, the link control byte and the page checksum
/// (REQ-PKT-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Links {
    /// The magazine.
    pub magazine: Magazine,
    /// Links 0 to 5. `None` for a null link or an uncorrectable byte
    /// (decision E.7).
    pub links: [Option<Link>; 6],
    /// Data bit 4 of the link control byte: display row 24 (Table 14).
    pub display_row_24: bool,
    /// The transmitted checksum, bytes 44 and 45 as received.
    pub checksum_bytes: [u8; 2],
}

/// Decodes a 6-byte page reference: units, tens, S1, S2 with M1, S3, S4 with
/// M2 and M3 (Figure 12). Returns page, sub-code and the three magazine bits.
pub(super) fn page_reference(packet: &Packet, start: usize) -> Option<(PageNumber, SubCode, u8)> {
    let mut values = [0u8; 6];
    for (offset, value) in values.iter_mut().enumerate() {
        *value = nibble(packet, start.saturating_add(offset))?;
    }
    let [units, tens, s1, s2m1, s3, s4m23] = values;
    let page = PageNumber((tens << 4) | units);
    let subcode = SubCode::new(
        u16::from(s1) | (u16::from(s2m1 & 0x7) << 4) | (u16::from(s3) << 8) | (u16::from(s4m23 & 0x3) << 12),
    );
    // M1 is data bit 4 of the S2 byte; M2 and M3 are data bits 3 and 4 of
    // the S4 byte (Figure 12).
    let magazine_bits = (s2m1 >> 3) | ((s4m23 >> 1) & 0x6);
    Some((page, subcode, magazine_bits & MAGAZINE_MASK))
}

/// Index of link 0 (packet byte 7) and the bytes per link.
const LINK_START: usize = 3;
const LINK_BYTES: usize = 6;
/// Index of the link control byte (packet byte 43).
const LINK_CONTROL: usize = 39;
/// Data bit 4 of the link control byte: display row 24 (Table 14). Only
/// this bit is read; the reserved bits beside it are ignored (REQ-PKT-009).
const LINK_CONTROL_ROW_24: u8 = 0x8;
/// Index of the first checksum byte (packet byte 44).
const LINK_CHECKSUM: usize = 40;

/// Decodes packet X/27/0 (REQ-PKT-005).
pub(super) fn decode_links(packet: &Packet, magazine: Magazine) -> Links {
    let mut links = [None; 6];
    for (index, slot) in links.iter_mut().enumerate() {
        let start = LINK_START.saturating_add(index.saturating_mul(LINK_BYTES));
        *slot = page_reference(packet, start).and_then(|(page, subcode, relative)| {
            if page.0 == LINK_NULL_PAGE && subcode.value() == LINK_NULL_SUBCODE {
                return None;
            }
            // Clause 9.6.1: set bits complement the packet's magazine bits.
            let linked = Magazine::from_bits(magazine.bits() ^ relative);
            Some(Link { magazine: linked, page, subcode })
        });
    }
    let display_row_24 = nibble(packet, LINK_CONTROL).is_some_and(|control| control & LINK_CONTROL_ROW_24 != 0);
    let checksum_bytes = [packet.byte(LINK_CHECKSUM), packet.byte(LINK_CHECKSUM.saturating_add(1))];
    Links { magazine, links, display_row_24, checksum_bytes }
}

/// The encoder of X/27/0: the inverse of [`decode_links`].
pub(super) mod encode {
    use super::super::encode::{address, set};
    use super::{LINK_BYTES, LINK_CHECKSUM, LINK_CONTROL, LINK_CONTROL_ROW_24, LINK_START, Packet};
    use crate::consts::DATA_START;
    use crate::ecc::hamming84_encode;
    use crate::types::{Magazine, PageNumber, SubCode};

    /// Writes a 6-byte page reference with three magazine bits.
    fn page_reference(packet: &mut Packet, start: usize, page: PageNumber, subcode: SubCode, magazine_bits: u8) {
        let sub = subcode.value();
        let values = [
            page.units(),
            page.tens(),
            (sub & 0xF) as u8,
            (((sub >> 4) & 0x7) as u8) | ((magazine_bits & 1) << 3),
            ((sub >> 8) & 0xF) as u8,
            (((sub >> 12) & 0x3) as u8) | ((magazine_bits & 0x6) << 1),
        ];
        for (offset, value) in values.iter().enumerate() {
            set(packet, start.saturating_add(offset), hamming84_encode(*value));
        }
    }

    /// Packet X/27/0. Each link is `(magazine, page, subcode)` or `None` for
    /// a null link.
    #[must_use]
    pub fn links(
        magazine: Magazine,
        links: &[Option<(Magazine, PageNumber, SubCode)>; 6],
        display_row_24: bool,
        checksum: u16,
    ) -> Packet {
        let mut packet = address(magazine, crate::consts::PACKET_LINKS);
        set(&mut packet, DATA_START, hamming84_encode(0));
        for (index, link) in links.iter().enumerate() {
            let start = LINK_START.saturating_add(index.saturating_mul(LINK_BYTES));
            match link {
                Some((target, page, subcode)) => {
                    let relative = magazine.bits() ^ target.bits();
                    page_reference(&mut packet, start, *page, *subcode, relative);
                }
                None => page_reference(
                    &mut packet,
                    start,
                    PageNumber(crate::consts::LINK_NULL_PAGE),
                    SubCode::new(crate::consts::LINK_NULL_SUBCODE),
                    0,
                ),
            }
        }
        // Table 14: bits 1 to 3 should be 1 without a local code of practice.
        let control = 0x7 | if display_row_24 { LINK_CONTROL_ROW_24 } else { 0 };
        set(&mut packet, LINK_CONTROL, hamming84_encode(control));
        let [high, low] = checksum.to_be_bytes();
        set(&mut packet, LINK_CHECKSUM, high);
        set(&mut packet, LINK_CHECKSUM.saturating_add(1), low);
        packet
    }
}
