//! The font writer (REQ-FNT-010): a glyph table in the 12 by 10 or 12 by 20
//! BDF form of REQ-FNT-002 (decision E.77), which the reader of loaded fonts
//! accepts.

use std::io::{self, Write};
use std::string::String;
use std::vec::Vec;
use std::{format, vec};

use neoteletext_core::consts::{CELL_FIELD_LINES, CELL_HEIGHT, CELL_WIDTH, FONT_MAX_CHARACTERS, FONT_MAX_LINE_BYTES};
use neoteletext_core::types::FontGlyph;

/// Hexadecimal digits of one bitmap row: 12 dots padded to 16 bits.
const ROW_DIGITS: usize = 4;
/// The padding bits at the right of each row.
const ROW_PAD_BITS: usize = ROW_DIGITS * 4 - CELL_WIDTH;
/// The scalable width BDF 2.1 asks for: the advance in thousandths of the
/// point size, 12 pixels of a 10-pixel font ...
const SWIDTH_SMALL: usize = CELL_WIDTH * 1000 / CELL_FIELD_LINES;
/// ... and of a 20-pixel font (decision E.77).
const SWIDTH_TALL: usize = CELL_WIDTH * 1000 / CELL_HEIGHT;

/// The texts a converted font carries over: the bytes between the quotes
/// of the source's `FAMILY_NAME`, `COPYRIGHT` and `NOTICE`, doubled quotes
/// as written, and a comment saying where it came from.
#[derive(Clone, Copy, Debug, Default)]
pub struct FontText<'a> {
    /// `FAMILY_NAME`.
    pub family: Option<&'a [u8]>,
    /// `COPYRIGHT`.
    pub copyright: Option<&'a [u8]>,
    /// `NOTICE`.
    pub notice: Option<&'a [u8]>,
    /// A `COMMENT` line's text.
    pub comment: &'a str,
}

/// Writes `glyphs`, sorted by code point, as a REQ-FNT-002 font, 12 by 20
/// when they are tall and 12 by 10 otherwise. A line that would be longer
/// than REQ-FNT-002 allows, more glyphs than it allows, or glyphs of two
/// heights, is an error and nothing more is written.
pub fn write_font(out: &mut dyn Write, text: &FontText<'_>, glyphs: &[FontGlyph]) -> io::Result<()> {
    if glyphs.len() > FONT_MAX_CHARACTERS {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "more than 1,024 characters"));
    }
    let tall = glyphs.first().is_some_and(|glyph| glyph.tall);
    if glyphs.iter().any(|glyph| glyph.tall != tall) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "glyphs of two heights"));
    }
    let (height, swidth) = if tall { (CELL_HEIGHT, SWIDTH_TALL) } else { (CELL_FIELD_LINES, SWIDTH_SMALL) };
    let mut lines: Vec<Vec<u8>> = vec![b"STARTFONT 2.1".to_vec()];
    for part in text.comment.lines() {
        lines.push([b"COMMENT ".as_slice(), part.as_bytes()].concat());
    }
    lines.push(b"FONT -neoteletext-converted-medium-r-normal--10-100-75-75-c-120-iso10646-1".to_vec());
    lines.push(format!("SIZE {height} 75 75").into_bytes());
    lines.push(format!("FONTBOUNDINGBOX {CELL_WIDTH} {height} 0 0").into_bytes());
    let mut properties: Vec<Vec<u8>> = Vec::new();
    for (name, value) in [("FAMILY_NAME", text.family), ("COPYRIGHT", text.copyright), ("NOTICE", text.notice)] {
        if let Some(value) = value {
            properties.push([name.as_bytes(), b" \"", value, b"\""].concat());
        }
    }
    properties.push(b"CHARSET_REGISTRY \"ISO10646\"".to_vec());
    properties.push(b"CHARSET_ENCODING \"1\"".to_vec());
    properties.push(b"SPACING \"C\"".to_vec());
    lines.push(format!("STARTPROPERTIES {}", properties.len()).into_bytes());
    lines.extend(properties);
    lines.push(b"ENDPROPERTIES".to_vec());
    lines.push(format!("CHARS {}", glyphs.len()).into_bytes());
    for glyph in glyphs {
        lines.push(format!("STARTCHAR U+{:04X}", glyph.code_point).into_bytes());
        lines.push(format!("ENCODING {}", glyph.code_point).into_bytes());
        lines.push(format!("SWIDTH {swidth} 0").into_bytes());
        lines.push(format!("DWIDTH {CELL_WIDTH} 0").into_bytes());
        lines.push(format!("BBX {CELL_WIDTH} {height} 0 0").into_bytes());
        lines.push(b"BITMAP".to_vec());
        for &row in glyph.lines() {
            lines.push(format!("{:0ROW_DIGITS$X}", u32::from(row) << ROW_PAD_BITS).into_bytes());
        }
        lines.push(b"ENDCHAR".to_vec());
    }
    lines.push(b"ENDFONT".to_vec());
    if let Some(long) = lines.iter().find(|line| line.len() > FONT_MAX_LINE_BYTES) {
        let start = String::from_utf8_lossy(long.get(..20).unwrap_or(long)).into_owned();
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("a line longer than 512 bytes: {start}…")));
    }
    for line in lines {
        out.write_all(&line)?;
        out.write_all(b"\n")?;
    }
    out.flush()
}
