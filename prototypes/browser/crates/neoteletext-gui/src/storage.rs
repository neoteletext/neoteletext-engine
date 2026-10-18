//! Where the browser keeps what it saves (REQ-BRW-006, `prototypes/browser/docs/desktop-browser.md`
//! section 11): the platform's application data directory, only ever
//! written by the interface process. The formats are
//! `neoteletext_browser::saved`'s.

use neoteletext_browser::fonts::{self, FontCache};
use neoteletext_browser::saved::{self, Saved, Settings};
use neoteletext_web::sha256::Digest;
use std::path::{Path, PathBuf};

/// The largest saved file read.
const MAX_FILE: u64 = 4 * 1024 * 1024;
const BOOKMARKS: &str = "bookmarks.tsv";
const HISTORY: &str = "history.tsv";
const TABS: &str = "tabs.tsv";
const SETTINGS: &str = "settings.txt";
/// The font cache's directory and its index (REQ-FNT-009).
const FONTS: &str = "fonts";
const FONT_INDEX: &str = "index.tsv";
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

    /// The font cache's index (REQ-FNT-009).
    #[must_use]
    pub fn font_index(&self) -> FontCache {
        FontCache::read(&self.load(&format!("{FONTS}/{FONT_INDEX}")))
    }

    /// Saves the font cache's index, whole, as the fonts are written.
    pub fn save_font_index(&self, cache: &FontCache) {
        let _ = self.store_in_fonts(FONT_INDEX, cache.write().as_bytes());
    }

    /// A cached font's bytes, at most 256 kilobytes; its checksum is the
    /// caller's to check on every load (REQ-FNT-009).
    #[must_use]
    pub fn cached_font(&self, sha256: &Digest) -> Option<Vec<u8>> {
        let path = self.path(FONTS)?.join(fonts::file_name(sha256));
        let small = std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() <= fonts::FONT_LIMIT_BYTES);
        if small { std::fs::read(path).ok() } else { None }
    }

    /// Stores a font whole, named by its SHA-256 alone (REQ-FNT-009): to a
    /// temporary name first, then renamed. False when it could not be.
    #[must_use]
    pub fn store_font(&self, sha256: &Digest, bytes: &[u8]) -> bool {
        self.store_in_fonts(&fonts::file_name(sha256), bytes)
    }

    /// Writes a file of the font cache whole: to a temporary name in the
    /// same folder first, then renamed (REQ-FNT-009).
    fn store_in_fonts(&self, name: &str, bytes: &[u8]) -> bool {
        let Some(directory) = self.path(FONTS) else { return false };
        if std::fs::create_dir_all(&directory).is_err() {
            return false;
        }
        let temporary = directory.join(format!(".{name}.new"));
        std::fs::write(&temporary, bytes).is_ok() && std::fs::rename(&temporary, directory.join(name)).is_ok()
    }

    /// Deletes a cached font's file.
    pub fn delete_font(&self, sha256: &Digest) {
        if let Some(directory) = self.path(FONTS) {
            let _ = std::fs::remove_file(directory.join(fonts::file_name(sha256)));
        }
    }
}
