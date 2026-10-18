//! The fonts embedded for testing (build.rs): the complete 12 x 10 fonts of
//! the owner's font tree, each drawing all 24 character sets. The user picks
//! one, or Bedstead, the bundled font, which is the default (REQ-FNT-001).

use neoteletext_core::glyphs::LoadedFont;
use neoteletext_core::types::FontGlyph;

/// One embedded font.
#[derive(Debug)]
pub struct EmbeddedFont {
    /// Its id in the font tree.
    pub id: &'static str,
    /// Its title.
    pub title: &'static str,
    /// Under an open licence; otherwise embedded only with the
    /// `nonfree-fonts` feature.
    pub free: bool,
    /// Its glyphs, 12 by 10.
    pub glyphs: &'static [FontGlyph],
}

include!(concat!(env!("OUT_DIR"), "/embedded_fonts.rs"));

/// The embedded font with this id, if any.
#[must_use]
pub fn find(id: &str) -> Option<&'static EmbeddedFont> {
    EMBEDDED.iter().find(|font| font.id == id)
}

/// Whether character rounding applies to the font with this id: Bedstead
/// and fonts that are doubled dot matrices are rounded as the SAA5050
/// rounded its matrix; a font drawn at half-dot resolution carries its own
/// smoothing and is shown as drawn, as every decoder with such a font
/// showed it (owner, 14 October 2026).
#[must_use]
pub fn rounds(id: &str) -> bool {
    find(id).is_none_or(|font| LoadedFont::new(font.glyphs).is_ok_and(|loaded| loaded.is_dot_matrix()))
}
