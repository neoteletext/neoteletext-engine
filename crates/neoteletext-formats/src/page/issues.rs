//! The problems and warnings a reader records on a page, for the page
//! checker (REQ-CLI-006) and the viewer.

/// Problems recorded on one page for the page checker.
pub const PAGE_ISSUES: usize = 16;

/// Problems a reader records on a page, for the page checker
/// (REQ-CLI-006) and the viewer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum IssueKind {
    /// More than four coloured `LN` links; the page has no links
    /// (Appendix E.2 rule 1).
    TooManyLinks,
    /// A key used twice; the page has no links (Appendix E.2 rule 1).
    RepeatedKey,
    /// An `LN` target that could not be read.
    BadLinkTarget,
    /// The withdrawn `TF` line, which named a page's font (decision E.111).
    WithdrawnCommand,
    /// A `CU` line that could not be read (REQ-INT-006).
    BadCursorLine,
    /// An `IM` line that could not be read or names modes not defined
    /// (REQ-INT-114).
    BadModesLine,
    /// An `LG` line that was refused (REQ-INP-026).
    BadLogLine(crate::log::LogRefused),
    /// More `LG` lines than a page may carry (REQ-INP-026).
    TooManyLogLines,
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
    /// Content in row 24 of a page with links, which the receiver replaces
    /// (REQ-NAV-011).
    HiddenContent,
}

impl Warning {
    /// Every warning.
    pub const ALL: [Self; 3] = [Self::PageNumberNotShown, Self::NoOtherRoute, Self::HiddenContent];

    /// The name used in `CK` lines and reports.
    #[must_use]
    pub const fn name(self) -> &'static [u8] {
        match self {
            Self::PageNumberNotShown => b"page-number-not-shown",
            Self::NoOtherRoute => b"no-other-route",
            Self::HiddenContent => b"hidden-content",
        }
    }

    /// The warning with a name, ignoring case.
    #[must_use]
    pub fn from_name(name: &[u8]) -> Option<Self> {
        Self::ALL.into_iter().find(|warning| warning.name().eq_ignore_ascii_case(name))
    }

    /// The warning's bit in [`PageFile::considered`](super::PageFile::considered).
    #[must_use]
    pub const fn bit(self) -> u16 {
        1 << (self as u16)
    }
}

/// A problem and the line it was found on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct PageIssue {
    /// What was wrong.
    pub kind: IssueKind,
    /// The line of the file.
    pub line: u32,
}
