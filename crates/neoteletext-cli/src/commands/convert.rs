//! `neottx convert` (REQ-INP-014, REQ-INP-018): between formats.

use super::{ConvertTarget, Selection, parse_selection};
use neoteletext_app::Failure;
use neoteletext_app::convert as conversion;
use neoteletext_app::input::Options;
use std::path::Path;

/// `neottx convert`.
pub fn convert(options: &Options, output: &Path, to: ConvertTarget, page: Option<&str>) -> Result<(), Failure> {
    let filter = match page {
        Some(text) => match parse_selection(text)? {
            Selection::Page(address, _) => Some(address),
            Selection::All => None,
        },
        None => None,
    };
    let target = match to {
        ConvertTarget::T42 => conversion::Target::T42,
        ConvertTarget::Tti => conversion::Target::Tti,
    };
    conversion::convert(options, output, target, filter)
}
