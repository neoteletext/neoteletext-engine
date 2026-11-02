//! The real-broadcast corpus as a test reads it (REQ-TST-022, REQ-SEC-022),
//! included by each crate's corpus tests. Such a test is marked ignored, so a
//! run counts it as not run, and asked for with `--ignored` it fails if a file
//! is absent or differs from its pin: it never reports `ok` without reading.
// Each test file uses its own subset of these.
#![allow(dead_code, clippy::expect_used, clippy::panic, missing_docs)]

use std::path::{Path, PathBuf};
use std::process::Command;

const VARIABLE: &str = "NEOTELETEXT_CORPUS";
const MANIFEST: &str = include_str!("corpus-manifest.txt");
/// The manifest's mark for a line whose checksum nobody has yet fixed.
const UNPINNED: &str = "unpinned";

/// The corpus directory, or why there is none.
fn directory_in(variable: Option<std::ffi::OsString>) -> Result<PathBuf, String> {
    variable.map(PathBuf::from).ok_or_else(|| {
        format!("{VARIABLE} is not set: name the corpus directory; `cargo xtask corpus` fetches into it")
    })
}

/// The pinned checksum of corpus file `name` in a manifest's text.
fn pin<'a>(manifest: &'a str, name: &str) -> Option<&'a str> {
    manifest.lines().filter(|line| !line.starts_with('#')).find_map(|line| {
        let mut fields = line.split_whitespace();
        (fields.next() == Some(name)).then(|| fields.nth(1)).flatten().filter(|sum| *sum != UNPINNED)
    })
}

/// That the file at `path` has the checksum `pin`, if it has a pin. Local
/// recordings that no address serves have none until a machine holds them.
fn verified(path: &Path, pin: Option<&str>) -> Result<(), String> {
    let Some(pin) = pin else { return Ok(()) };
    let output = Command::new("shasum").args(["-a", "256"]).arg(path).output().map_err(|e| format!("shasum: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    match text.split_whitespace().next() {
        Some(sum) if sum == pin => Ok(()),
        sum => Err(format!("{}: SHA-256 {sum:?} is not the pinned {pin}", path.display())),
    }
}

/// `directory`'s file `name`, there and as pinned, or why not.
fn located(directory: &Path, name: &str, manifest: &str) -> Result<PathBuf, String> {
    let path = directory.join(name);
    if !path.is_file() {
        return Err(format!("corpus file {name} is not in {}", directory.display()));
    }
    verified(&path, pin(manifest, name))?;
    Ok(path)
}

/// The corpus directory. Panics if `$NEOTELETEXT_CORPUS` is not set.
pub fn directory() -> PathBuf {
    directory_in(std::env::var_os(VARIABLE)).unwrap_or_else(|why| panic!("{why}"))
}

/// The path of corpus file `name`, verified against its pin. Panics if the
/// file is absent or differs.
pub fn path(name: &str) -> PathBuf {
    located(&directory(), name, MANIFEST).unwrap_or_else(|why| panic!("{why}"))
}

/// The bytes of corpus file `name`, verified against its pin first.
pub fn read(name: &str) -> Vec<u8> {
    std::fs::read(path(name)).unwrap_or_else(|why| panic!("{name}: {why}"))
}

/// A corpus file's checks, without the variable, for the tests of this file.
#[derive(Debug, PartialEq, Eq)]
pub enum Checked {
    Found,
    Refused(String),
}

/// What [`path`] does with `manifest` in place of the repository's.
pub fn check(directory: Option<&Path>, name: &str, manifest: &str) -> Checked {
    match directory_in(directory.map(|path| path.as_os_str().to_owned()))
        .and_then(|directory| located(&directory, name, manifest))
    {
        Ok(_) => Checked::Found,
        Err(why) => Checked::Refused(why),
    }
}
