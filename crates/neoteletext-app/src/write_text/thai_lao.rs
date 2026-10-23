//! Thai and Lao pages in the Philips SAA5246AP/S scheme (decision E.80):
//! letter rows with their marks on the rows above and below.

use super::sets::{Cell, LAST_CODE, encode};
use super::{Sets, WriteError};
use neoteletext_core::charset::{CharRef, CharSet, to_unicode};
use neoteletext_core::consts::{COLUMNS, ESC, SPACE};

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

/// One line of a Thai or Lao page: its letter row, and the marks above
/// and below by column.
pub(super) struct MarkedLine {
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
pub(super) fn marked_lines(sets: &Sets, text: &str, line: usize) -> Result<Vec<MarkedLine>, WriteError> {
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
pub(super) fn marked_rows(lines: Vec<MarkedLine>) -> Vec<Vec<Cell>> {
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
