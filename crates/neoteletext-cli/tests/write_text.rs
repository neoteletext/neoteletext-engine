//! `neottx write-text` (REQ-CLI-010): a page written from text in one of
//! neoteletext's own sets decodes to the same text, with the designation in
//! X/28/0 and the tones in X/26, and a refusal names the character.
#![allow(missing_docs)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn neottx(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_neottx")).args(arguments).output().expect("run neottx")
}

/// A fresh folder of the test's own: tests run in parallel.
fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("make the test folder");
    folder
}

/// Writes `text` in `set` as page 100 and reads rows 1 onwards back at
/// `level`, trailing spaces removed.
fn round_trip(name: &str, set: &str, text: &str, level: &str) -> Vec<String> {
    let folder = folder(name);
    let input = folder.join("page.txt");
    let output = folder.join("P100.tti");
    std::fs::write(&input, text).unwrap();
    let run = neottx(&["write-text", "--set", set, "--page", "100", input.to_str().unwrap(), output.to_str().unwrap()]);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let read = neottx(&["text", "--level", level, output.to_str().unwrap(), "100"]);
    assert!(read.status.success(), "{}", String::from_utf8_lossy(&read.stderr));
    String::from_utf8(read.stdout).unwrap().lines().skip(1).map(|line| line.trim_end().to_string()).collect()
}

#[test]
fn vietnamese_reads_back_with_its_tones_at_level_15() {
    let rows = round_trip("vietnamese", "vietnamese", "Tiếng Việt có dấu\nHà Nội và Đà Nẵng", "1.5");
    assert_eq!(rows[..2], ["Tiếng Việt có dấu", "Hà Nội và Đà Nẵng"]);
    // A Level 1 decoder shows the letters without their tones.
    let rows = round_trip("vietnamese-1", "vietnamese", "Tiếng Việt", "1");
    assert_eq!(rows[0], "Tiêng Viêt");
}

#[test]
fn a_cyrillic_page_switches_to_english_with_esc() {
    let rows = round_trip("ukrainian", "ukrainian", "Новини BBC з Києва", "1.5");
    assert_eq!(rows[0], "Новини BBC з Києва", "each ESC shows as the space it replaces");
}

#[test]
fn georgian_and_armenian_read_back() {
    assert_eq!(round_trip("georgian", "georgian", "საქართველო", "1")[0], "საქართველო");
    assert_eq!(round_trip("armenian", "armenian", "Հայաստան", "1")[0], "Հայաստան");
}

#[test]
fn a_missing_character_is_named() {
    let folder = folder("missing");
    let input = folder.join("page.txt");
    std::fs::write(&input, "Київ\nok ☃").unwrap();
    let output = folder.join("P100.tti");
    let run = neottx(&[
        "write-text",
        "--set",
        "ukrainian",
        "--page",
        "100",
        input.to_str().unwrap(),
        output.to_str().unwrap(),
    ]);
    assert!(!run.status.success());
    let message = String::from_utf8_lossy(&run.stderr);
    assert!(message.contains("line 2") && message.contains("U+2603"), "{message}");
    let run =
        neottx(&["write-text", "--set", "klingon", "--page", "100", input.to_str().unwrap(), output.to_str().unwrap()]);
    assert!(String::from_utf8_lossy(&run.stderr).contains("unknown set"));
}

/// Arabic script reads back in visual order, as Table 44's rows do, right
/// aligned, with the plain letters (REQ-CHR-015: text output gives the
/// letter, not its form).
#[test]
fn persian_reads_back_in_visual_order() {
    let rows = round_trip("persian", "persian", "سلام دنیا", "1");
    assert_eq!(rows[0].trim_start(), "ایند م\u{FEFB}س");
}
