#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::string_slice, clippy::panic)]
//! The tests of the contents.txt reader (REQ-WEB-025).

use super::*;
use core::fmt::Write as _;
use std::vec::Vec;

fn all(text: &str) -> Vec<Result<Entry<'_>, LineFault>> {
    read(text.as_bytes()).unwrap().collect()
}

#[test]
fn each_kind_of_entry() {
    let text = "# a comment\r\ns\tNews\r\n\np\t101\tHeadlines\t2026-10-12\np\t102\tWorld\nx\tweather:100\tWeather\nx\thttps://radio.example/\tRadio\n";
    let entries = all(text);
    let page = |mpp| PageAddress::from_mpp(mpp).unwrap();
    assert_eq!(entries[0], Ok(Entry::Section { title: "News" }));
    assert_eq!(
        entries[1],
        Ok(Entry::Page { page: page(0x101), title: "Headlines", date: Some(Date { year: 2026, month: 10, day: 12 }) })
    );
    assert_eq!(entries[2], Ok(Entry::Page { page: page(0x102), title: "World", date: None }));
    assert_eq!(entries[3], Ok(Entry::Channel { target: "weather:100", title: "Weather" }));
    assert_eq!(entries[4], Ok(Entry::Channel { target: "https://radio.example/", title: "Radio" }));
    assert_eq!(entries.len(), 5);
}

#[test]
fn faults_are_ignored_and_counted() {
    let text = "q\tlater\np\t1A0\tHex\np\t100\nx\tnowhere\tNo\np\t100\tOne\np\t100\tAgain\np\t101\t\ns\tCaf\u{e9}\np\t102\tDay\t2026-13-01\nx\thttps://h.example/a/../b/\tTricks\n";
    let faults: Vec<Fault> = all(text).into_iter().filter_map(Result::err).map(|f| f.fault).collect();
    assert_eq!(
        faults,
        [
            Fault::UnknownKind,
            Fault::Page,
            Fault::Fields,
            Fault::Target,
            Fault::Duplicate,
            Fault::Title,
            Fault::Character,
            Fault::Date,
            Fault::Target
        ]
    );
}

#[test]
fn long_titles_are_cut() {
    let long = "x".repeat(MAX_TITLE + 8);
    let text = std::format!("s\t{long}\n");
    assert_eq!(all(&text)[0], Ok(Entry::Section { title: &long[..MAX_TITLE] }));
}

#[test]
fn limits() {
    assert_eq!(read(&std::vec![b' '; MAX_BYTES + 1]).err(), Some(ContentsError::TooLarge));
    let many = "#\n".repeat(MAX_LINES + 1);
    assert_eq!(read(many.as_bytes()).err(), Some(ContentsError::TooManyLines));
    let mut text = std::string::String::new();
    for magazine in 1..=8u16 {
        for number in 0..=99u16 {
            let _ = writeln!(text, "p\t{magazine}{number:02}\tPage");
        }
    }
    text.push_str("p\t899\tToo many\n");
    let entries = all(&text);
    assert_eq!(entries.iter().filter(|entry| entry.is_ok()).count(), MAX_PAGES);
    assert_eq!(entries.last().unwrap().unwrap_err().fault, Fault::Duplicate, "899 was already listed");
}
