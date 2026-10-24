//! Right-to-left pages: lines in visual order by the Unicode Bidirectional
//! Algorithm, brackets mirrored and lam-alef joined, broken into rows
//! (REQ-CHR-015).

use super::Sets;
use super::WriteError;
use super::sets::{Cell, encode};
use neoteletext_core::consts::COLUMNS;

/// Lam followed by alef, written as the sets' one lam-alef code.
const LAM: char = '\u{0644}';
const ALEF: char = '\u{0627}';
const LAM_ALEF: char = '\u{FEFB}';

/// The brackets a right-to-left run shows mirrored (Unicode
/// `BidiMirroring.txt`, the pairs the sets hold).
const MIRRORED: [(char, char); 8] =
    [('(', ')'), (')', '('), ('<', '>'), ('>', '<'), ('[', ']'), (']', '['), ('«', '»'), ('»', '«')];

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
pub(super) fn break_line_rtl(sets: &Sets, text: &str, line: usize) -> Result<Vec<Vec<Cell>>, WriteError> {
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
