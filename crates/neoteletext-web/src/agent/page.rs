//! Pages (REQ-WEB-003, REQ-WEB-004): the queue of requests, the cache and
//! its validators, and a page response.

use neoteletext_core::types::{Instant, PageAddress, SubCode};

use super::service::channel_file_media;
use super::{CacheEntry, Event, Exchange, Failure, Output, PageBytes, PageFile, Sink, Source, UserAgent, event};
use crate::consts::{MICROSECONDS_PER_SECOND, STATUS_GONE, STATUS_NOT_FOUND, STATUS_NOT_MODIFIED, STATUS_OK};
use crate::http::ResponseHead;
use crate::port::RandomSource;

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    pub(super) fn enqueue(&mut self, page: PageAddress, subcode: Option<SubCode>, sink: &mut Sink<'_>) {
        if self.queue.iter().flatten().any(|&queued| queued == (page, subcode)) {
            return;
        }
        match self.queue.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => *slot = Some((page, subcode)),
            None => event(sink, Event::FetchFailed { page, subcode, failure: Failure::QueueFull }),
        }
    }

    pub(super) fn take_queued(&mut self) -> Option<(PageAddress, Option<SubCode>)> {
        let first = self.queue.first_mut()?.take()?;
        self.queue.rotate_left(1);
        Some(first)
    }

    pub(super) fn cache_entry(&self, page: PageAddress, subcode: Option<SubCode>) -> Option<&CacheEntry> {
        self.cache.iter().flatten().find(|entry| entry.page == page && entry.subcode == subcode)
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
        let t42 = matches!(file, PageFile::Channel { t42: true, .. });
        let missing = status == STATUS_NOT_FOUND || (file != PageFile::Template && status == STATUS_GONE);
        if status == STATUS_NOT_MODIFIED {
            self.update_cache(now, page, subcode, head, t42);
            event(sink, Event::PageCurrent { page, subcode });
        } else if missing {
            match file {
                // REQ-WEB-019: `NNN.tti`, then `NNN.t42`, or the
                // other way round.
                PageFile::Channel { t42, second: false } => self.second_try = Some((page, subcode, !t42)),
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
                    if file != PageFile::Template {
                        self.t42_first = t42;
                    }
                    self.update_cache(now, page, subcode, head, t42);
                }
                Err(failure) => event(sink, Event::FetchFailed { page, subcode, failure }),
            }
        }
    }

    fn update_cache(
        &mut self,
        now: Instant,
        page: PageAddress,
        subcode: Option<SubCode>,
        head: &ResponseHead,
        t42: bool,
    ) {
        let index = self.cache.iter().position(|slot| slot.is_some_and(|e| e.page == page && e.subcode == subcode));
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
        };
        let slot = index.or_else(|| self.cache.iter().position(Option::is_none)).or_else(|| {
            self.cache.iter().enumerate().min_by_key(|(_, slot)| slot.map_or(0, |e| e.used)).map(|(i, _)| i)
        });
        if let Some(slot) = slot.and_then(|i| self.cache.get_mut(i)) {
            *slot = Some(entry);
        }
    }
}
