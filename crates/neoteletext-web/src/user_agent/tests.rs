#![allow(clippy::unwrap_used, reason = "a test")]
//! The tests of the User-Agent string (REQ-WEB-031).

use super::*;
use std::boxed::Box;
use std::format;

const fn product(name: &'static str, version: &'static str, family: &'static str, engine_own: bool) -> Product {
    Product { name, version, family, engine_own }
}

/// REQ-WEB-031: the engine's own programs, another's, and none.
#[test]
fn the_string_names_the_program_its_version_and_platform() {
    assert_eq!(
        UserAgentString::new(&product("NeoTeletext", "0.1.0", "macOS", true)).unwrap().as_str(),
        "NeoTeletext/0.1 (macOS)"
    );
    assert_eq!(
        UserAgentString::new(&product("neottx", "0.1.3", "Linux", true)).unwrap().as_str(),
        "neottx/0.1 (Linux)"
    );
    let engine = significant_version(ENGINE_VERSION);
    assert_eq!(
        UserAgentString::new(&product("Teefax", "1.4.2", "ESP-IDF", false)).unwrap().as_str(),
        format!("Teefax/1 (ESP-IDF) neoteletext/{engine}")
    );
    assert_eq!(UserAgentString::default().as_str(), format!("neoteletext/{engine}"));
    assert_eq!(UserAgentString::of(None), UserAgentString::default());
}

#[test]
fn only_the_significant_version_goes() {
    assert_eq!(significant_version("0.1.0"), "0.1");
    assert_eq!(significant_version("0.12.7"), "0.12");
    assert_eq!(significant_version("2.3.4"), "2");
    assert_eq!(significant_version("1.0.0-beta.2"), "1");
    assert_eq!(significant_version("0.3.0-rc1"), "0.3");
    assert_eq!(significant_version("7"), "7");
    assert_eq!(significant_version("0"), "0");
}

#[test]
fn what_breaks_the_syntax_or_the_length_is_refused() {
    assert_eq!(UserAgentString::new(&product("Neo Teletext", "0.1.0", "macOS", true)), Err(UserAgentError::Token));
    assert_eq!(UserAgentString::new(&product("Neo/Teletext", "0.1.0", "macOS", true)), Err(UserAgentError::Token));
    assert_eq!(UserAgentString::new(&product("x", "", "macOS", true)), Err(UserAgentError::Token));
    assert_eq!(UserAgentString::new(&product("x", "1", "mac(OS)", true)), Err(UserAgentError::Family));
    assert_eq!(UserAgentString::new(&product("x", "1", "", true)), Err(UserAgentError::Family));
    assert_eq!(UserAgentString::new(&product("x", "1", "macOS\u{e9}", true)), Err(UserAgentError::Family));
    let long: &'static str = Box::leak("a".repeat(MAX_USER_AGENT).into_boxed_str());
    assert_eq!(UserAgentString::new(&product(long, "1", "Linux", true)), Err(UserAgentError::TooLong));
    assert_eq!(
        UserAgentString::of(Some(&product("bad name", "1", "Linux", true))),
        UserAgentString::default(),
        "the engine's own instead"
    );
    assert!(
        UserAgentString::new(&product("a", "1", "b", true))
            .unwrap()
            .as_bytes()
            .iter()
            .all(|byte| (b' '..=b'~').contains(byte))
    );
}

#[test]
fn the_family_is_the_systems_on_the_systems_rust_names() {
    if cfg!(target_os = "macos") {
        assert_eq!(os_family(), Some("macOS"));
    }
    if cfg!(target_os = "linux") {
        assert_eq!(os_family(), Some("Linux"));
    }
    if cfg!(target_os = "none") {
        assert_eq!(os_family(), None);
    }
}
