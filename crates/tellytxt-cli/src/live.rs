//! Live sources for `ttx`: the decoder side of the fetcher process
//! (REQ-SEC-018, decision 26 of IMPLEMENTATION-LOG.md).

use crate::commands::Failure;
use crate::exit;
use crate::extras::{self, PageExtras};
use crate::pipe::{self, Frame, Notice, Origin, PageData, Submission};
use crate::session::Session;
use crate::view::Source;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use tellytxt_core::packet::{Packet, PacketMeta};
use tellytxt_core::types::{Instant, PageAddress, SubCode};
use tellytxt_formats::Item;
use tellytxt_formats::detect::{AutoConfig, AutoReader, Format};
use tellytxt_formats::item::Reader;
use tellytxt_formats::t42::T42Variant;

/// A live source: the fetcher child and the frames it sends.
pub struct Live {
    child: Child,
    requests: ChildStdin,
    frames: Receiver<Frame>,
    notices: Vec<Notice>,
    extras: HashMap<(PageAddress, SubCode), PageExtras>,
    services: Vec<String>,
    shown_form_page: Option<PageAddress>,
    ended: bool,
}

/// Starts the fetcher process for `address`. Must run before sandboxing:
/// starting a program is blocked afterwards.
pub fn open(address: &str, no_sandbox: bool, allow_local: &[std::net::IpAddr]) -> Result<Box<dyn Source>, Failure> {
    let program = std::env::current_exe().map_err(|error| (exit::INPUT, error.to_string()))?;
    let mut command = Command::new(program);
    command.arg("fetch").arg(address).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    if no_sandbox {
        command.arg("--no-sandbox");
    }
    for address in allow_local {
        command.arg("--allow-local").arg(address.to_string());
    }
    let mut child = command.spawn().map_err(|error| (exit::INPUT, format!("cannot start the fetcher: {error}")))?;
    let requests = child.stdin.take().ok_or((exit::INPUT, "no pipe to the fetcher".to_string()))?;
    let mut output = child.stdout.take().ok_or((exit::INPUT, "no pipe from the fetcher".to_string()))?;
    let (sender, frames) = mpsc::channel();
    std::thread::spawn(move || {
        // Every frame is validated by pipe::read; a bad one ends the link.
        while let Ok(Some(frame)) = pipe::read(&mut output) {
            if sender.send(frame).is_err() {
                break;
            }
        }
    });
    Ok(Box::new(Live {
        child,
        requests,
        frames,
        notices: Vec::new(),
        extras: HashMap::new(),
        services: Vec::new(),
        shown_form_page: None,
        ended: false,
    }))
}

impl Live {
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
        let extras = &mut self.extras;
        let mut collect = |item: Item<'_>| match item {
            Item::Packet { packet, meta } => packets.push((packet, meta)),
            Item::Page(file) => {
                first.get_or_insert(file.address);
                extras.insert(extras::key(file), extras::from_page(file, &page.url, &services));
            }
            _ => {}
        };
        reader.feed(&page.bytes, &mut collect);
        reader.finish(&mut collect);
        for (packet, meta) in &packets {
            session.push_at(packet, meta, now);
        }
        // A fetched page is complete once its bytes are in.
        session.tick(Instant(now.0.saturating_add(tellytxt_core::consts::PAGE_COMPLETION_TIMEOUT_US * 2)));
        first
    }

    fn send(&mut self, frame: &Frame) {
        let _ = pipe::write(&mut self.requests, frame);
    }
}

impl Source for Live {
    fn poll(&mut self, session: &mut Session, now: Instant) -> bool {
        while let Ok(frame) = self.frames.try_recv() {
            match frame {
                Frame::Packet(bytes) => session.push_at(&Packet { bytes }, &PacketMeta::default(), now),
                Frame::Tti(page) => self.receive(session, Format::Tti, &page, now),
                Frame::T42(page) => self.receive(session, Format::T42(T42Variant::Plain), &page, now),
                Frame::Notice(Notice::Service(names)) => {
                    // A new service: its page numbers are its own.
                    *session = Session::new(*session.decoder().config(), session.capacity());
                    self.extras.clear();
                    self.services = names;
                }
                Frame::Notice(notice) => self.notices.push(notice),
                // The fetcher never sends these; ignore rather than trust.
                Frame::Request(..) | Frame::Submit(_) | Frame::Confirm(_) | Frame::Follow(..) => {}
            }
        }
        session.tick(now);
        if !self.ended && matches!(self.child.try_wait(), Ok(Some(_))) {
            self.ended = true;
            self.notices.push(Notice::Message("the fetcher process ended".into()));
        }
        !self.ended
    }

    fn request(&mut self, page: PageAddress, subcode: SubCode) {
        let sub = if subcode.is_any() { u16::MAX } else { subcode.value() };
        self.send(&Frame::Request(page.mpp(), sub));
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

impl Live {
    fn receive(&mut self, session: &mut Session, format: Format, page: &PageData, now: Instant) {
        if !page.secure && page.origin != Origin::Push {
            self.notices.push(Notice::Unencrypted);
        }
        let first = self.feed_page(session, format, page, now);
        if page.origin == Origin::Form {
            // A form response carrying a page is shown (REQ-WEB-011).
            self.shown_form_page = first;
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The short names of the services a descriptor file lists, for
/// `ttx check --descriptor` (REQ-CLI-006).
pub fn descriptor_service_names(path: &Path) -> Result<Vec<Vec<u8>>, Failure> {
    let json = std::fs::read(path).map_err(|error| (exit::INPUT, format!("{}: {error}", path.display())))?;
    let base = tellytxt_web::url::Url::parse("https://localhost/descriptor.json")
        .map_err(|error| (exit::INPUT, format!("{error:?}")))?;
    let descriptor = tellytxt_web::descriptor::Descriptor::parse(&json, &base)
        .map_err(|error| (exit::INPUT, format!("{}: {error:?}", path.display())))?;
    Ok(descriptor.services().map(|service| service.short_name.as_bytes().to_vec()).collect())
}
