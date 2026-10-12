//! The font browser (REQ-BRW-010, decision E.74): a panel on the left that
//! lists Bedstead and the fonts of the user's libraries (REQ-FNT-004), where
//! moving to an entry, by click or by the up and down arrow keys, chooses
//! that font as the user's font (REQ-FNT-005).

use super::BrowserApp;
use crate::fonts::{Library, Request};
use eframe::egui::{Key, RichText, ScrollArea, Ui};
use neoteletext_app::pipe::LibraryFont;
use neoteletext_browser::fonts::{Chooser, Scope};
use neoteletext_web::sha256::{Digest, to_hex};
use std::collections::HashSet;

/// The panel's width when first shown, in points: room for a title of
/// `neoteletext_web::fonts::MAX_TITLE` characters.
pub(super) const PANEL_WIDTH: f32 = 240.0;

/// The panel's typing and scrolling.
#[derive(Default)]
pub struct FontBrowser {
    /// The up and down arrow keys move through the list rather than
    /// changing page.
    pub has_keys: bool,
    /// The chosen entry moved by keyboard, so the list scrolls to it.
    scroll: bool,
}

impl BrowserApp {
    /// The files of listed fonts that draw the same as a font listed before
    /// them (REQ-BRW-010, issue #207), found by the glyph digests of the
    /// fonts loaded so far; empty while duplicates are shown. The user's
    /// font is never among them.
    pub(super) fn hidden_duplicates(&self) -> HashSet<Digest> {
        let mut hidden = HashSet::new();
        if self.settings.show_duplicate_fonts {
            return hidden;
        }
        let wanted = self.wanted_font();
        let mut seen = HashSet::new();
        for font in self.library_fonts() {
            let chosen = wanted.as_ref().is_some_and(|wanted| to_hex(&font.sha256).as_slice() == wanted.as_bytes());
            if let Some(glyphs) = self.fonts.glyph_digest(&font.sha256)
                && !seen.insert(glyphs)
                && !chosen
            {
                hidden.insert(font.sha256);
            }
        }
        hidden
    }

    /// Every listed library's fonts, in the order the libraries were added
    /// and each index lists them.
    fn library_fonts(&self) -> Vec<LibraryFont> {
        self.settings
            .libraries
            .iter()
            .filter_map(|folder| match self.fonts.library(folder) {
                Some(Library::Listed(fonts, _)) => Some(fonts.iter().cloned()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    /// The fonts listed after Bedstead: the libraries' fonts, without the
    /// hidden duplicates.
    fn browser_fonts(&self) -> Vec<LibraryFont> {
        let hidden = self.hidden_duplicates();
        self.library_fonts().into_iter().filter(|font| !hidden.contains(&font.sha256)).collect()
    }

    /// The chosen entry: 0 for Bedstead, or one more than the font's place
    /// in `fonts`; none when the user's font is in no listed library.
    fn browser_chosen(&self, fonts: &[LibraryFont]) -> Option<usize> {
        let Some(wanted) = self.wanted_font() else { return Some(0) };
        fonts
            .iter()
            .position(|font| to_hex(&font.sha256).as_slice() == wanted.as_bytes())
            .map(|place| place.saturating_add(1))
    }

    /// Chooses an entry as the user's font, as the Fonts settings do.
    fn browser_choose(&mut self, entry: usize, fonts: &[LibraryFont]) {
        if self.browser_chosen(fonts) == Some(entry) {
            return;
        }
        match entry.checked_sub(1).and_then(|place| fonts.get(place)) {
            None => self.use_bedstead(),
            Some(font) => {
                let request = Request {
                    address: font.address.clone(),
                    sha256: Some(font.sha256),
                    chooser: Chooser::User,
                    scope: Scope::Shared,
                };
                self.choose(request, font.title.clone());
            }
        }
    }

    /// The up and down arrow keys and Esc while the panel has the keyboard
    /// (decision E.74 item 4); true when the panel used the key.
    pub(super) fn font_browser_key(&mut self, key: Key) -> bool {
        if !(self.settings.font_browser && self.font_browser.has_keys) {
            return false;
        }
        match key {
            Key::Escape => self.font_browser.has_keys = false,
            Key::ArrowUp | Key::ArrowDown => {
                let fonts = self.browser_fonts();
                let next = match (self.browser_chosen(&fonts), key) {
                    (None, _) => 0,
                    (Some(entry), Key::ArrowUp) => entry.saturating_sub(1),
                    (Some(entry), _) => entry.saturating_add(1).min(fonts.len()),
                };
                self.browser_choose(next, &fonts);
                self.font_browser.scroll = true;
            }
            _ => return false,
        }
        true
    }

    /// Shows or hides the panel, and saves which.
    pub(super) fn toggle_font_browser(&mut self) {
        self.settings.font_browser = !self.settings.font_browser;
        self.font_browser.has_keys = self.settings.font_browser;
        self.storage.save_settings(&self.settings);
    }

    /// The panel.
    pub(super) fn font_browser(&mut self, ui: &mut Ui) {
        ui.heading("Font browser");
        if let Some((name, chooser)) = &self.tab().font_shown
            && *chooser != Chooser::User
        {
            // Decision E.74 item 3: browsing does not displace the page's
            // or the channel's font.
            ui.label(format!(
                "This page is drawn in {name}, {}. Your font draws pages that name none.",
                chooser.label()
            ));
        }
        if self.font_browser.has_keys {
            ui.label(RichText::new("Up and down choose a font; Esc gives the keys back to the page.").weak());
        }
        self.font_status(ui);
        let hidden = self.hidden_duplicates().len();
        if ui.checkbox(&mut self.settings.show_duplicate_fonts, "Show duplicates").changed() {
            self.storage.save_settings(&self.settings);
        }
        if hidden > 0 {
            ui.label(RichText::new(format!("Hidden: {hidden}, each drawing the same as a font above.")).weak());
        }
        ui.separator();
        let fonts = self.browser_fonts();
        let chosen = self.browser_chosen(&fonts);
        let scroll = std::mem::take(&mut self.font_browser.scroll);
        let mut clicked = None;
        ScrollArea::vertical().id_salt("font-browser").show(ui, |ui| {
            let titles = std::iter::once("Bedstead (bundled)").chain(fonts.iter().map(|font| font.title.as_str()));
            for (entry, title) in titles.enumerate() {
                let response = ui.selectable_label(chosen == Some(entry), title);
                if scroll && chosen == Some(entry) {
                    response.scroll_to_me(None);
                }
                if response.clicked() {
                    // The list keeps the keys, not the clicked label.
                    response.surrender_focus();
                    clicked = Some(entry);
                }
            }
            self.library_states(ui);
        });
        if let Some(entry) = clicked {
            self.font_browser.has_keys = true;
            self.browser_choose(entry, &fonts);
        }
    }

    /// Lists libraries not yet listed, and says which are listing or failed.
    fn library_states(&mut self, ui: &mut Ui) {
        if self.settings.libraries.is_empty() {
            ui.label("Add a font library in Menu, Fonts, to list its fonts here.");
        }
        for folder in self.settings.libraries.clone() {
            match self.fonts.library(&folder) {
                None => self.fonts.list_library(&folder, &self.options, &self.wake),
                Some(Library::Loading) => {
                    ui.label(format!("Listing {folder}"));
                }
                Some(Library::Failed(why)) => {
                    ui.colored_label(super::WARNING, format!("{folder}: {why}"));
                }
                Some(Library::Listed(..)) => {}
            }
        }
    }
}
