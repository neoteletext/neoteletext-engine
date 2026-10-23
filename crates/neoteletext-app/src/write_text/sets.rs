//! The set table and the encoding of text into cells of the page's sets:
//! the row's G0 sets, X/26 marks and G2 characters (REQ-CHR-012,
//! REQ-CHR-014), and the breaking of a left-to-right line into rows.

use super::WriteError;
use neoteletext_core::charset::{
    CharRef, CharSet, G0, G2Set, compose, default_second_g0, designation, page_mark, to_unicode,
};
use neoteletext_core::consts::{COLUMNS, DESIGNATION_GROUP_SHIFT, ESC, MODE_DIACRITIC_MASK, SPACE};

/// The sets the writer names, with their Table 32 groups and options: the
/// standard's, then neoteletext's own (REQ-CHR-012).
pub const SETS: [(&str, u8, u8); 46] = [
    ("english", 0b0000, 0b000),
    ("german", 0b0000, 0b001),
    ("swedish-finnish-hungarian", 0b0000, 0b010),
    ("italian", 0b0000, 0b011),
    ("french", 0b0000, 0b100),
    ("portuguese-spanish", 0b0000, 0b101),
    ("czech-slovak", 0b0000, 0b110),
    ("polish", 0b0001, 0b000),
    ("turkish", 0b0010, 0b110),
    ("serbian-croatian-slovenian", 0b0011, 0b101),
    ("romanian", 0b0011, 0b111),
    ("estonian", 0b0100, 0b010),
    ("lettish-lithuanian", 0b0100, 0b011),
    ("cyrillic-serbian-croatian", 0b0100, 0b000),
    ("russian-bulgarian", 0b0100, 0b100),
    ("cyrillic-ukrainian", 0b0100, 0b101),
    ("greek", 0b0110, 0b111),
    ("hebrew", 0b1010, 0b101),
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

/// The last character code of a set; 7/F is the block in every G0 set
/// (clause 15.6) and is never written from text.
pub(super) const LAST_CODE: u8 = 0x7E;
/// The highest diacritical mark number (Table 29, modes 10001 to 11111).
const LAST_MARK: u8 = MODE_DIACRITIC_MASK;

/// One cell: its row code and, for a character the row cannot show, the
/// triplet that places it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Cell {
    Code(u8),
    Marked { code: u8, mark: u8 },
    G2 { stand_in: u8, code: u8 },
}

/// What the page's sets can write.
pub(super) struct Sets {
    pub(super) default: CharSet,
    pub(super) second: Option<CharSet>,
    /// The G0 set without national option, whose base forms X/26 marks
    /// are placed on (REQ-CHR-002).
    pub(super) base: CharSet,
    pub(super) g2: G2Set,
}

impl Sets {
    pub(super) fn code_in(set: CharSet, character: char) -> Option<u8> {
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

    /// The mark number of a combining mark in the page's G2 set.
    fn mark_number(&self, combining: char) -> Option<u8> {
        (1..=LAST_MARK).find(|&mark| {
            let placed = CharRef { diacritic: page_mark(self.g2, mark), ..CharRef::new(self.base, SPACE) };
            to_unicode(placed).combining == Some(combining)
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
pub(super) fn encode(
    sets: &Sets,
    characters: &[char],
    mut in_second: bool,
    line: usize,
) -> Result<(Vec<Cell>, bool), WriteError> {
    let mut cells = Vec::with_capacity(characters.len());
    for &character in characters {
        // A combining mark after a letter of the default set: the letter in
        // the row and the mark by X/26, one mark a cell (REQ-CHR-014).
        if let (false, Some(Cell::Code(code)), Some(mark)) =
            (in_second, cells.last().copied(), sets.mark_number(character))
            && to_unicode(CharRef::new(sets.base, code)).base == to_unicode(CharRef::new(sets.default, code)).base
            && let Some(last) = cells.last_mut()
        {
            *last = Cell::Marked { code, mark };
            continue;
        }
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
pub(super) fn break_line(sets: &Sets, text: &str, line: usize) -> Result<Vec<Vec<Cell>>, WriteError> {
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

/// The page's sets for the set named `name`.
pub(super) fn page_sets(name: &str) -> Result<(u8, Sets), WriteError> {
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
