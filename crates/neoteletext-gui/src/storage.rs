//! Where the browser keeps what it saves (`docs/desktop-browser.md`
//! section 11): the platform's application data directory, only ever
//! written by the interface process. The formats are
//! `neoteletext_browser::saved`'s.

use neoteletext_browser::saved::{self, Saved, Settings};
use std::path::{Path, PathBuf};

/// The largest saved file read.
const MAX_FILE: u64 = 4 * 1024 * 1024;
const BOOKMARKS: &str = "bookmarks.tsv";
const HISTORY: &str = "history.tsv";
const TABS: &str = "tabs.tsv";
const SETTINGS: &str = "settings.txt";
/// The directory's name under the platform's data directory.
const APPLICATION: &str = "neoteletext";

/// The data directory, made on first use.
#[derive(Clone, Debug)]
pub struct Storage {
    directory: Option<PathBuf>,
}

/// The platform's data directory: Application Support on macOS,
/// `$XDG_DATA_HOME` or `~/.local/share` elsewhere, `%APPDATA%` on Windows.
fn platform_directory() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        return std::env::var_os("APPDATA").map(PathBuf::from);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home.map(|home| home.join("Library").join("Application Support"));
    }
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| home.map(|home| home.join(".local").join("share")))
}

fn read_bounded(path: &Path) -> String {
    let small = std::fs::metadata(path).is_ok_and(|metadata| metadata.len() <= MAX_FILE);
    if !small {
        return String::new();
    }
    std::fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default()
}

impl Storage {
    /// The platform's directory, or `directory` when given (the tests', or
    /// `--data`).
    #[must_use]
    pub fn new(directory: Option<PathBuf>) -> Self {
        Self { directory: directory.or_else(|| platform_directory().map(|base| base.join(APPLICATION))) }
    }

    fn path(&self, name: &str) -> Option<PathBuf> {
        self.directory.as_ref().map(|directory| directory.join(name))
    }

    fn load(&self, name: &str) -> String {
        self.path(name).map(|path| read_bounded(&path)).unwrap_or_default()
    }

    /// Writes a file whole: to a temporary name first, then renamed over
    /// the old one, so a crash never leaves half a file.
    fn store(&self, name: &str, text: &str) {
        let (Some(directory), Some(path)) = (&self.directory, self.path(name)) else { return };
        if std::fs::create_dir_all(directory).is_err() {
            return;
        }
        let temporary = directory.join(format!(".{name}.new"));
        if std::fs::write(&temporary, text).is_ok() {
            let _ = std::fs::rename(&temporary, &path);
        }
    }

    /// The bookmarks.
    #[must_use]
    pub fn bookmarks(&self) -> Vec<Saved> {
        saved::read(&self.load(BOOKMARKS))
    }

    /// Saves the bookmarks.
    pub fn save_bookmarks(&self, bookmarks: &[Saved]) {
        self.store(BOOKMARKS, &saved::write(bookmarks));
    }

    /// The history of visits, pruned to its retention.
    #[must_use]
    pub fn history(&self, now: u64) -> Vec<Saved> {
        saved::prune_history(saved::read(&self.load(HISTORY)), now)
    }

    /// Saves the history.
    pub fn save_history(&self, visits: &[Saved]) {
        self.store(HISTORY, &saved::write(visits));
    }

    /// The tabs open when the browser last closed.
    #[must_use]
    pub fn tabs(&self) -> Vec<Saved> {
        saved::read(&self.load(TABS))
    }

    /// Saves the open tabs.
    pub fn save_tabs(&self, tabs: &[Saved]) {
        self.store(TABS, &saved::write(tabs));
    }

    /// The settings.
    #[must_use]
    pub fn settings(&self) -> Settings {
        Settings::read(&self.load(SETTINGS))
    }

    /// Saves the settings.
    pub fn save_settings(&self, settings: &Settings) {
        self.store(SETTINGS, &settings.write());
    }
}
