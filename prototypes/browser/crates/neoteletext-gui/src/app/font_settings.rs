//! The window's font (area FNT): one font for every page, chosen from a
//! single dropdown of Bedstead, the bundled font and the default, and the
//! fonts embedded for testing (`crate::embedded`). There is no font per
//! character set (owner, 14 October 2026).

use super::BrowserApp;
use crate::embedded::{EMBEDDED, EmbeddedFont};
use crate::fonts::update_tab;
use eframe::egui::{self, Ui};

/// The dropdown's name for Bedstead.
const BEDSTEAD: &str = "Bedstead (bundled)";
/// The font dropdown's width, in points.
pub const FONT_MENU_WIDTH: f32 = 200.0;

impl BrowserApp {
    /// Sends each tab's decoder the chosen font when it changes. Called
    /// every frame.
    pub(super) fn update_fonts(&mut self, _now: u64) {
        for tab in &mut self.tabs {
            update_tab(tab, &self.settings);
        }
    }

    /// The status line's naming of the font drawing the page.
    #[must_use]
    pub fn font_label(&self) -> String {
        match &self.tab().font_shown {
            Some((name, _)) => name.clone(),
            None => "Bedstead (the bundled font)".to_string(),
        }
    }

    /// The settings, as the user last set them.
    #[must_use]
    pub const fn settings(&self) -> &neoteletext_browser::saved::Settings {
        &self.settings
    }

    /// The font dropdown: Bedstead, then every embedded font, non-free ones
    /// marked.
    pub(super) fn font_menu(&mut self, ui: &mut Ui) {
        let current = crate::embedded::find(&self.settings.embedded_font).map_or_else(|| BEDSTEAD.to_string(), label);
        let mut choice = self.settings.embedded_font.clone();
        egui::ComboBox::from_id_salt("font")
            .selected_text(current)
            .width(FONT_MENU_WIDTH)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut choice, String::new(), BEDSTEAD);
                for font in EMBEDDED {
                    ui.selectable_value(&mut choice, font.id.to_string(), label(font));
                }
            })
            .response
            .on_hover_text("The one font every page is drawn with");
        if choice != self.settings.embedded_font {
            self.settings.embedded_font = choice;
            self.storage.save_settings(&self.settings);
            self.shown = None;
        }
    }
}

/// A font's name in the dropdown.
fn label(font: &EmbeddedFont) -> String {
    format!("{}{}", font.title, if font.free { "" } else { " (non-free)" })
}
