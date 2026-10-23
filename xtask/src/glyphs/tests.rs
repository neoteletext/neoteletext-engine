//! The stored glyph data against its generators, and the provenance check.

use super::bedstead::recorded_checksum;
use super::*;

fn stored(path: &str) -> String {
    fs::read_to_string(crate::workspace_root().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// REQ-RND-037: the generator is deterministic and the stored G3 data
/// matches its output; run `cargo xtask glyphs` after changing it.
#[test]
fn stored_g3_glyphs_are_up_to_date() {
    assert_eq!(g3::text(), g3::text(), "the generator is deterministic");
    assert_eq!(stored(G3_TEXT), g3::text(), "{G3_TEXT} is stale: run cargo xtask glyphs");
    assert_eq!(stored(G3_TARGET), g3_source(&stored(G3_TEXT)).unwrap(), "{G3_TARGET} is stale");
}

/// REQ-RND-020: the stored Bedstead table matches its source.
#[test]
fn stored_bedstead_glyphs_are_up_to_date() {
    assert_eq!(stored(TARGET), bedstead_source(&stored(SOURCE)).unwrap().0, "{TARGET} is stale");
}

/// The required characters are generated from the character set
/// tables, never chosen by hand.
#[test]
fn stored_required_characters_are_up_to_date() {
    assert_eq!(stored(REQUIRED), required_text(), "{REQUIRED} is stale: run cargo xtask glyphs");
    assert_eq!(stored(FORMS), forms_text(), "{FORMS} is stale: run cargo xtask glyphs");
}

/// REQ-FNT-001: the export is the one its provenance record names.
#[test]
fn bedstead_export_matches_its_recorded_checksum() {
    check_bedstead(&crate::workspace_root()).unwrap();
    assert_eq!(recorded_checksum("x\n**Export SHA-256:** `ab12`, from the command below.\n"), Some("ab12"));
    assert_eq!(recorded_checksum("no checksum here"), None);
    assert_eq!(recorded_checksum("**Export SHA-256:** `a`\n**Export SHA-256:** `b`\n"), None, "two lines");
}

#[test]
fn g3_text_is_checked() {
    assert!(g3_source("2/0 10 000").is_err(), "too few lines");
    assert!(g3_source("8/0 10 0 0 0 0 0 0 0 0 0 0").is_err(), "code above 7/F");
    assert!(g3_source("2/0 12 0 0 0 0 0 0 0 0 0 0 0 0").is_err(), "unknown height");
    assert!(g3_source("2/0 10 1000 0 0 0 0 0 0 0 0 0").is_err(), "wider than 12 bits");
    assert!(g3_source("").is_err(), "positions missing");
}
