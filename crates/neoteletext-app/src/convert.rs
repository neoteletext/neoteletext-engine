//! Converting between formats (REQ-INP-014, REQ-INP-018), and stored
//! pages back to page files for `neottx convert --to tti`: display pages with
//! their packets 26 to 28, enhancement pages coded by their page coding,
//! and each magazine's packets M/29 on its page mFF (REQ-INP-023).

use crate::input::{self, Options};
use crate::{Failure, exit};
use neoteletext_core::types::PageAddress;
use neoteletext_formats::Item;
use neoteletext_formats::page::PageFile;
use std::fs::File;
use std::io::Write;
use std::path::Path;

mod pages;

pub use pages::{enhancement_page_file, magazine_page_file, page_file, session_page_files};

/// Conversion targets (REQ-CLI-001 `convert`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// T42 packets.
    T42,
    /// TTI page files.
    Tti,
}

/// Converts the input to `to`, written to `output`; with a filter, only
/// that page (REQ-INP-014, REQ-INP-018).
pub fn convert(options: &Options, output: &Path, to: Target, filter: Option<PageAddress>) -> Result<(), Failure> {
    let file = File::create(output).map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
    match to {
        Target::T42 => {
            let filter = neoteletext_formats::write::PacketFilter {
                magazine: filter.map(|address| address.magazine),
                page: filter.map(|address| address.page),
            };
            let mut writer = neoteletext_formats::write::T42Writer::with_filter(std::io::BufWriter::new(file), filter);
            let mut input = input::open(options)?;
            let mut reader = input::reader(options, true);
            let mut failure = None;
            input::pump(input.as_mut(), reader.as_mut(), &mut |item| {
                if let Item::Packet { packet, .. } = item
                    && let Err(error) = writer.write_packet(&packet)
                {
                    failure = Some(error);
                }
            })?;
            if let Some(error) = failure {
                return Err((exit::INPUT, error.to_string()));
            }
            writer.into_inner().flush().map_err(|error| (exit::INPUT, error.to_string()))
        }
        Target::Tti => {
            let session = input::load(options)?;
            let pages = session_page_files(&session, filter);
            let references: Vec<&PageFile> = pages.iter().collect();
            let mut out = std::io::BufWriter::new(file);
            neoteletext_formats::write::write_tti(&references, &mut out)
                .map_err(|error| (exit::INPUT, error.to_string()))?;
            out.flush().map_err(|error| (exit::INPUT, error.to_string()))
        }
    }
}

#[cfg(test)]
mod tests;
