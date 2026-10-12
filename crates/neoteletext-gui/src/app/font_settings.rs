//! The window's fonts (area FNT): choosing the font for each tab's page
//! (REQ-FNT-005), the settings for the user's font, libraries, the
//! override, fonts from other sites and the cache (REQ-BRW-007), and the
//! status line's naming of the font in use.

use super::BrowserApp;
use crate::fonts::{Library, Request, State, Work, update_tab, user_request};
use eframe::egui::{self, RichText, TextEdit, Ui};
use neoteletext_browser::charsets::set_label;
use neoteletext_browser::fonts::{Chooser, Scope};
use neoteletext_browser::saved::UserFont;
use neoteletext_core::glyphs::LoadedFont;
use neoteletext_core::glyphs::conformance::{FONT_SET_COUNT, conformance_masks};
use neoteletext_web::sha256::{self, Digest, to_hex};
use neoteletext_web::url::Url;

/// What the Fonts panel is typing and waiting for.
#[derive(Default)]
pub struct FontPanel {
    /// The address typed for a font.
    address: String,
    /// The address typed for a library.
    library: String,
    /// The path typed for a local file.
    path: String,
    /// The font being chosen, its title, and the sets it is for, until it
    /// has loaded.
    choosing: Option<(Request, String, Target)>,
    /// Why the last choice failed.
    problem: Option<String>,
}

/// The character sets a font is chosen for (REQ-FNT-005, decision E.75
/// item 6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    /// One set, by its place in `FONT_SETS`: chosen under it in the font
    /// browser.
    Set(usize),
    /// Every set the font conforms for (REQ-FNT-011): chosen by address,
    /// from a file or from a library in the Fonts settings.
    Conforming,
}

fn hex(sha256: &Digest) -> String {
    String::from_utf8_lossy(&to_hex(sha256)).into_owned()
}

impl BrowserApp {
    /// Loads and chooses the fonts of every tab, and sends each decoder
    /// the fonts its page is drawn with when they change (REQ-FNT-005,
    /// REQ-FNT-008). Called every frame.
    pub(super) fn update_fonts(&mut self, now: u64) {
        let changed = self.fonts.poll(&self.storage, &self.options, &self.wake, now);
        self.finish_choice();
        let users = self.settings.fonts.clone().map(|font| font.as_ref().and_then(user_request));
        let work = Work { storage: &self.storage, options: &self.options, wake: &self.wake, now };
        for tab in &mut self.tabs {
            update_tab(tab, &mut self.fonts, &self.settings, &users, &work);
        }
        if changed {
            self.shown = None;
        }
    }

    /// The status line's naming of the font drawing the page, and who
    /// chose it (REQ-FNT-005).
    #[must_use]
    pub fn font_label(&self) -> String {
        match &self.tab().font_shown {
            Some((name, chooser)) => format!("{name} ({})", chooser.label()),
            None => "Bedstead (the bundled font)".to_string(),
        }
    }

    /// Makes a font the user chose theirs once it has loaded: for its one
    /// set, or for every set it conforms for.
    fn finish_choice(&mut self) {
        let Some((request, title, target)) = &self.font_panel.choosing else { return };
        match self.fonts.state(request) {
            Some(State::Ready(sha256)) => {
                let font = self.fonts.font(sha256);
                let title = if title.is_empty() {
                    font.map(|font| font.name.clone()).unwrap_or_default()
                } else {
                    title.clone()
                };
                let chosen = UserFont {
                    sha256: hex(sha256),
                    address: request.address.clone(),
                    title,
                    by_address: request.sha256.is_none(),
                };
                let sets = match *target {
                    Target::Set(place) => 1u32.checked_shl(u32::try_from(place).unwrap_or(u32::MAX)).unwrap_or(0),
                    Target::Conforming => font
                        .and_then(|font| LoadedFont::new(&font.glyphs).ok())
                        .map_or(0, |table| conformance_masks(&table).0),
                };
                self.font_panel.problem = (sets == 0)
                    .then(|| "it conforms for no character set; choose it under a set in the font browser".to_string());
                for (place, slot) in self.settings.fonts.iter_mut().enumerate() {
                    if (sets >> place) & 1 == 1 {
                        *slot = Some(chosen.clone());
                    }
                }
                self.storage.save_settings(&self.settings);
                self.font_panel.choosing = None;
            }
            Some(State::Failed(why)) => {
                self.font_panel.problem = Some(why.clone());
                self.font_panel.choosing = None;
            }
            _ => {}
        }
    }

    /// The settings, as the user last set them.
    #[must_use]
    pub const fn settings(&self) -> &neoteletext_browser::saved::Settings {
        &self.settings
    }

    /// Chooses a font for some sets; it becomes the user's once it has
    /// loaded.
    pub(super) fn choose(&mut self, request: Request, title: String, target: Target) {
        self.font_panel.problem = None;
        self.fonts.want(&request, &self.storage, &self.options, &self.wake, crate::now_seconds());
        self.font_panel.choosing = Some((request, title, target));
    }

    /// Clears the user's font for one set, or for every set, so those sets
    /// are drawn in Bedstead.
    pub(super) fn use_bedstead(&mut self, place: Option<usize>) {
        self.font_panel.choosing = None;
        self.font_panel.problem = None;
        for (index, slot) in self.settings.fonts.iter_mut().enumerate() {
            if place.is_none_or(|place| place == index) {
                *slot = None;
            }
        }
        self.storage.save_settings(&self.settings);
    }

    /// The SHA-256, in hexadecimal, of the font being chosen for a set or
    /// else of the user's font for it; none for Bedstead.
    pub(super) fn wanted_font(&self, place: usize) -> Option<String> {
        if let Some((request, _, Target::Set(target))) = &self.font_panel.choosing
            && *target == place
            && let Some(sha256) = &request.sha256
        {
            return Some(hex(sha256));
        }
        self.settings.fonts.get(place).cloned().flatten().map(|font| font.sha256)
    }

    /// The font being loaded, or why the last choice failed.
    pub(super) fn font_status(&self, ui: &mut Ui) {
        if let Some((request, title, _)) = &self.font_panel.choosing {
            ui.label(format!("Loading {}", if title.is_empty() { &request.address } else { title }));
        }
        if let Some(problem) = &self.font_panel.problem {
            ui.colored_label(super::WARNING, format!("Not used: {problem}"));
        }
    }

    /// The Fonts panel (REQ-BRW-007).
    pub(super) fn font_panel(&mut self, ui: &mut Ui) {
        ui.heading("Fonts");
        ui.label("Pages are drawn in Bedstead unless you, the channel or the page choose another 12 x 10 font.");
        ui.separator();
        // Your font for each character set (REQ-FNT-005, decision E.75).
        ui.label(RichText::new("Your fonts, by character set").strong());
        let chosen: Vec<(usize, String)> = (0..FONT_SET_COUNT)
            .filter_map(|place| Some((place, self.settings.fonts.get(place)?.as_ref()?.title.clone())))
            .collect();
        if chosen.is_empty() {
            ui.label("Bedstead for every set.");
        }
        for (place, title) in &chosen {
            ui.label(format!("{}: {title}", set_label(*place)));
        }
        if !chosen.is_empty() && ui.button("Use Bedstead for every set").clicked() {
            self.use_bedstead(None);
        }
        self.font_status(ui);
        if ui.checkbox(&mut self.settings.always_my_font, "Always use my fonts").changed()
            | ui.checkbox(&mut self.settings.fonts_from_other_sites, "Allow fonts from other sites").changed()
        {
            self.storage.save_settings(&self.settings);
        }
        ui.separator();
        let label = ui.label("A font's address");
        ui.add(TextEdit::singleline(&mut self.font_panel.address).hint_text("https://example.org/font.bdf"))
            .labelled_by(label.id);
        if ui.button("Use this font").clicked() {
            let address = self.font_panel.address.trim().to_string();
            match Url::parse(&address) {
                Ok(url) if !url.scheme().is_websocket() => {
                    let request = Request { address, sha256: None, chooser: Chooser::User, scope: Scope::Shared };
                    self.choose(request, String::new(), Target::Conforming);
                }
                _ => self.font_panel.problem = Some("not an http or https address".into()),
            }
        }
        let label = ui.label("A local file");
        ui.add(TextEdit::singleline(&mut self.font_panel.path).hint_text("/path/to/font.bdf")).labelled_by(label.id);
        if ui.button("Use this file").clicked() {
            self.use_local_file();
        }
        ui.separator();
        self.library_section(ui);
        ui.separator();
        self.cache_section(ui);
    }

    fn use_local_file(&mut self) {
        let path = self.font_panel.path.trim().to_string();
        let limit = neoteletext_browser::fonts::FONT_LIMIT_BYTES;
        let bytes = match std::fs::metadata(&path) {
            Ok(metadata) if metadata.len() <= limit => std::fs::read(&path).map_err(|error| error.to_string()),
            Ok(_) => Err("larger than 256 kilobytes".into()),
            Err(error) => Err(error.to_string()),
        };
        match bytes {
            Ok(bytes) => {
                let request = Request {
                    address: path,
                    sha256: Some(sha256::digest(&bytes)),
                    chooser: Chooser::User,
                    scope: Scope::Shared,
                };
                self.font_panel.problem = None;
                self.fonts.read_local(&request, bytes, &self.options, &self.wake);
                self.font_panel.choosing = Some((request, String::new(), Target::Conforming));
            }
            Err(problem) => self.font_panel.problem = Some(problem),
        }
    }

    fn library_section(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Font libraries").strong());
        let label = ui.label("A library's address");
        ui.add(TextEdit::singleline(&mut self.font_panel.library).hint_text("https://example.org/fonts/"))
            .labelled_by(label.id);
        if ui.button("Add library").clicked() {
            let mut folder = self.font_panel.library.trim().to_string();
            if !folder.ends_with('/') {
                folder.push('/');
            }
            if Url::parse(&folder).is_ok() && !self.settings.libraries.contains(&folder) {
                self.settings.libraries.push(folder.clone());
                self.storage.save_settings(&self.settings);
                self.fonts.list_library(&folder, &self.options, &self.wake);
            }
        }
        // The font browser's choice of hiding duplicates applies here too.
        let hidden = self.hidden_duplicates();
        let mut chosen = None;
        let mut remove = None;
        for folder in self.settings.libraries.clone() {
            ui.horizontal(|ui| {
                if ui.small_button("x").clicked() {
                    remove = Some(folder.clone());
                }
                ui.label(&folder);
            });
            match self.fonts.library(&folder) {
                None => {
                    // Listed when the panel is first shown; listing fetches
                    // no font (REQ-FNT-004).
                    self.fonts.list_library(&folder, &self.options, &self.wake);
                }
                Some(Library::Loading) => {
                    ui.label("Listing…");
                }
                Some(Library::Failed(why)) => {
                    ui.colored_label(super::WARNING, why.as_str());
                }
                Some(Library::Listed(fonts, ignored)) => {
                    for font in fonts.iter().filter(|font| !hidden.contains(&font.sha256)) {
                        if ui.link(&font.title).clicked() {
                            chosen = Some(font.clone());
                        }
                    }
                    if *ignored > 0 {
                        ui.label(format!("{ignored} lines of its fonts.txt were ignored."));
                    }
                }
            }
        }
        if let Some(folder) = remove {
            self.settings.libraries.retain(|known| *known != folder);
            self.storage.save_settings(&self.settings);
        }
        if let Some(font) = chosen {
            let request = Request {
                address: font.address,
                sha256: Some(font.sha256),
                chooser: Chooser::User,
                scope: Scope::Shared,
            };
            self.choose(request, font.title, Target::Conforming);
        }
    }

    fn cache_section(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Cached fonts").strong());
        let entries = self.fonts.cache().entries().to_vec();
        if entries.is_empty() {
            ui.label("None.");
        }
        egui::ScrollArea::vertical().id_salt("font-cache").max_height(240.0).show(ui, |ui| {
            for entry in &entries {
                ui.group(|ui| {
                    ui.label(RichText::new(&entry.name).strong());
                    ui.label(&entry.address);
                    if !entry.notice.is_empty() {
                        ui.label(&entry.notice);
                    }
                    let whose = match &entry.scope {
                        Scope::Shared => "yours".to_string(),
                        Scope::Channel(folder) => format!("for {folder}"),
                    };
                    ui.label(format!("{} bytes, {whose}", entry.size));
                    ui.horizontal(|ui| {
                        let mut pinned = entry.pinned;
                        if ui.checkbox(&mut pinned, "Keep").changed() {
                            self.fonts.set_pinned(&entry.sha256, &entry.scope, pinned, &self.storage);
                        }
                        if ui.small_button("Remove").clicked() {
                            self.fonts.remove(&entry.sha256, &entry.scope, &self.storage);
                        }
                    });
                });
            }
        });
        if !entries.is_empty() && ui.button("Remove every cached font").clicked() {
            self.fonts.clear(&self.storage);
        }
    }
}
