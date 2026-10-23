//! PSI section reassembly (ISO/IEC 13818-1 clause 2.4.4), the continuity
//! counter check (clause 2.4.3.3) and the section CRC (Annex A).

use crate::consts::{
    CRC32_INITIAL, CRC32_POLYNOMIAL, SECTION_HEADER_BYTES, SECTION_LENGTH_MASK, SECTION_MAX_BYTES, SECTION_STUFFING,
    TS_CONTINUITY_MASK,
};

use super::Section;

/// The most significant bit of the CRC register.
const CRC_TOP_BIT: u32 = 1 << 31;
/// `section_syntax_indicator`, bit 7 of the second section byte.
pub(super) const SECTION_SYNTAX: u8 = 0x80;
/// `current_next_indicator`, bit 0 of the version byte.
pub(super) const SECTION_CURRENT: u8 = 0x01;

impl Section {
    pub(super) const fn new() -> Self {
        Self { data: [0; SECTION_MAX_BYTES], len: 0, active: false, continuity: None }
    }

    fn total(&self) -> Option<usize> {
        let high = self.data.get(1).copied()?;
        let low = self.data.get(2).copied()?;
        // REQ-GEN-007: MPEG-2 fields are big-endian whatever the host.
        let length = u16::from_be_bytes([high, low]) & SECTION_LENGTH_MASK;
        Some(SECTION_HEADER_BYTES.saturating_add(usize::from(length)))
    }

    /// Adds one payload byte.
    pub(super) fn push(&mut self, byte: u8) -> Push {
        if !self.active {
            if byte == SECTION_STUFFING {
                return Push::Stuffing;
            }
            self.active = true;
            self.len = 0;
        }
        if let Some(slot) = self.data.get_mut(self.len) {
            *slot = byte;
            self.len = self.len.saturating_add(1);
        }
        if self.len >= SECTION_HEADER_BYTES {
            let total = self.total().unwrap_or(0);
            if total > SECTION_MAX_BYTES {
                self.active = false;
                return Push::TooLong;
            }
            if self.len == total {
                self.active = false;
                return Push::Complete(total);
            }
        }
        Push::More
    }
}

/// What adding a byte to a section did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Push {
    /// The section needs more bytes.
    More,
    /// The byte is stuffing after the last section of the packet.
    Stuffing,
    /// The section claims to be longer than any section can be.
    TooLong,
    /// The section is complete, with this many bytes.
    Complete(usize),
}

/// The outcome of a continuity counter check.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Continuity {
    Follows,
    Duplicate,
    Broken,
}

/// Checks a continuity counter (ISO/IEC 13818-1 clause 2.4.3.3): it
/// increments only with a payload, and one duplicate packet is allowed.
pub(super) fn check_continuity(
    last: &mut Option<u8>,
    counter: u8,
    has_payload: bool,
    discontinuity: bool,
) -> Continuity {
    if !has_payload {
        return Continuity::Follows;
    }
    let result = match *last {
        Some(previous) if !discontinuity && counter == previous => Continuity::Duplicate,
        Some(previous) if !discontinuity && counter != (previous.wrapping_add(1) & TS_CONTINUITY_MASK) => {
            Continuity::Broken
        }
        _ => Continuity::Follows,
    };
    *last = Some(counter);
    result
}

/// The MPEG-2 `CRC_32` (ISO/IEC 13818-1 Annex A); a section with its CRC
/// appended gives 0.
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = CRC32_INITIAL;
    for &byte in data {
        crc ^= u32::from(byte) << 24;
        for _ in 0..8 {
            crc = if crc & CRC_TOP_BIT != 0 { (crc << 1) ^ CRC32_POLYNOMIAL } else { crc << 1 };
        }
    }
    crc
}
