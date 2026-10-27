#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::string_slice, clippy::panic)]
//! The tests of the `NNN.sub` reader and writer (REQ-WEB-034, decision
//! E.132).

use super::*;
use std::string::String;
use std::vec::Vec;

fn all(text: &str) -> Vec<Result<Entry<'_>, LineFault>> {
    read(text.as_bytes()).unwrap().collect()
}

fn validator(text: &str) -> Validator {
    Validator::parse(text.as_bytes()).unwrap()
}

/// Every sub-code a header can carry, 0000 to 3F7F, in ascending order.
fn every_subcode() -> impl Iterator<Item = SubCode> {
    (0..=limits::SUBCODE_MASK).filter(|&value| limits::is_sub_code(value)).map(SubCode::new)
}

#[test]
fn each_field_of_an_entry() {
    let text = "0001\t3fa94c0e12d7b655\tHeadlines\r\n\n3F7E\tv-2_x\n0000\tA\tFirst page\n";
    let entries = all(text);
    assert_eq!(
        entries,
        [
            Ok(Entry { subcode: SubCode::new(1), validator: validator("3fa94c0e12d7b655"), title: Some("Headlines") }),
            Ok(Entry { subcode: SubCode::new(0x3F7E), validator: validator("v-2_x"), title: None }),
            Ok(Entry { subcode: SubCode::new(0), validator: validator("A"), title: Some("First page") }),
        ]
    );
}

#[test]
fn faults_are_ignored_and_counted() {
    let text = "0001\n0001\tv\tTitle\tMore\n0080\tv\n00a1\tv\n001\tv\n0002\t\n0003\tv v\n0004\t01234567890123456\n0005\tv\t\n0006\tv\tCaf\u{e9}\n0001\tv\n0007\tv\n0007\tw\n";
    let results = all(text);
    let faults: Vec<(usize, Fault)> = results.iter().filter_map(|r| r.err()).map(|f| (f.line, f.fault)).collect();
    assert_eq!(
        faults,
        [
            (1, Fault::Fields),
            (2, Fault::Fields),
            (3, Fault::SubCode),
            (4, Fault::SubCode),
            (5, Fault::SubCode),
            (6, Fault::Validator),
            (7, Fault::Validator),
            (8, Fault::Validator),
            (9, Fault::Title),
            (10, Fault::Character),
            (13, Fault::Duplicate),
        ]
    );
    let kept: Vec<u16> = results.iter().filter_map(|r| r.ok()).map(|e| e.subcode.value()).collect();
    assert_eq!(kept, [0x0001, 0x0007], "the first of two entries for 0001 and 0007 is kept");
}

#[test]
fn long_titles_are_cut() {
    let long = "x".repeat(MAX_TITLE + 8);
    let text = std::format!("0001\tv\t{long}\n");
    assert_eq!(all(&text)[0].unwrap().title, Some(&long[..MAX_TITLE]));
}

#[test]
fn every_sub_code_is_carried_and_the_count_is_bounded() {
    // REQ-NUM-002: every sub-code 0000 to 3F7F is accepted; at most 8,190
    // entries are kept (REQ-NUM-020), the rest counted.
    let mut buffer = std::vec![0u8; MAX_BYTES + MAX_LINE * 2];
    let mut writer = Writer::new(&mut buffer);
    let codes: Vec<SubCode> = every_subcode().collect();
    assert_eq!(codes.len(), 8192);
    for &code in &codes {
        write_entry(&mut writer, code, &Validator::of(&code.value().to_be_bytes()), "A title").unwrap();
    }
    let results: Vec<_> = read(writer.written()).unwrap().collect();
    let kept: Vec<SubCode> = results.iter().filter_map(|r| r.ok()).map(|e| e.subcode).collect();
    assert_eq!(MAX_ENTRIES, 8190);
    assert_eq!(kept, codes[..MAX_ENTRIES]);
    let faults: Vec<Fault> = results.iter().filter_map(|r| r.err()).map(|f| f.fault).collect();
    assert_eq!(faults, [Fault::TooMany, Fault::TooMany]);
}

#[test]
fn a_full_catalogue_fits_and_a_larger_file_is_refused() {
    let mut buffer = std::vec![0u8; MAX_BYTES];
    let mut writer = Writer::new(&mut buffer);
    let title = "T".repeat(MAX_TITLE);
    for code in every_subcode().take(MAX_ENTRIES) {
        write_entry(&mut writer, code, &validator("0123456789abcdef"), &title).unwrap();
    }
    assert!(writer.len() <= MAX_BYTES);
    assert_eq!(read(writer.written()).unwrap().flatten().count(), MAX_ENTRIES);
    assert_eq!(read(&std::vec![b'\n'; MAX_BYTES + 1]).err(), Some(SubPagesError::TooLarge));
}

#[test]
fn the_writer_makes_titles_valid() {
    let mut buffer = [0u8; MAX_LINE];
    let cases = [
        ("  Weather \u{1fb00}\t today  ", "0123\tv\tWeather today\n"),
        ("", "0123\tv\n"),
        ("\u{e9}\u{e9}", "0123\tv\n"),
    ];
    for (title, expected) in cases {
        let mut writer = Writer::new(&mut buffer);
        write_entry(&mut writer, SubCode::new(0x0123), &validator("v"), title).unwrap();
        assert_eq!(core::str::from_utf8(writer.written()).unwrap(), expected, "{title:?}");
    }
    let mut writer = Writer::new(&mut buffer);
    let long = "word ".repeat(MAX_TITLE);
    write_entry(&mut writer, SubCode::new(1), &validator("v"), &long).unwrap();
    let written = core::str::from_utf8(writer.written()).unwrap();
    let title = written.trim_end().rsplit('\t').next().unwrap();
    assert!(title.len() <= MAX_TITLE && !title.ends_with(' '), "{title:?}");
    // An entry that does not fit leaves nothing behind.
    let mut small = [0u8; 8];
    let mut writer = Writer::new(&mut small);
    assert_eq!(write_entry(&mut writer, SubCode::new(1), &validator("v"), "Title"), Err(Overflow));
    assert!(writer.is_empty());
}

#[test]
fn validators() {
    let of = Validator::of(b"a sub-page");
    let hex: String = sha256::to_hex(&sha256::digest(b"a sub-page")).iter().map(|&b| char::from(b)).collect();
    assert_eq!(of.as_str(), &hex[..MAX_VALIDATOR]);
    assert_ne!(Validator::of(b"a sub-page, changed"), of);
    assert!(Validator::parse(b"").is_none());
    assert!(Validator::parse(&[b'a'; MAX_VALIDATOR + 1]).is_none());
    assert!(Validator::parse(b"W/\"1\"").is_none());
    assert_eq!(Validator::parse(b"Az09-_").unwrap().as_str(), "Az09-_");
}

#[test]
fn sub_codes_as_written() {
    assert_eq!(parse_subcode(b"3F7F"), Some(SubCode::ANY));
    assert_eq!(parse_subcode(b"0000"), Some(SubCode::new(0)));
    for refused in [&b"3f7e"[..], b"0080", b"4000", b"008", b"00001", b"+001"] {
        assert_eq!(parse_subcode(refused), None, "{refused:?}");
    }
}
