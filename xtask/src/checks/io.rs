//! REQ-SEC-016: no operating-system access in the library crates, and every
//! crate classed as a library or an application crate.

use super::{APPLICATION_CRATES, LIBRARY_CRATES, non_test_source, report, rust_files};
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
    rust_files(source, &mut files);
    for file in files {
        let code_of_file = non_test_source(&file);
        for (number, line) in code_of_file.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            for item in items {
                if code.contains(item) {
                    violations.push(format!("{}:{}: {item}", file.display(), number + 1));
                }
            }
        }
    }
}
