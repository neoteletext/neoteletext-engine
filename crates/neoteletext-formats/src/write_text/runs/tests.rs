#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "a test"
)]

//! Text runs at their places (REQ-2214, REQ-2148), read back through the
//! engine's own decoder to the Unicode they were written from.

extern crate std;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use neoteletext_core::acq::{Decoder, StoredPage};
use neoteletext_core::charset::{compose, designation, to_unicode};
use neoteletext_core::compose::compose_level;
use neoteletext_core::config::Config;
use neoteletext_core::consts::{
    COLUMNS, ESC, LOCAL_TRIPLETS, MODE_G0_LEVEL_25, MODE_G2, MODE_MODIFIED_DESIGNATION, MODE_ROW_SET, ROWS, SPACE,
    STATUS_COLUMNS,
};
use neoteletext_core::level1::{DisplayOptions, Glyph, Grid, build_grid};
use neoteletext_core::packet::{Packet, PacketMeta, Triplet, TripletFunction};
use neoteletext_core::types::{Designations, Instant, PageAddress, SubCode};

use super::*;
use crate::encode::encode_page;
use crate::page::{EnhancementPacket, PageFile};

const ENGLISH: u8 = 0;
const PAGE: u16 = 0x100;

fn run<'a>(row: u8, column: u8, text: &'a str, language: Option<&'a str>) -> TextRun<'a> {
    TextRun { row, column, text, language }
}

fn blank() -> [[u8; COLUMNS]; ROWS] {
    [[SPACE; COLUMNS]; ROWS]
}

/// Writes `runs` on an English page at `level` over a blank grid.
fn written(level: Level, runs: &[TextRun<'_>]) -> ([[u8; COLUMNS]; ROWS], Encoded) {
    let mut grid = blank();
    let page = PageText { language: Some("en"), designation: ENGLISH, level, runs };
    let encoded = write_runs(&mut grid, &page, &[]).unwrap();
    (grid, encoded)
}

fn refused(level: Level, runs: &[TextRun<'_>], program: &[Triplet], grid: &mut [[u8; COLUMNS]; ROWS]) -> RunError {
    let page = PageText { language: Some("en"), designation: ENGLISH, level, runs };
    write_runs(grid, &page, program).unwrap_err()
}

/// The page decoded by the engine, row by row, trailing spaces removed:
/// `row_set` false decodes as a strict Level 3.5 decoder does.
fn read(grid: &[[u8; COLUMNS]; ROWS], encoded: &Encoded, row_set: bool) -> Vec<String> {
    let address = PageAddress::from_mpp(PAGE).unwrap();
    let mut page = Box::new(PageFile::new());
    page.address = address;
    page.subcode = SubCode::new(0);
    for (row, codes) in grid.iter().enumerate().skip(1) {
        page.set_row(row, codes);
    }
    page.set_national_index(encoded.designations.default & 0b111);
    page.region = Some(encoded.designations.default >> 3);
    for (designation, triplets) in (0u8..).zip(packets(&encoded.triplets)) {
        assert!(page.enhancements.push(EnhancementPacket { designation, triplets }));
    }
    let mut bytes = Vec::new();
    encode_page(&page, &mut |packet| bytes.push(packet.bytes));
    let config = Config { row_set_triplets: row_set, ..Config::default() };
    let mut slots = vec![StoredPage::EMPTY; 1].into_boxed_slice();
    let mut decoder = Decoder::new(&mut slots, config);
    for &bytes in &bytes {
        decoder.push(&Packet { bytes }, &PacketMeta::default(), Instant(0), &mut |_| {});
    }
    let stored = decoder.store().pages().iter().find(|stored| stored.key.address == address).unwrap();
    let mut grid_out = Box::new(Grid::EMPTY);
    let options = DisplayOptions { status: [b' '; STATUS_COLUMNS], ..DisplayOptions::default() };
    let magazine = decoder.magazine_data(address.magazine);
    build_grid(&mut grid_out, Some(stored), &magazine, decoder.config(), &options);
    let _ = compose_level(&mut grid_out, stored, &magazine, &decoder.enhancement_lookup(), decoder.config(), &options);
    grid_out
        .cells
        .iter()
        .map(|cells| {
            let mut line = String::new();
            for cell in cells {
                let Glyph::Char(character) = cell.glyph else {
                    line.push(' ');
                    continue;
                };
                let unicode = to_unicode(character);
                match unicode.combining {
                    Some(mark) => match compose(unicode.base, mark) {
                        Some(one) => line.push(one),
                        None => line.extend([unicode.base, mark]),
                    },
                    None => line.push(unicode.base),
                }
            }
            String::from(line.trim_end())
        })
        .collect()
}

fn value(tag: &str) -> u8 {
    language_value(tag, true).unwrap()
}

fn row_set(column: u8, data: u8) -> Triplet {
    TripletFunction::ReservedColumn { column, mode: MODE_ROW_SET, data }.to_triplet()
}

fn position(row: u8) -> Triplet {
    TripletFunction::SetActivePosition { address: 40 + row, column: 0 }.to_triplet()
}

fn end() -> Triplet {
    TripletFunction::Termination { kind: TERMINATION_LOCAL_DATA_END }.to_triplet()
}

#[test]
fn russian_and_greek_runs_on_an_english_page_switch_by_row_set_triplets() {
    let runs = [run(3, 0, "Привет мир", Some("ru")), run(3, 12, "Γειά σου", Some("el"))];
    let (grid, encoded) = written(Level::NeoTeletext, &runs);
    // One row address, a row set triplet at each run's first cell and no
    // switch back, since nothing follows the Greek run; nothing else.
    assert_eq!(encoded.triplets, [position(3), row_set(0, value("ru")), row_set(12, value("el")), end()]);
    assert_eq!(encoded.count, 4);
    assert!(encoded.changed);
    assert_eq!(encoded.runs[0], Report { cells: 10, triplets: 3, mark_rows: 0 });
    assert_eq!(encoded.runs[1], Report { cells: 8, triplets: 1, mark_rows: 0 });
    assert_eq!(encoded.designations, Designations { default: ENGLISH, second: NO_SECOND_G0 });
    assert_eq!(read(&grid, &encoded, true)[3], "Привет мир  Γειά σου");
}

#[test]
fn a_run_followed_by_english_switches_back_at_the_cell_after_it() {
    let mut grid = blank();
    grid[4][..30].copy_from_slice(b"                    then home!");
    let runs = [run(4, 2, "Москва", Some("ru"))];
    let page = PageText { language: None, designation: ENGLISH, level: Level::NeoTeletext, runs: &runs };
    let encoded = write_runs(&mut grid, &page, &[]).unwrap();
    assert_eq!(encoded.triplets, [position(4), row_set(2, value("ru")), row_set(8, ENGLISH), end()]);
    assert_eq!(read(&grid, &encoded, true)[4], "  Москва            then home!");
}

#[test]
fn at_level_3_5_another_set_is_a_designation_and_a_triplet_a_character() {
    let runs = [run(2, 5, "Αθήνα", Some("el"))];
    let (grid, encoded) = written(Level::Standard, &runs);
    let greek = value("el");
    assert_eq!(encoded.triplets[0], position(2));
    assert_eq!(
        encoded.triplets[1],
        TripletFunction::ModifiedDesignation { column: 5, designation: greek }.to_triplet()
    );
    assert_eq!(encoded.triplets[1].mode, MODE_MODIFIED_DESIGNATION);
    // Five letters, each by its own triplet, over spaces in the row.
    assert_eq!(encoded.count, 1 + 1 + 5 + 1);
    assert!(encoded.triplets[2..7].iter().all(|triplet| triplet.mode == MODE_G0_LEVEL_25 || triplet.mode > MODE_G2));
    assert!(grid[2][5..10].iter().all(|&code| code == SPACE));
    // A strict decoder, which ignores the row set triplet, reads it.
    assert_eq!(read(&grid, &encoded, false)[2], "     Αθήνα");
}

#[test]
fn a_marked_letter_of_the_page_set_costs_one_triplet() {
    let runs = [run(1, 0, "café", None)];
    let (grid, encoded) = written(Level::NeoTeletext, &runs);
    assert_eq!(encoded.count, 3, "a row address, the mark and the end");
    assert_eq!(read(&grid, &encoded, true)[1], "café");
}

#[test]
fn an_arabic_run_is_in_visual_order() {
    let text = "مرحبا بالعالم";
    let runs = [run(6, 10, text, Some("ar"))];
    let (grid, encoded) = written(Level::NeoTeletext, &runs);
    let visual: String = text.chars().rev().collect();
    assert_eq!(read(&grid, &encoded, true)[6].trim_start(), visual);
}

#[test]
fn a_thai_run_writes_its_marks_on_the_rows_above_and_below() {
    // สวัสดี: ั over ว and ี over ด; ครู: ู under ร.
    let runs = [run(5, 0, "สวัสดี ครู", Some("th"))];
    let (grid, encoded) = written(Level::NeoTeletext, &runs);
    assert_eq!(encoded.runs[0].mark_rows, 1);
    let rows = read(&grid, &encoded, true);
    assert_eq!(rows[5], "สวสด คร");
    let above: Vec<char> = rows[4].chars().collect();
    assert_eq!((above[1], above[3]), ('\u{0E31}', '\u{0E35}'));
    let below: Vec<char> = rows[6].chars().collect();
    assert_eq!(below[6], '\u{0E39}');
    // Each of the three rows starts in the Thai set.
    let thai = value("th");
    for row in 4..=6 {
        assert!(encoded.triplets.contains(&row_set(0, thai)), "row {row}");
    }
}

#[test]
fn thai_runs_share_a_mark_row_where_no_column_needs_two_marks() {
    let runs = [run(3, 0, "ครู", Some("th")), run(5, 4, "สวัสดี", Some("th"))];
    let (grid, encoded) = written(Level::NeoTeletext, &runs);
    let rows = read(&grid, &encoded, true);
    assert_eq!(rows[4].chars().nth(1), Some('\u{0E39}'));
    assert_eq!(rows[4].chars().nth(5), Some('\u{0E31}'));
}

#[test]
fn the_same_page_always_encodes_the_same() {
    let runs = [run(3, 0, "Привет", Some("ru")), run(8, 3, "Αθήνα", Some("el")), run(10, 0, "Tiếng Việt", Some("vi"))];
    assert_eq!(written(Level::NeoTeletext, &runs), written(Level::NeoTeletext, &runs));
    assert_eq!(written(Level::Standard, &runs), written(Level::Standard, &runs));
}

#[test]
fn the_programs_triplets_are_merged_in_screen_order() {
    let control = TripletFunction::ReservedColumn { column: 30, mode: 0b00101, data: 1 }.to_triplet();
    let early = TripletFunction::ReservedColumn { column: 2, mode: 0b00101, data: 1 }.to_triplet();
    let program = [position(2), early, position(3), control, end()];
    let mut grid = blank();
    let runs = [run(3, 4, "Мир", Some("ru"))];
    let page = PageText { language: None, designation: ENGLISH, level: Level::NeoTeletext, runs: &runs };
    let encoded = write_runs(&mut grid, &page, &program).unwrap();
    assert_eq!(encoded.triplets, [position(2), early, position(3), row_set(4, value("ru")), control, end()]);
    assert_eq!(encoded.runs[0].triplets, 1);
}

#[test]
fn runs_in_the_page_set_leave_the_programs_triplets_as_sent() {
    let program = [position(2), TripletFunction::G2Character { column: 1, code: 0x41 }.to_triplet()];
    let mut grid = blank();
    let runs = [run(5, 0, "plain", None)];
    let page = PageText { language: None, designation: ENGLISH, level: Level::NeoTeletext, runs: &runs };
    let encoded = write_runs(&mut grid, &page, &program).unwrap();
    assert!(!encoded.changed);
    assert_eq!(encoded.triplets, program);
    assert_eq!(encoded.count, 2);
}

#[test]
fn faults_name_their_run() {
    let fault = |runs: &[TextRun<'_>]| {
        let mut grid = blank();
        let error = refused(Level::NeoTeletext, runs, &[], &mut grid);
        (error.run, error.fault)
    };
    assert_eq!(fault(&[run(1, 0, "one", None), run(1, 2, "two", None)]), (Some(1), RunFault::Overlap(0)));
    assert_eq!(fault(&[run(1, 35, "too long", None)]), (Some(0), RunFault::OutOfColumns(42)));
    assert_eq!(fault(&[run(1, 0, "สวัสดี", Some("th"))]), (Some(0), RunFault::MarkRowsOutOfRange));
    assert_eq!(fault(&[run(2, 0, "中", None)]), (Some(0), RunFault::Unshown('中')));
    assert_eq!(fault(&[run(2, 0, "x", Some("tlh"))]), (Some(0), RunFault::UnknownLanguage("tlh".into())));
    // A Thai run's marks take the row of a run above it.
    assert_eq!(fault(&[run(2, 0, "abc", None), run(3, 0, "สวัสดี", Some("th"))]), (Some(1), RunFault::Overlap(0)));
    let page = PageText { language: Some("tlh"), designation: ENGLISH, level: Level::NeoTeletext, runs: &[] };
    assert_eq!(write_runs(&mut blank(), &page, &[]).unwrap_err().run, None);
}

#[test]
fn the_programs_cells_and_sets_are_kept() {
    let mut grid = blank();
    let program = [position(4), TripletFunction::G2Character { column: 6, code: 0x41 }.to_triplet(), end()];
    let error = refused(Level::NeoTeletext, &[run(4, 5, "Мир", Some("ru"))], &program, &mut grid);
    assert_eq!(error.fault, RunFault::CellTaken { row: 4, column: 6 });
    let mut grid = blank();
    grid[4][1] = ESC;
    let error = refused(Level::NeoTeletext, &[run(4, 5, "Мир", Some("ru"))], &[], &mut grid);
    assert_eq!(error.fault, RunFault::SetChanged { row: 4, column: 1 });
    let mut grid = blank();
    let program = [position(4), row_set(3, value("el")), end()];
    let error = refused(Level::NeoTeletext, &[run(4, 5, "Мир", Some("ru"))], &program, &mut grid);
    assert_eq!(error.fault, RunFault::SetChanged { row: 4, column: 3 });
    // Local objects follow the program's data: their places would move.
    let mut grid = blank();
    let objects = [
        position(4),
        TripletFunction::Termination { kind: crate::consts::TERMINATION_LOCAL_DATA_OBJECTS_FOLLOW }.to_triplet(),
    ];
    let error = refused(Level::NeoTeletext, &[run(4, 5, "Мир", Some("ru"))], &objects, &mut grid);
    assert_eq!(error.fault, RunFault::ProgramObjects);
}

#[test]
fn a_page_over_the_budget_names_the_first_run_that_did_not_fit() {
    let line = "αβγδεζηθικλμνξοπρστυφχψωαβγδεζηθικλμνξοπ";
    assert_eq!(line.chars().count(), COLUMNS);
    let runs: Vec<TextRun<'_>> = (1..=6).map(|row| run(row, 0, line, Some("el"))).collect();
    let mut grid = blank();
    let error = refused(Level::Standard, &runs, &[], &mut grid);
    // Each row costs its address, a designation and forty characters: four
    // rows and the end are 169, a fifth 211.
    assert_eq!(error, RunError { run: Some(4), fault: RunFault::OverBudget(211) });
    assert!(211 > usize::from(LOCAL_TRIPLETS));
    // At the NeoTeletext level the same page costs two triplets a row.
    let (_, encoded) = written(Level::NeoTeletext, &runs);
    assert_eq!(encoded.count, 6 * 2 + 1);
}

#[test]
fn the_designation_is_written_into_x28_keeping_its_other_fields() {
    let mut triplets = [Triplet::from_value(0x3_FFFF); 13];
    designate(&mut triplets, Designations { default: 0b011_0111, second: 0b101_0010 });
    let fields = neoteletext_core::packet::presentation_fields(&triplets.map(|triplet| Some(triplet.value())));
    assert_eq!(fields.designations, Some(Designations { default: 0b011_0111, second: 0b101_0010 }));
    // Bits 1 to 7 of triplet 1, and triplet 2 from bit 4, are untouched.
    assert_eq!(triplets[0].value() & 0x7F, 0x7F);
    assert_eq!(triplets[1].value() >> 3, 0x3_FFFF >> 3);
    assert!(triplets[2..].iter().all(|triplet| triplet.value() == 0x3_FFFF));
}

#[test]
fn every_language_is_listed_with_its_direction_and_mark_rows() {
    let listed = languages();
    assert_eq!(listed.len(), LANGUAGES.len());
    let find = |tag: &str| listed.iter().find(|language| language.tag == tag).unwrap();
    assert_eq!(find("ar").direction, Direction::Rtl);
    assert_eq!(find("he").direction, Direction::Rtl);
    assert_eq!(find("ru").direction, Direction::Ltr);
    assert_eq!((find("th").mark_rows, find("lo").mark_rows, find("en").mark_rows), (1, 1, 0));
    assert_eq!(find("sr-Latn").name, "Serbian (Latin script)");
    // Every tag names exactly its own entry.
    for language in &listed {
        let entry = neoteletext_core::charset::language::language(&language.tag).unwrap();
        assert_eq!(entry.name, language.name);
        assert!(designation(entry.value).is_some());
    }
    let mut tags: Vec<&str> = listed.iter().map(|language| language.tag.as_str()).collect();
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(tags.len(), listed.len(), "tags are unique");
}
