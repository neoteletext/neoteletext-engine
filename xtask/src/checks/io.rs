//! REQ-SEC-016: no operating-system access in the library crates, and every
//! crate classed as a library or an application crate.

use super::{APPLICATION_CRATES, LIBRARY_CRATES, report, source_files};
use crate::test_code::production_source;
use std::fs;
use std::path::Path;

const FORBIDDEN: &[&str] =
    &["std::fs", "std::net", "std::process", "std::env", "std::thread", "SystemTime", "Instant::now", "std::time"];

/// REQ-SEC-016: no operating-system access in library crates.
pub fn io(root: &Path) -> Result<(), String> {
    let mut violations = Vec::new();
    // A new crate must be placed in one class or the other, so that none
    // escapes this check unnoticed.
    let entries = fs::read_dir(root.join("crates")).map_err(|e| format!("crates: {e}"))?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let library = LIBRARY_CRATES.contains(&name.as_str());
        let application = APPLICATION_CRATES.iter().any(|(krate, _)| *krate == name);
        if entry.path().is_dir() && !library && !application {
            violations.push(format!("crates/{name} is neither a library crate nor an application crate"));
        }
    }
    for krate in LIBRARY_CRATES {
        forbid(&root.join("crates").join(krate).join("src"), FORBIDDEN, &mut violations);
    }
    report(&violations)
}

/// Every use of a forbidden item in the non-test Rust files under `source`.
fn forbid(source: &Path, items: &[&str], violations: &mut Vec<String>) {
    let mut files = Vec::new();
    source_files(source, &["rs"], &mut files);
    for file in files {
        for (number, item) in forbidden_items(&production_source(&file), items) {
            violations.push(format!("{}:{number}: {item}", file.display()));
        }
    }
}

/// Each forbidden item a production source uses, with its line number.
fn forbidden_items<'a>(source: &str, items: &[&'a str]) -> Vec<(usize, &'a str)> {
    let mut found = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        for item in items {
            if code.contains(item) {
                found.push((number + 1, *item));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_code::production_text;

    /// A test-only item above production code does not hide the code from
    /// the check, as cutting the file at its first `#[cfg(test)]` did.
    #[test]
    fn a_file_with_an_early_test_item_is_still_scanned() {
        let source = "#[cfg(test)]\nuse std::sync::Arc;\n\npub fn run() { std::thread::spawn(|| ()); }\n\n#[cfg(test)]\nmod tests {\n    fn t() { std::fs::read(\"x\"); }\n}\n";
        assert_eq!(forbidden_items(&production_text(source), FORBIDDEN), vec![(4, "std::thread")]);
    }
}
