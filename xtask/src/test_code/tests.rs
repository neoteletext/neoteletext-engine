//! Test code is told from production code by module and by item, never by
//! cutting a file at its first `#[cfg(test)]`.

use super::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A production file whose first item is test-only, with production code
/// after it and a test module at its end.
const EARLY_TEST_ITEM: &str = "\
#[cfg(test)]
use std::sync::Arc;

pub fn run() -> u8 { std::thread::spawn(|| 1); 0x27 }

#[cfg(test)]
mod tests {
    fn helper() { std::thread::sleep(); let _ = \"}\"; let _ = '{'; }
}
";

#[test]
fn an_early_test_item_does_not_hide_the_code_after_it() {
    let (production, tests) = production_and_tests_of(EARLY_TEST_ITEM);
    let line = |text: &str, wanted: &str| text.lines().position(|line| line.contains(wanted)).map(|index| index + 1);
    assert_eq!(line(&production, "std::thread::spawn"), Some(4), "the production code is kept on its own line");
    assert_eq!(line(&production, "0x27"), Some(4));
    assert_eq!(line(&production, "Arc"), None, "the test-only use line is test code");
    assert_eq!(line(&production, "sleep"), None, "the test module is test code, braces in literals included");
    assert_eq!(line(&tests, "sleep"), Some(8));
    assert_eq!(line(&tests, "spawn"), None);
    assert_eq!(production.lines().count(), EARLY_TEST_ITEM.lines().count());
}

#[test]
fn test_only_fields_statements_and_items_are_cut_exactly() {
    let text = "\
struct Wakes {
    ended: bool,
    #[cfg(test)]
    pub(super) waits: std::sync::Arc<u8>,
    network: bool,
}
fn new() -> Wakes {
    #[cfg(test)]
    count(1, 2);
    Wakes { ended: false, #[cfg(test)] waits: Arc::default(), network: true }
}
#[cfg(all(test, feature = \"bedstead\"))]
#[must_use]
pub(crate) const fn entries(&self) -> &'static [u8] { self.entries }
#[cfg(any(test, feature = \"std\"))]
fn shared() {}
#[cfg(not(test))]
fn production_only() {}
";
    let production = production_text(text);
    for kept in ["ended: bool", "network: bool", "network: true", "fn shared", "fn production_only", "fn new"] {
        assert!(production.contains(kept), "{kept} is production code:\n{production}");
    }
    for cut in ["waits", "count(", "entries", "bedstead"] {
        assert!(!production.contains(cut), "{cut} is test code:\n{production}");
    }
}

#[test]
fn comments_and_literals_are_not_code() {
    let text = "\
// #[cfg(test)] mod hidden;
/* nested /* #[cfg(test)] */ still a comment */
const RAW: &str = r#\"#[cfg(test)] mod x { \"#;
const BYTES: &[u8] = b\"#[cfg(test)]\";
fn lifetime<'a>(text: &'a str) -> char { let _ = text; '\\'' }
fn production() {}
";
    let production = production_text(text);
    assert!(production.contains("fn production()"), "{production}");
    assert!(production.contains("fn lifetime"), "{production}");
    assert!(production.contains("const RAW"), "{production}");
}

/// A crate's files, in memory.
fn tree(files: &[(&str, &str)]) -> BTreeMap<PathBuf, String> {
    files.iter().map(|(path, text)| (PathBuf::from(path), (*text).to_string())).collect()
}

#[test]
fn a_file_is_test_code_when_its_module_is_declared_for_tests() {
    let files = tree(&[
        (
            "crate/src/lib.rs",
            "mod a;\nmod b;\n#[cfg(test)]\nmod tests;\n#[cfg(all(test, feature = \"x\"))]\nmod feature_tests;\n",
        ),
        ("crate/src/a.rs", "mod inner;\n\n#[cfg(test)]\nmod tests;\n"),
        ("crate/src/a/inner.rs", "pub fn inner() {}\n"),
        ("crate/src/a/tests.rs", "#![allow(clippy::unwrap_used, reason = \"a test\")]\nuse super::*;\n"),
        ("crate/src/tests.rs", "mod deep;\n"),
        ("crate/src/tests/deep.rs", "fn deep() {}\n"),
        ("crate/src/feature_tests.rs", "fn feature_test() {}\n"),
        ("crate/src/b/mod.rs", "#[cfg(test)]\nmod inline {\n    mod child;\n}\nmod real;\n"),
        ("crate/src/b/inline/child.rs", "fn child() {}\n"),
        ("crate/src/b/real.rs", "fn real() {}\n"),
        ("crate/tests/integration.rs", "fn integration() {}\n"),
    ]);
    let read = |path: &Path| files.get(path).cloned();
    let is_test = |path: &str| is_test_module(Path::new(path), &read);
    for production in
        ["crate/src/lib.rs", "crate/src/a.rs", "crate/src/a/inner.rs", "crate/src/b/mod.rs", "crate/src/b/real.rs"]
    {
        assert!(!is_test(production), "{production} is production code");
    }
    for test in [
        "crate/src/a/tests.rs",
        "crate/src/tests.rs",
        "crate/src/tests/deep.rs",
        "crate/src/feature_tests.rs",
        "crate/src/b/inline/child.rs",
    ] {
        assert!(is_test(test), "{test} is test code");
    }
    assert!(!is_test("crate/tests/integration.rs"), "an integration test is not a module of the crate");
    let (production, tests) = split(Path::new("crate/src/a/tests.rs"), &read);
    assert!(production.is_empty() && tests.contains("use super::*;"), "a test module's file is all test code");
}
