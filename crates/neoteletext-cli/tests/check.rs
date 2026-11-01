//! `neottx check` (REQ-CLI-006), run as a user runs it.
#![cfg(feature = "live")]
#![allow(missing_docs)]

mod common;

use common::neottx;
use neoteletext_app::exit::CHECK_ERRORS;
use std::fs;
use std::path::Path;
use std::process::Output;

/// A page that links to the service `weather`, whose number and name it shows.
const PAGE: &str = "PN,10000\nOL,1,weather 400\nLN,R,weather:400,Weather\n";

/// A folder of this test's own, which holds `files`.
fn folder_of(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let folder = common::folder(name);
    for (file, text) in files {
        fs::write(folder.join(file), text).expect("write a test file");
    }
    folder
}

fn check_with(folder: &Path, descriptor: &str, page: &str) -> Output {
    let (descriptor, page) = (folder.join(descriptor), folder.join(page));
    neottx(&["check", "--descriptor", descriptor.to_str().unwrap(), page.to_str().unwrap()])
}

/// A channel's own file is read as one: every member is optional, and the
/// services it lists are the ones a link may name.
#[test]
fn a_channels_own_file_is_a_descriptor() {
    const LISTED: &str =
        r#"{"services":[{"name":"weather","display_name":"Weather","channel":"https://w.example/ttx/"}]}"#;
    let folder = folder_of("check-channel", &[("channel.ttx", LISTED), ("page.tti", PAGE)]);
    let listed = check_with(&folder, "channel.ttx", "page.tti");
    assert!(listed.status.success(), "{listed:?}");

    fs::write(folder.join("channel.ttx"), "{}").expect("write a test file");
    let unlisted = check_with(&folder, "channel.ttx", "page.tti");
    assert_eq!(unlisted.status.code(), Some(i32::from(CHECK_ERRORS)), "{unlisted:?}");
    assert!(String::from_utf8_lossy(&unlisted.stdout).contains("UnknownService"), "{unlisted:?}");
}

/// A descriptor of any other name is a service's, whose members are required.
#[test]
fn any_other_file_is_a_services_descriptor() {
    const SERVICE: &str = r#"{"name":"S","initial_page":"100","page_template":"/{mpp}"}"#;
    let files = [("service.json", SERVICE), ("short.json", "{}"), ("page.tti", "PN,10000\n")];
    let folder = folder_of("check-service", &files);
    assert!(check_with(&folder, "service.json", "page.tti").status.success());
    assert!(!check_with(&folder, "short.json", "page.tti").status.success());
}
