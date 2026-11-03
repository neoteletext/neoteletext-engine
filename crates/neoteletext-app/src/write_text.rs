//! Writing a page from text in one of neoteletext's own character sets
//! (REQ-CLI-010, decision E.82: rdm REQ-95 and DEC-606).
//!
//! Each character is written, in order of preference, as:
//! 1. a code of the current G0 set in the row;
//! 2. after an ESC, a code of the second G0 set, Latin English (REQ-CHR-012);
//! 3. a G0 letter in the row with its mark as an X/26 "G0 character with
//!    diacritical mark" triplet (REQ-CHR-014);
//! 4. a G2 character as an X/26 triplet, over a stand-in in the row.
//!
//! 5. on a page for NeoTeletext's own receivers, a row set triplet (REQ-2180)
//!    starting a run in the set that holds the most of what follows, and
//!    in place of an ESC that no space can take (REQ-2148, REQ-2188).
//!
//! Anything else is refused, naming the character and its line. Half-width
//! katakana and kana with combining voicing marks are taken as the
//! full-width voiced kana (REQ-2188). The standard's Arabic set of Table 44
//! is written with each letter in its joined form, chosen here (REQ-2190).
//!
//! The Arabic-script sets of group 1001 are written in visual order, as
//! Table 44's rows are: each line is broken at the row width, then
//! reordered right to left by the Unicode Bidirectional Algorithm, with
//! runs of digits and Latin left to right and brackets mirrored. لا is
//! written as the lam-alef code. The receiver joins the letters
//! (REQ-CHR-015).
//!
//! Thai and Lao follow the Philips SAA5246AP/S scheme (decision E.80,
//! rdm DEC-604): each line takes a row of
//! letters, with the marks above them on the row before and the lower
//! vowels on the row after, each in its letter's column; a line's lower
//! vowels share a row with the next line's upper marks unless a column
//! needs both. A vowel with a tone is one G2 glyph placed by X/26 over the
//! vowel alone; ำ is written as ํ over the consonant and า beside it.

use neoteletext_core::charset::language::language_value;
use neoteletext_core::charset::{CharSet, G0, G0Set, G2Set};
use neoteletext_core::consts::{COLUMNS, MODE_ROW_SET, NATIONAL_INDEX_MASK, SPACE};
use neoteletext_core::packet::TripletFunction;
use neoteletext_core::types::Designations;
use neoteletext_core::types::SubCode;
use neoteletext_formats::author::{self, AuthorError, DisplayPage, Stream, neutral_fields};
use rtl::break_line_rtl;
use sets::{Cell, break_line, page_sets, set_value};
use thai_lao::{marked_lines, marked_rows};

mod kana;
mod rtl;
mod sets;
mod table44;
mod thai_lao;

pub use sets::SETS;

/// The first display row the writer fills; row 0 is the header.
const FIRST_ROW: u8 = 1;
/// The last display row the writer fills: row 24 is left for Fastext
/// prompts (clause 9.3.1).
const LAST_ROW: u8 = 23;

/// What the page's sets can write.
struct Sets {
    /// The page's Table 32 value.
    value: u8,
    default: CharSet,
    second: Option<CharSet>,
    /// The Table 33 value of the second set.
    second_value: Option<u8>,
    /// The G0 set without national option, whose base forms X/26 marks
    /// are placed on (REQ-CHR-002).
    base: CharSet,
    g2: G2Set,
    /// Row set triplets may be written: the page is for NeoTeletext's own
    /// receivers (REQ-2151).
    runs: bool,
}

/// How a page's set is named.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice<'a> {
    /// One of [`SETS`].
    Set(&'a str),
    /// A BCP 47 language tag, resolved by the core's table of languages
    /// (REQ-2181).
    Language(&'a str),
}

/// The presentation level a page is written for (REQ-2151).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Level {
    /// NeoTeletext's own receivers: Level 3.5 with NeoTeletext's own
    /// extensions, the row set triplet among them.
    #[default]
    NeoTeletext,
    /// Level 3.5 and below: no row set triplets, and no set that needs
    /// NeoTeletext's own receiver.
    Standard,
}

/// Why a page could not be written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WriteError {
    /// The set is not one of [`SETS`], or this build lacks it.
    UnknownSet(String),
    /// The core's table of languages has no set for the tag at this level.
    UnknownLanguage(String),
    /// The set cannot write a character: the character and its line,
    /// counted from 1.
    Missing {
        /// The character.
        character: char,
        /// Its line.
        line: usize,
    },
    /// The text needs more rows than rows 1 to 23.
    TooManyRows(usize),
    /// A Thai or Lao word on this line is wider than a row.
    WordTooLong(usize),
    /// The page builder refused the page; for diacritical marks this is
    /// more triplets than the sixteen packets X/26 carry.
    Author(AuthorError),
}

impl core::fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownSet(name) => write!(formatter, "unknown set {name:?}; the sets are {}", set_names()),
            Self::UnknownLanguage(tag) => {
                write!(formatter, "no set writes the language {tag:?} on a page for this presentation level")
            }
            Self::Missing { character, line } => {
                write!(formatter, "line {line}: the set has no {character:?} (U+{:04X})", u32::from(*character))
            }
            Self::TooManyRows(rows) => write!(formatter, "the text needs {rows} rows; a page has rows 1 to 23"),
            Self::WordTooLong(line) => write!(formatter, "line {line}: a word is wider than a row"),
            Self::Author(AuthorError::LocalDataFull) => {
                formatter.write_str("the marks need more triplets than the sixteen packets X/26 carry")
            }
            Self::Author(error) => write!(formatter, "the page could not be built: {error:?}"),
        }
    }
}

impl core::error::Error for WriteError {}

/// The names of [`SETS`], for messages.
#[must_use]
pub fn set_names() -> String {
    SETS.iter().map(|(name, ..)| *name).collect::<Vec<_>>().join(", ")
}

/// A written page: the TTI file and what it used.
#[derive(Clone, Debug)]
pub struct Written {
    /// The page as a TTI file (REQ-INP-023).
    pub tti: Vec<u8>,
    /// Display rows used.
    pub rows: usize,
    /// X/26 triplets written, row addresses included.
    pub triplets: usize,
}

/// Writes `text` as page `mpp` in the set `choice` names, for `level`
/// (REQ-CLI-010, REQ-2151).
///
/// # Errors
///
/// [`WriteError`] when the set or language is unknown, a character cannot
/// be written, or the page is full.
pub fn write_text(choice: Choice<'_>, level: Level, mpp: u16, text: &str) -> Result<Written, WriteError> {
    let own_level = level == Level::NeoTeletext;
    let value = match choice {
        Choice::Set(name) => set_value(name).ok_or_else(|| WriteError::UnknownSet(name.to_string()))?,
        Choice::Language(tag) => {
            language_value(tag, own_level).ok_or_else(|| WriteError::UnknownLanguage(tag.to_string()))?
        }
    };
    let unknown = || match choice {
        Choice::Set(name) => WriteError::UnknownSet(name.to_string()),
        Choice::Language(tag) => WriteError::UnknownLanguage(tag.to_string()),
    };
    let sets = page_sets(value, own_level).ok_or_else(unknown)?;
    let text = kana::normalise(text);
    let text = if matches!(sets.default, CharSet::G0(G0 { set: G0Set::Arabic, .. })) {
        table44::join(&text, |character| sets.holds(character))
    } else {
        text
    };
    let text = text.as_str();
    let mut rows = Vec::new();
    let right_to_left = matches!(
        sets.default,
        CharSet::G0(G0 { set, .. }) if set.is_joined() || set == G0Set::Hebrew || set == G0Set::Arabic
    );
    let marked = matches!(sets.default, CharSet::G0(G0 { set: G0Set::Thai | G0Set::Lao, .. }));
    if marked {
        let mut lines = Vec::new();
        for (index, line) in text.lines().enumerate() {
            lines.extend(marked_lines(&sets, line, index.saturating_add(1))?);
        }
        rows = marked_rows(lines);
    }
    for (index, line) in text.lines().enumerate().filter(|_| !marked) {
        let number = index.saturating_add(1);
        rows.extend(if right_to_left {
            break_line_rtl(&sets, line, number)?
        } else {
            break_line(&sets, line, number)?
        });
    }
    // A right-to-left row starts at the right edge.
    if right_to_left {
        for row in &mut rows {
            let indent = COLUMNS.saturating_sub(row.len());
            row.splice(0..0, core::iter::repeat_n(Cell::Code(SPACE), indent));
        }
    }
    while rows.last().is_some_and(Vec::is_empty) {
        rows.pop();
    }
    let available = usize::from(LAST_ROW - FIRST_ROW + 1);
    if rows.len() > available {
        return Err(WriteError::TooManyRows(rows.len()));
    }
    let mut page = DisplayPage::new(author::address(mpp).map_err(WriteError::Author)?, SubCode::new(0));
    // C12 to C14 and X/28/0 Format 1 carry the designation, with the second
    // set that ESC reaches (REQ-CHR-012).
    page.national(value & NATIONAL_INDEX_MASK);
    let mut fields = neutral_fields(value);
    let second = sets.second_value.unwrap_or(neoteletext_core::consts::NO_SECOND_G0);
    fields.designations = Some(Designations { default: value, second });
    page.page_data(fields);
    let mut triplets = 0usize;
    for (row, cells) in (FIRST_ROW..).zip(&rows) {
        let codes: Vec<u8> = cells
            .iter()
            .map(|cell| match *cell {
                Cell::Code(code) | Cell::Marked { code, .. } | Cell::Run { code, .. } => code,
                Cell::G2 { stand_in, .. } => stand_in,
            })
            .collect();
        page.text(row, 0, &codes).map_err(WriteError::Author)?;
        let placed: Vec<(u8, Cell)> =
            (0u8..).zip(cells.iter().copied()).filter(|(_, cell)| !matches!(cell, Cell::Code(_))).collect();
        if placed.is_empty() {
            continue;
        }
        page.local().position(row, 0).map_err(WriteError::Author)?;
        triplets = triplets.saturating_add(1);
        for (column, cell) in placed {
            match cell {
                Cell::Marked { code, mark } => page.local().diacritic(column, code, mark),
                Cell::G2 { code, .. } => page.local().g2(column, code),
                Cell::Run { value, .. } => {
                    page.local().function(TripletFunction::ReservedColumn { column, mode: MODE_ROW_SET, data: value })
                }
                Cell::Code(_) => continue,
            };
            triplets = triplets.saturating_add(1);
        }
    }
    let built = page.build().map_err(WriteError::Author)?;
    let mut stream = Stream::default();
    stream.push(built);
    let tti = stream.tti().map_err(|_| WriteError::Author(AuthorError::PageFull))?;
    Ok(Written { tti, rows: rows.len(), triplets })
}

#[cfg(test)]
mod tests;
