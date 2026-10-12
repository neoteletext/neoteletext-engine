//! The neoteletext browser (area BRW, REQ-BRW-001, `docs/desktop-browser.md`): a window with tabs,
//! an address bar, back and forward and bookmarks, over a fetcher process
//! and a decoder process per tab.
//!
//! The program is also its own helpers: run as `neoteletext fetch` it is a
//! tab's fetcher, and as `neoteletext decode` a tab's decoder. The window
//! starts both for each tab, confined (`helpers`).

pub mod app;
pub mod helpers;
pub mod picture;
pub mod storage;
pub mod tab;

use eframe::egui;
use neoteletext_core::glyphs::GlyphSet;
use std::sync::Arc;

/// Bedstead, the one bundled font (REQ-FNT-001), which a tab's decoder
/// draws with when no other font applies.
pub static BEDSTEAD: GlyphSet = GlyphSet::bedstead();

/// Seconds since 1970, for the history of visits.
#[must_use]
pub fn now_seconds() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// The system's fonts for the window's own text: the name, the file and the
/// face in it. Fonts are read from the system, never shipped, so their
/// licences stay the system's. A page never uses them: pages are drawn
/// with the teletext glyph sets.
const FONTS: [(&str, &str, u32); 9] = [
    ("Helvetica", "/System/Library/Fonts/Helvetica.ttc", 0),
    ("Apple Symbols", "/System/Library/Fonts/Apple Symbols.ttf", 0),
    ("Menlo", "/System/Library/Fonts/Menlo.ttc", 0),
    ("DejaVu Sans", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
    ("DejaVu Sans Mono", "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 0),
    ("Noto Sans", "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf", 0),
    ("Segoe UI", "C:\\Windows\\Fonts\\segoeui.ttf", 0),
    ("Segoe UI Symbol", "C:\\Windows\\Fonts\\seguisym.ttf", 0),
    ("Consolas", "C:\\Windows\\Fonts\\consola.ttf", 0),
];
/// The fonts preferred for fixed-width text.
const MONOSPACE: [&str; 3] = ["Menlo", "DejaVu Sans Mono", "Consolas"];

/// The window's fonts, read from the system.
pub fn system_fonts() -> Result<egui::FontDefinitions, String> {
    let mut definitions = egui::FontDefinitions::empty();
    let mut names = Vec::new();
    for (name, path, index) in FONTS {
        if let Ok(bytes) = std::fs::read(path) {
            let mut data = egui::FontData::from_owned(bytes);
            data.index = index;
            definitions.font_data.insert(name.to_string(), Arc::new(data));
            names.push(name.to_string());
        }
    }
    if names.is_empty() {
        return Err("no system font was found for the window's text".to_string());
    }
    let mut monospace: Vec<String> = names.iter().filter(|name| MONOSPACE.contains(&name.as_str())).cloned().collect();
    monospace.extend(names.iter().filter(|name| !MONOSPACE.contains(&name.as_str())).cloned());
    definitions.families.insert(egui::FontFamily::Proportional, names);
    definitions.families.insert(egui::FontFamily::Monospace, monospace);
    Ok(definitions)
}
