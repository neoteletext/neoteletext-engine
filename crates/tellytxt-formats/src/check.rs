//! The page checker behind `ttx check` (REQ-CLI-006): problems that would
//! make a TTI page fail on some receivers or behave unsafely, reported per
//! page with the line they were found on. The program adds the file name.

use tellytxt_core::consts::{NATIONAL_POSITIONS, PACKET_ROW_24, SPACE};
use tellytxt_core::types::PageAddress;

use crate::encode::{LABEL_CHARACTERS, OTHER_SERVICE_LABEL_CHARACTERS};
use crate::page::{FieldKind, FormField, IssueKind, LinkLine, PageFile};

/// The first printable character a label may use; codes below are control
/// codes.
const PRINTABLE_FIRST: u8 = SPACE;
/// The last printable ASCII character; above it lie DEL and the bytes no
/// G0 set has.
const PRINTABLE_LAST: u8 = b'~';
/// Digits in a run that are taken for a telephone number.
const TELEPHONE_DIGITS: usize = 6;
/// Fragments that mark a web or e-mail address in page text. The check is
/// approximate, as Appendix C.6 allows.
const ADDRESS_MARKS: [&[u8]; 6] = [b"www.", b"http", b".co", b".org", b".net", b"@"];

/// How serious a finding is: errors fail the check (exit status 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Severity {
    /// The page will fail on some receivers or is unsafe.
    Error,
    /// The page works, but something deserves a look.
    Warning,
}

/// The warnings, which a page may mark as considered with
/// `CK,considered,<name>` (REQ-CLI-006).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Warning {
    /// A same-service link whose page number is not in the page text
    /// (Appendix E.2 rule 5).
    PageNumberNotShown,
    /// A link to another service with no other route given (Appendix E.2
    /// rule 6).
    NoOtherRoute,
    /// A form page with no fallback for broadcast viewers (Appendix C.6).
    NoBroadcastFallback,
    /// Form placeholders using national option characters (Appendix C.4).
    UnreliablePlaceholders,
    /// Content in row 24 of a page with links, which the receiver replaces
    /// (REQ-NAV-011).
    HiddenContent,
}

impl Warning {
    /// Every warning.
    pub const ALL: [Self; 5] = [
        Self::PageNumberNotShown,
        Self::NoOtherRoute,
        Self::NoBroadcastFallback,
        Self::UnreliablePlaceholders,
        Self::HiddenContent,
    ];

    /// The name used in `CK` lines and reports.
    #[must_use]
    pub const fn name(self) -> &'static [u8] {
        match self {
            Self::PageNumberNotShown => b"page-number-not-shown",
            Self::NoOtherRoute => b"no-other-route",
            Self::NoBroadcastFallback => b"no-broadcast-fallback",
            Self::UnreliablePlaceholders => b"unreliable-placeholders",
            Self::HiddenContent => b"hidden-content",
        }
    }

    /// The warning with a name, ignoring case.
    #[must_use]
    pub fn from_name(name: &[u8]) -> Option<Self> {
        Self::ALL.into_iter().find(|warning| warning.name().eq_ignore_ascii_case(name))
    }

    /// The warning's bit in [`PageFile::considered`].
    #[must_use]
    pub const fn bit(self) -> u16 {
        1 << (self as u16)
    }
}

/// What a finding is about.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Problem {
    /// More than four coloured links, or a repeated key.
    TooManyLinks,
    /// A link target that could not be read.
    BadLinkTarget,
    /// A link target naming a service missing from the descriptor.
    UnknownService,
    /// A label too long for its prompt.
    LabelTooLong,
    /// A label with characters the page's G0 set cannot show.
    LabelCharacters,
    /// Fields that overlap, fall outside their row, or cannot be read.
    BadFormField,
    /// A version 2.0 form.
    FormVersion2,
    /// The withdrawn `FH` command.
    WithdrawnCommand,
    /// A warning.
    Warning(Warning),
}

/// One problem found.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Finding {
    /// Error or warning.
    pub severity: Severity,
    /// What is wrong.
    pub problem: Problem,
    /// The page.
    pub page: PageAddress,
    /// Its sub-page number.
    pub subpage: u8,
    /// The line of the file.
    pub line: u32,
}

/// Options of the check.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CheckOptions<'a> {
    /// The service names of the descriptor, when one is given.
    pub services: Option<&'a [&'a [u8]]>,
}

/// Rows the page's own text occupies: 1 to 23. Row 0 is the header and row
/// 24 is the receiver's on pages with links.
fn text_rows(page: &PageFile) -> impl Iterator<Item = &[u8; tellytxt_core::consts::COLUMNS]> {
    (1..usize::from(PACKET_ROW_24)).filter_map(|row| page.row(row))
}

/// True when some row holds `pattern`, ignoring case.
fn text_contains(page: &PageFile, pattern: &[u8]) -> bool {
    !pattern.is_empty()
        && text_rows(page).any(|row| row.windows(pattern.len()).any(|window| window.eq_ignore_ascii_case(pattern)))
}

/// True when some row holds `digits` not run together with other digits.
fn text_has_number(page: &PageFile, digits: [u8; 3]) -> bool {
    text_rows(page).any(|row| {
        row.windows(digits.len()).enumerate().any(|(start, window)| {
            let before = start.checked_sub(1).and_then(|index| row.get(index)).is_some_and(u8::is_ascii_digit);
            let after = row.get(start.saturating_add(digits.len())).is_some_and(u8::is_ascii_digit);
            window.eq_ignore_ascii_case(&digits) && !before && !after
        })
    })
}

/// True when some row holds a run of digits, spaces allowed between them,
/// long enough for a telephone number.
fn text_has_telephone(page: &PageFile) -> bool {
    text_rows(page).any(|row| {
        let mut digits = 0usize;
        for &code in row {
            if code.is_ascii_digit() {
                digits = digits.saturating_add(1);
                if digits >= TELEPHONE_DIGITS {
                    return true;
                }
            } else if code != SPACE {
                digits = 0;
            }
        }
        false
    })
}

/// The cells a field's placeholder occupies, as (row, first column,
/// columns).
fn placeholder_runs(page: &PageFile, index: usize, field: &FormField, out: &mut dyn FnMut(usize, usize, usize)) {
    let rows = usize::from(field.row)..usize::from(field.row).saturating_add(usize::from(field.rows));
    for row in rows {
        out(row, usize::from(field.column), usize::from(field.width));
        if field.kind == FieldKind::Radio {
            let options = page.form.options.as_slice().iter().filter(|option| usize::from(option.field) == index);
            for column in options.skip(1).filter_map(|option| option.column) {
                out(row, usize::from(column), usize::from(field.width));
            }
        }
    }
}

fn check_links(page: &PageFile, options: &CheckOptions<'_>, report: &mut dyn FnMut(Severity, Problem, u32)) {
    for link in page.links.as_slice() {
        if let (Some(services), true) = (options.services, link.is_other_service())
            && !services.iter().any(|service| service.eq_ignore_ascii_case(link.service.as_bytes()))
        {
            report(Severity::Error, Problem::UnknownService, link.line);
        }
        if !link.key.is_coloured() {
            continue;
        }
        check_label(link, report);
        if link.is_other_service() {
            let named = text_contains(page, link.service.as_bytes());
            let addressed = ADDRESS_MARKS.iter().any(|mark| text_contains(page, mark));
            if !named && !addressed {
                report(Severity::Warning, Problem::Warning(Warning::NoOtherRoute), link.line);
            }
        } else if !text_has_number(page, link.address.ascii()) {
            report(Severity::Warning, Problem::Warning(Warning::PageNumberNotShown), link.line);
        }
    }
    if page.uses_link_lines() && page.links_valid {
        let row_24 = page.row(usize::from(PACKET_ROW_24)).filter(|_| page.has_row(usize::from(PACKET_ROW_24)));
        if row_24.is_some_and(|row| row.iter().any(|&code| code > SPACE)) {
            report(Severity::Warning, Problem::Warning(Warning::HiddenContent), page.line);
        }
    }
}

fn check_label(link: &LinkLine, report: &mut dyn FnMut(Severity, Problem, u32)) {
    let limit = if link.is_other_service() { OTHER_SERVICE_LABEL_CHARACTERS } else { LABEL_CHARACTERS };
    if link.label_truncated || link.label.len() > limit {
        report(Severity::Error, Problem::LabelTooLong, link.line);
    }
    if link.label.as_bytes().iter().any(|byte| !(PRINTABLE_FIRST..=PRINTABLE_LAST).contains(byte)) {
        report(Severity::Error, Problem::LabelCharacters, link.line);
    }
}

fn check_form(page: &PageFile, report: &mut dyn FnMut(Severity, Problem, u32)) {
    if page.form.fields.is_empty() {
        return;
    }
    let addressed = ADDRESS_MARKS.iter().any(|mark| text_contains(page, mark));
    if !addressed && !text_has_telephone(page) {
        report(Severity::Warning, Problem::Warning(Warning::NoBroadcastFallback), page.line);
    }
    for (index, field) in page.form.fields.as_slice().iter().enumerate() {
        if matches!(field.kind, FieldKind::Submit | FieldKind::Reset) {
            continue;
        }
        let mut national = false;
        placeholder_runs(page, index, field, &mut |row, column, width| {
            if let Some(cells) = page.row(row).and_then(|codes| codes.get(column..column.saturating_add(width))) {
                national |= cells
                    .iter()
                    .any(|&code| NATIONAL_POSITIONS.contains(&(code & tellytxt_core::consts::CODE_MASK_7BIT)));
            }
        });
        if national {
            report(Severity::Warning, Problem::Warning(Warning::UnreliablePlaceholders), field.line);
        }
    }
}

/// Checks one page, reporting each finding (REQ-CLI-006). Warnings the page
/// marks as considered are not reported.
pub fn check_page(page: &PageFile, options: &CheckOptions<'_>, out: &mut dyn FnMut(Finding)) {
    let mut report = |severity: Severity, problem: Problem, line: u32| {
        if let Problem::Warning(warning) = problem
            && page.considered & warning.bit() != 0
        {
            return;
        }
        out(Finding { severity, problem, page: page.address, subpage: page.subpage, line });
    };
    for issue in page.issues.as_slice() {
        let problem = match issue.kind {
            IssueKind::TooManyLinks | IssueKind::RepeatedKey => Problem::TooManyLinks,
            IssueKind::BadLinkTarget => Problem::BadLinkTarget,
            IssueKind::WithdrawnCommand => Problem::WithdrawnCommand,
            IssueKind::FormNeedsConversion => Problem::FormVersion2,
            IssueKind::FieldOverlap
            | IssueKind::FieldOutsideRow
            | IssueKind::FieldWithoutRow
            | IssueKind::BadField
            | IssueKind::OptionWithoutField => Problem::BadFormField,
        };
        report(Severity::Error, problem, issue.line);
    }
    check_links(page, options, &mut report);
    check_form(page, &mut report);
}

#[cfg(test)]
mod tests;
