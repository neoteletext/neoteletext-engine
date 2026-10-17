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
    CELL_FIELD_LINES, CELL_WIDTH, FONT_MAX_CHARACTERS, FONT_MAX_FILE_BYTES, FONT_MAX_LINE_BYTES, FONT_MAX_PROPERTIES,
};
use neoteletext_core::types::FontGlyph;

pub mod convert;
pub mod source;

// The cell's 12 and 10 fit an `i32` with room to spare.
#[allow(clippy::cast_possible_wrap)]
const WIDTH: i32 = CELL_WIDTH as i32;
#[allow(clippy::cast_possible_wrap)]
const HEIGHT: i32 = CELL_FIELD_LINES as i32;
/// The font's cell, as `FONTBOUNDINGBOX` and every `BBX` give it.
const CELL_BOX: [i32; 4] = [WIDTH, HEIGHT, 0, 0];
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
    /// `FONTBOUNDINGBOX` is not `12 10 0 0`, or is missing.
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
    /// `BBX` is not `12 10 0 0`.
    CharacterBox,
    /// A bitmap row is not four hexadecimal digits whose last four bits
    /// are zero.
    Row,
    /// A character ended without `ENCODING`, `DWIDTH`, `BBX` or ten bitmap
    /// rows.
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
    bitmap: [u16; CELL_FIELD_LINES],
}

/// The global parts seen so far.
// One flag for each keyword or property that REQ-FNT-002 allows once.
#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct Header<'a> {
    font: bool,
    size: bool,
    bounding_box: bool,
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
                if numbers::<4>(value)? != CELL_BOX {
                    return Err(FontErrorKind::BoundingBox);
                }
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
                if numbers::<4>(value)? != CELL_BOX {
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
            if character.rows != CELL_FIELD_LINES {
                return Err(FontErrorKind::Incomplete);
            }
            let code = character.code.ok_or(FontErrorKind::Incomplete)?;
            insert(glyphs, self.count, FontGlyph { code_point: code, rows: character.bitmap })?;
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
        let slot = character.bitmap.get_mut(character.rows).ok_or(FontErrorKind::Row)?;
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use core::fmt::Write as _;
    use std::string::String;
    use std::vec::Vec;
    use std::{format, vec};

    /// A glyph table with room for any font, on the heap.
    fn table() -> Vec<FontGlyph> {
        vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS]
    }

    /// A character: its code point, then ten rows of four digits.
    fn character(code: i64, rows: &str) -> String {
        format!(
            "STARTCHAR u{code:04X}\nENCODING {code}\nSWIDTH 1200 0\nDWIDTH 12 0\nBBX 12 10 0 0\nBITMAP\n{rows}ENDCHAR\n"
        )
    }

    const A_ROWS: &str = "0000\n0F00\n30C0\n6060\n6060\n7FE0\n6060\n6060\n0000\n0000\n";
    const BLANK_ROWS: &str = "0000\n0000\n0000\n0000\n0000\n0000\n0000\n0000\n0000\n0000\n";

    /// A valid font with `characters`, sent through `edit` first.
    fn font(characters: &[String], edit: impl Fn(String) -> String) -> String {
        let mut text = String::from(
            "STARTFONT 2.1\nCOMMENT anywhere\nFONT -test-Test-Medium-R-Normal--10-100-75-75-C-120-ISO10646-1\n\
             SIZE 10 75 75\nFONTBOUNDINGBOX 12 10 0 0\nSTARTPROPERTIES 6\nFAMILY_NAME \"Test \"\"Font\"\"\"\n\
             COPYRIGHT \"(c) nobody\"\nNOTICE \"CC0\"\nCHARSET_REGISTRY \"ISO10646\"\nCHARSET_ENCODING \"1\"\n\
             SPACING \"C\"\nENDPROPERTIES\n",
        );
        let _ = writeln!(text, "CHARS {}", characters.len());
        for character in characters {
            text.push_str(character);
        }
        text.push_str("ENDFONT\n");
        edit(text)
    }

    fn valid() -> String {
        font(&[character(0x42, A_ROWS), character(0x41, A_ROWS), character(0x20, BLANK_ROWS)], |text| text)
    }

    fn refused(text: &str) -> FontError {
        let mut glyphs = table();
        read(text.as_bytes(), &mut glyphs).unwrap_err()
    }

    fn kind(text: &str) -> FontErrorKind {
        refused(text).kind
    }

    /// REQ-FNT-002: a valid font gives its glyphs sorted, its rows as 12
    /// dots with bit 11 the leftmost, and its name and notices.
    #[test]
    fn a_valid_font_is_read() {
        let text = valid();
        let mut glyphs = table();
        let font = read(text.as_bytes(), &mut glyphs).unwrap();
        assert_eq!(font.characters, 3);
        assert_eq!(font.family, Some(&b"Test \"\"Font\"\""[..]));
        assert_eq!(font.copyright, Some(&b"(c) nobody"[..]));
        assert_eq!(font.notice, Some(&b"CC0"[..]));
        let codes: Vec<u32> = glyphs[..3].iter().map(|glyph| glyph.code_point).collect();
        assert_eq!(codes, [0x20, 0x41, 0x42], "sorted by code point");
        assert_eq!(glyphs[1].rows, [0x000, 0x0F0, 0x30C, 0x606, 0x606, 0x7FE, 0x606, 0x606, 0x000, 0x000]);
        assert_eq!(glyphs[0].rows, [0; 10]);
    }

    /// Line endings may be CRLF, the last line may lack its newline, empty
    /// lines may follow ENDFONT, the keywords that BDF 2.1 requires are
    /// accepted, and the case of the registry and spacing does not matter.
    #[test]
    fn tolerated_forms() {
        let mut glyphs = table();
        for text in [
            valid().replace('\n', "\r\n"),
            valid().trim_end().into(),
            format!("{}\n\n", valid()),
            valid().replace("\"ISO10646\"", "\"iso10646\"").replace("SPACING \"C\"", "SPACING \"m\""),
            valid().replace("STARTFONT 2.1", "STARTFONT 2.2"),
            format!("COMMENT before\n{}", valid()),
            valid().replace("BITMAP\n0000", "BITMAP\nCOMMENT inside\n0000"),
            valid().replace("SIZE 10 75 75\n", "").replace("FONT -test", "COMMENT -test"),
            valid().replace("SWIDTH 1200 0\n", ""),
            valid().replace("0F00", "0f00"),
        ] {
            assert!(read(text.as_bytes(), &mut glyphs).is_ok(), "{text}");
        }
        let unnamed = valid().replace("STARTPROPERTIES 6\nFAMILY_NAME \"Test \"\"Font\"\"\"\n", "STARTPROPERTIES 5\n");
        assert_eq!(read(unnamed.as_bytes(), &mut glyphs).unwrap().family, None);
    }

    /// REQ-FNT-002: each rule broken refuses the whole file, with the line.
    #[test]
    fn each_rule_broken_refuses_the_file() {
        let v = valid();
        let cases: Vec<(String, FontErrorKind)> = vec![
            (v.replace("STARTFONT 2.1\n", ""), FontErrorKind::NotBdf),
            (String::new(), FontErrorKind::NotBdf),
            (v.replace("STARTFONT 2.1", "STARTFONT 2.3"), FontErrorKind::Version),
            (v.replace("COMMENT anywhere", "METRICSSET 0"), FontErrorKind::UnknownKeyword),
            (v.replace("COMMENT anywhere", "COMMENT anywhere\n"), FontErrorKind::BadLine),
            (v.replace("COMMENT anywhere", "COMMENT any\u{7}where"), FontErrorKind::BadLine),
            (v.replace("FONTBOUNDINGBOX 12 10 0 0", "FONTBOUNDINGBOX 6 10 0 0"), FontErrorKind::BoundingBox),
            (v.replace("FONTBOUNDINGBOX 12 10 0 0\n", ""), FontErrorKind::BoundingBox),
            (
                v.replace("FONTBOUNDINGBOX 12 10 0 0", "FONTBOUNDINGBOX 12 10 0 0\nFONTBOUNDINGBOX 12 10 0 0"),
                FontErrorKind::Misplaced,
            ),
            (v.replace("SIZE 10 75 75", "SIZE 10 75"), FontErrorKind::Malformed),
            (v.replace("SIZE 10 75 75", "SIZE 10 75 x"), FontErrorKind::Malformed),
            (v.replace("STARTPROPERTIES 6", "STARTPROPERTIES 7"), FontErrorKind::Properties),
            (v.replace("STARTPROPERTIES 6", "STARTPROPERTIES 5"), FontErrorKind::Properties),
            (v.replace("STARTPROPERTIES 6", "STARTPROPERTIES 65"), FontErrorKind::Properties),
            (v.replace("NOTICE \"CC0\"", "NOTICE \"C\"C0\""), FontErrorKind::Properties),
            (v.replace("NOTICE \"CC0\"", "NOTICE \"CC0"), FontErrorKind::Properties),
            (v.replace("NOTICE \"CC0\"", "notice \"CC0\""), FontErrorKind::Properties),
            (v.replace("NOTICE \"CC0\"", "FAMILY_NAME \"Again\""), FontErrorKind::Properties),
            (v.replace("\"ISO10646\"", "\"ISO8859\""), FontErrorKind::Registry),
            (v.replace("CHARSET_REGISTRY \"ISO10646\"", "PIXEL_SIZE 10"), FontErrorKind::Registry),
            (v.replace("CHARSET_ENCODING \"1\"", "CHARSET_ENCODING \"2\""), FontErrorKind::CharsetEncoding),
            (v.replace("SPACING \"C\"", "SPACING \"P\""), FontErrorKind::Spacing),
            (v.replace("CHARS 3", "CHARS 4"), FontErrorKind::Characters),
            (v.replace("CHARS 3", "CHARS 2"), FontErrorKind::Characters),
            (v.replace("CHARS 3", "CHARS 1025"), FontErrorKind::Characters),
            (v.replace("ENCODING 65", "ENCODING -1"), FontErrorKind::Code),
            (v.replace("ENCODING 65", "ENCODING -1 65"), FontErrorKind::Code),
            (v.replace("ENCODING 65", "ENCODING 55296"), FontErrorKind::Code),
            (v.replace("ENCODING 65", "ENCODING 1114112"), FontErrorKind::Code),
            (v.replace("ENCODING 65", "ENCODING 66"), FontErrorKind::RepeatedCode),
            (v.replacen("DWIDTH 12 0", "DWIDTH 6 0", 1), FontErrorKind::Advance),
            (v.replacen("BBX 12 10 0 0", "BBX 12 9 0 1", 1), FontErrorKind::CharacterBox),
            (v.replacen("0F00", "0F01", 1), FontErrorKind::Row),
            (v.replacen("0F00", "0F0", 1), FontErrorKind::Row),
            (v.replacen("0F00", "0G00", 1), FontErrorKind::Row),
            (v.replacen("0F00\n", "", 1), FontErrorKind::Incomplete),
            (v.replacen("0F00\n", "0F00\n0F00\n", 1), FontErrorKind::Row),
            (v.replacen("DWIDTH 12 0\n", "", 1), FontErrorKind::Incomplete),
            (v.replacen("BITMAP\n", "ENDCHAR\nBITMAP\n", 1), FontErrorKind::Incomplete),
            (v.replace("ENDFONT\n", ""), FontErrorKind::NoEnd),
            (format!("{v}COMMENT after\n"), FontErrorKind::AfterEnd),
            (v.replace("CHARS 3", "STARTCHAR early\nCHARS 3"), FontErrorKind::Misplaced),
        ];
        for (text, expected) in cases {
            assert_eq!(kind(&text), expected, "{text}");
        }
    }

    /// The fault's line is reported, counted from 1.
    #[test]
    fn the_line_is_reported() {
        let text = valid().replace("SPACING \"C\"", "SPACING \"P\"");
        let line = text.lines().position(|line| line.starts_with("SPACING")).unwrap() + 1;
        assert_eq!(refused(&text), FontError { line: u32::try_from(line).unwrap(), kind: FontErrorKind::Spacing });
    }

    /// The size limits of REQ-FNT-002, and a table too small for the font.
    #[test]
    fn limits() {
        let too_large = format!("{}{}", valid(), "\n".repeat(FONT_MAX_FILE_BYTES));
        assert_eq!(refused(&too_large), FontError { line: 0, kind: FontErrorKind::TooLarge });
        let long = valid().replace("COMMENT anywhere", &format!("COMMENT {}", "x".repeat(FONT_MAX_LINE_BYTES)));
        assert_eq!(kind(&long), FontErrorKind::LineTooLong);
        let mut small = [FontGlyph::EMPTY; 2];
        assert_eq!(read(valid().as_bytes(), &mut small).unwrap_err().kind, FontErrorKind::Characters);
        // 1,024 characters fit.
        let all: Vec<String> = (0..1024).map(|code| character(0x100 + code, BLANK_ROWS)).collect();
        let full = font(&all, |text| text);
        let mut glyphs = table();
        assert_eq!(read(full.as_bytes(), &mut glyphs).unwrap().characters, 1024);
    }
}
