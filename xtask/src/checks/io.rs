//! REQ-SEC-016: no operating-system access in the library crates, and every
//! crate classed as a library or an application crate.

use super::paths::{Named, named_paths, names_item};
use super::{APPLICATION_CRATES, LIBRARY_CRATES, report, source_files};
use crate::test_code::production_source;
use std::fs;
use std::path::Path;

const FORBIDDEN: &[&str] = &[
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "std::thread",
    "SystemTime",
    "Instant::now",
    "std::time",
    // A glob brings every item above into scope under its bare name.
    "std::*",
];

/// `std` under another name, which hides every path through it from this
/// check; the plain `extern crate std;` of a `std` feature is no such name.
const RENAMED_STD: &str = "std under another name";

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

/// Each forbidden item a production source uses, however it is written, with
/// its line number.
fn forbidden_items<'a>(source: &str, items: &[&'a str]) -> Vec<(usize, &'a str)> {
    let mut found = Vec::new();
    for Named { line, path, renamed } in named_paths(source) {
        if renamed && path == "std" {
            found.push((line, RENAMED_STD));
        }
        for item in items {
            if names_item(&path, item) {
                found.push((line, *item));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_code::production_text;

    fn found(source: &str) -> Vec<(usize, &'static str)> {
        forbidden_items(source, FORBIDDEN)
    }

    /// A test-only item above production code does not hide the code from
    /// the check, as cutting the file at its first `#[cfg(test)]` did.
    #[test]
    fn a_file_with_an_early_test_item_is_still_scanned() {
        let source = "#[cfg(test)]\nuse std::sync::Arc;\n\npub fn run() { std::thread::spawn(|| ()); }\n\n#[cfg(test)]\nmod tests {\n    fn t() { std::fs::read(\"x\"); }\n}\n";
        assert_eq!(forbidden_items(&production_text(source), FORBIDDEN), vec![(4, "std::thread")]);
    }

    /// TODO-2094: a grouped import holds no text `std::fs`, and the check that
    /// looked for that text printed ok.
    #[test]
    fn a_grouped_import_is_found() {
        assert_eq!(found("use std::{fs, io};"), [(1, "std::fs")]);
    }

    #[test]
    fn a_nested_import_is_found() {
        assert_eq!(
            found("use std::{\n    io::{self, Write},\n    net::TcpStream,\n    time::{Duration, SystemTime},\n};"),
            [(3, "std::net"), (4, "std::time"), (4, "SystemTime"), (4, "std::time")]
        );
    }

    #[test]
    fn a_rename_written_across_lines_is_found() {
        assert_eq!(found("use std::thread\n    as worker;"), [(2, "std::thread")]);
    }

    #[test]
    fn a_path_written_with_spaces_is_found() {
        assert_eq!(found("fn f() { std :: fs :: read(\"x\"); }"), [(1, "std::fs")]);
    }

    #[test]
    fn a_glob_of_std_is_found() {
        assert_eq!(found("use std::*;"), [(1, "std::*")]);
    }

    #[test]
    fn std_under_another_name_is_found() {
        assert_eq!(found("extern crate std as host;"), [(1, RENAMED_STD)]);
        assert_eq!(found("use std as host;"), [(1, RENAMED_STD)]);
        assert_eq!(found("use std::{self as host};"), [(1, RENAMED_STD)]);
    }

    /// What the library crates may write: `std` for its collections, the
    /// `extern crate` of a `std` feature, and the core's own `Instant`.
    #[test]
    fn what_is_allowed_is_not_found() {
        let source = "extern crate std;\nuse std::{format, vec};\nuse std::io::Write as _;\nuse neoteletext_core::types::Instant;\n// use std::fs;\nconst A: &str = \"std::fs\";\n";
        assert_eq!(found(source), []);
    }
}
