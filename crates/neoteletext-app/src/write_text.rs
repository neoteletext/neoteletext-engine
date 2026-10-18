//! Writing a page from text in one of neoteletext's own character sets
//! (REQ-CLI-010, decision E.82, `docs/spec/design/languages.md` section 9).
//!
//! Each character is written, in order of preference, as:
//! 1. a code of the current G0 set in the row;
//! 2. after an ESC, a code of the second G0 set, Latin English (REQ-CHR-012);
//! 3. a G0 letter in the row with its mark as an X/26 "G0 character with
//!    diacritical mark" triplet (REQ-CHR-014);
//! 4. a G2 character as an X/26 triplet, over a stand-in in the row.
//!
//! Anything else is refused, naming the character and its line.
//!
//! The Arabic-script sets of group 1001 are written in visual order, as
//! Table 44's rows are: each line is broken at the row width, then
//! reordered right to left by the Unicode Bidirectional Algorithm, with
//! runs of digits and Latin left to right and brackets mirrored. لا is
//! written as the lam-alef code. The receiver joins the letters
//! (REQ-CHR-015).
//!
//! Thai and Lao follow the Philips SAA5246AP/S scheme (decision E.80,
//! `docs/spec/design/languages.md` section 7.5): each line takes a row of
//! letters, with the marks above them on the row before and the lower
//! vowels on the row after, each in its letter's column; a line's lower
//! vowels share a row with the next line's upper marks unless a column
//! needs both. A vowel with a tone is one G2 glyph placed by X/26 over the
//! vowel alone; ำ is written as ํ over the consonant and า beside it.

use neoteletext_core::charset::{
    CharRef, CharSet, G0, G0Set, G2Set, compose, default_second_g0, designation, page_mark, to_unicode,
};
use neoteletext_core::consts::{
    COLUMNS, DESIGNATION_GROUP_SHIFT, ESC, MODE_DIACRITIC_MASK, NATIONAL_INDEX_MASK, SPACE,
};
use neoteletext_core::types::Designations;
use neoteletext_core::types::SubCode;
use neoteletext_formats::author::{self, AuthorError, DisplayPage, Stream, neutral_fields};

/// The sets of REQ-CHR-012 the writer names, with their Table 32 groups
/// and options.
pub const SETS: [(&str, u8, u8); 28] = [
    ("hausa-fula", 0b0101, 0b000),
    ("west-african", 0b0101, 0b001),
    ("ewe", 0b0101, 0b010),
    ("kabyle-tamazight", 0b0101, 0b011),
    ("azerbaijani", 0b0101, 0b100),
    ("kanuri-mossi", 0b0101, 0b101),
    ("yoruba-igbo", 0b0101, 0b110),
    ("vietnamese", 0b0101, 0b111),
    ("ukrainian", 0b0111, 0b000),
    ("belarusian", 0b0111, 0b001),
    ("macedonian", 0b0111, 0b010),
    ("serbian", 0b0111, 0b011),
    ("kyrgyz-mongolian", 0b0111, 0b101),
    ("tatar", 0b0111, 0b110),
    ("tajik", 0b0111, 0b111),
    ("georgian", 0b1011, 0b000),
    ("armenian", 0b1011, 0b001),
    ("tifinagh", 0b1011, 0b010),
    ("persian", 0b1001, 0b000),
    ("urdu", 0b1001, 0b001),
    ("pashto", 0b1001, 0b010),
    ("sindhi", 0b1001, 0b011),
    ("sorani", 0b1001, 0b100),
    ("uyghur", 0b1001, 0b101),
    ("balochi", 0b1001, 0b110),
    ("arabic", 0b1001, 0b111),
    ("thai", 0b1100, 0b010),
    ("lao", 0b1100, 0b100),
];

/// Thai and Lao marks written below their letter; every other mark is
/// written above.
const LOWER_MARKS: [char; 6] = ['\u{0E38}', '\u{0E39}', '\u{0E3A}', '\u{0EB8}', '\u{0EB9}', '\u{0EBC}'];
/// The tone marks and the thanthakhat, written after a vowel sign in a
/// two-mark glyph.
const SECOND_MARKS: [char; 9] =
    ['\u{0E48}', '\u{0E49}', '\u{0E4A}', '\u{0E4B}', '\u{0E4C}', '\u{0EC8}', '\u{0EC9}', '\u{0ECA}', '\u{0ECB}'];
/// Thai sara am, written as nikhahit over its consonant and sara aa.
const SARA_AM: char = '\u{0E33}';
const NIKHAHIT: char = '\u{0E4D}';
const SARA_AA: char = '\u{0E32}';
/// The first Thai digit; Thai digits are written as Western digits, which
/// the Thai set has (section 7.3).
const THAI_DIGIT_ZERO: char = '\u{0E50}';
const THAI_DIGITS: core::ops::RangeInclusive<char> = '\u{0E50}'..='\u{0E59}';

/// True for a Thai or Lao combining mark (Unicode general category Mn in
/// the two blocks).
fn is_mark(character: char) -> bool {
    matches!(character,
        '\u{0E31}' | '\u{0E34}'..='\u{0E3A}' | '\u{0E47}'..='\u{0E4E}'
        | '\u{0EB1}' | '\u{0EB4}'..='\u{0EBC}' | '\u{0EC8}'..='\u{0ECD}')
}

/// Lam followed by alef, written as the sets' one lam-alef code.
const LAM: char = '\u{0644}';
const ALEF: char = '\u{0627}';
const LAM_ALEF: char = '\u{FEFB}';

/// The brackets a right-to-left run shows mirrored (Unicode
/// `BidiMirroring.txt`, the pairs the sets hold).
const MIRRORED: [(char, char); 8] =
    [('(', ')'), (')', '('), ('<', '>'), ('>', '<'), ('[', ']'), (']', '['), ('«', '»'), ('»', '«')];

/// The first display row the writer fills; row 0 is the header.
const FIRST_ROW: u8 = 1;
/// The last display row the writer fills: row 24 is left for Fastext
/// prompts (clause 9.3.1).
const LAST_ROW: u8 = 23;
/// The last character code of a set; 7/F is the block in every G0 set
/// (clause 15.6) and is never written from text.
const LAST_CODE: u8 = 0x7E;
/// The highest diacritical mark number (Table 29, modes 10001 to 11111).
const LAST_MARK: u8 = MODE_DIACRITIC_MASK;

/// Why a page could not be written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WriteError {
    /// The set is not one of [`SETS`], or this build lacks it.
    UnknownSet(String),
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

/// One cell: its row code and, for a character the row cannot show, the
/// triplet that places it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    Code(u8),
    Marked { code: u8, mark: u8 },
    G2 { stand_in: u8, code: u8 },
}

/// What the page's sets can write.
struct Sets {
    default: CharSet,
    second: Option<CharSet>,
    /// The G0 set without national option, whose base forms X/26 marks
    /// are placed on (REQ-CHR-002).
    base: CharSet,
    g2: G2Set,
}

impl Sets {
    fn code_in(set: CharSet, character: char) -> Option<u8> {
        (SPACE..=LAST_CODE).find(|&code| to_unicode(CharRef::new(set, code)).base == character)
    }

    /// The letter and mark whose composition is `character`, with the
    /// letter the same in the row as in the base forms.
    fn marked(&self, character: char) -> Option<Cell> {
        (SPACE..=LAST_CODE).find_map(|code| {
            let base = to_unicode(CharRef::new(self.base, code)).base;
            if to_unicode(CharRef::new(self.default, code)).base != base {
                return None;
            }
            (1..=LAST_MARK).find_map(|mark| {
                let placed = CharRef { diacritic: page_mark(self.g2, mark), ..CharRef::new(self.base, code) };
                let combining = to_unicode(placed).combining?;
                (compose(base, combining) == Some(character)).then_some(Cell::Marked { code, mark })
            })
        })
    }

    fn g2(&self, character: char) -> Option<Cell> {
        let code = Self::code_in(CharSet::G2(self.g2), character)?;
        let stand_in = stand_in(character).and_then(|letter| Self::code_in(self.default, letter)).unwrap_or(SPACE);
        Some(Cell::G2 { stand_in, code })
    }
}

/// The Latin letter shown at Level 1 for a G2 letter, where one is plain.
fn stand_in(character: char) -> Option<char> {
    const STAND_INS: [(char, char); 10] = [
        ('Đ', 'D'),
        ('đ', 'd'),
        ('Ŋ', 'N'),
        ('ŋ', 'n'),
        ('Ø', 'O'),
        ('ø', 'o'),
        ('Ł', 'L'),
        ('ł', 'l'),
        ('ı', 'i'),
        ('ß', 's'),
    ];
    STAND_INS.iter().find(|(letter, _)| *letter == character).map(|(_, plain)| *plain)
}

/// Adds an ESC. It shows as a space (REQ-CHR-009), so it takes the place
/// of a space just written rather than a cell of its own.
fn switch(cells: &mut Vec<Cell>) {
    if cells.last() == Some(&Cell::Code(SPACE)) {
        cells.pop();
    }
    cells.push(Cell::Code(ESC));
}

/// Encodes `characters` from the set state `in_second`, giving the cells
/// and the state after them.
fn encode(sets: &Sets, characters: &[char], mut in_second: bool, line: usize) -> Result<(Vec<Cell>, bool), WriteError> {
    let mut cells = Vec::with_capacity(characters.len());
    for &character in characters {
        let current = if in_second { sets.second.unwrap_or(sets.default) } else { sets.default };
        if let Some(code) = Sets::code_in(current, character) {
            cells.push(Cell::Code(code));
            continue;
        }
        let other = if in_second { Some(sets.default) } else { sets.second };
        if let Some(code) = other.and_then(|set| Sets::code_in(set, character)) {
            switch(&mut cells);
            cells.push(Cell::Code(code));
            in_second = !in_second;
            continue;
        }
        // X/26 characters are placed over the default set's row.
        if in_second {
            switch(&mut cells);
            in_second = false;
        }
        match sets.marked(character).or_else(|| sets.g2(character)) {
            Some(cell) => cells.push(cell),
            None => return Err(WriteError::Missing { character, line }),
        }
    }
    Ok((cells, in_second))
}

/// Breaks one line of text into rows of at most 40 cells, at spaces where
/// it can.
fn break_line(sets: &Sets, text: &str, line: usize) -> Result<Vec<Vec<Cell>>, WriteError> {
    let mut rows = Vec::new();
    let mut row: Vec<Cell> = Vec::new();
    let mut in_second = false;
    for word in text.split(' ') {
        let mut characters: Vec<char> = word.chars().collect();
        if !row.is_empty() {
            characters.insert(0, ' ');
        }
        let (cells, after) = encode(sets, &characters, in_second, line)?;
        if row.len().saturating_add(cells.len()) <= COLUMNS {
            row.extend(cells);
            in_second = after;
            continue;
        }
        if !row.is_empty() {
            rows.push(core::mem::take(&mut row));
        }
        // A new row starts in the default set (REQ-CHR-009).
        let word: Vec<char> = word.chars().collect();
        let (cells, after) = encode(sets, &word, false, line)?;
        in_second = after;
        let mut cells = cells.into_iter().peekable();
        while cells.peek().is_some() {
            row.extend(cells.by_ref().take(COLUMNS.saturating_sub(row.len())));
            if cells.peek().is_some() {
                rows.push(core::mem::take(&mut row));
            }
        }
    }
    rows.push(row);
    Ok(rows)
}

/// `text` in visual order for a right-to-left page: the Unicode
/// Bidirectional Algorithm with a right-to-left paragraph, brackets in
/// right-to-left runs mirrored.
fn visual(text: &str) -> Vec<char> {
    use unicode_bidi::{BidiInfo, Level};
    let info = BidiInfo::new(text, Some(Level::rtl()));
    let Some(paragraph) = info.paragraphs.first() else { return Vec::new() };
    let (levels, runs) = info.visual_runs(paragraph, paragraph.range.clone());
    let mut out = Vec::with_capacity(text.len());
    for run in runs {
        let rtl = levels.get(run.start).is_some_and(Level::is_rtl);
        let piece = text.get(run).unwrap_or_default();
        if rtl {
            out.extend(
                piece.chars().rev().map(|c| MIRRORED.iter().find(|(from, _)| *from == c).map_or(c, |(_, to)| *to)),
            );
        } else {
            out.extend(piece.chars());
        }
    }
    out
}

/// Breaks one line of a right-to-left page into rows of at most 40 cells,
/// each in visual order.
fn break_line_rtl(sets: &Sets, text: &str, line: usize) -> Result<Vec<Vec<Cell>>, WriteError> {
    let text = text.replace(&[LAM, ALEF][..].iter().collect::<String>(), &LAM_ALEF.to_string());
    let mut rows = Vec::new();
    let mut words: Vec<&str> = Vec::new();
    let mut accepted: Option<Vec<Cell>> = None;
    for word in text.split(' ') {
        words.push(word);
        let candidate = encode(sets, &visual(&words.join(" ")), false, line)?.0;
        if candidate.len() <= COLUMNS {
            accepted = Some(candidate);
            continue;
        }
        words.pop();
        if let Some(row) = accepted.take() {
            rows.push(row);
        }
        words.clear();
        words.push(word);
        let alone = encode(sets, &visual(word), false, line)?.0;
        if alone.len() > COLUMNS {
            // A word longer than a row is cut, in visual order.
            let mut cells = alone.into_iter().peekable();
            while cells.peek().is_some() {
                rows.push(cells.by_ref().take(COLUMNS).collect());
            }
            words.clear();
        } else {
            accepted = Some(alone);
        }
    }
    rows.push(accepted.unwrap_or_default());
    Ok(rows)
}

/// One line of a Thai or Lao page: its letter row, and the marks above
/// and below by column.
struct MarkedLine {
    letters: Vec<Cell>,
    upper: Vec<(usize, Cell)>,
    lower: Vec<(usize, Cell)>,
}

/// The cell of one or two marks: a code of the default set, or a G2
/// glyph placed by X/26 over the first mark alone (REQ-CHR-016).
fn mark_cell(sets: &Sets, marks: &[char], line: usize) -> Result<Cell, WriteError> {
    let missing = |character: char| WriteError::Missing { character, line };
    let g2 = CharSet::G2(sets.g2);
    match *marks {
        [mark] => Sets::code_in(sets.default, mark)
            .map(Cell::Code)
            .or_else(|| Sets::code_in(g2, mark).map(|code| Cell::G2 { stand_in: SPACE, code }))
            .ok_or_else(|| missing(mark)),
        [first, second] => {
            let code = (SPACE..=LAST_CODE)
                .find(|&code| {
                    let unicode = to_unicode(CharRef::new(g2, code));
                    unicode.base == first && unicode.combining == Some(second)
                })
                .ok_or_else(|| missing(second))?;
            Ok(Cell::G2 { stand_in: Sets::code_in(sets.default, first).unwrap_or(SPACE), code })
        }
        [.., extra] => Err(missing(extra)),
        [] => Ok(Cell::Code(SPACE)),
    }
}

/// A Thai or Lao line in clusters: each spacing character with the marks
/// that follow it, sara am split and Thai digits made Western.
fn clusters(text: &str, line: usize) -> Result<Vec<(char, Vec<char>)>, WriteError> {
    let mut out: Vec<(char, Vec<char>)> = Vec::new();
    for character in text.chars() {
        if is_mark(character) {
            let Some((_, marks)) = out.last_mut() else { return Err(WriteError::Missing { character, line }) };
            marks.push(character);
        } else if character == SARA_AM {
            let Some((_, marks)) = out.last_mut() else { return Err(WriteError::Missing { character, line }) };
            marks.push(NIKHAHIT);
            out.push((SARA_AA, Vec::new()));
        } else if THAI_DIGITS.contains(&character) {
            let value = u32::from(character).saturating_sub(u32::from(THAI_DIGIT_ZERO));
            out.push((char::from_digit(value, 10).unwrap_or(' '), Vec::new()));
        } else {
            out.push((character, Vec::new()));
        }
    }
    // A vowel sign comes before a tone mark in a two-mark glyph.
    for (_, marks) in &mut out {
        marks.sort_by_key(|mark| SECOND_MARKS.contains(mark));
    }
    Ok(out)
}

/// Lays out one input line of a Thai or Lao page as lines of at most 40
/// letter cells, broken at spaces.
fn marked_lines(sets: &Sets, text: &str, line: usize) -> Result<Vec<MarkedLine>, WriteError> {
    let empty = || MarkedLine { letters: Vec::new(), upper: Vec::new(), lower: Vec::new() };
    let mut lines = Vec::new();
    let mut current = empty();
    let mut in_second = false;
    for word in text.split(' ') {
        let word_clusters = clusters(word, line)?;
        // Try the word on the current line, then on a line of its own.
        for attempt in 0..2 {
            let mut letters = current.letters.clone();
            let mut state = in_second;
            let mut placed = Vec::new();
            let space = (!letters.is_empty()).then_some((' ', Vec::new()));
            for (letter, marks) in space.iter().chain(&word_clusters) {
                let (cells, after) = encode(sets, &[*letter], state, line)?;
                state = after;
                // An ESC takes the place of a space before it (REQ-CHR-009).
                if cells.first() == Some(&Cell::Code(ESC)) && letters.last() == Some(&Cell::Code(SPACE)) {
                    letters.pop();
                }
                letters.extend(cells);
                placed.push((letters.len().saturating_sub(1), marks.clone()));
            }
            if letters.len() > COLUMNS {
                if attempt == 0 && !current.letters.is_empty() {
                    lines.push(core::mem::replace(&mut current, empty()));
                    in_second = false;
                    continue;
                }
                return Err(WriteError::WordTooLong(line));
            }
            current.letters = letters;
            in_second = state;
            for (column, marks) in placed {
                let (lower, upper): (Vec<char>, Vec<char>) = marks.iter().partition(|mark| LOWER_MARKS.contains(mark));
                if !upper.is_empty() {
                    current.upper.push((column, mark_cell(sets, &upper, line)?));
                }
                if !lower.is_empty() {
                    current.lower.push((column, mark_cell(sets, &lower, line)?));
                }
            }
            break;
        }
    }
    lines.push(current);
    Ok(lines)
}

/// A row of 40 spaces with the given cells placed.
fn mark_row(cells: &[(usize, Cell)]) -> Vec<Cell> {
    let mut row = vec![Cell::Code(SPACE); COLUMNS];
    for &(column, cell) in cells {
        if let Some(slot) = row.get_mut(column) {
            *slot = cell;
        }
    }
    row
}

/// The rows of a Thai or Lao page (section 7.5): each line's mark row and
/// letter row, a line's lower vowels sharing the next line's mark row
/// unless a column needs both.
fn marked_rows(lines: Vec<MarkedLine>) -> Vec<Vec<Cell>> {
    let mut rows = Vec::new();
    let mut pending: Vec<(usize, Cell)> = Vec::new();
    for line in lines {
        let clash = pending.iter().any(|(column, _)| line.upper.iter().any(|(other, _)| other == column));
        if clash {
            rows.push(mark_row(&pending));
            rows.push(mark_row(&line.upper));
        } else {
            pending.extend(line.upper);
            rows.push(mark_row(&pending));
        }
        rows.push(line.letters);
        pending = line.lower;
    }
    if !pending.is_empty() {
        rows.push(mark_row(&pending));
    }
    rows
}

/// The page's sets for the set named `name`.
fn page_sets(name: &str) -> Result<(u8, Sets), WriteError> {
    let unknown = || WriteError::UnknownSet(name.to_string());
    let &(_, group, option) = SETS.iter().find(|(set, ..)| *set == name).ok_or_else(unknown)?;
    let value = (group << DESIGNATION_GROUP_SHIFT) | option;
    let found = designation(value).ok_or_else(unknown)?;
    let sets = Sets {
        default: CharSet::G0(found.g0),
        second: default_second_g0(value).map(CharSet::G0),
        base: CharSet::G0(G0 { set: found.g0.set, national: None }),
        g2: found.g2,
    };
    Ok((value, sets))
}

/// Writes `text` as page `mpp` in the set named `name` (REQ-CLI-010).
///
/// # Errors
///
/// [`WriteError`] when the set is unknown, a character cannot be written,
/// or the page is full.
pub fn write_text(name: &str, mpp: u16, text: &str) -> Result<Written, WriteError> {
    let (value, sets) = page_sets(name)?;
    let mut rows = Vec::new();
    let right_to_left = matches!(sets.default, CharSet::G0(G0 { set, .. }) if set.is_joined());
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
    // C12 to C14 and X/28/0 Format 1 carry the designation, with Latin
    // English as the second set that ESC reaches (REQ-CHR-012).
    page.national(value & NATIONAL_INDEX_MASK);
    let mut fields = neutral_fields(value);
    let second = if sets.second.is_some() { 0 } else { neoteletext_core::consts::NO_SECOND_G0 };
    fields.designations = Some(Designations { default: value, second });
    page.page_data(fields);
    let mut triplets = 0usize;
    for (row, cells) in (FIRST_ROW..).zip(&rows) {
        let codes: Vec<u8> = cells
            .iter()
            .map(|cell| match *cell {
                Cell::Code(code) | Cell::Marked { code, .. } => code,
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// What a decoder shows for cells written in `name`'s sets, ESC as a
    /// space (REQ-CHR-009): codes through the set in force, marks composed
    /// and G2 characters from the page's G2 set.
    fn shown(name: &str, cells: &[Cell]) -> String {
        let (_, sets) = page_sets(name).unwrap();
        let mut in_second = false;
        cells
            .iter()
            .map(|cell| match *cell {
                Cell::Code(ESC) => {
                    in_second = !in_second;
                    ' '
                }
                Cell::Code(code) => {
                    let set = if in_second { sets.second.unwrap() } else { sets.default };
                    to_unicode(CharRef::new(set, code)).base
                }
                Cell::Marked { code, mark } => {
                    let placed = CharRef { diacritic: page_mark(sets.g2, mark), ..CharRef::new(sets.base, code) };
                    let unicode = to_unicode(placed);
                    compose(unicode.base, unicode.combining.unwrap()).unwrap()
                }
                Cell::G2 { code, .. } => to_unicode(CharRef::new(CharSet::G2(sets.g2), code)).base,
            })
            .collect()
    }

    fn cells(name: &str, text: &str) -> Vec<Cell> {
        let (_, sets) = page_sets(name).unwrap();
        let characters: Vec<char> = text.chars().collect();
        encode(&sets, &characters, false, 1).unwrap().0
    }

    #[test]
    fn every_set_name_designates_its_set() {
        for (name, group, option) in SETS {
            let (value, sets) = page_sets(name).unwrap();
            assert_eq!(value, (group << DESIGNATION_GROUP_SHIFT) | option);
            assert!(sets.second.is_some(), "{name} has English second");
        }
        assert_eq!(page_sets("klingon").err(), Some(WriteError::UnknownSet("klingon".into())));
    }

    /// Each set writes a sample and reads it back unchanged.
    #[test]
    fn samples_round_trip() {
        let samples = [
            ("vietnamese", "Tiếng Việt có dấu, Hà Nội và Đà Nẵng."),
            ("yoruba-igbo", "Ẹ kú àárọ, ọmọ Ṣàngó"),
            ("hausa-fula", "Ɓarna ƙasa ɗaya Ƴan"),
            ("ewe", "Eʋegbe ɖe Ŋkɔ"),
            ("azerbaijani", "Azərbaycan dili Ğ İ ı"),
            ("ukrainian", "Україна, ґанок і Київ"),
            ("serbian", "Србија, Ђорђе и Љубица"),
            ("macedonian", "Македонија Ѓ Ќ Ѕ"),
            ("georgian", "საქართველო"),
            ("armenian", "Հայաստան, Երևան"),
            ("tifinagh", "ⵜⴰⵎⴰⵣⵉⵖⵜ"),
        ];
        for (name, text) in samples {
            let (_, sets) = page_sets(name).unwrap();
            let characters: Vec<char> = text.chars().collect();
            let (written, _) = encode(&sets, &characters, false, 1).unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(shown(name, &written), text, "{name}");
        }
    }

    /// Vietnamese tones go in X/26, the letter in the row.
    #[test]
    fn a_tone_is_a_triplet_over_its_letter() {
        let written = cells("vietnamese", "ệ");
        let [Cell::Marked { code, mark }] = written[..] else { panic!("{written:?}") };
        assert_eq!(to_unicode(CharRef::new(page_sets("vietnamese").unwrap().1.base, code)).base, 'ê');
        assert_eq!(mark, 0x9, "dot below");
        let [Cell::Marked { mark, .. }] = cells("vietnamese", "ả")[..] else { panic!() };
        assert_eq!(mark, 0xC, "the hook above is 4/C of the Vietnamese G2 set");
    }

    /// Latin text on a Cyrillic page is switched with ESC, and each ESC
    /// takes a cell.
    #[test]
    fn latin_text_is_switched_with_esc() {
        let written = cells("ukrainian", "Київ BBC");
        assert_eq!(written.iter().filter(|cell| **cell == Cell::Code(ESC)).count(), 1);
        assert_eq!(written.len(), "Київ BBC".chars().count(), "the ESC takes the space's cell");
        assert_eq!(cells("ukrainian", "BBC").len(), "BBC".len() + 1, "without a space the ESC takes a cell");
    }

    /// A letter with two marks, such as Yoruba ọ̀, needs two marks in one
    /// cell, which X/26 cannot place: the second is refused.
    #[test]
    fn a_second_mark_is_refused() {
        let error = write_text("yoruba-igbo", 0x100, "ọ\u{0300}").unwrap_err();
        assert_eq!(error, WriteError::Missing { character: '\u{0300}', line: 1 });
    }

    #[test]
    fn a_missing_character_is_refused_with_its_line() {
        let error = write_text("georgian", 0x100, "საქართველო\nok ☃").unwrap_err();
        assert_eq!(error, WriteError::Missing { character: '☃', line: 2 });
    }

    #[test]
    fn lines_break_at_the_row_width_and_pages_end_at_row_23() {
        let word = "ა".repeat(10);
        let line = [word.as_str(); 8].join(" ");
        let written = write_text("georgian", 0x100, &line).unwrap();
        // Three words of ten cells and two spaces take 32 cells; a fourth
        // with its space would take 43.
        assert_eq!(written.rows, 3);
        let many = "ა\n".repeat(24);
        assert_eq!(write_text("georgian", 0x100, &many).unwrap_err(), WriteError::TooManyRows(24));
    }

    /// The cells of a right-to-left row as shown, left to right, without
    /// the right-aligning indent.
    fn shown_rtl(name: &str, text: &str) -> String {
        let (_, sets) = page_sets(name).unwrap();
        let rows = break_line_rtl(&sets, text, 1).unwrap();
        shown(name, &rows[0])
    }

    /// REQ-CLI-010: لا is the lam-alef code, and a line is reordered into
    /// visual order with digits and Latin left to right and brackets
    /// mirrored.
    #[test]
    fn arabic_script_is_written_in_visual_order() {
        assert_eq!(shown_rtl("persian", "سلام"), "م\u{FEFB}س");
        // Logical: salam, space, (1405); visual: (۱۴۰۵) on the left with
        // its digits in order and its brackets mirrored back.
        assert_eq!(shown_rtl("persian", "سلام (۱۴۰۵)"), "(۱۴۰۵) م\u{FEFB}س");
        // A Latin run keeps its order. The row starts in the Urdu set, so
        // an ESC (shown as a space) leads into BBC, and the ESC back takes
        // the place of the space between the words.
        assert_eq!(shown_rtl("urdu", "خبر BBC"), " BBC ربخ");
    }

    /// A long right-to-left line breaks between words, and each row is
    /// right-aligned on the page.
    #[test]
    fn right_to_left_rows_break_and_align_right() {
        let word = "ب".repeat(12);
        let line = [word.as_str(); 4].join(" ");
        let written = write_text("persian", 0x100, &line).unwrap();
        assert_eq!(written.rows, 2, "three words of 12 and two spaces fit 40 cells");
    }

    /// The rows of a Thai page as codes, and its X/26 cells.
    fn thai_rows(text: &str) -> Vec<Vec<Cell>> {
        let (_, sets) = page_sets("thai").unwrap();
        let mut lines = Vec::new();
        for (index, line) in text.lines().enumerate() {
            lines.extend(marked_lines(&sets, line, index + 1).unwrap());
        }
        marked_rows(lines)
    }

    fn code(character: char) -> Cell {
        let (_, sets) = page_sets("thai").unwrap();
        Cell::Code(Sets::code_in(sets.default, character).unwrap())
    }

    /// Section 7.5: a mark goes on the row above its consonant, in its
    /// column, and a lower vowel on the row below.
    #[test]
    fn thai_marks_take_rows_of_their_own() {
        let rows = thai_rows("กิ กุ");
        assert_eq!(rows.len(), 3, "marks, letters, lower vowels");
        assert_eq!(rows[0][0], code('\u{0E34}'), "sara i over ko kai");
        assert_eq!(rows[1][..3], [code('ก'), Cell::Code(SPACE), code('ก')]);
        assert_eq!(rows[2][2], code('\u{0E38}'), "sara u under the second ko kai");
    }

    /// A vowel with a tone is one G2 glyph over the vowel alone; ำ is ํ
    /// over its consonant and า beside it; with a tone, ํ and the tone are
    /// one G2 glyph.
    #[test]
    fn thai_two_marks_are_one_g2_glyph() {
        let rows = thai_rows("นี้ น้ำ");
        let Cell::G2 { stand_in, code: g2 } = rows[0][0] else { panic!("{:?}", rows[0][0]) };
        assert_eq!(Cell::Code(stand_in), code('\u{0E35}'), "the vowel alone at Level 1");
        let unicode = to_unicode(CharRef::new(CharSet::G2(G2Set::Thai), g2));
        assert_eq!((unicode.base, unicode.combining), ('\u{0E35}', Some('\u{0E49}')));
        assert_eq!(rows[1][2..4], [code('น'), code('\u{0E32}')], "sara am's aa beside its consonant");
        let Cell::G2 { code: g2, .. } = rows[0][2] else { panic!("{:?}", rows[0][2]) };
        let unicode = to_unicode(CharRef::new(CharSet::G2(G2Set::Thai), g2));
        assert_eq!((unicode.base, unicode.combining), ('\u{0E4D}', Some('\u{0E49}')));
    }

    /// A line's lower vowels share the next line's mark row, unless a
    /// column needs both; then the pair of lines takes a third row.
    #[test]
    fn lower_vowels_share_a_row_unless_a_column_needs_both() {
        assert_eq!(thai_rows("กุ\nขก").len(), 4, "u under ko kai shares the row above line 2");
        let rows = thai_rows("กุ\nกิ");
        assert_eq!(rows.len(), 5, "column 0 needs u and i: a row each");
        assert_eq!(rows[2][0], code('\u{0E38}'));
        assert_eq!(rows[3][0], code('\u{0E34}'));
    }

    /// REQ-CLI-010: a page whose marks exceed the sixteen packets X/26 is
    /// refused.
    #[test]
    fn the_x26_budget_is_enforced() {
        let row = "ả".repeat(COLUMNS);
        let fits = [row.as_str(); 5].join("\n");
        let written = write_text("vietnamese", 0x100, &fits).unwrap();
        assert_eq!(written.triplets, 5 * (COLUMNS + 1));
        let over = [row.as_str(); 6].join("\n");
        assert_eq!(write_text("vietnamese", 0x100, &over).unwrap_err(), WriteError::Author(AuthorError::LocalDataFull));
    }
}
