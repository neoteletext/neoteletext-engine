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
use crate::consts::{DESIGNATION_GROUP_SHIFT, DOUBLE_HEIGHT, ESC, NORMAL_SIZE, PAGE_FUNCTION_BASIC};

/// REQ-L25-006: the side-panel area is addressed by screen position,
/// columns 40 to 71 of rows 1 to 24, and a grid is cleared in place
/// to the empty grid.
#[test]
fn side_panel_area_and_clearing() {
    let mut grid: Grid = Grid::empty();
    let small: Grid<0> = Grid::empty();
    assert!(small.panel_cell(1, 40).is_none(), "a grid without panel rows");
    assert!(grid.panel_cell(0, 40).is_none(), "row 0 has no panel content");
    assert!(grid.panel_cell(1, 39).is_none() && grid.panel_cell(1, 72).is_none() && grid.panel_cell(25, 40).is_none());
    grid.panel_cell_mut(24, 71).unwrap().foreground = 20;
    assert_eq!(grid.panels[23][31].foreground, 20);
    let attributes = CellAttributes::DEFAULT.with_invert(true);
    grid.panel_cell_mut(1, 40).unwrap().attributes = attributes;
    assert!(grid.panels[0][0].attributes.invert());
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
            for rate in [FlashRate::Slow, FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3] {
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
