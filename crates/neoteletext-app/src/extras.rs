//! Page extension data that packets cannot carry (decision 7 of
//! IMPLEMENTATION-LOG.md): TTI Link Extension links with their labels and
//! services (Appendix E) and forms (Appendix C), kept beside the page store
//! and read by the viewer.

use neoteletext_core::charset::{CharRef, CharSet, G2Set};
use neoteletext_core::consts::{
    ALPHA_RED, ALPHA_YELLOW, COLOUR_ATTRIBUTE_MASK, COLUMNS, G2_RIGHT_GUILLEMET, ROWS, SPACE,
};
use neoteletext_core::forms::{FieldKind, FieldSpec, FormSpec, Method, OptionSpec};
use neoteletext_core::level1::{Glyph, Grid};
use neoteletext_core::nav::{LinkTarget, PageLinks};
use neoteletext_core::types::{FixedVec, PageAddress, SubCode};
use neoteletext_formats::page::{self, LinkKey, PageFile};

/// Colours of the four prompts: red, green, yellow, cyan (REQ-NAV-011).
const KEY_COLOURS: [u8; 4] = [ALPHA_RED, ALPHA_RED + 1, ALPHA_YELLOW, ALPHA_YELLOW + 3];
/// Columns per prompt: four equal prompts across the row.
const PROMPT_COLUMNS: usize = COLUMNS / 4;
/// Label characters after the colour attribute.
const LABEL_CHARACTERS: usize = PROMPT_COLUMNS - 1;

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
pub const KEYS_WITH_INDEX: usize = 5;
/// Index's place among [`PageExtras::key_addresses`].
pub const INDEX_KEY: usize = 4;

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

/// The service a link names.
enum Named {
    /// This service.
    This,
    /// A linked service, by its index among the descriptor's.
    Linked(u8),
}

/// The service named in a link; `None` for one the descriptor does not
/// name, which makes the link null (Appendix E.2 rule 2).
fn service_index(name: &[u8], services: &[String]) -> Option<Named> {
    if name.is_empty() {
        return Some(Named::This);
    }
    let name = String::from_utf8_lossy(name);
    services.iter().position(|known| *known == name).and_then(|index| u8::try_from(index).ok()).map(Named::Linked)
}

/// A link's destination with its service resolved; `None` when null. A
/// link to a web page is null here: this viewer opens only teletext
/// (decision E.88).
fn destination(target: &page::LinkDestination, services: &[String]) -> Option<Destination> {
    if target.is_web() {
        return None;
    }
    if !target.url.is_empty() {
        return Some(Destination::Address(String::from_utf8_lossy(target.url.as_bytes()).into_owned()));
    }
    let service = match service_index(target.service.as_bytes(), services)? {
        Named::This => None,
        Named::Linked(index) => Some(index),
    };
    Some(Destination::Page(LinkTarget {
        address: target.address,
        subcode: target.subcode.unwrap_or(SubCode::ANY),
        service,
    }))
}

fn key_index(key: LinkKey) -> Option<usize> {
    match key {
        LinkKey::Red => Some(0),
        LinkKey::Green => Some(1),
        LinkKey::Yellow => Some(2),
        LinkKey::Cyan => Some(3),
        LinkKey::Index => None,
    }
}

/// The extension data of a page file. `services` are the descriptor's
/// linked services, in order; a link naming another is null (Appendix E.2
/// rule 2).
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
    if page.uses_link_lines() && page.links_valid {
        let mut links = PageLinks::default();
        let mut row = [SPACE; COLUMNS];
        let mut marks = Vec::new();
        for line in page.links.as_slice() {
            let target = page::LinkDestination {
                service: line.service,
                address: line.address,
                subcode: line.subcode,
                url: line.url,
            };
            let Some(destination) = destination(&target, services) else { continue };
            let other_site = match &destination {
                Destination::Page(target) => target.service.is_some(),
                Destination::Address(_) => true,
            };
            let (page_target, address) = match destination {
                Destination::Page(target) => (Some(target), None),
                // An address is not a page of this service: the key has
                // no X/27 link, and the address goes to the interface.
                Destination::Address(address) => (None, Some(address)),
            };
            if let Some(index) = key_index(line.key) {
                links.keys[index] = page_target;
                extras.key_addresses[index] = address;
                let start = index * PROMPT_COLUMNS;
                row[start] = KEY_COLOURS[index] & COLOUR_ATTRIBUTE_MASK;
                let mut column = start + 1;
                if other_site {
                    // The receiver's mark for leaving the service.
                    marks.push(column);
                    column += 1;
                }
                for &byte in line.label.as_bytes().iter().take(start + 1 + LABEL_CHARACTERS - column) {
                    row[column] = byte & 0x7F;
                    column += 1;
                }
            } else {
                links.index = page_target;
                extras.key_addresses[INDEX_KEY] = address;
            }
        }
        extras.links = Some(links);
        extras.row24 = Some((row, marks));
    }
    for link in page.number_links.as_slice() {
        if let Some(destination) = destination(&link.destination, services) {
            extras.number_links.push((link.number, destination));
        }
    }
    for link in page.text_links.as_slice() {
        if let Some(destination) = destination(&link.destination, services) {
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

fn fixed<const N: usize>(bytes: &[u8]) -> FixedVec<u8, N> {
    let mut out = FixedVec::new(0);
    for &byte in bytes {
        if !out.push(byte & 0x7F) {
            break;
        }
    }
    out
}

fn number(text: &[u8], default: i32) -> i32 {
    std::str::from_utf8(text).ok().and_then(|text| text.trim().parse().ok()).unwrap_or(default)
}

/// The core form model's specification of a TTI form (Appendix C).
#[must_use]
pub fn form_spec(form: &page::Form) -> Option<FormSpec> {
    let (method, target) = form.submission.as_ref()?;
    let mut spec = FormSpec {
        fields: FixedVec::new(FieldSpec::new(FieldKind::Text, 0, 0, 1)),
        options: FixedVec::new(OptionSpec {
            value: FixedVec::new(0),
            display: FixedVec::new(0),
            selected: false,
            column: None,
        }),
        method: match method {
            page::FormMethod::Post => Method::Post,
            page::FormMethod::Get => Method::Get,
        },
        target: fixed(target.as_bytes()),
        success: form.result_pages.map(|(success, _)| success),
        error: form.result_pages.map(|(_, error)| error),
        token: fixed(form.token.as_bytes()),
    };
    for (index, field) in form.fields.as_slice().iter().enumerate() {
        let kind = match field.kind {
            page::FieldKind::Text => FieldKind::Text,
            page::FieldKind::Email => FieldKind::Email,
            page::FieldKind::Telephone => FieldKind::Tel,
            page::FieldKind::Number => FieldKind::Number,
            page::FieldKind::Date => FieldKind::Date,
            page::FieldKind::Time => FieldKind::Time,
            page::FieldKind::Select => FieldKind::Select,
            page::FieldKind::Radio => FieldKind::Radio,
            page::FieldKind::Checkbox => FieldKind::Checkbox,
            page::FieldKind::TextArea => FieldKind::TextArea,
            page::FieldKind::Submit => FieldKind::Submit,
            page::FieldKind::Reset => FieldKind::Reset,
        };
        let mut spec_field = FieldSpec::new(kind, field.row, field.column, field.width);
        spec_field.name = fixed(field.name.as_bytes());
        spec_field.rows = field.rows.max(1);
        spec_field.required = field.required;
        spec_field.maxlen = field.max_length.unwrap_or(0);
        spec_field.pattern = fixed(field.pattern.as_bytes());
        spec_field.default = fixed(field.default.as_bytes());
        spec_field.checked = field.checked;
        spec_field.value = fixed(field.value.as_bytes());
        spec_field.label = fixed(field.label.as_bytes());
        spec_field.action = fixed(field.action.as_bytes());
        if kind == FieldKind::Number {
            spec_field.min = number(field.minimum.as_bytes(), i32::MIN);
            spec_field.max = number(field.maximum.as_bytes(), i32::MAX);
            spec_field.step = number(field.step.as_bytes(), 0);
        } else {
            spec_field.min_text = fixed(field.minimum.as_bytes());
            spec_field.max_text = fixed(field.maximum.as_bytes());
        }
        let first = spec.options.len();
        for option in form.options.as_slice().iter().filter(|option| usize::from(option.field) == index) {
            let _ = spec.options.push(OptionSpec {
                value: fixed(option.value.as_bytes()),
                display: fixed(option.display.as_bytes()),
                selected: option.selected,
                column: option.column,
            });
        }
        spec_field.first_option = u8::try_from(first).unwrap_or(0);
        spec_field.option_count = u8::try_from(spec.options.len() - first).unwrap_or(0);
        if !spec.fields.push(spec_field) {
            break;
        }
    }
    Some(spec)
}

/// The page and sub-code of a page file, the key of the extension table.
#[must_use]
pub const fn key(page: &PageFile) -> (PageAddress, SubCode) {
    (page.address, page.subcode)
}
