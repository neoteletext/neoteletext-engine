//! Real colours for the core's indices: the 144-entry palette of the
//! palette mode (REQ-RND-005, REQ-RND-023) and the 32 entries of a page's
//! Colour Map in the resolved mode (REQ-RND-030).

use neoteletext_core::colour::ColourMap;
use neoteletext_core::consts::{
    COLOUR_BLUE_BIT, COLOUR_GREEN_BIT, COLOUR_MAP_ENTRIES, COLOUR_RED_BIT, COLOURS, PALETTE_SIZE, PRIMARY_FULL,
};
use neoteletext_core::render::fill_palette;

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

/// The colours of a Colour Map, for the resolved mode (REQ-RND-030): each
/// 4-bit primary times 17, and index 8, CLUT 1 entry 0, the picture:
/// transparent by default, or a fill colour for formats without alpha
/// (REQ-RND-006).
#[must_use]
pub fn colour_map_palette(map: &ColourMap, picture: Rgba) -> [Rgba; COLOUR_MAP_ENTRIES] {
    core::array::from_fn(|index| {
        let index = index as u8;
        if ColourMap::is_transparent(index) {
            return picture;
        }
        let [r, g, b] = map.rgb888(index);
        Rgba::opaque(r, g, b)
    })
}

/// The eight colours of CLUT 0 of a Colour Map: the base colours of the
/// palette mode, which equal [`CLUT0`] unless the user replaced them
/// (REQ-RND-005).
#[must_use]
pub fn clut0_of(map: &ColourMap) -> [Rgba; COLOURS] {
    core::array::from_fn(|index| {
        let [r, g, b] = map.rgb888(index as u8);
        Rgba::opaque(r, g, b)
    })
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use neoteletext_core::colour::colour_index;
    use neoteletext_core::types::{ColourWords, Rgb12};

    /// REQ-RND-030: every index converts as Table 30 gives, index 8 is the
    /// picture, and redefined CLUTs convert with their own values.
    #[test]
    fn colour_map_palette_follows_table_30() {
        let fill = Rgba::opaque(1, 2, 3);
        let palette = colour_map_palette(&ColourMap::DEFAULT, fill);
        for (index, colour) in palette.iter().enumerate() {
            let entry = ColourMap::DEFAULT.entry(index as u8);
            let expected =
                if index == 8 { fill } else { Rgba::opaque(entry.red() * 17, entry.green() * 17, entry.blue() * 17) };
            assert_eq!(*colour, expected, "index {index}");
        }
        assert_eq!(palette[usize::from(colour_index(1, 5))], Rgba::opaque(119, 0, 119), "half magenta");
        assert_eq!(&palette[..8], &CLUT0[..]);
        assert_eq!(clut0_of(&ColourMap::DEFAULT), CLUT0);
        let mut map = ColourMap::DEFAULT;
        let words: ColourWords = core::array::from_fn(|index| Rgb12::new(15, index as u8, 0));
        map.set_cluts(0, &words);
        map.set_cluts(2, &words);
        let palette = colour_map_palette(&map, Rgba::TRANSPARENT);
        assert_eq!(palette[3], Rgba::opaque(255, 51, 0));
        assert_eq!(palette[8], Rgba::TRANSPARENT, "CLUT 1 entry 0 stays the picture");
        assert_eq!(palette[16 + 15], Rgba::opaque(255, 255, 0));
    }

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
