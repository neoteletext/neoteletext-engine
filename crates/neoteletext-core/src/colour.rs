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

use crate::config::PresentationLevel;
use crate::consts::{
    CLUT_ENTRIES, CLUT_ENTRY_MASK, CLUT_LEVEL_25_FIRST, CLUT_LEVEL_35_FIRST, CLUT_SHIFT, COLOUR_BLACK,
    COLOUR_MAP_DEFAULTS, COLOUR_MAP_ENTRIES, COLOUR_TRANSPARENT, COLOUR_WORDS, COLUMNS, PRIMARY_4_TO_8,
    REMAPPING_CLUTS, SIDE_PANEL_AREA_COLUMNS, SIDE_PANEL_MAX_COLUMNS, TRIPLET_COLOUR_INDEX_MASK,
};
use crate::types::{ColourWords, FallbackFields, PresentationFields, Rgb12, SidePanelFields};

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

/// The widths of the side panels (decision E.49).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct SidePanels {
    /// Columns of the left panel, 0 to 16: columns 72 − left to 71 of the
    /// panel area.
    pub left: u8,
    /// Columns of the right panel, 0 to 16: columns 40 to 40 + right − 1.
    pub right: u8,
}

impl SidePanels {
    /// No side panels.
    pub const NONE: Self = Self { left: 0, right: 0 };

    /// The widths that the fields give, by the width table of decision
    /// E.49. At Level 2.5 panels are shown only when the Side Panel Status
    /// Flag is 1; at Level 3.5 whenever they are enabled; at Levels 1 and
    /// 1.5 never.
    #[must_use]
    pub const fn from_fields(fields: SidePanelFields, level: PresentationLevel) -> Self {
        let shown = match level {
            PresentationLevel::Level1 | PresentationLevel::Level15 => false,
            PresentationLevel::Level25 => fields.status_flag,
            PresentationLevel::Level35 => true,
        };
        if !shown {
            return Self::NONE;
        }
        let n = if fields.left_columns > SIDE_PANEL_MAX_COLUMNS { SIDE_PANEL_MAX_COLUMNS } else { fields.left_columns };
        let rest = SIDE_PANEL_MAX_COLUMNS.saturating_sub(n);
        match (fields.left, fields.right) {
            (false, false) => Self::NONE,
            (true, false) => Self { left: if n == 0 { SIDE_PANEL_MAX_COLUMNS } else { n }, right: 0 },
            (false, true) => Self { left: 0, right: if n == 0 { SIDE_PANEL_MAX_COLUMNS } else { rest } },
            (true, true) => Self { left: n, right: rest },
        }
    }

    /// The first panel-area column of the left panel: 72 − left width.
    #[must_use]
    pub const fn left_first_column(self) -> usize {
        (COLUMNS + SIDE_PANEL_AREA_COLUMNS).saturating_sub(self.left as usize)
    }

    /// The width of the display in columns: 40 plus both panels.
    #[must_use]
    pub const fn display_columns(self) -> usize {
        COLUMNS.saturating_add(self.left as usize).saturating_add(self.right as usize)
    }
}

/// The sources of page-level presentation data, in the order of decision
/// E.44. Each is the decoded packet's field groups, or `None` when the
/// packet is absent. The caller passes the page's X/28/0 only when it is
/// read as format 1 on a displayed page.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PresentationSources<'a> {
    /// The page's X/28/0 format 1.
    pub page: Option<&'a PresentationFields>,
    /// The page's X/28/4, used at Level 3.5 only.
    pub page_level_35: Option<&'a PresentationFields>,
    /// The magazine's M/29/0.
    pub magazine: Option<&'a PresentationFields>,
    /// The magazine's M/29/4, used at Level 3.5 only.
    pub magazine_level_35: Option<&'a PresentationFields>,
    /// The fallback bits of the page's POP link in the Magazine
    /// Organization Table (decision E.53).
    pub fallback: Option<FallbackFields>,
}

/// The page-level presentation data of a page at Level 2.5 or 3.5, as data
/// that the enhancement processor applies (REQ-L25-030, REQ-L25-031,
/// REQ-L25-040).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct PagePresentation {
    /// The page's Colour Map.
    pub colour_map: ColourMap,
    /// The default screen colour; a Full Screen Colour triplet replaces it
    /// (decision E.45).
    pub default_screen: ColourIndex,
    /// The default row colour of every row; Full Row Colour triplets
    /// replace it (decision E.45).
    pub default_row: ColourIndex,
    /// Black Background Colour Substitution.
    pub black_background_substitution: bool,
    /// Colour Table Re-mapping.
    pub remapping: Remapping,
    /// The side-panel widths.
    pub side_panels: SidePanels,
}

impl PagePresentation {
    /// The defaults of Annex D.2.1 over a base Colour Map: black screen and
    /// rows, no re-mapping, no substitution and no side panels.
    #[must_use]
    pub const fn defaults(colour_map: ColourMap) -> Self {
        Self {
            colour_map,
            default_screen: COLOUR_BLACK,
            default_row: COLOUR_BLACK,
            black_background_substitution: false,
            remapping: Remapping::NONE,
            side_panels: SidePanels::NONE,
        }
    }
}

/// Builds the page-level presentation data (decisions E.43, E.44 and
/// E.49). `base` is the Colour Map before page data: Table 30, or the
/// user's replacement colours (REQ-RND-030).
///
/// - CLUTs 2 and 3 come from the first of X/28/0 and M/29/0 whose colour
///   words are valid; CLUTs 0 and 1, at Level 3.5 only, from the first of
///   X/28/4 and M/29/4. CLUT 1 entry 0 always stays transparent.
/// - The side panels, the default colours, substitution and re-mapping come
///   from the first source, in the order X/28/0, X/28/4 (Level 3.5),
///   M/29/0, M/29/4 (Level 3.5), whose field group is valid; the side
///   panels and substitution then fall back to the table's POP link.
/// - At Levels 1 and 1.5 the result is the defaults over `base`.
#[must_use]
pub fn resolve(sources: &PresentationSources<'_>, level: PresentationLevel, base: &ColourMap) -> PagePresentation {
    let mut out = PagePresentation::defaults(*base);
    if !cfg!(feature = "level25-32colors") {
        // Decision E.105: without the Colour Map of Level 2.5, CLUTs 2 and
        // 3 are copies of CLUTs 0 and 1, and nothing redefines a CLUT.
        let (low, high) = out.colour_map.entries.split_at_mut(COLOUR_MAP_ENTRIES / 2);
        high.copy_from_slice(low);
    }
    if !level.is_enhanced() {
        return out;
    }
    let level_35 = level.is_level35();
    let (page_35, magazine_35) =
        if level_35 { (sources.page_level_35, sources.magazine_level_35) } else { (None, None) };
    let ordered = [sources.page, page_35, sources.magazine, magazine_35];
    let first = |group: fn(&PresentationFields) -> bool| ordered.into_iter().flatten().find(|fields| group(fields));

    // CLUTs 2 and 3 are the Level 2.5 Colour Map's, CLUTs 0 and 1 the
    // Level 3.5 one's (decision E.105).
    if cfg!(feature = "level25-32colors")
        && let Some(words) =
            [sources.page, sources.magazine].into_iter().flatten().find_map(|fields| fields.colour_words)
    {
        out.colour_map.set_cluts(CLUT_LEVEL_25_FIRST, &words);
    }
    if cfg!(feature = "level35-32colors")
        && let Some(words) = [page_35, magazine_35].into_iter().flatten().find_map(|fields| fields.colour_words)
    {
        out.colour_map.set_cluts(CLUT_LEVEL_35_FIRST, &words);
    }

    let colours = first(|fields| fields.colours.is_some()).and_then(|fields| fields.colours);
    if let Some(colours) = colours {
        out.default_screen = colours.screen & TRIPLET_COLOUR_INDEX_MASK;
        out.default_row = colours.row & TRIPLET_COLOUR_INDEX_MASK;
        out.black_background_substitution = colours.black_background_substitution;
        out.remapping = Remapping::from_value(colours.remapping);
    } else if let Some(fallback) = sources.fallback {
        out.black_background_substitution = fallback.black_background_substitution;
    }

    let panels = first(|fields| fields.side_panels.is_some())
        .and_then(|fields| fields.side_panels)
        .or(sources.fallback.map(|fallback| fallback.side_panels));
    // Without side panels compiled in, a page is 40 columns (decision E.71).
    if cfg!(feature = "side-panels")
        && let Some(panels) = panels
    {
        out.side_panels = SidePanels::from_fields(panels, level);
    }
    out
}

/// The colour words of a [`ColourMap`]'s two CLUTs from `first_clut`, in
/// transmission order: the inverse of [`ColourMap::set_cluts`], for the
/// encoder and tests.
#[must_use]
pub fn colour_words(map: &ColourMap, first_clut: u8) -> ColourWords {
    let start = usize::from(first_clut).saturating_mul(CLUT_ENTRIES);
    core::array::from_fn(|offset| map.entries.get(start.saturating_add(offset)).copied().unwrap_or_default())
}

/// The words of two CLUTs are sixteen, as the packets carry them.
const _: () = assert!(COLOUR_WORDS == 2 * CLUT_ENTRIES);

#[cfg(test)]
#[cfg_attr(
    not(all(feature = "side-panels", feature = "level35-32colors")),
    allow(dead_code, unused_imports, reason = "helpers of the tests whose capability is compiled out (decision E.71)")
)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::types::DefaultColourFields;

    fn words(seed: u8) -> ColourWords {
        core::array::from_fn(|index| Rgb12::new(seed, index as u8, 15 - index as u8))
    }

    fn fields(words: Option<ColourWords>, screen: u8) -> PresentationFields {
        PresentationFields {
            designations: None,
            side_panels: Some(SidePanelFields {
                left: true,
                right: true,
                status_flag: false,
                left_columns: screen % 16,
            }),
            colour_words: words,
            colours: Some(DefaultColourFields {
                screen,
                row: screen + 1,
                black_background_substitution: screen % 2 == 1,
                remapping: screen % 8,
            }),
        }
    }

    /// REQ-L25-030, REQ-RND-030: the Table 30 defaults, including CLUT 1
    /// entries 5 and 6, and their 8-bit values.
    #[test]
    fn table_30_defaults() {
        let map = ColourMap::DEFAULT;
        let rgb = |index: u8| {
            let colour = map.entry(index);
            (colour.red(), colour.green(), colour.blue())
        };
        assert_eq!(rgb(colour_index(0, 1)), (15, 0, 0));
        assert_eq!(rgb(colour_index(0, 7)), (15, 15, 15));
        assert_eq!(rgb(colour_index(1, 5)), (7, 0, 7), "half magenta (decision E.43)");
        assert_eq!(rgb(colour_index(1, 6)), (0, 7, 7), "half cyan (decision E.43)");
        assert_eq!(rgb(colour_index(1, 7)), (7, 7, 7));
        assert_eq!(rgb(colour_index(2, 0)), (15, 0, 5));
        assert_eq!(rgb(colour_index(2, 6)), (6, 5, 2));
        assert_eq!(rgb(colour_index(3, 0)), (3, 3, 3));
        assert_eq!(rgb(colour_index(3, 7)), (13, 13, 13));
        assert!(ColourMap::is_transparent(colour_index(1, 0)));
        assert_eq!(map.rgb888(colour_index(0, 7)), [255, 255, 255]);
        assert_eq!(map.rgb888(colour_index(1, 5)), [119, 0, 119]);
        for index in 0..32 {
            let [red, green, blue] = map.rgb888(index);
            let colour = map.entry(index);
            assert_eq!((red, green, blue), (colour.red() * 17, colour.green() * 17, colour.blue() * 17));
        }
        // The Level 1 colours are CLUT 0 (decision E.43).
        for entry in 0..8 {
            assert_eq!(colour_index(0, entry), entry);
        }
    }

    /// REQ-L25-030: each source and order of precedence, Level 3.5 data
    /// ignored at Level 2.5, and CLUT 1 entry 0 kept transparent.
    // Needs its capability compiled in (decision E.105).
    #[cfg(feature = "level35-32colors")]
    #[test]
    fn colour_map_sources_and_precedence() {
        let base = ColourMap::DEFAULT;
        let page = fields(Some(words(1)), 3);
        let magazine = fields(Some(words(2)), 4);
        let page_35 = fields(Some(words(5)), 6);
        let magazine_35 = fields(Some(words(7)), 8);
        let all = PresentationSources {
            page: Some(&page),
            page_level_35: Some(&page_35),
            magazine: Some(&magazine),
            magazine_level_35: Some(&magazine_35),
            fallback: None,
        };
        let at_35 = resolve(&all, PresentationLevel::Level35, &base);
        assert_eq!(colour_words(&at_35.colour_map, 2), words(1), "X/28/0 beats M/29/0");
        let mut expected_01 = words(5);
        expected_01[8] = base.entry(COLOUR_TRANSPARENT);
        assert_eq!(colour_words(&at_35.colour_map, 0), expected_01, "X/28/4 beats M/29/4, entry 8 kept");
        let at_25 = resolve(&all, PresentationLevel::Level25, &base);
        assert_eq!(colour_words(&at_25.colour_map, 0), colour_words(&base, 0), "CLUTs 0 and 1 fixed at 2.5");
        assert_eq!(colour_words(&at_25.colour_map, 2), words(1));
        // Invalid page words: the magazine's apply.
        let damaged = fields(None, 3);
        let sources = PresentationSources { page: Some(&damaged), page_level_35: Some(&damaged), ..all };
        let map = resolve(&sources, PresentationLevel::Level35, &base).colour_map;
        assert_eq!(colour_words(&map, 2), words(2));
        let mut expected_01 = words(7);
        expected_01[8] = base.entry(COLOUR_TRANSPARENT);
        assert_eq!(colour_words(&map, 0), expected_01);
        // No source at all: Table 30; and nothing at Level 1.5.
        let none = PresentationSources::default();
        assert_eq!(resolve(&none, PresentationLevel::Level35, &base), PagePresentation::defaults(base));
        assert_eq!(resolve(&all, PresentationLevel::Level15, &base), PagePresentation::defaults(base));
    }

    /// REQ-L25-031: the default colours, substitution and re-mapping come
    /// from the first valid source, X/28/0, X/28/4, M/29/0, M/29/4; then
    /// substitution falls back to the table.
    #[test]
    fn default_colours_follow_decision_e44() {
        let base = ColourMap::DEFAULT;
        let page = fields(None, 3);
        let page_35 = fields(None, 10);
        let magazine = fields(None, 12);
        let magazine_35 = fields(None, 17);
        let mut sources = PresentationSources {
            page: Some(&page),
            page_level_35: Some(&page_35),
            magazine: Some(&magazine),
            magazine_level_35: Some(&magazine_35),
            fallback: Some(FallbackFields {
                side_panels: SidePanelFields::default(),
                black_background_substitution: true,
            }),
        };
        let screen = |sources: &PresentationSources<'_>, level| resolve(sources, level, &base).default_screen;
        assert_eq!(screen(&sources, PresentationLevel::Level35), 3);
        let no_colours = PresentationFields { colours: None, ..page };
        sources.page = Some(&no_colours);
        assert_eq!(screen(&sources, PresentationLevel::Level35), 10, "X/28/4 next at 3.5");
        assert_eq!(screen(&sources, PresentationLevel::Level25), 12, "M/29/0 next at 2.5");
        sources.page_level_35 = Some(&no_colours);
        sources.magazine = Some(&no_colours);
        let at_35 = resolve(&sources, PresentationLevel::Level35, &base);
        assert_eq!((at_35.default_screen, at_35.default_row), (17, 18));
        assert_eq!(at_35.remapping, Remapping::from_value(1));
        assert!(at_35.black_background_substitution);
        sources.magazine_level_35 = Some(&no_colours);
        let fallback = resolve(&sources, PresentationLevel::Level35, &base);
        assert_eq!((fallback.default_screen, fallback.default_row, fallback.remapping), (0, 0, Remapping::NONE));
        assert!(fallback.black_background_substitution, "from the table's fallback (decision E.53)");
    }

    /// Table 4: the eight re-mapping values.
    #[test]
    fn remapping_follows_table_4() {
        let expected = [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 1), (2, 2), (2, 3)];
        for (value, (foreground, background)) in expected.into_iter().enumerate() {
            let remapping = Remapping::from_value(value as u8);
            assert_eq!((remapping.foreground_clut, remapping.background_clut), (foreground, background));
            assert_eq!(remapping.foreground(7), foreground * 8 + 7);
            assert_eq!(remapping.background(0), background * 8);
        }
    }

    /// REQ-L25-040: every row of the width table of decision E.49, the
    /// status flag at each level, and the table's fallback values.
    // Needs its capability compiled in (decision E.71).
    #[cfg(feature = "side-panels")]
    #[test]
    fn side_panel_widths() {
        let panels = |left, right, status_flag, left_columns, level| {
            SidePanels::from_fields(SidePanelFields { left, right, status_flag, left_columns }, level)
        };
        let l35 = PresentationLevel::Level35;
        let widths = |p: SidePanels| (p.left, p.right);
        assert_eq!(widths(panels(false, false, true, 5, l35)), (0, 0));
        assert_eq!(widths(panels(true, false, true, 0, l35)), (16, 0));
        assert_eq!(widths(panels(true, false, true, 5, l35)), (5, 0));
        assert_eq!(widths(panels(false, true, true, 0, l35)), (0, 16));
        assert_eq!(widths(panels(false, true, true, 5, l35)), (0, 11));
        assert_eq!(widths(panels(true, true, true, 5, l35)), (5, 11));
        assert_eq!(widths(panels(true, true, true, 0, l35)), (0, 16));
        assert_eq!(widths(panels(true, true, false, 8, l35)), (8, 8), "Level 3.5 ignores the status flag");
        assert_eq!(widths(panels(true, true, false, 8, PresentationLevel::Level25)), (0, 0));
        assert_eq!(widths(panels(true, true, true, 8, PresentationLevel::Level25)), (8, 8));
        assert_eq!(widths(panels(true, true, true, 8, PresentationLevel::Level15)), (0, 0));
        let both = panels(true, true, true, 6, l35);
        assert_eq!((both.left_first_column(), both.display_columns()), (66, 56));
        // The fallback values of Table 21.
        let fallback = |bits| SidePanels::from_fields(crate::packet::mot_fallback(bits).side_panels, l35);
        assert_eq!(widths(fallback(0b0000)), (0, 0));
        assert_eq!(widths(fallback(0b0010)), (16, 0));
        assert_eq!(widths(fallback(0b0100)), (0, 16));
        assert_eq!(widths(fallback(0b0110)), (8, 8));
        assert_eq!(widths(fallback(0b0111)), (0, 0), "bit 1 set: no side panels");
        // Precedence: X/28/0's panels, then the fallback.
        let page = fields(None, 4);
        let sources = PresentationSources {
            page: Some(&page),
            fallback: Some(crate::packet::mot_fallback(0b0010)),
            ..PresentationSources::default()
        };
        assert_eq!(widths(resolve(&sources, l35, &ColourMap::DEFAULT).side_panels), (4, 12));
        let sources = PresentationSources { page: None, ..sources };
        assert_eq!(widths(resolve(&sources, l35, &ColourMap::DEFAULT).side_panels), (16, 0));
    }
}
