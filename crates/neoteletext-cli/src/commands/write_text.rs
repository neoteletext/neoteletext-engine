//! `neottx write-text --set SET --page MPP INPUT OUTPUT` (REQ-CLI-010):
//! a TTI page from a UTF-8 text file in one of neoteletext's own sets.

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
pub fn write_text(_: &str, _: &str, _: &Path, _: &Path, _: bool) -> Result<(), Failure> {
    Err(neoteletext_app::not_in_build("writing pages from text", "write-text"))
}
