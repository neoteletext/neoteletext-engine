//! `neottx write-text` (REQ-CLI-010): a page written from text in one of
//! neoteletext's own sets decodes to the same text, with the designation in
//! X/28/0 and the tones in X/26, and a refusal names the character.
#![cfg(feature = "write-text")]
#![allow(missing_docs)]

mod common;

use common::{folder, neottx};

/// Writes `text` in `set` as page 100 and reads rows 1 onwards back at
/// `level`, trailing spaces removed.
fn round_trip(name: &str, set: &str, text: &str, level: &str) -> Vec<String> {
    written(name, &["--set", set], text, level).0
}

/// Writes `text` with the options `choice` as page 100, and gives rows 1
/// onwards read back at `level`, trailing spaces removed, and the number of
/// X/26 triplets the writer reports.
fn written(name: &str, choice: &[&str], text: &str, level: &str) -> (Vec<String>, usize) {
    let folder = folder(name);
    let input = folder.join("page.txt");
    let output = folder.join("P100.tti");
    std::fs::write(&input, text).unwrap();
    let mut arguments = std::vec!["write-text"];
    arguments.extend_from_slice(choice);
    arguments.extend_from_slice(&["--page", "100", input.to_str().unwrap(), output.to_str().unwrap()]);
    let run = neottx(&arguments);
    let message = String::from_utf8_lossy(&run.stderr).to_string();
    assert!(run.status.success(), "{message}");
    let triplets = message
        .split(", ")
        .nth(1)
        .and_then(|part| part.split(' ').next())
        .and_then(|count| count.parse().ok())
        .unwrap_or_else(|| panic!("no triplet count in {message:?}"));
    let read = neottx(&["text", "--level", level, output.to_str().unwrap(), "100"]);
    assert!(read.status.success(), "{}", String::from_utf8_lossy(&read.stderr));
    let rows =
        String::from_utf8(read.stdout).unwrap().lines().skip(1).map(|line| line.trim_end().to_string()).collect();
    (rows, triplets)
}

/// The writer's message when it refuses `text` written with `choice`.
fn refused(name: &str, choice: &[&str], text: &str) -> String {
    let folder = folder(name);
    let input = folder.join("page.txt");
    let output = folder.join("P100.tti");
    std::fs::write(&input, text).unwrap();
    let mut arguments = std::vec!["write-text"];
    arguments.extend_from_slice(choice);
    arguments.extend_from_slice(&["--page", "100", input.to_str().unwrap(), output.to_str().unwrap()]);
    let run = neottx(&arguments);
    assert!(!run.status.success());
    String::from_utf8_lossy(&run.stderr).to_string()
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

/// REQ-2185: the rōmaji set holds the macron vowels and the yen, so a page
/// in it needs no X/26 at all, and reads back at every level; written for
/// Level 3.5, `ja-Latn` is Latin English with X/26 macrons, which a Level 1
/// decoder shows without them.
#[test]
fn romaji_in_its_own_set_and_with_the_standards_macrons() {
    let text = "Tōkyō e yōkoso! ¥500\nŌsaka, Ūtsu, kin'en";
    let (rows, triplets) = written("romaji", &["--set", "japanese-romaji"], text, "1");
    assert_eq!(rows[..2], ["Tōkyō e yōkoso! ¥500", "Ōsaka, Ūtsu, kin'en"]);
    assert_eq!(triplets, 0, "the long vowels are the set's own codes");
    let (rows, triplets) = written("romaji-language", &["--language", "ja-Latn"], text, "1");
    assert_eq!((rows[0].as_str(), triplets), ("Tōkyō e yōkoso! ¥500", 0));
    let standard = ["--language", "ja-Latn", "--level", "3.5"];
    let (rows, triplets) = written("romaji-standard", &standard, "Tōkyō e yōkoso", "1.5");
    assert_eq!((rows[0].as_str(), triplets), ("Tōkyō e yōkoso", 4), "a row address and three macrons");
    let (rows, _) = written("romaji-standard-1", &standard, "Tōkyō e yōkoso", "1");
    assert_eq!(rows[0], "Tokyo e yokoso");
}

/// REQ-2188: kana. An ESC between the scripts takes the space between
/// words; where there is no space a row set triplet switches with no cell;
/// half-width katakana and voicing marks are joined into the voiced kana;
/// a Latin character neither set has is a run in Latin English.
#[test]
fn kana_switch_scripts_and_reach_latin() {
    let (rows, triplets) = written("kana-space", &["--set", "hiragana"], "きょうは テレビ", "3.5");
    assert_eq!((rows[0].as_str(), triplets), ("きょうは テレビ", 0), "the ESC takes the space");
    let (rows, triplets) = written("kana-joined", &["--set", "hiragana"], "コーヒーの ねだん", "3.5");
    assert_eq!(rows[0], "コーヒーの ねだん");
    assert_eq!(triplets, 3, "a row address and two row set triplets, and no cell lost");
    let (rows, _) = written("kana-half", &["--language", "ja"], "ｺｰﾋｰ テ\u{3099}ﾚﾋﾞ", "3.5");
    assert_eq!(rows[0], "コーヒー デレビ");
    let (rows, triplets) = written("kana-latin", &["--language", "ja"], "こうすいかくりつ 20% です", "3.5");
    assert_eq!((rows[0].as_str(), triplets), ("こうすいかくりつ 20% です", 3));
    let (rows, _) = written("katakana", &["--language", "ja-Kana"], "テレビ と ラジオ", "3.5");
    assert_eq!(rows[0], "テレビ と ラジオ");
    let message = refused("kanji", &["--language", "ja"], "日本");
    assert!(message.contains("U+65E5"), "{message}");
    let message = refused("kana-standard", &["--language", "ja", "--level", "3.5"], "にほん");
    assert!(message.contains("no set writes the language"), "{message}");
}

/// REQ-2180, REQ-2148 item 4: on a page for NeoTeletext's receivers, text
/// in a set the page lacks is a row set run; at Level 3.5 it is refused.
#[test]
fn a_run_in_another_set_on_an_english_page() {
    let (rows, triplets) = written("greek-run", &["--set", "english"], "News: Αθήνα today", "3.5");
    assert_eq!(rows[0], "News: Αθήνα today");
    assert_eq!(triplets, 3, "a row address, a run to Greek and one back");
    let message = refused("greek-standard", &["--set", "english", "--level", "3.5"], "Αθήνα");
    assert!(message.contains("U+0391"), "{message}");
}

/// REQ-2190: the standard's Arabic set of Table 44, each letter in the
/// joined form its neighbours call for, from Table 44 or by X/26 from its
/// G2 set (Table 45), or the nearest form they have (they have no final
/// meem); rows in visual order, right-aligned, with Latin reached by ESC.
#[test]
fn arabic_in_the_standards_table_44() {
    let (rows, triplets) = written("table44", &["--set", "arabic-table44"], "سلام عليكم\nلا 123 abc", "1.5");
    let row = |index: usize| rows[index].trim_start().chars().map(u32::from).collect::<Vec<_>>();
    assert_eq!(triplets, 2, "a row address and the final lam-alef from G2");
    assert_eq!(
        row(0),
        [0x645, 0xFEDB, 0xFEF4, 0xFEE0, 0xFECB, 0x20, 0x645, 0xFEFC, 0xFEB3],
        "meem, kaf initial for medial, yeh and lam medial, ain initial; meem, final lam-alef, seen initial"
    );
    assert_eq!(rows[1].trim_start(), "abc 123 \u{FEFB}");
}
