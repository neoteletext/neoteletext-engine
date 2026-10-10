//! Level 2.5 and 3.5 on real broadcasts (REQ-TST-022, REQ-TST-025,
//! REQ-L25-010): the TVP recording of 10 November 2023 carries packets X/26
//! that write Polish letters, and a capture of the Teefax service taken
//! over zxnet on 10 October 2026 has pages that write G3 smooth mosaics.
//! The files are read from `$TELLYTXT_CORPUS`, or from
//! `~/devel/tellytxt-corpus`, and never copied into the repository; without
//! them the tests print a note and pass.
#![allow(missing_docs)]

use std::path::PathBuf;
use std::process::Command;

const RECORDING: &str = "tsduck-PL_MUX3_2023-11-10.ts";
/// The Teefax capture: 714,882 bytes of T42, SHA-256
/// 8c65051e37884618f01f35ee15db9a53c0c1ca97d1e74433975ffbb0d98f1ceb.
const TEEFAX: &str = "live/teefax-2026-10-10.t42";

fn recording() -> Option<PathBuf> {
    corpus_file(RECORDING)
}

fn corpus_file(name: &str) -> Option<PathBuf> {
    let directory = std::env::var_os("TELLYTXT_CORPUS").map(PathBuf::from).or_else(|| {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("devel/tellytxt-corpus"))
    })?;
    let path = directory.join(name);
    if path.is_file() {
        Some(path)
    } else {
        eprintln!("SKIPPED: corpus file {name} not found");
        None
    }
}

/// Row `row` of page 341 as `ttx text` writes it at a level.
fn row(path: &PathBuf, level: &str, row: usize) -> Vec<char> {
    page_row(path, "341", level, row)
}

/// Row `row` of a page as `ttx text` writes it at a level.
fn page_row(path: &PathBuf, page: &str, level: &str, row: usize) -> Vec<char> {
    let output = Command::new(env!("CARGO_BIN_EXE_ttx"))
        .arg("text")
        .arg(path)
        .arg(page)
        .args(["--level", level])
        .output()
        .expect("run ttx");
    assert!(output.status.success(), "ttx failed: {}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.lines().nth(row).unwrap_or_default().chars().collect()
}

/// REQ-L25-010, REQ-L15-008: page 341 places Ą at row 3, column 9 through
/// packet X/26, at Levels 1.5, 2.5 and 3.5 alike; at Level 1 the Level 1
/// character shows.
#[test]
fn polish_letter_from_x26() {
    let Some(path) = recording() else { return };
    for level in ["1.5", "2.5", "3.5"] {
        let line = row(&path, level, 3);
        assert_eq!(line.get(9), Some(&'Ą'), "Level {level}: {}", line.iter().collect::<String>());
    }
    assert_eq!(row(&path, "1", 3).get(9), Some(&'A'));
}

/// REQ-TST-025, REQ-L25-010, REQ-CHR-011: Teefax page 581 rounds the roof
/// of a house with G3 smooth mosaics written by packet X/26 (U+1FB47 and
/// U+1FB44 at row 8, columns 4 and 5); at Level 1.5 the G1 block mosaic of
/// the Level 1 row shows there instead (U+1FB35 at column 5).
#[test]
fn teefax_smooth_mosaics_from_x26() {
    let Some(path) = corpus_file(TEEFAX) else { return };
    for level in ["2.5", "3.5"] {
        let line = page_row(&path, "581", level, 8);
        assert_eq!(
            line.get(4..6),
            Some(&['\u{1FB47}', '\u{1FB44}'][..]),
            "Level {level}: {}",
            line.iter().collect::<String>()
        );
    }
    let line = page_row(&path, "581", "1.5", 8);
    assert_eq!(line.get(4..6), Some(&[' ', '\u{1FB35}'][..]), "{}", line.iter().collect::<String>());
}
