//! The page file representation shared by the TTI, EP1, edit.tf and screen
//! dump readers: one sub-page of fixed size, with the extension data of the
//! TTI Link Extension (Appendix E) and Form Extension (Appendix C) kept
//! beside it (implementation decision 7).

use neoteletext_core::consts::{
    C4_ERASE, C12, C13, C14, COLUMNS, CONTROL_MASK, DEFAULT_INITIAL_PAGE, DESIGNATION_CODES, NATIONAL_INDEX_MASK,
    NATIONAL_SHIFT, SPACE, STORED_ROWS, TRIPLETS_PER_PACKET,
};
use neoteletext_core::packet::Triplet;
use neoteletext_core::types::{ControlBits, FixedVec, Magazine, PageAddress, PageNumber, SubCode};

use crate::consts::{TTI_PACKED_CHARACTERS, TTI_PS_C4, TTI_PS_C5_SHIFT, TTI_PS_C5_TO_C11, TTI_PS_C12_BIT};
use crate::text::Text;

/// Bytes kept of a description (`DE`) or source (`DS`); the MRG format
/// allows 24 characters.
pub const DESCRIPTION_BYTES: usize = 64;
/// Bytes kept of a link label. Labels may hold 9 characters (Appendix E.1);
/// more are kept so that the page checker can report them.
pub const LINK_LABEL_BYTES: usize = 40;
/// Bytes of a service name in a link target.
pub const SERVICE_NAME_BYTES: usize = 32;
/// Link Extension links per page: four coloured keys and Index
/// (Appendix E.1).
pub const LINK_LINES: usize = 5;
/// The longest address a link may name (`prototypes/browser/docs/web-native-concept.md`
/// section 3): longer addresses make the link invalid.
pub const LINK_ADDRESS_BYTES: usize = 256;
/// `LK` lines per page: page numbers linked elsewhere (concept section 5
/// item 3).
pub const NUMBER_LINKS: usize = 8;
/// `LT` lines per page: runs of text linked elsewhere (concept section 5
/// item 2).
pub const TEXT_LINKS: usize = 8;
/// Packed enhancement lines other than X/26 (`OL,27` with designation 1 to
/// 15, `OL,28`, `OL,29`) per page.
pub const RAW_PACKETS: usize = 16;
/// Form fields per page.
pub const FORM_FIELDS: usize = 32;
/// Form options per page, over all SELECT and RADIO fields.
pub const FORM_OPTIONS: usize = 64;
/// Bytes of a field name or option value.
pub const FIELD_NAME_BYTES: usize = 32;
/// Bytes of a field's default, value, label, pattern or option text: a
/// whole row.
pub const FIELD_TEXT_BYTES: usize = COLUMNS;
/// Bytes of a field's minimum, maximum or step.
pub const FIELD_LIMIT_BYTES: usize = 16;
/// Bytes of a form submission address.
pub const FORM_ADDRESS_BYTES: usize = 128;
/// Bytes of a form token.
pub const FORM_TOKEN_BYTES: usize = 64;
/// Problems recorded on one page for the page checker.
pub const PAGE_ISSUES: usize = 16;
/// Bytes of a `TF` line's font address (REQ-FNT-007); a longer one makes
/// the line malformed.
pub const FONT_ADDRESS_BYTES: usize = 256;
/// Hexadecimal digits of a `TF` line's SHA-256 (REQ-FNT-007).
pub const FONT_CHECKSUM_DIGITS: usize = 64;

/// The font a page names with a `TF` line (REQ-FNT-007). Pages from T42
/// packets or a broadcast cannot name one.
// Held inline, like every text of a page file: the crate allocates nothing,
// so the large variant cannot be boxed.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PageFont {
    /// No `TF` line.
    #[default]
    None,
    /// `TF,address,sha256`: the address as written, absolute or relative to
    /// the page's own address, and the 64 hexadecimal digits of its SHA-256.
    /// The address is checked by the strict address parser where it is
    /// used.
    Named {
        /// The font's address.
        address: Text<FONT_ADDRESS_BYTES>,
        /// Its SHA-256, as written.
        sha256: [u8; FONT_CHECKSUM_DIGITS],
    },
    /// A second `TF` line or a malformed one: the page names no font, and
    /// is drawn in the font that applies without it.
    Null,
}
/// Number of X/27/0 links (clause 9.6.1).
pub const FASTEXT_LINKS: usize = 6;

/// One packet X/26 of a page.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EnhancementPacket {
    /// The designation code, 0 to 15.
    pub designation: u8,
    /// The thirteen triplets.
    pub triplets: [Triplet; TRIPLETS_PER_PACKET],
}

/// The body of a packed enhancement line other than X/26.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RawBody {
    /// Thirteen triplets (`OL,28`, `OL,29`, `OL,27` designation 4 to 15).
    Triplets([Triplet; TRIPLETS_PER_PACKET]),
    /// Thirty-nine 4-bit values (`OL,27` designation 1 to 3).
    Nibbles([u8; TTI_PACKED_CHARACTERS]),
}

/// A packed enhancement line kept for encoding and writing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RawPacket {
    /// The packet number: 27, 28 or 29.
    pub number: u8,
    /// The designation code.
    pub designation: u8,
    /// The contents.
    pub body: RawBody,
}

/// One X/27/0 link: a page and sub-code.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct FastextLink {
    /// The page.
    pub address: PageAddress,
    /// The sub-code; 3F7F means any sub-page.
    pub subcode: SubCode,
}

/// The keys of the TTI Link Extension (Appendix E.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum LinkKey {
    /// Red.
    Red,
    /// Green.
    Green,
    /// Yellow.
    Yellow,
    /// Cyan.
    Cyan,
    /// Index.
    Index,
}

impl LinkKey {
    /// The key for a letter of an `LN` line.
    #[must_use]
    pub const fn from_letter(letter: u8) -> Option<Self> {
        match letter.to_ascii_uppercase() {
            b'R' => Some(Self::Red),
            b'G' => Some(Self::Green),
            b'Y' => Some(Self::Yellow),
            b'C' => Some(Self::Cyan),
            b'I' => Some(Self::Index),
            _ => None,
        }
    }

    /// The letter written in `LN` lines.
    #[must_use]
    pub const fn letter(self) -> u8 {
        match self {
            Self::Red => b'R',
            Self::Green => b'G',
            Self::Yellow => b'Y',
            Self::Cyan => b'C',
            Self::Index => b'I',
        }
    }

    /// The X/27/0 link this key uses: links 1 to 4 for the colours and link
    /// 6 for Index (REQ-NAV-010), counted from 0 here.
    #[must_use]
    pub const fn fastext_index(self) -> usize {
        match self {
            Self::Red => 0,
            Self::Green => 1,
            Self::Yellow => 2,
            Self::Cyan => 3,
            Self::Index => 5,
        }
    }

    /// True for the four coloured keys.
    #[must_use]
    pub const fn is_coloured(self) -> bool {
        !matches!(self, Self::Index)
    }
}

/// A link of the TTI Link Extension (Appendix E.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkLine {
    /// The key.
    pub key: LinkKey,
    /// The other service named, empty for this service.
    pub service: Text<SERVICE_NAME_BYTES>,
    /// The page.
    pub address: PageAddress,
    /// The sub-page, `None` for any sub-page.
    pub subcode: Option<SubCode>,
    /// The address of another site's page file, when the target is an
    /// address (`prototypes/browser/docs/web-native-concept.md` section 3); empty otherwise.
    pub url: Text<LINK_ADDRESS_BYTES>,
    /// The prompt text, as written.
    pub label: Text<LINK_LABEL_BYTES>,
    /// True when the label in the file was longer than this record keeps.
    pub label_truncated: bool,
    /// The line of the file.
    pub line: u32,
}

impl LinkLine {
    /// True when the target is in another service or at an address: both
    /// lead to another site.
    #[must_use]
    pub const fn is_other_service(&self) -> bool {
        !self.service.is_empty() || !self.url.is_empty()
    }
}

/// Where a page-number or text link leads: a page of this service, a page
/// of a named service, or an address (concept section 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkDestination {
    /// The other service named, empty for this service or an address.
    pub service: Text<SERVICE_NAME_BYTES>,
    /// The page; for an address, the page its file holds is the browser's
    /// to find out.
    pub address: PageAddress,
    /// The sub-page, `None` for any sub-page.
    pub subcode: Option<SubCode>,
    /// The address, when the target is one.
    pub url: Text<LINK_ADDRESS_BYTES>,
}

/// An `LK` line: typing or choosing `number` on this page leads to
/// `destination` (concept section 5 item 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NumberLink {
    /// The page number linked.
    pub number: PageAddress,
    /// Where it leads.
    pub destination: LinkDestination,
    /// The line of the file.
    pub line: u32,
}

/// An `LT` line: the run of `length` characters at `row` and `column` is
/// a link to `destination` (concept section 5 item 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextLink {
    /// The row, 1 to 23.
    pub row: u8,
    /// The first column.
    pub column: u8,
    /// The characters linked.
    pub length: u8,
    /// Where it leads.
    pub destination: LinkDestination,
    /// The line of the file.
    pub line: u32,
}

/// The field types of the Form Extension (Appendix C.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FieldKind {
    /// TEXT.
    Text,
    /// EMAIL.
    Email,
    /// TEL.
    Telephone,
    /// NUMBER.
    Number,
    /// DATE.
    Date,
    /// TIME.
    Time,
    /// SELECT.
    Select,
    /// RADIO.
    Radio,
    /// CHECKBOX.
    Checkbox,
    /// TEXTAREA.
    TextArea,
    /// SUBMIT.
    Submit,
    /// RESET.
    Reset,
}

impl FieldKind {
    /// Every kind with its keyword.
    pub const KEYWORDS: [(Self, &'static [u8]); 12] = [
        (Self::Text, b"TEXT"),
        (Self::Email, b"EMAIL"),
        (Self::Telephone, b"TEL"),
        (Self::Number, b"NUMBER"),
        (Self::Date, b"DATE"),
        (Self::Time, b"TIME"),
        (Self::Select, b"SELECT"),
        (Self::Radio, b"RADIO"),
        (Self::Checkbox, b"CHECKBOX"),
        (Self::TextArea, b"TEXTAREA"),
        (Self::Submit, b"SUBMIT"),
        (Self::Reset, b"RESET"),
    ];

    /// The kind named by a keyword, ignoring case.
    #[must_use]
    pub fn from_keyword(keyword: &[u8]) -> Option<Self> {
        Self::KEYWORDS.iter().find(|(_, name)| name.eq_ignore_ascii_case(keyword)).map(|&(kind, _)| kind)
    }

    /// The keyword written in `FD` lines.
    #[must_use]
    pub fn keyword(self) -> &'static [u8] {
        Self::KEYWORDS.iter().find(|(kind, _)| *kind == self).map_or(&[], |&(_, name)| name)
    }

    /// True for the kinds that take `OPTION` lines.
    #[must_use]
    pub const fn has_options(self) -> bool {
        matches!(self, Self::Select | Self::Radio)
    }
}

/// One field of a form (Appendix C.2 and C.3). Text members that a kind does
/// not use are empty.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FormField {
    /// The type.
    pub kind: FieldKind,
    /// The name submitted.
    pub name: Text<FIELD_NAME_BYTES>,
    /// The row of the most recent `OL` line.
    pub row: u8,
    /// The first column.
    pub column: u8,
    /// Columns occupied.
    pub width: u8,
    /// Rows occupied: TEXTAREA's `rows`, otherwise 1.
    pub rows: u8,
    /// `required` was `Y`.
    pub required: bool,
    /// `maxlen`, where given.
    pub max_length: Option<u16>,
    /// TEXT and TEL `pattern`.
    pub pattern: Text<FIELD_TEXT_BYTES>,
    /// The default value of the text-like kinds.
    pub default: Text<FIELD_TEXT_BYTES>,
    /// NUMBER, DATE and TIME minimum.
    pub minimum: Text<FIELD_LIMIT_BYTES>,
    /// NUMBER, DATE and TIME maximum.
    pub maximum: Text<FIELD_LIMIT_BYTES>,
    /// NUMBER step.
    pub step: Text<FIELD_LIMIT_BYTES>,
    /// CHECKBOX value.
    pub value: Text<FIELD_TEXT_BYTES>,
    /// CHECKBOX `checked` was `Y`.
    pub checked: bool,
    /// SUBMIT and RESET label.
    pub label: Text<FIELD_TEXT_BYTES>,
    /// SUBMIT action, sent as `ttx_action`.
    pub action: Text<FIELD_NAME_BYTES>,
    /// The line of the file.
    pub line: u32,
}

impl FormField {
    /// An empty field of a kind.
    #[must_use]
    pub const fn new(kind: FieldKind) -> Self {
        Self {
            kind,
            name: Text::new(),
            row: 0,
            column: 0,
            width: 0,
            rows: 1,
            required: false,
            max_length: None,
            pattern: Text::new(),
            default: Text::new(),
            minimum: Text::new(),
            maximum: Text::new(),
            step: Text::new(),
            value: Text::new(),
            checked: false,
            label: Text::new(),
            action: Text::new(),
            line: 0,
        }
    }
}

/// One `OPTION` line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FormOption {
    /// Index of its field in [`Form::fields`].
    pub field: u8,
    /// The value submitted.
    pub value: Text<FIELD_NAME_BYTES>,
    /// The text shown by SELECT.
    pub display: Text<FIELD_TEXT_BYTES>,
    /// `selected` was `Y`.
    pub selected: bool,
    /// RADIO marker column for the second and later choices.
    pub column: Option<u8>,
    /// The line of the file.
    pub line: u32,
}

/// The HTTP method of a form submission.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FormMethod {
    /// POST.
    Post,
    /// GET.
    Get,
}

/// A form: the declarative description of Appendix C. The receiver runs it;
/// this crate only reads it.
#[derive(Clone, Copy, Debug)]
pub struct Form {
    /// The fields in file order.
    pub fields: FixedVec<FormField, FORM_FIELDS>,
    /// The options in file order.
    pub options: FixedVec<FormOption, FORM_OPTIONS>,
    /// `FS`: method and address.
    pub submission: Option<(FormMethod, Text<FORM_ADDRESS_BYTES>)>,
    /// `FP`: the success and error pages.
    pub result_pages: Option<(PageAddress, PageAddress)>,
    /// `FT`: the token.
    pub token: Text<FORM_TOKEN_BYTES>,
    /// True when the form may be made interactive: it has fields and no
    /// form problem was found (Appendix C.2 and C.6).
    pub interactive: bool,
}

impl Form {
    /// No form.
    #[must_use]
    pub const fn new() -> Self {
        let option =
            FormOption { field: 0, value: Text::new(), display: Text::new(), selected: false, column: None, line: 0 };
        Self {
            fields: FixedVec::new(FormField::new(FieldKind::Text)),
            options: FixedVec::new(option),
            submission: None,
            result_pages: None,
            token: Text::new(),
            interactive: false,
        }
    }

    /// True when the page has any form command.
    #[must_use]
    pub fn is_present(&self) -> bool {
        !self.fields.is_empty() || self.submission.is_some()
    }
}

// `FixedVec`'s derived equality also compares its unused slots, which hold
// whatever an earlier page left; pages compare by what they hold.
impl PartialEq for Form {
    fn eq(&self, other: &Self) -> bool {
        self.fields.as_slice() == other.fields.as_slice()
            && self.options.as_slice() == other.options.as_slice()
            && self.submission == other.submission
            && self.result_pages == other.result_pages
            && self.token == other.token
            && self.interactive == other.interactive
    }
}

impl Eq for Form {}

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}

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
    /// The withdrawn `FH` command (Appendix C.2).
    WithdrawnCommand,
    /// A version 2.0 field definition, with no column or width
    /// (Appendix C.6).
    FormNeedsConversion,
    /// Fields that overlap (Appendix C.2).
    FieldOverlap,
    /// A field that does not fit within its row or the page.
    FieldOutsideRow,
    /// A field definition before any `OL` line.
    FieldWithoutRow,
    /// A field definition that could not be read.
    BadField,
    /// An `OPTION` line with no SELECT or RADIO field before it.
    OptionWithoutField,
    /// A `TF` line that is malformed or repeated (REQ-FNT-007).
    BadFontLine,
}

impl IssueKind {
    /// True for the problems that stop a form being interactive
    /// (Appendix C.2 and C.6).
    #[must_use]
    pub const fn affects_form(self) -> bool {
        matches!(
            self,
            Self::FormNeedsConversion
                | Self::FieldOverlap
                | Self::FieldOutsideRow
                | Self::FieldWithoutRow
                | Self::BadField
        )
    }
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

/// A problem and the line it was found on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct PageIssue {
    /// What was wrong.
    pub kind: IssueKind,
    /// The line of the file.
    pub line: u32,
}

/// The page function and page coding of `PF` or X/28/0 (Tables 3 and 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct PageFunction {
    /// Page function, 4 bits.
    pub function: u8,
    /// Page coding, 3 bits.
    pub coding: u8,
}

/// One sub-page read from a page file.
#[derive(Clone, Copy, Debug)]
pub struct PageFile {
    /// Magazine and page number.
    pub address: PageAddress,
    /// The sub-page number of `PN`.
    pub subpage: u8,
    /// The sub-code.
    pub subcode: SubCode,
    /// The page status word in TTI `PS` form; see [`PageFile::control`].
    pub status: u16,
    /// `RE`: bits 3 to 6 of the default character set designation.
    pub region: Option<u8>,
    /// `PF`: page function and coding.
    pub function: Option<PageFunction>,
    /// `DE`.
    pub description: Text<DESCRIPTION_BYTES>,
    /// `DS`.
    pub source: Text<DESCRIPTION_BYTES>,
    /// Rows 0 to 25 as 7-bit codes; row 0 is the header row.
    pub rows: [[u8; COLUMNS]; STORED_ROWS],
    /// Bit r set when row r was given.
    pub rows_present: u32,
    /// Packets X/26, at most one per designation code.
    pub enhancements: FixedVec<EnhancementPacket, DESIGNATION_CODES>,
    /// Other packed enhancement lines.
    pub raw_packets: FixedVec<RawPacket, RAW_PACKETS>,
    /// X/27/0 links from `FL` or `OL,27`; `None` for a null link.
    pub fastext: [Option<FastextLink>; FASTEXT_LINKS],
    /// The X/27/0 "display row 24" flag.
    pub fastext_row_24: bool,
    /// `LN` lines.
    pub links: FixedVec<LinkLine, LINK_LINES>,
    /// False when the `LN` lines broke Appendix E.2 rule 1, so the page has
    /// no links.
    pub links_valid: bool,
    /// `LK` lines.
    pub number_links: FixedVec<NumberLink, NUMBER_LINKS>,
    /// `LT` lines.
    pub text_links: FixedVec<TextLink, TEXT_LINKS>,
    /// The form.
    pub form: Form,
    /// Problems found while reading.
    pub issues: FixedVec<PageIssue, PAGE_ISSUES>,
    /// Bit w set when warning w was marked `CK,considered`.
    pub considered: u16,
    /// The font the page names (REQ-FNT-007).
    pub font: PageFont,
    /// The line of `PN`, or 0.
    pub line: u32,
}

/// The default page address of files that carry none.
pub(crate) const DEFAULT_ADDRESS: PageAddress = PageAddress {
    magazine: Magazine::from_bits((DEFAULT_INITIAL_PAGE >> 8) as u8),
    page: PageNumber(DEFAULT_INITIAL_PAGE as u8),
};

impl PageFile {
    /// An empty page at the default address 100, all rows absent.
    #[must_use]
    pub const fn new() -> Self {
        let triplets = [Triplet { address: 0, mode: 0, data: 0 }; TRIPLETS_PER_PACKET];
        let link = LinkLine {
            key: LinkKey::Index,
            service: Text::new(),
            address: DEFAULT_ADDRESS,
            subcode: None,
            url: Text::new(),
            label: Text::new(),
            label_truncated: false,
            line: 0,
        };
        let destination =
            LinkDestination { service: Text::new(), address: DEFAULT_ADDRESS, subcode: None, url: Text::new() };
        Self {
            address: DEFAULT_ADDRESS,
            subpage: 0,
            subcode: SubCode::new(0),
            status: 0,
            region: None,
            function: None,
            description: Text::new(),
            source: Text::new(),
            rows: [[SPACE; COLUMNS]; STORED_ROWS],
            rows_present: 0,
            enhancements: FixedVec::new(EnhancementPacket { designation: 0, triplets }),
            raw_packets: FixedVec::new(RawPacket { number: 0, designation: 0, body: RawBody::Triplets(triplets) }),
            fastext: [None; FASTEXT_LINKS],
            fastext_row_24: false,
            links: FixedVec::new(link),
            links_valid: true,
            number_links: FixedVec::new(NumberLink { number: DEFAULT_ADDRESS, destination, line: 0 }),
            text_links: FixedVec::new(TextLink { row: 0, column: 0, length: 0, destination, line: 0 }),
            form: Form::new(),
            issues: FixedVec::new(PageIssue { kind: IssueKind::BadField, line: 0 }),
            considered: 0,
            font: PageFont::None,
            line: 0,
        }
    }

    /// Returns the page to the empty state of [`PageFile::new`] without
    /// building a new value on the stack.
    pub fn clear(&mut self) {
        self.address = DEFAULT_ADDRESS;
        self.subpage = 0;
        self.subcode = SubCode::new(0);
        self.status = 0;
        self.region = None;
        self.function = None;
        self.description.clear();
        self.source.clear();
        self.rows = [[SPACE; COLUMNS]; STORED_ROWS];
        self.rows_present = 0;
        self.enhancements.clear();
        self.raw_packets.clear();
        self.fastext = [None; FASTEXT_LINKS];
        self.fastext_row_24 = false;
        self.links.clear();
        self.links_valid = true;
        self.form.fields.clear();
        self.form.options.clear();
        self.form.submission = None;
        self.form.result_pages = None;
        self.form.token.clear();
        self.form.interactive = false;
        self.issues.clear();
        self.considered = 0;
        self.font = PageFont::None;
        self.line = 0;
    }

    /// The control bits C4 to C14 of the page status word (decision E.21):
    /// 0x4000 is C4, bits 0 to 6 are C5 to C11 and bits 7 to 9 are C12 to
    /// C14.
    #[must_use]
    pub fn control(&self) -> ControlBits {
        let national = u16::from(NATIONAL_INDEX_MASK);
        let mut bits = 0u16;
        if self.status & TTI_PS_C4 != 0 {
            bits |= C4_ERASE;
        }
        bits |= (self.status & TTI_PS_C5_TO_C11) << TTI_PS_C5_SHIFT;
        bits |= ((self.status >> TTI_PS_C12_BIT) & national) << NATIONAL_SHIFT;
        ControlBits(bits & CONTROL_MASK)
    }

    /// Sets the control bits, keeping the status bits that are not control
    /// bits, such as 0x8000 "transmit page".
    pub fn set_control(&mut self, control: ControlBits) {
        let national = u16::from(NATIONAL_INDEX_MASK);
        let mask = TTI_PS_C4 | TTI_PS_C5_TO_C11 | (national << TTI_PS_C12_BIT);
        let mut status = self.status & !mask;
        if control.0 & C4_ERASE != 0 {
            status |= TTI_PS_C4;
        }
        status |= (control.0 >> TTI_PS_C5_SHIFT) & TTI_PS_C5_TO_C11;
        status |= ((control.0 >> NATIONAL_SHIFT) & national) << TTI_PS_C12_BIT;
        self.status = status;
    }

    /// Sets C12 to C14 so that the national option index 4×C12 + 2×C13 +
    /// C14 is `index` (decision E.21).
    pub fn set_national_index(&mut self, index: u8) {
        let mut bits = self.control().0 & !(C12 | C13 | C14);
        for (bit, weight) in [(C12, 0b100), (C13, 0b010), (C14, 0b001)] {
            if index & weight != 0 {
                bits |= bit;
            }
        }
        self.set_control(ControlBits(bits));
    }

    /// True when row `row` was given.
    #[must_use]
    pub fn has_row(&self, row: usize) -> bool {
        u32::try_from(row).ok().and_then(|row| self.rows_present.checked_shr(row)).is_some_and(|bits| bits & 1 == 1)
    }

    /// Row `row`'s codes, whether or not it was given (absent rows are
    /// spaces).
    #[must_use]
    pub fn row(&self, row: usize) -> Option<&[u8; COLUMNS]> {
        self.rows.get(row)
    }

    /// Sets row `row` from 7-bit codes, padding with spaces.
    pub fn set_row(&mut self, row: usize, codes: &[u8]) {
        if let Some(target) = self.rows.get_mut(row) {
            *target = [SPACE; COLUMNS];
            for (slot, &code) in target.iter_mut().zip(codes) {
                *slot = code;
            }
            if let Some(bit) = u32::try_from(row).ok().and_then(|row| 1u32.checked_shl(row)) {
                self.rows_present |= bit;
            }
        }
    }

    /// Records a problem. Returns false when the issue list was full.
    pub fn add_issue(&mut self, kind: IssueKind, line: u32) -> bool {
        self.issues.push(PageIssue { kind, line })
    }

    /// True when a problem of this kind was recorded.
    #[must_use]
    pub fn has_issue(&self, kind: IssueKind) -> bool {
        self.issues.as_slice().iter().any(|issue| issue.kind == kind)
    }

    /// True when the page's links come from `LN` lines (Appendix E.2 rule 3:
    /// `LN` wins over `FL`).
    #[must_use]
    pub fn uses_link_lines(&self) -> bool {
        !self.links.is_empty()
    }
}

// As for `Form`: compare what the page holds, not unused storage.
impl PartialEq for PageFile {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
            && self.subpage == other.subpage
            && self.subcode == other.subcode
            && self.status == other.status
            && self.region == other.region
            && self.function == other.function
            && self.description == other.description
            && self.source == other.source
            && self.rows == other.rows
            && self.rows_present == other.rows_present
            && self.enhancements.as_slice() == other.enhancements.as_slice()
            && self.raw_packets.as_slice() == other.raw_packets.as_slice()
            && self.fastext == other.fastext
            && self.fastext_row_24 == other.fastext_row_24
            && self.links.as_slice() == other.links.as_slice()
            && self.links_valid == other.links_valid
            && self.number_links.as_slice() == other.number_links.as_slice()
            && self.text_links.as_slice() == other.text_links.as_slice()
            && self.form == other.form
            && self.issues.as_slice() == other.issues.as_slice()
            && self.considered == other.considered
            && self.line == other.line
    }
}

impl Eq for PageFile {}

impl Default for PageFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use neoteletext_core::consts::{C5_NEWSFLASH, C8_UPDATE, C11_SERIAL, C12, C14};

    #[test]
    fn status_maps_to_control_bits_as_e21() {
        let mut page = PageFile::new();
        page.status = 0x8000 | 0x4000 | 0x0001 | 0x0040 | 0x0080;
        let control = page.control();
        assert_eq!(control.0, C4_ERASE | C5_NEWSFLASH | C11_SERIAL | C12);
        assert_eq!(control.national_index(), 4);
        page.set_control(ControlBits(C8_UPDATE | C14));
        assert_eq!(page.status, 0x8000 | 0x0008 | 0x0200);
        for bit in 4..=14 {
            page.set_control(ControlBits(1 << bit));
            assert_eq!(page.control(), ControlBits(1 << bit), "C{bit}");
        }
    }

    #[test]
    fn rows_are_tracked() {
        let mut page = PageFile::new();
        assert!(!page.has_row(3));
        page.set_row(3, b"AB");
        assert!(page.has_row(3));
        assert_eq!(&page.row(3).unwrap()[..3], b"AB ");
        page.clear();
        assert!(!page.has_row(3));
        assert_eq!(page.address.mpp(), 0x100);
    }
}
