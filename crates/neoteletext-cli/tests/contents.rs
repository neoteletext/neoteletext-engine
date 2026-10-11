//! `ttx contents` (REQ-CLI-008): a channel's `contents.txt` written from a
//! folder of page files, which the contents reader of REQ-WEB-025 accepts.
#![allow(missing_docs)]

use std::path::Path;
use std::process::{Command, Output};

fn ttx(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ttx")).args(arguments).output().expect("run ttx")
}

fn write(folder: &Path, name: &str, text: &str) {
    std::fs::write(folder.join(name), text.replace('\n', "\r\n")).expect("write a test file");
}

#[test]
fn a_folder_of_page_files_gives_its_contents() {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contents-test");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join("sub")).expect("make the test folder");
    write(&folder, "100.tti", "DE,News index\nPN,10000\nOL,1,INDEX\n");
    write(&folder, "101.tti", "PN,10100\nOL,1,\nOL,2,  Top story today\n");
    write(&folder, "301.tti", "PN,30100\nOL,1,SPORT\n");
    // Page 400 as T42, converted by ttx itself.
    write(&folder, "400.source", "PN,40000\nOL,1,WEATHER\n");
    let converted = ttx(&[
        "convert",
        folder.join("400.source").to_str().unwrap(),
        "--format",
        "tti",
        "--to",
        "t42",
        "-o",
        folder.join("400.t42").to_str().unwrap(),
    ]);
    assert!(converted.status.success(), "{}", String::from_utf8_lossy(&converted.stderr));
    // Not a channel's file name, a subfolder's page, and an invalid page.
    write(&folder, "notes.txt", "PN,50000\n");
    write(&folder.join("sub"), "500.tti", "PN,50000\nOL,1,ELSEWHERE\n");
    write(&folder, "600.tti", "[% INCLUDE header %]\n");

    let output = ttx(&["contents", folder.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(4), "a file was skipped");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("600.tti") && stderr.contains("skipped"), "{stderr}");
    let text = String::from_utf8(output.stdout).expect("ASCII");
    let lines: Vec<Vec<&str>> = text.lines().map(|line| line.split('\t').collect()).collect();
    let kinds: Vec<(&str, &str, &str)> = lines.iter().map(|f| (f[0], f[1], f.get(2).copied().unwrap_or(""))).collect();
    assert_eq!(
        kinds,
        [
            ("s", "News index", ""),
            ("p", "100", "News index"),
            ("p", "101", "Top story today"),
            ("s", "Magazine 3", ""),
            ("p", "301", "SPORT"),
            ("s", "WEATHER", ""),
            ("p", "400", "WEATHER"),
        ],
        "{text}"
    );
    assert!(lines.iter().filter(|f| f[0] == "p").all(|f| f.len() == 4 && f[3].len() == 10), "every page dated: {text}");
    let entries = neoteletext_web::contents::read(text.as_bytes()).expect("within the limits");
    assert!(entries.into_iter().all(|entry| entry.is_ok()), "REQ-WEB-025 accepts every line");
}
