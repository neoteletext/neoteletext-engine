//! What `neottx` says of an input: a live input that could not be read exits
//! as any other that could not be read (REQ-CLI-005). No network is touched:
//! the fetcher refuses a feed it does not know.
#![cfg(feature = "live")]
#![allow(clippy::unwrap_used, missing_docs, reason = "a test")]

mod common;
use common::neottx;

/// The status of an input that could not be read (REQ-CLI-005).
const INPUT_STATUS: i32 = 3;
/// The status of a page or sub-page not found (REQ-CLI-005).
const NOT_FOUND_STATUS: i32 = 1;

// Verifies REQ-90.
#[test]
fn a_live_input_that_could_not_be_read_exits_3() {
    let listing = neottx(&["list", "zxnet:nosuchfeed", "--duration", "2"]);
    assert_eq!(listing.status.code(), Some(INPUT_STATUS), "{}", String::from_utf8_lossy(&listing.stderr));
    let page = neottx(&["text", "zxnet:nosuchfeed", "100", "--duration", "2"]);
    assert_eq!(page.status.code(), Some(INPUT_STATUS), "{}", String::from_utf8_lossy(&page.stderr));
}

/// What `neottx list` says of `input`, a feed on a port nothing listens on,
/// so that no network beyond this machine is touched.
fn listing(input: &str) -> String {
    let output = neottx(&["list", input, "--duration", "1", "--allow-local", "127.0.0.1"]);
    assert_eq!(output.status.code(), Some(INPUT_STATUS));
    String::from_utf8_lossy(&output.stderr).into_owned()
}

// Verifies REQ-186: a `wss` address reaches the feed; it is no identifier.
#[test]
fn a_wss_address_opens_as_a_feed() {
    let said = listing("wss://127.0.0.1:1/ws/teefax");
    assert!(!said.contains("unknown live feed"), "{said}");
}

// Verifies REQ-186: a channel identifier may be given directly, after
// `zxnet:`. It decodes to ["t","127.0.0.1:1","/ws/t"].
#[test]
fn a_channel_identifier_opens_as_a_feed() {
    let said = listing("zxnet:WyJ0IiwiMTI3LjAuMC4xOjEiLCIvd3MvdCJd");
    assert!(!said.contains("unknown live feed"), "{said}");
}

// Verifies REQ-90: a file's statuses, for `text`: 1 for a page not found, and
// 3 for a file missing and for one that holds no teletext.
#[test]
fn a_files_statuses_are_those_of_the_requirement() {
    let folder = common::folder("exit-statuses");
    let page = folder.join("page.tti");
    std::fs::write(&page, "PN,10000\nOL,1,HELLO\n").unwrap();
    let text = |file: &std::path::Path, number: &str| neottx(&["text", file.to_str().unwrap(), number]).status.code();
    assert_eq!(text(&page, "100"), Some(0));
    assert_eq!(text(&page, "200"), Some(NOT_FOUND_STATUS));
    assert_eq!(text(&folder.join("missing.tti"), "100"), Some(INPUT_STATUS));
    let empty = folder.join("empty.t42");
    std::fs::write(&empty, "").unwrap();
    assert_eq!(text(&empty, "100"), Some(INPUT_STATUS));
}

// Verifies REQ-171: a stream may be chosen by programme.
#[test]
fn a_program_may_be_named() {
    let folder = common::folder("program-option");
    let page = folder.join("page.tti");
    std::fs::write(&page, "PN,10000\nOL,1,HELLO\n").unwrap();
    let output = neottx(&["list", page.to_str().unwrap(), "--program", "2"]);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
}
