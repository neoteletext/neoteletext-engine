//! The Colour Map and the page-level presentation data of Levels 2.5 and
//! 3.5 (layer L3; decisions E.43 to E.45 and E.49; REQ-L25-030,
//! REQ-L25-031 and REQ-L25-040).
//!
//! Every colour of the grid is a Colour Map index, 0 to 31, equal to
//! 8 × CLUT + entry; the Level 1 colours are CLUT 0, so their numbers keep
//! their meaning. [`resolve`] builds the page's [`ColourMap`] and the other
//! page-level settings from X/28/0 format 1, X/28/4, M/29/0, M/29/4 and the
//! fallback of the Magazine Organization Table, in the order of decision
//! E.44. The result is data: the enhancement processor applies it, and the
//! Colour Map travels with the grid to the renderer.

use crate::consts::{
    CLUT_ENTRIES, CLUT_ENTRY_MASK, CLUT_SHIFT, COLOUR_MAP_DEFAULTS, COLOUR_MAP_ENTRIES, COLOUR_TRANSPARENT,
    PRIMARY_4_TO_8, REMAPPING_CLUTS, TRIPLET_COLOUR_INDEX_MASK,
};
use crate::types::{ColourWords, Rgb12};

mod presentation;
mod side_panels;

pub use presentation::{PagePresentation, PresentationSources, colour_words, resolve};
pub use side_panels::SidePanels;

/// A Colour Map index, 0 to 31: 8 × CLUT + entry (decision E.43).
pub type ColourIndex = u8;

/// The Colour Map index of an entry of a CLUT.
#[must_use]
pub const fn colour_index(clut: u8, entry: u8) -> ColourIndex {
    (clut << CLUT_SHIFT | entry & CLUT_ENTRY_MASK) & TRIPLET_COLOUR_INDEX_MASK
}

/// The 32 entries of 12-bit colour (Table 30).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct ColourMap {
    /// The entries, by Colour Map index.
    pub entries: [Rgb12; COLOUR_MAP_ENTRIES],
}

impl ColourMap {
    /// The defaults of Table 30.
    // Evaluated at compile time over two arrays of the same constant
    // length: an index out of range would fail the build, not panic.
    #[allow(clippy::indexing_slicing)]
    pub const DEFAULT: Self = {
        let mut entries = [Rgb12(0); COLOUR_MAP_ENTRIES];
        let mut index = 0;
        while index < COLOUR_MAP_ENTRIES {
            entries[index] = Rgb12(COLOUR_MAP_DEFAULTS[index]);
            index += 1;
        }
        Self { entries }
    };

    /// The colour of an index; indices above 31 use their low five bits.
    #[must_use]
    pub fn entry(&self, index: ColourIndex) -> Rgb12 {
        self.entries.get(usize::from(index & TRIPLET_COLOUR_INDEX_MASK)).copied().unwrap_or_default()
    }

    /// True for CLUT 1 entry 0, which is transparent at every level.
    #[must_use]
    pub const fn is_transparent(index: ColourIndex) -> bool {
        index == COLOUR_TRANSPARENT
    }

    /// The 8-bit red, green and blue of an index: each 4-bit primary times
    /// 17 (REQ-RND-030). Transparent gives its stored value; callers draw
    /// it as the picture.
    #[must_use]
    pub fn rgb888(&self, index: ColourIndex) -> [u8; 3] {
        let colour = self.entry(index);
        [colour.red(), colour.green(), colour.blue()].map(|primary| primary.saturating_mul(PRIMARY_4_TO_8))
    }

    /// Replaces two CLUTs, `first_clut` and the next, with sixteen colour
    /// words in transmission order. CLUT 1 entry 0 always stays transparent:
    /// its word is ignored (Table 10, REQ-PKT-015).
    pub fn set_cluts(&mut self, first_clut: u8, words: &ColourWords) {
        let start = usize::from(first_clut).saturating_mul(CLUT_ENTRIES);
        for (offset, word) in words.iter().enumerate() {
            let index = start.saturating_add(offset);
            if index == usize::from(COLOUR_TRANSPARENT) {
                continue;
            }
            if let Some(entry) = self.entries.get_mut(index) {
                *entry = *word;
            }
        }
    }
}

impl Default for ColourMap {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Colour Table Re-mapping (Table 4, triplet 13 bits 16 to 18): the CLUTs
/// that the Level 1 spacing colour attributes, and the start-of-row default
/// colours, select from (decision E.43, REQ-L25-022).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Remapping {
    /// The CLUT of foreground colours.
    pub foreground_clut: u8,
    /// The CLUT of background colours.
    pub background_clut: u8,
}

impl Remapping {
    /// No re-mapping: CLUT 0 for both.
    pub const NONE: Self = Self { foreground_clut: 0, background_clut: 0 };

    /// The CLUTs of a 3-bit re-mapping value.
    #[must_use]
    pub fn from_value(value: u8) -> Self {
        let [foreground_clut, background_clut] =
            REMAPPING_CLUTS.get(usize::from(value & CLUT_ENTRY_MASK)).copied().unwrap_or_default();
        Self { foreground_clut, background_clut }
    }

    /// The Colour Map index of a Level 1 foreground colour, entry 0 to 7.
    #[must_use]
    pub const fn foreground(self, entry: u8) -> ColourIndex {
        colour_index(self.foreground_clut, entry)
    }

    /// The Colour Map index of a Level 1 background colour, entry 0 to 7.
    #[must_use]
    pub const fn background(self, entry: u8) -> ColourIndex {
        colour_index(self.background_clut, entry)
    }
}

#[cfg(test)]
mod tests;
