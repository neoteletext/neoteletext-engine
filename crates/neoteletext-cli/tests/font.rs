//! `neottx font convert` (REQ-FNT-010): an exact conversion is written in the
//! form of REQ-FNT-002, a refusal names the character and writes nothing
//! usable, and a font outside ISO10646 needs the map.
#![allow(missing_docs)]

use neoteletext_core::consts::FONT_MAX_CHARACTERS;
use neoteletext_core::types::FontGlyph;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn neottx(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_neottx")).args(arguments).output().expect("run neottx")
}

/// A fresh folder of the test's own: tests run in parallel.
fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("make the test folder");
    folder
}

/// A 6 by 10 source font with characters 65 and 66, both a block in the
/// middle of the cell, in the given registry.
fn source(registry: &str, rows_of_b: [&str; 10]) -> String {
    let block = ["00", "00", "78", "78", "78", "78", "78", "78", "00", "00"];
    let mut text = format!(
        "STARTFONT 2.1\nFONT -test-source\nSIZE 10 75 75\nFONTBOUNDINGBOX 6 10 0 0\nSTARTPROPERTIES 3\n\
         FAMILY_NAME \"Test\"\nCHARSET_REGISTRY \"{registry}\"\nCHARSET_ENCODING \"1\"\nENDPROPERTIES\nCHARS 2\n"
    );
    for (code, rows) in [(65, block), (66, rows_of_b)] {
        let _ = write!(text, "STARTCHAR c{code}\nENCODING {code}\nDWIDTH 6 0\nBBX 6 10 0 0\nBITMAP\n");
        for row in rows {
            text.push_str(row);
            text.push('\n');
        }
        text.push_str("ENDCHAR\n");
    }
    text.push_str("ENDFONT\n");
    text
}

const BLOCK: [&str; 10] = ["00", "00", "78", "78", "78", "78", "78", "78", "00", "00"];

#[test]
fn an_exact_conversion_is_written_and_loads() {
    let folder = folder("font-convert-exact");
    let input = folder.join("source.bdf");
    let output = folder.join("font.bdf");
    std::fs::write(&input, source("ISO10646", BLOCK)).unwrap();
    let run = neottx(&["font", "convert", input.to_str().unwrap(), output.to_str().unwrap()]);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let written = std::fs::read(&output).unwrap();
    let mut glyphs = vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS];
    let font = neoteletext_formats::bdf::read(&written, &mut glyphs).expect("REQ-FNT-002 form");
    assert_eq!((font.characters, font.family), (2, Some(&b"Test"[..])));
    // 0x78 in six pixels is ".####." doubled: "..########..".
    assert_eq!(glyphs[0].rows, [0, 0, 0x3FC, 0x3FC, 0x3FC, 0x3FC, 0x3FC, 0x3FC, 0, 0]);
}

#[test]
fn refusals_name_the_character() {
    let folder = folder("font-convert-refusals");
    let input = folder.join("source.bdf");
    let output = folder.join("font.bdf");
    std::fs::write(&input, source("Teletext", BLOCK)).unwrap();
    let run = neottx(&["font", "convert", input.to_str().unwrap(), output.to_str().unwrap()]);
    assert_eq!(run.status.code(), Some(3));
    let message = String::from_utf8_lossy(&run.stderr);
    assert!(message.contains("character 65") && message.contains("--map"), "{message}");
    let map = folder.join("map.txt");
    std::fs::write(&map, "65\tU+0041\n66\tU+0042\n").unwrap();
    let run =
        neottx(&["font", "convert", "--map", map.to_str().unwrap(), input.to_str().unwrap(), output.to_str().unwrap()]);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    // A cell with no exact conversion is refused at its line.
    std::fs::write(&input, source("ISO10646", BLOCK).replace("FONTBOUNDINGBOX 6 10", "FONTBOUNDINGBOX 8 10")).unwrap();
    let run = neottx(&["font", "convert", input.to_str().unwrap(), output.to_str().unwrap()]);
    let message = String::from_utf8_lossy(&run.stderr);
    assert!(!run.status.success() && message.contains(":4: the cell is not"), "{message}");
}

/// A 12 x 10 font of every character of G0 Latin with the English option,
/// each drawn with one dot clear of the top line and the left column, under
/// a family name.
fn english_font(family: &str) -> Vec<u8> {
    use neoteletext_core::charset::{CharSet, G0, G0Set, NationalOption};
    use neoteletext_core::glyphs::conformance::assigned;
    let set = CharSet::G0(G0 { set: G0Set::Latin, national: Some(NationalOption::English) });
    let mut glyphs: Vec<FontGlyph> = (0x20u8..=0x7F)
        .filter_map(|code| assigned(set, code))
        .map(|c| FontGlyph {
            code_point: u32::from(c),
            rows: if c == ' ' { [0; 10] } else { [0, 0x0C0, 0x0C0, 0, 0, 0, 0, 0, 0, 0] },
        })
        .collect();
    glyphs.sort_by_key(|glyph| glyph.code_point);
    let text = neoteletext_formats::write::FontText {
        family: Some(family.as_bytes()),
        copyright: None,
        notice: None,
        comment: "test",
    };
    let mut out = Vec::new();
    neoteletext_formats::write::write_font(&mut out, &text, &glyphs).unwrap();
    out
}

/// REQ-FNT-011: `neottx font sets` reports each set, and the library line
/// names exactly the sets the font conforms for; the name does not count.
#[test]
fn font_sets_reports_conformance() {
    let folder = folder("font-sets");
    let font = folder.join("english.bdf");
    std::fs::write(&font, english_font("English test")).unwrap();
    let run = neottx(&["font", "sets", font.to_str().unwrap()]);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let report = String::from_utf8(run.stdout).unwrap();
    let line = |set: &str| report.lines().find(|line| line.split_whitespace().next() == Some(set)).unwrap().to_string();
    assert!(line("G0-Latin-English").ends_with("96/96  conforms, SAA5050 cell"), "{report}");
    assert!(line("G0-Latin-German").contains("rule 2: no glyph at 4/0"), "German § is missing: {report}");
    assert_eq!(report.lines().count(), 24);
    let run = neottx(&["font", "sets", "--library-line", "english.bdf", font.to_str().unwrap()]);
    assert_eq!(String::from_utf8(run.stdout).unwrap(), "c\tenglish.bdf\tG0-Latin-English*\n");
    // Rule 5: another family name, the same glyphs, the same conformance.
    let renamed = folder.join("renamed.bdf");
    std::fs::write(&renamed, english_font("Another name")).unwrap();
    let again = neottx(&["font", "sets", "--library-line", "english.bdf", renamed.to_str().unwrap()]);
    assert_eq!(String::from_utf8(again.stdout).unwrap(), "c\tenglish.bdf\tG0-Latin-English*\n");
}
