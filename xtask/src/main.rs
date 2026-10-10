//! Project automation (REQ-GEN-010): `cargo xtask <command>`.
//!
//! - `glyphs`: regenerate the core's glyph tables: Bedstead from its export,
//!   and the G3 set from the project's own geometry (REQ-RND-037).
//! - `corpus`: fetch and pin the real-broadcast test corpus (REQ-TST-022).
//! - `traceability`: write docs/traceability.md (REQ-DOC-003).
//! - `check-layers`: enforce the module dependency table of ARCHITECTURE.md
//!   (REQ-ARCH-011).
//! - `check-deps`: enforce the permitted dependency lists (REQ-GEN-017),
//!   and that each application crate uses only the workspace crates below
//!   it.
//! - `check-constants`: find teletext constants restated outside the
//!   constants module (REQ-GEN-014).
//! - `check-io`: find operating-system access in library crates, and
//!   check that every crate is classed as a library or an application
//!   crate (REQ-SEC-016).
//! - `level25-stream OUT.t42 [--tti DIR]`: write the authored Level 2.5 and
//!   3.5 test stream (REQ-TST-024).
//! - `glyph-inventory`: write the characters the original glyph set must
//!   draw (decision E.62).
//! - `glyph-check [--complete]`: check the original glyph set (decision
//!   E.62).
//! - `glyph-proof OUT.png [TEXT...]`: draw the original glyphs for review.
//! - `features`: lint and test the fixed list of feature builds of decision
//!   E.61.
//! - `size [--target TRIPLE]`: link the microcontroller test image for each
//!   build of decision E.61 and check its size and symbols (REQ-GEN-009,
//!   REQ-GEN-019).

mod checks;
mod corpus;
mod features;
mod g3;
mod glyphs;
mod level25;
mod original;
mod size;
mod trace;

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
        "traceability" => trace::generate(&root),
        "check-layers" => checks::layers(&root),
        "check-deps" => checks::dependencies(&root),
        "check-constants" => checks::constants(&root),
        "check-io" => checks::io(&root),
        "level25-stream" => level25::write(&std::env::args().skip(2).collect::<Vec<_>>()),
        "glyph-inventory" => original::inventory(&root),
        "glyph-check" => original::check(&root, &std::env::args().skip(2).collect::<Vec<_>>()),
        "glyph-proof" => original::proof(&root, &std::env::args().skip(2).collect::<Vec<_>>()),
        "features" => features::run(&root),
        "size" => size::run(&root, &std::env::args().skip(2).collect::<Vec<_>>()),
        _ => Err(format!(
            "usage: cargo xtask <glyphs|corpus|traceability|check-layers|check-deps|check-constants|check-io|\
             level25-stream|glyph-inventory|glyph-check|glyph-proof|features|size> (got {command:?})"
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
