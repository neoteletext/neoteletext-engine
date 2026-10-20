//! Source checks that enforce the architecture and the project rules.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    ("level1", &["charset", "acq", "colour", "drcs"]),
    ("level15", &["charset", "level1", "packet"]),
    ("colour", &[]),
    ("drcs", &["packet", "acq", "colour"]),
    ("objects", &["packet", "acq"]),
    ("enhance", &["charset", "level1", "packet", "acq", "colour", "objects", "drcs"]),
    // REQ-CHR-015: letters are joined after the level's pass.
    ("joining", &["charset", "level1"]),
    ("compose", &["acq", "level1", "level15", "enhance", "joining"]),
    ("glyphs", &["charset"]),
    ("render", &["glyphs", "level1", "colour", "drcs"]),
    ("viewer", &["acq", "level1", "compose", "colour", "render", "service", "nav", "forms", "charset"]),
    ("nav", &["acq", "level1", "level15", "enhance", "colour", "render", "service", "forms", "charset"]),
    ("forms", &["acq", "level1", "level15", "enhance", "colour", "render", "service", "charset"]),
    ("port", &[]),
    ("policy", &[]),
    ("config", &[]),
    ("trace", &[]),
];

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
        let source = non_test_source(&file);
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

/// The permitted dependencies of each library crate (REQ-GEN-017,
/// decision E.38). `std`-only crates are not listed.
const PERMITTED: &[(&str, &[&str])] = &[
    ("neoteletext-core", &[]),
    ("neoteletext-formats", &["neoteletext-core"]),
    ("neoteletext-slicer", &["neoteletext-core"]),
    ("neoteletext-web", &["neoteletext-core", "embedded-io"]),
    ("neoteletext-image", &["neoteletext-core"]),
    ("neoteletext-audio", &["neoteletext-core"]),
    ("neoteletext-audio-mix", &[]),
    ("neoteletext-input", &["neoteletext-core", "neoteletext-audio"]),
];

/// REQ-GEN-017: compares each microcontroller crate's resolved normal
/// dependencies, with default features, against its permitted list.
pub fn dependencies(root: &Path) -> Result<(), String> {
    let mut violations = Vec::new();
    for (krate, permitted) in PERMITTED {
        let output = Command::new("cargo")
            .current_dir(root)
            .args(["tree", "-p", krate, "-e", "normal", "--prefix", "none", "--no-default-features"])
            .output()
            .map_err(|e| format!("cargo tree: {e}"))?;
        if !output.status.success() {
            return Err(format!("cargo tree -p {krate} failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let names: BTreeSet<String> = text
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .map(str::to_string)
            .filter(|name| name != krate)
            .collect();
        for name in names {
            if !permitted.contains(&name.as_str()) {
                violations.push(format!("{krate} depends on `{name}`, which is not permitted"));
            }
        }
    }
    for (krate, permitted) in APPLICATION_CRATES {
        let output = Command::new("cargo")
            .current_dir(root)
            .args(["tree", "-p", krate, "-e", "normal", "--prefix", "none", "--depth", "1", "--all-features"])
            .output()
            .map_err(|e| format!("cargo tree: {e}"))?;
        if !output.status.success() {
            return Err(format!("cargo tree -p {krate} failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let workspace_crates: BTreeSet<&str> = text
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .filter(|name| name.starts_with(WORKSPACE_PREFIX) && name != krate)
            .collect();
        for name in workspace_crates {
            if !permitted.contains(&name) {
                violations.push(format!("{krate} depends on `{name}`, which is not below it"));
            }
        }
    }
    report(&violations)
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

/// Files that are data tables of code points or glyphs, where the literal
/// values are characters, not teletext constants.
const DATA_FILES: &[&str] = &["charset/tables.rs", "glyphs/bedstead.rs", "consts.rs"];
/// Teletext values that must only appear in the constants module
/// (REQ-GEN-014).
const WATCHED_LITERALS: &[&str] = &["0x27", "0xE4", "0xe4", "6937500", "6_937_500", "0x55"];

/// REQ-GEN-014: literal teletext values outside the constants module.
pub fn constants(root: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    let mut violations = Vec::new();
    for file in files {
        let display = file.to_string_lossy().to_string();
        if DATA_FILES.iter().any(|data| display.ends_with(data)) {
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
const FORBIDDEN: &[&str] =
    &["std::fs", "std::net", "std::process", "std::env", "std::thread", "SystemTime", "Instant::now", "std::time"];

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
    rust_files(source, &mut files);
    for file in files {
        let code_of_file = non_test_source(&file);
        for (number, line) in code_of_file.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            for item in items {
                if code.contains(item) {
                    violations.push(format!("{}:{}: {item}", file.display(), number + 1));
                }
            }
        }
    }
}

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
