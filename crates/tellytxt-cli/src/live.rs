//! Live sources for `ttx view`: the decoder side of the fetcher process
//! (REQ-SEC-018, decision 26 of IMPLEMENTATION-LOG.md).

use crate::commands::Failure;
use crate::exit;
use crate::pipe::{self, Frame, Notice};
use crate::session::Session;
use crate::view::Source;
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
    ended: bool,
}

/// Starts the fetcher process for `address`. Must run before sandboxing:
/// starting a program is blocked afterwards.
pub fn open(address: &str, no_sandbox: bool) -> Result<Box<dyn Source>, Failure> {
    let program = std::env::current_exe().map_err(|error| (exit::INPUT, error.to_string()))?;
    let mut command = Command::new(program);
    command.arg("fetch").arg(address).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    if no_sandbox {
        command.arg("--no-sandbox");
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
    Ok(Box::new(Live { child, requests, frames, notices: Vec::new(), ended: false }))
}

fn feed_page(session: &mut Session, format: Format, bytes: &[u8], now: Instant) {
    let mut reader =
        Box::new(AutoReader::new(AutoConfig { format: Some(format), encode_pages: true, ..AutoConfig::default() }));
    let mut packets: Vec<(Packet, PacketMeta)> = Vec::new();
    let mut collect = |item: Item<'_>| {
        if let Item::Packet { packet, meta } = item {
            packets.push((packet, meta));
        }
    };
    reader.feed(bytes, &mut collect);
    reader.finish(&mut collect);
    for (packet, meta) in &packets {
        session.push_at(packet, meta, now);
    }
    // A fetched page is complete once its bytes are in.
    session.tick(Instant(now.0.saturating_add(tellytxt_core::consts::PAGE_COMPLETION_TIMEOUT_US * 2)));
}

impl Source for Live {
    fn poll(&mut self, session: &mut Session, now: Instant) -> bool {
        while let Ok(frame) = self.frames.try_recv() {
            match frame {
                Frame::Packet(bytes) => session.push_at(&Packet { bytes }, &PacketMeta::default(), now),
                Frame::Tti(secure, bytes) => {
                    if !secure {
                        self.notices.push(Notice::Unencrypted);
                    }
                    feed_page(session, Format::Tti, &bytes, now);
                }
                Frame::T42(secure, bytes) => {
                    if !secure {
                        self.notices.push(Notice::Unencrypted);
                    }
                    feed_page(session, Format::T42(T42Variant::Plain), &bytes, now);
                }
                Frame::Notice(notice) => self.notices.push(notice),
                // The fetcher never sends requests; ignore rather than trust.
                Frame::Request(..) => {}
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
        let _ = pipe::write(&mut self.requests, &Frame::Request(page.mpp(), sub));
    }

    fn notices(&mut self) -> Vec<Notice> {
        std::mem::take(&mut self.notices)
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
