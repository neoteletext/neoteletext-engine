//! A build that leaves an area out (decision E.61) refuses it plainly, with
//! the usage exit status (REQ-CLI-005), rather than failing some other way.
//! Each test runs only in a build without its feature.
#![allow(missing_docs)]

use std::process::Command;

/// REQ-CLI-005: invalid usage.
#[cfg(any(not(feature = "live"), not(feature = "view"), not(feature = "slice")))]
const USAGE: i32 = 2;

/// Runs ttx; the exit status and standard error.
#[cfg(any(not(feature = "live"), not(feature = "view"), not(feature = "slice")))]
fn ttx(arguments: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ttx")).args(arguments).output().expect("run ttx");
    (output.status.code(), String::from_utf8_lossy(&output.stderr).into_owned())
}

#[cfg(any(not(feature = "live"), not(feature = "view"), not(feature = "slice")))]
fn assert_refused(arguments: &[&str], feature: &str) {
    let (status, errors) = ttx(arguments);
    assert_eq!(status, Some(USAGE), "{arguments:?}: {errors}");
    assert!(errors.contains("not in this build"), "{arguments:?}: {errors}");
    assert!(errors.contains(&format!("`{feature}`")), "{arguments:?} names the feature: {errors}");
}

#[cfg(not(feature = "live"))]
#[test]
fn live_inputs_are_refused_without_live() {
    for input in ["zxnet:teefax", "wss://example.org/feed", "http://example.org/s.json", "https://example.org/s.json"] {
        assert_refused(&["text", input, "100", "--duration", "1"], "live");
        assert_refused(&["list", input, "--duration", "1"], "live");
    }
}

#[cfg(not(feature = "live"))]
#[test]
fn the_fetcher_is_refused_without_live() {
    assert_refused(&["fetch", "zxnet:teefax"], "live");
}

#[cfg(not(feature = "live"))]
#[test]
fn descriptors_are_refused_without_live() {
    assert_refused(&["check", "--descriptor", "service.json", "page.tti"], "live");
}

#[cfg(not(feature = "view"))]
#[test]
fn the_viewer_is_refused_without_view() {
    assert_refused(&["view", "missing.t42"], "view");
}

#[cfg(not(feature = "slice"))]
#[test]
fn slicing_is_refused_without_slice() {
    let output = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("not-in-build-slice.t42");
    assert_refused(&["slice", "missing.raw", "-o", &output.to_string_lossy()], "slice");
    assert!(!output.exists(), "nothing is written");
}

/// The commands stay defined in every build, so the help text is the same.
#[test]
fn every_command_is_in_the_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_ttx")).arg("--help").output().expect("run ttx");
    let help = String::from_utf8_lossy(&output.stdout);
    for command in ["list", "show", "view", "render", "text", "convert", "slice", "services", "service-info", "check"] {
        assert!(help.lines().any(|line| line.trim_start().starts_with(command)), "{command}: {help}");
    }
}
