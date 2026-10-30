//! `neottx text` (REQ-RND-008): one page, or all, as UTF-8 text.

use super::{Selection, grid_of, mosaic_style, parse_selection, sandbox_closed, selected};
use crate::ansi::plain;
use neoteletext_app::Failure;
use neoteletext_app::input::{self, Options};
use std::io::Write;

/// `neottx text`.
pub fn text(options: &Options, page: &str, reveal: bool, legacy_mosaics: bool) -> Result<(), Failure> {
    let selection = parse_selection(page)?;
    let mut session = input::load_for(options, selection.page())?;
    sandbox_closed(options.no_sandbox)?;
    let mut out = std::io::stdout().lock();
    for key in selected(&session, selection)? {
        let grid = grid_of(&mut session, key)?;
        if selection == Selection::All {
            let _ = writeln!(out, "=== {}:{:04X}", String::from_utf8_lossy(&key.address.ascii()), key.subcode.value());
        }
        let written = plain(&grid, reveal, mosaic_style(legacy_mosaics));
        let _ = out.write_all(written.as_bytes());
    }
    Ok(())
}
