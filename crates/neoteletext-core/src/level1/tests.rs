//! Tests of the cell and grid model, the designation and page assembly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see `level1.rs`); the attribute
// also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::charset;

use crate::acq::{MagazineData, StoredPage};
use crate::charset::{G0, second_g0};
use crate::config::Config;
use crate::consts::{COLUMNS, DESIGNATION_GROUP_SHIFT, DOUBLE_HEIGHT, ESC, NORMAL_SIZE, PAGE_FUNCTION_BASIC, ROWS};

/// REQ-1644: the grid holds 40 columns by 25 rows and no other cells: what
/// it holds besides them is smaller than one row of cells.
#[test]
fn the_grid_has_no_panel_storage() {
    use core::mem::size_of;
    let others = size_of::<Grid>() - size_of::<[[Cell; COLUMNS]; ROWS]>();
    assert!(others < size_of::<[Cell; COLUMNS]>(), "{others} bytes beside the cells");
}

/// REQ-L25-006: a grid is cleared in place to the empty grid.
#[test]
fn clearing_a_grid_empties_it() {
    let mut grid = Grid::empty();
    let attributes = CellAttributes::DEFAULT.with_invert(true);
    grid.cells[3][4].attributes = attributes.with_underline(true);
    grid.row_colours[5] = 17;
    grid.clear();
    assert_eq!(grid, Grid::empty());
}

/// REQ-L25-006: every packed attribute reads back as set, without
/// disturbing the others.
#[test]
fn cell_attributes_pack_independently() {
    let sources =
        [CharSource::Level1, CharSource::Local, CharSource::Active, CharSource::Adaptive, CharSource::Passive];
    for width in [Width::Normal, Width::LeftHalf, Width::RightHalf] {
        for mode in [FlashMode::Normal, FlashMode::InvertPhase, FlashMode::AdjacentClut] {
            for rate in [
                FlashRate::Level1,
                FlashRate::Slow,
                FlashRate::FastPhase1,
                FlashRate::FastPhase2,
                FlashRate::FastPhase3,
            ] {
                for source in &sources {
                    for flags in 0..4 {
                        let attributes = CellAttributes::DEFAULT
                            .with_width(width)
                            .with_flash_mode(mode)
                            .with_flash_rate(rate)
                            .with_underline(flags & 1 != 0)
                            .with_invert(flags & 2 != 0)
                            .with_source(*source);
                        assert_eq!(attributes.width(), width);
                        assert_eq!((attributes.flash_mode(), attributes.flash_rate()), (mode, rate));
                        assert_eq!((attributes.underline(), attributes.invert()), (flags & 1 != 0, flags & 2 != 0));
                        assert_eq!(attributes.source(), *source);
                    }
                }
            }
        }
    }
    assert_eq!(Cell::BLANK.attributes.source(), CharSource::Level1);
}

/// REQ-L25-012, decisions E.4, E.44 and E.50: the designation order of
/// Annex C.4 rule 1 at each level, and the national option from the
/// designation at Levels 2.5 and 3.5 only.
#[test]
fn designation_follows_annex_c4_and_decision_e50() {
    use crate::config::PresentationLevel as Level;
    use crate::packet::{PageEnhancement, PageFormat};
    use crate::types::{ControlBits, Designations, PresentationFields};
    let fields = |default: u8| PresentationFields {
        designations: Some(Designations { default, second: 0x7F }),
        ..PresentationFields::default()
    };
    let basic = Some(PageFormat { function: PAGE_FUNCTION_BASIC, coding: 0 });
    // Group 1 (Central European): index 0 Polish, index 1 German.
    let polish = 0b0001 << DESIGNATION_GROUP_SHIFT;
    let mut page = StoredPage::EMPTY;
    // C14 set: header national index 1.
    page.control = ControlBits(1 << 14);
    let magazine =
        MagazineData { data: Some(fields(0)), level35: Some(fields(0b0110 << DESIGNATION_GROUP_SHIFT | 0b111)) };
    let option = |page: &StoredPage, level| {
        let config = Config { level, ..Config::default() };
        resolve_designation(Some(page), &magazine, &config).0.g0
    };
    let latin = |national| G0 { set: charset::G0Set::Latin, national: Some(national) };
    // X/28/0 first; the header's option at 1.5, the designation's at 2.5.
    page.page_data = Some(PageEnhancement { format: basic, fields: fields(polish) });
    page.page_data_level35 = Some(PageEnhancement { format: basic, fields: fields(0b0100 << DESIGNATION_GROUP_SHIFT) });
    assert_eq!(option(&page, Level::Level15), latin(charset::NationalOption::German));
    assert_eq!(option(&page, Level::Level25), latin(charset::NationalOption::Polish));
    assert_eq!(option(&page, Level::Level35), latin(charset::NationalOption::Polish));
    // Without X/28/0: X/28/4 at 3.5 only (Cyrillic 1), M/29/0 otherwise.
    page.page_data = None;
    assert_eq!(option(&page, Level::Level35).set, charset::G0Set::Cyrillic1);
    assert_eq!(option(&page, Level::Level25), latin(charset::NationalOption::English));
    // Then M/29/0, then M/29/4 (Greek) at 3.5.
    page.page_data_level35 = None;
    assert_eq!(option(&page, Level::Level35), latin(charset::NationalOption::English));
    let only_35 = MagazineData { data: None, ..magazine };
    let config = Config { level: Level::Level35, ..Config::default() };
    assert_eq!(resolve_designation(Some(&page), &only_35, &config).0.g0.set, charset::G0Set::Greek);
    let config = Config { level: Level::Level25, ..Config::default() };
    let header_only = resolve_designation(Some(&page), &only_35, &config).0.g0;
    assert_eq!(header_only, latin(charset::NationalOption::German), "no designation: C12 to C14");
}

/// REQ-CHR-003: the designation report names every step and its
/// source: the receiver's region for a Level 1 page, the page's X/28/0,
/// the magazine's M/29/0, a reserved entry's fallback, the second G0 set
/// and the rows with ESC.
#[test]
fn the_designation_report() {
    use crate::config::PresentationLevel as Level;
    use crate::packet::{PageEnhancement, PageFormat};
    use crate::types::{ControlBits, Designations, PresentationFields};
    let fields = |default: u8, second: u8| PresentationFields {
        designations: Some(Designations { default, second }),
        ..PresentationFields::default()
    };
    let config = Config { level: Level::Level15, ..Config::default() };
    let mut page = StoredPage::EMPTY;
    page.control = ControlBits(1 << 14);
    page.cells[3][5] = crate::ecc::with_odd_parity(ESC);
    page.cells[5][0] = crate::ecc::with_odd_parity(ESC);
    page.cells[0][9] = crate::ecc::with_odd_parity(ESC);
    // A Level 1 page: only C12 to C14, the group from the receiver.
    let report = designation_report(Some(&page), &MagazineData::default(), &config);
    assert_eq!((report.source, report.group, report.header_national), (DesignationSource::Receiver, 0, 1));
    assert_eq!((report.national, report.national_source), (1, NationalSource::Header));
    assert_eq!(report.designation.g0.national, Some(charset::NationalOption::German));
    assert_eq!((report.fallback, report.second, report.esc_rows), (false, None, 2), "row 0 does not count");
    // The page's X/28/0 Format 1 gives group 0001 and a second G0 set.
    let second = 0b0110 << DESIGNATION_GROUP_SHIFT | 0b111;
    let basic = Some(PageFormat { function: PAGE_FUNCTION_BASIC, coding: 0 });
    page.page_data = Some(PageEnhancement { format: basic, fields: fields(0b0001 << DESIGNATION_GROUP_SHIFT, second) });
    let report = designation_report(Some(&page), &MagazineData::default(), &config);
    assert_eq!(
        (report.source, report.group, report.value),
        (DesignationSource::PageX28Format1, 1, 0b0001 << DESIGNATION_GROUP_SHIFT | 1)
    );
    assert_eq!(report.second, second_g0(second));
    assert!(report.second.is_some());
    // Without it, the magazine's M/29/0.
    page.page_data = None;
    let magazine =
        MagazineData { data: Some(fields(0b0100 << DESIGNATION_GROUP_SHIFT, 0x7F)), ..MagazineData::default() };
    let report = designation_report(Some(&page), &magazine, &config);
    assert_eq!((report.source, report.group), (DesignationSource::MagazineM29Format0, 0b0100));
    // A reserved entry falls back to group 0000 with the same option.
    let reserved = MagazineData { data: Some(fields(0b1101 << DESIGNATION_GROUP_SHIFT, 0x7F)), ..magazine };
    let report = designation_report(Some(&page), &reserved, &config);
    assert!(report.fallback);
    assert_eq!(report.designation, charset::designation(1).unwrap());
}

/// REQ-CHR-012: neoteletext's own designations name Latin English as
/// the second set when no Table 33 value is received, and a received
/// "no second set" is kept.
#[cfg(feature = "extended-sets")]
#[test]
fn an_extended_designation_has_english_second() {
    let ukrainian = 0b0111 << DESIGNATION_GROUP_SHIFT;
    let config = Config { default_region: 0b0111, ..Config::default() };
    let report = designation_report(None, &MagazineData::default(), &config);
    assert_eq!((report.value, report.designation.g0.set), (ukrainian, charset::G0Set::Ukrainian));
    assert_eq!(report.second, second_g0(0));
    let fields = crate::types::PresentationFields {
        designations: Some(crate::types::Designations { default: ukrainian, second: 0x7F }),
        ..crate::types::PresentationFields::default()
    };
    let sent = MagazineData { data: Some(fields), ..MagazineData::default() };
    assert_eq!(designation_report(None, &sent, &Config::default()).second, None);
    // The standard's designations have no default second set.
    let report = designation_report(None, &MagazineData::default(), &Config::default());
    assert_eq!(report.second, None);
}

/// REQ-TST-013's double height with its ignored lower row.
#[test]
fn double_height_fills_the_row_below() {
    let mut page = StoredPage::EMPTY;
    page.state = crate::fsm::PageState::Complete;
    page.cells[1][..4].copy_from_slice(&[DOUBLE_HEIGHT, b'D', NORMAL_SIZE, b'N']);
    page.cells[2][0] = b'X';
    for row in &mut page.cells {
        for byte in row.iter_mut() {
            *byte = crate::ecc::with_odd_parity(*byte);
        }
    }
    page.parity_ok = [u64::MAX; 26];
    page.ever_good = [u64::MAX; 26];
    page.rows_received = 0b110;
    let mut grid = Grid::EMPTY;
    build_grid(&mut grid, Some(&page), &MagazineData::default(), &Config::default(), &DisplayOptions::default());
    assert!(grid.row_has_double_height(1));
    assert_eq!(grid.cells[1][1].size, Size::DoubleTop);
    assert_eq!(grid.cells[2][1].size, Size::DoubleBottom);
    assert_eq!(grid.cells[2][0].glyph, Glyph::Space, "row 2's own X is ignored");
    assert_eq!(grid.cells[2][3].glyph, Glyph::Space);
}

/// A complete page whose rows are `rows`, every byte with odd parity
/// and recorded as passed.
fn page_with_rows(rows: &[(usize, &[u8])]) -> StoredPage {
    let mut page = StoredPage::EMPTY;
    page.state = crate::fsm::PageState::Complete;
    page.cells = [[crate::ecc::with_odd_parity(b' '); 40]; 26];
    for &(row, text) in rows {
        for (cell, &byte) in page.cells[row].iter_mut().zip(text) {
            *cell = crate::ecc::with_odd_parity(byte);
        }
        page.rows_received |= 1 << row;
    }
    page.parity_ok = [u64::MAX; 26];
    page.ever_good = [u64::MAX; 26];
    page
}

/// REQ-L1-014, decision E.10: row 0 is the receiver's status area in
/// columns 0 to 6, a colour attribute in column 7 (white when found,
/// green when searching), then header bytes 14 to 45: the rolling
/// header's while searching, where a byte failing parity is a space,
/// and otherwise the displayed page's, whose failures count from column
/// 8 only. With neither, columns 8 to 39 are spaces.
#[test]
fn row_0_is_the_status_area_and_a_header() {
    use crate::consts::{COLOUR_GREEN, COLOUR_WHITE, SPACE};
    let config = Config::default();
    let mut header = [b' '; 40];
    header[8..14].copy_from_slice(b"SHOWN!");
    let mut page = page_with_rows(&[(0, &header)]);
    page.parity_ok[0] &= !(1 << 9 | 1 << 3);
    page.ever_good[0] &= !(1 << 9 | 1 << 3);
    let mut options = DisplayOptions { status: *b"P100\xC1  ", ..DisplayOptions::default() };
    let (codes, failed) = row_codes(0, Some(&page), &config, &options, config.parity_policy);
    assert_eq!(&codes[..7], b"P100A  ", "7-bit codes of the status area");
    assert_eq!(codes[7], COLOUR_WHITE);
    assert_eq!((&codes[8..14], codes[9]), (&b"S OWN!"[..], SPACE), "column 9 failed parity");
    assert_eq!(failed, 1 << 9, "the receiver's own columns report no failure");
    let mut rolling = [crate::ecc::with_odd_parity(b' '); 32];
    // The X has even parity, a failure.
    let failed = crate::ecc::with_odd_parity(b'X') ^ 0x80;
    rolling[..3].copy_from_slice(&[crate::ecc::with_odd_parity(b'R'), failed, crate::ecc::with_odd_parity(b'L')]);
    options.searching = true;
    options.rolling_header = Some(rolling);
    let (codes, failed) = row_codes(0, Some(&page), &config, &options, config.parity_policy);
    assert_eq!((codes[7], &codes[8..12], failed), (COLOUR_GREEN, &b"R L "[..], 0));
    options.rolling_header = None;
    let (codes, _) = row_codes(0, None, &config, &options, config.parity_policy);
    assert_eq!(codes[8..], [SPACE; 32]);
}

/// Decision E.11: on a page found, columns 32 to 39 are the latest
/// header's clock, of any magazine, so the time keeps ticking on a page
/// that does not change; a byte failing parity is a space. With no clock
/// the page keeps its own.
#[test]
fn a_found_page_shows_the_latest_clock() {
    let config = Config::default();
    let mut header = [b' '; 40];
    header[8..16].copy_from_slice(b"SERVICE ");
    header[32..40].copy_from_slice(b"12:00/00");
    let page = page_with_rows(&[(0, &header)]);
    let options = DisplayOptions { status: *b"P100   ", ..DisplayOptions::default() };
    let (codes, _) = row_codes(0, Some(&page), &config, &options, config.parity_policy);
    assert_eq!(&codes[32..], b"12:00/00", "its own, with no clock received");
    let mut clock = [0_u8; 8];
    for (slot, &byte) in clock.iter_mut().zip(b"23:15/42") {
        *slot = crate::ecc::with_odd_parity(byte);
    }
    clock[7] ^= 0x80;
    let options = DisplayOptions { clock: Some(clock), ..options };
    let (codes, _) = row_codes(0, Some(&page), &config, &options, config.parity_policy);
    assert_eq!(&codes[8..16], b"SERVICE ", "the page's own header");
    assert_eq!(&codes[32..], b"23:15/4 ", "the latest clock, its failed byte a space");
}

/// REQ-L1-014, REQ-SEC-005: a page with C7 set has no row 0, except the
/// status area when the configuration asks for it or a security
/// indicator is active. REQ-L1-016: with C10 set rows 1 to 24 are not
/// displayed and row 0 is.
#[test]
fn c7_hides_row_0_and_c10_rows_1_to_24() {
    use crate::consts::{C7_SUPPRESS_HEADER, C10_INHIBIT_DISPLAY};
    use crate::types::ControlBits;
    let mut header = [b' '; 40];
    header[8..12].copy_from_slice(b"HEAD");
    let mut page = page_with_rows(&[(0, &header), (1, b"ROW ONE"), (24, b"ROW 24")]);
    let options = DisplayOptions { status: *b"P100   ", ..DisplayOptions::default() };
    let build = |page: &StoredPage, config: &Config, options: &DisplayOptions| {
        let mut grid = Grid::EMPTY;
        build_grid(&mut grid, Some(page), &MagazineData::default(), config, options);
        grid
    };
    let shown = |grid: &Grid, row: usize, column: usize| grid.cells[row][column].glyph != Glyph::Space;
    page.control = ControlBits(C7_SUPPRESS_HEADER);
    let grid = build(&page, &Config::default(), &options);
    assert_eq!(grid.cells[0], [Cell::BLANK; 40]);
    assert!(shown(&grid, 1, 0));
    for (config, options) in [
        (Config { status_on_suppressed_header: true, ..Config::default() }, options),
        (Config::default(), DisplayOptions { security_indicator: true, ..options }),
    ] {
        let grid = build(&page, &config, &options);
        assert!(shown(&grid, 0, 0) && shown(&grid, 0, 3), "the status area is kept");
        assert_eq!(grid.cells[0][8..], [Cell::BLANK; 32], "the header is not");
    }
    page.control = ControlBits(C10_INHIBIT_DISPLAY);
    let config = Config { row24: crate::config::Row24Policy::WhenPresent, ..Config::default() };
    let grid = build(&page, &config, &options);
    assert!(shown(&grid, 0, 0) && shown(&grid, 0, 8));
    for row in 1..25 {
        assert_eq!(grid.cells[row], [Cell::BLANK; 40], "row {row}");
    }
}

/// REQ-L1-015, decision E.23, REQ-NAV-011: row 24 is the receiver's own
/// when it draws one, the page's when the configured rule shows it, and
/// blank otherwise. REQ-L1-017: with no page a row is 40 spaces.
#[test]
fn row_24_follows_decision_e23() {
    use crate::config::Row24Policy;
    use crate::consts::SPACE;
    let page = page_with_rows(&[(1, b"ONE"), (24, b"PAGE ROW 24")]);
    let codes = |policy: Row24Policy, options: &DisplayOptions, page: Option<&StoredPage>| {
        let config = Config { row24: policy, ..Config::default() };
        row_codes(24, page, &config, options, config.parity_policy).0
    };
    let plain = DisplayOptions::default();
    assert_eq!(&codes(Row24Policy::WhenPresent, &plain, Some(&page))[..11], b"PAGE ROW 24");
    assert_eq!(codes(Row24Policy::Never, &plain, Some(&page)), [SPACE; 40]);
    assert_eq!(codes(Row24Policy::AnnexH, &plain, Some(&page)), [SPACE; 40], "no X/27/0 asking for it");
    let mut drawn = [b' '; 40];
    drawn[..4].copy_from_slice(b"\x81RED");
    let receiver = DisplayOptions { row24: Some(drawn), ..DisplayOptions::default() };
    assert_eq!(&codes(Row24Policy::WhenPresent, &receiver, Some(&page))[..4], b"\x01RED", "7-bit codes");
    let config = Config::default();
    assert_eq!(row_codes(5, None, &config, &plain, config.parity_policy), ([SPACE; 40], 0));
}

/// REQ-L1-018, decision E.8: a character that failed parity and never
/// passed is a space, or under the diagnostic policy the solid block
/// 7/F of Table 35, and its column is reported as failed.
#[test]
fn failed_characters_follow_the_parity_policy() {
    use crate::config::ParityPolicy;
    use crate::consts::SPACE;
    // Table 35: position 7/F of the G0 sets, a solid block.
    const SOLID_BLOCK: u8 = 0x7F;
    let mut page = page_with_rows(&[(2, b"ABC")]);
    page.parity_ok[2] &= !(1 << 1);
    page.ever_good[2] &= !(1 << 1);
    let config = Config::default();
    let options = DisplayOptions::default();
    for (policy, shown) in [
        (ParityPolicy::KeepLastGood, SPACE),
        (ParityPolicy::AlwaysSpace, SPACE),
        (ParityPolicy::ErrorMarker, SOLID_BLOCK),
    ] {
        let (codes, failed) = row_codes(2, Some(&page), &config, &options, policy);
        assert_eq!((&codes[..3], failed), (&[b'A', shown, b'C'][..], 1 << 1), "{policy:?}");
    }
}

/// REQ-CHR-003, REQ-CHR-012: a receiver region whose group is reserved
/// in Table 32 falls back to group 0000 with the same national option,
/// and names no second G0 set.
#[test]
fn a_reserved_receiver_region_falls_back_with_no_second_set() {
    let config = Config { default_region: 0b1101, ..Config::default() };
    let report = designation_report(None, &MagazineData::default(), &config);
    assert_eq!((report.source, report.group, report.fallback), (DesignationSource::Receiver, 0b1101, true));
    assert_eq!((report.designation, report.second), (charset::designation(0).unwrap(), None));
}
