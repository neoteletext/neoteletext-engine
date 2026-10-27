//! The viewer's keys (REQ-FSM-005, REQ-NAV-012, REQ-SEC-024): page entry,
//! Fastext, stepping, history and the confirmation of links that leave the
//! service.

use super::{Action, Key, KeyContext, SecurityIndicator, Viewer, ViewerState};
use crate::acq::PageStore;
use crate::config::{RevealStyle, StepOrder};
use crate::level1::Zoom;
use crate::nav::{EntryResult, PageLinks, next_page, previous_page, step_stored};
use crate::types::{Instant, SubCode};

impl Viewer {
    /// Handles a key (REQ-FSM-005). `links` are the links of the page shown
    /// (from X/27/0 or the TTI Link Extension); `confirmed` says whether
    /// another service's origin has been confirmed this session.
    pub fn key(&mut self, key: Key, context: &KeyContext<'_>) -> Action {
        let KeyContext { links, store, initial, config, now, confirmed, row24_shown } = *context;
        if let ViewerState::ConfirmingLink(waiting) = self.state {
            return self.answer_confirmation(key, waiting, links);
        }
        match key {
            Key::Digit(digit) if self.state != ViewerState::Holding => self.digit(digit),
            Key::Digit(_) => Action::None,
            Key::Hold => {
                self.state = match self.state {
                    ViewerState::Displaying => ViewerState::Holding,
                    ViewerState::Holding => ViewerState::Displaying,
                    other => other,
                };
                Action::None
            }
            Key::Reveal => {
                match config.reveal {
                    RevealStyle::Latched => self.reveal = !self.reveal,
                    RevealStyle::Momentary => {
                        self.reveal_until = Some(Instant(now.0.saturating_add(config.momentary_reveal_us)));
                    }
                }
                Action::None
            }
            Key::Mix => {
                self.mix = !self.mix;
                Action::None
            }
            Key::Zoom => {
                self.zoom = match self.zoom {
                    Zoom::Off => Zoom::Top,
                    Zoom::Top => Zoom::Bottom,
                    Zoom::Bottom => Zoom::Off,
                };
                Action::None
            }
            _ if !matches!(self.state, ViewerState::Displaying | ViewerState::Searching) => Action::None,
            // REQ-SEC-024: a link may be followed only while its prompt is
            // shown whole in row 24.
            Key::Red | Key::Green | Key::Yellow | Key::Cyan if !row24_shown => Action::None,
            Key::Red | Key::Green | Key::Yellow | Key::Cyan => self.follow_colour_link(key, links, confirmed),
            Key::Index => match links.index {
                Some(target) if target.service.is_none() => self.select(target.address, target.subcode, true),
                _ => self.select(initial, SubCode::ANY, true),
            },
            Key::Home => self.select(initial, SubCode::ANY, true),
            Key::NextPage | Key::PreviousPage => self.step_page(key == Key::NextPage, config.step_order, store),
            Key::NextSubPage | Key::PreviousSubPage => self.step_subpage(key == Key::NextSubPage, store),
            Key::Back => match self.history.back() {
                Some(entry) => self.select(entry.address, entry.subcode, false),
                None => Action::None,
            },
            Key::Forward => match self.history.forward() {
                Some(entry) => self.select(entry.address, entry.subcode, false),
                None => Action::None,
            },
        }
    }

    /// A key while a link to another service waits for confirmation
    /// (REQ-SEC-024): the same coloured key follows it, any other cancels.
    fn answer_confirmation(&mut self, key: Key, waiting: u8, links: &PageLinks) -> Action {
        let index = colour_index(key);
        if index == Some(waiting)
            && let Some(target) = links.keys.get(usize::from(waiting)).copied().flatten()
        {
            self.state = ViewerState::Searching;
            self.indicator = SecurityIndicator::None;
            return Action::FollowService(target);
        }
        // Any other key cancels and restores row 24.
        self.state = ViewerState::Displaying;
        self.indicator = SecurityIndicator::None;
        Action::None
    }

    /// A coloured key: follows its Fastext link, asking first when the link
    /// leaves the service and that service is not yet confirmed.
    fn follow_colour_link(&mut self, key: Key, links: &PageLinks, confirmed: &dyn Fn(u8) -> bool) -> Action {
        let index = colour_index(key).unwrap_or(0);
        match links.keys.get(usize::from(index)).copied().flatten() {
            Some(target) if target.service.is_none() => self.select(target.address, target.subcode, true),
            Some(target) => {
                let service = target.service.unwrap_or(0);
                if confirmed(service) {
                    self.state = ViewerState::Searching;
                    Action::FollowService(target)
                } else if self.state == ViewerState::Displaying {
                    self.state = ViewerState::ConfirmingLink(index);
                    self.indicator = SecurityIndicator::LeavingService;
                    Action::ConfirmService(target)
                } else {
                    Action::None
                }
            }
            None => Action::None,
        }
    }

    /// Next or previous page, in the configured step order.
    fn step_page(&mut self, forward: bool, step_order: StepOrder, store: &PageStore<'_>) -> Action {
        let target = match step_order {
            StepOrder::Numerical if forward => next_page(self.requested),
            StepOrder::Numerical => previous_page(self.requested),
            StepOrder::Stored => step_stored(self.requested, forward, store),
        };
        self.select(target, SubCode::ANY, true)
    }

    fn digit(&mut self, digit: u8) -> Action {
        match self.entry.push(digit) {
            EntryResult::Rejected => Action::None,
            EntryResult::Partial => {
                self.state = if self.entry.count() == 1 { ViewerState::Entering1 } else { ViewerState::Entering2 };
                Action::None
            }
            EntryResult::Complete(address) => self.select(address, SubCode::ANY, true),
        }
    }

    /// The next or previous stored sub-page, wrapping at either end
    /// (REQ-NAV-005). The store is asked for the neighbour of the sub-page
    /// shown, so every one of up to 8,190 is reached and the viewer holds
    /// no list (decision E.122). A page with no listed sub-pages, such as
    /// one that keeps its latest version only (REQ-NUM-021) or a data page
    /// (REQ-NUM-024), is not stepped.
    fn step_subpage(&mut self, forward: bool, store: &PageStore<'_>) -> Action {
        if self.state != ViewerState::Displaying {
            return Action::None;
        }
        let address = self.requested;
        let current =
            if self.subcode.is_any() { store.latest(address).map(|page| page.key.subcode) } else { Some(self.subcode) };
        let next = match (current, forward) {
            (Some(current), true) => store.next_subpage(address, current).or_else(|| store.first_subpage(address)),
            (Some(current), false) => store.previous_subpage(address, current).or_else(|| store.last_subpage(address)),
            (None, true) => store.first_subpage(address),
            (None, false) => store.last_subpage(address),
        };
        let Some(subcode) = next else { return Action::None };
        self.subcode = subcode;
        Action::Request(address, subcode)
    }
}

fn colour_index(key: Key) -> Option<u8> {
    match key {
        Key::Red => Some(0),
        Key::Green => Some(1),
        Key::Yellow => Some(2),
        Key::Cyan => Some(3),
        _ => None,
    }
}
