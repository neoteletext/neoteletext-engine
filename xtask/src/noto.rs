//! `cargo xtask check-noto` (REQ-FNT-012, decision E.77): every converted
//! font in `third_party/noto/` is the file its provenance record names, by
//! SHA-256, and every font the record names is there.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// The folder of the Noto-derived fonts.
const FOLDER: &str = "third_party/noto";
/// The provenance record within it.
const PROVENANCE: &str = "third_party/noto/PROVENANCE.md";
/// A checksum line of the record: "- `NAME.bdf`: `SHA-256`".
const LINE_PREFIX: &str = "- `";

/// The checksums the record gives, by file name.
fn recorded(record: &str) -> Result<BTreeMap<String, String>, String> {
    let mut found = BTreeMap::new();
    for line in record.lines() {
        let Some(rest) = line.strip_prefix(LINE_PREFIX) else { continue };
        let Some((name, after)) = rest.split_once("`: `") else { continue };
        if Path::new(name).extension().is_none_or(|extension| extension != "bdf") {
            continue;
        }
        let digest = after.trim_end_matches('`');
        if found.insert(name.to_string(), digest.to_string()).is_some() {
            return Err(format!("{PROVENANCE} records {name} twice"));
        }
    }
    Ok(found)
}

/// Checks the fonts against their record.
pub fn check(root: &Path) -> Result<(), String> {
    let record = fs::read_to_string(root.join(PROVENANCE)).map_err(|e| format!("{PROVENANCE}: {e}"))?;
    let mut expected = recorded(&record)?;
    let entries = fs::read_dir(root.join(FOLDER)).map_err(|e| format!("{FOLDER}: {e}"))?;
    let mut checked = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "bdf") {
            continue;
        }
        let name = path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
        let digest = expected.remove(&name).ok_or(format!("{FOLDER}/{name} is not in {PROVENANCE}"))?;
        let bytes = fs::read(&path).map_err(|e| format!("{FOLDER}/{name}: {e}"))?;
        let actual = neoteletext_web::sha256::to_hex(&neoteletext_web::sha256::digest(&bytes));
        if digest.as_bytes() != actual {
            return Err(format!(
                "{FOLDER}/{name} has SHA-256 {}, but {PROVENANCE} records {digest}",
                String::from_utf8_lossy(&actual)
            ));
        }
        checked += 1;
    }
    if let Some(name) = expected.keys().next() {
        return Err(format!("{PROVENANCE} records {name}, which is not in {FOLDER}"));
    }
    println!("{FOLDER}: {checked} fonts match {PROVENANCE}");
    Ok(())
}
