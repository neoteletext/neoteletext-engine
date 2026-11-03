//! Text runs at their places on a page (REQ-2214, REQ-2148): each run a
//! row, a column, its Unicode text and its language, written over the
//! codes a program already gave the page, with the X/26 triplets the runs
//! need merged into the program's own in EN 300 706 Annex A.4's order, and
//! the page's designation for its header and X/28/0 Format 1. The Rust
//! server encodes a channel program's text with it (DEC-2199).
//!
//! A run never takes a cell its text does not show: no ESC is written. For
//! each run the writer uses, as REQ-2214 item 5 orders them:
//! 1. the page's default set, in the row's own codes, its marked letters
//!    by modes 10000 to 11111 and its G2 characters by mode 01111;
//! 2. on a page for NeoTeletext's own receivers, a row set triplet (column
//!    mode 01010, REQ-2180) naming the run's set at its first cell, the
//!    run's characters in the row's own codes, and a row set triplet back
//!    to the page's set at the cell after it when the row shows anything
//!    after it;
//! 3. on a page for Level 3.5, a Modified G0 and G2 Character Set
//!    Designation (column mode 01000) at its first cell and every
//!    character as a triplet of its own: mode 01001, or a marked letter or
//!    G2 character of the designated sets, over a space in the row; and a
//!    designation back to the page's sets after it, as in 2.
//!
//! Right-to-left runs are put in visual order and Arabic letters of Table
//! 44 take their joined forms, as the page writer does; a Thai or Lao run
//! writes its marks in the same columns of the rows above and below
//! (DEC-604), which runs of the same set may share where no column needs
//! two marks.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use neoteletext_core::charset::language::{LANGUAGES, language, language_value};
use neoteletext_core::charset::{CharSet, G0, G0Set, default_second_value, modified_designation};
use neoteletext_core::consts::{
    COLUMNS, ESC, LOCAL_TRIPLETS, MODE_ADDRESS_ROW_0, MODE_DRCS_CHARACTER, MODE_FULL_ROW_COLOUR,
    MODE_FULL_SCREEN_COLOUR, MODE_G0_LEVEL_25, MODE_G0_NO_DIACRITIC, MODE_G2, MODE_G3_LEVEL_15, MODE_G3_LEVEL_25,
    MODE_MODIFIED_DESIGNATION, MODE_ROW_PDC_FIRST, MODE_ROW_PDC_LAST, MODE_ROW_SET, MODE_SET_ACTIVE_POSITION,
    MODE_TERMINATION, NO_SECOND_G0, ROW_ADDRESS_BASE, ROW_ADDRESS_LAST, ROW_ADDRESS_ROW_24, ROWS, SPACE,
    TERMINATION_TYPE_MASK,
};
use neoteletext_core::packet::{Triplet, TripletFunction};
use neoteletext_core::types::Designations;

use super::sets::{Cell, State, encode_from, page_sets};
use super::thai_lao::{LOWER_MARKS, clusters, mark_cell};
use super::{Level, Sets, WriteError, kana, rtl, table44};
use crate::consts::TERMINATION_LOCAL_DATA_END;

/// The language of a run that names none on a page that names none:
/// Latin English (REQ-2214 item 4).
pub const LATIN_ENGLISH_TAG: &str = "en";
/// The first row a run may be on: row 0 is the header.
pub const FIRST_TEXT_ROW: u8 = 1;
/// The last row a run may be on, 24, the Fastext row.
pub const LAST_TEXT_ROW: u8 = 24;
/// Line numbers in the page writer's refusals: a run is one line.
const LINE: usize = 1;
/// The last code a row shows as itself; 7/F is the block in every set.
const LAST_PRINTABLE: u8 = 0x7E;

/// One run of text: where it starts and what it says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextRun<'a> {
    /// Its row, 1 to 24.
    pub row: u8,
    /// Its first column, 0 to 39.
    pub column: u8,
    /// Its text, in Unicode normalisation form C.
    pub text: &'a str,
    /// Its language, a BCP 47 tag of [`LANGUAGES`]; none for the page's.
    pub language: Option<&'a str>,
}

/// A page's text: its default language or designation, its level and its
/// runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PageText<'a> {
    /// The page's language, which sets its default designation; none to
    /// keep `designation`.
    pub language: Option<&'a str>,
    /// The page's Table 32 value when it names no language.
    pub designation: u8,
    /// The level it is written for (REQ-2151).
    pub level: Level,
    /// The runs, in order.
    pub runs: &'a [TextRun<'a>],
}

/// What one run cost (REQ-2216 item 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Report {
    /// The cells it takes on its row.
    pub cells: u8,
    /// The triplets it added to the page's local enhancement data, its
    /// share of row addresses and of the Termination Marker included.
    pub triplets: usize,
    /// 1 when its marks take the rows above and below it, else 0.
    pub mark_rows: u8,
}

/// The page with its runs written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Encoded {
    /// The page's local enhancement data, the program's and the runs'
    /// merged, through its Termination Marker, not padded; when `changed`
    /// is false, the program's triplets as given.
    pub triplets: Vec<Triplet>,
    /// Whether the runs added any triplet: when not, the program's packets
    /// X/26 stand as they were sent.
    pub changed: bool,
    /// The triplets the page's local enhancement data uses (REQ-2149).
    pub count: usize,
    /// The page's default and second G0 designations (Tables 32 and 33).
    pub designations: Designations,
    /// What each run cost, in order.
    pub runs: Vec<Report>,
}

/// Why runs could not be written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RunError {
    /// The run at fault, by its index; none for the page as a whole.
    pub run: Option<usize>,
    /// What is wrong.
    pub fault: RunFault,
}

/// What is wrong with a run or the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RunFault {
    /// No set writes the language at the page's level.
    UnknownLanguage(String),
    /// The run is not on rows 1 to 24 or starts after column 39.
    OutOfRows,
    /// The run's text does not end by column 39: its last column.
    OutOfColumns(usize),
    /// Its marks would be on row 0 or row 25.
    MarkRowsOutOfRange,
    /// It takes a cell another run takes: that run's index.
    Overlap(usize),
    /// No means the level allows shows this character in the language.
    Unshown(char),
    /// The program's own triplets select a character at a cell of the run.
    CellTaken {
        /// The row.
        row: u8,
        /// The column.
        column: u8,
    },
    /// The program's own codes or triplets change the set in force before
    /// or within the run on its row: an ESC, or a triplet of mode 01000
    /// or 01010.
    SetChanged {
        /// The row.
        row: u8,
        /// The column of the ESC or triplet.
        column: u8,
    },
    /// The page's triplets, the program's and the runs' together, are more
    /// than the 208 of sixteen packets X/26: how many.
    OverBudget(usize),
    /// The program's local enhancement data holds objects, or triplets
    /// whose order text cannot be merged into.
    ProgramObjects,
}

impl core::fmt::Display for RunFault {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownLanguage(tag) => write!(formatter, "no set writes the language {tag:?} at this level"),
            Self::OutOfRows => formatter.write_str("a run is on rows 1 to 24 and starts by column 39"),
            Self::OutOfColumns(last) => write!(formatter, "the run ends at column {last}, after column 39"),
            Self::MarkRowsOutOfRange => {
                formatter.write_str("its marks need the rows above and below it, within rows 1 to 24")
            }
            Self::Overlap(other) => write!(formatter, "it takes a cell run {other} takes"),
            Self::Unshown(character) => {
                write!(
                    formatter,
                    "the page cannot show {character:?} (U+{:04X}) in the run's language",
                    u32::from(*character)
                )
            }
            Self::CellTaken { row, column } => {
                write!(formatter, "the page's own triplets select a character at row {row}, column {column}")
            }
            Self::SetChanged { row, column } => {
                write!(formatter, "the page's own codes or triplets change the set at row {row}, column {column}")
            }
            Self::OverBudget(count) => {
                write!(formatter, "{count} triplets; sixteen packets X/26 carry {LOCAL_TRIPLETS}")
            }
            Self::ProgramObjects => {
                formatter.write_str("the page's own enhancement data holds objects, which text cannot be merged into")
            }
        }
    }
}

/// Which way a language is written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    /// Left to right.
    Ltr,
    /// Right to left.
    Rtl,
}

/// One language a page or run may name (REQ-2215).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Language {
    /// The BCP 47 tag that names it.
    pub tag: String,
    /// Its English name.
    pub name: &'static str,
    /// Which way it is written.
    pub direction: Direction,
    /// 1 when its marks take the rows above and below its letters.
    pub mark_rows: u8,
}

/// Every language of the core's table (REQ-2181), in its order, as a page
/// or run names it.
#[must_use]
pub fn languages() -> Vec<Language> {
    LANGUAGES
        .iter()
        .map(|entry| {
            let (language, script) = entry.tag_parts();
            let mut tag = language.to_string();
            if let Some(script) = script {
                tag.push('-');
                tag.push_str(script);
            }
            let direction = if entry.right_to_left() { Direction::Rtl } else { Direction::Ltr };
            Language { tag, name: entry.name, direction, mark_rows: entry.mark_rows() }
        })
        .collect()
}

/// Writes the `page`'s runs into `grid`, the program's 25 rows of codes,
/// over the cells they take, and merges the triplets they need with the
/// program's local enhancement data `program` (its packets X/26 in order
/// of designation code). Deterministic (REQ-2214 item 9). On an error the
/// grid may hold some runs and is to be dropped.
///
/// # Errors
///
/// [`RunError`] naming the run at fault, or none for the page.
pub fn write_runs(
    grid: &mut [[u8; COLUMNS]; ROWS],
    page: &PageText<'_>,
    program: &[Triplet],
) -> Result<Encoded, RunError> {
    let own = page.level == Level::NeoTeletext;
    let page_value = match page.language {
        Some(tag) => language_value(tag, own)
            .ok_or_else(|| RunError { run: None, fault: RunFault::UnknownLanguage(tag.to_string()) })?,
        None => page.designation,
    };
    let designations =
        Designations { default: page_value, second: default_second_value(page_value).unwrap_or(NO_SECOND_G0) };
    let parsed = parse(program);
    let original = *grid;
    let mut placed: Vec<Placed> = Vec::with_capacity(page.runs.len());
    for (index, run) in page.runs.iter().enumerate() {
        let at = |fault| RunError { run: Some(index), fault };
        let one = encode_run(run, page.language, page_value, own).map_err(at)?;
        check_place(&one, &placed, parsed.as_ref().ok(), &original).map_err(at)?;
        placed.push(one);
    }
    let program_count = local_length(program);
    let mut reports = Vec::with_capacity(placed.len());
    let mut before = program_count;
    let mut merged: Option<Vec<Triplet>> = None;
    let mut marked = [[false; COLUMNS]; ROWS];
    for count in 1..=placed.len() {
        let index = count.saturating_sub(1);
        if let Some(run) = placed.get(index) {
            write_cells(grid, &mut marked, run, own);
        }
        let prefix = placed.get(..count).unwrap_or_default();
        let rows = encoder_rows(grid, prefix, page_value, own, parsed.as_ref().ok());
        let now = if rows.iter().all(Vec::is_empty) {
            merged = None;
            program_count
        } else {
            let Ok(program_data) = parsed.as_ref() else {
                return Err(RunError { run: Some(index), fault: RunFault::ProgramObjects });
            };
            let stream = merge(program_data, &rows);
            let length = stream.len();
            merged = Some(stream);
            length
        };
        if now > usize::from(LOCAL_TRIPLETS) {
            return Err(RunError { run: Some(index), fault: RunFault::OverBudget(now) });
        }
        let mark_rows = placed.get(index).map_or(0, |run| run.mark_rows);
        let cells = placed.get(index).map_or(0, |run| u8::try_from(run.letters.len()).unwrap_or(u8::MAX));
        reports.push(Report { cells, triplets: now.saturating_sub(before), mark_rows });
        before = now;
    }
    let (triplets, changed, count) = match merged {
        Some(stream) => {
            let length = stream.len();
            (stream, true, length)
        }
        None => (program.to_vec(), false, program_count),
    };
    Ok(Encoded { triplets, changed, count, designations, runs: reports })
}

/// The triplets of local enhancement data that count against the budget:
/// through the first Termination Marker, or all of them when there is none.
fn local_length(program: &[Triplet]) -> usize {
    program.iter().position(|&triplet| is_termination(triplet)).map_or(program.len(), |place| place.saturating_add(1))
}

fn is_termination(triplet: Triplet) -> bool {
    triplet.address == ROW_ADDRESS_LAST && triplet.mode == MODE_TERMINATION
}

// ------------------------------------------------------- one run encoded

/// One run, encoded but not yet written.
struct Placed {
    row: u8,
    column: u8,
    /// Its set's Table 32 value.
    value: u8,
    /// Its set is not the page's default.
    other: bool,
    /// Its cells on its row.
    letters: Vec<Cell>,
    /// Its marks above and below, by their place in the run.
    upper: Vec<(usize, Cell)>,
    lower: Vec<(usize, Cell)>,
    /// 1 when its marks take the rows above and below.
    mark_rows: u8,
}

impl Placed {
    /// One past its last column.
    fn end(&self) -> usize {
        usize::from(self.column).saturating_add(self.letters.len())
    }

    /// The rows its mark spans are on.
    fn mark_spans(&self) -> [u8; 2] {
        [self.row.saturating_sub(1), self.row.saturating_add(1)]
    }
}

fn unshown(error: WriteError) -> RunFault {
    match error {
        WriteError::Missing { character, .. } => RunFault::Unshown(character),
        WriteError::UnknownSet(name) | WriteError::UnknownLanguage(name) => RunFault::UnknownLanguage(name),
        // The run encoder makes no rows and builds no page.
        _ => RunFault::OutOfColumns(COLUMNS),
    }
}

/// The sets a run is written in: its set's own, with no second set, since
/// no ESC is written; on a page for Level 3.5 a run in another set than the
/// page's is reached by Modified Designation, which takes no national
/// option (clause 15.4, note).
fn run_sets(value: u8, other: bool, own: bool) -> Option<Sets> {
    let mut sets = page_sets(value, false)?;
    if other && !own {
        let (g0, g2) = modified_designation(value)?;
        let base = CharSet::G0(G0 { set: g0, national: None });
        sets.default = base;
        sets.base = base;
        sets.g2 = g2;
    }
    sets.second = None;
    sets.second_value = None;
    Some(sets)
}

fn encode_run(run: &TextRun<'_>, page_language: Option<&str>, page_value: u8, own: bool) -> Result<Placed, RunFault> {
    if !(FIRST_TEXT_ROW..=LAST_TEXT_ROW).contains(&run.row) || usize::from(run.column) >= COLUMNS {
        return Err(RunFault::OutOfRows);
    }
    let tag = run.language.or(page_language).unwrap_or(LATIN_ENGLISH_TAG);
    let unknown = || RunFault::UnknownLanguage(tag.to_string());
    let value = language_value(tag, own).ok_or_else(unknown)?;
    let entry = language(tag).ok_or_else(unknown)?;
    let other = value != page_value;
    let sets = run_sets(value, other, own).ok_or_else(unknown)?;
    let set = match sets.base {
        CharSet::G0(G0 { set, .. }) => Some(set),
        CharSet::G2(_) | CharSet::G3 => None,
    };
    let text = kana::normalise(run.text);
    let text = if set == Some(G0Set::Arabic) {
        table44::join(&text, |character| sets.holds(character) || sets.g2_holds(character))
    } else {
        text
    };
    let mark_rows = entry.mark_rows();
    let mut placed = Placed {
        row: run.row,
        column: run.column,
        value,
        other,
        letters: Vec::new(),
        upper: Vec::new(),
        lower: Vec::new(),
        mark_rows,
    };
    if matches!(set, Some(G0Set::Thai | G0Set::Lao)) {
        for (letter, marks) in clusters(&text, LINE).map_err(unshown)? {
            let (cells, _) = encode_from(&sets, &[letter], State::default(), false, LINE).map_err(unshown)?;
            placed.letters.extend(cells);
            let place = placed.letters.len().saturating_sub(1);
            let (lower, upper): (Vec<char>, Vec<char>) = marks.iter().partition(|mark| LOWER_MARKS.contains(mark));
            if !upper.is_empty() {
                placed.upper.push((place, mark_cell(&sets, &upper, LINE).map_err(unshown)?));
            }
            if !lower.is_empty() {
                placed.lower.push((place, mark_cell(&sets, &lower, LINE).map_err(unshown)?));
            }
        }
    } else {
        let right_to_left = set.is_some_and(|set| set.is_joined() || set == G0Set::Hebrew || set == G0Set::Arabic);
        let characters: Vec<char> = if right_to_left { rtl::visual_line(&text) } else { text.chars().collect() };
        placed.letters = encode_from(&sets, &characters, State::default(), false, LINE).map_err(unshown)?.0;
    }
    let end = placed.end();
    if end > COLUMNS {
        return Err(RunFault::OutOfColumns(end.saturating_sub(1)));
    }
    if mark_rows > 0 && (run.row <= FIRST_TEXT_ROW || run.row >= LAST_TEXT_ROW) {
        return Err(RunFault::MarkRowsOutOfRange);
    }
    Ok(placed)
}

// ------------------------------------------------------- where runs go

/// Who holds a cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Holder {
    run: usize,
    /// For a mark span, its set's value; none for a run's own cells.
    marks: Option<u8>,
    /// A mark is written here.
    mark: bool,
}

fn grid_cell<T: Copy>(grid: &[[T; COLUMNS]; ROWS], row: u8, column: usize) -> Option<T> {
    grid.get(usize::from(row)).and_then(|cells| cells.get(column)).copied()
}

/// Checks that `run` takes no cell an earlier run takes, and that the
/// program's own codes and triplets leave its cells and its set alone.
fn check_place(
    run: &Placed,
    earlier: &[Placed],
    program: Option<&ProgramData>,
    original: &[[u8; COLUMNS]; ROWS],
) -> Result<(), RunFault> {
    let mut holders = vec![[None::<Holder>; COLUMNS]; ROWS];
    let hold = |holders: &mut Vec<[Option<Holder>; COLUMNS]>, row: u8, column: usize, holder: Holder| {
        if let Some(slot) = holders.get_mut(usize::from(row)).and_then(|cells| cells.get_mut(column)) {
            *slot = Some(holder);
        }
    };
    for (index, other) in earlier.iter().enumerate() {
        for column in usize::from(other.column)..other.end() {
            hold(&mut holders, other.row, column, Holder { run: index, marks: None, mark: false });
        }
        if other.mark_rows > 0 {
            let [above, below] = other.mark_spans();
            for column in usize::from(other.column)..other.end() {
                let marked = |marks: &[(usize, Cell)]| {
                    marks.iter().any(|(place, _)| usize::from(other.column).saturating_add(*place) == column)
                };
                for (row, marks) in [(above, &other.upper), (below, &other.lower)] {
                    let before = holders.get(usize::from(row)).and_then(|cells| cells.get(column)).copied().flatten();
                    let mark = marked(marks) || before.is_some_and(|holder| holder.mark);
                    hold(&mut holders, row, column, Holder { run: index, marks: Some(other.value), mark });
                }
            }
        }
    }
    for column in usize::from(run.column)..run.end() {
        if let Some(holder) = holders.get(usize::from(run.row)).and_then(|cells| cells.get(column)).copied().flatten() {
            return Err(RunFault::Overlap(holder.run));
        }
    }
    if run.mark_rows > 0 {
        let [above, below] = run.mark_spans();
        for (row, marks) in [(above, &run.upper), (below, &run.lower)] {
            for column in usize::from(run.column)..run.end() {
                let Some(holder) = holders.get(usize::from(row)).and_then(|cells| cells.get(column)).copied().flatten()
                else {
                    continue;
                };
                let mark = marks.iter().any(|(place, _)| usize::from(run.column).saturating_add(*place) == column);
                if holder.marks != Some(run.value) || (mark && holder.mark) {
                    return Err(RunFault::Overlap(holder.run));
                }
            }
        }
    }
    let mut rows = vec![run.row];
    if run.mark_rows > 0 {
        rows.extend(run.mark_spans());
    }
    for row in rows {
        // An ESC before the run would leave its codes in the second set.
        if let Some(column) = (0..usize::from(run.column)).find(|&column| grid_cell(original, row, column) == Some(ESC))
        {
            return Err(RunFault::SetChanged { row, column: u8::try_from(column).unwrap_or(u8::MAX) });
        }
        let Some(program) = program else { continue };
        for &(column, triplet) in
            program.rows.get(usize::from(row)).map(|section| section.columns.as_slice()).unwrap_or_default()
        {
            let at = usize::from(column);
            if at < run.end() && is_set_change(triplet.mode) {
                return Err(RunFault::SetChanged { row, column });
            }
            if (usize::from(run.column)..run.end()).contains(&at) && is_character(triplet.mode) {
                return Err(RunFault::CellTaken { row, column });
            }
        }
    }
    Ok(())
}

/// Column modes that select a character at their cell (Table 29).
fn is_character(mode: u8) -> bool {
    matches!(mode, MODE_G3_LEVEL_15 | MODE_G0_LEVEL_25 | MODE_G3_LEVEL_25 | MODE_DRCS_CHARACTER | MODE_G2)
        || mode >= MODE_G0_NO_DIACRITIC
}

/// Column modes that change the set the row's codes or its X/26
/// characters are read in.
fn is_set_change(mode: u8) -> bool {
    matches!(mode, MODE_MODIFIED_DESIGNATION | MODE_ROW_SET)
}

// ---------------------------------------------- what runs write and cost

/// Whether the run's characters are written by triplets of their own: a
/// run in another set on a page for Level 3.5.
fn by_triplets(run: &Placed, own: bool) -> bool {
    run.other && !own
}

/// The row's code for a cell, and the triplet that writes its character
/// when the row's code cannot (its mode and data).
fn cell_parts(cell: Cell, by_triplets: bool) -> (u8, Option<(u8, u8)>) {
    let (code, triplet) = match cell {
        Cell::Code(code) | Cell::Run { code, .. } => (code, by_triplets.then_some((MODE_G0_LEVEL_25, code))),
        Cell::Marked { code, mark } => (code, Some((MODE_G0_NO_DIACRITIC | mark, code))),
        Cell::G2 { stand_in, code } => (stand_in, Some((MODE_G2, code))),
    };
    // A character written by a triplet in a set the row cannot read has a
    // space beneath it.
    (if by_triplets { SPACE } else { code }, triplet)
}

/// The cells of `run`'s mark rows, by row and column.
fn mark_cells(run: &Placed) -> Vec<(u8, usize, Cell)> {
    let [above, below] = run.mark_spans();
    let at = |place: usize| usize::from(run.column).saturating_add(place);
    run.upper
        .iter()
        .map(|&(place, cell)| (above, at(place), cell))
        .chain(run.lower.iter().map(|&(place, cell)| (below, at(place), cell)))
        .collect()
}

/// Writes `run`'s cells into the grid: its own, its mark spans as spaces
/// where no earlier run of its set wrote a mark (`marked`), then its marks.
fn write_cells(grid: &mut [[u8; COLUMNS]; ROWS], marked: &mut [[bool; COLUMNS]; ROWS], run: &Placed, own: bool) {
    let triplets = by_triplets(run, own);
    let mut put = |row: u8, column: usize, code: u8| {
        if let Some(slot) = grid.get_mut(usize::from(row)).and_then(|cells| cells.get_mut(column)) {
            *slot = code;
        }
    };
    for (column, &cell) in (usize::from(run.column)..).zip(&run.letters) {
        put(run.row, column, cell_parts(cell, triplets).0);
    }
    if run.mark_rows == 0 {
        return;
    }
    for row in run.mark_spans() {
        for column in usize::from(run.column)..run.end() {
            if grid_cell(marked, row, column) == Some(false) {
                put(row, column, SPACE);
            }
        }
    }
    for (row, column, cell) in mark_cells(run) {
        put(row, column, cell_parts(cell, triplets).0);
        if let Some(slot) = marked.get_mut(usize::from(row)).and_then(|cells| cells.get_mut(column)) {
            *slot = true;
        }
    }
}

/// One triplet the runs need on a row, in the order it goes: at a column,
/// set changes before characters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Needed {
    column: u8,
    /// A change of set, which goes before the program's triplets at its
    /// column; a character goes after them.
    switch: bool,
    triplet: Triplet,
}

/// The triplets the runs `placed` need, by row, in column order.
fn encoder_rows(
    grid: &[[u8; COLUMNS]; ROWS],
    placed: &[Placed],
    page_value: u8,
    own: bool,
    program: Option<&ProgramData>,
) -> Vec<Vec<Needed>> {
    let mut rows: Vec<Vec<Needed>> = vec![Vec::new(); ROWS];
    // The set each column needs, by row; none where the page's set serves.
    let mut need = vec![[None::<u8>; COLUMNS]; ROWS];
    let mut characters: Vec<(u8, u8, (u8, u8))> = Vec::new();
    for run in placed {
        let triplets = by_triplets(run, own);
        let mut spans = vec![run.row];
        if run.mark_rows > 0 {
            spans.extend(run.mark_spans());
        }
        if run.other {
            for &row in &spans {
                for column in usize::from(run.column)..run.end() {
                    if let Some(slot) = need.get_mut(usize::from(row)).and_then(|cells| cells.get_mut(column)) {
                        *slot = Some(run.value);
                    }
                }
            }
        }
        let own_cells =
            (usize::from(run.column)..).zip(run.letters.iter().copied()).map(|(column, cell)| (run.row, column, cell));
        for (row, column, cell) in own_cells.chain(mark_cells(run)) {
            if let (Some(triplet), Ok(column)) = (cell_parts(cell, triplets).1, u8::try_from(column)) {
                characters.push((row, column, triplet));
            }
        }
    }
    let switch_mode = if own { MODE_ROW_SET } else { MODE_MODIFIED_DESIGNATION };
    for (row, needs) in (0u8..).zip(&need) {
        let Some(out) = rows.get_mut(usize::from(row)) else { continue };
        let mut current = page_value;
        for (column, &needed) in (0u8..).zip(needs) {
            let wanted = match needed {
                Some(value) => Some(value),
                None if current != page_value && shows_after(grid, program, row, usize::from(column), needs) => {
                    Some(page_value)
                }
                None => None,
            };
            if let Some(value) = wanted.filter(|&value| value != current) {
                let triplet = if own {
                    TripletFunction::ReservedColumn { column, mode: switch_mode, data: value }.to_triplet()
                } else {
                    TripletFunction::ModifiedDesignation { column, designation: value }.to_triplet()
                };
                out.push(Needed { column, switch: true, triplet });
                current = value;
            }
        }
        let mut here: Vec<Needed> = characters
            .iter()
            .filter(|(at, ..)| *at == row)
            .map(|&(_, column, (mode, data))| Needed {
                column,
                switch: false,
                triplet: Triplet { address: column, mode, data },
            })
            .collect();
        out.append(&mut here);
        out.sort_by_key(|needed| (needed.column, !needed.switch));
    }
    rows
}

/// Whether the row shows anything from `column` up to the next run in a
/// set of its own that a change of set could alter: a printable code
/// other than a space, or a character the program's triplets place.
fn shows_after(
    grid: &[[u8; COLUMNS]; ROWS],
    program: Option<&ProgramData>,
    row: u8,
    column: usize,
    needs: &[Option<u8>; COLUMNS],
) -> bool {
    let until = (column..COLUMNS).find(|&at| needs.get(at).copied().flatten().is_some()).unwrap_or(COLUMNS);
    let codes =
        (column..until).any(|at| grid_cell(grid, row, at).is_some_and(|code| code > SPACE && code <= LAST_PRINTABLE));
    let placed = program.and_then(|program| program.rows.get(usize::from(row))).is_some_and(|section| {
        section
            .columns
            .iter()
            .any(|&(at, triplet)| (column..until).contains(&usize::from(at)) && is_character(triplet.mode))
    });
    codes || placed
}

// ------------------------------------------------- the program's triplets

/// The program's triplets for one row: the row address triplets that
/// start it, and its column triplets in their order.
#[derive(Clone, Debug, Default)]
struct Section {
    addresses: Vec<Triplet>,
    columns: Vec<(u8, Triplet)>,
}

/// The program's local enhancement data, by row.
#[derive(Clone, Debug)]
struct ProgramData {
    /// Triplets before the first that moves the active position to a row.
    prefix: Vec<Triplet>,
    rows: Vec<Section>,
    /// Its Termination Marker, if it has one.
    termination: Option<Triplet>,
}

/// The page row a row address names: 41 to 63 rows 1 to 23, and 40 row 24
/// (clause 12.3.1, Table 27).
fn row_of(address: u8) -> u8 {
    if address == ROW_ADDRESS_ROW_24 { LAST_TEXT_ROW } else { address.saturating_sub(ROW_ADDRESS_BASE) }
}

/// The row address of a page row.
fn address_of(row: u8) -> u8 {
    if row == LAST_TEXT_ROW { ROW_ADDRESS_ROW_24 } else { ROW_ADDRESS_BASE.saturating_add(row) }
}

/// The program's data by row, or `Err` when it holds objects or other
/// triplets whose place text cannot be merged around: object invocations
/// and definitions, the origin modifier, and reserved row modes.
fn parse(program: &[Triplet]) -> Result<ProgramData, ()> {
    let mut data = ProgramData { prefix: Vec::new(), rows: vec![Section::default(); ROWS], termination: None };
    let mut row: Option<u8> = None;
    for (index, &triplet) in program.iter().enumerate() {
        if is_termination(triplet) {
            // Only repeated markers, the filler of the last packet, may
            // follow; anything else is a definition of an object.
            let rest = program.get(index.saturating_add(1)..).unwrap_or_default();
            if rest.iter().any(|&after| after != triplet)
                || triplet.data & TERMINATION_TYPE_MASK != TERMINATION_LOCAL_DATA_END
            {
                return Err(());
            }
            data.termination = Some(triplet);
            break;
        }
        if triplet.is_row_address() {
            let moves = match triplet.mode {
                MODE_SET_ACTIVE_POSITION | MODE_FULL_ROW_COLOUR => Some(row_of(triplet.address)),
                MODE_ADDRESS_ROW_0 => Some(0),
                MODE_FULL_SCREEN_COLOUR => None,
                mode if (MODE_ROW_PDC_FIRST..=MODE_ROW_PDC_LAST).contains(&mode) => None,
                _ => return Err(()),
            };
            if let Some(moved) = moves {
                row = Some(moved);
            }
            match row.and_then(|row| data.rows.get_mut(usize::from(row))) {
                Some(section) => section.addresses.push(triplet),
                None => data.prefix.push(triplet),
            }
            continue;
        }
        match row.and_then(|row| data.rows.get_mut(usize::from(row))) {
            Some(section) => section.columns.push((triplet.address, triplet)),
            None => data.prefix.push(triplet),
        }
    }
    Ok(data)
}

/// The program's data and the runs' needs in one stream, in Annex A.4's
/// order: row by row from the top, each row's address triplets, then its
/// column triplets by column, the runs' set changes before the program's
/// triplets at a column and the runs' characters after them; then the
/// Termination Marker.
fn merge(program: &ProgramData, encoder: &[Vec<Needed>]) -> Vec<Triplet> {
    let mut out = program.prefix.clone();
    for (row, section) in (0u8..).zip(&program.rows) {
        let needs = encoder.get(usize::from(row)).map(Vec::as_slice).unwrap_or_default();
        if section.addresses.is_empty() && section.columns.is_empty() && needs.is_empty() {
            continue;
        }
        if section.addresses.is_empty() {
            out.push(TripletFunction::SetActivePosition { address: address_of(row), column: 0 }.to_triplet());
        } else {
            out.extend_from_slice(&section.addresses);
        }
        let mut needs = needs.iter().peekable();
        for &(column, triplet) in &section.columns {
            while let Some(needed) =
                needs.next_if(|needed| needed.column < column || (needed.switch && needed.column == column))
            {
                out.push(needed.triplet);
            }
            out.push(triplet);
        }
        out.extend(needs.map(|needed| needed.triplet));
    }
    out.push(
        program
            .termination
            .unwrap_or_else(|| TripletFunction::Termination { kind: TERMINATION_LOCAL_DATA_END }.to_triplet()),
    );
    out
}

/// The program's local enhancement data packed into packets X/26 of
/// thirteen triplets, designation codes from 0, the last packet filled
/// with its Termination Marker (clause 12.3.1, Annex A.3).
#[must_use]
pub fn packets(triplets: &[Triplet]) -> Vec<[Triplet; neoteletext_core::consts::TRIPLETS_PER_PACKET]> {
    let filler = triplets.last().copied().unwrap_or_default();
    triplets
        .chunks(neoteletext_core::consts::TRIPLETS_PER_PACKET)
        .map(|chunk| {
            let mut packet = [filler; neoteletext_core::consts::TRIPLETS_PER_PACKET];
            for (slot, &triplet) in packet.iter_mut().zip(chunk) {
                *slot = triplet;
            }
            packet
        })
        .collect()
}

/// Writes `designations` into the triplets of a packet X/28/0 Format 1:
/// the default G0 and G2 designation in triplet 1 bits 8 to 14, the
/// second G0 designation in triplet 1 bits 15 to 18 and triplet 2 bits 1
/// to 3 (clause 9.4.2, Table 4), every other bit as it was.
pub fn designate(triplets: &mut [Triplet; neoteletext_core::consts::TRIPLETS_PER_PACKET], designations: Designations) {
    use neoteletext_core::consts::{
        DESIGNATION_DEFAULT_SHIFT, DESIGNATION_MASK, DESIGNATION_SECOND_HIGH_MASK, DESIGNATION_SECOND_LOW_BITS,
        DESIGNATION_SECOND_LOW_SHIFT, NIBBLE_MASK,
    };
    let second = u32::from(designations.second);
    let low_mask = u32::from(NIBBLE_MASK) << DESIGNATION_SECOND_LOW_SHIFT;
    let default_mask = DESIGNATION_MASK << DESIGNATION_DEFAULT_SHIFT;
    if let Some(first) = triplets.first_mut() {
        let value = first.value() & !(default_mask | low_mask)
            | (u32::from(designations.default) & DESIGNATION_MASK) << DESIGNATION_DEFAULT_SHIFT
            | (second & u32::from(NIBBLE_MASK)) << DESIGNATION_SECOND_LOW_SHIFT;
        *first = Triplet::from_value(value);
    }
    if let Some(next) = triplets.get_mut(1) {
        let value = next.value() & !DESIGNATION_SECOND_HIGH_MASK
            | (second >> DESIGNATION_SECOND_LOW_BITS) & DESIGNATION_SECOND_HIGH_MASK;
        *next = Triplet::from_value(value);
    }
}

#[cfg(test)]
mod tests;
