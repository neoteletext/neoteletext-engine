//! The whole window, rendered without a display (REQ-TST-081, REQ-BRW-002,
//! REQ-BRW-004, REQ-BRW-006, REQ-BRW-007, `prototypes/browser/docs/desktop-browser.md`
//! section 14): the keys a user presses reach the right places, and the
//! window draws. Set `NEOTELETEXT_SNAPSHOTS` to a directory to keep the
//! rendered window as PNG files.
#![allow(missing_docs, clippy::unwrap_used, clippy::panic)]

mod common;

use common::sites;
use eframe::egui::{Event, Key};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
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

/// The place of G0 Latin with the English option, the set of the test
/// sites' pages, in the user's fonts by set (REQ-FNT-005).
fn english() -> usize {
    neoteletext_core::glyphs::conformance::font_set_index("G0-Latin-English").unwrap()
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

/// REQ-BRW-007, REQ-FNT-004, REQ-FNT-005: the Fonts settings chosen with
/// the pointer and the keyboard: a library added and a font chosen from it,
/// "always use my font", fonts from other sites, and the cache cleared;
/// each saved at once, and the status line naming the font and who chose
/// it.
#[test]
fn the_font_settings() {
    let Some(sites) = sites() else { return };
    let data = data_directory("fonts");
    let mut harness = harness(vec![sites.address("styled", "/")], data.clone());
    harness.run_steps(3);
    harness.key_press(Key::Enter);
    wait(&mut harness, "the channel's font", |app| app.font_label() == "Channel stripe (the channel's font)");
    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.get_by_label("Fonts").click();
    harness.run_steps(2);
    let library = sites.address("fontsite", "/lib/");
    let field = harness.get_by_label("A library's address");
    field.click();
    field.type_text(&library);
    harness.run_steps(2);
    harness.get_by_label("Add library").click();
    wait(&mut harness, "the library's listing", |app| app.settings().libraries == [library.clone()]);
    let start = Instant::now();
    while harness.query_by_label("Library stripe").is_none() && start.elapsed() < PATIENCE {
        harness.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    harness.get_by_label("Library stripe").click();
    wait(&mut harness, "the user's font", |app| {
        app.settings().fonts[english()].as_ref().is_some_and(|font| font.title == "Library stripe")
    });
    let saved = std::fs::read_to_string(data.join("settings.txt")).unwrap();
    assert!(saved.contains(&format!("library={library}")) && saved.contains("Library stripe"), "{saved}");
    // The channel's font still wins until the user insists.
    assert_eq!(harness.state().font_label(), "Channel stripe (the channel's font)");
    harness.get_by_label("Always use my fonts").click();
    wait(&mut harness, "the user's font in use", |app| app.font_label() == "Library stripe (your font)");
    snapshot(&mut harness, "6-fonts");
    harness.get_by_label("Allow fonts from other sites").click();
    harness.run_steps(2);
    let saved = std::fs::read_to_string(data.join("settings.txt")).unwrap();
    assert!(saved.contains("always_my_font=1") && saved.contains("fonts_from_other_sites=0"), "{saved}");
    let index = std::fs::read_to_string(data.join("fonts").join("index.tsv")).unwrap();
    assert!(index.contains("Library stripe") && index.contains("Channel stripe"), "{index}");
    harness.get_by_label("Remove every cached font").click();
    harness.run_steps(2);
    assert_eq!(std::fs::read_to_string(data.join("fonts").join("index.tsv")).unwrap(), "");
    let _ = std::fs::remove_dir_all(data);
}

/// REQ-BRW-010, decision E.74: the font browser lists a library's fonts,
/// and clicking or arrowing through them chooses the user's font.
#[test]
fn the_font_browser() {
    let Some(sites) = sites() else { return };
    let data = data_directory("font-browser");
    let library = sites.address("fontsite", "/lib/");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("settings.txt"), format!("font_browser=1\nlibrary={library}\n")).unwrap();
    let mut harness = harness(vec![sites.address("news", "/")], data.clone());
    harness.run_steps(3);
    harness.key_press(Key::Enter);
    wait(&mut harness, "the news front page", |app| shown(app, 0x100, "NEWS"));
    // REQ-BRW-011: the page's designation, step by step. A TTI page sends no
    // X/28, so the group is the receiver's.
    harness.run_steps(3);
    for text in [
        "P100: how its character sets were chosen",
        "from the receiver's setting",
        "Group 0000, West European",
        "Sets this page uses: G0 Latin, English · G2 Latin",
    ] {
        assert!(harness.query_by_label_contains(text).is_some(), "{text}");
    }
    let start = Instant::now();
    while harness.query_by_label("Library stripe").is_none() && start.elapsed() < PATIENCE {
        harness.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    // Only the index is fetched to list the fonts (REQ-FNT-004).
    assert_eq!(harness.state().font_label(), "Bedstead (the bundled font)");
    harness.get_by_label("Library stripe").click();
    let ours = |title: &'static str| {
        move |app: &BrowserApp| {
            app.settings().fonts[english()].as_ref().is_some_and(|font| font.title == title)
                && app.font_label() == format!("{title} (your font)")
        }
    };
    wait(&mut harness, "the clicked font", ours("Library stripe"));
    harness.key_press(Key::ArrowDown);
    wait(&mut harness, "the next font", ours("Page stripe"));
    harness.key_press(Key::ArrowDown);
    wait(&mut harness, "the copy", ours("Library copy"));
    // Issue #207: once loaded, the copy draws the same as Library stripe,
    // so leaving it hides it, and the end of the list is Page stripe.
    harness.key_press(Key::ArrowUp);
    wait(&mut harness, "back to the next font", ours("Page stripe"));
    harness.run_steps(3);
    assert!(harness.query_by_label("Library copy").is_none(), "the duplicate is hidden");
    snapshot(&mut harness, "7-font-browser");
    harness.key_press(Key::ArrowDown);
    harness.run_steps(3);
    assert!(ours("Page stripe")(harness.state()));
    let index = std::fs::read_to_string(data.join("fonts").join("index.tsv")).unwrap();
    // The glyph digest is the index's thirteenth field (REQ-FNT-009).
    let digest = |name: &str| index.lines().find(|line| line.contains(name)).and_then(|line| line.split('\t').nth(12));
    assert_eq!(digest("Library copy"), digest("Library stripe"), "one glyph digest: {index}");
    assert_ne!(digest("Page stripe"), digest("Library stripe"));
    harness.get_by_label("Show duplicates").click();
    harness.run_steps(3);
    assert!(harness.query_by_label("Library copy").is_some(), "shown on request");
    harness.get_by_label("Show duplicates").click();
    harness.run_steps(3);
    assert!(harness.query_by_label("Library copy").is_none());
    // Issue #208: the sample replaces the page while it is on.
    harness.get_by_label("Show sample").click();
    harness.run_steps(3);
    assert!(harness.query_by_label("Font sample").is_some(), "the sample is shown");
    snapshot(&mut harness, "8-font-sample");
    harness.get_by_label("Show sample").click();
    harness.run_steps(3);
    assert!(harness.query_by_label("Font sample").is_none(), "the page is back");
    harness.key_press(Key::ArrowUp);
    harness.key_press(Key::ArrowUp);
    wait(&mut harness, "Bedstead", |app| {
        app.settings().fonts[english()].is_none() && app.font_label() == "Bedstead (the bundled font)"
    });
    // Esc gives the arrow keys back to the page: no font is chosen.
    harness.key_press(Key::Escape);
    harness.key_press(Key::ArrowDown);
    harness.run_steps(3);
    assert!(harness.state().settings().fonts[english()].is_none());
    let saved = std::fs::read_to_string(data.join("settings.txt")).unwrap();
    assert!(saved.lines().any(|line| line == "font_browser=1"), "{saved}");
    assert!(!saved.lines().any(|line| line.starts_with("font.")), "Bedstead is no saved font: {saved}");
    let _ = std::fs::remove_dir_all(data);
}

#[test]
fn the_font_switcher_steps_with_the_arrow_keys() {
    let Some(sites) = sites() else { return };
    let data = data_directory("font-switcher");
    let (library, second) = (sites.address("fontsite", "/lib/"), sites.address("fontsite", "/second/"));
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("settings.txt"), format!("font_browser=1\nlibrary={library}\nlibrary={second}\n"))
        .unwrap();
    let mut harness = harness(vec![sites.address("news", "/")], data);
    harness.run_steps(3);
    harness.key_press(Key::Enter);
    wait(&mut harness, "the news front page", |app| shown(app, 0x100, "NEWS"));
    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.get_by_label("Switch font").click();
    harness.run_steps(2);
    let start = Instant::now();
    while harness.query_by_label_contains("Library stripe again   ").is_none() && start.elapsed() < PATIENCE {
        harness.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    harness.get_by_label_contains("Library stripe   ").click();
    let ours = |title: &'static str| {
        move |app: &BrowserApp| app.settings().fonts[english()].as_ref().is_some_and(|font| font.title == title)
    };
    wait(&mut harness, "the clicked font", ours("Library stripe"));
    harness.key_press(Key::ArrowDown);
    wait(&mut harness, "the next font", ours("Page stripe"));
    // The same font listed again in the second library: the arrow steps to
    // the entry after the one clicked, not after the font's first listing.
    harness.get_by_label_contains("Library stripe again   ").click();
    wait(&mut harness, "the font clicked in the second library", ours("Library stripe again"));
    harness.key_press(Key::ArrowDown);
    wait(&mut harness, "the next entry of the second library", ours("Library copy again"));
}
