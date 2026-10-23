//! Live sources: the decoder side of the fetcher process
//! (REQ-SEC-018, decision 26 of IMPLEMENTATION-LOG.md).

use crate::extras::{self, PageExtras};
use crate::pipe::{self, Frame, Notice, Origin, PageData, Submission};
use crate::session::Session;
use crate::source::{Source, Wake};
use fill::Fill;
use neoteletext_core::consts::{PACKET_HEADER, PACKET_MAGAZINE_ENHANCEMENT};
use neoteletext_core::packet::{Decoded, Packet, PacketMeta, decode, decode_address};
use neoteletext_core::types::{Instant, PageAddress, SubCode};
use neoteletext_formats::Item;
use neoteletext_formats::detect::{AutoConfig, AutoReader, Format};
use neoteletext_formats::item::Reader;
use neoteletext_formats::t42::T42Variant;
use std::collections::HashMap;
use std::io::Write;
use std::process::Child;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, OnceLock};

mod fill;
mod spawn;

pub use spawn::open;

/// A live source: the fetcher and the frames it sends.
pub struct Live {
    /// The fetcher process; none when the frames come from a channel, as
    /// in the tests.
    child: Option<Child>,
    requests: Box<dyn Write + Send>,
    frames: Receiver<Frame>,
    notices: Vec<Notice>,
    extras: HashMap<(PageAddress, SubCode), PageExtras>,
    services: Vec<String>,
    shown_form_page: Option<PageAddress>,
    ended: bool,
    /// The bytes each page last arrived as, to take in again when the
    /// server confirms them current (REQ-WEB-009).
    last_bytes: HashMap<u16, (Format, PageData)>,
    /// The page the viewer asked for last.
    requested: Option<(PageAddress, SubCode)>,
    /// When the last fetch failed for want of a network, if it has not
    /// succeeded since (REQ-WEB-009).
    offline_since: Option<Instant>,
    /// When the requested page was last asked for again.
    last_retry: Instant,
    /// The time of the latest poll, the viewer's clock.
    last_poll: Instant,
    /// The service's refresh interval for the page on screen, when its
    /// descriptor gives one and it has no push (REQ-WEB-020).
    refresh_us: Option<u64>,
    /// Every page asked for, `0xMPP`: a page's file may fill only these.
    asked: std::collections::HashSet<u16>,
    /// Called by the pipe reader as each frame arrives, once the viewer
    /// asks (REQ-INT-008).
    wake: Arc<OnceLock<Wake>>,
}

/// Bounds of a service's refresh interval, in seconds (concept decision 13).
const MIN_REFRESH_SECONDS: u32 = neoteletext_web::descriptor::MIN_REFRESH_SECONDS;
const MAX_REFRESH_SECONDS: u32 = neoteletext_web::descriptor::MAX_REFRESH_SECONDS;
/// Microseconds in a second.
const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
/// Microseconds between requests for the viewed page while offline, so
/// that it is the first page checked when the network returns
/// (REQ-WEB-009).
const OFFLINE_RETRY_US: u64 = 10_000_000;

impl Live {
    fn new(child: Option<Child>, requests: Box<dyn Write + Send>, frames: Receiver<Frame>) -> Self {
        Self {
            child,
            requests,
            frames,
            notices: Vec::new(),
            extras: HashMap::new(),
            services: Vec::new(),
            shown_form_page: None,
            ended: false,
            last_bytes: HashMap::new(),
            requested: None,
            offline_since: None,
            last_retry: Instant(0),
            last_poll: Instant(0),
            refresh_us: None,
            asked: std::collections::HashSet::new(),
            wake: Arc::default(),
        }
    }

    /// Decodes page bytes into the session, keeping each page's extension
    /// data. Returns the first page the bytes held.
    fn feed_page(
        &mut self,
        session: &mut Session,
        format: Format,
        page: &PageData,
        now: Instant,
    ) -> Option<PageAddress> {
        let mut reader =
            Box::new(AutoReader::new(AutoConfig { format: Some(format), encode_pages: true, ..AutoConfig::default() }));
        let mut packets: Vec<(Packet, PacketMeta)> = Vec::new();
        let mut first = None;
        let services = self.services.clone();
        let allowed = if page.origin == Origin::Page { Fill::Asked(&self.asked) } else { Fill::Any };
        let extras = &mut self.extras;
        let mut collect = |item: Item<'_>| match item {
            Item::Packet { packet, meta } => packets.push((packet, meta)),
            Item::Page(file) if allowed.allows(file.address) => {
                first.get_or_insert(file.address);
                extras.insert(extras::key(file), extras::from_page(file, &page.url, &services));
            }
            _ => {}
        };
        reader.feed(&page.bytes, &mut collect);
        reader.finish(&mut collect);
        // Each magazine's packets belong to the page its last header
        // named; magazine-wide packets (M/29, 8/30) always pass.
        let mut filling = [true; neoteletext_core::consts::MAGAZINES];
        for (packet, meta) in &packets {
            let Ok(address) = decode_address(packet) else { continue };
            let slot = usize::from(address.magazine.bits());
            if address.number == PACKET_HEADER
                && let Ok(Decoded::Header(header)) = decode(packet)
            {
                let page = header.page.map(|number| PageAddress { magazine: header.magazine, page: number });
                if let Some(state) = filling.get_mut(slot) {
                    *state = page.is_some_and(|page| allowed.allows(page));
                }
            }
            let magazine_wide = address.number >= PACKET_MAGAZINE_ENHANCEMENT;
            if magazine_wide || filling.get(slot).copied().unwrap_or(false) {
                session.push_at(packet, meta, now);
            }
        }
        // A fetched page is complete once its bytes are in.
        session.tick(Instant(now.0.saturating_add(neoteletext_core::consts::PAGE_COMPLETION_TIMEOUT_US * 2)));
        first
    }

    fn send(&mut self, frame: &Frame) {
        let _ = pipe::write(&mut self.requests, frame);
    }

    fn receive(&mut self, session: &mut Session, format: Format, page: &PageData, now: Instant) {
        if !page.secure && page.origin != Origin::Push {
            self.notices.push(Notice::Unencrypted);
        }
        self.offline_since = None;
        let first = self.feed_page(session, format, page, now);
        if let Some(address) = first
            && page.origin == Origin::Page
        {
            self.last_bytes.insert(address.mpp(), (format, page.clone()));
        }
        if page.origin == Origin::Form {
            // A form response carrying a page is shown (REQ-WEB-011).
            self.shown_form_page = first;
        }
        if page.origin == Origin::FrontDoor
            && let Some(first) = first
        {
            self.notices.push(Notice::Bundle(first.mpp()));
        }
    }
}

impl Source for Live {
    fn set_wake(&mut self, wake: Wake) {
        let _ = self.wake.set(wake);
    }

    fn poll(&mut self, session: &mut Session, now: Instant) -> bool {
        self.last_poll = now;
        let mut disconnected = false;
        loop {
            let frame = match self.frames.try_recv() {
                Ok(frame) => frame,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            };
            match frame {
                Frame::Packet(bytes) => session.push_at(&Packet { bytes }, &PacketMeta::default(), now),
                Frame::Tti(page) => self.receive(session, Format::Tti, &page, now),
                Frame::T42(page) => self.receive(session, Format::T42(T42Variant::Plain), &page, now),
                Frame::Notice(Notice::Service(names)) => {
                    // A new service: its page numbers are its own.
                    *session = Session::new(*session.decoder().config(), session.capacity());
                    self.extras.clear();
                    self.last_bytes.clear();
                    self.services = names;
                }
                Frame::Notice(Notice::Refresh(seconds)) => {
                    // The descriptor bounds it; the pipe could carry any value.
                    let bounded = seconds.clamp(MIN_REFRESH_SECONDS, MAX_REFRESH_SECONDS);
                    self.refresh_us = Some(u64::from(bounded) * MICROSECONDS_PER_SECOND);
                }
                Frame::Notice(Notice::Current(mpp)) => {
                    // A page confirmed unchanged is as good as received
                    // again: it is current, and not stale (REQ-WEB-009).
                    self.offline_since = None;
                    if let Some((format, page)) = self.last_bytes.get(&mpp).cloned() {
                        self.feed_page(session, format, &page, now);
                    }
                }
                Frame::Notice(Notice::Offline(mpp)) => {
                    self.offline_since.get_or_insert(now);
                    let stored =
                        PageAddress::from_mpp(mpp).is_some_and(|page| session.page(page, SubCode::ANY).is_some());
                    if !stored {
                        self.notices.push(Notice::Offline(mpp));
                    }
                }
                Frame::Notice(notice) => self.notices.push(notice),
                // The fetcher never sends these to a decoder; ignore rather
                // than trust.
                Frame::Request(..) | Frame::Submit(_) | Frame::Confirm(_) | Frame::Follow(..) | Frame::Input(_) => {}
            }
        }
        session.tick(now);
        // The page on screen is asked for again before it would go stale,
        // and more often while offline, so that it is the first page
        // checked when the network returns (REQ-WEB-009). The web module's
        // cache answers without the network while the page is fresh.
        let fresh = session.decoder().config().stale_after_us / 2;
        let interval = if self.offline_since.is_some() {
            OFFLINE_RETRY_US
        } else {
            self.refresh_us.map_or(fresh, |refresh| refresh.min(fresh))
        };
        if now.since(self.last_retry) >= interval
            && let Some((page, subcode)) = self.requested
        {
            self.last_retry = now;
            self.request(page, subcode);
        }
        let gone = match &mut self.child {
            Some(child) => matches!(child.try_wait(), Ok(Some(_))),
            None => disconnected,
        };
        if !self.ended && gone {
            self.ended = true;
            self.notices.push(Notice::Message("the fetcher process ended".into()));
        }
        !self.ended
    }

    fn request(&mut self, page: PageAddress, subcode: SubCode) {
        self.asked.insert(page.mpp());
        self.requested = Some((page, subcode));
        self.last_retry = self.last_poll;
        let sub = if subcode.is_any() { u16::MAX } else { subcode.value() };
        self.send(&Frame::Request(page.mpp(), sub));
    }

    fn prefetch(&mut self, page: PageAddress) {
        self.asked.insert(page.mpp());
        self.send(&Frame::Request(page.mpp(), u16::MAX));
    }

    fn notices(&mut self) -> Vec<Notice> {
        let mut notices = std::mem::take(&mut self.notices);
        if let Some(page) = self.shown_form_page.take() {
            notices.push(Notice::ShowPage(page.mpp()));
        }
        notices
    }

    fn extras(&self, page: PageAddress, subcode: SubCode) -> Option<&PageExtras> {
        self.extras
            .get(&(page, subcode))
            .or_else(|| self.extras.iter().find(|((address, _), _)| *address == page).map(|(_, extras)| extras))
    }

    fn submit(&mut self, submission: Submission) {
        self.send(&Frame::Submit(submission));
    }

    fn confirm(&mut self, yes: bool) {
        self.send(&Frame::Confirm(yes));
    }

    fn send_input(&mut self, message: &[u8]) {
        self.send(&Frame::Input(message.to_vec()));
    }

    fn follow(&mut self, service: u8, page: PageAddress, subcode: SubCode) {
        if let Some(name) = self.services.get(usize::from(service)).cloned() {
            let sub = if subcode.is_any() { u16::MAX } else { subcode.value() };
            self.send(&Frame::Follow(name, page.mpp(), sub));
        }
    }

    fn service_name(&self, service: u8) -> Option<String> {
        self.services.get(usize::from(service)).cloned()
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests;
