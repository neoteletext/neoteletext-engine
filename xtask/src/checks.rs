//! Source checks that enforce the architecture and the project rules.

mod constants;
mod deps;
mod io;
mod layers;

use std::fs;
use std::path::{Path, PathBuf};

pub use constants::constants;
pub use deps::dependencies;
pub use io::io;
pub use layers::layers;

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// The source of a file with its `#[cfg(test)]` tail removed.
fn non_test_source(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_default();
    // Test modules may also need a feature (decision E.61):
    // `#[cfg(all(test, feature = ...))]`.
    let start = ["#[cfg(test)]", "#[cfg(all(test,"].iter().filter_map(|marker| text.find(marker)).min();
    match start {
        Some(index) => text[..index].to_string(),
        None => text,
    }
}

/// The prefix of every crate of the workspace.
const WORKSPACE_PREFIX: &str = "neoteletext-";

/// The application crates of layer L9: `std`, free to use outside crates,
/// and with `neoteletext-ffi` the only crates that may touch the operating
/// system (REQ-SEC-016). Each may use only the workspace crates listed for
/// it, which keeps the shared application layer below the programs.
const APPLICATION_CRATES: &[(&str, &[&str])] = &[
    // A phone as a keyboard through qrkeyboard's relay (decision E.93):
    // sockets, TLS and cryptography, for desktop programs only.
    ("neoteletext-qrkeyboard", &["neoteletext-web"]),
    (
        "neoteletext-app",
        &["neoteletext-core", "neoteletext-formats", "neoteletext-slicer", "neoteletext-web", "neoteletext-image"],
    ),
    (
        "neoteletext-cli",
        &[
            "neoteletext-app",
            "neoteletext-audio",
            "neoteletext-input",
            "neoteletext-qrkeyboard",
            "neoteletext-core",
            "neoteletext-formats",
            "neoteletext-slicer",
            "neoteletext-web",
            "neoteletext-image",
        ],
    ),
];

/// Library crates that must not touch the operating system (REQ-SEC-016).
const LIBRARY_CRATES: &[&str] = &[
    "neoteletext-core",
    "neoteletext-formats",
    "neoteletext-slicer",
    "neoteletext-web",
    "neoteletext-image",
    "neoteletext-audio",
    "neoteletext-audio-mix",
    "neoteletext-input",
];

fn report(violations: &[String]) -> Result<(), String> {
    if violations.is_empty() {
        println!("ok");
        Ok(())
    } else {
        for violation in violations {
            eprintln!("{violation}");
        }
        Err(format!("{} violation(s)", violations.len()))
    }
}
