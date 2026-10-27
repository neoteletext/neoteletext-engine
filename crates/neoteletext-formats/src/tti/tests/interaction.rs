//! The interaction and author lines: `TF` and `IM` (withdrawn),
//! `CU` (REQ-INT-006) and `LG` (REQ-INP-026).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::cast_possible_truncation
)]

use super::*;

const FONT_SHA: &str = "5ee6246b7f9bd197f47ce0741ea799a55ac08af775869a08c085803e02a3f14d";

/// Decision E.111: a page no longer names its font. A `TF` line, well
/// formed or not, is reported as a withdrawn command and nothing of it is
/// kept; the page is read as if it were absent.
#[test]
fn a_font_line_is_a_withdrawn_command() {
    for line in [std::format!("TF,fonts/teletext.bdf,{FONT_SHA}"), "TF,a.bdf,123".into(), "TF".into()] {
        let text = std::format!("PN,10000\n{line}\nOL,1,STILL SHOWN\n");
        let (pages, diagnostics) = read(text.as_bytes());
        assert!(pages[0].issues.as_slice().iter().any(|issue| issue.kind == IssueKind::WithdrawnCommand), "{line}");
        assert_eq!(diagnostics.len(), 1, "{line}: {diagnostics:?}");
        let (without, _) = read(b"PN,10000\nOL,1,STILL SHOWN\n");
        assert_eq!(pages[0].rows, without[0].rows, "{line}");
    }
}

/// Decision E.125: the input modes are withdrawn; an `IM` line is reported
/// as a withdrawn command and changes nothing of the page.
#[test]
fn a_modes_line_is_withdrawn() {
    for line in ["IM,72", "IM,x", "IM"] {
        let text = std::format!("PN,30000\n{line}\nOL,1,STILL SHOWN\n");
        let (pages, _) = read(text.as_bytes());
        assert!(pages[0].has_issue(IssueKind::WithdrawnCommand), "{line}");
        let (without, _) = read(b"PN,30000\nOL,1,STILL SHOWN\n");
        assert_eq!(pages[0].rows, without[0].rows, "{line}");
    }
}

/// REQ-INT-006: `CU,row,column,width,flags[,acked]` is the page's cursor
/// record; the last line is kept, the writer writes it back, and a line
/// that cannot be read is reported and dropped.
#[test]
fn a_cursor_line_is_read_kept_and_written_back() {
    let (pages, _) = read(b"PN,10000\nOL,5,Name: \nCU,1,0,0,0\nCU,5,6,20,1,42\nPN,10100\nCU,255,0,0,0\n");
    let cursor = pages[0].cursor.unwrap();
    assert_eq!((cursor.row, cursor.column, cursor.width, cursor.flags, cursor.acked), (5, 6, 20, 1, 42));
    assert_eq!(pages[1].cursor.map(|cursor| (cursor.row, cursor.acked)), Some((255, 0)), "acked defaults to 0");
    #[cfg(feature = "std")]
    {
        let mut written = Vec::new();
        crate::write::write_tti(&[&pages[0]], &mut written).unwrap();
        let (again, _) = read(&written);
        assert_eq!(
            again[0].cursor.map(|cursor| (cursor.row, cursor.column, cursor.width, cursor.flags, cursor.acked)),
            Some((5, 6, 20, 1, 42))
        );
    }
    for line in [b"CU,5,6\n".as_slice(), b"CU,256,0,0,0\n", b"CU,5,6,20,1,70000\n", b"CU,a,0,0,0\n"] {
        let text = [b"PN,10000\n".as_slice(), line].concat();
        let (pages, _) = read(&text);
        assert!(pages[0].cursor.is_none() && pages[0].has_issue(IssueKind::BadCursorLine), "{line:?}");
    }
}

/// The log lines a TTI file gives, in order: level, text, page, line.
fn log_lines(text: &[u8]) -> Vec<(crate::log::LogLevel, std::string::String, Option<u16>, u32)> {
    // REQ-INP-001: the same in any pieces.
    let _ = same_in_any_chunks(TtiReader::new, text);
    let mut lines = Vec::new();
    let mut reader = TtiReader::new();
    let mut take = |item: Item<'_>| {
        if let Item::Log { line, page, line_number } = item {
            lines.push((line.level, line.text.into(), page.map(PageAddress::mpp), line_number));
        }
    };
    reader.feed(text, &mut take);
    reader.finish(&mut take);
    lines
}

/// REQ-INP-026: `LG` lines reach the port as they are read, before any
/// page or within one, and are no part of the page.
#[test]
fn log_lines_are_passed_on_for_the_author() {
    use crate::log::LogLevel::{Error, Information, Warning};
    let file =
        "LG,I,the file starts\r\nPN,10000\r\nOL,1,TEXT\r\nLG,W,feed late: 3 s\r\nPN,20000\r\nLG,E,no data — météo\r\n";
    assert_eq!(
        log_lines(file.as_bytes()),
        [
            (Information, "the file starts".into(), None, 1),
            (Warning, "feed late: 3 s".into(), Some(0x100), 4),
            (Error, "no data — météo".into(), Some(0x200), 6),
        ]
    );
    let (pages, diagnostics) = read(file.as_bytes());
    assert_eq!(pages.len(), 2);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(pages[0].row(1).map(|row| row[..4].to_vec()), Some(b"TEXT".to_vec()), "the page is unchanged");
}

/// REQ-INP-026: a refused line gives no log line and a diagnostic; at most
/// `MAX_LOG_LINES_PER_PAGE` from each `PN`.
#[test]
fn refused_and_surplus_log_lines_are_diagnosed() {
    use crate::log::{LogRefused, MAX_LOG_LINES_PER_PAGE};
    let (_, diagnostics) = read(b"PN,10000\r\nLG,X,level\r\nLG,W,bell\x07\r\nLG,W,\r\n");
    let kinds: Vec<_> = diagnostics.iter().map(|diagnostic| diagnostic.kind).collect();
    assert_eq!(
        kinds,
        [
            DiagnosticKind::Page(IssueKind::BadLogLine(LogRefused::Level)),
            DiagnosticKind::Page(IssueKind::BadLogLine(LogRefused::Character)),
            DiagnosticKind::Page(IssueKind::BadLogLine(LogRefused::Empty)),
        ]
    );
    let mut file = b"PN,10000\r\n".to_vec();
    for _ in 0..=MAX_LOG_LINES_PER_PAGE {
        file.extend_from_slice(b"LG,I,again\r\n");
    }
    file.extend_from_slice(b"PN,20000\r\nLG,I,next page\r\n");
    let lines = log_lines(&file);
    assert_eq!(lines.len(), MAX_LOG_LINES_PER_PAGE + 1, "the surplus line is dropped, the next page's kept");
    assert_eq!(lines.last().map(|line| line.1.as_str()), Some("next page"));
    let (_, diagnostics) = read(&file);
    assert_eq!(
        diagnostics.iter().map(|diagnostic| diagnostic.kind).collect::<Vec<_>>(),
        [DiagnosticKind::Page(IssueKind::TooManyLogLines)]
    );
}

/// REQ-INP-026: the writer writes what the reader takes, and refuses what
/// it would refuse.
// The writer is in `write`, which needs the standard library.
#[cfg(feature = "std")]
#[test]
fn the_writer_writes_log_lines_the_reader_takes() {
    use crate::log::LogLevel;
    let mut out = Vec::new();
    crate::write::write_log_line(LogLevel::Warning, "page 200: no feed", &mut out).unwrap();
    assert_eq!(out, b"LG,W,page 200: no feed\n");
    assert_eq!(log_lines(&out), [(LogLevel::Warning, "page 200: no feed".into(), None, 1)]);
    assert!(crate::write::write_log_line(LogLevel::Error, "two\nlines", &mut out).is_err());
}
