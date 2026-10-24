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
//!   cells of each side panel.
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

use crate::acq::{EnhancementLookup, EnhancementReport, PageBuild, StoredPage};
use crate::charset::{G0, G0Set, G2Set};
use crate::colour::{ColourIndex, PagePresentation, SidePanels};
use crate::config::{Config, PresentationLevel};
use crate::consts::{
    LOCAL_TRIPLETS, OBJECT_BODY_TRIPLETS, OBJECT_INVOCATIONS_PER_TYPE, OBJECT_TYPES, ROWS, SCREEN_COLUMNS,
};
use crate::level1::{Cell, DisplayOptions};
use crate::objects::{Body, EnhancementLinks};
use crate::packet::DisplayAttributes;
use crate::types::FixedVec;

#[cfg(feature = "additional-flash-functions")]
mod additional_flash;
mod attributes;
mod characters;
mod machine;
mod page;
mod planes;
mod row;
mod sizes;
mod walk;

use attributes::Flash;
use characters::EnhChar;
use planes::{Area, Plane};
use row::Resolved;

pub use machine::{
    Ended, WalkAction, WalkEvent, WalkState, walk_action_name, walk_event_name, walk_state_name, walk_transition,
};
pub use page::{apply, apply_traced};

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
    designation: Option<(G0Set, G2Set)>,
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

#[cfg(test)]
mod tests;
