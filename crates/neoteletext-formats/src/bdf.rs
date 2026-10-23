//! The font reader (REQ-FNT-002, decision E.72): the neoteletext profile of
//! the Glyph Bitmap Distribution Format (BDF) versions 2.1 and 2.2, whose
//! every character is a 12 by 10 cell named by its Unicode code point.
//!
//! The reader faces fonts from the network (`docs/spec/design/font-security.md` section
//! 3.1), so it is strict and bounded: the file, each line, the properties
//! and the characters all have limits, every line must be one the profile
//! names, and the first fault refuses the whole file with its line number.
//! Nothing of a refused file is used. It reads the whole file at once, from
//! a buffer the caller holds, and writes the glyphs into a table the caller
//! supplies; it allocates nothing.
//!
//! `FONT`, `SIZE` and `SWIDTH`, which BDF 2.1 requires, are accepted and
//! not used (REQ-FNT-002 as amended for issue #200).

use neoteletext_core::consts::{
    CELL_FIELD_LINES, CELL_HEIGHT, CELL_WIDTH, FONT_MAX_CHARACTERS, FONT_MAX_FILE_BYTES, FONT_MAX_LINE_BYTES,
    FONT_MAX_PROPERTIES,
};
use neoteletext_core::types::FontGlyph;

pub mod convert;
pub mod source;

// The cell's 12, 10 and 20 fit an `i32` with room to spare.
#[allow(clippy::cast_possible_wrap)]
const WIDTH: i32 = CELL_WIDTH as i32;
#[allow(clippy::cast_possible_wrap)]
const HEIGHT: i32 = CELL_FIELD_LINES as i32;
#[allow(clippy::cast_possible_wrap)]
const TALL_HEIGHT: i32 = CELL_HEIGHT as i32;
/// The 12 by 10 cell, as `FONTBOUNDINGBOX` and every `BBX` give it.
const CELL_BOX: [i32; 4] = [WIDTH, HEIGHT, 0, 0];
/// The 12 by 20 cell (decision E.77).
const TALL_CELL_BOX: [i32; 4] = [WIDTH, TALL_HEIGHT, 0, 0];

/// The cell box and line count of a font of either height.
const fn cell_box(tall: bool) -> ([i32; 4], usize) {
    if tall { (TALL_CELL_BOX, CELL_HEIGHT) } else { (CELL_BOX, CELL_FIELD_LINES) }
}
/// Every character's advance, as `DWIDTH` gives it.
const ADVANCE: [i32; 2] = [WIDTH, 0];
/// Hexadecimal digits of one bitmap row: 12 dots padded to 16 bits.
const ROW_DIGITS: usize = 4;
/// The padding bits at the right of each row, which must be zero.
const ROW_PAD_BITS: u32 = (ROW_DIGITS * 4 - CELL_WIDTH) as u32;
/// The largest Unicode scalar value, and the surrogates, which are not
/// scalar values (Unicode section 3.9).
const MAX_SCALAR: i32 = 0x10_FFFF;
const SURROGATES: core::ops::RangeInclusive<i32> = 0xD800..=0xDFFF;
/// The versions of the format accepted.
const VERSIONS: [&[u8]; 2] = [b"2.1", b"2.2"];

/// A font that passed every rule. Its glyphs are the first
/// [`characters`](Font::characters) entries of the table given to [`read`],
/// sorted by code point. The strings are the bytes between the quotes of
/// the property, with any doubled quote as the file has it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Font<'a> {
    /// `FAMILY_NAME`: the font's name.
    pub family: Option<&'a [u8]>,
    /// `COPYRIGHT`.
    pub copyright: Option<&'a [u8]>,
    /// `NOTICE`.
    pub notice: Option<&'a [u8]>,
    /// How many glyphs the table holds.
    pub characters: usize,
    /// The font is 12 by 20 (decision E.77); otherwise 12 by 10.
    pub tall: bool,
}

/// Why a file was refused, and the line, counted from 1, where the fault
/// was found; line 0 when the fault is the file as a whole.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FontError {
    /// The line.
    pub line: u32,
    /// The fault.
    pub kind: FontErrorKind,
}

/// The faults that refuse a font (REQ-FNT-002).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontErrorKind {
    /// The file is larger than 256 kilobytes.
    TooLarge,
    /// A line is longer than 512 bytes.
    LineTooLong,
    /// A line holds a control character other than a tab, or an empty line
    /// comes before `ENDFONT`.
    BadLine,
    /// The first line other than a comment is not `STARTFONT`.
    NotBdf,
    /// `STARTFONT` names a version other than 2.1 or 2.2.
    Version,
    /// A keyword the profile does not name.
    UnknownKeyword,
    /// A keyword that may appear once appears again, or a line comes where
    /// it may not.
    Misplaced,
    /// A field is missing, extra or not a number.
    Malformed,
    /// `FONTBOUNDINGBOX` is neither `12 10 0 0` nor `12 20 0 0`, or is
    /// missing.
    BoundingBox,
    /// The properties are more than 64, not as many as declared, malformed
    /// or repeated.
    Properties,
    /// `CHARSET_REGISTRY` is not `"ISO10646"`, or is missing.
    Registry,
    /// `CHARSET_ENCODING` is not `"1"`, or is missing.
    CharsetEncoding,
    /// `SPACING` is not `"C"` or `"M"`, or is missing.
    Spacing,
    /// `CHARS` is missing, above 1,024, or not the number of characters
    /// that follow.
    Characters,
    /// `ENCODING` is not a Unicode scalar value.
    Code,
    /// `ENCODING` repeats a code point already in the font.
    RepeatedCode,
    /// `DWIDTH` is not `12 0`.
    Advance,
    /// `BBX` is not the font's cell, `12 10 0 0` or `12 20 0 0`.
    CharacterBox,
    /// A bitmap row is not four hexadecimal digits whose last four bits
    /// are zero.
    Row,
    /// A character ended without `ENCODING`, `DWIDTH`, `BBX` or its ten or
    /// twenty bitmap rows.
    Incomplete,
    /// The file ends before `ENDFONT`.
    NoEnd,
    /// Something other than empty lines follows `ENDFONT`.
    AfterEnd,
    /// A source font's cell is not one REQ-FNT-010 converts.
    Cell,
    /// A source glyph reaches outside its cell.
    GlyphTooLarge,
    /// A source font has more than one bit per pixel.
    Greyscale,
}

/// Where the reader is in the file.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    /// Before `STARTFONT`.
    Start,
    /// The global part, before `CHARS`.
    Header,
    /// Between `STARTPROPERTIES` and `ENDPROPERTIES`.
    Properties,
    /// After `CHARS`, between characters.
    Characters,
    /// Between `STARTCHAR` and `BITMAP`.
    Character,
    /// The bitmap rows of a character.
    Bitmap,
    /// After `ENDFONT`.
    End,
}

/// The parts of one character seen so far.
#[derive(Clone, Copy, Default)]
struct Character {
    code: Option<u32>,
    advance: bool,
    cell: bool,
    swidth: bool,
    rows: usize,
    bitmap: [u16; CELL_HEIGHT],
}

/// The global parts seen so far.
// One flag for each keyword or property that REQ-FNT-002 allows once.
#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct Header<'a> {
    font: bool,
    size: bool,
    bounding_box: bool,
    tall: bool,
    properties_seen: bool,
    properties_declared: usize,
    properties_read: usize,
    registry: bool,
    charset_encoding: bool,
    spacing: bool,
    family: Option<&'a [u8]>,
    copyright: Option<&'a [u8]>,
    notice: Option<&'a [u8]>,
    declared_characters: Option<usize>,
}

/// Reads a font into `glyphs`, which must have room for every character
/// (1,024 is always enough). On success the first [`Font::characters`]
/// entries hold the glyphs, sorted by code point; on failure the table's
/// contents are meaningless.
pub fn read<'a>(bytes: &'a [u8], glyphs: &mut [FontGlyph]) -> Result<Font<'a>, FontError> {
    if bytes.len() > FONT_MAX_FILE_BYTES {
        return Err(FontError { line: 0, kind: FontErrorKind::TooLarge });
    }
    let mut reader =
        Parser { section: Section::Start, header: Header::default(), character: Character::default(), count: 0 };
    let mut number: u32 = 0;
    let mut rest = bytes;
    while !rest.is_empty() {
        number = number.saturating_add(1);
        let (line, after) = match rest.iter().position(|&byte| byte == b'\n') {
            Some(end) => (rest.get(..end).unwrap_or(&[]), rest.get(end.saturating_add(1)..).unwrap_or(&[])),
            None => (rest, &[][..]),
        };
        rest = after;
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        reader.line(line, glyphs).map_err(|kind| FontError { line: number, kind })?;
    }
    reader.finish().map_err(|kind| FontError { line: number, kind })
}

struct Parser<'a> {
    section: Section,
    header: Header<'a>,
    character: Character,
    count: usize,
}

impl<'a> Parser<'a> {
    fn line(&mut self, line: &'a [u8], glyphs: &mut [FontGlyph]) -> Result<(), FontErrorKind> {
        if line.len() > FONT_MAX_LINE_BYTES {
            return Err(FontErrorKind::LineTooLong);
        }
        if line.iter().any(|&byte| byte < b' ' && byte != b'\t' || byte == 0x7F) {
            return Err(FontErrorKind::BadLine);
        }
        if self.section == Section::End {
            return if line.is_empty() { Ok(()) } else { Err(FontErrorKind::AfterEnd) };
        }
        let (keyword, value) = split_keyword(line);
        if keyword.is_empty() {
            return Err(FontErrorKind::BadLine);
        }
        // REQ-FNT-002: comments may appear anywhere.
        if keyword == b"COMMENT" {
            return Ok(());
        }
        match self.section {
            Section::Start => self.start(keyword, value),
            Section::Header => self.global(keyword, value),
            Section::Properties => self.property(keyword, value),
            Section::Characters => self.between(keyword, value, glyphs),
            Section::Character => self.in_character(keyword, value),
            Section::Bitmap => self.row(keyword, value, glyphs),
            Section::End => Err(FontErrorKind::AfterEnd),
        }
    }

    fn start(&mut self, keyword: &[u8], value: &[u8]) -> Result<(), FontErrorKind> {
        if keyword != b"STARTFONT" {
            return Err(FontErrorKind::NotBdf);
        }
        if !VERSIONS.contains(&value) {
            return Err(FontErrorKind::Version);
        }
        self.section = Section::Header;
        Ok(())
    }

    fn global(&mut self, keyword: &[u8], value: &'a [u8]) -> Result<(), FontErrorKind> {
        let header = &mut self.header;
        match keyword {
            b"FONT" => {
                // Accepted and not used: the name is FAMILY_NAME.
                once(&mut header.font)?;
                if value.is_empty() {
                    return Err(FontErrorKind::Malformed);
                }
            }
            b"SIZE" => {
                // Accepted and not used: the cell is fixed.
                once(&mut header.size)?;
                numbers::<3>(value)?;
            }
            b"FONTBOUNDINGBOX" => {
                once(&mut header.bounding_box)?;
                header.tall = match numbers::<4>(value)? {
                    CELL_BOX => false,
                    TALL_CELL_BOX => true,
                    _ => return Err(FontErrorKind::BoundingBox),
                };
            }
            b"STARTPROPERTIES" => {
                once(&mut header.properties_seen)?;
                let [declared] = numbers::<1>(value)?;
                let declared = usize::try_from(declared).map_err(|_| FontErrorKind::Properties)?;
                if declared > FONT_MAX_PROPERTIES {
                    return Err(FontErrorKind::Properties);
                }
                header.properties_declared = declared;
                self.section = Section::Properties;
            }
            b"CHARS" => {
                let [declared] = numbers::<1>(value)?;
                let declared = usize::try_from(declared).map_err(|_| FontErrorKind::Characters)?;
                if declared > FONT_MAX_CHARACTERS {
                    return Err(FontErrorKind::Characters);
                }
                if !header.bounding_box {
                    return Err(FontErrorKind::BoundingBox);
                }
                check_properties(header)?;
                header.declared_characters = Some(declared);
                self.section = Section::Characters;
            }
            b"ENDPROPERTIES" | b"STARTCHAR" | b"ENCODING" | b"SWIDTH" | b"DWIDTH" | b"BBX" | b"BITMAP" | b"ENDCHAR"
            | b"ENDFONT" | b"STARTFONT" => return Err(FontErrorKind::Misplaced),
            _ => return Err(FontErrorKind::UnknownKeyword),
        }
        Ok(())
    }

    fn property(&mut self, name: &[u8], value: &'a [u8]) -> Result<(), FontErrorKind> {
        let header = &mut self.header;
        if name == b"ENDPROPERTIES" {
            if !value.is_empty() || header.properties_read != header.properties_declared {
                return Err(FontErrorKind::Properties);
            }
            self.section = Section::Header;
            return Ok(());
        }
        header.properties_read = header.properties_read.saturating_add(1);
        if header.properties_read > header.properties_declared
            || !name.iter().all(|&byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(FontErrorKind::Properties);
        }
        let text = property_value(value)?;
        match name {
            b"CHARSET_REGISTRY" => {
                once(&mut header.registry).map_err(|_| FontErrorKind::Properties)?;
                if !text.is_some_and(|text| text.eq_ignore_ascii_case(b"ISO10646")) {
                    return Err(FontErrorKind::Registry);
                }
            }
            b"CHARSET_ENCODING" => {
                once(&mut header.charset_encoding).map_err(|_| FontErrorKind::Properties)?;
                if text != Some(b"1") {
                    return Err(FontErrorKind::CharsetEncoding);
                }
            }
            b"SPACING" => {
                once(&mut header.spacing).map_err(|_| FontErrorKind::Properties)?;
                if !matches!(text, Some(b"C" | b"c" | b"M" | b"m")) {
                    return Err(FontErrorKind::Spacing);
                }
            }
            b"FAMILY_NAME" => keep(&mut header.family, text)?,
            b"COPYRIGHT" => keep(&mut header.copyright, text)?,
            b"NOTICE" => keep(&mut header.notice, text)?,
            _ => {}
        }
        Ok(())
    }

    fn between(&mut self, keyword: &[u8], value: &[u8], glyphs: &[FontGlyph]) -> Result<(), FontErrorKind> {
        match keyword {
            b"STARTCHAR" => {
                if value.is_empty() {
                    return Err(FontErrorKind::Malformed);
                }
                if self.count >= self.header.declared_characters.unwrap_or(0) || self.count >= glyphs.len() {
                    return Err(FontErrorKind::Characters);
                }
                self.character = Character::default();
                self.section = Section::Character;
                Ok(())
            }
            b"ENDFONT" => {
                if !value.is_empty() {
                    return Err(FontErrorKind::Malformed);
                }
                if Some(self.count) != self.header.declared_characters {
                    return Err(FontErrorKind::Characters);
                }
                self.section = Section::End;
                Ok(())
            }
            _ => Err(misplaced_or_unknown(keyword)),
        }
    }

    fn in_character(&mut self, keyword: &[u8], value: &[u8]) -> Result<(), FontErrorKind> {
        let character = &mut self.character;
        match keyword {
            b"ENCODING" => {
                if character.code.is_some() {
                    return Err(FontErrorKind::Misplaced);
                }
                // A second integer (the non-standard code of `ENCODING -1 n`)
                // is not part of the profile.
                let [code] = numbers::<1>(value).map_err(|_| FontErrorKind::Code)?;
                if !(0..=MAX_SCALAR).contains(&code) || SURROGATES.contains(&code) {
                    return Err(FontErrorKind::Code);
                }
                character.code = u32::try_from(code).ok();
            }
            b"SWIDTH" => {
                // Accepted and not used: the advance is DWIDTH.
                once(&mut character.swidth)?;
                numbers::<2>(value)?;
            }
            b"DWIDTH" => {
                once(&mut character.advance)?;
                if numbers::<2>(value)? != ADVANCE {
                    return Err(FontErrorKind::Advance);
                }
            }
            b"BBX" => {
                once(&mut character.cell)?;
                if numbers::<4>(value)? != cell_box(self.header.tall).0 {
                    return Err(FontErrorKind::CharacterBox);
                }
            }
            b"BITMAP" => {
                if !value.is_empty() {
                    return Err(FontErrorKind::Malformed);
                }
                if character.code.is_none() || !character.advance || !character.cell {
                    return Err(FontErrorKind::Incomplete);
                }
                self.section = Section::Bitmap;
            }
            b"ENDCHAR" => return Err(FontErrorKind::Incomplete),
            _ => return Err(misplaced_or_unknown(keyword)),
        }
        Ok(())
    }

    fn row(&mut self, keyword: &[u8], value: &[u8], glyphs: &mut [FontGlyph]) -> Result<(), FontErrorKind> {
        let character = &mut self.character;
        if keyword == b"ENDCHAR" {
            if !value.is_empty() {
                return Err(FontErrorKind::Malformed);
            }
            let tall = self.header.tall;
            if character.rows != cell_box(tall).1 {
                return Err(FontErrorKind::Incomplete);
            }
            let code = character.code.ok_or(FontErrorKind::Incomplete)?;
            insert(glyphs, self.count, FontGlyph { code_point: code, rows: character.bitmap, tall })?;
            self.count = self.count.saturating_add(1);
            self.section = Section::Characters;
            return Ok(());
        }
        if !value.is_empty() || keyword.len() != ROW_DIGITS {
            return Err(FontErrorKind::Row);
        }
        let mut row: u16 = 0;
        for &digit in keyword {
            let value = (digit as char).to_digit(16).ok_or(FontErrorKind::Row)?;
            row = (row << 4) | u16::try_from(value).map_err(|_| FontErrorKind::Row)?;
        }
        if row.trailing_zeros() < ROW_PAD_BITS {
            return Err(FontErrorKind::Row);
        }
        let lines = cell_box(self.header.tall).1;
        let slot =
            character.bitmap.get_mut(character.rows).filter(|_| character.rows < lines).ok_or(FontErrorKind::Row)?;
        *slot = row >> ROW_PAD_BITS;
        character.rows = character.rows.saturating_add(1);
        Ok(())
    }

    fn finish(self) -> Result<Font<'a>, FontErrorKind> {
        match self.section {
            Section::End => {}
            Section::Start => return Err(FontErrorKind::NotBdf),
            _ => return Err(FontErrorKind::NoEnd),
        }
        Ok(Font {
            family: self.header.family,
            copyright: self.header.copyright,
            notice: self.header.notice,
            characters: self.count,
            tall: self.header.tall,
        })
    }
}

/// The keyword and the rest of the line after the whitespace that follows
/// it.
fn split_keyword(line: &[u8]) -> (&[u8], &[u8]) {
    let end = line.iter().position(|&byte| byte == b' ' || byte == b'\t').unwrap_or(line.len());
    let (keyword, rest) = line.split_at(end);
    let start = rest.iter().position(|&byte| byte != b' ' && byte != b'\t').unwrap_or(rest.len());
    (keyword, rest.get(start..).unwrap_or(&[]))
}

/// Exactly `N` decimal integers separated by whitespace.
fn numbers<const N: usize>(value: &[u8]) -> Result<[i32; N], FontErrorKind> {
    let mut out = [0i32; N];
    let mut fields = value.split(|&byte| byte == b' ' || byte == b'\t').filter(|field| !field.is_empty());
    for slot in &mut out {
        *slot = integer(fields.next().ok_or(FontErrorKind::Malformed)?)?;
    }
    if fields.next().is_some() {
        return Err(FontErrorKind::Malformed);
    }
    Ok(out)
}

/// A decimal integer with an optional minus sign, within `i32`.
fn integer(field: &[u8]) -> Result<i32, FontErrorKind> {
    let (negative, digits) = match field.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, field),
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return Err(FontErrorKind::Malformed);
    }
    let mut value: i32 = 0;
    for &digit in digits {
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(i32::from(digit.wrapping_sub(b'0'))))
            .ok_or(FontErrorKind::Malformed)?;
    }
    Ok(if negative { value.wrapping_neg() } else { value })
}

/// A property's value: a quoted string, given as the bytes between the
/// quotes, or an integer, given as `None`.
fn property_value(value: &[u8]) -> Result<Option<&[u8]>, FontErrorKind> {
    let Some(inner) = value.strip_prefix(b"\"") else {
        integer(value).map_err(|_| FontErrorKind::Properties)?;
        return Ok(None);
    };
    let inner = inner.strip_suffix(b"\"").ok_or(FontErrorKind::Properties)?;
    // Inside, a quote is written twice (BDF 2.1, "Properties").
    let mut bytes = inner.iter();
    while let Some(&byte) = bytes.next() {
        if byte == b'"' && bytes.next() != Some(&b'"') {
            return Err(FontErrorKind::Properties);
        }
    }
    Ok(Some(inner))
}

fn once(seen: &mut bool) -> Result<(), FontErrorKind> {
    if core::mem::replace(seen, true) { Err(FontErrorKind::Misplaced) } else { Ok(()) }
}

fn keep<'a>(slot: &mut Option<&'a [u8]>, text: Option<&'a [u8]>) -> Result<(), FontErrorKind> {
    if slot.is_some() || text.is_none() {
        return Err(FontErrorKind::Properties);
    }
    *slot = text;
    Ok(())
}

fn check_properties(header: &Header<'_>) -> Result<(), FontErrorKind> {
    if !header.properties_seen {
        return Err(FontErrorKind::Properties);
    }
    if !header.registry {
        return Err(FontErrorKind::Registry);
    }
    if !header.charset_encoding {
        return Err(FontErrorKind::CharsetEncoding);
    }
    if !header.spacing {
        return Err(FontErrorKind::Spacing);
    }
    Ok(())
}

fn misplaced_or_unknown(keyword: &[u8]) -> FontErrorKind {
    const KNOWN: [&[u8]; 15] = [
        b"STARTFONT",
        b"FONT",
        b"SIZE",
        b"FONTBOUNDINGBOX",
        b"STARTPROPERTIES",
        b"ENDPROPERTIES",
        b"CHARS",
        b"STARTCHAR",
        b"ENCODING",
        b"SWIDTH",
        b"DWIDTH",
        b"BBX",
        b"BITMAP",
        b"ENDCHAR",
        b"ENDFONT",
    ];
    if KNOWN.contains(&keyword) { FontErrorKind::Misplaced } else { FontErrorKind::UnknownKeyword }
}

/// Puts a glyph into the sorted table of `count` glyphs.
fn insert(glyphs: &mut [FontGlyph], count: usize, glyph: FontGlyph) -> Result<(), FontErrorKind> {
    let sorted = glyphs.get(..count).ok_or(FontErrorKind::Characters)?;
    let Err(position) = sorted.binary_search_by_key(&glyph.code_point, |entry| entry.code_point) else {
        return Err(FontErrorKind::RepeatedCode);
    };
    let end = count.checked_add(1).ok_or(FontErrorKind::Characters)?;
    let window = glyphs.get_mut(position..end).ok_or(FontErrorKind::Characters)?;
    window.rotate_right(1);
    if let Some(slot) = window.first_mut() {
        *slot = glyph;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
