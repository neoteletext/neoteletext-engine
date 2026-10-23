//! The page walk (REQ-INP-022): every page the service lists, requested
//! in turn from 100 to 8FF, each left when its rows settle or its time
//! passes.

use neoteletext_core::types::{Instant, Magazine, PageAddress, PageNumber};

use super::{LiveEvent, LiveSink, PAGES_PER_MAGAZINE, Phase, Walk, ZxnetFeed, event};
use crate::consts::{ZXNET_MAX_PAGES, ZXNET_WALK_SETTLE_US, ZXNET_WALK_TIMEOUT_US};
use crate::port::RandomSource;

pub(super) fn page_index(page: PageAddress) -> usize {
    page.magazine.index().saturating_mul(usize::from(PAGES_PER_MAGAZINE)).saturating_add(usize::from(page.page.0))
}

/// Pages walk in the order 100 to 8FF: magazine 8 (index 0) last.
fn walk_order(page: PageAddress) -> usize {
    let magazine = usize::from(page.magazine.number().saturating_sub(1));
    magazine.saturating_mul(usize::from(PAGES_PER_MAGAZINE)).saturating_add(usize::from(page.page.0))
}

fn page_from_walk_order(order: usize) -> Option<PageAddress> {
    let per_magazine = usize::from(PAGES_PER_MAGAZINE);
    let magazine = u8::try_from(order.checked_div(per_magazine)?).ok()?.checked_add(1)?;
    let page = u8::try_from(order.checked_rem(per_magazine)?).ok()?;
    Some(PageAddress { magazine: Magazine::new(magazine)?, page: PageNumber(page) })
}

impl<R: RandomSource> ZxnetFeed<'_, R> {
    /// Moves the walk to the first listed page after `after`.
    pub(super) fn walk_next(&mut self, now: Instant, after: Option<PageAddress>, sink: &mut LiveSink<'_>) {
        let start = after.map_or(0, |page| walk_order(page).saturating_add(1));
        let next = (start..ZXNET_MAX_PAGES).filter_map(page_from_walk_order).find(|&page| self.page_exists(page));
        let Some(walk) = &mut self.walk else { return };
        *walk = Walk { current: next, started: now, header_seen: false, rows: 0, last_row: None };
        match next {
            Some(page) => {
                event(sink, LiveEvent::WalkPage(page));
                if self.phase == Phase::Open {
                    self.page_search(page, None, sink);
                }
            }
            None if after.is_some() => {
                self.walk = None;
                event(sink, LiveEvent::WalkComplete);
            }
            None => {}
        }
    }

    pub(super) fn tick_walk(&mut self, now: Instant, sink: &mut LiveSink<'_>) {
        let Some(walk) = self.walk else { return };
        let Some(page) = walk.current else {
            // No pages were listed yet: try again as they arrive.
            self.walk_next(now, None, sink);
            return;
        };
        let settled = walk.last_row.is_some_and(|last| now.since(last) >= ZXNET_WALK_SETTLE_US);
        let timed_out = now.since(walk.started) >= ZXNET_WALK_TIMEOUT_US;
        if settled || timed_out {
            event(sink, LiveEvent::WalkPageDone { page, rows: walk.rows, timed_out: !settled });
            self.walk_next(now, Some(page), sink);
        }
    }

    /// When the walk next acts: its page settled, or its time limit.
    pub(super) fn walk_deadline(&self) -> Option<Instant> {
        self.walk.and_then(|walk| {
            walk.last_row
                .map(|last| Instant(last.0.saturating_add(ZXNET_WALK_SETTLE_US)))
                .or(Some(Instant(walk.started.0.saturating_add(ZXNET_WALK_TIMEOUT_US))))
        })
    }
}
