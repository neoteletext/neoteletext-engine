//! REQ-ARCH-011: the core's module dependency table (ARCHITECTURE.md
//! section 9), enforced on every non-test source file of the core.

use super::{report, rust_files};
use crate::test_code::production_source;
use std::collections::BTreeSet;
use std::path::Path;

/// The allowed-dependency table of ARCHITECTURE.md section 9. Foundation
/// modules and the cross-cutting modules are allowed everywhere listed.
const FOUNDATION: &[&str] = &["consts", "ecc", "types"];
const CROSS_CUTTING: &[&str] = &["policy", "config", "trace"];
const LAYER_TABLE: &[(&str, &[&str])] = &[
    ("consts", &[]),
    ("ecc", &[]),
    ("types", &[]),
    ("packet", &[]),
    ("fsm", &["packet"]),
    ("acq", &["packet", "fsm"]),
    ("service", &["packet"]),
    ("charset", &[]),
    ("level1", &["charset", "acq", "colour"]),
    ("level15", &["charset", "level1", "packet"]),
    ("colour", &[]),
    ("objects", &["packet", "acq"]),
    ("enhance", &["charset", "level1", "packet", "acq", "colour", "objects"]),
    // REQ-CHR-015: letters are joined after the level's pass.
    ("joining", &["charset", "level1"]),
    ("compose", &["acq", "level1", "level15", "enhance", "joining"]),
    ("glyphs", &["charset"]),
    ("render", &["glyphs", "level1", "colour"]),
    ("viewer", &["acq", "level1", "compose", "colour", "render", "service", "nav", "forms", "charset"]),
    ("nav", &["acq", "level1", "level15", "enhance", "colour", "render", "service", "forms", "charset"]),
    ("forms", &["acq", "level1", "level15", "enhance", "colour", "render", "service", "charset"]),
    ("port", &[]),
    ("policy", &[]),
    ("config", &[]),
    ("trace", &[]),
];

/// The core module a file belongs to: its first path component under src.
fn module_of(src: &Path, file: &Path) -> Option<String> {
    let relative = file.strip_prefix(src).ok()?;
    let first = relative.components().next()?.as_os_str().to_string_lossy().to_string();
    Some(first.trim_end_matches(".rs").to_string())
}

/// REQ-ARCH-011: each core module may use only the modules listed for it.
pub fn layers(root: &Path) -> Result<(), String> {
    let src = root.join("crates/neoteletext-core/src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let mut violations = Vec::new();
    for file in files {
        let Some(module) = module_of(&src, &file) else { continue };
        if module == "lib" {
            continue;
        }
        let Some((_, allowed)) = LAYER_TABLE.iter().find(|(name, _)| *name == module) else {
            violations.push(format!("{}: module `{module}` is not in the layer table", file.display()));
            continue;
        };
        let source = production_source(&file);
        for used in uses(&source) {
            let ok = used == module
                || FOUNDATION.contains(&used.as_str())
                || CROSS_CUTTING.contains(&used.as_str())
                || allowed.contains(&used.as_str());
            let cross_cutting_module = CROSS_CUTTING.contains(&module.as_str());
            let ok = if cross_cutting_module { used == module || FOUNDATION.contains(&used.as_str()) } else { ok };
            if !ok {
                violations.push(format!("{}: `{module}` may not use `{used}`", file.display()));
            }
        }
    }
    report(&violations)
}

/// Every `crate::<module>` named in a source text.
fn uses(source: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = source;
    while let Some(index) = rest.find("crate::") {
        rest = &rest[index + "crate::".len()..];
        let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if !name.is_empty() {
            found.insert(name);
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
        let source = "#[cfg(test)]\nuse crate::render::Grid;\n\nuse crate::viewer::Viewer;\n\n#[cfg(test)]\nmod tests {\n    use crate::forms::FormModel;\n}\n";
        assert_eq!(uses(&production_text(source)), BTreeSet::from(["viewer".to_string()]));
    }
}
