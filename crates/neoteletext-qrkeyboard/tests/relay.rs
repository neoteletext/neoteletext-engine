//! The screen and the phone paired through a running qrkeyboard relay:
//! `QRKEYBOARD_RELAY=http://127.0.0.1:8791` names one, such as the relay's
//! own `npm run dev -- --port 8791 --ip 127.0.0.1`. Without it these tests
//! pass without running, and say so.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use neoteletext_qrkeyboard::config::{Layout, ScreenConfig};
use neoteletext_qrkeyboard::phone::Phone;
use neoteletext_qrkeyboard::screen::{Event, Join, Screen};

const POLL: Duration = Duration::from_millis(50);
const LIMIT: Duration = Duration::from_secs(30);

fn relay() -> Option<String> {
    let relay = std::env::var("QRKEYBOARD_RELAY").ok();
    if relay.is_none() {
        eprintln!("QRKEYBOARD_RELAY is not set: no relay to pair through, test not run");
    }
    relay
}

/// Runs a screen in its own thread, sending its join details and then
/// every event, until it finishes or the limit passes.
fn screen(relay: String, config: ScreenConfig) -> (Join, mpsc::Receiver<Event>) {
    let (join_sender, join) = mpsc::channel();
    let (events_sender, events) = mpsc::channel();
    thread::spawn(move || {
        let mut screen = Screen::create(&relay, config).unwrap();
        join_sender.send(screen.join().clone()).unwrap();
        let started = Instant::now();
        while started.elapsed() < LIMIT {
            for event in screen.poll(POLL).unwrap() {
                let finished = event == Event::Finished;
                if events_sender.send(event).is_err() || finished {
                    return;
                }
            }
        }
    });
    (join.recv_timeout(LIMIT).unwrap(), events)
}

fn until(events: &mpsc::Receiver<Event>, wanted: &Event) -> Vec<Event> {
    let mut seen = Vec::new();
    while let Ok(event) = events.recv_timeout(LIMIT) {
        let done = event == *wanted;
        seen.push(event);
        if done {
            return seen;
        }
    }
    panic!("never saw {wanted:?}: {seen:?}");
}

#[test]
fn a_phone_pairs_by_key_and_types() {
    let Some(relay) = relay() else { return };
    let config =
        ScreenConfig { name: "neoteletext test".into(), layout: Layout::Terminal, pin: false, rejoin_seconds: Some(0) };
    let (join, events) = screen(relay, config);
    assert!(join.url.contains("&k=") && join.pin.is_none());
    let mut phone = Phone::join(&join.url, None).unwrap();
    until(&events, &Event::Connected);
    assert!(phone.press("ColorF0Red").unwrap());
    assert!(phone.type_text("hé").unwrap());
    let seen = until(&events, &Event::Text("hé".into()));
    assert!(
        seen.contains(&Event::Down("ColorF0Red".into())) && seen.contains(&Event::Up("ColorF0Red".into())),
        "{seen:?}"
    );
    phone.finish().unwrap();
    until(&events, &Event::Finished);
}

#[test]
fn a_phone_pairs_by_pin_and_a_wrong_pin_fails() {
    let Some(relay) = relay() else { return };
    let config = ScreenConfig {
        name: "neoteletext test".into(),
        layout: Layout::Preset("teletext"),
        pin: true,
        rejoin_seconds: Some(0),
    };
    let (join, events) = screen(relay, config);
    let pin = join.pin.clone().unwrap();
    assert!(!join.url.contains("&k="), "no key in a PIN code's QR");
    let wrong = if pin == "1234" { "4321" } else { "1234" };
    assert!(Phone::join(&join.url, Some(wrong)).is_err());
    until(&events, &Event::PinFailed(4));
    let mut phone = Phone::join(&join.url, Some(&pin)).unwrap();
    until(&events, &Event::Connected);
    assert!(phone.press("5").unwrap());
    until(&events, &Event::Up("5".into()));
    phone.leave();
}
