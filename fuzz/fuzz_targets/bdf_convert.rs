//! The font converter (REQ-TST-040, REQ-FNT-010): reading a source font and
//! converting it never panics, and whatever it converts is written as a
//! font the reader of loaded fonts accepts with the same glyphs.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_core::consts::FONT_MAX_CHARACTERS;
use neoteletext_core::types::FontGlyph;
use neoteletext_formats::bdf::convert::{convert, read_map};
use neoteletext_formats::bdf::source::{SourceGlyph, read_source};
use neoteletext_formats::write::{FontText, write_font};

/// Bytes of the input given to the map reader first.
const MAP_BYTES: usize = 64;

fuzz_target!(|data: &[u8]| {
    let (map_text, font_text) = data.split_at(data.len().min(MAP_BYTES));
    let mut pairs = vec![(0u32, 0u32); 64];
    let pairs = read_map(map_text, &mut pairs).ok().map(|count| pairs[..count].to_vec());
    let mut glyphs = vec![SourceGlyph::EMPTY; 4096];
    let Ok(font) = read_source(font_text, &mut glyphs) else { return };
    let mut table = vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS];
    let Ok(count) = convert(&font, &glyphs, pairs.as_deref(), &mut table) else { return };
    let text = FontText { family: font.family, copyright: font.copyright, notice: font.notice, comment: "fuzz" };
    let mut written = Vec::new();
    // A carried text too long for REQ-FNT-002 is refused by the writer.
    if write_font(&mut written, &text, &table[..count]).is_err() {
        return;
    }
    let mut read_back = vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS];
    let loaded = neoteletext_formats::bdf::read(&written, &mut read_back).expect("the written font is accepted");
    assert_eq!(loaded.characters, count);
    assert_eq!(read_back[..count], table[..count]);
});
