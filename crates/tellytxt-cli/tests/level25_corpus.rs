//! Level 2.5 and 3.5 on a real broadcast (REQ-TST-022, REQ-L25-010): the
//! TVP recording of 10 November 2023 carries packets X/26 that write
//! Polish letters. The file is read from `$TELLYTXT_CORPUS`, or from
//! `~/devel/tellytxt-corpus`, and never copied into the repository; without
//! it the test prints a note and passes.
#![allow(missing_docs)]

use std::path::PathBuf;
use std::process::Command;

const RECORDING: &str = "tsduck-PL_MUX3_2023-11-10.ts";

fn recording() -> Option<PathBuf> {
    let directory = std::env::var_os("TELLYTXT_CORPUS").map(PathBuf::from).or_else(|| {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("devel/tellytxt-corpus"))
    })?;
    let path = directory.join(RECORDING);
    if path.is_file() {
        Some(path)
    } else {
        eprintln!("SKIPPED: corpus file {RECORDING} not found");
        None
    }
}

/// Row `row` of page 341 as `ttx text` writes it at a level.
fn row(path: &PathBuf, level: &str, row: usize) -> Vec<char> {
    let output = Command::new(env!("CARGO_BIN_EXE_ttx"))
        .arg("text")
        .arg(path)
        .arg("341")
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
