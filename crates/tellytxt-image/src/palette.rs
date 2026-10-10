//! Real colours for the core's palette indices (REQ-RND-005, REQ-RND-023).

use tellytxt_core::consts::{COLOUR_BLUE_BIT, COLOUR_GREEN_BIT, COLOUR_RED_BIT, COLOURS, PALETTE_SIZE, PRIMARY_FULL};
use tellytxt_core::render::fill_palette;

/// A colour with alpha; alpha 0 is fully transparent.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha.
    pub a: u8,
}

/// Fully opaque.
const OPAQUE: u8 = 0xFF;

impl Rgba {
    /// An opaque colour.
    #[must_use]
    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: OPAQUE }
    }

    /// Fully transparent.
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };
}

const fn primary(colour: u8, bit: u8) -> u8 {
    if colour & bit != 0 { PRIMARY_FULL } else { 0 }
}

const fn clut0(colour: u8) -> Rgba {
    Rgba::opaque(primary(colour, COLOUR_RED_BIT), primary(colour, COLOUR_GREEN_BIT), primary(colour, COLOUR_BLUE_BIT))
}

/// The eight Level 1 colours of colour lookup table 0 (Table 30), each
/// primary 0 or full intensity (REQ-RND-005).
pub const CLUT0: [Rgba; COLOURS] = [clut0(0), clut0(1), clut0(2), clut0(3), clut0(4), clut0(5), clut0(6), clut0(7)];

/// The base colours plus the colour used for "picture" areas: transparent
/// by default, or a fill colour for formats without alpha (REQ-RND-006).
#[must_use]
pub fn base_colours(colours: &[Rgba; COLOURS], picture: Rgba) -> [Rgba; COLOURS + 1] {
    let mut base = [picture; COLOURS + 1];
    for (slot, colour) in base.iter_mut().zip(colours.iter()) {
        *slot = *colour;
    }
    base
}

/// The full 144-entry palette for a flash phase and reveal setting.
#[must_use]
pub fn palette(colours: &[Rgba; COLOURS], picture: Rgba, flash_visible: bool, reveal: bool) -> [Rgba; PALETTE_SIZE] {
    let mut out = [Rgba::TRANSPARENT; PALETTE_SIZE];
    fill_palette(&mut out, &base_colours(colours, picture), flash_visible, reveal);
    out
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn clut0_primaries() {
        assert_eq!(CLUT0[0], Rgba::opaque(0, 0, 0));
        assert_eq!(CLUT0[1], Rgba::opaque(255, 0, 0));
        assert_eq!(CLUT0[6], Rgba::opaque(0, 255, 255));
        assert_eq!(CLUT0[7], Rgba::opaque(255, 255, 255));
        let full = palette(&CLUT0, Rgba::TRANSPARENT, true, false);
        assert_eq!(full[8], Rgba::TRANSPARENT);
    }
}
