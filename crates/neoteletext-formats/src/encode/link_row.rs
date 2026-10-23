//! Row 24 drawn from a page's Link Extension links (Appendix E.2 rule 4,
//! REQ-NAV-011): the one drawing of the row, for broadcast and for a
//! receiver.

use neoteletext_core::consts::{
    ALPHA_BLACK, CODE_MASK_7BIT, COLOUR_BLUE_BIT, COLOUR_GREEN_BIT, COLOUR_RED_BIT, COLOUR_WHITE, COLUMNS, SPACE,
};
use neoteletext_core::types::FixedVec;

use crate::page::{LinkKey, LinkLine, PageFile};

use super::{GUILLEMET_FALLBACK, LABEL_CHARACTERS, OTHER_SERVICE_LABEL_CHARACTERS, PROMPT_COLUMNS};

/// The receiver-style row 24 of a page with Link Extension links
/// (Appendix E.2 rule 4 and REQ-NAV-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkRow {
    /// The row's 7-bit codes.
    pub codes: [u8; COLUMNS],
    /// Columns that hold the other-service mark, to be written as the G2
    /// guillemet by X/26.
    pub marks: FixedVec<u8, 4>,
}

/// The colour of a key's prompt: red, green, yellow and cyan (REQ-NAV-011).
pub(crate) const fn key_colour(key: LinkKey) -> u8 {
    match key {
        LinkKey::Red => COLOUR_RED_BIT,
        LinkKey::Green => COLOUR_GREEN_BIT,
        LinkKey::Yellow => COLOUR_RED_BIT | COLOUR_GREEN_BIT,
        LinkKey::Cyan => COLOUR_GREEN_BIT | COLOUR_BLUE_BIT,
        LinkKey::Index => COLOUR_WHITE,
    }
}

/// How one coloured link's prompt is drawn in row 24.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Prompt {
    /// The prompt's colour, as an alphanumeric colour's bits.
    pub(crate) colour: u8,
    /// The link leaves the service: the "»" mark goes before its label.
    pub(crate) leaves: bool,
}

/// Draws row 24 from a page's coloured `LN` links, each as `prompt` says,
/// or blank where it says `None` (REQ-NAV-011). The one drawing of the row,
/// for broadcast ([`link_row`]) and for a receiver
/// ([`crate::links::receiver_links`]).
pub(crate) fn draw_link_row(page: &PageFile, mut prompt: impl FnMut(&LinkLine) -> Option<Prompt>) -> LinkRow {
    let mut row = LinkRow { codes: [SPACE; COLUMNS], marks: FixedVec::new(0) };
    for link in page.links.as_slice().iter().filter(|link| link.key.is_coloured()) {
        let Some(Prompt { colour, leaves }) = prompt(link) else { continue };
        let start = link.key.fastext_index().saturating_mul(PROMPT_COLUMNS);
        let Some(codes) = row.codes.get_mut(start..start.saturating_add(PROMPT_COLUMNS)) else { continue };
        let (label_start, limit) = if leaves { (2, OTHER_SERVICE_LABEL_CHARACTERS) } else { (1, LABEL_CHARACTERS) };
        if let Some(attribute) = codes.first_mut() {
            *attribute = ALPHA_BLACK | colour;
        }
        if leaves {
            if let Some(mark) = codes.get_mut(1) {
                *mark = GUILLEMET_FALLBACK;
            }
            let _ = row.marks.push(u8::try_from(start.saturating_add(1)).unwrap_or(0));
        }
        let label = link.label.as_bytes().iter().take(limit);
        for (slot, &byte) in codes.iter_mut().skip(label_start).zip(label) {
            let code = byte & CODE_MASK_7BIT;
            // REQ-NAV-011: a label may not carry attributes into the row.
            *slot = if code < SPACE { SPACE } else { code };
        }
    }
    row
}

/// Draws row 24 from a page's `LN` links for broadcast, or `None` when the
/// page has no valid `LN` links. A link to another service is drawn in
/// white, with the "»" mark, as Appendix E.2 rule 4 asks for broadcast.
#[must_use]
pub fn link_row(page: &PageFile) -> Option<LinkRow> {
    if !page.uses_link_lines() || !page.links_valid {
        return None;
    }
    Some(draw_link_row(page, |link| {
        let leaves = link.is_other_service();
        Some(Prompt { colour: if leaves { COLOUR_WHITE } else { key_colour(link.key) }, leaves })
    }))
}
