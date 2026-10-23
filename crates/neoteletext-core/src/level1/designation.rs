//! How a page's character sets are chosen (decision E.4, REQ-CHR-003,
//! Annex C.4 rule 1).

use crate::acq::{MagazineData, StoredPage};
use crate::charset::{Designation, G0, default_second_g0, designation_with_fallback, second_g0};
use crate::config::Config;
use crate::consts::{CODE_MASK_7BIT, DESIGNATION_GROUP_SHIFT, ESC, NATIONAL_INDEX_MASK, PAGE_FUNCTION_BASIC, ROWS};

/// Where a page's designation group came from (REQ-CHR-003): the
/// highest-priority packet present, in the order of Annex C.4 rule 1, or
/// the receiver's setting when the page sends none.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DesignationSource {
    /// The page's packet X/28/0 Format 1.
    PageX28Format1,
    /// The page's packet X/28/4 (Level 3.5).
    PageX28Format4,
    /// The magazine's packet M/29/0.
    MagazineM29Format0,
    /// The magazine's packet M/29/4 (Level 3.5).
    MagazineM29Format4,
    /// None was sent: the receiver's configured region (decision E.4).
    Receiver,
}

/// Where the three national option bits came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NationalSource {
    /// C12 to C14 of the page header.
    Header,
    /// Bits 8 to 10 of the designation, at Levels 2.5 and 3.5 (Annex E.9).
    Designation,
    /// The receiver's fixed setting (`Config::fixed_national_index`).
    Fixed,
}

/// How a page's character sets were chosen, as the decoder resolved them
/// (REQ-CHR-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DesignationReport {
    /// C12 to C14 of the page header, as the national option index.
    pub header_national: u8,
    /// The four-bit group, and where it came from.
    pub group: u8,
    /// Where the group came from.
    pub source: DesignationSource,
    /// The national option index used, and where it came from.
    pub national: u8,
    /// Where the national option came from.
    pub national_source: NationalSource,
    /// The seven-bit Table 32 value looked up.
    pub value: u8,
    /// The entry used: the value's, or the fallback's.
    pub designation: Designation,
    /// The value's entry is reserved and REQ-CHR-003's fallback was used.
    pub fallback: bool,
    /// The second G0 set and national option of Table 33, if any.
    pub second: Option<G0>,
    /// Display rows 1 to 24 holding an ESC code, which toggles to the
    /// second set.
    pub esc_rows: u8,
}

/// How the page's character sets are chosen (decision E.4, REQ-CHR-003):
/// the group from the highest-priority designation, or the
/// configured default.
///
/// The designations are taken in the order of Annex C.4 rule 1: X/28/0
/// format 1, X/28/4 (Level 3.5 only), M/29/0, M/29/4 (Level 3.5 only)
/// (decision E.44); a source whose designation group is damaged is absent.
/// The national option comes from C12 to C14, except at Levels 2.5 and
/// 3.5, where bits 8 to 10 of the chosen designation give it when there is
/// one (Annex E.9, decisions E.4 and E.50; REQ-L25-012).
#[must_use]
pub fn designation_report(page: Option<&StoredPage>, magazine: &MagazineData, config: &Config) -> DesignationReport {
    let level_35 = config.level.is_level35();
    // A page's own X/28/0 or X/28/4 counts only when it is a basic Level 1
    // page; then the magazine's packets apply (decision E.4).
    let page_designations = page
        .and_then(|page| page.page_data)
        .filter(|data| data.format.is_some_and(|format| format.function == PAGE_FUNCTION_BASIC))
        .and_then(|data| data.fields.designations);
    let page_35 = page
        .and_then(|page| page.page_data_level35)
        .filter(|data| level_35 && data.format.is_some_and(|format| format.function == PAGE_FUNCTION_BASIC))
        .and_then(|data| data.fields.designations);
    let magazine_29 = magazine.data.and_then(|fields| fields.designations);
    let magazine_35 = magazine.level35.filter(|_| level_35).and_then(|fields| fields.designations);
    let (designations, source) = match (page_designations, page_35, magazine_29, magazine_35) {
        (Some(found), ..) => (Some(found), DesignationSource::PageX28Format1),
        (None, Some(found), ..) => (Some(found), DesignationSource::PageX28Format4),
        (None, None, Some(found), _) => (Some(found), DesignationSource::MagazineM29Format0),
        (None, None, None, Some(found)) => (Some(found), DesignationSource::MagazineM29Format4),
        (None, None, None, None) => (None, DesignationSource::Receiver),
    };
    let header_national = page.map_or(0, |page| page.control.national_index()) & NATIONAL_INDEX_MASK;
    let (national, national_source) = match (config.fixed_national_index, designations) {
        (Some(fixed), _) => (fixed, NationalSource::Fixed),
        (None, Some(designations)) if config.level.is_enhanced() => (designations.default, NationalSource::Designation),
        _ => (header_national, NationalSource::Header),
    };
    let national = national & NATIONAL_INDEX_MASK;
    let group =
        designations.map_or(config.default_region, |designations| designations.default >> DESIGNATION_GROUP_SHIFT);
    let value = (group << DESIGNATION_GROUP_SHIFT) | national;
    let (designation, fallback) = designation_with_fallback(value);
    // A received Table 33 value is used as it is; without one, the
    // designation's own default applies (REQ-CHR-012).
    let second = match designations {
        Some(designations) => second_g0(designations.second),
        None if fallback => None,
        None => default_second_g0(value),
    };
    let esc_rows = page.map_or(0, |page| {
        let rows = page.cells.iter().take(ROWS).skip(1);
        let with_esc = rows.filter(|row| row.iter().any(|&byte| byte & CODE_MASK_7BIT == ESC)).count();
        u8::try_from(with_esc).unwrap_or(u8::MAX)
    });
    DesignationReport {
        header_national,
        group,
        source,
        national,
        national_source,
        value,
        designation,
        fallback,
        second,
        esc_rows,
    }
}

/// Resolves the page's character sets (decision E.4, REQ-CHR-003): the
/// designation, the second G0 set, and whether a reserved entry needed the
/// fallback, from [`designation_report`].
#[must_use]
pub fn resolve_designation(
    page: Option<&StoredPage>,
    magazine: &MagazineData,
    config: &Config,
) -> (Designation, Option<G0>, bool) {
    let report = designation_report(page, magazine, config);
    (report.designation, report.second, report.fallback)
}
