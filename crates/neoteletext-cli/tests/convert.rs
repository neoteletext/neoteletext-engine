//! `neottx convert --page` (REQ-CLI-003, REQ-INP-014, REQ-INP-018): a
//! selection writes that page, and a sub-code that one sub-page.
#![allow(clippy::unwrap_used, missing_docs, reason = "a test")]

mod common;
use common::{folder, neottx};
use std::fs;
use std::path::Path;

/// The status of a page or sub-page not found (REQ-CLI-005).
const NOT_FOUND_STATUS: i32 = 1;

/// Page 100 with sub-pages 0000 and 0001, and page 200.
const PAGES: &str =
    "PN,10000\nSC,0000\nOL,1,SUB ZERO\nPN,10001\nSC,0001\nOL,1,SUB ONE\nPN,20000\nSC,0000\nOL,1,OTHER\n";

/// Converts `PAGES` selecting `page`, and returns the sub-page's rows the
/// output holds, by listing it back as text.
fn converted(name: &str, to: &str, page: &str) -> Vec<u8> {
    let folder = folder(name);
    let input = folder.join("in.tti");
    fs::write(&input, PAGES).unwrap();
    let output = folder.join(format!("out.{to}"));
    let run = neottx(&["convert", input.to_str().unwrap(), "--to", to, "--page", page, "-o", output.to_str().unwrap()]);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    fs::read(output).unwrap()
}

fn holds(bytes: &[u8], text: &str) -> bool {
    bytes.windows(text.len()).any(|window| window == text.as_bytes())
}

/// The text of `page` of a converted file.
fn text_of(file: &Path, page: &str) -> String {
    let run = neottx(&["text", file.to_str().unwrap(), page]);
    String::from_utf8_lossy(&run.stdout).into_owned()
}

// Verifies REQ-88.
#[test]
fn a_sub_code_selects_one_sub_page_in_tti() {
    let written = converted("convert-sub-page-tti", "tti", "100/0001");
    assert!(holds(&written, "SUB ONE"));
    assert!(!holds(&written, "SUB ZERO"), "the other sub-page");
    assert!(!holds(&written, "OTHER"), "the other page");
}

// Verifies REQ-88.
#[test]
fn a_sub_code_selects_one_sub_page_in_t42() {
    let written = converted("convert-sub-page-t42", "t42", "100/0001");
    let file = Path::new(env!("CARGO_TARGET_TMPDIR")).join("convert-sub-page-t42/out.t42");
    assert_eq!(written.len() % 42, 0);
    let text = text_of(&file, "100");
    assert!(text.contains("SUB ONE"), "{text}");
    let other = neottx(&["text", file.to_str().unwrap(), "100/0000"]);
    assert_eq!(other.status.code(), Some(NOT_FOUND_STATUS), "the other sub-page");
}

// Verifies REQ-88: a page with no sub-code keeps every sub-page.
#[test]
fn a_page_alone_selects_all_its_sub_pages() {
    let written = converted("convert-page", "tti", "100");
    assert!(holds(&written, "SUB ONE") && holds(&written, "SUB ZERO"));
    assert!(!holds(&written, "OTHER"));
}
