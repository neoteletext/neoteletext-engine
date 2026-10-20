//! The join address as a QR code (ISO/IEC 18004), made by Project
//! Nayuki's `qrcodegen`, and as teletext mosaics: a page can show it, so a
//! channel can be the screen a phone types into, on any receiver.

use qrcodegen::{QrCode, QrCodeEcc};

use crate::Error;

/// The quiet zone ISO/IEC 18004 asks for round a code: four modules.
pub const QUIET_ZONE: usize = 4;
/// A mosaic character's sixels: two across, three down (EN 300 706 Table
/// 47, the G1 block mosaics).
const SIXELS_ACROSS: usize = 2;
const SIXELS_DOWN: usize = 3;
/// The G1 mosaic codes: 0x20 plus a bit for each lit sixel, left to right
/// and top to bottom, with bit 5 (0x20) left out, as Table 47 lays out the
/// codes 0x20 to 0x3F and 0x60 to 0x7F.
const MOSAIC_BASE: u8 = 0x20;
const SIXEL_BITS: [[u8; SIXELS_ACROSS]; SIXELS_DOWN] = [[0x01, 0x02], [0x04, 0x08], [0x10, 0x40]];
/// Mosaic white (EN 300 706 Table 26, spacing attribute 0x17): the row's
/// first cell, so the cells after it are mosaics, lit in white.
pub const MOSAIC_WHITE: u8 = 0x17;

/// How much the code may be damaged and still read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Correction {
    /// About 7 per cent: the smallest code, for mosaics.
    Low,
    /// About 15 per cent: what qrkeyboard's screen SDK draws.
    Medium,
}

/// A QR code's modules.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Matrix {
    /// Modules a side, without the quiet zone.
    pub size: usize,
    dark: Vec<bool>,
}

impl Matrix {
    /// The code for `text`.
    ///
    /// # Errors
    /// Text too long for any QR version.
    pub fn new(text: &str, correction: Correction) -> Result<Self, Error> {
        let ecc = match correction {
            Correction::Low => QrCodeEcc::Low,
            Correction::Medium => QrCodeEcc::Medium,
        };
        let code = QrCode::encode_text(text, ecc).map_err(|_| Error::Limit("text too long for a QR code"))?;
        let size = usize::try_from(code.size()).map_err(|_| Error::Limit("a QR code's size"))?;
        let dark = (0..code.size())
            .flat_map(|y| (0..code.size()).map(move |x| (x, y)))
            .map(|(x, y)| code.get_module(x, y))
            .collect();
        Ok(Self { size, dark })
    }

    /// Whether the module at `x`, `y` is dark; outside the code, and so in
    /// the quiet zone, it is light.
    #[must_use]
    pub fn dark(&self, x: usize, y: usize) -> bool {
        x < self.size
            && y < self.size
            && self.dark.get(y.saturating_mul(self.size).saturating_add(x)).copied().unwrap_or(false)
    }

    /// Modules a side with the quiet zone.
    #[must_use]
    pub fn with_quiet_zone(&self) -> usize {
        self.size.saturating_add(2 * QUIET_ZONE)
    }

    /// Whether the module at `x`, `y` of the code with its quiet zone is
    /// dark.
    #[must_use]
    pub fn dark_with_quiet_zone(&self, x: usize, y: usize) -> bool {
        match (x.checked_sub(QUIET_ZONE), y.checked_sub(QUIET_ZONE)) {
            (Some(x), Some(y)) => self.dark(x, y),
            _ => false,
        }
    }

    /// The code as rows of teletext character codes: each row the mosaic
    /// white attribute, then mosaic characters, one sixel a module, the
    /// light modules lit and the dark ones not, so the code is dark on
    /// white with its quiet zone, as a phone's camera reads it. A sixel is
    /// about twice as wide as it is tall; cameras read the stretched code.
    #[must_use]
    pub fn mosaic_rows(&self) -> Vec<Vec<u8>> {
        let side = self.with_quiet_zone();
        let columns = side.div_ceil(SIXELS_ACROSS);
        let rows = side.div_ceil(SIXELS_DOWN);
        (0..rows)
            .map(|row| {
                let mut line = vec![MOSAIC_WHITE];
                for column in 0..columns {
                    let mut code = MOSAIC_BASE;
                    for (down, bits) in SIXEL_BITS.iter().enumerate() {
                        for (across, bit) in bits.iter().enumerate() {
                            let x = column.saturating_mul(SIXELS_ACROSS).saturating_add(across);
                            let y = row.saturating_mul(SIXELS_DOWN).saturating_add(down);
                            // Beyond the quiet zone's edge is page, not code: unlit.
                            if x < side && y < side && !self.dark_with_quiet_zone(x, y) {
                                code |= bit;
                            }
                        }
                    }
                    line.push(code);
                }
                line
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// A join address of qrkeyboard's length: the relay, a 16-symbol code,
    /// a 43-symbol key and a 22-symbol hash.
    const JOIN: &str = "https://qrkeyboard.com/app#c=ABCDEFGHIJKLMNOP&k=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&h=AAAAAAAAAAAAAAAAAAAAAA";

    #[test]
    fn a_join_address_fits_a_page_as_mosaics() {
        let matrix = Matrix::new(JOIN, Correction::Low).unwrap();
        let rows = matrix.mosaic_rows();
        // Within the page's text rows and columns, beside a line of text.
        assert!(rows.len() <= 23 - 2, "{} rows", rows.len());
        assert!(rows[0].len() <= 40, "{} columns", rows[0].len());
        assert!(rows.iter().flatten().skip(1).all(|&code| code == MOSAIC_WHITE || (0x20..0x80).contains(&code)));
        // The top-left finder pattern's dark corner, after the quiet zone.
        assert!(matrix.dark(0, 0) && !matrix.dark_with_quiet_zone(0, 0));
    }

    #[test]
    fn the_quiet_zone_is_lit() {
        let matrix = Matrix::new("x", Correction::Medium).unwrap();
        let rows = matrix.mosaic_rows();
        // The first row of cells holds three rows of quiet zone: all lit,
        // but for a last cell that runs past an odd width.
        let side = matrix.with_quiet_zone();
        let whole = side / 2;
        assert!(rows[0][1..=whole].iter().all(|&code| code == 0x7F), "{:?}", rows[0]);
        if side % 2 == 1 {
            assert_eq!(rows[0][whole + 1], 0x20 | 0x01 | 0x04 | 0x10, "only its left sixels");
        }
    }
}
