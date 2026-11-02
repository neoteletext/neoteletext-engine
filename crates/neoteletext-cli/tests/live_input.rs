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
