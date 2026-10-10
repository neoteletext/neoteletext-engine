//! The enhancement processor of Levels 2.5 and 3.5 (area L25, layer L3;
//! decisions E.40 to E.54; REQ-FSM-011 and REQ-FSM-012).
//!
//! [`apply`] runs after the Level 1 grid of a page has been built, in place
//! of the Level 1.5 pass, when the presentation level is 2.5 or 3.5
//! (REQ-L25-001, REQ-PRF-006). It:
//!
//! - finds the enhancement pages the page uses (`objects::links`) and
//!   builds the page-level presentation data with `colour::resolve`: the
//!   Colour Map, the default screen and row colours, re-mapping,
//!   substitution and side panels (REQ-L25-030 to REQ-L25-041);
//! - walks the page's local enhancement data, the objects it invokes and
//!   the default objects, as the machine of REQ-FSM-011 describes;
//! - resolves each row from its Level 1 bytes and the events of the walk
//!   that fall on it, cell by cell, as REQ-FSM-012 describes, and then the
//!   cells of each side panel;
//! - puts into the grid the DRCS patterns and colour tables that `drcs`
//!   decodes for the characters the cells use (REQ-DRC-007).
//!
//! **How the walk and the rows meet.** The walk is run once for the
//! page-level data (screen and row colours) and then once for each row,
//! keeping only the events that fall on that row. Each run is a pure
//! function of the stored data, so every run makes the same decisions,
//! including those of the limits of Annex A.8, and a row needs only one
//! row of event storage (decision E.41, principle 5: no allocation). The
//! work is bounded by the limits: at most 127 objects of each type of at
//! most 507 triplets, each scanned once before it runs, for each of the 26
//! runs ([`MAX_WALK_STEPS`], REQ-OBJ-020, REQ-TST-019).
//!
//! It never writes row 0, columns 0 to 7 (REQ-L25-050), and it reaches
//! stored enhancement pages only through `acq::EnhancementLookup`
//! (REQ-ACQ-020).

use crate::acq::{EnhancementFaults, EnhancementKey, EnhancementLookup, MagazineData, PageBuild, StoredPage};
use crate::charset::{self, CharRef, CharSet, G0, G0Set, G2Set, mosaic_pattern};
use crate::colour::{self, ColourIndex, ColourMap, PagePresentation, PresentationSources, Remapping, SidePanels};
use crate::config::{Config, PresentationLevel};
use crate::consts::{
    ALPHA_BLACK, ALPHA_WHITE, ATTRIBUTE_LAST, BLACK_BACKGROUND, C5_NEWSFLASH, C6_SUBTITLE, C7_SUPPRESS_HEADER,
    C10_INHIBIT_DISPLAY, CLUT_ENTRY_MASK, COLOUR_BLACK, COLOUR_WHITE, COLUMNS, CONCEAL, CONTIGUOUS_MOSAICS,
    DOUBLE_HEIGHT, DOUBLE_HEIGHT_FIRST_ROW, DOUBLE_HEIGHT_LAST_ROW, DOUBLE_SIZE, DOUBLE_WIDTH, DRCS_LEVEL_MASK,
    END_BOX, ESC, FLASH, FLASH_MODE_ADJACENT_CLUT, FLASH_MODE_INVERTED, FLASH_MODE_NORMAL, FLASH_RATE_DECREMENTAL,
    FLASH_RATE_INCREMENTAL, FLASH_RATE_PHASE_1, FLASH_RATE_PHASE_2, FLASH_RATE_PHASE_3, FLASH_RATE_SLOW,
    G1_G0_COLUMNS_FIRST, G1_G0_COLUMNS_LAST, G3_UNDEFINED, HOLD_MOSAICS, LATIN_AT_SIGN, LATIN_G2_UNDEFINED,
    LEVEL_BIT_25, LEVEL_BIT_35, LOCAL_TRIPLETS, MOSAIC_BIT5, MOSAIC_BLACK, MOSAIC_WHITE, NEW_BACKGROUND, NORMAL_SIZE,
    OBJECT_BODY_TRIPLETS, OBJECT_INVOCATIONS_PER_TYPE, OBJECT_TYPES, OBJECTS_OPEN_AT_ONCE,
    OBJECTS_OPENED_IN_SUCCESSION, PAGE_FUNCTION_DRCS, PAGE_FUNCTION_GDRCS, PAGE_FUNCTION_GPOP, PAGE_FUNCTION_MOT,
    PAGE_FUNCTION_POP, PAGE_MOT, RECEIVER_COLUMNS, RELEASE_MOSAICS, ROW_ADDRESS_LAST, ROW_COLOUR_DOWN_TO_BOTTOM,
    ROW_COLOUR_THIS_ROW, ROWS, SCREEN_COLUMNS, SEPARATED_MOSAICS, SIDE_PANEL_FIRST_ROW, SIDE_PANEL_ROWS, SPACE,
    START_BOX, STEADY, TRIPLET_AT_SIGN_CODE, TRIPLET_FIRST_CHARACTER, TRIPLETS_PER_PACKET, WALK_DEPTH,
};
use crate::drcs::{DrcsColourTables, DrcsFault, decode_character};
use crate::level1::{
    Cell, CellAttributes, CharSource, DisplayOptions, FlashMode, FlashRate, FontStyle, Glyph, Grid, Size, Width,
    resolve_designation, row_24_shown, row_codes,
};
use crate::objects::{self, Body, EnhancementLinks, Fetch, ObjectRef, row_of};
use crate::packet::{
    ColourData, DisplayAttributes, FontStyleData, ObjectKind, ObjectNumber, ObjectSource, TripletFunction,
};
use crate::types::{DrcsCharacter, DrcsTable, FixedVec};

/// What one build of a grid found, for the decoder's statistics and the
/// trace (REQ-L25-051, REQ-ARCH-007, REQ-TST-019). Fault counts are of the
/// page-level run of the walk and of the row resolution; `steps` counts
/// every triplet read in every run. Pass `build` and `faults` to
/// `Decoder::record_enhancement` once the grid is built.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnhancementReport {
    /// The page, reception and level the grid was built from.
    pub build: PageBuild,
    /// The level the page was processed at: Level 2.5 when a Level 3.5
    /// decoder falls back to the table's Level 2.5 links (clause 10.6.1).
    pub level: PresentationLevel,
    /// The faults met, one count for each kind.
    pub faults: EnhancementFaults,
    /// Objects run.
    pub objects: u32,
    /// Triplets read, over every run of the walk.
    pub steps: u32,
}

impl EnhancementReport {
    const fn new(build: PageBuild, level: PresentationLevel) -> Self {
        Self { build, level, faults: EnhancementFaults::NONE, objects: 0, steps: 0 }
    }
}

/// The runs of the walk for one grid: the page-level run, then one for each
/// row.
const WALK_RUNS: u32 = 1 + ROWS as u32;

/// The most triplets one run can read: the local enhancement data, and for
/// each object the limits allow, a scan and a run of its whole body.
const MAX_RUN_STEPS: u32 =
    LOCAL_TRIPLETS as u32 + OBJECT_TYPES as u32 * OBJECT_INVOCATIONS_PER_TYPE as u32 * 2 * OBJECT_BODY_TRIPLETS as u32;

/// The bound on the triplets read to build one grid (REQ-OBJ-020,
/// REQ-SEC-012, REQ-TST-019).
pub const MAX_WALK_STEPS: u32 = WALK_RUNS * MAX_RUN_STEPS;

// ---------------------------------------------------------------------------
// Attributes
// ---------------------------------------------------------------------------

/// The rate part of a flash attribute, before incremental and decremental
/// flash are resolved into a phase for each cell (decision E.48).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Rate {
    /// One fixed rate and phase.
    Fixed(FlashRate),
    /// Phases 1, 2, 3, 1, ... rightwards from the column it was set at.
    Incremental,
    /// Phases 1, 3, 2, 1, ... rightwards from the column it was set at.
    Decremental,
}

/// The flash attribute type (Annex A.7.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Flash {
    /// No flash.
    Steady,
    /// Flashing.
    On {
        /// How.
        mode: FlashMode,
        /// The rate and phase.
        rate: Rate,
        /// The column it was set at, for incremental and decremental
        /// phases.
        start: u8,
    },
}

/// Level 1 Flash at Levels 2.5 and 3.5: slow, normal mode (decision E.48).
const LEVEL_1_FLASH: Flash = Flash::On { mode: FlashMode::Normal, rate: Rate::Fixed(FlashRate::Slow), start: 0 };

/// The attributes a cell is drawn with (REQ-L25-006), each attribute type
/// of decision E.47 a field.
// Each flag is one attribute type of Annex A.7.1 and decision E.47.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Attrs {
    foreground: ColourIndex,
    background: ColourIndex,
    flash: Flash,
    conceal: bool,
    boxed: bool,
    double_height: bool,
    double_width: bool,
    /// The non-spacing underline or separated mosaics attribute.
    underline: bool,
    invert: bool,
    font: FontStyle,
}

impl Attrs {
    /// The start of every Passive object: white (CLUT 0 entry 7), black
    /// (CLUT 0 entry 0), normal size, steady, boxing off, not concealed,
    /// contiguous mosaics (clause 13.5, REQ-OBJ-011).
    const PASSIVE: Self = Self {
        foreground: COLOUR_WHITE,
        background: COLOUR_BLACK,
        flash: Flash::Steady,
        conceal: false,
        boxed: false,
        double_height: false,
        double_width: false,
        underline: false,
        invert: false,
        font: FontStyle { bold: false, italic: false, proportional: false },
    };

    /// Applies the attributes a change sets.
    fn apply(&mut self, change: &AttrChange) {
        if let Some(foreground) = change.foreground {
            self.foreground = foreground;
        }
        if let Some(background) = change.background {
            self.background = background;
        }
        if let Some(flash) = change.flash {
            self.flash = flash;
        }
        if let Some(display) = change.display {
            self.double_height = display.double_height;
            self.double_width = display.double_width;
            self.boxed = display.boxing;
            self.conceal = display.conceal;
            self.invert = display.invert;
            self.underline = display.underline;
        }
        if let Some(font) = change.font {
            self.font = font;
        }
    }
}

/// Non-spacing attributes placed at one cell, each attribute type the
/// latest placed there (Annex C.2 rule 3), and the Modified G0 and G2
/// designation of local enhancement data and Active objects.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct AttrChange {
    foreground: Option<ColourIndex>,
    background: Option<ColourIndex>,
    flash: Option<Flash>,
    /// The six display attributes, which one triplet sets together; each
    /// is still its own type (decision E.47), and a 0 bit cancels.
    display: Option<DisplayAttributes>,
    font: Option<FontStyle>,
    designation: Option<(G0Set, G2Set)>,
}

impl AttrChange {
    /// This change followed by a later one at the same cell.
    fn then(self, later: Self) -> Self {
        Self {
            foreground: later.foreground.or(self.foreground),
            background: later.background.or(self.background),
            flash: later.flash.or(self.flash),
            display: later.display.or(self.display),
            font: later.font.or(self.font),
            designation: later.designation.or(self.designation),
        }
    }
}

/// The flash an Additional Flash Functions triplet sets (Table 29 mode
/// 00111, decision E.48); `None` for its reserved values, which only set the
/// column (REQ-L25-027).
fn flash_of(mode: u8, rate: u8, reserved: u8, column: u8) -> Option<Flash> {
    if reserved != 0 {
        return None;
    }
    let rate = match rate {
        FLASH_RATE_SLOW => Rate::Fixed(FlashRate::Slow),
        FLASH_RATE_PHASE_1 => Rate::Fixed(FlashRate::FastPhase1),
        FLASH_RATE_PHASE_2 => Rate::Fixed(FlashRate::FastPhase2),
        FLASH_RATE_PHASE_3 => Rate::Fixed(FlashRate::FastPhase3),
        FLASH_RATE_INCREMENTAL => Rate::Incremental,
        FLASH_RATE_DECREMENTAL => Rate::Decremental,
        _ => return None,
    };
    let mode = match mode {
        FLASH_MODE_NORMAL => FlashMode::Normal,
        FLASH_MODE_INVERTED => FlashMode::Inverted,
        FLASH_MODE_ADJACENT_CLUT => FlashMode::AdjacentClut,
        _ => return Some(Flash::Steady),
    };
    Some(Flash::On { mode, rate, start: column })
}

/// The phase of the cell in column `column` (decision E.48): successive
/// characters rightwards from the triplet's column take phases 1, 2, 3 for
/// incremental flash and 1, 3, 2 for decremental.
fn flash_rate(rate: Rate, start: u8, column: usize) -> FlashRate {
    const INCREMENTAL: [FlashRate; 3] = [FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3];
    const DECREMENTAL: [FlashRate; 3] = [FlashRate::FastPhase1, FlashRate::FastPhase3, FlashRate::FastPhase2];
    let step = column.saturating_sub(usize::from(start)).checked_rem(INCREMENTAL.len()).unwrap_or(0);
    match rate {
        Rate::Fixed(rate) => rate,
        Rate::Incremental => INCREMENTAL.get(step).copied().unwrap_or(FlashRate::FastPhase1),
        Rate::Decremental => DECREMENTAL.get(step).copied().unwrap_or(FlashRate::FastPhase1),
    }
}

// ---------------------------------------------------------------------------
// Characters
// ---------------------------------------------------------------------------

/// A character written by enhancement data (REQ-L25-010), before its
/// character set is resolved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EnhChar {
    /// A G0 character with a diacritical mark (0 for none); `at` for the
    /// "@" of mode 10000 with data 2/A.
    G0 { code: u8, diacritic: u8, at: bool },
    /// A G1 character: a block mosaic, or in columns 4 and 5 the current G0
    /// set's character (Table 47 note 3).
    G1(u8),
    /// A G2 character.
    G2(u8),
    /// A G3 character.
    G3(u8),
    /// A DRCS character.
    Drcs(DrcsCharacter),
}

/// What an enhancement character shows (REQ-L25-010, REQ-L25-011): national
/// options never apply, G1 mosaics are separated by the non-spacing
/// attribute only, and undefined positions show a space (decision E.50).
/// The flag is true for an undefined position.
fn enhancement_glyph(character: EnhChar, sets: (G0Set, G2Set), separated: bool) -> (Glyph, bool) {
    let g0 = CharSet::G0(G0 { set: sets.0, national: None });
    let glyph = match character {
        EnhChar::G0 { at: true, .. } => Glyph::Char(CharRef::new(g0, LATIN_AT_SIGN)),
        EnhChar::G0 { code, diacritic, .. } => {
            let mut character = CharRef::new(g0, code);
            character.diacritic = diacritic;
            Glyph::Char(character)
        }
        EnhChar::G1(code) if (G1_G0_COLUMNS_FIRST..=G1_G0_COLUMNS_LAST).contains(&code) => {
            Glyph::Char(CharRef::new(g0, code))
        }
        EnhChar::G1(code) => Glyph::Mosaic { pattern: mosaic_pattern(code), separated },
        EnhChar::G2(code) if sets.1 == G2Set::Latin && LATIN_G2_UNDEFINED.contains(&code) => {
            return (Glyph::Space, true);
        }
        EnhChar::G2(code) => Glyph::Char(CharRef::new(CharSet::G2(sets.1), code)),
        EnhChar::G3(code) if G3_UNDEFINED.contains(&code) => return (Glyph::Space, true),
        EnhChar::G3(code) => Glyph::Char(CharRef::new(CharSet::G3, code)),
        EnhChar::Drcs(character) => Glyph::Char(CharRef::drcs(character)),
    };
    (glyph, false)
}

/// True for a G0 or G2 character, the only ones that take underline and
/// the Level 3.5 font styles (decisions E.47 and E.50).
const fn is_text(glyph: Glyph) -> bool {
    matches!(glyph, Glyph::Char(CharRef { set: CharSet::G0(_) | CharSet::G2(_), .. }))
}

// ---------------------------------------------------------------------------
// The walk (REQ-FSM-011)
// ---------------------------------------------------------------------------

/// The display planes of clause 13.2, which are also the sources of the
/// walk.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Plane {
    /// The page's local enhancement data.
    Local,
    /// An Active object.
    Active,
    /// An Adaptive object.
    Adaptive,
    /// A Passive object.
    Passive,
}

impl Plane {
    const fn of(kind: ObjectKind) -> Self {
        match kind {
            ObjectKind::Active => Self::Active,
            ObjectKind::Adaptive => Self::Adaptive,
            ObjectKind::Passive => Self::Passive,
        }
    }

    /// REQ-OBJ-008: local data invokes anything; Active objects Adaptive
    /// and Passive ones; Adaptive objects Passive ones; Passive nothing.
    const fn may_invoke(self, kind: ObjectKind) -> bool {
        match self {
            Self::Local => true,
            Self::Active => matches!(kind, ObjectKind::Adaptive | ObjectKind::Passive),
            Self::Adaptive => matches!(kind, ObjectKind::Passive),
            Self::Passive => false,
        }
    }

    /// REQ-OBJ-013: Full Screen Colour, Full Row Colour and Address Display
    /// Row 0 act only in local data and Active objects.
    const fn sets_colours(self) -> bool {
        matches!(self, Self::Local | Self::Active)
    }

    const fn source(self) -> CharSource {
        match self {
            Self::Local => CharSource::Local,
            Self::Active => CharSource::Active,
            Self::Adaptive => CharSource::Adaptive,
            Self::Passive => CharSource::Passive,
        }
    }
}

/// The type's index for the per-type limit.
const fn type_index(kind: ObjectKind) -> usize {
    match kind {
        ObjectKind::Active => 0,
        ObjectKind::Adaptive => 1,
        ObjectKind::Passive => 2,
    }
}

/// The display area of a column (clause 12.5.3, decision E.49).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Area {
    /// Columns 0 to 39.
    Main,
    /// The right panel, columns 40 to 40 + right width − 1.
    Right,
    /// The left panel, columns 72 − left width to 71.
    Left,
    /// Panel columns that are not enabled, and beyond column 71.
    Outside,
}

fn area_of(column: usize, panels: SidePanels) -> Area {
    if column < COLUMNS {
        Area::Main
    } else if column < COLUMNS.saturating_add(usize::from(panels.right)) {
        Area::Right
    } else if column >= panels.left_first_column() && column < SCREEN_COLUMNS {
        Area::Left
    } else {
        Area::Outside
    }
}

/// A position in scan order, for the open area of Annex A.8: row-major over
/// the 72 columns of Annex A.4.
fn scan_index(row: usize, column: usize) -> usize {
    row.saturating_mul(SCREEN_COLUMNS).saturating_add(column.min(SCREEN_COLUMNS))
}

/// A walk record of REQ-FSM-011: one for the local enhancement data and one
/// for each object being run.
#[derive(Clone, Copy, Debug)]
struct Frame<'s> {
    plane: Plane,
    body: Body<'s>,
    /// The next triplet, and one past the last.
    cursor: u16,
    end: u16,
    /// The local enhancement data skips missing designation codes; an
    /// object never meets one, because the scan found none.
    local_data: bool,
    /// The origin, absolute, `(row, column)`.
    origin: (usize, usize),
    /// The active position, relative to the origin.
    row: u8,
    column: u8,
    /// The chosen sub-tables: global, then normal (REQ-DRC-005).
    drcs: [u8; 2],
    /// The display area of the origin; elements elsewhere are discarded
    /// (REQ-OBJ-014, decision E.49).
    area: Area,
    /// The invocation's number, in walk order: later is latest invoked.
    slot: u16,
    /// The Modified G0 and G2 designation of an Adaptive or Passive object,
    /// which lasts to the end of the object (Annex C.4 rule 2).
    designation: Option<(G0Set, G2Set)>,
    /// A Passive object's own attributes (REQ-OBJ-011).
    passive: Attrs,
}

impl Frame<'_> {
    /// The local enhancement data at row 0, column 0 (REQ-L25-003).
    const fn local() -> Self {
        Self {
            plane: Plane::Local,
            body: Body::Local,
            cursor: 0,
            end: LOCAL_TRIPLETS,
            local_data: true,
            origin: (0, 0),
            row: 0,
            column: 0,
            drcs: [0; 2],
            area: Area::Main,
            slot: 0,
            designation: None,
            passive: Attrs::PASSIVE,
        }
    }

    /// The absolute active position.
    fn position(&self) -> (usize, usize) {
        (self.origin.0.saturating_add(usize::from(self.row)), self.origin.1.saturating_add(usize::from(self.column)))
    }
}

/// An Adaptive object's extent on the row being resolved (REQ-OBJ-010).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Range {
    slot: u16,
    first: u8,
    last: u8,
}

/// The ranges one row can hold: every Adaptive invocation the limits allow.
const RANGES: usize = OBJECT_INVOCATIONS_PER_TYPE as usize;

/// The objects whose open areas are kept: every invocation the limits
/// allow.
const OPEN_RECORDS: usize = OBJECT_TYPES * OBJECT_INVOCATIONS_PER_TYPE as usize;

/// A G0 and G2 pair of character sets.
type Sets = (G0Set, G2Set);

/// The events of the walk that fall on one cell of the row being resolved.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct Slot {
    /// The non-spacing attributes of the underlying plane: local data and
    /// Active objects (REQ-FSM-012 step 3).
    underlying: AttrChange,
    local: Option<EnhChar>,
    active: Option<EnhChar>,
    adaptive: Option<(EnhChar, Option<Sets>)>,
    /// The attributes Adaptive objects placed here, with the latest one's
    /// invocation: Adaptive objects shall not overlap (Annex A.7.1), so one
    /// set is kept.
    adaptive_change: AttrChange,
    adaptive_owner: Option<u16>,
    passive: Option<(EnhChar, Option<Sets>, Attrs)>,
}

/// Where the walk's events go: the page-level run keeps screen and row
/// colours and which rows have events; a row run keeps the events of one
/// row.
struct Collector {
    row: Option<usize>,
    last_row: usize,
    screen: Option<ColourIndex>,
    row_colours: [Option<ColourIndex>; ROWS],
    /// Bit r set when an event falls on row r: rows without events need no
    /// run of their own.
    event_rows: u32,
    slots: [Slot; SCREEN_COLUMNS],
    ranges: FixedVec<Range, RANGES>,
}

impl Collector {
    fn new(row: Option<usize>, last_row: usize) -> Self {
        Self {
            row,
            last_row,
            screen: None,
            row_colours: [None; ROWS],
            event_rows: 0,
            slots: [Slot::default(); SCREEN_COLUMNS],
            ranges: FixedVec::new(Range { slot: 0, first: 0, last: 0 }),
        }
    }

    /// The slot of a cell of the row being resolved.
    fn slot(&mut self, row: usize, column: usize) -> Option<&mut Slot> {
        if self.row != Some(row) {
            self.event_rows |= 1u32.checked_shl(row as u32).unwrap_or(0);
            return None;
        }
        self.slots.get_mut(column)
    }

    /// REQ-L25-033, decision E.45: commands apply in transmission order.
    fn row_colour(&mut self, row: usize, colour: ColourData) {
        if self.row.is_some() {
            return;
        }
        let last = match colour.high {
            ROW_COLOUR_THIS_ROW => row,
            ROW_COLOUR_DOWN_TO_BOTTOM => self.last_row,
            // Decision E.45: the reserved selections set no colour.
            _ => return,
        };
        for slot in self.row_colours.iter_mut().take(last.saturating_add(1)).skip(row) {
            *slot = Some(colour.index);
        }
    }

    /// An Adaptive object addresses a cell, for its range on the row.
    fn touch(&mut self, row: usize, column: usize, slot: u16) {
        if self.row != Some(row) {
            return;
        }
        let column = column as u8;
        if let Some(range) = self.ranges.as_mut_slice().iter_mut().find(|range| range.slot == slot) {
            range.first = range.first.min(column);
            range.last = range.last.max(column);
        } else {
            // There is a range for every Adaptive invocation the limits allow.
            let _ = self.ranges.push(Range { slot, first: column, last: column });
        }
    }
}

/// The inputs of every run of the walk.
struct Inputs<'a, 's> {
    page: &'a StoredPage,
    lookup: &'a EnhancementLookup<'s>,
    build: PageBuild,
    links: &'a EnhancementLinks,
    level: PresentationLevel,
    panels: SidePanels,
    /// The page's default G0 and G2 sets.
    default_sets: (G0Set, G2Set),
}

/// States of the enhancement walk (REQ-FSM-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalkState {
    /// Before the walk.
    Start,
    /// Taking the triplets of the current source.
    Walking,
    /// An Origin Modifier waits for the invocation that follows it.
    ModifierPending {
        /// The row offset, the modifier's address minus 40.
        row_offset: u8,
        /// The column offset, its data.
        column_offset: u8,
    },
    /// The current source has ended.
    Returning,
    /// Placing the default objects of REQ-OBJ-040.
    PlacingDefaults,
    /// The walk is over.
    Finished,
}

/// The source that ended, as the Returning state is told it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ended {
    /// The page's local enhancement data.
    Local,
    /// An object invoked by another source.
    Object,
    /// A default object.
    Default,
}

/// The events of the walk (REQ-FSM-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalkEvent {
    /// The walk begins; `display` when the page has a packet X/26 with a
    /// display triplet (decision E.53).
    Begin {
        /// The page's local data displays something.
        display: bool,
    },
    /// The next triplet of the current source; `None` when it failed its
    /// Hamming 24/18 check.
    Triplet(Option<TripletFunction>),
    /// The current source has no more data.
    EndOfData,
    /// A source ended.
    Returned(Ended),
    /// The next default object was looked for: `found` when it was found and
    /// accepted.
    DefaultObject {
        /// A default object is ready to run.
        found: bool,
    },
}

/// What a transition of the walk does (REQ-FSM-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalkAction {
    /// Nothing.
    None,
    /// The source becomes the local enhancement data at row 0, column 0,
    /// with DRCS sub-tables 0 and no Origin Modifier.
    BeginLocal,
    /// The source becomes the default object found, at row 0, column 0.
    StartDefault,
    /// Skip a damaged triplet; the position does not move (decision E.46).
    Skip,
    /// End the current source.
    EndSource,
    /// Pop the walk record: the invoker's position, origin and sub-tables
    /// return, and the Origin Modifier is cancelled (REQ-OBJ-002).
    Pop,
    /// After a default object, the position is row 0, column 0 again.
    ResetPosition,
    /// Full Screen Colour (REQ-L25-032).
    ScreenColour(ColourData),
    /// Full Row Colour or Address Display Row 0: the position becomes `row`,
    /// column 0, and the row colour is set (REQ-L25-033).
    RowColour {
        /// The row, relative to the source's origin.
        row: u8,
        /// The colour and row selection.
        colour: ColourData,
    },
    /// Set Active Position (REQ-L25-003).
    Position {
        /// The row.
        row: u8,
        /// The column, kept when 40 or more.
        column: u8,
    },
    /// DRCS Mode (REQ-DRC-005).
    DrcsMode {
        /// The normal table, or the global.
        normal: bool,
        /// The level bits.
        levels: u8,
        /// The sub-table.
        subtable: u8,
    },
    /// An Object Invocation with the offsets of an Origin Modifier that
    /// immediately preceded it, or none (REQ-OBJ-001, REQ-OBJ-002).
    Invoke {
        /// The source.
        source: ObjectSource,
        /// The type.
        kind: ObjectKind,
        /// The number.
        number: ObjectNumber,
        /// The row offset.
        row_offset: u8,
        /// The column offset.
        column_offset: u8,
    },
    /// A column triplet: its column, then its character or attribute
    /// (REQ-L25-004, REQ-L25-010, REQ-L25-020).
    Column(TripletFunction),
}

/// The transition of the walk for one triplet in the Walking state, and in
/// Modifier pending for anything but an invocation (REQ-FSM-011).
const fn triplet_transition(function: TripletFunction) -> (WalkState, WalkAction) {
    use TripletFunction as F;
    let walking = WalkState::Walking;
    match function {
        // Decision E.46: any Termination Marker, and an Object Definition,
        // end the source.
        F::Termination { .. } | F::ObjectDefinition { .. } => (WalkState::Returning, WalkAction::EndSource),
        F::ObjectInvocation { source, kind, number } => {
            (walking, WalkAction::Invoke { source, kind, number, row_offset: 0, column_offset: 0 })
        }
        F::OriginModifier { row_offset, column_offset } => {
            (WalkState::ModifierPending { row_offset, column_offset }, WalkAction::None)
        }
        F::FullScreenColour(colour) => (walking, WalkAction::ScreenColour(colour)),
        F::FullRowColour { address, colour } => (walking, WalkAction::RowColour { row: row_of(address), colour }),
        F::AddressDisplayRow0 { address, colour } => {
            if address == ROW_ADDRESS_LAST {
                (walking, WalkAction::RowColour { row: 0, colour })
            } else {
                // Addresses 40 to 62 are reserved: no move (REQ-L25-003).
                (walking, WalkAction::None)
            }
        }
        F::SetActivePosition { address, column } => (walking, WalkAction::Position { row: row_of(address), column }),
        F::DrcsMode { normal, levels, subtable } => (walking, WalkAction::DrcsMode { normal, levels, subtable }),
        // REQ-L25-052: Programme Delivery Control and reserved triplets do
        // nothing, and do not move the position.
        F::ProgrammeDeliveryRow { .. }
        | F::ReservedRow { .. }
        | F::ProgrammeDeliveryColumn { .. }
        | F::ReservedColumn { .. } => (walking, WalkAction::None),
        F::ForegroundColour { .. }
        | F::G1Character { .. }
        | F::G3Character { .. }
        | F::BackgroundColour { .. }
        | F::AdditionalFlash { .. }
        | F::ModifiedDesignation { .. }
        | F::G0Character { .. }
        | F::DisplayAttributes { .. }
        | F::DrcsCharacter { .. }
        | F::FontStyle { .. }
        | F::G2Character { .. }
        | F::G0Diacritic { .. } => (walking, WalkAction::Column(function)),
    }
}

/// The transition function of the enhancement walk (REQ-FSM-011,
/// REQ-FSM-001): the next state and the action for a state and an event.
/// Events that cannot arrive in a state leave it unchanged, so that no
/// input can drive the machine astray (REQ-FSM-008).
#[must_use]
pub const fn walk_transition(state: WalkState, event: WalkEvent) -> (WalkState, WalkAction) {
    use WalkEvent as E;
    use WalkState as S;
    match (state, event) {
        (S::Start, E::Begin { display: true }) => (S::Walking, WalkAction::BeginLocal),
        (S::Start, E::Begin { display: false }) => (S::PlacingDefaults, WalkAction::None),
        // Decision E.46: a damaged triplet is skipped; one after an Origin
        // Modifier is not the invocation it needs, so the offsets go.
        (S::Walking | S::ModifierPending { .. }, E::Triplet(None)) => (S::Walking, WalkAction::Skip),
        (
            S::ModifierPending { row_offset, column_offset },
            E::Triplet(Some(TripletFunction::ObjectInvocation { source, kind, number })),
        ) => (S::Walking, WalkAction::Invoke { source, kind, number, row_offset, column_offset }),
        (S::Walking | S::ModifierPending { .. }, E::Triplet(Some(function))) => triplet_transition(function),
        (S::Walking | S::ModifierPending { .. }, E::EndOfData) => (S::Returning, WalkAction::EndSource),
        (S::Returning, E::Returned(Ended::Object)) => (S::Walking, WalkAction::Pop),
        (S::Returning, E::Returned(Ended::Local)) | (S::PlacingDefaults, E::DefaultObject { found: false }) => {
            (S::Finished, WalkAction::None)
        }
        (S::Returning, E::Returned(Ended::Default)) => (S::PlacingDefaults, WalkAction::ResetPosition),
        (S::PlacingDefaults, E::DefaultObject { found: true }) => (S::Walking, WalkAction::StartDefault),
        (S::Start, E::Triplet(_) | E::EndOfData | E::Returned(_) | E::DefaultObject { .. })
        | (S::Walking | S::ModifierPending { .. }, E::Begin { .. } | E::Returned(_) | E::DefaultObject { .. })
        | (S::Returning, E::Begin { .. } | E::Triplet(_) | E::EndOfData | E::DefaultObject { .. })
        | (S::PlacingDefaults, E::Begin { .. } | E::Triplet(_) | E::EndOfData | E::Returned(_))
        | (S::Finished, E::Begin { .. } | E::Triplet(_) | E::EndOfData | E::Returned(_) | E::DefaultObject { .. }) => {
            (state, WalkAction::None)
        }
    }
}

/// One run of the walk of REQ-FSM-011.
struct Walker<'a, 's> {
    inputs: &'a Inputs<'a, 's>,
    invoked: [u16; OBJECT_TYPES],
    open: FixedVec<(usize, usize), OPEN_RECORDS>,
    successive: u8,
    next_slot: u16,
    /// The one non-default pair of Modified G0 and G2 sets a Level 2.5 page
    /// may use (REQ-L25-012).
    extra_pair: Option<(G0Set, G2Set)>,
    report: EnhancementReport,
}

impl<'a, 's> Walker<'a, 's> {
    fn new(inputs: &'a Inputs<'a, 's>) -> Self {
        Self {
            inputs,
            invoked: [0; OBJECT_TYPES],
            open: FixedVec::new((0, 0)),
            successive: 0,
            next_slot: 1,
            extra_pair: None,
            report: EnhancementReport::new(inputs.build, inputs.level),
        }
    }

    /// Runs the machine of REQ-FSM-011 to the end: the local enhancement
    /// data when it displays anything, otherwise the default objects, with
    /// every object they invoke on an explicit stack of walk records at most
    /// [`WALK_DEPTH`] deep (REQ-GEN-016). Each loop reads one triplet, ends
    /// one source or places one default object, so the loop is bounded by
    /// the data and the limits.
    fn walk(&mut self, out: &mut Collector) {
        let mut stack: FixedVec<Frame<'s>, WALK_DEPTH> = FixedVec::new(Frame::local());
        let mut defaults = self.inputs.links.default_objects.into_iter().flatten();
        let mut found: Option<Frame<'s>> = None;
        let mut state = WalkState::Start;
        loop {
            let event = match state {
                WalkState::Start => WalkEvent::Begin { display: has_display_triplets(self.inputs.page) },
                WalkState::Walking | WalkState::ModifierPending { .. } => self.next_triplet(&mut stack),
                WalkState::Returning => {
                    let ended = match stack.as_slice() {
                        [_, _, ..] => Ended::Object,
                        [frame] if frame.local_data => Ended::Local,
                        _ => Ended::Default,
                    };
                    WalkEvent::Returned(ended)
                }
                WalkState::PlacingDefaults => {
                    found = defaults.find_map(|default| self.default_object(default));
                    WalkEvent::DefaultObject { found: found.is_some() }
                }
                WalkState::Finished => return,
            };
            let (next, action) = walk_transition(state, event);
            state = next;
            self.act(action, &mut stack, &mut found, out);
        }
    }

    /// The next event of the current source: its next triplet, or the end
    /// of its data. The local enhancement data skips missing designation
    /// codes (REQ-L25-002).
    fn next_triplet(&mut self, stack: &mut FixedVec<Frame<'s>, WALK_DEPTH>) -> WalkEvent {
        let Some(frame) = stack.as_mut_slice().last_mut() else { return WalkEvent::EndOfData };
        while frame.cursor < frame.end {
            let index = frame.cursor;
            frame.cursor = frame.cursor.saturating_add(1);
            self.report.steps = self.report.steps.saturating_add(1);
            match frame.body.fetch(self.inputs.page, index) {
                Fetch::Triplet(triplet) => return WalkEvent::Triplet(Some(triplet.function())),
                Fetch::Damaged => return WalkEvent::Triplet(None),
                Fetch::Missing if frame.local_data => {
                    let per_packet = TRIPLETS_PER_PACKET as u16;
                    let next =
                        index.saturating_add(per_packet).saturating_sub(index.checked_rem(per_packet).unwrap_or(0));
                    frame.cursor = frame.cursor.max(next);
                }
                // An object's scan found every packet it needs.
                Fetch::Missing => break,
            }
        }
        WalkEvent::EndOfData
    }

    /// A default object of REQ-OBJ-040, found and accepted, with origin and
    /// active position row 0, column 0 (clause 13.8).
    fn default_object(&mut self, default: objects::DefaultInvocation) -> Option<Frame<'s>> {
        // Annex A.8 item 1: default objects count toward the limits.
        if !self.within_limits(default.kind, (0, 0)) {
            return None;
        }
        let inputs = self.inputs;
        let found = objects::find_public(
            inputs.lookup,
            Some(default.page),
            inputs.page,
            default.kind,
            default.number,
            inputs.level,
        );
        let Ok(object) = found else {
            self.report.faults.absent_objects = self.report.faults.absent_objects.saturating_add(1);
            return None;
        };
        self.accept(object, (0, 0), (0, 0))
    }

    /// Performs the action of a transition.
    fn act(
        &mut self,
        action: WalkAction,
        stack: &mut FixedVec<Frame<'s>, WALK_DEPTH>,
        found: &mut Option<Frame<'s>>,
        out: &mut Collector,
    ) {
        match action {
            WalkAction::None => {}
            WalkAction::BeginLocal => {
                stack.clear();
                let _ = stack.push(Frame::local());
            }
            WalkAction::StartDefault => {
                stack.clear();
                if let Some(frame) = found.take() {
                    let _ = stack.push(frame);
                }
            }
            // Popping restores the invoker's position and sub-tables, which
            // are in its own record (REQ-OBJ-002); a default object's record
            // goes, leaving row 0, column 0 (clause 13.8).
            WalkAction::Pop | WalkAction::ResetPosition => {
                let _ = stack.pop();
            }
            WalkAction::Skip => {
                self.report.faults.damaged_triplets = self.report.faults.damaged_triplets.saturating_add(1);
            }
            WalkAction::EndSource => {
                if let Some(frame) = stack.as_mut_slice().last_mut() {
                    frame.cursor = frame.end;
                }
            }
            WalkAction::Invoke { source, kind, number, row_offset, column_offset } => {
                let child = stack
                    .as_slice()
                    .last()
                    .and_then(|frame| self.invoke(frame, source, kind, number, (row_offset, column_offset)));
                if let Some(child) = child {
                    // The nesting rules bound the depth (decision E.51).
                    let _ = stack.push(child);
                }
            }
            WalkAction::ScreenColour(_)
            | WalkAction::RowColour { .. }
            | WalkAction::Position { .. }
            | WalkAction::DrcsMode { .. }
            | WalkAction::Column(_) => {
                if let Some(frame) = stack.as_mut_slice().last_mut() {
                    self.act_on_frame(action, frame, out);
                }
            }
        }
    }

    /// The actions that change the current source's record or emit events.
    fn act_on_frame(&mut self, action: WalkAction, frame: &mut Frame<'s>, out: &mut Collector) {
        // REQ-OBJ-013: Full Screen Colour, Full Row Colour and Address
        // Display Row 0 act only in local data and Active objects, and the
        // colours only in the main page.
        let restricted = !frame.plane.sets_colours();
        match action {
            WalkAction::ScreenColour(colour) => {
                if !restricted && frame.area == Area::Main && colour.is_defined() && out.row.is_none() {
                    out.screen = Some(colour.index);
                }
            }
            WalkAction::RowColour { row, colour } if !restricted => {
                (frame.row, frame.column) = (row, 0);
                let (row, _) = frame.position();
                if frame.area == Area::Main && row < ROWS {
                    out.row_colour(row, colour);
                }
            }
            WalkAction::Position { row, column } => {
                frame.row = row;
                // Decision E.46: a column of 40 or more leaves the column.
                if usize::from(column) < COLUMNS {
                    frame.column = column;
                }
            }
            WalkAction::DrcsMode { normal, levels, subtable } => {
                // Decision E.54: the reserved level value 00 counts as both.
                let bit = if self.inputs.level.is_level35() { LEVEL_BIT_35 } else { LEVEL_BIT_25 };
                let applies = levels & DRCS_LEVEL_MASK == 0 || levels & bit != 0;
                if applies && let Some(chosen) = frame.drcs.get_mut(usize::from(normal)) {
                    *chosen = subtable;
                }
            }
            WalkAction::Column(function) => self.column_triplet(frame, function, out),
            WalkAction::RowColour { .. }
            | WalkAction::None
            | WalkAction::BeginLocal
            | WalkAction::StartDefault
            | WalkAction::Pop
            | WalkAction::ResetPosition
            | WalkAction::Skip
            | WalkAction::EndSource
            | WalkAction::Invoke { .. } => {}
        }
    }

    /// The cell of the active position, when an element may be placed there
    /// (REQ-OBJ-014, REQ-L25-041, REQ-L25-050).
    fn placeable(&mut self, frame: &Frame<'s>) -> Option<(usize, usize)> {
        let (row, column) = frame.position();
        let area = area_of(column, self.inputs.panels);
        let receiver = row == 0 && column < RECEIVER_COLUMNS;
        let panel_row_0 = row == 0 && area != Area::Main;
        if row >= ROWS || area != frame.area || receiver || panel_row_0 {
            self.report.faults.discarded_writes = self.report.faults.discarded_writes.saturating_add(1);
            return None;
        }
        Some((row, column))
    }

    /// A column triplet: its column, then its character or attribute.
    fn column_triplet(&mut self, frame: &mut Frame<'s>, function: TripletFunction, out: &mut Collector) {
        // REQ-L25-004: reserved modes and Programme Delivery Control leave
        // the position; so do row triplets that reach here, which are
        // reserved or Programme Delivery Control, or restricted (REQ-L25-052,
        // REQ-OBJ-013).
        let Some(column) = function.active_column() else { return };
        frame.column = column;
        let level_35 = self.inputs.level.is_level35();
        let mut change = AttrChange::default();
        let mut extra_rows = 0;
        let character = match function {
            TripletFunction::ForegroundColour { colour, .. } if colour.is_defined() => {
                change.foreground = Some(colour.index);
                None
            }
            TripletFunction::BackgroundColour { colour, .. } if colour.is_defined() => {
                change.background = Some(colour.index);
                None
            }
            TripletFunction::AdditionalFlash { mode, rate, reserved, .. } => {
                // The phases count from the absolute column, set below.
                let Some(flash) = flash_of(mode, rate, reserved, 0) else { return };
                change.flash = Some(flash);
                None
            }
            TripletFunction::DisplayAttributes { attributes, .. } => {
                change.display = Some(attributes);
                None
            }
            TripletFunction::FontStyle { style, .. } if level_35 => {
                change.font = Some(font_of(style));
                extra_rows = style.rows;
                None
            }
            TripletFunction::ModifiedDesignation { designation, .. } => {
                let Some(pair) = self.designation(designation) else { return };
                if matches!(frame.plane, Plane::Adaptive | Plane::Passive) {
                    // Annex C.4 rule 2: to the end of the object.
                    frame.designation = Some(pair);
                    return;
                }
                change.designation = Some(pair);
                None
            }
            TripletFunction::G1Character { code, .. } => Some(EnhChar::G1(code)),
            TripletFunction::G3Character { code, .. } => Some(EnhChar::G3(code)),
            TripletFunction::G0Character { code, .. } => Some(EnhChar::G0 { code, diacritic: 0, at: false }),
            TripletFunction::G2Character { code, .. } => Some(EnhChar::G2(code)),
            TripletFunction::G0Diacritic { code, diacritic, .. } => {
                Some(EnhChar::G0 { code, diacritic, at: diacritic == 0 && code == TRIPLET_AT_SIGN_CODE })
            }
            TripletFunction::DrcsCharacter { normal, character, .. } => {
                let table = if normal { DrcsTable::Normal } else { DrcsTable::Global };
                let subtable = frame.drcs.get(usize::from(normal)).copied().unwrap_or(0);
                Some(EnhChar::Drcs(DrcsCharacter { table, subtable, character }))
            }
            // Reserved data: the triplet only sets the column.
            _ => return,
        };
        // REQ-L25-004: a character code below 2/0 sets the column and writes
        // nothing. DRCS characters are numbered from 0, so the rule does not
        // apply to them.
        if let Some(EnhChar::G0 { code, .. } | EnhChar::G1(code) | EnhChar::G2(code) | EnhChar::G3(code)) = character
            && code < TRIPLET_FIRST_CHARACTER
        {
            return;
        }
        let Some((row, column)) = self.placeable(frame) else { return };
        // Annex A.8 item 3: something displayable was written.
        self.successive = 0;
        if frame.plane == Plane::Adaptive {
            out.touch(row, column, frame.slot);
        }
        if let Some(Flash::On { start, .. }) = change.flash.as_mut() {
            *start = column as u8;
        }
        match character {
            Some(character) => Self::character(frame, row, column, character, out),
            None => Self::attribute(frame, row, column, change, extra_rows, out),
        }
    }

    /// A Modified G0 and G2 designation (REQ-L25-012): `None` when reserved,
    /// or at Level 2.5 when it would make a third pair on the page.
    fn designation(&mut self, value: u8) -> Option<(G0Set, G2Set)> {
        let accepted = charset::modified_designation(value).filter(|&pair| {
            self.inputs.level.is_level35()
                || pair == self.inputs.default_sets
                || *self.extra_pair.get_or_insert(pair) == pair
        });
        if accepted.is_none() {
            self.report.faults.ignored_designations = self.report.faults.ignored_designations.saturating_add(1);
        }
        accepted
    }

    fn character(frame: &Frame<'s>, row: usize, column: usize, character: EnhChar, out: &mut Collector) {
        let Some(slot) = out.slot(row, column) else { return };
        // Clause 13.2: within one plane the later write wins (Annex A.7.1).
        match frame.plane {
            Plane::Local => slot.local = Some(character),
            Plane::Active => slot.active = Some(character),
            Plane::Adaptive => slot.adaptive = Some((character, frame.designation)),
            Plane::Passive => slot.passive = Some((character, frame.designation, frame.passive)),
        }
    }

    fn attribute(
        frame: &mut Frame<'s>,
        row: usize,
        column: usize,
        change: AttrChange,
        extra_rows: u8,
        out: &mut Collector,
    ) {
        if frame.plane == Plane::Passive {
            // REQ-OBJ-011: a Passive object's attributes are its own and
            // persist across rows until it changes them.
            frame.passive.apply(&change);
            return;
        }
        // REQ-L25-028: a Font Style extends over the next 0 to 7 rows from
        // the same column, stopping at row 24.
        for target in row..=row.saturating_add(usize::from(extra_rows)).min(ROWS.saturating_sub(1)) {
            let Some(slot) = out.slot(target, column) else { continue };
            if frame.plane == Plane::Adaptive {
                slot.adaptive_change = slot.adaptive_change.then(change);
                slot.adaptive_owner = Some(frame.slot);
            } else {
                slot.underlying = slot.underlying.then(change);
            }
        }
    }

    /// An Object Invocation (REQ-OBJ-001 to REQ-OBJ-008).
    fn invoke(
        &mut self,
        frame: &Frame<'s>,
        source: ObjectSource,
        kind: ObjectKind,
        number: ObjectNumber,
        (row_offset, column_offset): (u8, u8),
    ) -> Option<Frame<'s>> {
        let inputs = self.inputs;
        // REQ-OBJ-008; a local object only from the page's own data.
        let allowed = frame.plane.may_invoke(kind)
            && match source {
                ObjectSource::Illegal => false,
                ObjectSource::Local => frame.body.is_local(),
                ObjectSource::Public | ObjectSource::Global => true,
            };
        if !allowed {
            self.report.faults.forbidden_invocations = self.report.faults.forbidden_invocations.saturating_add(1);
            return None;
        }
        let invocation = frame.position();
        if !self.within_limits(kind, invocation) {
            return None;
        }
        let found = if source == ObjectSource::Local {
            objects::find_local(inputs.page, kind, number, inputs.level)
        } else {
            objects::find_public(
                inputs.lookup,
                inputs.links.object_page(source),
                inputs.page,
                kind,
                number,
                inputs.level,
            )
        };
        let Ok(object) = found else {
            self.report.faults.absent_objects = self.report.faults.absent_objects.saturating_add(1);
            return None;
        };
        let origin = (
            invocation.0.saturating_add(usize::from(row_offset)),
            invocation.1.saturating_add(usize::from(column_offset)),
        );
        self.accept(object, origin, invocation)
    }

    /// The limits of Annex A.8 (REQ-OBJ-020), checked before an object is
    /// looked for or scanned, so that the work stays bounded whatever the
    /// data: at most 127 invocations of each type, every permitted
    /// invocation counted whether or not its object is found; at most 15
    /// objects open at the invocation's position; at most 15 objects opened
    /// in succession without anything displayable written.
    fn within_limits(&mut self, kind: ObjectKind, invocation: (usize, usize)) -> bool {
        let start = scan_index(invocation.0, invocation.1);
        let open = self.open.as_slice().iter().filter(|&&(first, last)| first <= start && start <= last).count();
        let Some(count) = self.invoked.get_mut(type_index(kind)) else { return false };
        if *count >= OBJECT_INVOCATIONS_PER_TYPE
            || open >= OBJECTS_OPEN_AT_ONCE
            || self.successive >= OBJECTS_OPENED_IN_SUCCESSION
        {
            self.report.faults.limited_invocations = self.report.faults.limited_invocations.saturating_add(1);
            return false;
        }
        *count = count.saturating_add(1);
        true
    }

    /// Scans an object and makes its walk record (REQ-OBJ-006): it is open
    /// from the invocation to the last position it addresses (Annex A.8).
    fn accept(
        &mut self,
        object: ObjectRef<'s>,
        origin: (usize, usize),
        invocation: (usize, usize),
    ) -> Option<Frame<'s>> {
        let scanned = objects::prescan(&object, self.inputs.page);
        let Ok(extent) = scanned else {
            self.report.steps = self.report.steps.saturating_add(u32::from(OBJECT_BODY_TRIPLETS));
            self.report.faults.absent_objects = self.report.faults.absent_objects.saturating_add(1);
            return None;
        };
        self.report.steps = self.report.steps.saturating_add(u32::from(extent.end.saturating_sub(object.first)));
        let start = scan_index(invocation.0, invocation.1);
        let end = extent.last.map_or(start, |(row, column)| {
            scan_index(origin.0.saturating_add(usize::from(row)), origin.1.saturating_add(usize::from(column)))
        });
        let _ = self.open.push((start, end.max(start)));
        self.successive = self.successive.saturating_add(1);
        self.report.objects = self.report.objects.saturating_add(1);
        let slot = self.next_slot;
        self.next_slot = self.next_slot.saturating_add(1);
        Some(Frame {
            plane: Plane::of(object.kind),
            body: object.body,
            cursor: object.first,
            end: extent.end,
            local_data: false,
            origin,
            row: 0,
            column: 0,
            drcs: [0; 2],
            area: area_of(origin.1, self.inputs.panels),
            slot,
            designation: None,
            passive: Attrs::PASSIVE,
        })
    }
}

/// The styles of a Font Style triplet (REQ-L25-028).
const fn font_of(style: FontStyleData) -> FontStyle {
    FontStyle { bold: style.bold, italic: style.italic, proportional: style.proportional }
}

/// True for a triplet that displays something: a character, an attribute,
/// a screen or row colour, or an object (decision E.53).
fn is_display(function: TripletFunction) -> bool {
    matches!(
        function,
        TripletFunction::FullScreenColour(_)
            | TripletFunction::FullRowColour { .. }
            | TripletFunction::AddressDisplayRow0 { .. }
            | TripletFunction::ObjectInvocation { .. }
    ) || function.active_column().is_some()
}

/// True when the page's local enhancement data displays anything, so that
/// default objects are not placed (REQ-OBJ-040, decision E.53).
fn has_display_triplets(page: &StoredPage) -> bool {
    for index in 0..LOCAL_TRIPLETS {
        if let Fetch::Triplet(triplet) = Body::Local.fetch(page, index) {
            let function = triplet.function();
            if matches!(function, TripletFunction::Termination { .. } | TripletFunction::ObjectDefinition { .. }) {
                return false;
            }
            if is_display(function) {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Row resolution (REQ-FSM-012)
// ---------------------------------------------------------------------------

/// The inputs of a row's resolution that do not change along it.
// Each flag is an independent input: a page setting, a profile setting or
// the row's position.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy)]
struct RowContext {
    remapping: Remapping,
    substitution: bool,
    row_colour: ColourIndex,
    boxed_page: bool,
    double_start_box: bool,
    /// Double height and double size take effect in this row.
    double_allowed: bool,
    default_sets: (G0Set, G2Set),
    level_1_g0: G0,
    second_g0: Option<G0>,
}

/// The record of REQ-FSM-006 for the underlying plane, with the fields
/// REQ-FSM-012 adds.
// Each flag is a field REQ-FSM-006 or REQ-FSM-012 lists.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy)]
struct RowState {
    /// The attributes, with `conceal` the non-spacing conceal.
    attrs: Attrs,
    /// Conceal set by the Level 1 code 1/8, which a colour code ends
    /// (decision E.47).
    level_1_conceal: bool,
    /// The entry number of the foreground colour the Level 1 page selected,
    /// for New Background (REQ-L25-022).
    level_1_foreground: u8,
    mosaic: bool,
    /// Separated mosaics set by the Level 1 code 1/A.
    separated: bool,
    hold: bool,
    held: Option<(u8, bool)>,
    second_g0: bool,
    designation: Option<(G0Set, G2Set)>,
    /// A spacing double height or double size code acted in the row.
    spacing_double: bool,
}

impl RowState {
    /// The start-of-row defaults, re-mapped (REQ-L25-022): white
    /// foreground, and a black background unless substitution replaces it
    /// with the row colour.
    fn start(context: &RowContext) -> Self {
        let background =
            if context.substitution { context.row_colour } else { context.remapping.background(COLOUR_BLACK) };
        Self {
            attrs: Attrs { foreground: context.remapping.foreground(COLOUR_WHITE), background, ..Attrs::PASSIVE },
            level_1_conceal: false,
            level_1_foreground: COLOUR_WHITE,
            mosaic: false,
            separated: false,
            hold: false,
            held: None,
            second_g0: false,
            designation: None,
            spacing_double: false,
        }
    }

    /// The attributes shown.
    fn shown(&self) -> Attrs {
        Attrs { conceal: self.attrs.conceal || self.level_1_conceal, ..self.attrs }
    }

    /// A Level 1 size code (Table 26 notes 2, 5 and 8: any size attribute
    /// ends any other); the held mosaic resets when the size changes.
    fn size(&mut self, height: bool, width: bool) {
        if (self.attrs.double_height, self.attrs.double_width) != (height, width) {
            self.held = None;
        }
        self.attrs.double_height = height;
        self.attrs.double_width = width;
    }

    /// Step 1: the set-after attributes of the previous cell's byte.
    fn set_after(&mut self, code: u8, next: u8, context: &RowContext) {
        match code {
            // REQ-L25-022: 0/0 and 1/0 are black at these levels, and the
            // colour comes from the re-mapped CLUT.
            ALPHA_BLACK..=ALPHA_WHITE | MOSAIC_BLACK..=MOSAIC_WHITE => {
                let mosaic = code >= MOSAIC_BLACK;
                if mosaic != self.mosaic {
                    self.held = None;
                }
                self.mosaic = mosaic;
                self.level_1_foreground = code & CLUT_ENTRY_MASK;
                self.attrs.foreground = context.remapping.foreground(self.level_1_foreground);
                // Decision E.47: a colour code ends the Level 1 conceal only.
                self.level_1_conceal = false;
            }
            FLASH => self.attrs.flash = LEVEL_1_FLASH,
            // Decision E.47: the Level 1 box codes act only on boxed pages.
            END_BOX if context.boxed_page => self.attrs.boxed = false,
            START_BOX if context.boxed_page && (!context.double_start_box || next == START_BOX) => {
                self.attrs.boxed = true;
            }
            DOUBLE_HEIGHT | DOUBLE_SIZE => {
                self.size(true, code == DOUBLE_SIZE);
                // REQ-L25-026: the row below is suppressed.
                self.spacing_double |= context.double_allowed;
            }
            DOUBLE_WIDTH => self.size(false, true),
            ESC => self.second_g0 = !self.second_g0,
            RELEASE_MOSAICS => self.hold = false,
            _ => {}
        }
    }

    /// Step 2: the set-at attributes of the cell's byte.
    fn set_at(&mut self, code: u8, context: &RowContext) {
        match code {
            STEADY => self.attrs.flash = Flash::Steady,
            NORMAL_SIZE => self.size(false, false),
            CONCEAL => self.level_1_conceal = true,
            CONTIGUOUS_MOSAICS => self.separated = false,
            SEPARATED_MOSAICS => self.separated = true,
            // REQ-L25-022: entry 0 of the background CLUT, or the row colour
            // under substitution.
            BLACK_BACKGROUND => {
                self.attrs.background =
                    if context.substitution { context.row_colour } else { context.remapping.background(COLOUR_BLACK) };
            }
            NEW_BACKGROUND => self.attrs.background = context.remapping.background(self.level_1_foreground),
            HOLD_MOSAICS => self.hold = true,
            _ => {}
        }
    }

    /// Step 3: the non-spacing attributes of the underlying plane, which win
    /// against step 2 (REQ-L25-020, decision E.47).
    fn apply(&mut self, change: &AttrChange) {
        self.attrs.apply(change);
        if let Some(display) = change.display {
            // Decision E.47: a 0 bit also cancels the Level 1 function.
            if !display.conceal {
                self.level_1_conceal = false;
            }
            if !display.underline {
                self.separated = false;
            }
        }
        if let Some(sets) = change.designation {
            self.designation = Some(sets);
        }
    }
}

/// An Adaptive object's overlay on the row (REQ-OBJ-010).
#[derive(Clone, Copy)]
struct Overlay {
    range: Range,
    change: AttrChange,
}

/// What resolving one cell gives, before sizes are applied.
#[derive(Clone, Copy)]
struct Resolved {
    cell: Cell,
    double_height: bool,
    double_width: bool,
}

/// Builds a cell from a glyph and the attributes it is drawn with.
fn cell_of(glyph: Glyph, attrs: &Attrs, source: CharSource, column: usize) -> Resolved {
    let text = is_text(glyph);
    let mut attributes = CellAttributes::DEFAULT.with_source(source).with_invert(attrs.invert);
    let flash = match attrs.flash {
        Flash::Steady => false,
        Flash::On { mode, rate, start } => {
            attributes = attributes.with_flash_mode(mode).with_flash_rate(flash_rate(rate, start, column));
            true
        }
    };
    if text {
        // Decisions E.47 and E.50: underline and the font styles are for G0
        // and G2 characters only.
        attributes = attributes.with_underline(attrs.underline).with_font(attrs.font);
    }
    Resolved {
        cell: Cell {
            glyph,
            foreground: attrs.foreground,
            background: attrs.background,
            flash,
            conceal: attrs.conceal,
            boxed: attrs.boxed,
            size: Size::Normal,
            held: false,
            parity_failed: false,
            attributes,
        },
        double_height: attrs.double_height,
        double_width: attrs.double_width,
    }
}

/// A cell of a row segment: what the walk put there and, in the main page,
/// the Level 1 byte.
struct Segment<'c> {
    first: usize,
    end: usize,
    /// The Level 1 codes and parity failures, `None` in side panels and in
    /// the row below spacing double height (REQ-L25-026).
    codes: Option<(&'c [u8; COLUMNS], u64)>,
    /// The row below spacing double height: cells without enhancement
    /// characters keep the fill of the cell above.
    lower: Option<&'c [Cell]>,
}

/// Resolves the cells of one row segment (REQ-FSM-012 steps 1 to 7).
/// `report` counts undefined characters.
fn resolve_segment(
    segment: &Segment<'_>,
    context: &RowContext,
    start: RowState,
    out: &Collector,
    resolved: &mut [Option<Resolved>; SCREEN_COLUMNS],
    report: &mut EnhancementReport,
) -> RowState {
    let mut state = start;
    let mut overlays: FixedVec<Overlay, RANGES> =
        FixedVec::new(Overlay { range: Range { slot: 0, first: 0, last: 0 }, change: AttrChange::default() });
    for range in out.ranges.as_slice() {
        if usize::from(range.first) >= segment.first && usize::from(range.last) < segment.end {
            let _ = overlays.push(Overlay { range: *range, change: AttrChange::default() });
        }
    }
    let mut pending: Option<u8> = None;
    for column in segment.first..segment.end {
        let slot = out.slots.get(column).copied().unwrap_or_default();
        let code = segment.codes.and_then(|(codes, _)| codes.get(column).copied()).unwrap_or(SPACE);
        // Steps 1 and 2: the Level 1 set-after and set-at attributes.
        if let Some(previous) = pending.take() {
            state.set_after(previous, code, context);
        }
        if segment.codes.is_some() {
            state.set_at(code, context);
        }
        // Step 3: the underlying plane's non-spacing attributes. A flash
        // placed here starts its phases here.
        state.apply(&slot.underlying);
        // Step 4: Adaptive overlays; the later invoked object's applies
        // where ranges overlap.
        let mut top: Option<(u16, AttrChange)> = None;
        for overlay in overlays.as_mut_slice() {
            let range = overlay.range;
            if column < usize::from(range.first) || column > usize::from(range.last) {
                continue;
            }
            if slot.adaptive_owner == Some(range.slot) {
                overlay.change = overlay.change.then(slot.adaptive_change);
            }
            if top.is_none_or(|(slot, _)| range.slot > slot) {
                top = Some((range.slot, overlay.change));
            }
        }
        let underlying = state.shown();
        let plane_attrs = top.map_or(underlying, |(_, change)| {
            let mut attrs = underlying;
            attrs.apply(&change);
            attrs
        });
        // A 0 underline bit of the overlay also clears Level 1 separated.
        let level_1_separated =
            state.separated && top.is_none_or(|(_, change)| change.display.is_none_or(|display| display.underline));
        // Step 6: the character of the highest priority (clause 13.2).
        let row_sets = state.designation.unwrap_or(context.default_sets);
        let enhancement = slot
            .passive
            .map(|(character, sets, attrs)| (character, sets.unwrap_or(context.default_sets), attrs, Plane::Passive))
            .or(slot.adaptive.map(|(character, sets)| {
                (character, sets.unwrap_or(context.default_sets), plane_attrs, Plane::Adaptive)
            }))
            .or(slot.local.map(|character| (character, row_sets, plane_attrs, Plane::Local)))
            .or(slot.active.map(|character| (character, row_sets, plane_attrs, Plane::Active)));
        let cell = match enhancement {
            Some((character, sets, attrs, plane)) => {
                let (glyph, undefined) = enhancement_glyph(character, sets, attrs.underline);
                if undefined {
                    report.faults.undefined_characters = report.faults.undefined_characters.saturating_add(1);
                }
                Some(cell_of(glyph, &attrs, plane.source(), column))
            }
            None if segment.lower.is_some() => None,
            None => {
                let separated = level_1_separated || plane_attrs.underline;
                let (glyph, held) = if segment.codes.is_some() {
                    level_1_glyph(&state, code, separated, context)
                } else {
                    (Glyph::Space, false)
                };
                let mut resolved = cell_of(glyph, &plane_attrs, CharSource::Level1, column);
                resolved.cell.held = held;
                resolved.cell.parity_failed = segment
                    .codes
                    .is_some_and(|(_, failed)| failed.checked_shr(column as u32).is_some_and(|bits| bits & 1 == 1));
                Some(resolved)
            }
        };
        if let Some(slot) = resolved.get_mut(column) {
            *slot = cell;
        }
        // Step 7: the held mosaic, from Level 1 characters only (decision
        // E.50), in the form shown at this cell.
        if segment.codes.is_some() {
            if state.mosaic && code > ATTRIBUTE_LAST && code & MOSAIC_BIT5 != 0 {
                state.held = Some((mosaic_pattern(code), level_1_separated || plane_attrs.underline));
            }
            pending = Some(code);
        }
    }
    state
}

/// What a Level 1 byte shows (REQ-FSM-006 step 2): a space or the held
/// mosaic for an attribute, a mosaic, or a character of the Level 1 G0 set
/// with its national option.
fn level_1_glyph(state: &RowState, code: u8, separated: bool, context: &RowContext) -> (Glyph, bool) {
    if code <= ATTRIBUTE_LAST {
        if state.hold
            && state.mosaic
            && let Some((pattern, separated)) = state.held
        {
            return (Glyph::Mosaic { pattern, separated }, true);
        }
        return (Glyph::Space, false);
    }
    if state.mosaic && code & MOSAIC_BIT5 != 0 {
        return (Glyph::Mosaic { pattern: mosaic_pattern(code), separated }, false);
    }
    let g0 = if state.second_g0 { context.second_g0.unwrap_or(context.level_1_g0) } else { context.level_1_g0 };
    (Glyph::Char(CharRef::new(CharSet::G0(g0), code)), false)
}

/// The cells of the row being placed, and what passes between rows.
struct RowCells {
    /// What each cell resolved to, before sizes.
    resolved: [Option<Resolved>; SCREEN_COLUMNS],
    /// The cells placed; `None` keeps the fill of the row below spacing
    /// double height.
    cells: [Option<Cell>; SCREEN_COLUMNS],
    /// The cells of the row above, for its lower halves.
    above: [Option<Cell>; SCREEN_COLUMNS],
    /// The cells of this row that enlarged characters of the row above
    /// cover.
    claimed: [bool; SCREEN_COLUMNS],
    /// The cells of the next row that this row's enlarged characters cover.
    claims: [bool; SCREEN_COLUMNS],
}

impl RowCells {
    const fn new() -> Self {
        Self {
            resolved: [None; SCREEN_COLUMNS],
            cells: [None; SCREEN_COLUMNS],
            above: [None; SCREEN_COLUMNS],
            claimed: [false; SCREEN_COLUMNS],
            claims: [false; SCREEN_COLUMNS],
        }
    }

    /// Starts a row: the claims of the previous row become this row's.
    fn next_row(&mut self) {
        self.claimed = self.claims;
        self.claims = [false; SCREEN_COLUMNS];
        self.resolved = [None; SCREEN_COLUMNS];
        self.cells = [None; SCREEN_COLUMNS];
    }

    /// Forgets the row above, for a row that is not rebuilt.
    fn forget(&mut self) {
        self.above = [None; SCREEN_COLUMNS];
        self.claims = [false; SCREEN_COLUMNS];
    }

    /// Applies the size rules of Annex C.3 to the resolved cells of a
    /// segment and places them (REQ-L25-025). Cells that the row above
    /// covers with lower halves show them.
    fn place(&mut self, first: usize, end: usize, double_allowed: bool) {
        let mut column = first;
        while column < end {
            let right = column.saturating_add(1);
            // Rule 2: earlier size attributes in scan order win, so a lower
            // half from the row above hides this cell.
            if self.claimed.get(column).copied().unwrap_or(false) {
                if let (Some(slot), Some(Some(upper))) = (self.cells.get_mut(column), self.above.get(column)) {
                    *slot = Some(Cell { size: Size::DoubleBottom, ..*upper });
                }
                column = right;
                continue;
            }
            let Some(Some(resolved)) = self.resolved.get(column).copied() else {
                column = right;
                continue;
            };
            // Rule 1: no doubling where it cannot be shown: rows 0, 23 and
            // 24, the last column of an area, or over a lower half. Rule 3:
            // a double size character keeps whichever doubling fits.
            let height = resolved.double_height && double_allowed;
            let width = resolved.double_width && right < end && !self.claimed.get(right).copied().unwrap_or(true);
            let mut cell = resolved.cell;
            cell.size = if height { Size::DoubleTop } else { Size::Normal };
            if width {
                cell.attributes = cell.attributes.with_width(Width::LeftHalf);
            }
            let parts = if width { 2 } else { 1 };
            for (part, target) in [column, right].into_iter().enumerate().take(parts) {
                // Rule 4: the origin's attributes apply to every part.
                let part_cell = if part == 0 {
                    cell
                } else {
                    Cell { attributes: cell.attributes.with_width(Width::RightHalf), ..cell }
                };
                if let Some(slot) = self.cells.get_mut(target) {
                    *slot = Some(part_cell);
                }
                if height && let Some(claim) = self.claims.get_mut(target) {
                    *claim = true;
                }
            }
            column = if width { right.saturating_add(1) } else { right };
        }
    }
}

/// True when the page is an enhancement page and so never shown as text
/// (REQ-OBJ-041): the Magazine Organization Table, or a page whose stored
/// enhancement copy names an object, DRCS or table page function. A page
/// named only by a link is still shown, so that a wrong link never hides a
/// display page.
fn is_enhancement_page(page: &StoredPage, lookup: &EnhancementLookup<'_>) -> bool {
    let address = page.key.address;
    if address.page.0 == PAGE_MOT {
        return true;
    }
    let s1 = EnhancementKey::new(address, page.key.subcode).s1;
    lookup.sub_page(address.magazine, address.page, s1).and_then(|sub_page| sub_page.format()).is_some_and(|format| {
        matches!(
            format.function,
            PAGE_FUNCTION_GPOP | PAGE_FUNCTION_POP | PAGE_FUNCTION_GDRCS | PAGE_FUNCTION_DRCS | PAGE_FUNCTION_MOT
        )
    })
}

/// What rebuilding the rows of a page needs.
struct Rows<'a, 's> {
    inputs: &'a Inputs<'a, 's>,
    config: &'a Config,
    options: &'a DisplayOptions,
    presentation: &'a PagePresentation,
    row_colours: [ColourIndex; ROWS],
    last_colour_row: usize,
    /// The rows on which the page-level run met events.
    with_events: u32,
    second_g0: Option<G0>,
    level_1_g0: G0,
    boxed_page: bool,
}

impl Rows<'_, '_> {
    /// The inputs of one row's resolution.
    fn context(&self, row: usize) -> RowContext {
        RowContext {
            remapping: self.presentation.remapping,
            substitution: self.presentation.black_background_substitution,
            row_colour: self.row_colours.get(row).copied().unwrap_or(COLOUR_BLACK),
            boxed_page: self.boxed_page,
            double_start_box: self.config.double_start_box,
            double_allowed: (DOUBLE_HEIGHT_FIRST_ROW..=DOUBLE_HEIGHT_LAST_ROW).contains(&row),
            default_sets: self.inputs.default_sets,
            level_1_g0: self.level_1_g0,
            second_g0: self.second_g0,
        }
    }

    /// Resolves and places one row and its side panels (REQ-FSM-012).
    /// `lower` is true for the row below spacing double height, whose
    /// cells without enhancement characters show `fill`. Returns true when
    /// a spacing double height or double size code acted in the row.
    fn row(
        &self,
        row: usize,
        lower: bool,
        fill: &[Cell; COLUMNS],
        cells: &mut RowCells,
        report: &mut EnhancementReport,
    ) -> bool {
        let mut events = Collector::new(Some(row), self.last_colour_row);
        if self.with_events.checked_shr(row as u32).is_some_and(|bits| bits & 1 == 1) {
            let mut walker = Walker::new(self.inputs);
            walker.walk(&mut events);
            report.steps = report.steps.saturating_add(walker.report.steps);
        }
        let context = self.context(row);
        let (codes, failed) =
            row_codes(row, Some(self.inputs.page), self.config, self.options, self.config.parity_policy);
        let main = Segment {
            first: 0,
            end: COLUMNS,
            codes: (!lower).then_some((&codes, failed)),
            lower: lower.then_some(&fill[..]),
        };
        let state = resolve_segment(&main, &context, RowState::start(&context), &events, &mut cells.resolved, report);
        cells.place(0, COLUMNS, context.double_allowed);
        // REQ-L25-041: each panel starts from the start-of-row defaults with
        // the row colour as its background, and no attribute crosses into or
        // out of it.
        if row >= SIDE_PANEL_FIRST_ROW {
            let panels = self.inputs.panels;
            let panel_start = RowState::start(&RowContext { substitution: true, ..context });
            let right = (COLUMNS, COLUMNS.saturating_add(usize::from(panels.right)));
            let left = (panels.left_first_column(), SCREEN_COLUMNS);
            for (first, end) in [right, left] {
                if first < end {
                    let segment = Segment { first, end, codes: None, lower: None };
                    let _ = resolve_segment(&segment, &context, panel_start, &events, &mut cells.resolved, report);
                    cells.place(first, end, context.double_allowed);
                }
            }
        }
        state.spacing_double
    }
}

/// Builds a page's grid at Level 2.5 or 3.5 (REQ-L25-001 and the areas
/// L25, OBJ and DRC): `grid` holds the page's Level 1 grid, built by
/// `level1::build_grid` with the same page, magazine data, configuration
/// and options. At other levels it does nothing.
///
/// Rows are rebuilt from their Level 1 bytes and the enhancement data; row
/// 0 keeps the receiver's status area, and rows the Level 1 grid does not
/// show (a suppressed header, inhibited display, a row 24 that is not
/// displayed or that the receiver draws) keep their Level 1 cells.
pub fn apply<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &mut Grid<DRCS, PANEL_ROWS>,
    page: &StoredPage,
    magazine: &MagazineData,
    lookup: &EnhancementLookup<'_>,
    config: &Config,
    options: &DisplayOptions,
) -> EnhancementReport {
    let build = PageBuild::of(page, config.level);
    if !config.level.is_enhanced() {
        return EnhancementReport::new(build, config.level);
    }
    let links = objects::links(page, lookup, config.level);
    let level = links.level;
    let config = Config { level, ..*config };
    // REQ-OBJ-041: an enhancement page shows no rows of text.
    if is_enhancement_page(page, lookup) {
        for row in grid.cells.iter_mut().skip(1) {
            row.fill(Cell::BLANK);
        }
        grid.double_height_rows = 0;
        return EnhancementReport::new(build, level);
    }
    let (designation, second_g0, _) = resolve_designation(Some(page), magazine, &config);
    grid.designation = designation;

    // REQ-L25-030, REQ-L25-031, REQ-L25-040: the page-level data.
    let page_fields = page.page_data.map(|data| data.fields);
    let page_fields_35 = page.page_data_level35.map(|data| data.fields);
    let sources = PresentationSources {
        page: page_fields.as_ref(),
        page_level_35: page_fields_35.as_ref(),
        magazine: magazine.data.as_ref(),
        magazine_level_35: magazine.level35.as_ref(),
        fallback: links.fallback,
    };
    let presentation = colour::resolve(&sources, level, &ColourMap::DEFAULT);
    // Panels need every panel row of the grid.
    let panels = if PANEL_ROWS >= SIDE_PANEL_ROWS { presentation.side_panels } else { SidePanels::NONE };
    let inputs = Inputs {
        page,
        lookup,
        links: &links,
        build,
        level,
        panels,
        default_sets: (designation.g0.set, designation.g2),
    };
    let row_24_shown = row_24_shown(page, &config) && options.row24.is_none();
    let last_colour_row = if row_24_shown { ROWS.saturating_sub(1) } else { ROWS.saturating_sub(2) };

    // The page-level run: screen and row colours (decision E.45).
    let mut page_level = Collector::new(None, last_colour_row);
    let mut walker = Walker::new(&inputs);
    walker.walk(&mut page_level);
    let mut report = walker.report;
    let screen = page_level.screen.unwrap_or(presentation.default_screen);
    let fallback_row = if page_level.screen.is_some() { screen } else { presentation.default_row };
    let mut row_colours = [fallback_row; ROWS];
    for (colour, explicit) in row_colours.iter_mut().zip(page_level.row_colours) {
        *colour = explicit.unwrap_or(fallback_row);
    }

    let control = page.control.0;
    let rows = Rows {
        inputs: &inputs,
        config: &config,
        options,
        presentation: &presentation,
        row_colours,
        last_colour_row,
        with_events: page_level.event_rows,
        second_g0,
        level_1_g0: designation.g0,
        boxed_page: control & (C5_NEWSFLASH | C6_SUBTITLE) != 0,
    };
    rebuild_rows(grid, &rows, row_24_shown, &mut report);

    grid.colour_map = presentation.colour_map;
    grid.screen_colour = screen;
    grid.row_colours = row_colours;
    grid.side_panels = panels;
    grid.double_height_rows = 0;
    for (row, cells) in grid.cells.iter().enumerate() {
        if cells.iter().any(|cell| cell.size == Size::DoubleTop) {
            grid.double_height_rows |= 1u32.checked_shl(row as u32).unwrap_or(0);
        }
    }
    let drcs_page = page.drcs_colours;
    grid.drcs_colours = DrcsColourTables::resolve(drcs_page.as_ref(), magazine.drcs_colours.as_ref(), level);
    fill_drcs(grid, &links, lookup, level, &mut report);
    report
}

/// Rebuilds the rows the page shows, top to bottom, so that each row knows
/// the row above it (REQ-L25-025, REQ-L25-026).
fn rebuild_rows<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &mut Grid<DRCS, PANEL_ROWS>,
    rows: &Rows<'_, '_>,
    row_24_shown: bool,
    report: &mut EnhancementReport,
) {
    let page = rows.inputs.page;
    let control = page.control.0;
    let inhibit = control & C10_INHIBIT_DISPLAY != 0 || !page.is_text_page();
    let receiver_area: [Cell; RECEIVER_COLUMNS] = core::array::from_fn(|column| {
        grid.cells.first().and_then(|row| row.get(column)).copied().unwrap_or(Cell::BLANK)
    });
    let mut cells = RowCells::new();
    let mut lower = false;
    for row in 0..ROWS {
        let shown = match row {
            0 => control & C7_SUPPRESS_HEADER == 0,
            _ if row == ROWS.saturating_sub(1) => !inhibit && row_24_shown,
            _ => !inhibit,
        };
        if !shown {
            cells.forget();
            lower = false;
            continue;
        }
        cells.next_row();
        // REQ-L25-026: the row below spacing double height shows, where no
        // enhancement character is written, the background of the cell
        // above and no foreground.
        let fill: [Cell; COLUMNS] = core::array::from_fn(|column| {
            let upper = cells.above.get(column).copied().flatten().unwrap_or(Cell::BLANK);
            Cell { foreground: upper.foreground, background: upper.background, boxed: upper.boxed, ..Cell::BLANK }
        });
        let spacing_double = rows.row(row, lower, &fill, &mut cells, report);
        for (column, cell) in cells.cells.iter().enumerate().take(COLUMNS) {
            let mut cell = cell.unwrap_or_else(|| fill.get(column).copied().unwrap_or(Cell::BLANK));
            // REQ-L25-050: row 0 columns 0 to 7 stay the receiver's.
            if row == 0
                && let Some(receiver) = receiver_area.get(column)
            {
                cell = *receiver;
            }
            if let Some(target) = grid.cell_mut(row, column) {
                *target = cell;
            }
        }
        for (column, cell) in cells.cells.iter().enumerate().skip(COLUMNS) {
            if let (Some(target), Some(cell)) = (grid.panel_cell_mut(row, column), cell)
                && area_of(column, rows.inputs.panels) != Area::Outside
            {
                *target = *cell;
            }
        }
        for (column, slot) in cells.above.iter_mut().enumerate() {
            *slot =
                if column < COLUMNS { grid.cell(row, column).copied() } else { grid.panel_cell(row, column).copied() };
        }
        lower = spacing_double && !lower;
    }
}

/// Puts into the grid the pattern of every DRCS character its cells use
/// (REQ-DRC-006, REQ-DRC-007, REQ-DRC-009). Each character is read from the
/// displayed page's GDRCS or DRCS link, also for characters inside objects
/// (Annex E.6). A cell whose data is invalid shows a space; one whose data
/// is missing keeps its character and has no pattern, which the renderer
/// draws as a space in the background colour.
fn fill_drcs<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &mut Grid<DRCS, PANEL_ROWS>,
    links: &EnhancementLinks,
    lookup: &EnhancementLookup<'_>,
    level: PresentationLevel,
    report: &mut EnhancementReport,
) {
    let rows = grid.cells.len().saturating_add(grid.panels.len());
    for index in 0..rows {
        let width = if index < ROWS { COLUMNS } else { SCREEN_COLUMNS.saturating_sub(COLUMNS) };
        for column in 0..width {
            let cell = if index < ROWS {
                grid.cells.get(index).and_then(|row| row.get(column)).copied()
            } else {
                grid.panels.get(index.saturating_sub(ROWS)).and_then(|row| row.get(column)).copied()
            };
            let Some(Cell { glyph: Glyph::Char(character), .. }) = cell else { continue };
            let Some(drcs) = character.drcs_character() else { continue };
            if grid.drcs_pattern(drcs).is_some() {
                continue;
            }
            let link = match drcs.table {
                DrcsTable::Normal => links.drcs,
                DrcsTable::Global => links.gdrcs,
            };
            let sub_page = link.and_then(|link| lookup.sub_page(link.magazine, link.page, drcs.subtable));
            match decode_character(sub_page, drcs, level) {
                Ok(pattern) => {
                    if !grid.drcs.push(pattern) {
                        // REQ-DRC-007: beyond the capacity, treated as missing.
                        report.faults.drcs_missing = report.faults.drcs_missing.saturating_add(1);
                    }
                }
                Err(DrcsFault::Missing) => report.faults.drcs_missing = report.faults.drcs_missing.saturating_add(1),
                Err(DrcsFault::Invalid) => {
                    report.faults.drcs_invalid = report.faults.drcs_invalid.saturating_add(1);
                    let target = if index < ROWS {
                        grid.cells.get_mut(index).and_then(|row| row.get_mut(column))
                    } else {
                        grid.panels.get_mut(index.saturating_sub(ROWS)).and_then(|row| row.get_mut(column))
                    };
                    if let Some(target) = target {
                        target.glyph = Glyph::Space;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::consts::{FLASH_MODE_STEADY, MOSAIC_FULL};

    /// REQ-L25-027, decision E.48: the flash a triplet sets, and its
    /// reserved values.
    #[test]
    fn additional_flash_values() {
        let on = |mode, rate| Some(Flash::On { mode, rate, start: 3 });
        assert_eq!(flash_of(FLASH_MODE_STEADY, FLASH_RATE_PHASE_2, 0, 3), Some(Flash::Steady));
        assert_eq!(
            flash_of(FLASH_MODE_NORMAL, FLASH_RATE_SLOW, 0, 3),
            on(FlashMode::Normal, Rate::Fixed(FlashRate::Slow))
        );
        assert_eq!(
            flash_of(FLASH_MODE_INVERTED, FLASH_RATE_PHASE_3, 0, 3),
            on(FlashMode::Inverted, Rate::Fixed(FlashRate::FastPhase3))
        );
        assert_eq!(
            flash_of(FLASH_MODE_ADJACENT_CLUT, FLASH_RATE_INCREMENTAL, 0, 3),
            on(FlashMode::AdjacentClut, Rate::Incremental)
        );
        assert_eq!(flash_of(FLASH_MODE_NORMAL, FLASH_RATE_DECREMENTAL, 0, 3), on(FlashMode::Normal, Rate::Decremental));
        assert_eq!(flash_of(FLASH_MODE_NORMAL, 0b110, 0, 3), None);
        assert_eq!(flash_of(FLASH_MODE_NORMAL, 0b111, 0, 3), None);
        assert_eq!(flash_of(FLASH_MODE_NORMAL, FLASH_RATE_SLOW, 0b01, 3), None);
        let phases: [FlashRate; 4] = core::array::from_fn(|step| flash_rate(Rate::Decremental, 10, 10 + step));
        assert_eq!(
            phases,
            [FlashRate::FastPhase1, FlashRate::FastPhase3, FlashRate::FastPhase2, FlashRate::FastPhase1]
        );
        assert_eq!(flash_rate(Rate::Incremental, 10, 9), FlashRate::FastPhase1, "left of the start");
    }

    /// Decision E.49: the display areas of a row.
    #[test]
    fn display_areas() {
        let panels = SidePanels { left: 6, right: 10 };
        let areas = [0, 39, 40, 49, 50, 65, 66, 71, 72].map(|column| area_of(column, panels));
        let expected = [
            Area::Main,
            Area::Main,
            Area::Right,
            Area::Right,
            Area::Outside,
            Area::Outside,
            Area::Left,
            Area::Left,
            Area::Outside,
        ];
        assert_eq!(areas, expected);
        assert_eq!(area_of(40, SidePanels::NONE), Area::Outside);
        assert_eq!(scan_index(1, 0), SCREEN_COLUMNS);
    }

    /// REQ-OBJ-008, REQ-OBJ-013: the nesting table and the planes that set
    /// colours.
    #[test]
    fn planes() {
        use ObjectKind::{Active, Adaptive, Passive};
        let table = [
            (Plane::Local, [true, true, true]),
            (Plane::Active, [false, true, true]),
            (Plane::Adaptive, [false, false, true]),
            (Plane::Passive, [false, false, false]),
        ];
        for (plane, allowed) in table {
            assert_eq!([Active, Adaptive, Passive].map(|kind| plane.may_invoke(kind)), allowed, "{plane:?}");
        }
        assert!(Plane::Local.sets_colours() && Plane::Active.sets_colours());
        assert!(!Plane::Adaptive.sets_colours() && !Plane::Passive.sets_colours());
    }

    /// REQ-L25-010, REQ-L25-011, decision E.50: enhancement glyphs.
    #[test]
    fn enhancement_glyphs() {
        let latin = (G0Set::Latin, G2Set::Latin);
        let full = Glyph::Mosaic { pattern: MOSAIC_FULL, separated: true };
        assert_eq!(enhancement_glyph(EnhChar::G1(0x7F), latin, true).0, full);
        let greek = CharSet::G0(G0 { set: G0Set::Greek, national: None });
        let (glyph, undefined) = enhancement_glyph(EnhChar::G1(b'Q'), (G0Set::Greek, G2Set::Greek), false);
        assert_eq!((glyph, undefined), (Glyph::Char(CharRef::new(greek, b'Q')), false));
        assert_eq!(enhancement_glyph(EnhChar::G2(0x59), latin, false), (Glyph::Space, true));
        assert!(!enhancement_glyph(EnhChar::G2(0x59), (G0Set::Cyrillic1, G2Set::Cyrillic), false).1, "Latin G2 only");
        for code in G3_UNDEFINED {
            assert_eq!(enhancement_glyph(EnhChar::G3(code), latin, false), (Glyph::Space, true));
        }
        assert!(is_text(enhancement_glyph(EnhChar::G2(0x41), latin, false).0));
        assert!(!is_text(enhancement_glyph(EnhChar::G3(0x41), latin, false).0));
    }

    /// REQ-FSM-009, REQ-FSM-011: every row of the walk's table, checked for
    /// its next state and action.
    #[test]
    fn every_transition_of_the_walk() {
        use TripletFunction as F;
        use WalkAction as A;
        use WalkEvent as E;
        use WalkState as S;
        let colour = ColourData { index: 3, high: 0 };
        let number = ObjectNumber(0x15);
        let pending = S::ModifierPending { row_offset: 2, column_offset: 56 };
        let invocation = F::ObjectInvocation { source: ObjectSource::Local, kind: ObjectKind::Passive, number };
        let column = F::G0Character { column: 4, code: b'A' };
        let attribute = F::ForegroundColour { column: 4, colour };
        let rows: [(S, E, S, A); 24] = [
            (S::Start, E::Begin { display: true }, S::Walking, A::BeginLocal),
            (S::Start, E::Begin { display: false }, S::PlacingDefaults, A::None),
            (S::Walking, E::Triplet(None), S::Walking, A::Skip),
            (S::Walking, E::Triplet(Some(F::Termination { kind: 7 })), S::Returning, A::EndSource),
            (S::Walking, E::EndOfData, S::Returning, A::EndSource),
            (
                S::Walking,
                E::Triplet(Some(F::ObjectDefinition {
                    usage: crate::packet::ObjectUsage(3),
                    kind: ObjectKind::Active,
                    number,
                })),
                S::Returning,
                A::EndSource,
            ),
            (
                S::Walking,
                E::Triplet(Some(F::FullRowColour { address: 45, colour })),
                S::Walking,
                A::RowColour { row: 5, colour },
            ),
            (
                S::Walking,
                E::Triplet(Some(F::SetActivePosition { address: 40, column: 7 })),
                S::Walking,
                A::Position { row: 24, column: 7 },
            ),
            (
                S::Walking,
                E::Triplet(Some(F::AddressDisplayRow0 { address: 63, colour })),
                S::Walking,
                A::RowColour { row: 0, colour },
            ),
            (S::Walking, E::Triplet(Some(F::AddressDisplayRow0 { address: 50, colour })), S::Walking, A::None),
            (S::Walking, E::Triplet(Some(F::FullScreenColour(colour))), S::Walking, A::ScreenColour(colour)),
            (
                S::Walking,
                E::Triplet(Some(F::DrcsMode { normal: true, levels: 3, subtable: 2 })),
                S::Walking,
                A::DrcsMode { normal: true, levels: 3, subtable: 2 },
            ),
            (S::Walking, E::Triplet(Some(F::OriginModifier { row_offset: 2, column_offset: 56 })), pending, A::None),
            (
                pending,
                E::Triplet(Some(invocation)),
                S::Walking,
                A::Invoke {
                    source: ObjectSource::Local,
                    kind: ObjectKind::Passive,
                    number,
                    row_offset: 2,
                    column_offset: 56,
                },
            ),
            (pending, E::Triplet(Some(column)), S::Walking, A::Column(column)),
            (pending, E::Triplet(None), S::Walking, A::Skip),
            (
                S::Walking,
                E::Triplet(Some(invocation)),
                S::Walking,
                A::Invoke {
                    source: ObjectSource::Local,
                    kind: ObjectKind::Passive,
                    number,
                    row_offset: 0,
                    column_offset: 0,
                },
            ),
            (S::Walking, E::Triplet(Some(F::ReservedRow { mode: 2, address: 41, data: 0 })), S::Walking, A::None),
            (S::Walking, E::Triplet(Some(F::ProgrammeDeliveryColumn { column: 3, data: 0 })), S::Walking, A::None),
            (S::Walking, E::Triplet(Some(attribute)), S::Walking, A::Column(attribute)),
            (S::Returning, E::Returned(Ended::Object), S::Walking, A::Pop),
            (S::Returning, E::Returned(Ended::Local), S::Finished, A::None),
            (S::Returning, E::Returned(Ended::Default), S::PlacingDefaults, A::ResetPosition),
            (S::PlacingDefaults, E::DefaultObject { found: true }, S::Walking, A::StartDefault),
        ];
        for (state, event, next, action) in rows {
            assert_eq!(walk_transition(state, event), (next, action), "{state:?} {event:?}");
        }
        assert_eq!(walk_transition(S::PlacingDefaults, E::DefaultObject { found: false }), (S::Finished, A::None));
        // Events that cannot arrive change nothing.
        assert_eq!(walk_transition(S::Finished, E::Begin { display: true }), (S::Finished, A::None));
        assert_eq!(walk_transition(S::Returning, E::EndOfData), (S::Returning, A::None));
    }

    /// Annex C.2 rule 3: the later of two changes at one cell wins, type by
    /// type.
    #[test]
    fn changes_at_one_cell() {
        let first = AttrChange { foreground: Some(1), background: Some(2), ..AttrChange::default() };
        let second = AttrChange { foreground: Some(3), ..AttrChange::default() };
        let both = first.then(second);
        assert_eq!((both.foreground, both.background), (Some(3), Some(2)));
    }

    /// REQ-L25-025: Annex C.3 on one segment: lower halves claimed for the
    /// next row, and no doubling where it cannot be shown.
    #[test]
    fn sizes_on_a_segment() {
        let attrs = Attrs { double_height: true, double_width: true, ..Attrs::PASSIVE };
        let resolved = cell_of(Glyph::Space, &attrs, CharSource::Local, 0);
        let mut cells = RowCells::new();
        cells.resolved[38] = Some(resolved);
        cells.resolved[39] = Some(resolved);
        cells.place(38, 40, true);
        let placed = |cells: &RowCells, column: usize| cells.cells[column].unwrap();
        assert_eq!(placed(&cells, 38).size, Size::DoubleTop);
        assert_eq!(placed(&cells, 38).attributes.width(), Width::LeftHalf);
        assert_eq!(placed(&cells, 39).attributes.width(), Width::RightHalf);
        assert!(cells.claims[38] && cells.claims[39]);
        let mut cells = RowCells::new();
        cells.resolved[39] = Some(resolved);
        cells.place(0, 40, false);
        assert_eq!((placed(&cells, 39).size, placed(&cells, 39).attributes.width()), (Size::Normal, Width::Normal));
    }
}
