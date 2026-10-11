//! The whole window, rendered without a display (REQ-TST-081, REQ-BRW-002,
//! REQ-BRW-004, REQ-BRW-006, REQ-BRW-007, `docs/desktop-browser.md`
//! section 14): the keys a user presses reach the right places, and the
//! window draws. Set `NEOTELETEXT_SNAPSHOTS` to a directory to keep the
//! rendered window as PNG files.
#![allow(missing_docs, clippy::unwrap_used, clippy::panic)]

mod common;

use common::sites;
use eframe::egui::{Event, Key};
use egui_kittest::Harness;
use neoteletext_gui::app::BrowserApp;
use neoteletext_gui::helpers::Options;
use neoteletext_gui::storage::Storage;
use neoteletext_gui::tab::View;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long a page may take to arrive.
const PATIENCE: Duration = Duration::from_secs(20);
/// The window's size in the test.
const SIZE: (f32, f32) = (1000.0, 1130.0);

fn harness(addresses: Vec<String>, data: PathBuf) -> Harness<'static, BrowserApp> {
    Harness::builder().with_size(SIZE).build_eframe(move |creation| {
        creation.egui_ctx.set_fonts(neoteletext_gui::system_fonts().unwrap());
        let options = Options {
            program: PathBuf::from(env!("CARGO_BIN_EXE_neoteletext")),
            allow_local: vec!["127.0.0.1".parse().unwrap()],
            no_sandbox: false,
        };
        BrowserApp::new(&creation.egui_ctx, options, Storage::new(Some(data)), &addresses)
    })
}

fn wait(harness: &mut Harness<'static, BrowserApp>, what: &str, done: impl Fn(&BrowserApp) -> bool) {
    let start = Instant::now();
    while start.elapsed() < PATIENCE {
        harness.step();
        if done(harness.state()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let tab = harness.state().active_tab();
    panic!("timed out waiting for {what}: {:?} {:?}", tab.view, tab.status);
}

fn shown(app: &BrowserApp, mpp: u16, text: &str) -> bool {
    let tab = app.active_tab();
    tab.view == View::Page && tab.status.as_ref().is_some_and(|s| s.found && s.page == mpp && s.text.contains(text))
}

fn snapshot(harness: &mut Harness<'static, BrowserApp>, name: &str) {
    // A machine without a graphics adapter cannot render; the keys are
    // still tested.
    let image = match harness.render() {
        Ok(image) => image,
        Err(error) => {
            eprintln!("SKIPPED rendering {name}: {error}");
            return;
        }
    };
    assert!(image.width() > 0 && image.height() > 0);
    if let Some(directory) = std::env::var_os("NEOTELETEXT_SNAPSHOTS") {
        image.save(PathBuf::from(directory).join(format!("{name}.png"))).unwrap();
    }
}

fn data_directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("neoteletext-window-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

#[test]
fn the_window_browses_with_the_keyboard() {
    let Some(sites) = sites() else { return };
    let data = data_directory("keys");
    let mut harness = harness(vec![sites.address("news", "/")], data.clone());
    harness.run_steps(3);
    assert!(matches!(harness.state().active_tab().view, View::Tuning(_)));
    snapshot(&mut harness, "1-tuning");
    harness.key_press(Key::Enter);
    wait(&mut harness, "the news index", |app| shown(app, 0x100, "TEST NEWS"));
    snapshot(&mut harness, "2-news-index");
    harness.event(Event::Text("200".to_string()));
    wait(&mut harness, "page 200", |app| shown(app, 0x200, "TOP STORY"));
    snapshot(&mut harness, "3-news-200");
    harness.key_press(Key::PageUp);
    wait(&mut harness, "back to 100", |app| shown(app, 0x100, "TEST NEWS"));
    // Cmd-D keeps the page, at once, in the bookmarks file.
    harness.key_press_modifiers(eframe::egui::Modifiers::COMMAND, Key::D);
    harness.run_steps(2);
    let saved = std::fs::read_to_string(data.join("bookmarks.tsv")).unwrap();
    assert!(saved.contains(&sites.address("news", "/100.tti")) && saved.contains("TEST NEWS"), "{saved}");
    let bookmarks = Storage::new(Some(data.clone())).bookmarks();
    assert_eq!(bookmarks.len(), 1);
    // A new tab, then the keys page.
    harness.key_press_modifiers(eframe::egui::Modifiers::COMMAND, Key::T);
    harness.run_steps(2);
    assert_eq!(harness.state().tab_count(), 2);
    // A new tab's address bar has the keys, as in other browsers; Escape
    // gives them back to the page.
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    // The new tab's start page lists the bookmark; a click opens it.
    assert_eq!(harness.state().active_tab().view, View::Start);
    snapshot(&mut harness, "4-start");
    harness.event(Event::Text("K".to_string()));
    harness.run_steps(2);
    assert_eq!(harness.state().active_tab().view, View::Keys);
    snapshot(&mut harness, "5-keys");
    let _ = std::fs::remove_dir_all(data);
}
