//! `neottx services` (REQ-INP-007): the teletext streams of a transport
//! stream.

use neoteletext_app::input::{self, Options};
use neoteletext_app::{Failure, exit};
use neoteletext_formats::Item;
use std::io::Write;

/// `neottx services`: the teletext streams of a transport stream (REQ-INP-007).
pub fn services(options: &Options) -> Result<(), Failure> {
    let mut input = input::open(options)?;
    let mut reader = input::reader(options, false);
    let mut found = 0u32;
    let mut out = std::io::stdout().lock();
    input::pump(input.as_mut(), reader.as_mut(), &mut |item| match item {
        Item::TeletextEntry { program, pid, entry } => {
            found = found.saturating_add(1);
            let _ = writeln!(
                out,
                "program {program:>5}  PID {pid:#06x}  language {}  {:?}  page {}",
                String::from_utf8_lossy(&entry.language),
                entry.kind,
                String::from_utf8_lossy(&entry.page.ascii())
            );
        }
        Item::SubtitlePage(page) => {
            let _ = writeln!(out, "default subtitle page: {}", String::from_utf8_lossy(&page.ascii()));
        }
        _ => {}
    })?;
    if found == 0 {
        return Err((exit::NOT_FOUND, "no teletext descriptors found".into()));
    }
    Ok(())
}
