//! Pages (REQ-WEB-003, REQ-WEB-004): the queue of requests, the cache and
//! its validators, a page response, and a page's catalogue of sub-pages
//! and its sub-pages' files (REQ-WEB-019, REQ-WEB-034).

use neoteletext_core::types::{Instant, PageAddress, SubCode};

use super::service::channel_file_media;
use super::{
    CacheEntry, Cached, Event, Exchange, FIRST_PAGE, Failure, Output, PAGE_SLOTS, PageBytes, PageFile, Queued, Sink,
    Source, UserAgent, WORD_BITS, Want, event,
};
use crate::consts::{MICROSECONDS_PER_SECOND, STATUS_GONE, STATUS_NOT_FOUND, STATUS_NOT_MODIFIED, STATUS_OK};
use crate::http::{MediaType, ResponseHead};
use crate::port::RandomSource;

/// The file a page request's validators belong to.
pub(super) const fn cached_as(file: PageFile) -> Cached {
    match file {
        PageFile::SubPage => Cached::SubPage,
        PageFile::Template | PageFile::Channel { .. } => Cached::Page,
    }
}

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    pub(super) fn enqueue(&mut self, page: PageAddress, subcode: Option<SubCode>, want: Want, sink: &mut Sink<'_>) {
        let wanted = Queued { page, subcode, want };
        if self.queue.iter().flatten().any(|&queued| queued == wanted) {
            return;
        }
        match self.queue.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => *slot = Some(wanted),
            None => match want {
                Want::Page => event(sink, Event::FetchFailed { page, subcode, failure: Failure::QueueFull }),
                Want::SubPages => event(sink, Event::SubPagesFailed { page, failure: Failure::QueueFull }),
            },
        }
    }

    pub(super) fn take_queued(&mut self) -> Option<Queued> {
        let first = self.queue.first_mut()?.take()?;
        self.queue.rotate_left(1);
        Some(first)
    }

    pub(super) fn cache_entry(&self, page: PageAddress, subcode: Option<SubCode>, file: Cached) -> Option<&CacheEntry> {
        self.cache.iter().flatten().find(|entry| entry.page == page && entry.subcode == subcode && entry.file == file)
    }

    /// The word and bit of a page in the set of catalogued pages.
    fn catalogue_slot(page: PageAddress) -> (usize, u32) {
        let index = usize::from(page.mpp().saturating_sub(FIRST_PAGE));
        (index / WORD_BITS, 1u32 << (index % WORD_BITS))
    }

    /// True when the page's last catalogue answered, so its sub-pages are
    /// files of their own.
    pub(super) fn is_catalogued(&self, page: PageAddress) -> bool {
        let (word, bit) = Self::catalogue_slot(page);
        self.catalogued.get(word).is_some_and(|slot| slot & bit != 0)
    }

    fn set_catalogued(&mut self, page: PageAddress, catalogued: bool) {
        let (word, bit) = Self::catalogue_slot(page);
        if let Some(slot) = self.catalogued.get_mut(word) {
            if catalogued {
                *slot |= bit;
            } else {
                *slot &= !bit;
            }
        }
    }

    /// Forgets every page's catalogue, as another service opens.
    pub(super) fn forget_catalogues(&mut self) {
        self.catalogued = [0; PAGE_SLOTS / WORD_BITS];
    }

    /// A page request is complete: the page's bytes, unchanged, missing
    /// (then, for a channel, its other file), or refused (REQ-WEB-019,
    /// REQ-WEB-027).
    pub(super) fn complete_page(
        &mut self,
        now: Instant,
        exchange: &Exchange,
        head: &ResponseHead,
        (page, subcode, file): (PageAddress, Option<SubCode>, PageFile),
        sink: &mut Sink<'_>,
    ) {
        let status = head.status;
        let body = self.buffers.body.get(..self.fetch.body_len).unwrap_or(&[]);
        let secure = exchange.url.is_secure();
        let t42 = matches!(file, PageFile::Channel { t42: true, .. } | PageFile::SubPage);
        let key = (page, subcode, cached_as(file));
        let missing = status == STATUS_NOT_FOUND || (file != PageFile::Template && status == STATUS_GONE);
        if status == STATUS_NOT_MODIFIED {
            self.update_cache(now, key, head, t42);
            event(sink, Event::PageCurrent { page, subcode });
        } else if missing {
            match file {
                // REQ-WEB-019: `NNN.tti`, then `NNN.t42`, or the
                // other way round.
                PageFile::Channel { t42, second: false } => self.second_try = Some((page, subcode, !t42)),
                // A sub-page that has gone from its page is not looked
                // for in the page's own file, which holds the first
                // sub-page only (REQ-WEB-019).
                _ => event(sink, Event::PageNotInService { page, subcode }),
            }
        } else if status != STATUS_OK {
            event(sink, Event::FetchFailed { page, subcode, failure: Failure::Status(status) });
        } else {
            let checked = match file {
                PageFile::Template => {
                    if head.media.is_page() {
                        Ok(head.media)
                    } else {
                        Err(Failure::MediaType)
                    }
                }
                PageFile::Channel { t42, .. } => channel_file_media(t42, head.media, body),
                PageFile::SubPage => channel_file_media(true, head.media, body),
            };
            match checked {
                Ok(media) => {
                    sink(Output::PageBytes(PageBytes {
                        source: Source::Page { page, subcode },
                        media,
                        url: exchange.url.as_str(),
                        secure,
                        bytes: body,
                    }));
                    if let PageFile::Channel { .. } = file {
                        self.t42_first = t42;
                    }
                    self.update_cache(now, key, head, t42);
                }
                Err(failure) => event(sink, Event::FetchFailed { page, subcode, failure }),
            }
        }
    }

    /// A catalogue request is complete (REQ-WEB-034): its bytes, unchanged,
    /// missing, or refused. A catalogue that answers makes the page's
    /// sub-pages files of their own; a missing one makes them its page
    /// file's again.
    pub(super) fn complete_subpages(
        &mut self,
        now: Instant,
        head: &ResponseHead,
        page: PageAddress,
        sink: &mut Sink<'_>,
    ) {
        let status = head.status;
        let key = (page, None, Cached::SubPages);
        if status == STATUS_NOT_MODIFIED {
            self.set_catalogued(page, true);
            self.update_cache(now, key, head, false);
            event(sink, Event::SubPagesCurrent { page });
        } else if status == STATUS_NOT_FOUND || status == STATUS_GONE {
            self.set_catalogued(page, false);
            self.drop_cache(key);
            event(sink, Event::SubPagesMissing { page });
        } else if status != STATUS_OK {
            event(sink, Event::SubPagesFailed { page, failure: Failure::Status(status) });
        } else if !matches!(head.media, MediaType::TextPlain | MediaType::OctetStream) {
            // REQ-WEB-027: a catalogue is text, as `contents.txt` is.
            event(sink, Event::SubPagesFailed { page, failure: Failure::MediaType });
        } else {
            self.set_catalogued(page, true);
            self.update_cache(now, key, head, false);
            let body = self.buffers.body.get(..self.fetch.body_len).unwrap_or(&[]);
            event(sink, Event::SubPages { page, bytes: body });
        }
    }

    fn drop_cache(&mut self, (page, subcode, file): (PageAddress, Option<SubCode>, Cached)) {
        for slot in &mut self.cache {
            if slot.is_some_and(|entry| entry.page == page && entry.subcode == subcode && entry.file == file) {
                *slot = None;
            }
        }
    }

    fn update_cache(
        &mut self,
        now: Instant,
        (page, subcode, file): (PageAddress, Option<SubCode>, Cached),
        head: &ResponseHead,
        t42: bool,
    ) {
        let index = self
            .cache
            .iter()
            .position(|slot| slot.is_some_and(|e| e.page == page && e.subcode == subcode && e.file == file));
        if head.no_store {
            if let Some(slot) = index.and_then(|i| self.cache.get_mut(i)) {
                *slot = None;
            }
            return;
        }
        self.cache_tick = self.cache_tick.wrapping_add(1);
        let fresh_until = match head.max_age {
            Some(seconds) if !head.no_cache => {
                Some(Instant(now.0.saturating_add(u64::from(seconds).saturating_mul(MICROSECONDS_PER_SECOND))))
            }
            _ => None,
        };
        let previous = index.and_then(|i| self.cache.get(i).copied().flatten());
        let entry = CacheEntry {
            page,
            subcode,
            // A 304 may omit validators; keep the ones held.
            etag: head.etag.or(previous.and_then(|p| p.etag)),
            last_modified: head.last_modified.or(previous.and_then(|p| p.last_modified)),
            fresh_until,
            used: self.cache_tick,
            t42,
            file,
        };
        let slot = index.or_else(|| self.cache.iter().position(Option::is_none)).or_else(|| {
            self.cache.iter().enumerate().min_by_key(|(_, slot)| slot.map_or(0, |e| e.used)).map(|(i, _)| i)
        });
        if let Some(slot) = slot.and_then(|i| self.cache.get_mut(i)) {
            *slot = Some(entry);
        }
    }
}
