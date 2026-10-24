//! REQ-GEN-014: teletext constants restated outside the core's constants
//! module.

use super::{report, source_files};
use crate::test_code::production_source;
use std::path::Path;

/// The core's source directory, which [`DATA_FILES`] is relative to.
const CORE_SRC: &str = "crates/neoteletext-core/src";
/// Files of the core that are the constants module itself or data tables
/// of code points or glyphs, where the literal values are characters, not
/// teletext constants. A path ending in `/` is a directory and covers
/// every file under it.
const DATA_FILES: &[&str] = &["consts.rs", "consts/", "charset/", "glyphs/bedstead.rs"];
/// Teletext values that must only appear in the constants module
/// (REQ-GEN-014).
const WATCHED_LITERALS: &[&str] = &["0x27", "0xE4", "0xe4", "6937500", "6_937_500", "0x55"];

/// Whether `path`, relative to the core's `src/`, is one of [`DATA_FILES`].
fn is_data_file(path: &str) -> bool {
    DATA_FILES.iter().any(|data| if data.ends_with('/') { path.starts_with(data) } else { path == *data })
}

/// REQ-GEN-014: literal teletext values outside the constants module.
pub fn constants(root: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    source_files(&root.join("crates"), &["rs"], &mut files);
    let core_src = root.join(CORE_SRC);
    let mut violations = Vec::new();
    for file in files {
        let display = file.to_string_lossy().to_string();
        let under_core = file.strip_prefix(&core_src).ok().map(|path| path.to_string_lossy().replace('\\', "/"));
        if under_core.is_some_and(|path| is_data_file(&path)) {
            continue;
        }
        for (number, literal) in watched_literals(&production_source(&file)) {
            violations.push(format!("{display}:{number}: literal {literal}"));
        }
    }
    report(&violations)
}

/// Each watched literal in a production source, with its line number.
fn watched_literals(source: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        for literal in WATCHED_LITERALS {
            if contains_literal(code, literal) {
                found.push((number + 1, *literal));
            }
        }
    }
    found
}

fn contains_literal(code: &str, literal: &str) -> bool {
    let mut rest = code;
    while let Some(index) = rest.find(literal) {
        let after = rest[index + literal.len()..].chars().next();
        let before = rest[..index].chars().last();
        let bounded = !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            && !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        if bounded {
            return true;
        }
        rest = &rest[index + literal.len()..];
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_code::production_text;

    /// A test-only item above production code does not hide the code from
    /// the check, as cutting the file at its first `#[cfg(test)]` did.
    #[test]
    fn a_file_with_an_early_test_item_is_still_scanned() {
        let source = "#[cfg(test)]\nuse super::*;\n\nconst PARITY: u8 = 0x27;\n\n#[cfg(test)]\nmod tests {\n    const X: u8 = 0x55;\n}\n";
        assert_eq!(watched_literals(&production_text(source)), vec![(4, "0x27")]);
    }

    /// REQ-GEN-014: the constants module and the character and glyph
    /// tables are exempt wherever their files live under `src/`; a module
    /// of code is not.
    #[test]
    fn data_files_are_matched_by_prefix() {
        for exempt in
            ["consts.rs", "consts/signal.rs", "charset/tables.rs", "charset/arabic_script.rs", "glyphs/bedstead.rs"]
        {
            assert!(is_data_file(exempt), "{exempt} is a data file");
        }
        for code in ["level1/cell.rs", "level1.rs", "charset.rs", "glyphs/g3.rs", "glyphs.rs", "ecc.rs"] {
            assert!(!is_data_file(code), "{code} is code");
        }
    }
}
