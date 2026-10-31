//! Tests of the Colour Map, re-mapping and the precedence of decision E.44.
#![cfg_attr(
    not(feature = "level35-32colors"),
    allow(dead_code, unused_imports, reason = "helpers of the tests whose capability is compiled out (decision E.71)")
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::config::PresentationLevel;
use crate::types::{DefaultColourFields, FallbackFields, PresentationFields, SidePanelFields};

fn words(seed: u8) -> ColourWords {
    core::array::from_fn(|index| Rgb12::new(seed, index as u8, 15 - index as u8))
}

fn fields(words: Option<ColourWords>, screen: u8) -> PresentationFields {
    PresentationFields {
        designations: None,
        side_panels: Some(SidePanelFields { left: true, right: true, status_flag: false, left_columns: screen % 16 }),
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
        fallback: Some(FallbackFields { side_panels: SidePanelFields::default(), black_background_substitution: true }),
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

/// REQ-1644: the side-panel fields of every source have no effect at any
/// level, nor do the fallback bits of the table's POP link.
#[test]
fn side_panel_fields_have_no_effect() {
    let enabled = SidePanelFields { left: true, right: true, status_flag: true, left_columns: 8 };
    let with = fields(None, 4);
    let without = PresentationFields { side_panels: None, ..with };
    let panels = PresentationFields { side_panels: Some(enabled), ..with };
    let fallback = |side_panels| FallbackFields { side_panels, black_background_substitution: true };
    for level in
        [PresentationLevel::Level1, PresentationLevel::Level15, PresentationLevel::Level25, PresentationLevel::Level35]
    {
        let plain = PresentationSources {
            page: Some(&without),
            page_level_35: Some(&without),
            magazine: Some(&without),
            magazine_level_35: Some(&without),
            fallback: Some(fallback(SidePanelFields::default())),
        };
        let carried = PresentationSources {
            page: Some(&panels),
            page_level_35: Some(&panels),
            magazine: Some(&panels),
            magazine_level_35: Some(&panels),
            fallback: Some(fallback(enabled)),
        };
        assert_eq!(resolve(&carried, level, &ColourMap::DEFAULT), resolve(&plain, level, &ColourMap::DEFAULT));
    }
}
