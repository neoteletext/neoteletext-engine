//! Page extension data that packets cannot carry (decision 7 of
//! IMPLEMENTATION-LOG.md): TTI Link Extension links with their labels and
//! services (Appendix E) and forms (Appendix C), kept beside the page store
//! and read by the viewer.

use neoteletext_core::charset::{CharRef, CharSet, G2Set};
use neoteletext_core::consts::{COLUMNS, G2_RIGHT_GUILLEMET, ROWS};
use neoteletext_core::forms::FormSpec;
use neoteletext_core::level1::{Glyph, Grid};
use neoteletext_core::nav::{LinkTarget, PageLinks};
use neoteletext_core::types::{PageAddress, SubCode};
use neoteletext_formats::links::{self, INDEX_KEY as FORMATS_INDEX_KEY, KEYS_WITH_INDEX as FORMATS_KEYS};
use neoteletext_formats::page::PageFile;

mod forms;

pub use forms::form_spec;

/// What the viewer needs beyond the packets of one page.
#[derive(Clone, Debug)]
pub struct PageExtras {
    /// The address the page came from: the base and origin of its form.
    pub url: String,
    /// The links of the TTI Link Extension, when the page has `LN` lines.
    pub links: Option<PageLinks>,
    /// Row 24 drawn by the receiver from the links (REQ-NAV-011), 7-bit
    /// codes, and the columns where the "»" mark goes.
    pub row24: Option<([u8; COLUMNS], Vec<usize>)>,
    /// The form, when the page has an interactive one.
    pub form: Option<FormSpec>,
    /// The addresses the coloured keys and Index lead to, when an `LN`
    /// line names an address; indexed red, green, yellow, cyan, Index.
    pub key_addresses: [Option<String>; KEYS_WITH_INDEX],
    /// `LK` lines: page numbers on this page that lead elsewhere.
    pub number_links: Vec<(PageAddress, Destination)>,
    /// `LT` lines: runs of text that are links.
    pub text_links: Vec<TextLinkExtras>,
    /// The `IM` line: the input modes the page asks for while it is open
    /// (REQ-INT-114).
    pub input_modes: Option<u32>,
}

/// The coloured keys and Index.
pub const KEYS_WITH_INDEX: usize = FORMATS_KEYS;
/// Index's place among [`PageExtras::key_addresses`].
pub const INDEX_KEY: usize = FORMATS_INDEX_KEY;

/// Where a page-number or text link leads, with services resolved.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Destination {
    /// A page of this service, or of a service the descriptor names.
    Page(LinkTarget),
    /// An address of another site's page file.
    Address(String),
}

/// A run of text that is a link (`LT`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextLinkExtras {
    /// The row.
    pub row: u8,
    /// The first column.
    pub column: u8,
    /// The characters linked.
    pub length: u8,
    /// Where it leads.
    pub destination: Destination,
}

impl PageExtras {
    /// The page link for a number typed or chosen on this page.
    #[must_use]
    pub fn number_link(&self, number: PageAddress) -> Option<&Destination> {
        self.number_links.iter().find(|(linked, _)| *linked == number).map(|(_, destination)| destination)
    }

    /// The text link at a cell, if any.
    #[must_use]
    pub fn text_link_at(&self, row: usize, column: usize) -> Option<&Destination> {
        self.text_links
            .iter()
            .find(|link| {
                usize::from(link.row) == row
                    && (usize::from(link.column)..usize::from(link.column) + usize::from(link.length)).contains(&column)
            })
            .map(|link| &link.destination)
    }
}

/// A destination of the shared reader of links (`neoteletext_formats::links`)
/// as this viewer follows it: a web page is null, since it opens only
/// teletext (decision E.88), and was already made null there.
fn destination(target: links::Destination<'_>) -> Option<Destination> {
    match target {
        links::Destination::Page { address, subcode, service } => {
            Some(Destination::Page(LinkTarget { address, subcode: subcode.unwrap_or(SubCode::ANY), service }))
        }
        links::Destination::Address(url) => Some(Destination::Address(String::from_utf8_lossy(url).into_owned())),
        links::Destination::Web(_) => None,
    }
}

/// The extension data of a page file. `services` are the descriptor's
/// linked services, in order; a link naming another is null (Appendix E.2
/// rule 2). The links and row 24 are `neoteletext_formats::links`'s, the
/// one implementation every port uses.
#[must_use]
pub fn from_page(page: &PageFile, url: &str, services: &[String]) -> PageExtras {
    let mut extras = PageExtras {
        url: url.to_string(),
        links: None,
        row24: None,
        form: None,
        key_addresses: Default::default(),
        number_links: Vec::new(),
        text_links: Vec::new(),
        input_modes: page.input_modes.map(|line| line.modes),
    };
    if let Some(receiver) = links::receiver_links(page, services, false) {
        let mut page_links = PageLinks::default();
        for (index, key) in receiver.keys.into_iter().enumerate() {
            let (target, address) = match key.and_then(destination) {
                Some(Destination::Page(target)) => (Some(target), None),
                // An address is not a page of this service: the key has
                // no X/27 link, and the address goes to the interface.
                Some(Destination::Address(address)) => (None, Some(address)),
                None => (None, None),
            };
            match page_links.keys.get_mut(index) {
                Some(slot) => *slot = target,
                None => page_links.index = target,
            }
            if let Some(slot) = extras.key_addresses.get_mut(index) {
                *slot = address;
            }
        }
        extras.links = Some(page_links);
        extras.row24 = Some((
            receiver.row.codes,
            receiver.row.marks.as_slice().iter().map(|&column| usize::from(column)).collect(),
        ));
    }
    for link in page.number_links.as_slice() {
        if let Some(destination) = links::resolve(&link.destination, services, false).and_then(destination) {
            extras.number_links.push((link.number, destination));
        }
    }
    for link in page.text_links.as_slice() {
        if let Some(destination) = links::resolve(&link.destination, services, false).and_then(destination) {
            extras.text_links.push(TextLinkExtras {
                row: link.row,
                column: link.column,
                length: link.length,
                destination,
            });
        }
    }
    if page.form.interactive {
        extras.form = form_spec(&page.form);
    }
    extras
}

/// Places the "»" marks of a receiver-drawn row 24 as Latin G2 characters.
pub fn draw_marks(grid: &mut Grid, marks: &[usize]) {
    for &column in marks {
        if let Some(cell) = grid.cell_mut(ROWS - 1, column) {
            cell.glyph = Glyph::Char(CharRef::new(CharSet::G2(G2Set::Latin), G2_RIGHT_GUILLEMET));
        }
    }
}

/// The page and sub-code of a page file, the key of the extension table.
#[must_use]
pub const fn key(page: &PageFile) -> (PageAddress, SubCode) {
    (page.address, page.subcode)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a test")]
mod tests {
    use super::*;

    fn page_file(tti: &[u8]) -> PageFile {
        let mut pages = Vec::new();
        let mut reader = neoteletext_formats::tti::TtiReader::new();
        let mut keep = |item: neoteletext_formats::Item<'_>| {
            if let neoteletext_formats::Item::Page(page) = item {
                pages.push(*page);
            }
        };
        neoteletext_formats::Reader::feed(&mut reader, tti, &mut keep);
        neoteletext_formats::Reader::finish(&mut reader, &mut keep);
        pages.remove(0)
    }

    /// REQ-NAV-011: no attribute from the page reaches the receiver's row:
    /// a label's control codes are drawn as spaces.
    #[test]
    fn a_labels_control_codes_never_reach_row_24() {
        let page = page_file(b"PN,10000\r\nOL,1,TEXT\r\nLN,R,101,Fl\x08ash\r\nLN,G,140,Big\x0dTall\r\n");
        let (row, _) = from_page(&page, "https://example.org/100.tti", &[]).row24.unwrap();
        assert!(row.iter().enumerate().all(|(column, &code)| code >= b' ' || column % (COLUMNS / 4) == 0), "{row:?}");
        assert_eq!(&row[1..7], b"Fl ash");
        assert_eq!(&row[11..19], b"Big Tall");
    }
}
