//! Generates the core's glyph data (REQ-FNT-001, REQ-RND-020,
//! REQ-RND-037). Glyphs stay data: replacing a source file and rerunning
//! `cargo xtask glyphs` swaps a glyph set without code changes.
//!
//! - `crates/neoteletext-core/src/glyphs/noto.rs` from the Noto Teletext
//!   files in `third_party/noto/`, which `cargo xtask check-noto` checks
//!   against their provenance record (REQ-FNT-012).
//! - `crates/neoteletext-core/src/glyphs/g3.txt` from the G3 geometry of
//!   [`crate::g3`], and `g3.rs` from that text.
//! - `crates/neoteletext-core/src/glyphs/required.txt` from the core's
//!   character set tables: every character a glyph set must draw.
//! - `crates/neoteletext-core/src/glyphs/forms.txt` from the core's joined
//!   forms (REQ-CHR-015): the code point drawn for each form of each
//!   joining letter, for font converters.
//!
//! The tests below regenerate every file in memory and compare it with the
//! stored one, so `cargo test` fails when the data is out of date.

mod g3_table;
mod noto;
mod required;

use crate::g3;
use g3_table::{G3_TARGET, G3_TEXT, g3_source};
use noto::{TARGET, noto_source, source_files};
use required::{FORMS, REQUIRED, forms_text, required_text};
use std::fs;
use std::path::Path;

/// Regenerates every glyph table. The lists the converter reads come first
/// (REQ-2163's order), so that a set added to the core reaches
/// `required.txt` even before the font files that will draw it exist.
pub fn generate(root: &Path) -> Result<(), String> {
    let required = required_text();
    fs::write(root.join(REQUIRED), &required).map_err(|e| format!("{REQUIRED}: {e}"))?;
    println!(
        "wrote {} required characters to {REQUIRED}",
        required.lines().filter(|line| line.starts_with("U+")).count()
    );
    let forms = forms_text();
    fs::write(root.join(FORMS), &forms).map_err(|e| format!("{FORMS}: {e}"))?;
    println!("wrote {} joined forms to {FORMS}", forms.lines().filter(|line| line.starts_with("U+")).count());
    let g3_text = g3::text();
    fs::write(root.join(G3_TEXT), &g3_text).map_err(|e| format!("{G3_TEXT}: {e}"))?;
    fs::write(root.join(G3_TARGET), g3_source(&g3_text)?).map_err(|e| format!("{G3_TARGET}: {e}"))?;
    println!("wrote {} G3 glyphs to {G3_TEXT} and {G3_TARGET}", g3::CODES);
    let (source, count) = noto_source(&source_files(root)?)?;
    fs::write(root.join(TARGET), source).map_err(|e| format!("{TARGET}: {e}"))?;
    println!("wrote {count} glyphs to {TARGET}");
    Ok(())
}

#[cfg(test)]
mod tests;
