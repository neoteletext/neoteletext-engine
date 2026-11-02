#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::string_slice, clippy::panic)]
//! The tests of the reader and writer of a part of a page's list,
//! `NNN/P.sub` (REQ-1888, DOC-2038).

use super::*;
use std::string::String;
use std::vec::Vec;

/// A header any test's part can start with.
const HEAD: &str = "3\tcafe0123\n";

fn part(text: &str) -> ListPart<'_> {
    read(text.as_bytes()).unwrap()
}

fn all(text: &str) -> Vec<Result<Entry<'_>, LineFault>> {
    part(text).entries.collect()
}

fn validator(text: &str) -> Validator {
    Validator::parse(text.as_bytes()).unwrap()
}

/// Every sub-code that names a sub-page, 0000 to 3F7E, in ascending order.
fn every_subcode() -> impl Iterator<Item = SubCode> {
    (0..limits::SUBCODE_ANY).filter(|&value| limits::is_sub_code(value)).map(SubCode::new)
}

/// REQ-1888: the header gives the page's number of sub-pages and its version.
#[test]
fn the_header_gives_the_total_and_the_version() {
    let read = part("8190\t3fa94c0e12d7b655\r\n");
    assert_eq!((read.total, read.version), (8190, validator("3fa94c0e12d7b655")));
    assert_eq!(read.entries.count(), 0, "a part may hold no row but its header");
}

/// REQ-1888: a header that is missing or invalid makes the part invalid.
#[test]
fn a_part_without_a_valid_header_is_refused() {
    let refused = [
        "",
        "\n",
        "0001\t3fa94c0e12d7b655\tHeadlines\n",
        "0\tv\n",
        "8191\tv\n",
        "03\tv\n",
        "+3\tv\n",
        "3\n",
        "3\t\n",
        "3\tv\textra\n",
        "3\t01234567890123456\n",
        "3\tv v\n",
    ];
    for text in refused {
        assert_eq!(read(text.as_bytes()).err(), Some(SubPagesError::Header), "{text:?}");
    }
}

/// REQ-1888: a row is a sub-code, a validator and optionally a title.
#[test]
fn each_field_of_a_row() {
    let text = std::format!("{HEAD}0000\tA\tFirst page\r\n0001\t3fa94c0e12d7b655\tHeadlines\n3F7E\tv-2_x\n");
    assert_eq!(
        all(&text),
        [
            Ok(Entry { subcode: SubCode::new(0), validator: validator("A"), title: Some("First page") }),
            Ok(Entry { subcode: SubCode::new(1), validator: validator("3fa94c0e12d7b655"), title: Some("Headlines") }),
            Ok(Entry { subcode: SubCode::new(0x3F7E), validator: validator("v-2_x"), title: None }),
        ]
    );
}

/// REQ-1888: every fault of a row is ignored and counted, and the rows around
/// it are kept. A sub-code not above the row before it is one of them, so a
/// list another writer made out of order is not followed (REQ-1890).
#[test]
fn faults_are_ignored_and_counted() {
    let rows = "0001\n\n0001\tv\tTitle\tMore\n0080\tv\n00a1\tv\n001\tv\n3F7F\tv\n0002\t\n0003\tv v\n\
                0004\t01234567890123456\n0005\tv\t\n0006\tv\tCaf\u{e9}\n0007\tv\n0007\tw\n0001\tv\n0008\tv\n";
    let text = std::format!("{HEAD}{rows}");
    let results = all(&text);
    let faults: Vec<(usize, Fault)> = results.iter().filter_map(|r| r.err()).map(|f| (f.line, f.fault)).collect();
    assert_eq!(
        faults,
        [
            (2, Fault::Fields),
            (3, Fault::Fields),
            (4, Fault::Fields),
            (5, Fault::SubCode),
            (6, Fault::SubCode),
            (7, Fault::SubCode),
            (8, Fault::SubCode),
            (9, Fault::Validator),
            (10, Fault::Validator),
            (11, Fault::Validator),
            (12, Fault::Title),
            (13, Fault::Character),
            (15, Fault::NotAscending),
            (16, Fault::NotAscending),
        ]
    );
    let kept: Vec<u16> = results.iter().filter_map(|r| r.ok()).map(|e| e.subcode.value()).collect();
    assert_eq!(kept, [0x0007, 0x0008]);
}

#[test]
fn long_titles_are_cut() {
    let long = "x".repeat(MAX_TITLE + 8);
    let text = std::format!("{HEAD}0001\tv\t{long}\n");
    assert_eq!(all(&text)[0].unwrap().title, Some(&long[..MAX_TITLE]));
}

/// REQ-1888: a 65th row is ignored and counted.
#[test]
fn a_part_holds_its_rows_and_no_more() {
    let mut buffer = std::vec![0u8; MAX_BYTES + MAX_LINE];
    let mut writer = Writer::new(&mut buffer);
    write_header(&mut writer, 8190, &validator("v")).unwrap();
    for code in every_subcode().take(ROWS_PER_PART + 1) {
        write_entry(&mut writer, code, &validator("v"), "").unwrap();
    }
    let results: Vec<_> = read(writer.written()).unwrap().entries.collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), ROWS_PER_PART);
    assert_eq!(results.last().unwrap().unwrap_err(), LineFault { line: ROWS_PER_PART + 2, fault: Fault::TooMany });
}

/// REQ-1904: a part at the bound is read, and one byte past it is refused
/// whole. REQ-1888: the bound is what DOC-2038 derives.
#[test]
fn a_full_part_fits_and_a_larger_file_is_refused() {
    let mut buffer = std::vec![0u8; MAX_BYTES];
    let mut writer = Writer::new(&mut buffer);
    let title = "T".repeat(MAX_TITLE);
    let longest = validator("0123456789abcdef");
    write_header(&mut writer, 8190, &longest).unwrap();
    for code in every_subcode().take(ROWS_PER_PART) {
        write_entry(&mut writer, code, &longest, &title).unwrap();
    }
    // The writer ends a line with a line feed alone, one byte under the bound
    // on each line, and the reader takes a line ended by CR LF as well.
    let lines = ROWS_PER_PART + 1;
    assert_eq!(writer.len(), MAX_BYTES - lines);
    let spelled: Vec<u8> = core::str::from_utf8(writer.written()).unwrap().replace('\n', "\r\n").into_bytes();
    assert_eq!(spelled.len(), MAX_BYTES);
    assert_eq!(read(&spelled).unwrap().entries.flatten().count(), ROWS_PER_PART);
    assert_eq!(read(&std::vec![b'\n'; MAX_BYTES + 1]).err(), Some(SubPagesError::TooLarge));
}

/// REQ-1888: part `P` holds the rows from place `64 × P`; a reader finds the
/// part of a place by dividing, and rows of one part ask for it once.
#[test]
fn a_place_is_in_one_part() {
    assert_eq!(MAX_PARTS, 128);
    assert_eq!(PartNumber::of_place(0), Some(PartNumber::FIRST));
    assert_eq!(PartNumber::of_place(63), Some(PartNumber::FIRST));
    assert_eq!(PartNumber::of_place(64).map(PartNumber::number), Some(1));
    assert_eq!(PartNumber::of_place(MAX_ENTRIES - 1).map(PartNumber::number), Some(127));
    assert_eq!(PartNumber::new(127).unwrap().first_place(), 8128);
    assert_eq!(PartNumber::new(128), None);
    let numbers = |first, count| parts_of_rows(first, count).map(PartNumber::number).collect::<Vec<u8>>();
    assert_eq!(numbers(4020, 23), [62, 63]);
    assert_eq!(numbers(4000, 23), [62]);
    assert_eq!(numbers(0, 0), [0u8; 0]);
    assert_eq!(numbers(8180, 64), [127], "rows past the last name no part");
}

/// REQ-1884: a part's number has one spelling in a file's name.
#[test]
fn a_part_number_has_one_spelling() {
    assert_eq!(PartNumber::parse(b"0"), Some(PartNumber::FIRST));
    assert_eq!(PartNumber::parse(b"127"), PartNumber::new(127));
    for refused in [&b""[..], b"00", b"07", b"128", b"-1", b"1a", b"00127"] {
        assert_eq!(PartNumber::parse(refused), None, "{refused:?}");
    }
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
    // A row that does not fit leaves nothing behind.
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
    assert_eq!(parse_subcode(b"3F7E"), Some(SubCode::new(0x3F7E)));
    assert_eq!(parse_subcode(b"0000"), Some(SubCode::new(0)));
    // 3F7F names no particular sub-page (DOC-2038).
    for refused in [&b"3F7F"[..], b"3f7e", b"0080", b"4000", b"008", b"00001", b"+001"] {
        assert_eq!(parse_subcode(refused), None, "{refused:?}");
    }
}

/// REQ-1889: a page's version is the validator of its sub-pages end to end,
/// and changes when a sub-page changes, is added or is removed.
#[test]
fn a_pages_version_follows_its_sub_pages() {
    let (first, second, third) = (&b"first"[..], &b"second"[..], &b"third"[..]);
    let version = Validator::of_page([first, second]);
    assert_eq!(version, Validator::of(b"firstsecond"));
    assert_ne!(version, Validator::of_page([first, &b"Second"[..]]), "a sub-page changed");
    assert_ne!(version, Validator::of_page([first, second, third]), "a sub-page added");
    assert_ne!(version, Validator::of_page([first]), "a sub-page removed");
    assert_ne!(version, Validator::of_page([second, first]), "in the page's order");
}
