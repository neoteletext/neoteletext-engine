//! REQ-FNT-001, REQ-CHR-004: the compiled font holds every character of
//! every character set a build can show, so none is drawn as the placeholder.
//! With `--nocapture` the test prints the table of its counts.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, missing_docs)]

use neoteletext_core::charset::{CharRef, CharSet, compositions, glyph_key, to_unicode};
#[cfg(feature = "extended-sets")]
use neoteletext_core::charset::{extended_compositions, joined_forms, mark_sequences};
use neoteletext_core::glyphs::conformance::FONT_SETS;
use neoteletext_core::glyphs::{GlyphSet, PLACEHOLDER, SPACING_MARKS};

/// Characters 2/0 to 7/F of a set (clause 15).
const CODES: core::ops::RangeInclusive<u8> = 0x20..=0x7F;
/// The standard's 24 sets, which `FONT_SETS` lists first (REQ-FNT-011).
const STANDARD_SETS: usize = 24;

/// The sets this build can show.
fn sets() -> &'static [(&'static str, CharSet)] {
    if cfg!(feature = "extended-sets") { &FONT_SETS } else { &FONT_SETS[..STANDARD_SETS] }
}

/// The characters of a set, by code.
fn characters(set: CharSet) -> Vec<(u8, char)> {
    CODES
        .map(|code| (code, glyph_key(CharRef::new(set, code))))
        .filter(|(code, character)| !character.is_control() && !to_unicode(CharRef::new(set, *code)).base.is_control())
        .collect()
}

/// The characters of one set the font does not hold.
fn lacking(font: &GlyphSet<'_>, set: CharSet) -> Vec<(u8, char)> {
    characters(set).into_iter().filter(|&(code, _)| font.glyph(CharRef::new(set, code)) == PLACEHOLDER).collect()
}

#[test]
fn every_character_of_every_set_is_in_the_font() {
    let font = GlyphSet::noto_teletext();
    println!("{:<36} {:>10} {:>6} {:>8}", "set", "characters", "held", "missing");
    let mut missing = Vec::new();
    for &(name, set) in sets() {
        let lacks = lacking(&font, set);
        let total = characters(set).len();
        println!("{name:<36} {total:>10} {:>6} {:>8}", total - lacks.len(), lacks.len());
        missing.extend(lacks.into_iter().map(|(code, c)| (name, code, c)));
    }
    assert_eq!(missing, []);
    println!("{} glyphs in this build", font.len());
}

/// Decision E.26: every spacing form of a mark, and every letter a base and
/// a mark compose to, is in the font, so a mark is overlaid only on a letter
/// Unicode does not compose.
#[test]
fn every_mark_and_composed_letter_is_in_the_font() {
    let font = GlyphSet::noto_teletext();
    let spacing: Vec<char> =
        SPACING_MARKS.iter().map(|&(_, spacing)| spacing).filter(|&c| font.lookup(c).is_none()).collect();
    assert_eq!(spacing, []);
    let composed: Vec<char> = compositions().iter().map(|&(_, _, c)| c).filter(|&c| font.lookup(c).is_none()).collect();
    assert_eq!(composed, []);
}

/// REQ-CHR-014, REQ-CHR-015, REQ-CHR-016: the composed letters of
/// neoteletext's own sets, every joined form and every Thai and Lao glyph of
/// two marks.
#[cfg(feature = "extended-sets")]
#[test]
fn every_form_of_the_extended_sets_is_in_the_font() {
    let font = GlyphSet::noto_teletext();
    let forms = joined_forms().iter().map(|&(_, _, point)| point);
    let sequences = mark_sequences().iter().map(|&(key, _, _)| key);
    let composed = extended_compositions().iter().map(|&(_, _, c)| c);
    let lacks: Vec<char> = forms.chain(sequences).chain(composed).filter(|&c| font.lookup(c).is_none()).collect();
    assert_eq!(lacks, []);
}

/// REQ-SEC-005: what the status area can show (`viewer/status.rs`: the page
/// number's hexadecimal digits, "P", "/", "?", "!", "+", "-" and "LEAVE?") is
/// drawn from real glyphs, each unlike every other, in the font row 0 uses.
#[test]
fn the_status_area_is_readable() {
    const STATUS: &[u8] = b"0123456789ABCDEFLPV/?!+-";
    let font = GlyphSet::noto_teletext();
    let glyphs: Vec<_> = STATUS.iter().map(|&byte| font.lookup(char::from(byte)).unwrap()).collect();
    for (index, glyph) in glyphs.iter().enumerate() {
        let shown = char::from(STATUS[index]);
        assert_ne!(**glyph, PLACEHOLDER, "{shown}");
        assert!(glyph.iter().any(|&row| row != 0), "{shown} draws something");
        // Letters and digits a reader could confuse differ by more than a
        // stray pixel.
        for (other, second) in glyphs.iter().enumerate().skip(index + 1) {
            let differing: u32 = glyph.iter().zip(second.iter()).map(|(a, b)| (a ^ b).count_ones()).sum();
            assert!(differing >= MINIMUM_DIFFERENCE, "{shown} and {} differ by {differing}", char::from(STATUS[other]));
        }
    }
}

/// The fewest pixels two status-area glyphs may differ by, so that a small
/// display still tells them apart: "E" and "F" differ by this, the bottom bar
/// of the "E"; the next closest, "8" and "B", differ by 20 (measured on
/// 1 November 2026).
const MINIMUM_DIFFERENCE: u32 = 12;
