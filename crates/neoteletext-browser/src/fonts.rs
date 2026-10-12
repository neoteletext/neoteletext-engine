//! Every decision the browser makes about fonts (area FNT, decision E.72,
//! `docs/font-security.md`), written once for every front end:
//!
//! - [`may_fetch`]: whether a font named by the user, the channel or the
//!   page may be fetched at all (REQ-FNT-003, REQ-WEB-014);
//! - [`select`]: which font draws a page, and whether the user's font fills
//!   in what it lacks (REQ-FNT-005);
//! - [`FontCache`]: the content-addressed cache of REQ-FNT-009, as a model
//!   the front end stores; the files themselves are the front end's.
//!
//! Nothing here reads a font: fonts are read only in confined helpers.

use neoteletext_web::sha256::{Digest, parse_hex, to_hex};
use neoteletext_web::url::{Origin, Url};
use std::fmt::Write as _;

/// The cache's total limit (REQ-FNT-009).
pub const CACHE_LIMIT_BYTES: u64 = 4 * 1024 * 1024;
/// One font's limit, the file limit of REQ-FNT-002.
pub const FONT_LIMIT_BYTES: u64 = neoteletext_core::consts::FONT_MAX_FILE_BYTES as u64;
/// How often a font the user named by address alone is checked again, in
/// seconds: at most once a day (REQ-FNT-009).
pub const RECHECK_SECONDS: u64 = 24 * 60 * 60;
/// The most entries the cache's index keeps.
pub const MAX_ENTRIES: usize = 1024;
/// The longest text kept for a font: its address, name or notice.
const MAX_TEXT: usize = 512;
/// The field separator of the cache's index.
const SEPARATOR: char = '\t';

/// Who named a font.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chooser {
    /// The page, with a `TF` line (REQ-FNT-007).
    Page,
    /// The channel, in `channel.json` (REQ-FNT-006).
    Channel,
    /// The viewer, in the settings (REQ-BRW-007).
    User,
}

impl Chooser {
    /// How the status line says who chose the font (REQ-FNT-005).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Page => "the page's font",
            Self::Channel => "the channel's font",
            Self::User => "your font",
        }
    }
}

/// Why a font is not fetched.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refused {
    /// Not an `http` or `https` address.
    Scheme,
    /// A channel's or page's font at another address, which needs its
    /// SHA-256 (REQ-WEB-014).
    NeedsChecksum,
    /// A channel's or page's font at another address, while the viewer
    /// refuses fonts from other sites (REQ-FNT-003).
    OtherSitesRefused,
}

/// Whether a font may be fetched (REQ-FNT-003, REQ-WEB-014): only from an
/// `http` or `https` address, never a WebSocket one; and a font the channel or the page names at
/// an address other than the channel's own origin only with its SHA-256,
/// and only while the viewer allows fonts from other sites. The fetch then
/// goes without cookies or `Referer`, and under the local-network and
/// downgrade rules of every request.
pub fn may_fetch(
    chooser: Chooser,
    font: &Url,
    channel: Option<&Origin>,
    sha256: Option<&Digest>,
    other_sites_allowed: bool,
) -> Result<(), Refused> {
    if font.scheme().is_websocket() {
        return Err(Refused::Scheme);
    }
    if chooser == Chooser::User {
        return Ok(());
    }
    let same_origin = channel.is_some_and(|origin| origin.same(&font.origin()));
    if same_origin {
        return Ok(());
    }
    if sha256.is_none() {
        return Err(Refused::NeedsChecksum);
    }
    if !other_sites_allowed {
        return Err(Refused::OtherSitesRefused);
    }
    Ok(())
}

/// Which loaded font draws a page's rows 1 to 24 (REQ-FNT-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Selection {
    /// The font in use, by who chose it; `None` is Bedstead.
    pub in_use: Option<Chooser>,
    /// The user's font fills in the characters the font in use lacks.
    pub user_fills_in: bool,
}

/// Fonts available for a page: each true when that font is loaded.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Available {
    /// The page's font.
    pub page: bool,
    /// The channel's font.
    pub channel: bool,
    /// The user's chosen font.
    pub user: bool,
}

/// The first of the page's font, the channel's, the user's and Bedstead;
/// with "always use my font", the user's font or Bedstead whatever the
/// channel or page names. Characters the font in use lacks come from the
/// user's font, then Bedstead (REQ-FNT-005).
#[must_use]
pub const fn select(available: Available, always_mine: bool) -> Selection {
    let in_use = if always_mine {
        if available.user { Some(Chooser::User) } else { None }
    } else if available.page {
        Some(Chooser::Page)
    } else if available.channel {
        Some(Chooser::Channel)
    } else if available.user {
        Some(Chooser::User)
    } else {
        None
    };
    let user_fills_in = available.user && matches!(in_use, Some(Chooser::Page | Chooser::Channel));
    Selection { in_use, user_fills_in }
}

/// What a cached font is kept for (REQ-FNT-009).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Scope {
    /// The user's fonts and the fonts of the user's libraries: any tab.
    Shared,
    /// A channel's or page's font, used only by that channel, named by its
    /// folder's address.
    Channel(String),
}

/// One cached font.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CacheEntry {
    /// The file's SHA-256, which names it.
    pub sha256: Digest,
    /// Bytes.
    pub size: u64,
    /// Who may use it.
    pub scope: Scope,
    /// Where it came from: an address, or a local file's name.
    pub address: String,
    /// Its `FAMILY_NAME`.
    pub name: String,
    /// Its licence notice: `NOTICE`, or `COPYRIGHT`.
    pub notice: String,
    /// The font was named with its SHA-256, so it is never checked again.
    pub named_by_checksum: bool,
    /// When it was last used, in seconds since 1970.
    pub last_used: u64,
    /// When it was last fetched or checked.
    pub checked: u64,
    /// The user keeps it whatever the limit.
    pub pinned: bool,
}

/// The cache's index: which fonts are kept, for whom, and when they were
/// used. The files are stored once each, named by their SHA-256 alone.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FontCache {
    entries: Vec<CacheEntry>,
}

/// Why a font is not cached.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CacheRefused {
    /// Larger than 256 kilobytes.
    TooLarge,
    /// The pinned fonts leave no room.
    Full,
}

impl FontCache {
    /// Every entry, for the settings' list.
    #[must_use]
    pub fn entries(&self) -> &[CacheEntry] {
        &self.entries
    }

    /// The entry a tab may use: a shared one, or one of its own channel.
    #[must_use]
    pub fn find(&self, sha256: &Digest, channel: Option<&str>) -> Option<&CacheEntry> {
        self.entries.iter().find(|entry| {
            entry.sha256 == *sha256
                && match &entry.scope {
                    Scope::Shared => true,
                    Scope::Channel(owner) => channel == Some(owner.as_str()),
                }
        })
    }

    /// The entry of a font the user named by address alone.
    #[must_use]
    pub fn find_address(&self, address: &str) -> Option<&CacheEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.scope == Scope::Shared && entry.address == address)
            .max_by_key(|entry| entry.checked)
    }

    /// True when a font named by address alone is due for its daily check
    /// (REQ-FNT-009); a font named with its SHA-256 never is.
    #[must_use]
    pub const fn due_for_check(entry: &CacheEntry, now: u64) -> bool {
        !entry.named_by_checksum && now.saturating_sub(entry.checked) >= RECHECK_SECONDS
    }

    /// Marks a font used now.
    pub fn touch(&mut self, sha256: &Digest, scope: &Scope, now: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.sha256 == *sha256 && entry.scope == *scope) {
            entry.last_used = now;
        }
    }

    /// Adds or replaces an entry, then removes the least recently used
    /// unpinned fonts until the files fit in 4 megabytes. Returns the files
    /// no entry refers to any more, for the front end to delete.
    pub fn insert(&mut self, entry: CacheEntry) -> Result<Vec<Digest>, CacheRefused> {
        if entry.size > FONT_LIMIT_BYTES {
            return Err(CacheRefused::TooLarge);
        }
        let before = self.files();
        let sha256 = entry.sha256;
        self.entries.retain(|old| !(old.sha256 == entry.sha256 && old.scope == entry.scope));
        self.entries.push(entry);
        while self.total() > CACHE_LIMIT_BYTES || self.entries.len() > MAX_ENTRIES {
            let oldest = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, old)| !old.pinned && old.sha256 != sha256)
                .min_by_key(|(_, old)| old.last_used)
                .map(|(index, _)| index);
            let Some(index) = oldest else {
                self.entries.retain(|old| old.sha256 != sha256);
                return Err(CacheRefused::Full);
            };
            self.entries.remove(index);
        }
        Ok(self.unreferenced(&before))
    }

    /// Removes one entry. Returns the file to delete, if nothing else
    /// refers to it.
    pub fn remove(&mut self, sha256: &Digest, scope: &Scope) -> Vec<Digest> {
        let before = self.files();
        self.entries.retain(|entry| !(entry.sha256 == *sha256 && entry.scope == *scope));
        self.unreferenced(&before)
    }

    /// Removes every entry, pinned or not. Returns every file to delete.
    pub fn clear(&mut self) -> Vec<Digest> {
        let files = self.files();
        self.entries.clear();
        files
    }

    /// Pins or unpins one entry.
    pub fn set_pinned(&mut self, sha256: &Digest, scope: &Scope, pinned: bool) {
        for entry in self.entries.iter_mut().filter(|entry| entry.sha256 == *sha256 && entry.scope == *scope) {
            entry.pinned = pinned;
        }
    }

    /// Records a font named by address alone as checked now.
    pub fn mark_checked(&mut self, sha256: &Digest, now: u64) {
        for entry in self.entries.iter_mut().filter(|entry| entry.sha256 == *sha256) {
            entry.checked = now;
        }
    }

    /// Each file once.
    fn files(&self) -> Vec<Digest> {
        let mut files: Vec<Digest> = self.entries.iter().map(|entry| entry.sha256).collect();
        files.sort_unstable();
        files.dedup();
        files
    }

    /// The bytes of the files, each counted once.
    fn total(&self) -> u64 {
        let mut seen: Vec<(Digest, u64)> = self.entries.iter().map(|entry| (entry.sha256, entry.size)).collect();
        seen.sort_unstable();
        seen.dedup_by_key(|(sha256, _)| *sha256);
        seen.iter().map(|(_, size)| size).sum()
    }

    fn unreferenced(&self, before: &[Digest]) -> Vec<Digest> {
        let now = self.files();
        before.iter().filter(|file| now.binary_search(file).is_err()).copied().collect()
    }

    /// The index as text: one entry per line, tab-separated.
    #[must_use]
    pub fn write(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            let scope = match &entry.scope {
                Scope::Shared => String::from("shared"),
                Scope::Channel(folder) => format!("channel {}", text(folder)),
            };
            let _ = writeln!(
                out,
                "{}{SEPARATOR}{}{SEPARATOR}{scope}{SEPARATOR}{}{SEPARATOR}{}{SEPARATOR}{}{SEPARATOR}{}{SEPARATOR}{}{SEPARATOR}{}{SEPARATOR}{}",
                String::from_utf8_lossy(&to_hex(&entry.sha256)),
                entry.size,
                u8::from(entry.named_by_checksum),
                entry.last_used,
                entry.checked,
                u8::from(entry.pinned),
                text(&entry.address),
                text(&entry.name),
                text(&entry.notice),
            );
        }
        out
    }

    /// Reads an index, skipping any line that is not exactly one entry: the
    /// file could have been changed by anything. Each file is checked again
    /// whenever it is loaded (REQ-FNT-009).
    #[must_use]
    pub fn read(index: &str) -> Self {
        let entries = index
            .lines()
            .filter_map(|line| {
                let fields: Vec<&str> = line.split(SEPARATOR).collect();
                let [sha256, size, scope, by_checksum, last_used, checked, pinned, address, name, notice] =
                    fields.as_slice()
                else {
                    return None;
                };
                let scope = match scope.split_once(' ') {
                    None if *scope == "shared" => Scope::Shared,
                    Some(("channel", folder)) if !folder.is_empty() => Scope::Channel((*folder).to_string()),
                    _ => return None,
                };
                let entry = CacheEntry {
                    sha256: parse_hex(sha256.as_bytes())?,
                    size: size.parse().ok().filter(|&size| size <= FONT_LIMIT_BYTES)?,
                    scope,
                    address: (*address).to_string(),
                    name: (*name).to_string(),
                    notice: (*notice).to_string(),
                    named_by_checksum: flag(by_checksum)?,
                    last_used: last_used.parse().ok()?,
                    checked: checked.parse().ok()?,
                    pinned: flag(pinned)?,
                };
                Some(entry)
            })
            .take(MAX_ENTRIES)
            .collect();
        Self { entries }
    }
}

/// A field made safe to store: no separators or line ends, at most 512
/// bytes.
fn text(value: &str) -> String {
    let mut out = String::new();
    for character in value.chars().map(|c| if c.is_control() { ' ' } else { c }) {
        if out.len() + character.len_utf8() > MAX_TEXT {
            break;
        }
        out.push(character);
    }
    out
}

fn flag(field: &str) -> Option<bool> {
    match field {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

/// The file name of a cached font: its SHA-256 alone, never anything taken
/// from its address (REQ-FNT-009).
#[must_use]
pub fn file_name(sha256: &Digest) -> String {
    String::from_utf8_lossy(&to_hex(sha256)).into_owned()
}

/// The channel scope of a channel's address: its folder.
#[must_use]
pub fn channel_scope(folder: &Url) -> Scope {
    Scope::Channel(folder.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    const SHA: Digest = [7; 32];

    /// REQ-FNT-003, REQ-WEB-014: a channel's or page's font elsewhere needs
    /// its checksum and the viewer's permission; at the channel's own
    /// origin, or named by the user, it does not.
    #[test]
    fn who_may_fetch_what() {
        let channel = url("https://news.example/ceefax/").origin();
        let own = url("https://news.example/fonts/f.bdf");
        let other = url("https://fonts.example/f.bdf");
        for chooser in [Chooser::Channel, Chooser::Page] {
            assert_eq!(may_fetch(chooser, &own, Some(&channel), None, false), Ok(()), "own origin");
            assert_eq!(may_fetch(chooser, &other, Some(&channel), None, true), Err(Refused::NeedsChecksum));
            assert_eq!(may_fetch(chooser, &other, Some(&channel), Some(&SHA), false), Err(Refused::OtherSitesRefused));
            assert_eq!(may_fetch(chooser, &other, Some(&channel), Some(&SHA), true), Ok(()));
            assert_eq!(may_fetch(chooser, &other, None, Some(&SHA), true), Ok(()), "a file of no channel");
        }
        assert_eq!(may_fetch(Chooser::User, &other, Some(&channel), None, false), Ok(()), "the user chose it");
        assert_eq!(may_fetch(Chooser::User, &url("http://fonts.example/f.bdf"), None, None, false), Ok(()));
        assert_eq!(may_fetch(Chooser::User, &url("wss://fonts.example/f"), None, None, true), Err(Refused::Scheme));
        let http_same_host = url("http://news.example/f.bdf");
        assert_eq!(
            may_fetch(Chooser::Channel, &http_same_host, Some(&channel), None, true),
            Err(Refused::NeedsChecksum),
            "another scheme is another origin"
        );
    }

    /// REQ-FNT-005: each step of the order, the user's override, and when
    /// the user's font fills in.
    #[test]
    fn the_order_of_fonts() {
        let all = Available { page: true, channel: true, user: true };
        assert_eq!(select(all, false), Selection { in_use: Some(Chooser::Page), user_fills_in: true });
        assert_eq!(
            select(Available { page: false, ..all }, false),
            Selection { in_use: Some(Chooser::Channel), user_fills_in: true }
        );
        assert_eq!(
            select(Available { page: false, channel: false, user: true }, false),
            Selection { in_use: Some(Chooser::User), user_fills_in: false }
        );
        assert_eq!(select(Available::default(), false), Selection { in_use: None, user_fills_in: false });
        assert_eq!(
            select(Available { page: true, channel: false, user: false }, false),
            Selection { in_use: Some(Chooser::Page), user_fills_in: false }
        );
        assert_eq!(select(all, true), Selection { in_use: Some(Chooser::User), user_fills_in: false }, "always mine");
        assert_eq!(
            select(Available { user: false, ..all }, true),
            Selection { in_use: None, user_fills_in: false },
            "always mine with no font of mine is Bedstead"
        );
    }

    fn entry(byte: u8, size: u64, scope: Scope, last_used: u64) -> CacheEntry {
        CacheEntry {
            sha256: [byte; 32],
            size,
            scope,
            address: format!("https://f.example/{byte}.bdf"),
            name: format!("Font {byte}"),
            notice: "CC0".into(),
            named_by_checksum: true,
            last_used,
            checked: last_used,
            pinned: false,
        }
    }

    /// REQ-FNT-009: channel fonts are kept apart, shared fonts are for all.
    #[test]
    fn channels_are_kept_apart() {
        let mut cache = FontCache::default();
        let a = "https://a.example/ceefax/";
        cache.insert(entry(1, 100, Scope::Channel(a.into()), 1)).unwrap();
        cache.insert(entry(2, 100, Scope::Shared, 1)).unwrap();
        assert!(cache.find(&[1; 32], Some(a)).is_some());
        assert!(cache.find(&[1; 32], Some("https://b.example/")).is_none(), "another channel cannot use it");
        assert!(cache.find(&[1; 32], None).is_none());
        assert!(cache.find(&[2; 32], Some("https://b.example/")).is_some(), "shared");
        // The same file for two channels is stored once, removed when both go.
        cache.insert(entry(1, 100, Scope::Channel("https://b.example/".into()), 2)).unwrap();
        assert_eq!(cache.remove(&[1; 32], &Scope::Channel(a.into())), Vec::<Digest>::new());
        assert_eq!(cache.remove(&[1; 32], &Scope::Channel("https://b.example/".into())), vec![[1; 32]]);
    }

    /// REQ-FNT-009: 4 megabytes in all, the least recently used removed
    /// first, pinned fonts kept, 256 kilobytes per font.
    #[test]
    fn eviction_and_pinning() {
        let mut cache = FontCache::default();
        let quarter = 256 * 1024;
        for byte in 0..16 {
            assert_eq!(
                cache.insert(entry(byte, quarter, Scope::Shared, u64::from(byte))).unwrap(),
                Vec::<Digest>::new()
            );
        }
        cache.set_pinned(&[0; 32], &Scope::Shared, true);
        let removed = cache.insert(entry(16, quarter, Scope::Shared, 100)).unwrap();
        assert_eq!(removed, vec![[1; 32]], "the oldest unpinned font goes");
        assert!(cache.find(&[0; 32], None).is_some(), "pinned");
        cache.touch(&[2; 32], &Scope::Shared, 200);
        assert_eq!(cache.insert(entry(17, quarter, Scope::Shared, 300)).unwrap(), vec![[3; 32]], "2 was used lately");
        assert_eq!(cache.insert(entry(18, quarter + 1, Scope::Shared, 1)), Err(CacheRefused::TooLarge));
        for byte in 0..=17 {
            cache.set_pinned(&[byte; 32], &Scope::Shared, true);
        }
        assert_eq!(cache.insert(entry(19, quarter, Scope::Shared, 400)), Err(CacheRefused::Full));
        assert!(cache.find(&[19; 32], None).is_none());
        assert_eq!(cache.clear().len(), 16);
    }

    /// REQ-FNT-009: a font named by address alone is checked again at most
    /// daily; one named with its checksum never.
    #[test]
    fn the_daily_check() {
        let mut by_address = entry(1, 10, Scope::Shared, 1000);
        by_address.named_by_checksum = false;
        assert!(!FontCache::due_for_check(&by_address, 1000 + RECHECK_SECONDS - 1));
        assert!(FontCache::due_for_check(&by_address, 1000 + RECHECK_SECONDS));
        assert!(!FontCache::due_for_check(&entry(2, 10, Scope::Shared, 0), u64::MAX), "named by checksum");
        let mut cache = FontCache::default();
        cache.insert(by_address).unwrap();
        cache.mark_checked(&[1; 32], 5000);
        assert_eq!(cache.find_address("https://f.example/1.bdf").map(|entry| entry.checked), Some(5000));
    }

    /// The index survives writing and reading, and a damaged line is
    /// skipped.
    #[test]
    fn the_index_round_trips() {
        let mut cache = FontCache::default();
        cache.insert(entry(1, 100, Scope::Shared, 5)).unwrap();
        let mut odd = entry(2, 200, Scope::Channel("https://a.example/x/".into()), 6);
        odd.name = "tab\there".into();
        odd.pinned = true;
        cache.insert(odd).unwrap();
        let text = cache.write();
        let again = FontCache::read(&text);
        assert_eq!(again.entries().len(), 2);
        assert_eq!(again.entries()[1].name, "tab here");
        assert!(again.entries()[1].pinned);
        let damaged =
            format!("{text}not an entry\n{}", text.lines().next().unwrap().replace("\t100\t", "\t999999999\t"));
        assert_eq!(FontCache::read(&damaged).entries().len(), 2, "both damaged lines skipped");
        assert_eq!(file_name(&[0xAB; 32]), "ab".repeat(32));
    }
}
