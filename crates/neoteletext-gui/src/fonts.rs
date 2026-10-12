//! The window's fonts (area FNT, `docs/font-security.md`): loading a font
//! named by the user, a channel or a page, from memory, the cache or the
//! network, and listing font libraries. The decisions are
//! `neoteletext_browser::fonts`'; this module does the work.
//!
//! The interface never parses a font (REQ-FNT-003). A font is fetched and
//! read by a `font-fetch` helper, or read by a `font-read` helper; the
//! interface computes the bytes' SHA-256 itself, compares it with the one
//! the font was named by, checks the glyph table's shape, and only then
//! keeps the font. Every load from the cache does all of this again
//! (REQ-FNT-009).

use crate::helpers::{FontJob, Options, Wake, font_helper};
use crate::storage::Storage;
use crate::tab::Tab;
use neoteletext_app::pipe::{DecoderFonts, FontReply, Frame, LibraryFont, LibraryReply};
use neoteletext_browser::fonts::{
    Available, CacheEntry, Chooser, FontCache, Refused, Scope, glyph_digest, may_fetch, select,
};
use neoteletext_browser::saved::{Settings, UserFont};
use neoteletext_core::glyphs::LoadedFont;
use neoteletext_core::types::FontGlyph;
use neoteletext_web::sha256::{self, Digest, parse_hex};
use neoteletext_web::url::Url;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};

/// A font asked for: where it is, the SHA-256 it was named with, who named
/// it, and whose cache it belongs to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Request {
    /// Its address, or for a local file its path.
    pub address: String,
    /// The SHA-256 it was named with; `None` for a font the user named by
    /// address alone.
    pub sha256: Option<Digest>,
    /// Who named it.
    pub chooser: Chooser,
    /// The cache it belongs to (REQ-FNT-009).
    pub scope: Scope,
}

/// A loaded font: its checked glyph table and what the settings show.
#[derive(Clone, Debug)]
pub struct Font {
    /// The glyphs, checked for shape.
    pub glyphs: Vec<FontGlyph>,
    /// `FAMILY_NAME`, or the address when it has none.
    pub name: String,
    /// The licence notice: `NOTICE`, or `COPYRIGHT`.
    pub notice: String,
}

/// Where a request stands.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum State {
    /// Being fetched or read.
    Loading,
    /// Loaded, with this SHA-256.
    Ready(Digest),
    /// It could not be loaded, and why.
    Failed(String),
}

/// A font library's listing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Library {
    /// Its index is being fetched.
    Loading,
    /// Its fonts by title, and how many lines were ignored.
    Listed(Vec<LibraryFont>, u32),
    /// Its index could not be fetched or read.
    Failed(String),
}

enum Job {
    /// `bytes` is what a reader was given; `fallback` is the cached copy of
    /// a font being checked again, used if the check fails (REQ-FNT-009).
    Font {
        key: Key,
        request: Request,
        bytes: Option<Vec<u8>>,
        fallback: Option<Digest>,
        answer: Receiver<Option<Frame>>,
    },
    Library {
        address: String,
        answer: Receiver<Option<Frame>>,
    },
}

/// A request's identity: its address and the checksum it was named with.
type Key = (String, Option<Digest>);

/// The window's fonts.
pub struct Fonts {
    cache: FontCache,
    loaded: HashMap<Digest, Font>,
    states: HashMap<Key, State>,
    libraries: HashMap<String, Library>,
    jobs: Vec<Job>,
}

fn key(request: &Request) -> Key {
    (request.address.clone(), request.sha256)
}

/// A font's text made one line, for the settings.
fn line(text: Option<&String>) -> String {
    text.map(|text| text.replace(['\n', '\r', '\t'], " ")).unwrap_or_default()
}

impl Fonts {
    /// The fonts the cache holds, none loaded yet.
    #[must_use]
    pub fn new(storage: &Storage) -> Self {
        Self {
            cache: storage.font_index(),
            loaded: HashMap::new(),
            states: HashMap::new(),
            libraries: HashMap::new(),
            jobs: Vec::new(),
        }
    }

    /// The cache's entries, for the settings' list.
    #[must_use]
    pub fn cache(&self) -> &FontCache {
        &self.cache
    }

    /// Where a request stands, if it was made.
    #[must_use]
    pub fn state(&self, request: &Request) -> Option<&State> {
        self.states.get(&key(request))
    }

    /// The SHA-256 of a cached font's glyph table, by the file's SHA-256:
    /// fonts with the same one draw alike (REQ-BRW-010).
    #[must_use]
    pub fn glyph_digest(&self, sha256: &Digest) -> Option<Digest> {
        self.cache.find(sha256, None).and_then(|entry| entry.glyphs)
    }

    /// A loaded font, by its SHA-256.
    #[must_use]
    pub fn font(&self, sha256: &Digest) -> Option<&Font> {
        self.loaded.get(sha256)
    }

    /// The loaded font a request found, if it is ready.
    #[must_use]
    pub fn ready(&self, request: &Request) -> Option<(Digest, &Font)> {
        match self.states.get(&key(request)) {
            Some(State::Ready(sha256)) => self.loaded.get(sha256).map(|font| (*sha256, font)),
            _ => None,
        }
    }

    /// A library's listing, if it was asked for.
    #[must_use]
    pub fn library(&self, address: &str) -> Option<&Library> {
        self.libraries.get(address)
    }

    /// Forgets the requests that failed, so they are tried again: the
    /// viewer reloaded.
    pub fn forget_failures(&mut self) {
        self.states.retain(|_, state| !matches!(state, State::Failed(_)));
    }

    /// Asks for a font: from memory, then the cache, then the network.
    /// Whether it may be fetched (REQ-FNT-003) was decided by the caller.
    pub fn want(&mut self, request: &Request, storage: &Storage, options: &Options, wake: &Wake, now: u64) {
        let key = key(request);
        if self.states.contains_key(&key) {
            return;
        }
        if let Some(sha256) = request.sha256
            && self.loaded.contains_key(&sha256)
        {
            self.cache.touch(&sha256, &request.scope, now);
            self.states.insert(key, State::Ready(sha256));
            return;
        }
        let channel = match &request.scope {
            Scope::Channel(folder) => Some(folder.as_str()),
            Scope::Shared => None,
        };
        let cached = match request.sha256 {
            Some(sha256) => self.cache.find(&sha256, channel).cloned(),
            None => self.cache.find_address(&request.address).cloned(),
        };
        let mut fallback = None;
        let mut validators = [None, None];
        if let Some(entry) = cached {
            if request.sha256.is_none() && FontCache::due_for_check(&entry, now) {
                // REQ-FNT-009: checked again at most daily with a
                // conditional request, the cached copy kept for when the
                // check fails or the server says it is current.
                fallback = Some(entry.sha256);
                let present = |value: &str| (!value.is_empty()).then(|| value.to_string());
                validators = [present(&entry.etag), present(&entry.last_modified)];
            } else if self.read_cached(&key, request, &entry, storage, options, wake) {
                return;
            }
        }
        if request.address.contains("://") {
            let job = FontJob::Fetch { address: request.address.clone(), index: false, validators };
            self.start(key, request.clone(), job, None, fallback, options, wake);
        } else {
            self.states.insert(key, State::Failed("not in the cache; choose the file again".into()));
        }
    }

    /// Starts reading a cached font, after checking its SHA-256 again
    /// (REQ-FNT-009); a file that fails is deleted. Returns false when there
    /// is nothing usable to read.
    fn read_cached(
        &mut self,
        key: &Key,
        request: &Request,
        entry: &CacheEntry,
        storage: &Storage,
        options: &Options,
        wake: &Wake,
    ) -> bool {
        match storage.cached_font(&entry.sha256) {
            Some(bytes) if sha256::digest(&bytes) == entry.sha256 => {
                let job = FontJob::Read(bytes.clone());
                self.start(key.clone(), request.clone(), job, Some(bytes), None, options, wake);
                true
            }
            _ => {
                for file in self.cache.remove(&entry.sha256, &entry.scope) {
                    storage.delete_font(&file);
                }
                storage.save_font_index(&self.cache);
                false
            }
        }
    }

    /// Reads a local file the user chose (REQ-FNT-003), which is then kept
    /// in the shared cache like any user's font.
    pub fn read_local(&mut self, request: &Request, bytes: Vec<u8>, options: &Options, wake: &Wake) {
        let key = key(request);
        self.states.remove(&key);
        self.start(key, request.clone(), FontJob::Read(bytes.clone()), Some(bytes), None, options, wake);
    }

    /// Fetches a library's index (REQ-FNT-004); listing it fetches no font.
    pub fn list_library(&mut self, folder: &str, options: &Options, wake: &Wake) {
        let Ok(index) = Url::parse(folder).and_then(|url| neoteletext_web::fonts::index_url(&url)) else {
            self.libraries.insert(folder.to_string(), Library::Failed("not a library's address".into()));
            return;
        };
        let job = FontJob::Fetch { address: index.as_str().to_string(), index: true, validators: [None, None] };
        match font_helper(options, job, wake.clone()) {
            Ok(answer) => {
                self.libraries.insert(folder.to_string(), Library::Loading);
                self.jobs.push(Job::Library { address: folder.to_string(), answer });
            }
            Err(error) => {
                self.libraries.insert(folder.to_string(), Library::Failed(error));
            }
        }
    }

    #[allow(clippy::too_many_arguments)] // The request, the job and what checks its answer.
    fn start(
        &mut self,
        key: Key,
        request: Request,
        job: FontJob,
        bytes: Option<Vec<u8>>,
        fallback: Option<Digest>,
        options: &Options,
        wake: &Wake,
    ) {
        match font_helper(options, job, wake.clone()) {
            Ok(answer) => {
                self.states.insert(key.clone(), State::Loading);
                self.jobs.push(Job::Font { key, request, bytes, fallback, answer });
            }
            Err(error) => {
                self.states.insert(key, State::Failed(error));
            }
        }
    }

    /// Takes the helpers' answers. Returns true when anything changed.
    pub fn poll(&mut self, storage: &Storage, options: &Options, wake: &Wake, now: u64) -> bool {
        let mut changed = false;
        let jobs = std::mem::take(&mut self.jobs);
        for job in jobs {
            match job {
                Job::Font { key, request, bytes, fallback, answer } => {
                    // `None` is the server's answer that the copy held is
                    // current (REQ-FNT-009).
                    let outcome = match answer.try_recv() {
                        Ok(Some(Frame::Font(FontReply::Current))) => None,
                        Ok(frame) => Some(self.answer(&request, bytes.as_deref(), frame, storage, now)),
                        Err(TryRecvError::Empty) => {
                            self.jobs.push(Job::Font { key, request, bytes, fallback, answer });
                            continue;
                        }
                        Err(TryRecvError::Disconnected) => Some(State::Failed("the font helper ended".into())),
                    };
                    changed = true;
                    let current = outcome.is_none();
                    let state = outcome.unwrap_or_else(|| State::Failed("current, but no copy is cached".into()));
                    // REQ-FNT-009: when the copy held is current, or the
                    // daily check fails, the cached copy is used.
                    let cached = fallback.and_then(|sha256| self.cache.find(&sha256, None).cloned());
                    if let (true, Some(entry)) = (current || matches!(state, State::Failed(_)), cached) {
                        self.cache.mark_checked(&entry.sha256, now);
                        storage.save_font_index(&self.cache);
                        if self.read_cached(&key, &request, &entry, storage, options, wake) {
                            continue;
                        }
                    }
                    self.states.insert(key, state);
                }
                Job::Library { address, answer } => match answer.try_recv() {
                    Ok(frame) => {
                        changed = true;
                        let listing = match frame {
                            Some(Frame::FontLibrary(LibraryReply::Listed(fonts, ignored))) => {
                                Library::Listed(fonts, ignored)
                            }
                            Some(Frame::FontLibrary(LibraryReply::Failed(why))) => Library::Failed(why),
                            _ => Library::Failed("the font helper gave no listing".into()),
                        };
                        self.libraries.insert(address, listing);
                    }
                    Err(TryRecvError::Empty) => self.jobs.push(Job::Library { address, answer }),
                    Err(TryRecvError::Disconnected) => {
                        changed = true;
                        self.libraries.insert(address, Library::Failed("the font helper ended".into()));
                    }
                },
            }
        }
        changed
    }

    /// Checks a helper's answer and keeps the font (REQ-FNT-003,
    /// REQ-FNT-009). `sent` is what a reader was given, which its answer
    /// must carry back unchanged.
    fn answer(
        &mut self,
        request: &Request,
        sent: Option<&[u8]>,
        frame: Option<Frame>,
        storage: &Storage,
        now: u64,
    ) -> State {
        let font = match frame {
            Some(Frame::Font(FontReply::Loaded(font))) => font,
            Some(Frame::Font(FontReply::Failed(why))) => return State::Failed(why),
            _ => return State::Failed("the font helper gave no font".into()),
        };
        if sent.is_some_and(|sent| sent != font.bytes) {
            return State::Failed("the font reader changed the font".into());
        }
        let sha256 = sha256::digest(&font.bytes);
        if request.sha256.is_some_and(|wanted| wanted != sha256) {
            return State::Failed("the font's SHA-256 is not the one it was named with".into());
        }
        if LoadedFont::new(&font.glyphs).is_err() {
            return State::Failed("the font's table is malformed".into());
        }
        let name = font.family.clone().filter(|name| !name.is_empty()).unwrap_or_else(|| request.address.clone());
        let notice = line(font.notice.as_ref().or(font.copyright.as_ref()));
        let entry = CacheEntry {
            sha256,
            size: font.bytes.len() as u64,
            scope: request.scope.clone(),
            address: request.address.clone(),
            name: line(Some(&name)),
            notice: notice.clone(),
            named_by_checksum: request.sha256.is_some(),
            last_used: now,
            checked: now,
            pinned: self.cache.find(&sha256, None).is_some_and(|entry| entry.pinned),
            etag: line(font.validators[0].as_ref()),
            last_modified: line(font.validators[1].as_ref()),
            glyphs: Some(glyph_digest(&font.glyphs)),
        };
        let stored = storage.cached_font(&sha256).is_some() || storage.store_font(&sha256, &font.bytes);
        // A font that cannot be written is used now and not kept.
        let inserted =
            if stored { self.cache.insert(entry) } else { Err(neoteletext_browser::fonts::CacheRefused::Full) };
        match inserted {
            Ok(removed) => {
                for file in removed {
                    self.loaded.remove(&file);
                    storage.delete_font(&file);
                }
            }
            // The font is used now, though it cannot be kept.
            Err(_) => storage.delete_font(&sha256),
        }
        storage.save_font_index(&self.cache);
        self.loaded.insert(sha256, Font { glyphs: font.glyphs, name, notice });
        State::Ready(sha256)
    }

    /// Pins or unpins a cached font (REQ-FNT-009).
    pub fn set_pinned(&mut self, sha256: &Digest, scope: &Scope, pinned: bool, storage: &Storage) {
        self.cache.set_pinned(sha256, scope, pinned);
        storage.save_font_index(&self.cache);
    }

    /// Removes a cached font.
    pub fn remove(&mut self, sha256: &Digest, scope: &Scope, storage: &Storage) {
        for file in self.cache.remove(sha256, scope) {
            storage.delete_font(&file);
        }
        storage.save_font_index(&self.cache);
        self.states.retain(|_, state| *state != State::Ready(*sha256));
    }

    /// Removes every cached font.
    pub fn clear(&mut self, storage: &Storage) {
        for file in self.cache.clear() {
            storage.delete_font(&file);
        }
        storage.save_font_index(&self.cache);
        self.states.clear();
    }
}

/// What loading a font needs from the window.
pub struct Work<'a> {
    /// The profile, where the cache is.
    pub storage: &'a Storage,
    /// How helpers are started.
    pub options: &'a Options,
    /// Wakes the window when a helper answers.
    pub wake: &'a Wake,
    /// Seconds since 1970.
    pub now: u64,
}

/// The request for the font the user chose (REQ-FNT-005).
#[must_use]
pub fn user_request(font: &UserFont) -> Option<Request> {
    let sha256 = if font.by_address { None } else { Some(parse_hex(font.sha256.as_bytes())?) };
    Some(Request { address: font.address.clone(), sha256, chooser: Chooser::User, scope: Scope::Shared })
}

/// Why a font was not fetched, for the status line.
const fn refusal(refused: Refused) -> &'static str {
    match refused {
        Refused::Scheme => "a font's address is not http or https",
        Refused::NeedsChecksum => "a font from another site came without its SHA-256",
        Refused::OtherSitesRefused => "fonts from other sites are refused in the settings",
    }
}

/// Loads the fonts a tab's page and channel name, when they may be fetched
/// (REQ-FNT-003), chooses the font in use (REQ-FNT-005), and sends the
/// decoder the fonts to draw with when they change (REQ-FNT-008). `user`
/// is the user's font, already asked for.
pub fn update_tab(tab: &mut Tab, fonts: &mut Fonts, settings: &Settings, user: Option<&Request>, work: &Work<'_>) {
    if let Some(request) = user {
        fonts.want(request, work.storage, work.options, work.wake, work.now);
    }
    let folder = tab.channel_folder();
    let origin = folder.as_deref().and_then(|folder| Url::parse(folder).ok()).map(|url| url.origin());
    let scope = folder.map(Scope::Channel);
    let mut named = [None, None];
    let sources = [(tab.page_font(), Chooser::Page), (tab.channel_font.clone(), Chooser::Channel)];
    for (slot, (font, chooser)) in named.iter_mut().zip(sources) {
        let (Some((address, sha256)), Some(scope)) = (font, scope.clone()) else { continue };
        let Ok(url) = Url::parse(&address) else { continue };
        match may_fetch(chooser, &url, origin.as_ref(), Some(&sha256), settings.fonts_from_other_sites) {
            Ok(()) => {
                let request = Request { address, sha256: Some(sha256), chooser, scope };
                fonts.want(&request, work.storage, work.options, work.wake, work.now);
                *slot = Some(request);
            }
            Err(refused) => {
                let message = refusal(refused).to_string();
                if tab.notice.as_deref() != Some(message.as_str()) {
                    tab.notice = Some(message);
                }
            }
        }
    }
    let ready = |request: Option<&Request>| request.and_then(|request| fonts.ready(request));
    let [page, channel] = &named;
    let (page, channel, mine) = (ready(page.as_ref()), ready(channel.as_ref()), ready(user));
    let selection = select(
        Available { page: page.is_some(), channel: channel.is_some(), user: mine.is_some() },
        settings.always_my_font,
    );
    let in_use: Option<(Digest, &Font, Chooser)> = match selection.in_use {
        Some(Chooser::Page) => page.map(|(sha256, font)| (sha256, font, Chooser::Page)),
        Some(Chooser::Channel) => channel.map(|(sha256, font)| (sha256, font, Chooser::Channel)),
        Some(Chooser::User) => mine.map(|(sha256, font)| (sha256, font, Chooser::User)),
        None => None,
    };
    let fallback = if selection.user_fills_in { mine } else { None };
    let signature = (in_use.map(|(sha256, ..)| sha256), fallback.map(|(sha256, _)| sha256));
    tab.font_shown = in_use.map(|(_, font, chooser)| (font.name.clone(), chooser));
    if tab.fonts_differ(signature) {
        let decoder_fonts = DecoderFonts {
            in_use: in_use.map(|(_, font, _)| font.glyphs.clone()).unwrap_or_default(),
            fallback: fallback.map(|(_, font)| font.glyphs.clone()).unwrap_or_default(),
        };
        tab.send_fonts(signature, decoder_fonts);
    }
}
