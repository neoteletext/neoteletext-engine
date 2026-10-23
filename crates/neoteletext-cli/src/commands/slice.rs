//! `neottx slice`: raw sampled video through the software data slicer, or
//! its refusal in a build without it.

use neoteletext_app::Failure;
use neoteletext_app::input::Options;
use std::path::Path;

/// `neottx slice`.
#[cfg(feature = "slice")]
pub fn slice(options: &Options, output: &Path, preset: &str) -> Result<(), Failure> {
    let counters = neoteletext_app::slice::slice(options, output, preset)?;
    eprintln!("{counters:?}");
    Ok(())
}

/// `neottx slice` in a build without the slicer.
#[cfg(not(feature = "slice"))]
pub fn slice(_options: &Options, _output: &Path, _preset: &str) -> Result<(), Failure> {
    Err(neoteletext_app::not_in_build("raw video slicing", "slice"))
}
