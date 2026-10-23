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
        let page = PageData { secure: true, origin, url: "https://a.example/100.t42".into(), bytes: two_pages.clone() };
        sender.send(Frame::Tti(page)).unwrap_or_else(|_| unreachable!());
        live.poll(&mut session, Instant(SECOND));
        let present: Vec<bool> =
            [0x100, 0x200, 0x1A0].iter().map(|&mpp| session.page(address(mpp), SubCode::ANY).is_some()).collect();
        assert_eq!(present, stored, "{origin:?}");
    }
}
