//! Tests of the bundled glyph set: order, size, the diacritics drawn over their bases, loaded
//! fonts and replaced G3 glyphs.
#![allow(clippy::indexing_slicing, clippy::unwrap_used)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;
use crate::charset::{CharSet, G0, G0Set};
use crate::glyphs::conformance::font_set_of;
use crate::types::FontGlyph;

#[test]
fn glyphs_are_sorted_and_unique() {
    let set = GlyphSet::bedstead();
    assert!(set.entries.windows(2).all(|pair| matches!(pair, [a, b] if a.code_point < b.code_point)));
}

/// REQ-RND-020: the glyph set is data; an empty set is allowed and
/// finds nothing.
#[test]
fn set_size_and_empty_set() {
    static NONE: [GlyphEntry; 0] = [];
    let set = GlyphSet::bedstead();
    assert_eq!(set.len(), set.entries.len());
    assert!(!set.is_empty());
    let empty = GlyphSet::from_entries(&NONE, "empty");
    assert_eq!((empty.len(), empty.is_empty(), empty.provenance), (0, true, "empty"));
    assert_eq!(empty.lookup('A'), None);
    let latin = CharSet::G0(G0 { set: G0Set::Latin, national: None });
    assert_eq!(empty.glyph(CharRef::new(latin, b'A')), PLACEHOLDER, "REQ-CHR-004: a missing glyph is visible");
}

/// REQ-RND-014, decision E.26: every one of the fifteen G2 column 4
/// marks (Table 28, modes 10001 to 11111) has a spacing form that
/// Bedstead draws, and with no precomposed letter the mark is overlaid.
#[test]
fn every_diacritic_overlays_its_spacing_form() {
    // The spacing form Unicode pairs with each combining mark, in mark
    // order 4/1 to 4/F: grave, acute, circumflex, tilde, macron, breve,
    // dot above, diaeresis, dot below, ring, cedilla, low line, double
    // acute, ogonek and caron.
    const SPACING: [char; 15] = [
        '`', '\u{B4}', '\u{2C6}', '\u{2DC}', '\u{AF}', '\u{2D8}', '\u{2D9}', '\u{A8}', '.', '\u{2DA}', '\u{B8}', '_',
        '\u{2DD}', '\u{2DB}', '\u{2C7}',
    ];
    let set = GlyphSet::bedstead();
    let latin = CharSet::G0(G0 { set: G0Set::Latin, national: None });
    // "q" has no precomposed form with any of the marks.
    let base = set.lookup('q').unwrap();
    for (mark, &spacing) in (1u8..).zip(&SPACING) {
        let character = CharRef { diacritic: mark, ..CharRef::new(latin, b'q') };
        let combining = to_unicode(character).combining.unwrap();
        assert_eq!(spacing_mark(combining), Some(spacing), "mark {mark}");
        assert_eq!(compose('q', combining), None);
        let mark_rows = set.lookup(spacing).unwrap();
        assert_ne!(mark_rows, [0; GLYPH_LINES], "mark {mark} draws something");
        let overlay: [u8; GLYPH_LINES] = core::array::from_fn(|row| base[row] | mark_rows[row]);
        assert_eq!(set.glyph(character), overlay, "mark {mark}");
    }
    // A combining character outside the fifteen has no spacing form.
    assert_eq!(spacing_mark('\u{0305}'), None);
    assert_eq!(spacing_mark('a'), None);
}

/// REQ-RND-014, decision E.26: a precomposed glyph when the set has
/// one, otherwise the spacing mark's glyph overlaid on the base.
#[test]
fn diacritics_use_the_precomposed_glyph_or_an_overlay() {
    const BASE: [u8; GLYPH_LINES] = [0, 0, 0x0E, 0x11, 0x1F, 0x10, 0x0E, 0, 0];
    const MARK: [u8; GLYPH_LINES] = [0x02, 0x04, 0, 0, 0, 0, 0, 0, 0];
    // Deliberately unlike the overlay, so the test tells them apart.
    const PRECOMPOSED: [u8; GLYPH_LINES] = [0x04, 0x08, 0x0E, 0x11, 0x1F, 0x10, 0x0F, 0, 0];
    static WITH_PRECOMPOSED: [GlyphEntry; 3] = [
        GlyphEntry { code_point: 'e' as u32, rows: BASE },
        GlyphEntry { code_point: '\u{B4}' as u32, rows: MARK },
        GlyphEntry { code_point: '\u{E9}' as u32, rows: PRECOMPOSED },
    ];
    static OVERLAY_ONLY: [GlyphEntry; 2] =
        [GlyphEntry { code_point: 'e' as u32, rows: BASE }, GlyphEntry { code_point: '\u{B4}' as u32, rows: MARK }];
    let latin = CharSet::G0(G0 { set: G0Set::Latin, national: None });
    // Diacritic 2 is the acute accent (Table 28, mode 10010).
    let character = CharRef { diacritic: 2, ..CharRef::new(latin, b'e') };
    assert_eq!(GlyphSet::from_entries(&WITH_PRECOMPOSED, "test").glyph(character), PRECOMPOSED);
    let overlay: [u8; GLYPH_LINES] = core::array::from_fn(|row| BASE[row] | MARK[row]);
    assert_eq!(GlyphSet::from_entries(&OVERLAY_ONLY, "test").glyph(character), overlay);
}

/// REQ-FNT-005, decision E.26: a loaded font draws a character with a
/// diacritical mark only by its precomposed form; with none, as for "q"
/// with an acute, the font is not asked and the set's own glyphs overlay
/// the mark. G3 characters never come from a font, and G3 has no place
/// among the sets fonts draw (REQ-FNT-011).
#[test]
fn loaded_fonts_draw_precomposed_marks_only() {
    const DOTS: [u16; CELL_FIELD_LINES] = [0xFFF; CELL_FIELD_LINES];
    static FONT: [FontGlyph; 3] = [
        FontGlyph::small('q' as u32, DOTS),
        FontGlyph::small('\u{B4}' as u32, DOTS),
        FontGlyph::small('\u{E9}' as u32, DOTS),
    ];
    let glyphs = GlyphSet::bedstead().with_fonts(Some(LoadedFont::new(&FONT).unwrap()), [None; FONT_SET_COUNT]);
    let latin = CharSet::G0(G0 { set: G0Set::Latin, national: None });
    // Diacritic 2 is the acute accent (Table 28, mode 10010).
    let acute = |code| CharRef { diacritic: 2, ..CharRef::new(latin, code) };
    assert_eq!(glyphs.loaded(acute(b'e')).map(|glyph| glyph.rows[0]), Some(0xFFF), "é, precomposed");
    assert_eq!(glyphs.loaded(acute(b'q')), None, "no precomposed form");
    assert!(glyphs.loaded(CharRef::new(latin, b'q')).is_some());
    assert_eq!(glyphs.loaded(CharRef::new(CharSet::G3, 0x20)), None);
    assert_eq!(font_set_of(CharSet::G3), None);
}

/// REQ-RND-020, decision E.50: the G3 glyphs are data that a set can
/// replace, named by their provenance; a code outside 2/0 to 7/F is blank.
#[test]
fn g3_glyphs_can_be_replaced() {
    const FULL: G3Glyph = G3Glyph { small: [0xFFF; CELL_FIELD_LINES], large: [0xFFF; CELL_HEIGHT] };
    static ALL_FULL: [G3Glyph; G3_CODES] = [FULL; G3_CODES];
    let bedstead = GlyphSet::bedstead();
    let replaced = bedstead.with_g3(&ALL_FULL, "test");
    assert_eq!(replaced.g3_provenance, "test");
    assert_eq!(*replaced.g3(0x20), FULL);
    assert_ne!(*bedstead.g3(0x20), FULL, "the shipped 2/0 differs");
    assert_eq!(*replaced.g3(0x1F), G3Glyph::BLANK);
    assert_eq!(*replaced.g3(0x80), G3Glyph::BLANK);
    assert_eq!(replaced.lookup('A'), bedstead.lookup('A'), "the text glyphs are kept");
}
