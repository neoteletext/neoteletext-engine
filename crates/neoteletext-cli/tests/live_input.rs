//! A live input that could not be read exits as any other input that could
//! not be read (REQ-CLI-005). No network is touched: the fetcher refuses a
//! feed it does not know.
#![allow(clippy::unwrap_used, missing_docs, reason = "a test")]

mod common;
use common::neottx;

/// The status of an input that could not be read (REQ-CLI-005).
const INPUT_STATUS: i32 = 3;

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
