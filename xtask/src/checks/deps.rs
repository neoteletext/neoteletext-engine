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
    // The channel's records share the audio profile's frame, and a LOG
    // record the TTI reader's rules for an `LG` line (DOC-1956).
    ("neoteletext-web", &["neoteletext-core", "embedded-io", "neoteletext-audio", "neoteletext-formats"]),
    ("neoteletext-image", &["neoteletext-core"]),
    // Decision E.124 item 5: the Opus decoder, behind the `opus` feature.
    ("neoteletext-audio", &["neoteletext-core", "neoteletext-opus"]),
    ("neoteletext-audio-mix", &[]),
    // Decision E.123: an integer Opus decoder written here, with no dependency.
    ("neoteletext-opus", &[]),
    ("neoteletext-input", &["neoteletext-core", "neoteletext-audio"]),
];

/// `cargo tree`'s output for `krate`: its resolved normal dependencies with
/// every feature on, so that one behind a feature is seen.
fn resolved_tree(root: &Path, krate: &str) -> Result<String, String> {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["tree", "-p", krate, "-e", "normal", "--prefix", "none", "--all-features"])
        .output()
        .map_err(|e| format!("cargo tree: {e}"))?;
    if !output.status.success() {
        return Err(format!("cargo tree -p {krate} failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Each difference between a crate's resolved dependencies, as `tree` lists
/// them, and its permitted list (REQ-GEN-017: "shall fail on any difference").
fn differences(krate: &str, tree: &str, permitted: &[&str]) -> Vec<String> {
    let names: BTreeSet<&str> =
        tree.lines().filter_map(|line| line.split_whitespace().next()).filter(|name| *name != krate).collect();
    let mut found = Vec::new();
    for name in &names {
        if !permitted.contains(name) {
            found.push(format!("{krate} depends on `{name}`, which is not permitted"));
        }
    }
    for name in permitted {
        if !names.contains(name) {
            found.push(format!("{krate} is permitted `{name}`, on which it no longer depends"));
        }
    }
    found
}

/// REQ-GEN-017: compares each microcontroller crate's resolved normal
/// dependencies, with every feature on, against its permitted list.
pub fn dependencies(root: &Path) -> Result<(), String> {
    let mut violations = Vec::new();
    for (krate, permitted) in PERMITTED {
        violations.extend(differences(krate, &resolved_tree(root, krate)?, permitted));
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

#[cfg(test)]
mod tests {
    use super::*;

    const TREE: &str = "neoteletext-audio v0.1.0 (/x)\nneoteletext-core v0.1.0 (/y)\nneoteletext-opus v0.1.0 (/z)\n";

    #[test]
    fn a_dependency_not_on_the_list_is_a_difference() {
        assert_eq!(
            differences("neoteletext-audio", TREE, &["neoteletext-core"]),
            ["neoteletext-audio depends on `neoteletext-opus`, which is not permitted"]
        );
    }

    #[test]
    fn a_listed_crate_no_longer_depended_on_is_a_difference() {
        assert_eq!(
            differences("neoteletext-audio", TREE, &["neoteletext-core", "neoteletext-opus", "embedded-io"]),
            ["neoteletext-audio is permitted `embedded-io`, on which it no longer depends"]
        );
    }

    #[test]
    fn a_tree_that_matches_its_list_has_no_difference() {
        assert_eq!(differences("neoteletext-audio", TREE, &["neoteletext-core", "neoteletext-opus"]), [""; 0]);
    }

    /// TODO-2101: `neoteletext-opus` is behind `neoteletext-audio`'s `opus`
    /// feature, and read with no default features the check never saw it.
    #[test]
    fn a_dependency_behind_a_feature_is_read() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert!(resolved_tree(root, "neoteletext-audio").unwrap().contains("neoteletext-opus"));
    }
}
