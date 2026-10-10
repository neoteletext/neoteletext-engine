//! DRCS downloading pages (clause 14, Figure 27, Table 31; REQ-INP-025):
//! characters of every mode drawn as text patterns, with the mode
//! identification of X/28/3 computed from them.

use std::collections::BTreeMap;
use std::vec::Vec;

use tellytxt_core::consts::{
    DATA_BYTES, DESIGNATION_DRCS_MODES, DRCS_PATTERN_DOTS, DRCS_PATTERN_ROWS, DRCS_PLANES, DRCS_PTUS,
    PACKET_PAGE_ENHANCEMENT, PAGE_CODING_PARITY, PAGE_FUNCTION_DRCS, PAGE_FUNCTION_GDRCS, SPACE,
};
use tellytxt_core::drcs::DrcsMode;
use tellytxt_core::packet::{DrcsModes, PageFormat, encode};
use tellytxt_core::types::{DrcsTable, PageAddress, SubCode};

use super::{AuthorError, Result, enhancement_subcode, new_page, raw_line};
use crate::consts::{
    DRCS_BYTE_BASE, DRCS_DOTS_PER_BYTE, DRCS_MODE_NO_DATA, DRCS_MODE_SUBSEQUENT, DRCS_MODE1_PTUS, DRCS_MODE2_PTUS,
    DRCS_MODE3_COLUMNS, DRCS_MODE3_ROWS, DRCS_PTU_BYTES, DRCS_PTUS_PER_PACKET,
};
use crate::page::{PageFile, PageFunction};

/// The highest S1 of a sub-page (Annex A.1).
const SUBPAGE_LAST: u8 = 0xF;
/// Planes, PTUs and mode identification code of each mode (Table 31).
const fn mode_shape(mode: DrcsMode) -> (usize, u8) {
    match mode {
        DrcsMode::Mode0 => (1, 0b0000),
        DrcsMode::Mode1 => (DRCS_MODE1_PTUS, 0b0001),
        DrcsMode::Mode2 => (DRCS_MODE2_PTUS, 0b0010),
        DrcsMode::Mode3 => (1, 0b0011),
    }
}

/// The highest pixel value of a mode: 1, 3 or 15.
const fn mode_maximum(mode: DrcsMode) -> u8 {
    match mode {
        DrcsMode::Mode0 => 1,
        DrcsMode::Mode1 => 3,
        DrcsMode::Mode2 | DrcsMode::Mode3 => 15,
    }
}

/// A DRCS character as a DRCS Mode and a DRCS Character Invocation name
/// it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct DrcsRef {
    /// The global or normal table.
    pub table: DrcsTable,
    /// The sub-table, the S1 of the DRCS sub-page.
    pub subtable: u8,
    /// The character: the address of its first PTU, 0 to 47.
    pub character: u8,
}

/// One DRCS sub-page: the D-bytes of each PTU and its mode code.
#[derive(Clone, Debug)]
struct SubPage {
    ptus: [Option<[u8; DRCS_PTU_BYTES]>; DRCS_PTUS],
    modes: [u8; DRCS_PTUS],
}

impl Default for SubPage {
    fn default() -> Self {
        Self { ptus: [None; DRCS_PTUS], modes: [DRCS_MODE_NO_DATA; DRCS_PTUS] }
    }
}

/// A normal or global DRCS downloading page.
#[derive(Clone, Debug)]
pub struct DrcsPage {
    address: PageAddress,
    global: bool,
    update: u8,
    mode_packet: bool,
    subpages: BTreeMap<u8, SubPage>,
}

/// The value of one pattern character: `.` and space are 0, `#` is the
/// highest value of the mode, and a hexadecimal digit is itself.
fn pixel(character: u8, maximum: u8) -> Result<u8> {
    let value = match character {
        b'.' | b' ' => 0,
        b'#' => maximum,
        other => {
            char::from(other).to_digit(16).and_then(|digit| u8::try_from(digit).ok()).ok_or(AuthorError::BadPattern)?
        }
    };
    if value > maximum { Err(AuthorError::BadPattern) } else { Ok(value) }
}

/// Parses a pattern of `rows` lines of `columns` characters.
fn parse(pattern: &[&str], rows: usize, columns: usize, maximum: u8) -> Result<Vec<Vec<u8>>> {
    if pattern.len() != rows {
        return Err(AuthorError::BadPattern);
    }
    pattern
        .iter()
        .map(|line| {
            if line.len() != columns {
                return Err(AuthorError::BadPattern);
            }
            line.bytes().map(|character| pixel(character, maximum)).collect()
        })
        .collect()
}

/// Six dots of plane `plane` as a D-byte: the leftmost dot is the most
/// significant of the six bits (clause 14.2.1).
fn d_byte(dots: &[u8], plane: usize) -> u8 {
    dots.iter().take(DRCS_DOTS_PER_BYTE).enumerate().fold(DRCS_BYTE_BASE, |byte, (x, &value)| {
        let bit = (value >> plane) & 1;
        byte | bit << (DRCS_DOTS_PER_BYTE.saturating_sub(1).saturating_sub(x))
    })
}

/// The PTU of plane `plane` of a 12 by 10 pattern: D-byte 2r + h holds
/// dots 6h to 6h + 5 of row r (clause 14.2.1).
fn plane_ptu(rows: &[Vec<u8>], plane: usize) -> [u8; DRCS_PTU_BYTES] {
    let mut ptu = [DRCS_BYTE_BASE; DRCS_PTU_BYTES];
    let halves = rows.iter().flat_map(|row| row.chunks(DRCS_DOTS_PER_BYTE));
    for (byte, dots) in ptu.iter_mut().zip(halves) {
        *byte = d_byte(dots, plane);
    }
    ptu
}

/// The PTU of a 6 by 5 by 4 pattern: D-byte 4r + k is plane k of row r
/// (clause 14.2.4).
fn mode3_ptu(rows: &[Vec<u8>]) -> [u8; DRCS_PTU_BYTES] {
    let mut ptu = [DRCS_BYTE_BASE; DRCS_PTU_BYTES];
    let bytes = rows.iter().flat_map(|row| (0..DRCS_PLANES).map(move |plane| d_byte(row, plane)));
    for (slot, byte) in ptu.iter_mut().zip(bytes) {
        *slot = byte;
    }
    ptu
}

/// The PTUs of a character in a mode from its pattern: 10 lines of 12
/// characters, or 5 lines of 6 for mode 3. Values are `.` (0), `#` (the
/// highest value) or hexadecimal digits up to 1, 3 or 15.
pub fn character_ptus(mode: DrcsMode, pattern: &[&str]) -> Result<Vec<[u8; DRCS_PTU_BYTES]>> {
    let maximum = mode_maximum(mode);
    if mode == DrcsMode::Mode3 {
        let rows = parse(pattern, DRCS_MODE3_ROWS, DRCS_MODE3_COLUMNS, maximum)?;
        return Ok(std::vec![mode3_ptu(&rows)]);
    }
    let rows = parse(pattern, DRCS_PATTERN_ROWS, DRCS_PATTERN_DOTS, maximum)?;
    let (planes, _) = mode_shape(mode);
    Ok((0..planes).map(|plane| plane_ptu(&rows, plane)).collect())
}

impl DrcsPage {
    /// An empty DRCS page: global (GDRCS) when `global`, otherwise normal.
    #[must_use]
    pub const fn new(address: PageAddress, global: bool) -> Self {
        Self { address, global, update: 0, mode_packet: true, subpages: BTreeMap::new() }
    }

    /// The page's address.
    #[must_use]
    pub const fn address(&self) -> PageAddress {
        self.address
    }

    /// Sets the update indicator S2 of every sub-page (Annex A.1).
    pub fn update_indicator(&mut self, update: u8) -> &mut Self {
        self.update = update;
        self
    }

    /// Leaves out packet X/28/3, which a page of mode 0 characters only
    /// may do (clause 14.2); a decoder then takes every PTU as mode 0.
    pub fn without_mode_packet(&mut self) -> &mut Self {
        self.mode_packet = false;
        self
    }

    /// Defines a character of a mode at `character` (its first PTU) on
    /// sub-page `s1`, from a text pattern ([`character_ptus`]).
    pub fn character(&mut self, s1: u8, character: u8, mode: DrcsMode, pattern: &[&str]) -> Result<DrcsRef> {
        let ptus = character_ptus(mode, pattern)?;
        let (_, code) = mode_shape(mode);
        let codes: Vec<u8> =
            (0..ptus.len()).map(|index| if index == 0 { code } else { DRCS_MODE_SUBSEQUENT }).collect();
        self.place(s1, character, &ptus, &codes)?;
        let table = if self.global { DrcsTable::Global } else { DrcsTable::Normal };
        Ok(DrcsRef { table, subtable: s1, character })
    }

    /// Writes a PTU as given, with any mode code, for invalid and partial
    /// data (decision E.54).
    pub fn raw_ptu(&mut self, s1: u8, ptu: u8, bytes: [u8; DRCS_PTU_BYTES], mode_code: u8) -> Result<&mut Self> {
        self.place(s1, ptu, &[bytes], &[mode_code])?;
        Ok(self)
    }

    fn place(&mut self, s1: u8, first: u8, ptus: &[[u8; DRCS_PTU_BYTES]], codes: &[u8]) -> Result<()> {
        if s1 > SUBPAGE_LAST {
            return Err(AuthorError::BadSubPage);
        }
        let sub = self.subpages.entry(s1).or_default();
        let start = usize::from(first);
        let end = start.saturating_add(ptus.len());
        if end > DRCS_PTUS || sub.ptus.get(start..end).is_none_or(|slots| slots.iter().any(Option::is_some)) {
            return Err(AuthorError::BadPtu);
        }
        for (offset, (ptu, &code)) in ptus.iter().zip(codes).enumerate() {
            let index = start.saturating_add(offset);
            if let (Some(slot), Some(mode)) = (sub.ptus.get_mut(index), sub.modes.get_mut(index)) {
                *slot = Some(*ptu);
                *mode = code;
            }
        }
        Ok(())
    }

    /// The page files, one per sub-page in ascending S1 (Figure 27).
    pub fn build(&self) -> Result<Vec<PageFile>> {
        let function = if self.global { PAGE_FUNCTION_GDRCS } else { PAGE_FUNCTION_DRCS };
        let format = PageFormat { function, coding: PAGE_CODING_PARITY };
        let mut pages = Vec::new();
        for (&s1, sub) in &self.subpages {
            let mut page = new_page(self.address, SubCode::new(0));
            page.function = Some(PageFunction { function, coding: PAGE_CODING_PARITY });
            let mut last_packet = 0u8;
            for (packet, pair) in (1u8..).zip(sub.ptus.chunks(DRCS_PTUS_PER_PACKET)) {
                if pair.iter().all(Option::is_none) {
                    continue;
                }
                // Clause 14.1: unused addresses may be padded with SPACE.
                let mut row = [SPACE; DATA_BYTES];
                for (half, ptu) in row.chunks_mut(DRCS_PTU_BYTES).zip(pair) {
                    if let Some(bytes) = ptu {
                        half.copy_from_slice(bytes);
                    }
                }
                page.set_row(usize::from(packet), &row);
                last_packet = packet;
            }
            if self.mode_packet {
                let modes = DrcsModes::from_codes(Some(format), &sub.modes);
                let packet = encode::drcs_modes(self.address.magazine, format, &modes);
                super::push_raw(&mut page, raw_line(&packet, PACKET_PAGE_ENHANCEMENT, DESIGNATION_DRCS_MODES))?;
            }
            page.subcode = enhancement_subcode(last_packet, self.update, s1);
            pages.push(page);
        }
        Ok(pages)
    }
}
