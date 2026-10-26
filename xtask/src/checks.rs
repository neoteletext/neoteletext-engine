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

/// Every file under `dir` whose extension is one of `extensions`, in the
/// order the directories list them; the source checks and the
/// traceability table share it.
pub(crate) fn source_files(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            source_files(&path, extensions, out);
        } else if path.extension().is_some_and(|ext| extensions.iter().any(|wanted| ext == *wanted)) {
            out.push(path);
        }
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
    "neoteletext-opus",
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
