//! The `User-Agent` string (REQ-WEB-031, decision E.99): the port's
//! program, its significant version and its platform's family, and the
//! engine when the program is not one of the engine's own, as a web
//! browser's user agent would have been without the compatibility lies:
//! `NeoTeletext/0.1 (macOS)`, `neottx/0.1 (Linux)`,
//! `Teefax/1 (ESP-IDF) neoteletext/0.1`.

/// The longest user agent string, in bytes (REQ-WEB-031).
pub const MAX_USER_AGENT: usize = 64;
/// The engine's own product name.
const ENGINE: &str = "neoteletext";
/// The engine's version, of which the significant part is sent.
const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The program a port names itself as.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Product {
    /// Its name, an RFC 9110 token: `NeoTeletext`, `neottx`.
    pub name: &'static str,
    /// Its version, full (`0.1.0`): only the significant part is sent.
    pub version: &'static str,
    /// The operating system's family, or for a port without one its board
    /// or framework: `macOS`, `ESP-IDF`. [`os_family`] gives it on the
    /// systems Rust names.
    pub family: &'static str,
    /// Whether it is one of the engine's own programs, which do not name
    /// the engine again.
    pub engine_own: bool,
}

/// Why a product cannot be a user agent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UserAgentError {
    /// A name or version that is not an RFC 9110 token.
    Token,
    /// A family that is empty or not printable ASCII outside `(`, `)` and
    /// `\`.
    Family,
    /// The string would be longer than [`MAX_USER_AGENT`].
    TooLong,
}

/// The significant part of a version: the major version, or before 1.0
/// `0.` and the minor (`0.1.0` is `0.1`, `2.3.4` is `2`).
#[must_use]
pub fn significant_version(version: &str) -> &str {
    let mut parts = version.split(['.', '-', '+']);
    let major = parts.next().unwrap_or(version);
    if major != "0" {
        return major;
    }
    match parts.next() {
        Some(minor) => version.get(..major.len().saturating_add(1).saturating_add(minor.len())).unwrap_or(major),
        None => major,
    }
}

/// The operating system's family on the systems Rust names, or `None` for
/// a port, such as a microcontroller's, that names its own board or
/// framework.
#[must_use]
pub const fn os_family() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("macOS")
    } else if cfg!(target_os = "ios") {
        Some("iOS")
    } else if cfg!(target_os = "windows") {
        Some("Windows")
    } else if cfg!(target_os = "android") {
        Some("Android")
    } else if cfg!(target_os = "linux") {
        Some("Linux")
    } else if cfg!(target_os = "freebsd") {
        Some("FreeBSD")
    } else {
        None
    }
}

/// RFC 9110 section 5.6.2: a token's characters.
const fn tchar(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
        )
}

fn token(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(tchar)
}

/// RFC 9110 section 5.6.5: a comment's text, printable ASCII without the
/// parentheses and the backslash that would need quoting.
fn comment(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| (b' '..=b'~').contains(&byte) && !matches!(byte, b'(' | b')' | b'\\'))
}

/// A user agent string, composed once for the life of the agent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UserAgentString {
    bytes: [u8; MAX_USER_AGENT],
    len: usize,
}

impl Default for UserAgentString {
    /// The engine's own: `neoteletext/` and its significant version, for a
    /// port that names no product.
    fn default() -> Self {
        let mut string = Self { bytes: [0; MAX_USER_AGENT], len: 0 };
        string.push(ENGINE);
        string.push("/");
        string.push(significant_version(ENGINE_VERSION));
        string
    }
}

impl UserAgentString {
    /// `product`'s string, checked against RFC 9110's product syntax and
    /// [`MAX_USER_AGENT`].
    ///
    /// # Errors
    /// A name or version that is not a token, a family that cannot be a
    /// comment, or a string too long.
    pub fn new(product: &Product) -> Result<Self, UserAgentError> {
        let version = significant_version(product.version);
        if !token(product.name) || !token(version) {
            return Err(UserAgentError::Token);
        }
        if !comment(product.family) {
            return Err(UserAgentError::Family);
        }
        let engine = significant_version(ENGINE_VERSION);
        let own = [product.name, "/", version, " (", product.family, ")"];
        let named = [" ", ENGINE, "/", engine];
        let length = own.iter().map(|part| part.len()).fold(0_usize, usize::saturating_add);
        let length = if product.engine_own {
            length
        } else {
            named.iter().map(|part| part.len()).fold(length, usize::saturating_add)
        };
        if length > MAX_USER_AGENT {
            return Err(UserAgentError::TooLong);
        }
        let mut string = Self { bytes: [0; MAX_USER_AGENT], len: 0 };
        for part in own {
            string.push(part);
        }
        if !product.engine_own {
            for part in named {
                string.push(part);
            }
        }
        Ok(string)
    }

    /// The port's product, or the engine's own string when it names none or
    /// names one that cannot be a user agent.
    #[must_use]
    pub fn of(product: Option<&Product>) -> Self {
        product.and_then(|product| Self::new(product).ok()).unwrap_or_default()
    }

    fn push(&mut self, text: &str) {
        for &byte in text.as_bytes() {
            if let Some(slot) = self.bytes.get_mut(self.len) {
                *slot = byte;
                self.len = self.len.saturating_add(1);
            }
        }
    }

    /// The string's bytes, as the header carries them.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or(&[])
    }

    /// The string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or(ENGINE)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a test")]
mod tests {
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
}
