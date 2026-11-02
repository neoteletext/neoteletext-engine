//! `neottx contents` (REQ-CLI-008): a channel's `contents.txt` written from a
//! folder of page files, which the contents reader of REQ-WEB-025 accepts.
#![allow(missing_docs)]

mod common;

use common::neottx;
use std::path::Path;

fn write(folder: &Path, name: &str, text: &str) {
    std::fs::write(folder.join(name), text.replace('\n', "\r\n")).expect("write a test file");
}

// Verifies REQ-WEB-025.
#[test]
fn a_folder_of_page_files_gives_its_contents() {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contents-test");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join("sub")).expect("make the test folder");
    write(&folder, "100.tti", "DE,News index\nPN,10000\nOL,1,INDEX\n");
    write(&folder, "101.tti", "PN,10100\nOL,1,\nOL,2,  Top story today\n");
    write(&folder, "301.tti", "PN,30100\nOL,1,SPORT\n");
    // Page 400 as T42, converted by neottx itself.
    write(&folder, "400.source", "PN,40000\nOL,1,WEATHER\n");
    let converted = neottx(&[
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

    let output = neottx(&["contents", folder.to_str().unwrap()]);
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

// Verifies REQ-1885, REQ-1887: a page's folder belongs to its channel. A page
// held only there is listed, from its lowest sub-page, and any other folder
// is a channel of its own.
#[test]
fn a_page_in_its_own_folder_is_listed_from_its_first_sub_page() {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contents-page-folder");
    let _ = std::fs::remove_dir_all(&folder);
    for inner in ["602", "1FE", "other", "90A", "2fa"] {
        std::fs::create_dir_all(folder.join(inner)).expect("make the test folders");
    }
    write(&folder, "100.tti", "PN,10000\nOL,1,INDEX\n");
    write(&folder.join("602"), "0102.tti", "PN,60200\nSC,0102\nOL,1,SECOND\n");
    write(&folder.join("602"), "0101.tti", "DE,Headlines\nPN,60200\nSC,0101\nOL,1,FIRST\n");
    write(&folder.join("602"), "0.sub", "2\tv\n0101\tv\n0102\tv\n");
    write(&folder.join("602"), "index.html", "<!doctype html>\n");
    // A page file wins over its folder.
    write(&folder, "103.tti", "PN,10300\nOL,1,PAGE FILE\n");
    std::fs::create_dir_all(folder.join("103")).expect("make the test folder");
    write(&folder.join("103"), "0001.tti", "PN,10300\nSC,0001\nOL,1,IN THE FOLDER\n");
    // None of these is a page's folder or a file of one.
    write(&folder.join("other"), "500.tti", "PN,50000\nOL,1,ELSEWHERE\n");
    write(&folder.join("90A"), "0001.tti", "PN,70000\nSC,0001\nOL,1,NO SUCH MAGAZINE\n");
    write(&folder.join("2fa"), "0001.tti", "PN,2FA00\nSC,0001\nOL,1,SMALL LETTERS\n");
    write(&folder.join("1FE"), "notes.tti", "PN,1FE00\nOL,1,NOT A SUB-CODE\n");

    let output = neottx(&["contents", folder.to_str().unwrap()]);
    let text = String::from_utf8(output.stdout).expect("ASCII");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let pages: Vec<(&str, &str)> = text
        .lines()
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .filter(|fields| fields[0] == "p")
        .map(|fields| (fields[1], fields[2]))
        .collect();
    assert_eq!(pages, [("100", "INDEX"), ("103", "PAGE FILE"), ("602", "Headlines")], "{text}");
}

// Verifies REQ-1904: a sub-page's file past its bound is skipped, whole.
#[test]
fn a_sub_pages_file_past_its_bound_is_skipped() {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contents-too-large");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join("200")).expect("make the test folder");
    let packet = [0x15u8; neoteletext_core::consts::PACKET_BYTES];
    let packets = neoteletext_web::subpages::MAX_SUBPAGE_FILE / packet.len() + 1;
    std::fs::write(folder.join("200").join("0001.t42"), packet.repeat(packets)).expect("write a test file");
    let output = neottx(&["contents", folder.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(4), "a file was skipped");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("0001.t42") && stderr.contains("larger than"), "{stderr}");
}

/// REQ-152: a page file over the largest page a reader accepts is skipped by
/// its length, before it is read.
#[test]
fn a_page_file_over_the_limit_is_skipped_before_it_is_read() {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contents-limit");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let limit = neoteletext_web::consts::DEFAULT_PAGE_LIMIT;
    let file = std::fs::File::create(folder.join("100.tti")).unwrap();
    file.set_len(limit as u64 + 1).unwrap();
    let output = neottx(&["contents", folder.to_str().unwrap()]);
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains(&format!("larger than {limit} bytes")), "{message}");
}
