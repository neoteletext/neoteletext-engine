//! `neottx write-text --set SET --page MPP INPUT OUTPUT` (REQ-CLI-010):
//! a TTI page from a UTF-8 text file in one of neoteletext's own sets, or
//! with `--language TAG` in the set the engine's table of languages gives
//! (REQ-2181), for `--level` (REQ-2151).

/// How the page's set is named, and the level it is written for.
#[derive(Clone, Copy, Debug)]
pub struct TextChoice<'a> {
    /// A set of the writer's table.
    pub set: Option<&'a str>,
    /// A BCP 47 language tag, when no set is named.
    pub language: Option<&'a str>,
    /// Written for Level 3.5 and below rather than NeoTeletext's own
    /// receivers.
    pub standard: bool,
}

#[cfg(not(feature = "write-text"))]
use neoteletext_app::Failure;
#[cfg(not(feature = "write-text"))]
use std::path::Path;

#[cfg(feature = "write-text")]
mod run;

#[cfg(feature = "write-text")]
pub use run::write_text;

/// `neottx write-text` in a build without it.
#[cfg(not(feature = "write-text"))]
pub fn write_text(_: TextChoice<'_>, _: &str, _: &Path, _: &Path, _: bool) -> Result<(), Failure> {
    Err(neoteletext_app::not_in_build("writing pages from text", "write-text"))
}
