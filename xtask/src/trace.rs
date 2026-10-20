//! The traceability table (REQ-DOC-003): for every requirement defined in
//! `docs/spec/requirements/`, the source files and test files that name it. A
//! requirement no file names is listed as such, so gaps stay visible.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
        if path.is_dir() && name != "target" && !name.starts_with('.') {
            files(&path, out);
        } else if path.extension().is_some_and(|ext| ["rs", "py", "toml", "sh"].iter().any(|wanted| ext == *wanted)) {
            out.push(path);
        }
    }
}

/// Configuration files at the top of the repository that can carry a
/// requirement, such as a lint that enforces it.
const ROOT_FILES: [&str; 3] = ["Cargo.toml", "clippy.toml", "rust-toolchain.toml"];
/// Documents that record checks by inspection, besides those in docs/.
const RECORD_FILES: [&str; 7] = [
    "ARCHITECTURE.md",
    "THREAT-MODEL.md",
    "SECURITY.md",
    "IMPLEMENTATION-LOG.md",
    "README.md",
    "docs/spec/module-sources.md",
    "docs/spec/open-questions.md",
];
/// The generated table, which must not count as a record of itself.
const TABLE: &str = "docs/traceability.md";
/// The requirement files, in the order of the specification's areas
/// (`docs/spec/requirements/README.md`), so the table keeps that order.
const REQUIREMENT_FILES: [&str; 28] = [
    "sec",
    "gen",
    "arch",
    "ecc",
    "pkt",
    "acq",
    "svc",
    "chr",
    "l1",
    "l15",
    "l25",
    "obj",
    "drc",
    "nav",
    "rnd",
    "inp",
    "slc",
    "prf",
    "cli",
    "tst",
    "doc",
    "fsm",
    "web",
    "brw",
    "fnt",
    "aud",
    "int",
    "out-of-scope",
];
/// The design documents, which record checks by inspection as the other
/// documents in docs/ do.
const DESIGN_DIR: &str = "docs/spec/design";

fn identifiers(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = text;
    while let Some(index) = rest.find("REQ-") {
        let tail = &rest[index..];
        let id: String = tail.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        if id.matches('-').count() == 2 && id.ends_with(|c: char| c.is_ascii_digit()) {
            found.insert(id);
        }
        rest = &rest[index + 4..];
    }
    found
}

/// Writes docs/traceability.md.
pub fn generate(root: &Path) -> Result<(), String> {
    let mut spec = String::new();
    for name in REQUIREMENT_FILES {
        let path = root.join("docs/spec/requirements").join(format!("{name}.md"));
        spec.push_str(&fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?);
        spec.push('\n');
    }
    let mut defined: Vec<(String, String, bool, bool)> = Vec::new();
    let mut lines = spec.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix("**REQ-") {
            let id: String = format!("REQ-{}", rest.chars().take_while(|c| *c != ' ' && *c != '*').collect::<String>());
            let title =
                rest.split_once(" — ").map_or("", |(_, title)| title).trim_end_matches("**").trim_end_matches('.');
            let withdrawn = title.starts_with("Withdrawn");
            let phase2 = lines.peek().is_some_and(|next| next.starts_with("*Phase:* Phase 2"));
            defined.push((id, title.to_string(), withdrawn, phase2));
        }
    }
    let mut paths = Vec::new();
    for dir in ["crates", "xtask", "tests", "scripts"] {
        files(&root.join(dir), &mut paths);
    }
    for name in ROOT_FILES {
        let path = root.join(name);
        if path.is_file() {
            paths.push(path);
        }
    }
    // The written records of checks by inspection (REQ-DOC-003).
    let mut records: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut record_paths: Vec<PathBuf> = RECORD_FILES.iter().map(|name| root.join(name)).collect();
    for dir in ["docs", DESIGN_DIR] {
        if let Ok(entries) = fs::read_dir(root.join(dir)) {
            record_paths.extend(
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.extension().is_some_and(|ext| ext == "md") && !path.ends_with(TABLE)),
            );
        }
    }
    record_paths.sort();
    for path in &record_paths {
        let text = fs::read_to_string(path).unwrap_or_default();
        let relative = path.strip_prefix(root).unwrap_or(path).to_string_lossy().to_string();
        for id in identifiers(&text) {
            records.entry(id).or_default().insert(relative.clone());
        }
    }
    let mut code: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut tests: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in &paths {
        let text = fs::read_to_string(path).unwrap_or_default();
        let relative = path.strip_prefix(root).unwrap_or(path).to_string_lossy().to_string();
        let (body, test_part) = match text.find("#[cfg(test)]") {
            Some(index) => text.split_at(index),
            None => (text.as_str(), ""),
        };
        let is_test_file = relative.contains("/tests/") || relative.starts_with("tests/");
        for id in identifiers(body) {
            if is_test_file { tests.entry(id).or_default() } else { code.entry(id).or_default() }
                .insert(relative.clone());
        }
        for id in identifiers(test_part) {
            tests.entry(id).or_default().insert(format!("{relative} (unit tests)"));
        }
    }
    let mut out = String::from(
        "# Traceability\n\nGenerated by `cargo xtask traceability` (REQ-DOC-003): for every requirement, the source and test files that name it. A requirement with no entries is not referenced anywhere yet. Requirements verified by inspection or demonstration are linked to the record of that check: the documents in the Records column.\n\n| Requirement | Title | Code | Tests | Records |\n|---|---|---|---|---|\n",
    );
    let mut unreferenced = 0usize;
    for (id, title, withdrawn, phase2) in &defined {
        let code_files =
            code.get(id).map(|set| set.iter().cloned().collect::<Vec<_>>().join("<br>")).unwrap_or_default();
        let test_files =
            tests.get(id).map(|set| set.iter().cloned().collect::<Vec<_>>().join("<br>")).unwrap_or_default();
        let record_files =
            records.get(id).map(|set| set.iter().cloned().collect::<Vec<_>>().join("<br>")).unwrap_or_default();
        let mut title = title.replace('|', "\\|");
        if *phase2 {
            title.push_str(" (Phase 2)");
        }
        if code_files.is_empty() && test_files.is_empty() && record_files.is_empty() && !withdrawn {
            unreferenced += 1;
        }
        let _ = writeln!(out, "| {id} | {title} | {code_files} | {test_files} | {record_files} |");
    }
    let _ = writeln!(out, "\n{} requirements; {unreferenced} not referenced by any file.", defined.len());
    fs::write(root.join(TABLE), out).map_err(|e| e.to_string())?;
    println!("{} requirements, {unreferenced} not referenced", defined.len());
    Ok(())
}
