//! The reader of the converter's source fonts (REQ-FNT-010): BDF 2.1 and
//! 2.2 files whose cell is one that converts exactly to the 12 by 10 form:
//! 5 by 9, 6 by 9, 5 by 10, 6 by 10, or 12 by 20.
//!
//! The converter reads files of any origin, offline and sandboxed
//! (`docs/spec/design/font-security.md` section 3.10), so this reader is as strict and
//! bounded as the reader of loaded fonts. It accepts every keyword of BDF
//! 2.1, places each glyph in the cell by its `BBX`, refuses a glyph larger
//! than the cell, and refuses any other cell at its `FONTBOUNDINGBOX` line.

use super::{FontError, FontErrorKind, numbers, property_value, split_keyword};
use crate::consts::{
    SOURCE_FONT_MAX_CHARACTERS, SOURCE_FONT_MAX_FILE_BYTES, SOURCE_FONT_MAX_LINE_BYTES, SOURCE_FONT_MAX_PROPERTIES,
};
use neoteletext_core::consts::{CELL_DOTS, CELL_FIELD_LINES, CELL_HEIGHT, CELL_WIDTH, GLYPH_DOTS, GLYPH_LINES};

/// The cells REQ-FNT-010 converts, as width and height in pixels.
pub const SOURCE_CELLS: [(usize, usize); 5] = [
    (GLYPH_DOTS, GLYPH_LINES),
    (CELL_DOTS, GLYPH_LINES),
    (GLYPH_DOTS, CELL_FIELD_LINES),
    (CELL_DOTS, CELL_FIELD_LINES),
    (CELL_WIDTH, CELL_HEIGHT),
];
/// The versions of the format accepted. BDF 2.3 adds greyscale, which has
/// no exact 1-bit form.
const VERSIONS: [&[u8]; 2] = [b"2.1", b"2.2"];
/// Bits in one hexadecimal digit of a bitmap row.
const BITS_PER_DIGIT: usize = 4;
/// Bits in one byte of a bitmap row: rows are padded to whole bytes.
const BITS_PER_BYTE: usize = 8;

/// One glyph of a source font, in its cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SourceGlyph {
    /// The `ENCODING`: a Unicode code point when the font says so, a code of
    /// the font's own encoding otherwise.
    pub encoding: u32,
    /// The cell's lines, top first, each `width` bits with the leftmost
    /// pixel the highest bit; lines below the cell's height are zero.
    pub rows: [u16; CELL_HEIGHT],
}

impl SourceGlyph {
    /// An empty slot of a glyph table.
    pub const EMPTY: Self = Self { encoding: 0, rows: [0; CELL_HEIGHT] };
}

/// A source font that passed every rule. Its glyphs are the first
/// [`characters`](SourceFont::characters) entries of the table given to
/// [`read_source`], in the file's order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SourceFont<'a> {
    /// The cell's width in pixels.
    pub width: usize,
    /// The cell's height in pixels.
    pub height: usize,
    /// `CHARSET_REGISTRY` is ISO10646 and `CHARSET_ENCODING` is 1: every
    /// `ENCODING` is a Unicode code point.
    pub unicode: bool,
    /// `FAMILY_NAME`, as the bytes between its quotes.
    pub family: Option<&'a [u8]>,
    /// `COPYRIGHT`.
    pub copyright: Option<&'a [u8]>,
    /// `NOTICE`.
    pub notice: Option<&'a [u8]>,
    /// How many glyphs the table holds.
    pub characters: usize,
}

/// Where the reader is in the file.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Start,
    Header,
    Properties,
    Characters,
    Character,
    Bitmap,
    End,
}

/// The parts of one character seen so far.
#[derive(Clone, Copy, Default)]
struct Character {
    encoding: Option<u32>,
    /// Width, height, and the cell's top line and left column the glyph
    /// starts at.
    place: Option<(usize, usize, usize, usize)>,
    rows: usize,
    bitmap: [u16; CELL_HEIGHT],
}

#[derive(Default)]
struct Header<'a> {
    cell: Option<(usize, usize, i32, i32)>,
    properties_declared: usize,
    properties_read: usize,
    registry: Option<&'a [u8]>,
    charset_encoding: Option<&'a [u8]>,
    family: Option<&'a [u8]>,
    copyright: Option<&'a [u8]>,
    notice: Option<&'a [u8]>,
    declared_characters: Option<usize>,
}

/// Reads a source font into `glyphs`, which must have room for every
/// character (4,096 is always enough).
pub fn read_source<'a>(bytes: &'a [u8], glyphs: &mut [SourceGlyph]) -> Result<SourceFont<'a>, FontError> {
    if bytes.len() > SOURCE_FONT_MAX_FILE_BYTES {
        return Err(FontError { line: 0, kind: FontErrorKind::TooLarge });
    }
    let mut parser =
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
        parser.line(line, glyphs).map_err(|kind| FontError { line: number, kind })?;
    }
    parser.finish().map_err(|kind| FontError { line: number, kind })
}

struct Parser<'a> {
    section: Section,
    header: Header<'a>,
    character: Character,
    count: usize,
}

impl<'a> Parser<'a> {
    fn line(&mut self, line: &'a [u8], glyphs: &mut [SourceGlyph]) -> Result<(), FontErrorKind> {
        if line.len() > SOURCE_FONT_MAX_LINE_BYTES {
            return Err(FontErrorKind::LineTooLong);
        }
        if line.iter().any(|&byte| (byte < b' ' && byte != b'\t') || byte == 0x7F) {
            return Err(FontErrorKind::BadLine);
        }
        if self.section == Section::End {
            return if line.is_empty() { Ok(()) } else { Err(FontErrorKind::AfterEnd) };
        }
        let (keyword, value) = split_keyword(line);
        if keyword.is_empty() {
            return Err(FontErrorKind::BadLine);
        }
        if keyword == b"COMMENT" {
            return Ok(());
        }
        match self.section {
            Section::Start => {
                if keyword != b"STARTFONT" {
                    return Err(FontErrorKind::NotBdf);
                }
                if !VERSIONS.contains(&value) {
                    return Err(FontErrorKind::Version);
                }
                self.section = Section::Header;
                Ok(())
            }
            Section::Header => self.global(keyword, value),
            Section::Properties => self.property(keyword, value),
            Section::Characters => self.between(keyword, value, glyphs),
            Section::Character => self.in_character(keyword, value),
            Section::Bitmap => self.row(keyword, value, glyphs),
            Section::End => Err(FontErrorKind::AfterEnd),
        }
    }

    fn global(&mut self, keyword: &[u8], value: &'a [u8]) -> Result<(), FontErrorKind> {
        let header = &mut self.header;
        match keyword {
            b"FONT" | b"CONTENTVERSION" => {}
            b"SIZE" => size(value)?,
            b"METRICSSET" => {
                // Only horizontal writing has a cell to convert.
                if numbers::<1>(value)? != [0] {
                    return Err(FontErrorKind::Malformed);
                }
            }
            b"SWIDTH" | b"DWIDTH" | b"SWIDTH1" | b"DWIDTH1" | b"VVECTOR" => {
                numbers::<2>(value)?;
            }
            b"FONTBOUNDINGBOX" => {
                if header.cell.is_some() {
                    return Err(FontErrorKind::Misplaced);
                }
                let [width, height, x, y] = numbers::<4>(value)?;
                let width = usize::try_from(width).map_err(|_| FontErrorKind::Cell)?;
                let height = usize::try_from(height).map_err(|_| FontErrorKind::Cell)?;
                if !SOURCE_CELLS.contains(&(width, height)) {
                    return Err(FontErrorKind::Cell);
                }
                header.cell = Some((width, height, x, y));
            }
            b"STARTPROPERTIES" => {
                let [declared] = numbers::<1>(value)?;
                let declared = usize::try_from(declared).map_err(|_| FontErrorKind::Properties)?;
                if declared > SOURCE_FONT_MAX_PROPERTIES || header.properties_declared != 0 {
                    return Err(FontErrorKind::Properties);
                }
                header.properties_declared = declared;
                self.section = Section::Properties;
            }
            b"CHARS" => {
                let [declared] = numbers::<1>(value)?;
                let declared = usize::try_from(declared).map_err(|_| FontErrorKind::Characters)?;
                if declared > SOURCE_FONT_MAX_CHARACTERS {
                    return Err(FontErrorKind::Characters);
                }
                if header.cell.is_none() {
                    return Err(FontErrorKind::BoundingBox);
                }
                header.declared_characters = Some(declared);
                self.section = Section::Characters;
            }
            _ => return Err(super::misplaced_or_unknown(keyword)),
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
        if header.properties_read > header.properties_declared {
            return Err(FontErrorKind::Properties);
        }
        let text = property_value(value)?;
        let slot = match name {
            b"CHARSET_REGISTRY" => &mut header.registry,
            b"CHARSET_ENCODING" => &mut header.charset_encoding,
            b"FAMILY_NAME" => &mut header.family,
            b"COPYRIGHT" => &mut header.copyright,
            b"NOTICE" => &mut header.notice,
            _ => return Ok(()),
        };
        if slot.is_some() || text.is_none() {
            return Err(FontErrorKind::Properties);
        }
        *slot = text;
        Ok(())
    }

    fn between(&mut self, keyword: &[u8], value: &[u8], glyphs: &[SourceGlyph]) -> Result<(), FontErrorKind> {
        match keyword {
            b"STARTCHAR" => {
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
            _ => Err(super::misplaced_or_unknown(keyword)),
        }
    }

    fn in_character(&mut self, keyword: &[u8], value: &[u8]) -> Result<(), FontErrorKind> {
        let character = &mut self.character;
        match keyword {
            b"ENCODING" => {
                if character.encoding.is_some() {
                    return Err(FontErrorKind::Misplaced);
                }
                // An unencoded glyph (`ENCODING -1`) has nothing to map.
                let [code] = numbers::<1>(value).map_err(|_| FontErrorKind::Code)?;
                character.encoding = Some(u32::try_from(code).map_err(|_| FontErrorKind::Code)?);
            }
            b"SWIDTH" | b"DWIDTH" | b"SWIDTH1" | b"DWIDTH1" | b"VVECTOR" => {
                numbers::<2>(value)?;
            }
            b"BBX" => {
                if character.place.is_some() {
                    return Err(FontErrorKind::Misplaced);
                }
                character.place = Some(place(numbers::<4>(value)?, self.header.cell)?);
            }
            b"BITMAP" => {
                if !value.is_empty() {
                    return Err(FontErrorKind::Malformed);
                }
                if character.encoding.is_none() || character.place.is_none() {
                    return Err(FontErrorKind::Incomplete);
                }
                self.section = Section::Bitmap;
            }
            b"ENDCHAR" => return Err(FontErrorKind::Incomplete),
            _ => return Err(super::misplaced_or_unknown(keyword)),
        }
        Ok(())
    }

    fn row(&mut self, keyword: &[u8], value: &[u8], glyphs: &mut [SourceGlyph]) -> Result<(), FontErrorKind> {
        let character = &mut self.character;
        let (width, height, top, left) = character.place.ok_or(FontErrorKind::Incomplete)?;
        let (cell_width, _, _, _) = self.header.cell.ok_or(FontErrorKind::BoundingBox)?;
        if keyword == b"ENDCHAR" {
            if !value.is_empty() {
                return Err(FontErrorKind::Malformed);
            }
            if character.rows != height {
                return Err(FontErrorKind::Incomplete);
            }
            let encoding = character.encoding.ok_or(FontErrorKind::Incomplete)?;
            let slot = glyphs.get_mut(self.count).ok_or(FontErrorKind::Characters)?;
            *slot = SourceGlyph { encoding, rows: character.bitmap };
            self.count = self.count.saturating_add(1);
            self.section = Section::Characters;
            return Ok(());
        }
        let digits = width.div_ceil(BITS_PER_BYTE).saturating_mul(BITS_PER_BYTE / BITS_PER_DIGIT);
        if !value.is_empty() || keyword.len() != digits || character.rows >= height {
            return Err(FontErrorKind::Row);
        }
        let mut bits: u32 = 0;
        for &digit in keyword {
            let value = (digit as char).to_digit(16).ok_or(FontErrorKind::Row)?;
            bits = (bits << BITS_PER_DIGIT) | value;
        }
        // The row's pixels are its leftmost `width` bits; the padding to a
        // whole byte is not part of the glyph.
        let padded = digits.saturating_mul(BITS_PER_DIGIT);
        let pixels = bits >> padded.saturating_sub(width);
        // Placed in the cell: `left` columns in from the cell's left edge.
        let shift = cell_width.saturating_sub(left).saturating_sub(width);
        let line = u16::try_from(pixels << shift).map_err(|_| FontErrorKind::GlyphTooLarge)?;
        let slot = character.bitmap.get_mut(top.saturating_add(character.rows)).ok_or(FontErrorKind::GlyphTooLarge)?;
        *slot = line;
        character.rows = character.rows.saturating_add(1);
        Ok(())
    }

    fn finish(self) -> Result<SourceFont<'a>, FontErrorKind> {
        match self.section {
            Section::End => {}
            Section::Start => return Err(FontErrorKind::NotBdf),
            _ => return Err(FontErrorKind::NoEnd),
        }
        let (width, height, _, _) = self.header.cell.ok_or(FontErrorKind::BoundingBox)?;
        let unicode = self.header.registry.is_some_and(|registry| registry.eq_ignore_ascii_case(b"ISO10646"))
            && self.header.charset_encoding == Some(b"1");
        Ok(SourceFont {
            width,
            height,
            unicode,
            family: self.header.family,
            copyright: self.header.copyright,
            notice: self.header.notice,
            characters: self.count,
        })
    }
}

/// `SIZE`: point size and resolutions, and in BDF 2.3 a bit depth, which
/// must then be 1.
fn size(value: &[u8]) -> Result<(), FontErrorKind> {
    if numbers::<3>(value).is_ok() {
        return Ok(());
    }
    let [_, _, _, depth] = numbers::<4>(value)?;
    if depth == 1 { Ok(()) } else { Err(FontErrorKind::Greyscale) }
}

/// Where a glyph's `BBX` puts it in the cell: its width and height, and the
/// cell's top line and left column it starts at. A glyph reaching outside
/// the cell is refused (REQ-FNT-010).
fn place(
    [width, height, x, y]: [i32; 4],
    cell: Option<(usize, usize, i32, i32)>,
) -> Result<(usize, usize, usize, usize), FontErrorKind> {
    let (cell_width, cell_height, cell_x, cell_y) = cell.ok_or(FontErrorKind::BoundingBox)?;
    let too_large = || FontErrorKind::GlyphTooLarge;
    let glyph_width = usize::try_from(width).map_err(|_| too_large())?;
    let glyph_height = usize::try_from(height).map_err(|_| too_large())?;
    let left = usize::try_from(x.checked_sub(cell_x).ok_or_else(too_large)?).map_err(|_| too_large())?;
    // BDF measures up from the baseline; the cell's top is height + y above it.
    let cell_top = i32::try_from(cell_height).map_err(|_| too_large())?.checked_add(cell_y).ok_or_else(too_large)?;
    let glyph_top = height.checked_add(y).ok_or_else(too_large)?;
    let top = usize::try_from(cell_top.checked_sub(glyph_top).ok_or_else(too_large)?).map_err(|_| too_large())?;
    if left.saturating_add(glyph_width) > cell_width || top.saturating_add(glyph_height) > cell_height {
        return Err(too_large());
    }
    Ok((glyph_width, glyph_height, top, left))
}
