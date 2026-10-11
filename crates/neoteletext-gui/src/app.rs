//! The window (`docs/desktop-browser.md` section 9): the tab strip, the
//! toolbar with the address bar and the Fastext buttons, the trusted bars
//! that ask for confirmation, the page area, the side panel of bookmarks
//! and history, and the keys of `docs/web-native-concept.md` section 7.
//!
//! Everything here is chrome the page cannot reach: it is drawn by egui
//! outside the page area, and row 0 inside it is drawn from the
//! interface's own knowledge (`picture`).

use crate::helpers::{Options, Wake};
use crate::picture::{self, Picture};
use crate::storage::Storage;
use crate::tab::{Tab, View};
use crate::{GLYPHS, now_seconds};
use eframe::egui::{
    self, Align, Button, CentralPanel, Color32, ColorImage, Context, Event, Key, Layout, Modifiers, Panel, RichText,
    Sense, TextEdit, TextureHandle, TextureOptions, Ui, ViewportCommand,
};
use neoteletext_app::pipe::{DisplaySettings, UserInput};
use neoteletext_browser::location::parse_input;
use neoteletext_browser::pages::START_LIST_ROW;
use neoteletext_browser::saved::{Saved, Settings};
use neoteletext_browser::tuning::Confirmed;
use neoteletext_browser::{Destination, Location, Why};
use neoteletext_core::consts::{COLUMNS, ROWS};
use std::collections::HashMap;
use std::time::Duration;

/// How often the window wakes with nothing else happening: the clock in
/// row 0 changes every second.
const CLOCK_TICK: Duration = Duration::from_millis(250);
/// The 4:3 picture of a television stretches a page's height by this much
/// (REQ-RND-004).
const ASPECT_STRETCH: f32 = 1.2;
/// The most tabs open at once; each has two processes.
const MAX_TABS: usize = 32;
/// The most visits listed in the side panel.
const HISTORY_LISTED: usize = 200;
/// The Fastext colours, red, green, yellow, cyan, as on a remote.
const FASTEXT: [(Color32, &str); 4] = [
    (Color32::from_rgb(0xD0, 0x20, 0x20), "Red (r, F1)"),
    (Color32::from_rgb(0x20, 0xB0, 0x20), "Green (g, F2)"),
    (Color32::from_rgb(0xD0, 0xC0, 0x20), "Yellow (y, F3)"),
    (Color32::from_rgb(0x20, 0xB0, 0xC0), "Cyan (c, F4)"),
];
/// The background of the bars that ask the user to confirm.
const CONFIRM_FILL: Color32 = Color32::from_rgb(0x40, 0x38, 0x00);
/// The colour of the warning that a connection is not encrypted.
const WARNING: Color32 = Color32::from_rgb(0xFF, 0x60, 0x60);

/// What the side panel shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    None,
    Bookmarks,
    History,
}

/// The browser window's state.
pub struct BrowserApp {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    address: String,
    editing: bool,
    focus_address: bool,
    confirmed: Confirmed,
    options: Options,
    wake: Wake,
    storage: Storage,
    settings: Settings,
    bookmarks: Vec<Saved>,
    visits: Vec<Saved>,
    recorded: HashMap<u64, (String, u16)>,
    side: Side,
    /// What the pointer is over on the page: a page number's destination.
    hovered: Option<String>,
    texture: Option<TextureHandle>,
    shown: Option<(u64, u64, String, u8, usize)>,
    picture: Option<Picture>,
    title: String,
}

/// Which of `cells` equal cells across `extent` points `offset` falls in.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is checked to be within 0 and the cell count, at most a few hundred, before the conversion"
)]
fn cell_index(offset: f32, extent: f32, cells: usize) -> Option<usize> {
    let count = f32::from(u16::try_from(cells).ok()?);
    let cell = (offset / extent * count).floor();
    if extent > 0.0 && (0.0..count).contains(&cell) { Some(cell as usize) } else { None }
}

/// The local time for row 0's clock.
fn clock() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

impl BrowserApp {
    /// A window with the tabs of the last session, waiting at their tuning
    /// screens, and then the addresses given on the command line.
    #[must_use]
    pub fn new(context: &Context, options: Options, storage: Storage, addresses: &[String]) -> Self {
        let wake_context = context.clone();
        let wake: Wake = std::sync::Arc::new(move || wake_context.request_repaint());
        let mut app = Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            address: String::new(),
            editing: false,
            focus_address: false,
            confirmed: Confirmed::default(),
            options,
            wake,
            settings: storage.settings(),
            bookmarks: storage.bookmarks(),
            visits: storage.history(now_seconds()),
            storage,
            recorded: HashMap::new(),
            side: Side::None,
            hovered: None,
            texture: None,
            shown: None,
            picture: None,
            title: String::new(),
        };
        for saved in app.storage.tabs() {
            app.new_tab();
            app.go(saved.destination, Why::Restored);
        }
        for address in addresses {
            app.new_tab();
            match parse_input(address) {
                Ok(destination) => app.go(destination, Why::Typed),
                Err(error) => app.tab_mut().notice = Some(format!("{address}: {error:?}")),
            }
        }
        if app.tabs.is_empty() {
            app.new_tab();
        } else {
            // A tab opened at an address waits for Enter on its tuning
            // screen, not in the address bar.
            app.focus_address = false;
        }
        app.active = 0;
        app
    }

    fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    /// The tab shown now.
    #[must_use]
    pub fn active_tab(&self) -> &Tab {
        self.tab()
    }

    /// The number of tabs open.
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    fn new_tab(&mut self) {
        if self.tabs.len() >= MAX_TABS {
            return;
        }
        let mut tab = Tab::new(self.next_id);
        tab.set_settings(DisplaySettings { glyphs: self.settings.glyphs });
        self.next_id += 1;
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.editing = false;
        self.focus_address = true;
    }

    fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(index);
        self.recorded.remove(&tab.id);
        if self.tabs.is_empty() {
            self.new_tab();
        }
        self.active = self.active.min(self.tabs.len() - 1);
        self.editing = false;
    }

    /// Goes somewhere in the active tab.
    fn go(&mut self, destination: Destination, why: Why) {
        let (confirmed, options, wake) = (&self.confirmed, &self.options, &self.wake);
        self.tabs[self.active].navigate(destination, why, confirmed, options, wake);
        self.editing = false;
    }

    fn confirm(&mut self) {
        let (confirmed, options, wake) = (&mut self.confirmed, &self.options, &self.wake);
        self.tabs[self.active].confirm(confirmed, options, wake);
    }

    fn back(&mut self) {
        let (confirmed, options, wake) = (&self.confirmed, &self.options, &self.wake);
        self.tabs[self.active].back(confirmed, options, wake);
    }

    fn forward(&mut self) {
        let (confirmed, options, wake) = (&self.confirmed, &self.options, &self.wake);
        self.tabs[self.active].forward(confirmed, options, wake);
    }

    fn reload(&mut self) {
        let tab = self.tab_mut();
        match (&tab.view, tab.location.clone()) {
            (View::Failed(..), Some(location)) => {
                let page = tab.navigation.current().map(|entry| entry.page);
                let (options, wake) = (&self.options, &self.wake);
                self.tabs[self.active].open(&Destination { location, page }, options, wake);
            }
            _ => tab.reload(),
        }
    }

    fn set_glyphs(&mut self, glyphs: u8) {
        self.settings.glyphs = glyphs;
        for tab in &mut self.tabs {
            tab.set_settings(DisplaySettings { glyphs });
        }
        self.storage.save_settings(&self.settings);
    }

    /// The address and page shown now, as a bookmark or a history entry.
    fn current_saved(&self, time: u64) -> Option<Saved> {
        let tab = self.tab();
        let entry = tab.navigation.current()?;
        let title = if entry.title.is_empty() { tab.title() } else { entry.title.clone() };
        Some(Saved {
            time,
            title,
            destination: Destination { location: entry.location.clone(), page: Some(entry.page) },
        })
    }

    fn bookmark(&mut self) {
        if let Some(saved) = self.current_saved(0) {
            if !self.bookmarks.iter().any(|bookmark| bookmark.destination == saved.destination) {
                self.bookmarks.push(saved);
                self.storage.save_bookmarks(&self.bookmarks);
            }
            self.tab_mut().notice = Some("Bookmarked.".to_string());
        }
    }

    /// Records each tab's newly shown page in the history of visits.
    fn record_visits(&mut self) {
        let now = now_seconds();
        let mut changed = false;
        for tab in &self.tabs {
            let Some(entry) = tab.navigation.current() else { continue };
            let key = (entry.location.site_key(), entry.page);
            if self.recorded.get(&tab.id) == Some(&key) {
                continue;
            }
            self.recorded.insert(tab.id, key);
            self.visits.push(Saved {
                time: now,
                title: entry.title.clone(),
                destination: Destination { location: entry.location.clone(), page: Some(entry.page) },
            });
            changed = true;
        }
        if changed {
            self.visits = neoteletext_browser::saved::prune_history(std::mem::take(&mut self.visits), now);
        }
    }

    /// Saves what is kept between runs.
    pub fn save_session(&self) {
        let tabs: Vec<Saved> = self
            .tabs
            .iter()
            .filter_map(|tab| {
                let entry = tab.navigation.current()?;
                Some(Saved {
                    time: 0,
                    title: entry.title.clone(),
                    destination: Destination { location: entry.location.clone(), page: Some(entry.page) },
                })
            })
            .collect();
        self.storage.save_tabs(&tabs);
        self.storage.save_history(&self.visits);
        self.storage.save_settings(&self.settings);
    }

    fn sync_address(&mut self) {
        if self.editing {
            return;
        }
        let tab = self.tab();
        self.address = match &tab.view {
            View::Tuning(tuning) => tuning.destination.location.address(tuning.destination.page),
            _ => match (&tab.location, tab.navigation.current()) {
                (Some(location), Some(entry)) if entry.location == *location => location.address(Some(entry.page)),
                (Some(location), _) => location.address(None),
                (None, _) => String::new(),
            },
        };
    }

    // -----------------------------------------------------------------
    // Keys
    // -----------------------------------------------------------------

    fn command_key(&mut self, context: &Context, key: Key, modifiers: Modifiers) -> bool {
        match key {
            Key::T => self.new_tab(),
            Key::W => self.close_tab(self.active),
            Key::L => self.focus_address = true,
            Key::D => self.bookmark(),
            Key::R => self.reload(),
            Key::OpenBracket | Key::ArrowLeft => self.back(),
            Key::CloseBracket | Key::ArrowRight => self.forward(),
            Key::Q => context.send_viewport_cmd(ViewportCommand::Close),
            Key::C if modifiers.shift => self.copy_text(context),
            Key::Tab if modifiers.shift => self.active = (self.active + self.tabs.len() - 1) % self.tabs.len(),
            Key::Tab => self.active = (self.active + 1) % self.tabs.len(),
            _ => {
                let index = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8]
                    .iter()
                    .position(|&number| number == key);
                match (index, key) {
                    (Some(index), _) if index < self.tabs.len() => self.active = index,
                    (_, Key::Num9) => self.active = self.tabs.len() - 1,
                    _ => return false,
                }
            }
        }
        self.editing = false;
        true
    }

    fn named_key(&mut self, context: &Context, key: Key) {
        let form = self.tab().status.as_ref().is_some_and(|status| status.form) && self.tab().view == View::Page;
        if let View::Tuning(_) = self.tab().view {
            match key {
                Key::Enter | Key::F4 => self.confirm(),
                Key::Escape | Key::Delete => self.tab_mut().cancel(),
                _ => {}
            }
            return;
        }
        if self.tab().submission.is_some() {
            match key {
                Key::Enter | Key::F4 => self.tab_mut().answer_submission(true),
                Key::Escape | Key::Delete => self.tab_mut().answer_submission(false),
                _ => {}
            }
            return;
        }
        let input = match key {
            Key::F1 => Some(UserInput::Colour(0)),
            Key::F2 => Some(UserInput::Colour(1)),
            Key::F3 => Some(UserInput::Colour(2)),
            Key::F4 => Some(UserInput::Colour(3)),
            Key::F5 => {
                self.reload();
                None
            }
            Key::F6 => {
                self.focus_address = true;
                None
            }
            Key::ArrowUp if form => Some(UserInput::Arrow(2)),
            Key::ArrowDown if form => Some(UserInput::Arrow(3)),
            Key::ArrowLeft if form => Some(UserInput::Arrow(0)),
            Key::ArrowRight if form => Some(UserInput::Arrow(1)),
            Key::ArrowUp => Some(UserInput::NextPage),
            Key::ArrowDown => Some(UserInput::PreviousPage),
            Key::ArrowRight => Some(UserInput::NextSubPage),
            Key::ArrowLeft => Some(UserInput::PreviousSubPage),
            // Enter confirms a form's field, or turns page catching on
            // and follows the selected link (concept section 7).
            Key::Enter => Some(UserInput::Enter),
            Key::Tab if self.tab().view == View::Page => Some(UserInput::Tab),
            Key::Backspace if form => Some(UserInput::Backspace),
            Key::Escape if form => Some(UserInput::Escape),
            Key::Home => Some(UserInput::Home),
            Key::End => Some(UserInput::Index),
            Key::Insert => Some(UserInput::Hold),
            Key::PageUp | Key::Backspace => {
                self.back();
                None
            }
            Key::PageDown => {
                self.forward();
                None
            }
            Key::Escape | Key::Delete if self.tab().view == View::Page => Some(UserInput::Escape),
            Key::Escape | Key::Delete => {
                self.tab_mut().cancel();
                None
            }
            _ => None,
        };
        if let Some(input) = input {
            self.tab_mut().input(input);
        }
        let _ = context;
    }

    fn text_key(&mut self, character: char) {
        let tab = self.tab();
        if !matches!(tab.view, View::Page | View::Start | View::Keys) || tab.submission.is_some() {
            return;
        }
        if tab.view == View::Page && tab.status.as_ref().is_some_and(|status| status.form) {
            self.tab_mut().input(UserInput::Char(character));
            return;
        }
        let input = match character {
            digit @ '0'..='9' => Some(UserInput::Digit(digit as u8 - b'0')),
            'r' => Some(UserInput::Colour(0)),
            'g' => Some(UserInput::Colour(1)),
            'y' => Some(UserInput::Colour(2)),
            'c' => Some(UserInput::Colour(3)),
            'i' => Some(UserInput::Index),
            'h' => Some(UserInput::Home),
            'H' => Some(UserInput::Hold),
            '+' => Some(UserInput::NextPage),
            '-' => Some(UserInput::PreviousPage),
            '?' | '*' => Some(UserInput::Reveal),
            'm' | '/' => Some(UserInput::Mix),
            'z' => Some(UserInput::Zoom),
            'l' => Some(UserInput::ShowLinks),
            'f' => {
                self.forward();
                None
            }
            'u' => {
                self.reload();
                None
            }
            'K' => {
                self.tab_mut().toggle_keys();
                None
            }
            'G' => {
                self.set_glyphs((self.settings.glyphs + 1) % 3);
                None
            }
            _ => None,
        };
        if let Some(input) = input {
            self.tab_mut().input(input);
        }
    }

    fn keys(&mut self, context: &Context) {
        if context.egui_wants_keyboard_input() {
            return;
        }
        let events = context.input(|input| input.events.clone());
        for event in events {
            match event {
                Event::Key { key, pressed: true, modifiers, .. } if modifiers.command => {
                    self.command_key(context, key, modifiers);
                }
                Event::Key { key, pressed: true, modifiers, .. } if modifiers.ctrl && key == Key::Tab => {
                    self.command_key(context, key, modifiers);
                }
                Event::Key { key, pressed: true, .. } => self.named_key(context, key),
                Event::Text(text) => {
                    for character in text.chars() {
                        self.text_key(character);
                    }
                }
                _ => {}
            }
        }
    }

    fn copy_text(&self, context: &Context) {
        if let Some(status) = &self.tab().status {
            context.copy_text(status.text.clone());
        }
    }

    fn dropped_files(&mut self, context: &Context) {
        let dropped = context.input(|input| input.raw.dropped_files.clone());
        for file in dropped {
            let path = file.path().to_string_lossy().into_owned();
            if let Ok(destination) = parse_input(&path) {
                self.new_tab();
                self.go(destination, Why::Chosen);
            }
        }
    }

    // -----------------------------------------------------------------
    // Drawing
    // -----------------------------------------------------------------

    fn tab_strip(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            let mut close = None;
            for (index, tab) in self.tabs.iter().enumerate() {
                let title: String = tab.title().chars().take(28).collect();
                let selected = index == self.active;
                if ui.selectable_label(selected, title).clicked() {
                    self.active = index;
                    self.editing = false;
                }
                if ui.small_button("x").on_hover_text("Close the tab (Cmd-W)").clicked() {
                    close = Some(index);
                }
                ui.separator();
            }
            if ui.button("+").on_hover_text("New tab (Cmd-T)").clicked() {
                self.new_tab();
            }
            if let Some(index) = close {
                self.close_tab(index);
            }
        });
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let can_back = self.tab().navigation.can_go_back();
            let can_forward = self.tab().navigation.can_go_forward();
            if ui.add_enabled(can_back, Button::new("<")).on_hover_text("Back (Cmd-[, Page Up)").clicked() {
                self.back();
            }
            if ui.add_enabled(can_forward, Button::new(">")).on_hover_text("Forward (Cmd-], Page Down)").clicked() {
                self.forward();
            }
            if ui.button("Reload").on_hover_text("Ask for the page again (Cmd-R, F5)").clicked() {
                self.reload();
            }
            if ui.button("Home").on_hover_text("The site's first page (h)").clicked() {
                self.tab_mut().input(UserInput::Home);
            }
            let tab = self.tab();
            let (security, colour) = match (&tab.view, &tab.location) {
                (View::Tuning(_) | View::Start | View::Keys, _) | (_, None) => ("", ui.visuals().text_color()),
                _ if tab.is_unencrypted() => ("Not encrypted", WARNING),
                (_, Some(Location::File { .. })) => ("File", ui.visuals().text_color()),
                _ => ("Encrypted", Color32::LIGHT_GREEN),
            };
            ui.label(RichText::new(security).color(colour));
            let fastext_width = 4.0 * 26.0 + 80.0;
            let width = (ui.available_width() - fastext_width).max(120.0);
            let response = ui.add(TextEdit::singleline(&mut self.address).desired_width(width).hint_text("Address"));
            if self.focus_address {
                response.request_focus();
                self.focus_address = false;
            }
            if response.gained_focus() {
                self.editing = true;
            }
            if response.lost_focus() {
                let entered = ui.input(|input| input.key_pressed(Key::Enter));
                self.editing = false;
                if entered {
                    match parse_input(&self.address) {
                        Ok(destination) => self.go(destination, Why::Typed),
                        Err(error) => self.tab_mut().notice = Some(format!("Not an address: {error:?}")),
                    }
                }
            }
            for (index, (colour, name)) in FASTEXT.iter().enumerate() {
                let button = Button::new("   ").fill(*colour);
                if ui.add(button).on_hover_text(*name).clicked() {
                    self.tab_mut().input(UserInput::Colour(u8::try_from(index).unwrap_or(0)));
                }
            }
            ui.menu_button("Menu", |ui| self.menu(ui));
        });
    }

    fn menu(&mut self, ui: &mut Ui) {
        if ui.button("New tab (Cmd-T)").clicked() {
            self.new_tab();
        }
        if ui.button("Close tab (Cmd-W)").clicked() {
            self.close_tab(self.active);
        }
        ui.separator();
        if ui.button("Bookmark this page (Cmd-D)").clicked() {
            self.bookmark();
        }
        if ui.button("Bookmarks").clicked() {
            self.side = if self.side == Side::Bookmarks { Side::None } else { Side::Bookmarks };
        }
        if ui.button("History").clicked() {
            self.side = if self.side == Side::History { Side::None } else { Side::History };
        }
        ui.separator();
        if ui.button("Copy the page's text (Cmd-Shift-C)").clicked() {
            self.copy_text(ui.ctx());
        }
        if ui.button("Copy the address").clicked() {
            ui.ctx().copy_text(self.address.clone());
        }
        ui.separator();
        ui.label("Glyphs (G)");
        for (value, name) in [(0, "The project's own"), (1, "Bedstead"), (2, "Both, side by side")] {
            if ui.radio(self.settings.glyphs == value, name).clicked() {
                self.set_glyphs(value);
            }
        }
        if ui.checkbox(&mut self.settings.aspect, "4:3 picture").changed() {
            self.storage.save_settings(&self.settings);
            self.shown = None;
        }
        ui.separator();
        if ui.button("Keys (K)").clicked() {
            self.tab_mut().toggle_keys();
        }
        if ui.button("Clear the history").clicked() {
            self.visits.clear();
            self.storage.save_history(&self.visits);
        }
        if ui.button("Quit (Cmd-Q)").clicked() {
            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn confirm_bar(&mut self, ui: &mut Ui) {
        let tab = self.tab();
        if let View::Tuning(tuning) = &tab.view {
            let label = tuning.label();
            let plain = !tuning.destination.location.is_secure();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Go to {label}?")).strong().color(Color32::YELLOW));
                if plain {
                    ui.label(RichText::new("Not encrypted.").color(WARNING));
                }
                if ui.button("Go (Enter)").clicked() {
                    self.confirm();
                }
                if ui.button("Stay (Escape)").clicked() {
                    self.tab_mut().cancel();
                }
            });
        } else if let Some(submission) = tab.submission.clone() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Send this form to {}?", submission.host)).strong().color(Color32::YELLOW),
                );
                if submission.plain_http {
                    ui.label(RichText::new("It is NOT encrypted: others can read it.").color(WARNING));
                }
                if ui.button("Send (Enter)").clicked() {
                    self.tab_mut().answer_submission(true);
                }
                if ui.button("Don't send (Escape)").clicked() {
                    self.tab_mut().answer_submission(false);
                }
            });
        }
    }

    fn needs_confirm_bar(&self) -> bool {
        matches!(self.tab().view, View::Tuning(_)) || self.tab().submission.is_some()
    }

    fn status_line(&self, ui: &mut Ui) {
        let tab = self.tab();
        let state = match (&tab.view, &tab.status) {
            (View::Page, Some(status)) if !status.found => format!("Searching for P{:03X}", status.page),
            (View::Page, None) => "Tuning in".to_string(),
            _ => String::new(),
        };
        let notice = self.hovered.clone().or_else(|| tab.notice.clone()).unwrap_or_default();
        let glyphs = ["own glyphs", "Bedstead", "both glyph sets"][usize::from(self.settings.glyphs.min(2))];
        ui.horizontal(|ui| {
            ui.label(state);
            ui.label(notice);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(glyphs));
        });
    }

    fn side_panel(&mut self, ui: &mut Ui) {
        let mut go = None;
        match self.side {
            Side::Bookmarks => {
                ui.heading("Bookmarks");
                let mut remove = None;
                for (index, bookmark) in self.bookmarks.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.small_button("x").clicked() {
                            remove = Some(index);
                        }
                        if ui.link(&bookmark.title).clicked() {
                            go = Some(bookmark.destination.clone());
                        }
                    });
                }
                if let Some(index) = remove {
                    self.bookmarks.remove(index);
                    self.storage.save_bookmarks(&self.bookmarks);
                }
            }
            Side::History => {
                ui.heading("History");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for visit in self.visits.iter().rev().take(HISTORY_LISTED) {
                        let page = visit.destination.page.map(|page| format!(" {page:03X}")).unwrap_or_default();
                        let text = format!("{}{page} - {}", visit.destination.location.label(), visit.title);
                        if ui.link(text).clicked() {
                            go = Some(visit.destination.clone());
                        }
                    }
                });
            }
            Side::None => {}
        }
        if let Some(destination) = go {
            self.go(destination, Why::Chosen);
        }
    }

    fn page_area(&mut self, ui: &mut Ui) {
        let clock = clock();
        let titles: Vec<String> = self.bookmarks.iter().map(|bookmark| bookmark.title.clone()).collect();
        let tab = self.tab();
        let key = (tab.id, tab.version, clock.clone(), self.settings.glyphs, titles.len());
        if self.shown.as_ref() != Some(&key) {
            let sets = GLYPHS[usize::from(self.settings.glyphs.min(2))];
            let picture = picture::for_tab(tab, &titles, Some(&clock), sets);
            let image = ColorImage::from_rgba_unmultiplied([picture.width, picture.height], &picture.rgba);
            match &mut self.texture {
                Some(texture) => texture.set(image, TextureOptions::NEAREST),
                None => self.texture = Some(ui.ctx().load_texture("page", image, TextureOptions::NEAREST)),
            }
            self.picture = Some(picture);
            self.shown = Some(key);
        }
        let (Some(texture), Some(picture)) = (&self.texture, &self.picture) else { return };
        let available = ui.available_size();
        let stretch = if self.settings.aspect { ASPECT_STRETCH } else { 1.0 };
        let width = f32::from(u16::try_from(picture.width).unwrap_or(u16::MAX));
        let height = f32::from(u16::try_from(picture.height).unwrap_or(u16::MAX)) * stretch;
        // Whole-number scaling keeps every teletext pixel the same size.
        let scale = (available.x / width).min(available.y / height).floor().max(1.0);
        let (rect, response) = ui.allocate_exact_size(available, Sense::click());
        let size = egui::vec2(width * scale, height * scale);
        let image_rect = egui::Rect::from_center_size(rect.center(), size);
        ui.painter().image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        // Screen readers get the page's text (design section 9.6).
        let text = self.tab().status.as_ref().map(|status| status.text.clone()).unwrap_or_default();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
        let columns = picture.width / (picture.width / COLUMNS).max(1);
        let cell = |position: egui::Pos2| {
            let column = cell_index(position.x - image_rect.left(), image_rect.width(), columns)?;
            let row = cell_index(position.y - image_rect.top(), image_rect.height(), ROWS)?;
            Some((row, column))
        };
        // A page number under the pointer says where it leads, as a web
        // browser shows a link's address (concept decision 11).
        self.hovered = response
            .hover_pos()
            .and_then(cell)
            .filter(|_| self.tab().view == View::Page)
            .and_then(|(row, column)| {
                let line = text.lines().nth(row)?;
                neoteletext_app::decoder::page_number_at(line, column)
            })
            .map(|page| format!("Go to P{:03X}", page.mpp()));
        if response.clicked()
            && let Some((row, column)) = response.interact_pointer_pos().and_then(cell)
        {
            self.click(row, column);
        }
    }

    fn click(&mut self, row: usize, column: usize) {
        match self.tab().view {
            View::Page => {
                if let (Ok(row), Ok(column)) = (u8::try_from(row), u8::try_from(column)) {
                    self.tab_mut().click(row, column);
                }
            }
            View::Start => {
                let chosen = row.checked_sub(START_LIST_ROW).and_then(|index| self.bookmarks.get(index)).cloned();
                if let Some(bookmark) = chosen {
                    self.go(bookmark.destination, Why::Chosen);
                }
            }
            _ => {}
        }
    }
}

impl eframe::App for BrowserApp {
    fn logic(&mut self, context: &Context, _frame: &mut eframe::Frame) {
        for tab in &mut self.tabs {
            tab.poll();
        }
        self.record_visits();
        self.dropped_files(context);
        self.keys(context);
        self.sync_address();
        let title = format!("{} - neoteletext", self.tab().title());
        if title != self.title {
            context.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        context.request_repaint_after(CLOCK_TICK);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        Panel::top("tabs").show(ui, |ui| self.tab_strip(ui));
        Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        if self.needs_confirm_bar() {
            Panel::top("confirm")
                .frame(egui::Frame::new().fill(CONFIRM_FILL).inner_margin(6.0))
                .show(ui, |ui| self.confirm_bar(ui));
        }
        Panel::bottom("status").show(ui, |ui| self.status_line(ui));
        if self.side != Side::None {
            Panel::right("side").default_size(260.0).show(ui, |ui| self.side_panel(ui));
        }
        CentralPanel::default().frame(egui::Frame::new().fill(Color32::BLACK)).show(ui, |ui| self.page_area(ui));
    }

    fn on_exit(&mut self) {
        self.save_session();
    }
}
