//! The page header, packet X/0 (clause 9.3.1, REQ-PKT-002): its Hamming 8/4
//! bytes, decoded and encoded.

use super::{Packet, nibble};
use crate::consts::{HEADER_DISPLAY_BYTES, HEADER_DISPLAY_START};
use crate::limits::sub_code;
use crate::types::{ControlBits, Magazine, PageNumber, SubCode};

/// A decoded page header (REQ-PKT-002).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Header {
    /// The magazine.
    pub magazine: Magazine,
    /// The page number, `None` when either digit was uncorrectable.
    pub page: Option<PageNumber>,
    /// The sub-code, `None` when any of its bytes was uncorrectable.
    pub subcode: Option<SubCode>,
    /// Control bits C4 to C14, `None` when any carrying byte was
    /// uncorrectable.
    pub control: Option<ControlBits>,
    /// Packet bytes 14 to 45, raw with parity.
    pub display: [u8; HEADER_DISPLAY_BYTES],
}

impl Header {
    /// True when page number, sub-code and control bits are all valid, so
    /// the header may start a page (decision E.7).
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.page.is_some() && self.subcode.is_some() && self.control.is_some()
    }

    /// True when the header is in serial mode (C11), if known.
    #[must_use]
    pub fn serial(&self) -> Option<bool> {
        self.control.map(|control| control.has(crate::consts::C11_SERIAL))
    }
}

/// Indices of the header's Hamming bytes, packet bytes 6 to 13.
pub(super) const HEADER_UNITS: usize = 2;
const HEADER_TENS: usize = 3;
const HEADER_S1: usize = 4;
const HEADER_S2_C4: usize = 5;
const HEADER_S3: usize = 6;
const HEADER_S4_C5_C6: usize = 7;
const HEADER_C7_C10: usize = 8;
const HEADER_C11_C14: usize = 9;

/// Decodes a page header (clause 9.3.1, REQ-PKT-002).
pub(super) fn decode_header(packet: &Packet, magazine: Magazine) -> Header {
    let units = nibble(packet, HEADER_UNITS);
    let tens = nibble(packet, HEADER_TENS);
    let page = match (units, tens) {
        (Some(units), Some(tens)) => Some(PageNumber((tens << 4) | units)),
        _ => None,
    };
    let s1 = nibble(packet, HEADER_S1);
    let s2c4 = nibble(packet, HEADER_S2_C4);
    let s3 = nibble(packet, HEADER_S3);
    let s4c5c6 = nibble(packet, HEADER_S4_C5_C6);
    let c7c10 = nibble(packet, HEADER_C7_C10);
    let c11c14 = nibble(packet, HEADER_C11_C14);
    // REQ-NUM-005: the bits beside S2 and S4 are C4, C5 and C6, which
    // `sub_code` masks away; they are never part of the sub-code.
    let subcode = match (s1, s2c4, s3, s4c5c6) {
        (Some(s1), Some(s2), Some(s3), Some(s4)) => Some(SubCode::new(sub_code(s4, s3, s2, s1))),
        _ => None,
    };
    let control = match (s2c4, s4c5c6, c7c10, c11c14) {
        (Some(s2c4), Some(s4c5c6), Some(c7c10), Some(c11c14)) => {
            // Clause 9.3.1.3: C4 is data bit 4 of byte 9; C5 and C6 bits 3
            // and 4 of byte 11; C7 to C10 byte 12; C11 to C14 byte 13. Bit n
            // of the control word is Cn.
            Some(ControlBits(
                (u16::from(s2c4 >> 3) << 4)
                    | (u16::from(s4c5c6 >> 2) << 5)
                    | (u16::from(c7c10) << 7)
                    | (u16::from(c11c14) << 11),
            ))
        }
        _ => None,
    };
    let mut display = [0; HEADER_DISPLAY_BYTES];
    if let Some(source) = packet.bytes.get(HEADER_DISPLAY_START..) {
        display.copy_from_slice(source);
    }
    Header { magazine, page, subcode, control, display }
}

/// The encoder of the header: the inverse of [`decode_header`].
pub(super) mod encode {
    use super::super::encode::{address, set};
    use super::{
        HEADER_C7_C10, HEADER_C11_C14, HEADER_S1, HEADER_S2_C4, HEADER_S3, HEADER_S4_C5_C6, HEADER_TENS, HEADER_UNITS,
        Packet,
    };
    use crate::consts::{HEADER_DISPLAY_BYTES, HEADER_DISPLAY_START};
    use crate::ecc::hamming84_encode;
    use crate::limits::{NULL_ADDRESS, S2_MASK, S4_MASK, s1, s2, s3, s4};
    use crate::types::{ControlBits, Magazine, PageNumber, SubCode};

    /// A page header. `display` is 32 bytes already carrying parity.
    ///
    /// `None` for the null page address XFF:3F7F, which shall never be
    /// sent as a header (clause 7.2.2; Annex A.1, rule 3; REQ-NUM-007).
    /// Every other address is written as given (REQ-NUM-002).
    #[must_use]
    pub fn header(
        magazine: Magazine,
        page: PageNumber,
        subcode: SubCode,
        control: ControlBits,
        display: &[u8; HEADER_DISPLAY_BYTES],
    ) -> Option<Packet> {
        if (page.0, subcode.value()) == NULL_ADDRESS {
            return None;
        }
        let mut packet = address(magazine, 0);
        let sub = subcode.value();
        let c = control.0;
        // C4 sits above S2's three bits, and C5 and C6 above S4's two
        // (clause 9.3.1.3).
        let c4_shift = S2_MASK.count_ones();
        let c5_shift = S4_MASK.count_ones();
        let nibbles = [
            (HEADER_UNITS, page.units()),
            (HEADER_TENS, page.tens()),
            (HEADER_S1, s1(sub)),
            (HEADER_S2_C4, s2(sub) | ((((c >> 4) & 1) as u8) << c4_shift)),
            (HEADER_S3, s3(sub)),
            (HEADER_S4_C5_C6, s4(sub) | ((((c >> 5) & 0x3) as u8) << c5_shift)),
            (HEADER_C7_C10, ((c >> 7) & 0xF) as u8),
            (HEADER_C11_C14, ((c >> 11) & 0xF) as u8),
        ];
        for (index, value) in nibbles {
            set(&mut packet, index, hamming84_encode(value));
        }
        if let Some(target) = packet.bytes.get_mut(HEADER_DISPLAY_START..) {
            target.copy_from_slice(display);
        }
        Some(packet)
    }
}
