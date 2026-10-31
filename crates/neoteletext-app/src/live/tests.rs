#![allow(clippy::unwrap_used, clippy::unreachable, reason = "a test")]
use super::spawn::read_frames;
use super::*;

/// A live source fed `frames` from a channel, writing its requests to
/// `requests`, for the tests.
fn routed(frames: Receiver<Frame>, requests: Box<dyn Write + Send>) -> Live {
    Live::new(None, requests, frames)
}

#[test]
fn the_pipe_reader_wakes_the_viewer_for_each_frame_and_at_the_end() {
    // The viewer took a page only at its next 40 ms look; it is woken
    // as each frame arrives instead (REQ-INT-008).
    let mut bytes = Vec::new();
    for _ in 0..2 {
        pipe::write(&mut bytes, &Frame::Notice(Notice::Unencrypted)).unwrap();
    }
    let woken = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = woken.clone();
    let wake: OnceLock<Wake> = OnceLock::new();
    let _ = wake.set(Arc::new(move || {
        count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }));
    let (sender, frames) = mpsc::channel();
    read_frames(&mut bytes.as_slice(), &sender, &wake);
    assert_eq!(frames.try_iter().count(), 2);
    assert_eq!(woken.load(std::sync::atomic::Ordering::Relaxed), 3, "each frame, then the end");
}
use std::sync::{Arc, Mutex};

/// A pipe end that keeps what is written.
#[derive(Clone, Default)]
struct Kept(Arc<Mutex<Vec<u8>>>);

impl Write for Kept {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().map_err(|_| std::io::Error::other("poisoned"))?.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Kept {
    fn requests(&self) -> usize {
        let bytes = self.0.lock().map(|bytes| bytes.clone()).unwrap_or_default();
        let mut reader = bytes.as_slice();
        std::iter::from_fn(|| pipe::read(&mut reader).ok().flatten())
            .filter(|frame| matches!(frame, Frame::Request(..)))
            .count()
    }
}

const SECOND: u64 = 1_000_000;

/// REQ-1672 items 1 and 2: a link holds an `http` or `https` address, its
/// scheme in either case, and nothing else.
#[test]
fn a_link_from_outside_unwraps_to_its_address() {
    use crate::input::{is_live, link_address};
    assert_eq!(link_address("neoteletext://https://a.example/demo/"), Ok("https://a.example/demo/"));
    assert_eq!(link_address("neoteletext://http://a.example/demo/?id=7"), Ok("http://a.example/demo/?id=7"));
    assert_eq!(link_address("NeoTeletext://HTTPS://a.example/demo/"), Ok("HTTPS://a.example/demo/"));
    assert_eq!(link_address("pages.t42"), Ok("pages.t42"));
    for refused in [
        "neoteletext://home",
        "neoteletext://",
        "neoteletext://file:///etc/passwd",
        "neoteletext://web:https://a.example/",
        "neoteletext://neoteletext://https://a.example/",
        "neoteletext://wss://a.example/",
    ] {
        assert!(link_address(refused).is_err(), "{refused}");
    }
    assert!(is_live("neoteletext://https://a.example/demo/"));
    assert!(is_live("NEOTELETEXT://home"), "a refused link is refused, not read as a file");
    assert!(!is_live("neoteletext:home"));
}

#[test]
fn a_new_service_keeps_the_records_the_viewer_asked_for() {
    // The viewer asks once, before the fetcher announces the service; had
    // the announcement forgotten, no WELCOME would reach it and keys would
    // never go to the channel.
    let (sender, frames) = mpsc::channel();
    let mut live = routed(frames, Box::new(std::io::sink()));
    let mut session = Session::new(neoteletext_core::config::Config::default(), 1);
    session.keep(|_| true);
    sender.send(Frame::Notice(Notice::Service(Vec::new()))).unwrap();
    sender.send(Frame::Packet([0; 42])).unwrap();
    live.poll(&mut session, Instant(0));
    assert_eq!(session.take_kept().len(), 1);
}

/// REQ-WEB-020, concept decision 13: a service's refresh interval
/// brings the page on screen again sooner than the stale timeout would.
#[test]
fn the_refresh_interval_asks_for_the_page_again() {
    let (sender, frames) = mpsc::channel();
    let kept = Kept::default();
    let mut live = routed(frames, Box::new(kept.clone()));
    let mut session = Session::new(neoteletext_core::config::Config::default(), 8);
    let page = PageAddress::from_mpp(0x100).unwrap_or_else(|| unreachable!());
    live.poll(&mut session, Instant(0));
    live.request(page, SubCode::ANY);
    assert_eq!(kept.requests(), 1);
    live.poll(&mut session, Instant(6 * SECOND));
    assert_eq!(kept.requests(), 1, "without a refresh interval, not yet");
    sender.send(Frame::Notice(Notice::Refresh(5))).unwrap_or_else(|_| unreachable!());
    live.poll(&mut session, Instant(7 * SECOND));
    assert_eq!(kept.requests(), 2, "five seconds have passed since the request");
    live.poll(&mut session, Instant(9 * SECOND));
    assert_eq!(kept.requests(), 2);
    live.poll(&mut session, Instant(13 * SECOND));
    assert_eq!(kept.requests(), 3);
}

/// REQ-WEB-019, concept section 2 item 4: a page's own file fills only
/// that page, while a front door fills every page it holds.
#[test]
fn a_page_file_fills_only_its_page() {
    let two_pages = b"PN,10000\nOL,1,ONE\nPN,20000\nOL,1,TWO\nPN,1A000\nOL,1,HEX\n".to_vec();
    let address = |mpp| PageAddress::from_mpp(mpp).unwrap_or_else(|| unreachable!());
    for (origin, stored) in [(Origin::Page, [true, false, true]), (Origin::FrontDoor, [true, true, true])] {
        let (sender, frames) = mpsc::channel();
        let mut live = routed(frames, Box::new(Kept::default()));
        let mut session = Session::new(neoteletext_core::config::Config::default(), 8);
        live.request(address(0x100), SubCode::ANY);
        let page = PageData { secure: true, origin, bytes: two_pages.clone() };
        sender.send(Frame::Tti(page)).unwrap_or_else(|_| unreachable!());
        live.poll(&mut session, Instant(SECOND));
        let present: Vec<bool> =
            [0x100, 0x200, 0x1A0].iter().map(|&mpp| session.page(address(mpp), SubCode::ANY).is_some()).collect();
        assert_eq!(present, stored, "{origin:?}");
    }
}

/// REQ-1693: a viewer opens the map's addresses, normalised, and refuses the
/// rest; a service's descriptor and a channel's own file name no place.
#[test]
fn an_address_says_where_a_viewer_opens() {
    use crate::input::{Opening, opening};
    use neoteletext_core::types::{PageAddress, SubCode};
    let page = |mpp| PageAddress::from_mpp(mpp).unwrap();
    let at = |mpp, code, parameters: &str| Opening { page: Some((page(mpp), code)), parameters: parameters.to_owned() };
    for (address, opens) in [
        ("https://a.example/home/#302", at(0x302, SubCode::ANY, "")),
        ("neoteletext://https://a.example/home/#302", at(0x302, SubCode::ANY, "")),
        ("https://a.example/home/?id=7#1fe", at(0x1FE, SubCode::ANY, "")),
        ("https://a.example/home/#302/0002", at(0x302, SubCode::new(2), "")),
        ("https://a.example/home/#302?a=1&b=", at(0x302, SubCode::ANY, "a=1&b=")),
        ("https://a.example/home/302.tti", at(0x302, SubCode::ANY, "")),
        ("https://a.example/home/302-0002.t42", at(0x302, SubCode::new(2), "")),
        ("https://a.example/home/", Opening::default()),
        ("https://a.example/home/#contents", Opening::default()),
        ("https://a.example/home/#wall", Opening::default()),
        ("https://a.example/home/channel.json", Opening::default()),
        ("https://a.example/service.json", Opening::default()),
        ("pages.t42", Opening::default()),
    ] {
        assert_eq!(opening(address), Ok(opens), "{address}");
    }
    for address in [
        "https://a.example/home/#",
        "https://a.example/home/#G02",
        "https://a.example/home/#302:0002",
        "https://a.example/home/#302/3F7F",
        "https://a.example/home/302.tti#303",
        "https://a.example/home/channel.json#302",
        "neoteletext://https://a.example/home/#WALL",
    ] {
        assert!(opening(address).is_err(), "{address}");
    }
}
