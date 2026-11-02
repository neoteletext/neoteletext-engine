//! REQ-ARCH-011: the core's module dependency table and what each library
//! crate may use of the core (ARCHITECTURE.md section 9), enforced on every
//! non-test source file.

use super::paths::{Named, named_paths};
use super::{LIBRARY_CRATES, report, source_files};
use crate::test_code::production_source;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// The allowed-dependency table of ARCHITECTURE.md section 9. Foundation
/// modules and the cross-cutting modules are allowed everywhere listed.
const FOUNDATION: &[&str] = &["consts", "ecc", "limits", "types"];
const CROSS_CUTTING: &[&str] = &["policy", "config", "trace"];
const LAYER_TABLE: &[(&str, &[&str])] = &[
    ("consts", &[]),
    ("ecc", &[]),
    // REQ-NUM-031: the numbers of area NUM, with no dependency at all.
    ("limits", &[]),
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
    ("viewer", &["acq", "level1", "compose", "colour", "render", "service", "nav", "charset"]),
    ("nav", &["acq", "level1", "level15", "enhance", "colour", "render", "service", "charset"]),
    // Decision E.127: a page's controls and the looks drawn on them.
    ("controls", &["acq", "level1", "objects", "packet"]),
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

/// REQ-ARCH-011: each core module may use only the modules listed for it, and
/// each library crate only what ARCHITECTURE.md section 9 lists for it.
pub fn layers(root: &Path) -> Result<(), String> {
    let mut violations = module_violations(root);
    let architecture = fs::read_to_string(root.join(ARCHITECTURE)).map_err(|e| format!("{ARCHITECTURE}: {e}"))?;
    let rows = crate_rows(&architecture)?;
    for krate in LIBRARY_CRATES.iter().filter(|krate| **krate != CORE) {
        let Some((_, allowed)) = rows.iter().find(|(name, _)| name == krate) else {
            violations.push(format!("{krate} has no row in section 9 of {ARCHITECTURE}"));
            continue;
        };
        let mut files = Vec::new();
        source_files(&root.join("crates").join(krate).join("src"), &["rs"], &mut files);
        for file in files {
            for fault in crate_violations(krate, &production_source(&file), allowed) {
                violations.push(format!("{}: {fault}", file.display()));
            }
        }
    }
    report(&violations)
}

/// The modules of the core's table, for one file of module `module`.
fn module_faults(module: &str, allowed: &[&str], source: &str) -> Vec<String> {
    let mut faults = Vec::new();
    let cross_cutting_module = CROSS_CUTTING.contains(&module);
    for used in uses(source) {
        let foundation = used == module || FOUNDATION.contains(&used.as_str());
        let ok = if cross_cutting_module {
            foundation
        } else {
            foundation || CROSS_CUTTING.contains(&used.as_str()) || allowed.contains(&used.as_str())
        };
        if !ok {
            faults.push(format!("`{module}` may not use `{used}`"));
        }
    }
    faults
}

fn module_violations(root: &Path) -> Vec<String> {
    let src = root.join("crates/neoteletext-core/src");
    let mut files = Vec::new();
    source_files(&src, &["rs"], &mut files);
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
        for fault in module_faults(&module, allowed, &production_source(&file)) {
            violations.push(format!("{}: {fault}", file.display()));
        }
    }
    violations
}

/// Every module of this crate a source names through `crate::`, however it
/// is written.
fn uses(source: &str) -> BTreeSet<String> {
    named_paths(source)
        .into_iter()
        .filter_map(|Named { path, .. }| path.strip_prefix("crate::")?.split("::").next().map(str::to_string))
        .collect()
}

/// A crate's name for the core in Rust paths.
const CORE_PATH: &str = "neoteletext_core";
const CORE: &str = "neoteletext-core";
const ARCHITECTURE: &str = "ARCHITECTURE.md";
/// The heading row of section 9's table of crates.
const CRATE_TABLE_HEADING: &str = "| Crate | May use of `neoteletext-core` |";

/// The rows of section 9's table of crates: each crate and the modules of the
/// core it may use. A cell lists `Foundation` or module names, each with any
/// reason in brackets, or a dash for none.
fn crate_rows(architecture: &str) -> Result<Vec<(String, BTreeSet<String>)>, String> {
    let mut lines = architecture.lines().skip_while(|line| !line.starts_with(CRATE_TABLE_HEADING));
    lines.next().ok_or_else(|| format!("{ARCHITECTURE} has no table headed {CRATE_TABLE_HEADING}"))?;
    let mut rows = Vec::new();
    for line in lines.skip(1).take_while(|line| line.starts_with('|')) {
        let mut cells = line.split('|').skip(1).map(str::trim);
        let (Some(krate), Some(modules)) = (cells.next(), cells.next()) else {
            return Err(format!("{ARCHITECTURE}: a row of the table of crates has no cell of modules: {line}"));
        };
        rows.push((krate.trim_matches('`').to_string(), listed_modules(modules)));
    }
    if rows.is_empty() {
        return Err(format!("{ARCHITECTURE}: the table of crates has no rows"));
    }
    Ok(rows)
}

/// The modules a cell lists, its reasons in brackets left out.
fn listed_modules(cell: &str) -> BTreeSet<String> {
    let mut depth = 0usize;
    let plain: String = cell
        .chars()
        .filter(|character| {
            match character {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ => (),
            }
            depth == 0 && *character != ')'
        })
        .collect();
    let mut modules = BTreeSet::new();
    for item in plain.split(',').map(|item| item.trim().trim_matches('`')).filter(|item| !item.is_empty()) {
        if item == "Foundation" {
            modules.extend(FOUNDATION.iter().map(ToString::to_string));
        } else if item != "—" {
            modules.insert(item.to_string());
        }
    }
    modules
}

/// What a source of crate `krate` uses of the core outside `allowed`. Another
/// name for the core would hide its paths from this check, so it is one.
fn crate_violations(krate: &str, source: &str, allowed: &BTreeSet<String>) -> Vec<String> {
    let mut faults = Vec::new();
    for Named { path, renamed, .. } in named_paths(source) {
        if renamed && path == CORE_PATH {
            faults.push(format!("{krate} renames `{CORE_PATH}`, which hides what it uses"));
        }
        let Some(used) = path.strip_prefix(CORE_PATH).and_then(|rest| rest.strip_prefix("::")) else { continue };
        let module = used.split("::").next().unwrap_or_default();
        if !allowed.contains(module) {
            faults.push(format!("{krate} may not use `{module}` of the core"));
        }
    }
    faults
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_code::production_text;

    /// A test-only item above production code does not hide the code from
    /// the check, as cutting the file at its first `#[cfg(test)]` did.
    #[test]
    fn a_file_with_an_early_test_item_is_still_scanned() {
        let source = "#[cfg(test)]\nuse crate::render::Grid;\n\nuse crate::viewer::Viewer;\n\n#[cfg(test)]\nmod tests {\n    use crate::nav::History;\n}\n";
        assert_eq!(uses(&production_text(source)), BTreeSet::from(["viewer".to_string()]));
    }

    /// REQ-ARCH-011's verification: a forbidden dependency is detected.
    #[test]
    fn a_module_using_what_its_row_does_not_list_is_found() {
        let faults = module_faults("packet", &[], "use crate::render::Grid;\n");
        assert_eq!(faults, ["`packet` may not use `render`"]);
    }

    #[test]
    fn a_grouped_use_of_a_forbidden_module_is_found() {
        let faults = module_faults("packet", &[], "use crate::{consts::PAGE_WIDTH, render::Grid};\n");
        assert_eq!(faults, ["`packet` may not use `render`"]);
    }

    #[test]
    fn foundation_and_the_cross_cutting_modules_are_allowed() {
        assert_eq!(module_faults("packet", &[], "use crate::{consts::A, trace::B, types::C};\n"), [""; 0]);
    }

    #[test]
    fn a_cross_cutting_module_uses_only_foundation() {
        let faults = module_faults("policy", &["config"], "use crate::config::Config;\n");
        assert_eq!(faults, ["`policy` may not use `config`"]);
    }

    const TABLE: &str = "text\n\n| Crate | May use of `neoteletext-core` |\n|---|---|\n| `a` | Foundation, `packet` (the frame, and `Packet`) |\n| `b` | — |\n\nmore | text\n";

    #[test]
    fn the_table_of_crates_is_read() {
        let rows = crate_rows(TABLE).unwrap();
        assert_eq!(rows[0].0, "a");
        assert_eq!(
            rows[0].1,
            ["consts", "ecc", "limits", "types", "packet"].map(str::to_string).into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(rows[1], ("b".to_string(), BTreeSet::new()));
    }

    /// The rule REQ-ARCH-011 states of the web crate: `packet` and no layer
    /// above it, however the use is written.
    #[test]
    fn a_crate_using_what_its_row_does_not_list_is_found() {
        let rows = crate_rows(ARCHITECTURE_TABLE_OF_WEB).unwrap();
        let (krate, allowed) = &rows[0];
        for source in [
            "use neoteletext_core::acq::Decoder;\n",
            "use neoteletext_core::{packet::Packet, acq::Decoder};\n",
            "fn f() { neoteletext_core::acq::Decoder::new(); }\n",
            "use neoteletext_core::*;\n",
        ] {
            assert_eq!(crate_violations(krate, source, allowed).len(), 1, "{source}");
        }
        assert_eq!(
            crate_violations(krate, "use neoteletext_core::{packet::Packet, types::Instant};\n", allowed),
            [""; 0]
        );
    }

    const ARCHITECTURE_TABLE_OF_WEB: &str = "| Crate | May use of `neoteletext-core` |\n|---|---|\n| `neoteletext-web` | Foundation, `policy`, `config`, `packet` |\n";

    #[test]
    fn another_name_for_the_core_is_found() {
        let faults = crate_violations("a", "use neoteletext_core as engine;\n", &BTreeSet::new());
        assert_eq!(faults, ["a renames `neoteletext_core`, which hides what it uses"]);
    }
}
