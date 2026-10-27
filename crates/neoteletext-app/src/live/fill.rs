//! Which pages the bytes of one response may fill (REQ-WEB-019).

use neoteletext_core::types::PageAddress;

/// The highest digit of a page number a viewer can type.
const DECIMAL_DIGIT_LAST: u8 = 9;

/// Which pages the bytes of one response may fill (REQ-WEB-019).
#[derive(Clone, Copy)]
pub(super) enum Fill<'a> {
    /// Any page: a front door, a push message, a file.
    Any,
    /// A page's own file: the pages asked for, and enhancement pages,
    /// whose numbers are not decimal and which a page's file may carry
    /// (item 6).
    Asked(&'a std::collections::HashSet<u16>),
}

impl Fill<'_> {
    pub(super) fn allows(self, page: PageAddress) -> bool {
        match self {
            Self::Any => true,
            Self::Asked(asked) => {
                asked.contains(&page.mpp())
                    || page.page.tens() > DECIMAL_DIGIT_LAST
                    || page.page.units() > DECIMAL_DIGIT_LAST
            }
        }
    }
}
