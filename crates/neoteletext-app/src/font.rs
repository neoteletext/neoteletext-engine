//! The browser's font helpers (REQ-FNT-003, REQ-FNT-009,
//! `docs/font-security.md` section 3.1). Fonts are read only in a confined
//! process: the interface sends bytes to a reader and receives a glyph
//! table, whose shape it checks, and never parses a font itself.
//!
//! - [`read_run`], `neoteletext font-read`: reads the bytes of one font
//!   from standard input under the decoder's confinement (nothing at all),
//!   for a cached font or a local file, and answers with its table.
//! - `fetch::run_font`, `neoteletext font-fetch`: fetches one font or one
//!   library index under the fetcher's confinement and answers the same
//!   way.

use crate::pipe::{self, FontData, FontReply, Frame, MAX_FONT_TEXT};
use crate::{Failure, exit};
use neoteletext_core::consts::FONT_MAX_CHARACTERS;
use neoteletext_core::types::FontGlyph;

/// Reads a font (REQ-FNT-002) into the answer the interface receives.
#[must_use]
pub fn read_font(bytes: Vec<u8>) -> FontReply {
    let mut glyphs = vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS];
    match neoteletext_formats::bdf::read(&bytes, &mut glyphs) {
        Ok(font) => {
            glyphs.truncate(font.characters);
            let family = font.family.map(printable);
            let copyright = font.copyright.map(printable);
            let notice = font.notice.map(printable);
            FontReply::Loaded(FontData { bytes, family, copyright, notice, glyphs })
        }
        Err(error) => FontReply::Failed(format!("line {}: {:?}", error.line, error.kind)),
    }
}

/// A font's text made safe to show: the doubled quotes of BDF made single,
/// anything that is not printable left out, and at most [`MAX_FONT_TEXT`]
/// bytes.
#[must_use]
pub fn printable(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw).replace("\"\"", "\"");
    let mut out = String::new();
    for character in text.chars().filter(|character| !character.is_control()) {
        if out.len() + character.len_utf8() > MAX_FONT_TEXT {
            break;
        }
        out.push(character);
    }
    out
}

/// `neoteletext font-read`: reads one [`Frame::FontBytes`] from standard
/// input, confines itself to nothing (REQ-SEC-026), and writes one
/// [`Frame::Font`].
pub fn read_run(no_sandbox: bool, sandboxed_by_parent: bool) -> Result<(), Failure> {
    let mut input = std::io::stdin().lock();
    let frame = pipe::read(&mut input).map_err(|error| (exit::INPUT, error.to_string()))?;
    if !sandboxed_by_parent {
        crate::sandbox::apply(
            crate::sandbox::Policy { writable_directory: None, network: false, readable: &[] },
            no_sandbox,
        )
        .map_err(|message| (exit::INPUT, message))?;
    }
    let reply = match frame {
        Some(Frame::FontBytes(bytes)) => read_font(bytes),
        _ => FontReply::Failed("no font was sent".into()),
    };
    let mut out = std::io::stdout().lock();
    pipe::write(&mut out, &Frame::Font(reply)).map_err(|error| (exit::INPUT, error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &str = "STARTFONT 2.1\nFONTBOUNDINGBOX 12 10 0 0\nSTARTPROPERTIES 5\nFAMILY_NAME \"Test \"\"one\"\"\"\n\
        COPYRIGHT \"(c) nobody\"\nCHARSET_REGISTRY \"ISO10646\"\nCHARSET_ENCODING \"1\"\nSPACING \"C\"\n\
        ENDPROPERTIES\nCHARS 1\nSTARTCHAR A\nENCODING 65\nDWIDTH 12 0\nBBX 12 10 0 0\nBITMAP\n\
        0000\n0F00\n30C0\n6060\n6060\n7FE0\n6060\n6060\n0000\n0000\nENDCHAR\nENDFONT\n";

    #[test]
    fn a_font_is_read_into_its_table_and_texts() {
        let FontReply::Loaded(font) = read_font(FONT.as_bytes().to_vec()) else { panic!("refused") };
        assert_eq!(font.glyphs.len(), 1);
        assert_eq!(font.glyphs[0].rows[1], 0x0F0);
        assert_eq!(font.family.as_deref(), Some("Test \"one\""));
        assert_eq!(font.copyright.as_deref(), Some("(c) nobody"));
        assert_eq!(font.bytes, FONT.as_bytes());
    }

    #[test]
    fn a_bad_font_is_refused_with_its_line() {
        let FontReply::Failed(why) = read_font(FONT.replace("SPACING \"C\"", "SPACING \"P\"").into_bytes()) else {
            panic!("accepted")
        };
        assert!(why.starts_with("line 8:"), "{why}");
        assert_eq!(printable(b"a\x01b\"\"c"), "ab\"c");
        assert_eq!(printable(&[b'x'; 600]).len(), MAX_FONT_TEXT);
    }
}
