//! The stored glyph data against its generators.

use super::*;
use neoteletext_core::consts::CELL_HEIGHT;
use neoteletext_core::types::FontGlyph;
use neoteletext_formats::write::{FontText, write_font};

fn stored(path: &str) -> String {
    fs::read_to_string(crate::workspace_root().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// REQ-RND-037: the generator is deterministic and the stored G3 data
/// matches its output; run `cargo xtask glyphs` after changing it.
#[test]
fn stored_g3_glyphs_are_up_to_date() {
    assert_eq!(g3::text(), g3::text(), "the generator is deterministic");
    assert_eq!(stored(G3_TEXT), g3::text(), "{G3_TEXT} is stale: run cargo xtask glyphs");
    assert_eq!(stored(G3_TARGET), g3_source(&stored(G3_TEXT)).unwrap(), "{G3_TARGET} is stale");
}

/// REQ-RND-020: the stored font is the Noto Teletext files.
#[test]
fn stored_noto_glyphs_are_up_to_date() {
    let files = source_files(&crate::workspace_root()).unwrap();
    assert_eq!(stored(TARGET), noto_source(&files).unwrap().0, "{TARGET} is stale: run cargo xtask glyphs");
}

/// REQ-FNT-001: a character has one shape, so files that disagree on one are
/// refused, as is a file that does not hold the 12 by 20 form.
#[test]
fn the_font_files_must_agree() {
    let font = |line: u16| {
        let mut bytes = Vec::new();
        let glyph = FontGlyph::tall(u32::from('A'), [line; CELL_HEIGHT]);
        write_font(&mut bytes, &FontText::default(), &[glyph]).unwrap();
        bytes
    };
    let error = noto_source(&[("one", font(0x800)), ("two", font(0x400))]).unwrap_err();
    assert!(error.contains("U+0041 differs"), "{error}");
    let same = noto_source(&[("one", font(0x800)), ("two", font(0x800))]).unwrap_err();
    assert!(same.contains("which the standard's sets need"), "{same}");
}

/// The required characters are generated from the character set
/// tables, never chosen by hand.
#[test]
fn stored_required_characters_are_up_to_date() {
    assert_eq!(stored(REQUIRED), required_text(), "{REQUIRED} is stale: run cargo xtask glyphs");
    assert_eq!(stored(FORMS), forms_text(), "{FORMS} is stale: run cargo xtask glyphs");
}

#[test]
fn g3_text_is_checked() {
    assert!(g3_source("2/0 10 000").is_err(), "too few lines");
    assert!(g3_source("8/0 10 0 0 0 0 0 0 0 0 0 0").is_err(), "code above 7/F");
    assert!(g3_source("2/0 12 0 0 0 0 0 0 0 0 0 0 0 0").is_err(), "unknown height");
    assert!(g3_source("2/0 10 1000 0 0 0 0 0 0 0 0 0").is_err(), "wider than 12 bits");
    assert!(g3_source("").is_err(), "positions missing");
}
