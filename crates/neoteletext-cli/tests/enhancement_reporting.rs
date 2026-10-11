//! Enhancement faults in `ttx list` (REQ-L25-051, REQ-ARCH-007) and the
//! walk trace of `-vv` (REQ-FSM-010, REQ-CLI-002), on the
//! authored Level 2.5 and 3.5 stream (REQ-INP-025), which carries faults on
//! purpose.
#![allow(missing_docs)]

use std::io::Write;
use std::process::{Command, Stdio};

/// Runs `ttx` with the authored stream on standard input, and returns its
/// standard output and standard error.
fn ttx_on_stream(arguments: &[&str]) -> (String, String) {
    let stream = neoteletext_formats::author::streams::stream().expect("the authored stream builds").t42();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ttx"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run ttx");
    child.stdin.take().expect("stdin").write_all(&stream).expect("write the stream");
    let output = child.wait_with_output().expect("ttx finishes");
    assert!(output.status.success(), "ttx failed: {}", String::from_utf8_lossy(&output.stderr));
    (String::from_utf8_lossy(&output.stdout).into_owned(), String::from_utf8_lossy(&output.stderr).into_owned())
}

/// The counts of the `enhancement faults:` line, by name.
fn fault_counts(listing: &str) -> Option<Vec<(String, u32)>> {
    let line = listing.lines().find_map(|line| line.strip_prefix("enhancement faults: "))?;
    Some(
        line.split("; ")
            .map(|item| {
                let (name, count) = item.rsplit_once(' ').expect("name and count");
                (name.to_owned(), count.parse().expect("a count"))
            })
            .collect(),
    )
}

/// REQ-L25-051: `ttx list` builds every page at Levels 2.5 and 3.5 and
/// shows each kind of fault met; at Level 1 there is nothing to count.
#[test]
fn list_shows_the_enhancement_faults() {
    let names = [
        "damaged triplets",
        "forbidden invocations",
        "absent objects",
        "limited invocations",
        "discarded writes",
        "ignored designations",
        "undefined characters",
        "DRCS missing",
        "DRCS invalid",
    ];
    for level in ["2.5", "3.5"] {
        let (listing, _) = ttx_on_stream(&["list", "-", "--level", level]);
        let counts = fault_counts(&listing).unwrap_or_else(|| panic!("no fault line at Level {level}:\n{listing}"));
        assert_eq!(counts.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), names);
        // The stream's DRCS pages carry invalid characters on purpose.
        assert!(counts.iter().any(|(name, count)| name == "DRCS invalid" && *count > 0), "{listing}");
    }
    let (listing, _) = ttx_on_stream(&["list", "-", "--level", "1"]);
    assert!(fault_counts(&listing).is_none(), "{listing}");
}

/// REQ-FSM-010, REQ-CLI-002: `-vv` prints the trace of the walk of the
/// level as it prints the other machines' traces; `-v` does not.
#[test]
fn verbosity_prints_the_walk_trace() {
    for (level, machine) in [("1.5", "machine: TripletWalk"), ("3.5", "machine: EnhancementWalk")] {
        let (_, trace) = ttx_on_stream(&["text", "-", "101", "--level", level, "-vv"]);
        let walk: Vec<&str> = trace.lines().filter(|line| line.contains(machine)).collect();
        assert!(!walk.is_empty(), "no {machine} entries at Level {level}:\n{trace}");
        assert!(walk.iter().all(|line| line.starts_with("ttx: Trace(TraceEntry {")), "{walk:#?}");
        let (_, quiet) = ttx_on_stream(&["text", "-", "101", "--level", level, "-v"]);
        assert!(!quiet.contains("TraceEntry"), "{quiet}");
    }
}
