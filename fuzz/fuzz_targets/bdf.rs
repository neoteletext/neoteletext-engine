//! The BDF font reader (REQ-TST-040, REQ-FNT-002): it never panics, and a
//! font it accepts meets every rule the interface checks again
//! (`docs/font-security.md` section 3.1).
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_core::consts::{CELL_WIDTH, FONT_MAX_CHARACTERS};
use neoteletext_core::types::FontGlyph;

fuzz_target!(|data: &[u8]| {
    let mut glyphs = [FontGlyph::EMPTY; FONT_MAX_CHARACTERS];
    let Ok(font) = neoteletext_formats::bdf::read(data, &mut glyphs) else { return };
    assert!(font.characters <= FONT_MAX_CHARACTERS);
    let table = &glyphs[..font.characters];
    assert!(table.windows(2).all(|pair| pair[0].code_point < pair[1].code_point), "sorted, no repeats");
    for glyph in table {
        assert!(char::from_u32(glyph.code_point).is_some(), "a Unicode scalar value");
        assert!(glyph.rows.iter().all(|&row| row >> CELL_WIDTH == 0), "12 dots per line");
    }
});
