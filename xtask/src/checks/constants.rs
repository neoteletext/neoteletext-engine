//! REQ-GEN-014: teletext constants restated outside the core's constants
//! module.

use super::{non_test_source, report, rust_files};
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
    rust_files(&root.join("crates"), &mut files);
    let core_src = root.join(CORE_SRC);
    let mut violations = Vec::new();
    for file in files {
        let display = file.to_string_lossy().to_string();
        let under_core = file.strip_prefix(&core_src).ok().map(|path| path.to_string_lossy().replace('\\', "/"));
        if under_core.is_some_and(|path| is_data_file(&path)) {
            continue;
        }
        let source = non_test_source(&file);
        for (number, line) in source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            for literal in WATCHED_LITERALS {
                if contains_literal(code, literal) {
                    violations.push(format!("{display}:{}: literal {literal}", number + 1));
                }
            }
        }
    }
    report(&violations)
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
