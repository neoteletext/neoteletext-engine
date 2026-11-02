//! `neottx slice` (REQ-CLI-001): what cannot be sliced is reported.
#![cfg(feature = "slice")]
#![allow(clippy::unwrap_used, missing_docs, reason = "a test")]

mod common;
use common::{folder, neottx};

// Verifies REQ-184: 5,000 bytes are two lines of 2,048 and 904 left over, a
// partial line that starts at byte 4,096.
#[test]
fn a_partial_line_is_reported_with_its_offset() {
    let folder = folder("slice-partial-line");
    let input = folder.join("r.raw");
    std::fs::write(&input, [0_u8; 5000]).unwrap();
    let output = folder.join("r.t42");
    let run = neottx(&["slice", input.to_str().unwrap(), "-o", output.to_str().unwrap(), "-v"]);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let said = String::from_utf8_lossy(&run.stderr);
    assert!(said.contains("input byte 4096: a partial line of 904 bytes"), "{said}");
}
