//! The set table and the encoding of text into cells of the page's sets:
//! the row's G0 sets, X/26 marks and G2 characters (REQ-CHR-012,
//! REQ-CHR-014), and the breaking of a left-to-right line into rows.

use super::{Sets, WriteError};
use neoteletext_core::charset::{
    CharRef, CharSet, G0, compose, default_second_value, designation, page_mark, to_unicode,
};
use neoteletext_core::consts::{COLUMNS, DESIGNATION_GROUP_SHIFT, ESC, MODE_DIACRITIC_MASK, SPACE};

/// The sets the writer names, with their Table 32 groups and options: the
/// standard's, then neoteletext's own (REQ-CHR-012).
pub const SETS: [(&str, u8, u8); 50] = [
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
    // The standard's Arabic set, its letters joined by the writer (REQ-2190).
    ("arabic-table44", 0b1000, 0b111),
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
    ("japanese-romaji", 0b1101, 0b000),
    ("hiragana", 0b1101, 0b001),
    ("katakana", 0b1101, 0b010),
];

/// The value of Latin English, the first set a row set run tries
/// (REQ-2188 item 4).
const LATIN_ENGLISH: u8 = 0;
/// The standard's Arabic set, Table 44 (REQ-2190).
const ARABIC_TABLE_44: u8 = (0b1000 << DESIGNATION_GROUP_SHIFT) | 0b111;

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
    Marked {
        code: u8,
        mark: u8,
    },
    G2 {
        stand_in: u8,
        code: u8,
    },
    /// A code of another set, with the row set triplet that starts a run
    /// in that set here (REQ-2180, DEC-2179): the set's Table 32 value.
    Run {
        code: u8,
        value: u8,
    },
}

/// Where a row's encoding stands: the set of a row set run, if one is
/// open, and whether ESC has switched to the page's second set.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) struct State {
    pub(super) in_second: bool,
    /// The run's Table 32 value and set; `None` for the page's own set.
    pub(super) run: Option<(u8, CharSet)>,
}

impl Sets {
    /// True when the page's default set has a code for `character`.
    pub(super) fn holds(&self, character: char) -> bool {
        Self::code_in(self.default, character).is_some()
    }

    /// True when the page's G2 set has a code for `character`.
    pub(super) fn g2_holds(&self, character: char) -> bool {
        Self::code_in(CharSet::G2(self.g2), character).is_some()
    }

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
/// and the state after them, with no row set runs: for callers that encode
/// a row a piece at a time without carrying a run between pieces.
pub(super) fn encode(
    sets: &Sets,
    characters: &[char],
    in_second: bool,
    line: usize,
) -> Result<(Vec<Cell>, bool), WriteError> {
    let (cells, after) = encode_from(sets, characters, State { in_second, run: None }, false, line)?;
    Ok((cells, after.in_second))
}

/// Encodes `characters` from `state`, giving the cells and the state after
/// them (REQ-2148 item 4, REQ-2188 item 4). Each character is written, by
/// the first means that can:
/// 1. a code of the row's current set;
/// 2. a code of the set ESC reaches: by ESC when a space just written can
///    take it, or at Level 3.5 and below; otherwise, with `runs`, by a row
///    set triplet naming that set, which takes no cell;
/// 3. on the page's own set, a G0 letter with an X/26 mark, or a G2
///    character by X/26;
/// 4. with `runs`, a row set run in the set that holds the most of the
///    text from here: the page's sets, Latin English, then every set
///    the writer names.
pub(super) fn encode_from(
    sets: &Sets,
    characters: &[char],
    mut state: State,
    runs: bool,
    line: usize,
) -> Result<(Vec<Cell>, State), WriteError> {
    let mut cells = Vec::with_capacity(characters.len());
    for (index, &character) in characters.iter().enumerate() {
        let base = state.run.map_or(sets.default, |(_, set)| set);
        // A combining mark after a letter of the default set: the letter in
        // the row and the mark by X/26, one mark a cell (REQ-CHR-014).
        if let (false, None, Some(Cell::Code(code)), Some(mark)) =
            (state.in_second, state.run, cells.last().copied(), sets.mark_number(character))
            && to_unicode(CharRef::new(sets.base, code)).base == to_unicode(CharRef::new(sets.default, code)).base
            && let Some(last) = cells.last_mut()
        {
            *last = Cell::Marked { code, mark };
            continue;
        }
        let current = if state.in_second { sets.second.unwrap_or(base) } else { base };
        if let Some(code) = Sets::code_in(current, character) {
            cells.push(Cell::Code(code));
            continue;
        }
        let other = if state.in_second { Some(base) } else { sets.second };
        if let Some(code) = other.and_then(|set| Sets::code_in(set, character)) {
            let free = cells.last() == Some(&Cell::Code(SPACE));
            if let (true, false, Some(value)) = (runs, free, other.and_then(|set| sets.value_of(set, state))) {
                cells.push(Cell::Run { code, value });
                state = State { in_second: false, run: sets.run_of(value) };
            } else {
                switch(&mut cells);
                cells.push(Cell::Code(code));
                state.in_second = !state.in_second;
            }
            continue;
        }
        if state.run.is_none() {
            // X/26 characters are placed over the default set's row.
            if let Some(cell) = sets.marked(character).or_else(|| sets.g2(character)) {
                if state.in_second {
                    switch(&mut cells);
                    state.in_second = false;
                }
                cells.push(cell);
                continue;
            }
        }
        let rest = characters.get(index..).unwrap_or_default();
        match sets.best_run(rest).filter(|_| runs) {
            Some((value, set, code)) => {
                cells.push(Cell::Run { code, value });
                state = State { in_second: false, run: (set != sets.default).then_some((value, set)) };
            }
            None => return Err(WriteError::Missing { character, line }),
        }
    }
    Ok((cells, state))
}

impl Sets {
    /// The Table 32 value naming `set` for a row set triplet: the page's
    /// own value, its second set's, or a run's.
    fn value_of(&self, set: CharSet, state: State) -> Option<u8> {
        if set == self.default {
            return Some(self.value);
        }
        if Some(set) == self.second {
            return self.second_value;
        }
        state.run.filter(|(_, run)| *run == set).map(|(value, _)| value)
    }

    /// The run a row set triplet naming `value` opens: none when it names
    /// the page's own set.
    fn run_of(&self, value: u8) -> Option<(u8, CharSet)> {
        let set = CharSet::G0(designation(value)?.g0);
        (set != self.default).then_some((value, set))
    }

    /// The set that writes the most of `rest` from its first character, in
    /// order of preference on a tie: the page's own, its second, Latin
    /// English, then every set the writer names; with its value and the
    /// first character's code.
    fn best_run(&self, rest: &[char]) -> Option<(u8, CharSet, u8)> {
        let first = *rest.first()?;
        let own = [Some(self.value), self.second_value, Some(LATIN_ENGLISH)];
        let named = SETS.iter().map(|&(_, group, option)| Some((group << DESIGNATION_GROUP_SHIFT) | option));
        let mut best: Option<(usize, u8, CharSet, u8)> = None;
        for value in own.into_iter().chain(named).flatten() {
            let Some(found) = designation(value) else { continue };
            let set = CharSet::G0(found.g0);
            let Some(code) = Self::code_in(set, first) else { continue };
            let length = rest.iter().take_while(|&&next| Self::code_in(set, next).is_some()).count();
            if best.is_none_or(|(longest, ..)| length > longest) {
                best = Some((length, value, set, code));
            }
        }
        best.map(|(_, value, set, code)| (value, set, code))
    }
}

/// Breaks one line of text into rows of at most 40 cells, at spaces where
/// it can.
pub(super) fn break_line(sets: &Sets, text: &str, line: usize) -> Result<Vec<Vec<Cell>>, WriteError> {
    let mut rows = Vec::new();
    let mut row: Vec<Cell> = Vec::new();
    let mut state = State::default();
    for word in text.split(' ') {
        let mut characters: Vec<char> = word.chars().collect();
        if !row.is_empty() {
            characters.insert(0, ' ');
        }
        let (cells, after) = encode_from(sets, &characters, state, sets.runs, line)?;
        if row.len().saturating_add(cells.len()) <= COLUMNS {
            row.extend(cells);
            state = after;
            continue;
        }
        if !row.is_empty() {
            rows.push(core::mem::take(&mut row));
        }
        // A new row starts in the default set (REQ-CHR-009, REQ-2180 item
        // 5), with no run open.
        let word: Vec<char> = word.chars().collect();
        let (cells, after) = encode_from(sets, &word, State::default(), sets.runs, line)?;
        state = after;
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

/// The Table 32 value of the set named `name`.
pub(super) fn set_value(name: &str) -> Option<u8> {
    SETS.iter().find(|(set, ..)| *set == name).map(|&(_, group, option)| (group << DESIGNATION_GROUP_SHIFT) | option)
}

/// The page's sets for the Table 32 value `value`; `runs` is true for a
/// page written for NeoTeletext's own receivers, which may use row set
/// triplets (REQ-2151).
pub(super) fn page_sets(value: u8, runs: bool) -> Option<Sets> {
    let found = designation(value)?;
    // REQ-2190 item 4: a page in the standard's Arabic set names Latin
    // English as its second set (Table 33), so ESC reaches Latin letters.
    let second_value = default_second_value(value).or((value == ARABIC_TABLE_44).then_some(LATIN_ENGLISH));
    Some(Sets {
        value,
        default: CharSet::G0(found.g0),
        second: second_value.and_then(designation).map(|found| CharSet::G0(found.g0)),
        second_value,
        base: CharSet::G0(G0 { set: found.g0.set, national: None }),
        g2: found.g2,
        runs,
    })
}
