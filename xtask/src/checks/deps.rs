//! REQ-GEN-017: the permitted dependencies of each library crate, and the
//! workspace crates each application crate may use.

use super::{APPLICATION_CRATES, WORKSPACE_PREFIX, report};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// The permitted dependencies of each library crate (REQ-GEN-017,
/// decision E.38). `std`-only crates are not listed.
const PERMITTED: &[(&str, &[&str])] = &[
    ("neoteletext-core", &[]),
    ("neoteletext-formats", &["neoteletext-core"]),
    ("neoteletext-slicer", &["neoteletext-core"]),
    ("neoteletext-web", &["neoteletext-core", "embedded-io"]),
    ("neoteletext-image", &["neoteletext-core"]),
    ("neoteletext-audio", &["neoteletext-core"]),
    ("neoteletext-audio-mix", &[]),
    ("neoteletext-input", &["neoteletext-core", "neoteletext-audio"]),
];

/// REQ-GEN-017: compares each microcontroller crate's resolved normal
/// dependencies, with default features, against its permitted list.
pub fn dependencies(root: &Path) -> Result<(), String> {
    let mut violations = Vec::new();
    for (krate, permitted) in PERMITTED {
        let output = Command::new("cargo")
            .current_dir(root)
            .args(["tree", "-p", krate, "-e", "normal", "--prefix", "none", "--no-default-features"])
            .output()
            .map_err(|e| format!("cargo tree: {e}"))?;
        if !output.status.success() {
            return Err(format!("cargo tree -p {krate} failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let names: BTreeSet<String> = text
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .map(str::to_string)
            .filter(|name| name != krate)
            .collect();
        for name in names {
            if !permitted.contains(&name.as_str()) {
                violations.push(format!("{krate} depends on `{name}`, which is not permitted"));
            }
        }
    }
    for (krate, permitted) in APPLICATION_CRATES {
        let output = Command::new("cargo")
            .current_dir(root)
            .args(["tree", "-p", krate, "-e", "normal", "--prefix", "none", "--depth", "1", "--all-features"])
            .output()
            .map_err(|e| format!("cargo tree: {e}"))?;
        if !output.status.success() {
            return Err(format!("cargo tree -p {krate} failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let workspace_crates: BTreeSet<&str> = text
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .filter(|name| name.starts_with(WORKSPACE_PREFIX) && name != krate)
            .collect();
        for name in workspace_crates {
            if !permitted.contains(&name) {
                violations.push(format!("{krate} depends on `{name}`, which is not below it"));
            }
        }
    }
    report(&violations)
}
