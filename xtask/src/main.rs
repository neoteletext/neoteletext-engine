//! Project automation (REQ-GEN-010): `cargo xtask <command>`.
//!
//! - `glyphs`: regenerate the core's glyph table from the Bedstead export.
//! - `corpus`: fetch and pin the real-broadcast test corpus (REQ-TST-022).
//! - `check-layers`: enforce the module dependency table of ARCHITECTURE.md
//!   (REQ-ARCH-011).
//! - `check-deps`: enforce the permitted dependency lists (REQ-GEN-017).
//! - `check-constants`: find teletext constants restated outside the
//!   constants module (REQ-GEN-014).
//! - `check-io`: find operating-system access in library crates
//!   (REQ-SEC-016).

mod checks;
mod corpus;
mod glyphs;

use std::path::PathBuf;
use std::process::ExitCode;

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map_or(manifest.clone(), PathBuf::from)
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();
    let root = workspace_root();
    let result = match command.as_str() {
        "glyphs" => glyphs::generate(&root),
        "corpus" => corpus::fetch(&root),
        "check-layers" => checks::layers(&root),
        "check-deps" => checks::dependencies(&root),
        "check-constants" => checks::constants(&root),
        "check-io" => checks::io(&root),
        _ => Err(format!(
            "usage: cargo xtask <glyphs|corpus|check-layers|check-deps|check-constants|check-io> (got {command:?})"
        )),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask {command}: {message}");
            ExitCode::FAILURE
        }
    }
}
