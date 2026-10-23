//! `neottx write-text` in a build with it.

use crate::bounded_file::read_bounded;
use crate::commands::io_failure;
use neoteletext_app::sandbox::{self, Policy};
use neoteletext_app::{Failure, exit};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// The largest text file read. A page holds 23 rows of 40 cells, at most
/// four UTF-8 bytes a character with a mark of two more: 23 x 40 x 6 is
/// 5,520 bytes, so this leaves room for long lines that are refused.
const TEXT_MAX_FILE_BYTES: usize = 16 * 1024;

/// `neottx write-text`.
pub fn write_text(set: &str, page: &str, input: &Path, output: &Path, no_sandbox: bool) -> Result<(), Failure> {
    let mpp = u16::from_str_radix(page, 16)
        .ok()
        .filter(|&mpp| neoteletext_core::types::PageAddress::from_mpp(mpp).is_some())
        .ok_or_else(|| (exit::USAGE, format!("{page}: not a page number 100 to 8FF")))?;
    let bytes = read_bounded(input, TEXT_MAX_FILE_BYTES)?;
    let text = String::from_utf8(bytes).map_err(|_| (exit::INPUT, format!("{}: not UTF-8", input.display())))?;
    let mut out = File::create(output).map_err(|error| io_failure(output, error))?;
    // Every file is open (REQ-SEC-017).
    sandbox::apply(Policy { writable_directory: None, network: false, readable: &[] }, no_sandbox)
        .map_err(|message| (exit::INPUT, message))?;
    let written = neoteletext_app::write_text::write_text(set, mpp, &text).map_err(|error| io_failure(input, error))?;
    out.write_all(&written.tti).and_then(|()| out.flush()).map_err(|error| io_failure(output, error))?;
    eprintln!("{}: {} rows, {} X/26 triplets", output.display(), written.rows, written.triplets);
    Ok(())
}
