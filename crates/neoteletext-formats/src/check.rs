//! The page checker behind `neottx check` (REQ-CLI-006): problems that would
//! make a TTI page fail on some receivers or behave unsafely, reported per
//! page with the line they were found on. The program adds the file name.

use neoteletext_core::consts::{PACKET_ROW_24, SPACE};
use neoteletext_core::types::PageAddress;

use crate::encode::{LABEL_CHARACTERS, OTHER_SERVICE_LABEL_CHARACTERS};
use crate::page::{IssueKind, LinkLine, PageFile};
// The warnings belong to the page, which records those marked considered;
// they are named here too, where the checker reports them.
pub use crate::page::Warning;

/// The first printable character a label may use; codes below are control
/// codes.
const PRINTABLE_FIRST: u8 = SPACE;
/// The last printable ASCII character; above it lie DEL and the bytes no
/// G0 set has.
const PRINTABLE_LAST: u8 = b'~';
/// Fragments that mark a web or e-mail address in page text. The check is
/// approximate: it only decides whether a warning is given.
const ADDRESS_MARKS: [&[u8]; 6] = [b"www.", b"http", b".co", b".org", b".net", b"@"];

/// How serious a finding is: errors fail the check (exit status 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Severity {
    /// The page will fail on some receivers or is unsafe.
    Error,
    /// The page works, but something deserves a look.
    Warning,
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
    /// The withdrawn `TF` line (decision E.111).
    WithdrawnCommand,
    /// A `CU` line that could not be read (REQ-INT-006).
    BadCursorLine,
    /// An `IM` line that could not be read (REQ-INT-114).
    BadModesLine,
    /// An `LG` line that was refused, and why (REQ-INP-026).
    BadLogLine(crate::log::LogRefused),
    /// More `LG` lines than a page may carry (REQ-INP-026).
    TooManyLogLines,
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
fn text_rows(page: &PageFile) -> impl Iterator<Item = &[u8; neoteletext_core::consts::COLUMNS]> {
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
    // A page that links a number it does not show makes typing that
    // number mean something the viewer cannot see
    // (the web-native concept section 5).
    for link in page.number_links.as_slice() {
        if !text_has_number(page, link.number.ascii()) {
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
            IssueKind::BadCursorLine => Problem::BadCursorLine,
            IssueKind::BadModesLine => Problem::BadModesLine,
            IssueKind::BadLogLine(why) => Problem::BadLogLine(why),
            IssueKind::TooManyLogLines => Problem::TooManyLogLines,
        };
        report(Severity::Error, problem, issue.line);
    }
    check_links(page, options, &mut report);
}

#[cfg(test)]
mod tests;
