//! The developer tools panel at the bottom of the window, in the manner of
//! a web browser's: tabs of facts about the page shown. "Character sets"
//! reports how the page's character sets were chosen and which font draws
//! each (REQ-BRW-011, decision E.75); the second tab is not used yet.

use super::BrowserApp;
use eframe::egui::{Grid, RichText, ScrollArea, Ui};
use neoteletext_browser::charsets::{group_name, set_label};
use neoteletext_core::consts::DESIGNATION_GROUP_SHIFT;
use neoteletext_core::level1::{DesignationSource, NationalSource};

/// Bits of the national option index.
const NATIONAL_BITS: usize = DESIGNATION_GROUP_SHIFT as usize;
/// Bits of the group.
const GROUP_BITS: usize = 4;
/// The panel's height when first shown, in points.
pub(super) const PANEL_HEIGHT: f32 = 240.0;

/// The panel's tabs.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum DevTab {
    /// How the page's character sets were chosen, and their fonts.
    #[default]
    CharacterSets,
    /// Not used yet.
    Unused,
}

/// The panel's state.
#[derive(Default)]
pub struct DevTools {
    /// The panel is shown.
    pub open: bool,
    /// The tab shown.
    tab: DevTab,
}

const fn source_text(source: DesignationSource) -> &'static str {
    match source {
        DesignationSource::PageX28Format1 => "the page's packet X/28/0 Format 1",
        DesignationSource::PageX28Format4 => "the page's packet X/28/4",
        DesignationSource::MagazineM29Format0 => "the magazine's packet M/29/0",
        DesignationSource::MagazineM29Format4 => "the magazine's packet M/29/4",
        DesignationSource::Receiver => "the receiver's setting (the page sends none: Level 1)",
    }
}

const fn national_text(source: NationalSource) -> &'static str {
    match source {
        NationalSource::Header => "C12 to C14 of the page header",
        NationalSource::Designation => "bits 8 to 10 of the designation (Levels 2.5 and 3.5)",
        NationalSource::Fixed => "the receiver's fixed setting",
    }
}

impl BrowserApp {
    /// Shows or hides the panel.
    pub(super) fn toggle_dev_tools(&mut self) {
        self.dev_tools.open = !self.dev_tools.open;
    }

    /// The panel.
    pub(super) fn dev_tools(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.strong("Developer tools");
            ui.separator();
            ui.selectable_value(&mut self.dev_tools.tab, DevTab::CharacterSets, "Character sets");
            ui.selectable_value(&mut self.dev_tools.tab, DevTab::Unused, "Unused");
        });
        ui.separator();
        ScrollArea::vertical().id_salt("dev-tools").show(ui, |ui| match self.dev_tools.tab {
            DevTab::CharacterSets => self.character_sets_tab(ui),
            DevTab::Unused => {
                ui.label("Unused");
            }
        });
    }

    /// Every fact the decoder reported about the page's character sets, and
    /// the font that draws each set the page uses.
    fn character_sets_tab(&self, ui: &mut Ui) {
        let Some(status) = &self.tab().status else {
            ui.label("No page is shown.");
            return;
        };
        let Some(d) = status.designation else {
            ui.label(format!("P{:03X}: no designation reported (the page is not shown yet).", status.page));
            return;
        };
        ui.label(RichText::new(format!("P{:03X}", status.page)).strong());
        Grid::new("designation").num_columns(2).striped(true).show(ui, |ui| {
            let mut row = |name: &str, value: String| {
                ui.label(name);
                ui.label(value);
                ui.end_row();
            };
            row("National option bits (X/0 C12 C13 C14)", format!("{:0NATIONAL_BITS$b}", d.header_national));
            row("Group", format!("{:0GROUP_BITS$b} ({})", d.group, group_name(d.group)));
            row("Group from", source_text(d.source).to_string());
            row("National option used", format!("{:0NATIONAL_BITS$b}", d.national));
            row("National option from", national_text(d.national_source).to_string());
            row("Table 32 value", format!("{:0GROUP_BITS$b} {:0NATIONAL_BITS$b} (0x{:02X})", d.group, d.national, d.value));
            row("Reserved entry, fallback used (REQ-CHR-003)", if d.fallback { "yes".into() } else { "no".into() });
            row("Default G0 character set", set_label(usize::from(d.g0)).to_string());
            row("Default G2 character set", set_label(usize::from(d.g2)).to_string());
            row("Second G0 character set (Table 33)", d.second.map_or("none".into(), |second| set_label(usize::from(second)).to_string()));
            row("Rows with ESC (switch to the second G0 character set)", d.esc_rows.to_string());
            row("X/26 Modified Designations applied (Table 34)", d.x26_designations.to_string());
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Character sets used and their fonts").strong());
        let mut used = vec![("default G0", usize::from(d.g0)), ("default G2", usize::from(d.g2))];
        if let Some(second) = d.second {
            used.push(("second G0", usize::from(second)));
        }
        Grid::new("sets-used").num_columns(4).striped(true).show(ui, |ui| {
            ui.strong("Role");
            ui.strong("Character set");
            ui.strong("Drawn in");
            ui.strong("Standing");
            ui.end_row();
            for (role, place) in used {
                let (font, standing) = self.font_for(place);
                ui.label(role);
                ui.label(set_label(place));
                ui.label(font);
                ui.label(standing);
                ui.end_row();
            }
        });
        if d.x26_designations > 0 {
            ui.label(
                RichText::new("The character sets the X/26 designations named are not reported yet; only their number is.")
                    .weak(),
            );
        }
    }

    /// The font drawing a set on this tab's page, and its standing for the
    /// set: the one font chosen (every embedded font draws all 24 sets), or
    /// Bedstead.
    /// Bedstead's standing for a set, in words, from its conformance
    /// (REQ-FNT-011).
    fn bedstead_standing(&self, place: usize) -> String {
        let _ = self;
        let mut table = crate::sample::bedstead_glyphs(neoteletext_core::glyphs::GlyphSet::bedstead().entries());
        table.truncate(neoteletext_core::consts::FONT_MAX_CHARACTERS);
        let (sets, cell) = neoteletext_core::glyphs::LoadedFont::new(&table)
            .map_or((0, 0), |font| neoteletext_core::glyphs::conformance::conformance_masks(&font));
        match ((sets >> place) & 1 == 1, (cell >> place) & 1 == 1) {
            (true, true) => "conforms, SAA5050 cell".into(),
            (true, false) => "conforms".into(),
            _ => "incomplete: missing characters show as hollow boxes".into(),
        }
    }

    fn font_for(&self, place: usize) -> (String, String) {
        match crate::embedded::find(&self.settings.embedded_font) {
            Some(font) => (font.title.to_string(), "complete: draws every character set".to_string()),
            None => ("Bedstead (bundled)".to_string(), self.bedstead_standing(place)),
        }
    }
}
