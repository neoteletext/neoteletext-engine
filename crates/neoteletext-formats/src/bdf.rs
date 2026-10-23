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

use neoteletext_core::types::FontGlyph;

pub mod convert;
mod reader;
pub mod source;

pub use reader::read;

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
