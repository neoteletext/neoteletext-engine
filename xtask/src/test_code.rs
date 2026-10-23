//! Test code told apart from production code, for the source checks
//! (`check-layers`, `check-constants`, `check-io`) and the traceability
//! table.
//!
//! A file is test code when the module it holds is declared under a `cfg`
//! that holds only under `test` (`#[cfg(test)] mod tests;`), in its parent
//! file or in any module above it. In any other file, each item, field,
//! statement or expression under such a `cfg` is test code, wherever it is
//! in the file, and the rest is production code: a test-only item above
//! production code no longer hides that code from the checks, as cutting
//! the file at its first `#[cfg(test)]` did.

mod items;
mod mask;

use std::fs;
use std::path::Path;

/// The production code of the file at `path`: empty for a test module's
/// file; otherwise its text with each test-only part blanked, so that line
/// numbers stay those of the file.
pub(crate) fn production_source(path: &Path) -> String {
    split(path, &|file| fs::read_to_string(file).ok()).0
}

/// The production code and the test code of the file at `path`, each with
/// the other blanked. A file that is not Rust is all production code.
pub(crate) fn production_and_tests(path: &Path) -> (String, String) {
    split(path, &|file| fs::read_to_string(file).ok())
}

/// [`production_and_tests`], reading files with `read`.
fn split(path: &Path, read: &dyn Fn(&Path) -> Option<String>) -> (String, String) {
    let text = read(path).unwrap_or_default();
    if path.extension().is_none_or(|extension| extension != "rs") {
        return (text, String::new());
    }
    if is_test_module(path, read) {
        return (String::new(), text);
    }
    production_and_tests_of(&text)
}

/// The production code and the test code of one Rust source text, by its
/// test-only items alone.
fn production_and_tests_of(text: &str) -> (String, String) {
    let ranges = items::test_ranges(&mask::code_only(text));
    let mut production = text.as_bytes().to_vec();
    let mut tests = text.as_bytes().to_vec();
    let mut outside = 0;
    for range in &ranges {
        blank(&mut tests[outside..range.start]);
        blank(&mut production[range.clone()]);
        outside = range.end;
    }
    blank(&mut tests[outside..]);
    // Only whole runs of bytes between ASCII brackets and attributes were
    // blanked, so both are still UTF-8.
    (String::from_utf8_lossy(&production).into_owned(), String::from_utf8_lossy(&tests).into_owned())
}

/// The production code of one Rust source text.
#[cfg(test)]
pub(crate) fn production_text(text: &str) -> String {
    production_and_tests_of(text).0
}

/// Every byte a space, newlines kept.
fn blank(bytes: &mut [u8]) {
    for byte in bytes.iter_mut().filter(|byte| **byte != b'\n') {
        *byte = b' ';
    }
}

/// Whether the file at `path` holds a module declared under a test-only
/// `cfg`, itself or through a module it is inside (the Reference, "Modules":
/// `a/b.rs` and `a/b/mod.rs` are module `b`, declared in `a.rs` or
/// `a/mod.rs`, or in `lib.rs` or `main.rs` when `a` is `src`).
fn is_test_module(path: &Path, read: &dyn Fn(&Path) -> Option<String>) -> bool {
    let (Some(dir), Some(stem)) = (path.parent(), path.file_stem().and_then(|stem| stem.to_str())) else {
        return false;
    };
    let dir_name = dir.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    // A crate root, or a file outside `src` (an integration test, a build
    // script): never declared by another file.
    if (dir_name == "src" && ["lib", "main"].contains(&stem)) || dir_name == "bin" || !inside_src(dir) {
        return false;
    }
    if stem == "mod" {
        let (Some(parent), Some(name)) = (dir.parent(), dir.file_name().and_then(|name| name.to_str())) else {
            return false;
        };
        return declared_under_test(parent, name, read);
    }
    declared_under_test(dir, stem, read)
}

/// Whether module `name`, whose file is in `dir`, is declared under a
/// test-only `cfg`, or inside a module that is.
fn declared_under_test(dir: &Path, name: &str, read: &dyn Fn(&Path) -> Option<String>) -> bool {
    let declarers = if dir.file_name().is_some_and(|dir_name| dir_name == "src") {
        vec![dir.join("lib.rs"), dir.join("main.rs")]
    } else {
        vec![dir.with_extension("rs"), dir.join("mod.rs")]
    };
    for declarer in declarers {
        if let Some(text) = read(&declarer)
            && let Some(gated) = declaration_gated(&text, name)
        {
            return gated || is_test_module(&declarer, read);
        }
    }
    // No file declares it: `dir` is then an inline module of the file
    // above, `mod dir { mod name; }`, and is test code if that module is.
    match (dir.parent(), dir.file_name().and_then(|dir_name| dir_name.to_str())) {
        (Some(parent), Some(dir_name)) if dir_name != "src" && inside_src(parent) => {
            declared_under_test(parent, dir_name, read)
        }
        _ => false,
    }
}

/// Whether `dir` is a crate's `src` directory or below one.
fn inside_src(dir: &Path) -> bool {
    dir.components().any(|component| component.as_os_str() == "src")
}

/// Whether the declarations of module `name` in `text` (`mod name;` or
/// `mod name {`) are all test-only, or `None` when `text` declares no such
/// module.
fn declaration_gated(text: &str, name: &str) -> Option<bool> {
    let code = mask::code_only(text);
    let tests = items::test_ranges(&code);
    let mut found = None;
    for (index, _) in code.iter().enumerate().filter(|&(_, &byte)| byte == b'm') {
        let after_word = index.checked_sub(1).is_none_or(|before| !mask::is_identifier_byte(code[before]));
        if !after_word || items::word_at(&code, index) != "mod" {
            continue;
        }
        let name_at = items::skip_space(&code, index + "mod".len());
        if items::word_at(&code, name_at) != name {
            continue;
        }
        let end = items::skip_space(&code, name_at + name.len());
        if !matches!(code.get(end), Some(b';' | b'{')) {
            continue;
        }
        let gated = tests.iter().any(|range| range.contains(&index));
        found = Some(found.unwrap_or(true) && gated);
    }
    found
}

#[cfg(test)]
mod tests;
