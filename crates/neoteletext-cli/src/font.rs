//! `neottx font convert` (REQ-FNT-010, decision E.72 item 7): a BDF font of
//! another cell size to the 12 by 10 form the browser loads, offline and
//! sandboxed, only where the conversion is exact. `neottx font sets`
//! (REQ-FNT-011, decision E.75): the character sets a font conforms for.

use neoteletext_app::sandbox::{self, Policy};
use neoteletext_app::{Failure, exit};
use neoteletext_core::consts::{FONT_MAX_CHARACTERS, FONT_MAX_FILE_BYTES};
use neoteletext_core::glyphs::LoadedFont;
use neoteletext_core::glyphs::conformance::{FONT_SETS, Failure as RuleFailure, SET_POSITIONS, conformance};
use neoteletext_core::types::FontGlyph;
use neoteletext_formats::bdf::FontErrorKind;
use neoteletext_formats::bdf::convert::{ConvertError, convert, read_map};
use neoteletext_formats::bdf::read;
use neoteletext_formats::bdf::source::{SourceGlyph, read_source};
use neoteletext_formats::consts::{FONT_MAP_MAX_FILE_BYTES, SOURCE_FONT_MAX_CHARACTERS, SOURCE_FONT_MAX_FILE_BYTES};
use neoteletext_formats::write::{FontText, write_font};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Reads a whole file of at most `limit` bytes.
fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, Failure> {
    let file = File::open(path).map_err(|error| (exit::INPUT, format!("{}: {error}", path.display())))?;
    let mut bytes = Vec::new();
    let cap = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    file.take(cap).read_to_end(&mut bytes).map_err(|error| (exit::INPUT, format!("{}: {error}", path.display())))?;
    if bytes.len() > limit {
        return Err((exit::INPUT, format!("{}: larger than {limit} bytes", path.display())));
    }
    Ok(bytes)
}

/// `neottx font convert INPUT OUTPUT [--map FILE]`.
pub fn convert_font(input: &Path, output: &Path, map: Option<&Path>, no_sandbox: bool) -> Result<(), Failure> {
    let source = read_bounded(input, SOURCE_FONT_MAX_FILE_BYTES)?;
    let map_text = map.map(|path| read_bounded(path, FONT_MAP_MAX_FILE_BYTES)).transpose()?;
    let mut out = File::create(output).map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
    // Every file is open: nothing that follows needs the file system or the
    // network (REQ-SEC-017).
    sandbox::apply(Policy { writable_directory: None, network: false, readable: &[] }, no_sandbox)
        .map_err(|message| (exit::INPUT, message))?;
    let name = input.display();
    let mut glyphs = vec![SourceGlyph::EMPTY; SOURCE_FONT_MAX_CHARACTERS];
    let font = read_source(&source, &mut glyphs)
        .map_err(|error| (exit::INPUT, format!("{name}:{}: {}", error.line, source_fault(error.kind))))?;
    let mut pairs = vec![(0u32, 0u32); SOURCE_FONT_MAX_CHARACTERS];
    let pairs = match (&map_text, map) {
        (Some(text), Some(path)) => {
            let count = read_map(text, &mut pairs).map_err(|error| {
                (exit::INPUT, format!("{}:{}: not a `code<TAB>U+XXXX` line", path.display(), error.line))
            })?;
            pairs.get(..count)
        }
        _ => None,
    };
    let mut table = vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS];
    let count = convert(&font, &glyphs, pairs, &mut table)
        .map_err(|error| (exit::INPUT, format!("{name}: {}", conversion_fault(error))))?;
    let comment = format!(
        "Converted by neottx font convert from a {} x {} cell (REQ-FNT-010); every pixel kept.",
        font.width, font.height
    );
    let text = FontText { family: font.family, copyright: font.copyright, notice: font.notice, comment: &comment };
    let mut written = Vec::new();
    write_font(&mut written, &text, table.get(..count).unwrap_or(&[]))
        .map_err(|error| (exit::INPUT, format!("{name}: {error}")))?;
    out.write_all(&written)
        .and_then(|()| out.flush())
        .map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
    eprintln!("{}: {count} characters from a {} x {} cell", output.display(), font.width, font.height);
    Ok(())
}

/// Reads a loaded font for drawing pages (`--font`, REQ-FNT-008): its glyph
/// table, checked by the reader of REQ-FNT-002 and for shape. Called before
/// the sandbox closes the file system.
pub fn read_table(path: &Path) -> Result<Vec<FontGlyph>, Failure> {
    let bytes = read_bounded(path, FONT_MAX_FILE_BYTES)?;
    let name = path.display();
    let mut glyphs = vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS];
    let font = read(&bytes, &mut glyphs)
        .map_err(|error| (exit::INPUT, format!("{name}:{}: {:?} (REQ-FNT-002)", error.line, error.kind)))?;
    glyphs.truncate(font.characters);
    LoadedFont::new(&glyphs).map_err(|error| (exit::INPUT, format!("{name}: {error:?}")))?;
    Ok(glyphs)
}

/// `neottx font sets FONT [--library-line NAME]`.
pub fn font_sets(path: &Path, library_line: Option<&str>, no_sandbox: bool) -> Result<(), Failure> {
    let bytes = read_bounded(path, FONT_MAX_FILE_BYTES)?;
    sandbox::apply(Policy { writable_directory: None, network: false, readable: &[] }, no_sandbox)
        .map_err(|message| (exit::INPUT, message))?;
    let name = path.display();
    let mut glyphs = vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS];
    let font = read(&bytes, &mut glyphs)
        .map_err(|error| (exit::INPUT, format!("{name}:{}: {:?} (REQ-FNT-011 rule 1)", error.line, error.kind)))?;
    let table = LoadedFont::new(glyphs.get(..font.characters).unwrap_or(&[]))
        .map_err(|error| (exit::INPUT, format!("{name}: {error:?}")))?;
    let results: Vec<_> = FONT_SETS.iter().map(|(set_name, set)| (*set_name, conformance(&table, *set))).collect();
    if let Some(file) = library_line {
        let sets: Vec<String> = results
            .iter()
            .filter(|(_, result)| result.conforms())
            .map(|(set_name, result)| format!("{set_name}{}", if result.saa5050_cell { "*" } else { "" }))
            .collect();
        println!("c\t{file}\t{}", sets.join(" "));
        return Ok(());
    }
    for (set_name, result) in &results {
        let verdict = match result.failure {
            None if result.saa5050_cell => "conforms, SAA5050 cell".to_string(),
            None => "conforms".to_string(),
            Some(RuleFailure::Missing(code)) => format!("rule 2: no glyph at {:X}/{:X}", code >> 4, code & 0xF),
            Some(RuleFailure::Space) => "rule 3: the space is missing or not blank".to_string(),
            Some(RuleFailure::Blank(code)) => format!("rule 4: blank glyph at {:X}/{:X}", code >> 4, code & 0xF),
        };
        println!("{set_name:34} {:>2}/{SET_POSITIONS}  {verdict}", result.drawn);
    }
    Ok(())
}

fn source_fault(kind: FontErrorKind) -> String {
    match kind {
        FontErrorKind::Cell => {
            "the cell is not 5 x 9, 6 x 9, 5 x 10, 6 x 10 or 12 x 20, so it has no exact 12 x 10 or 12 x 20 form".into()
        }
        FontErrorKind::GlyphTooLarge => "the glyph reaches outside its cell".into(),
        FontErrorKind::Greyscale => "more than one bit per pixel".into(),
        FontErrorKind::Code => "the character has no code (ENCODING must be a number from 0)".into(),
        other => format!("{other:?}"),
    }
}

fn conversion_fault(error: ConvertError) -> String {
    match error {
        ConvertError::Cell => "the cell has no exact 12 x 10 or 12 x 20 form".into(),
        ConvertError::Unmapped { encoding } => {
            format!("character {encoding}: the font is not in ISO10646 and the map (--map) has no line for it")
        }
        ConvertError::NotScalar { encoding } => format!("character {encoding}: not a Unicode code point"),
        ConvertError::Repeated { encoding } => format!("character {encoding}: its code point is already in the font"),
        ConvertError::TooMany => format!("more than {FONT_MAX_CHARACTERS} characters"),
    }
}
