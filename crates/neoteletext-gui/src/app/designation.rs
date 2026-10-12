//! The page's designation in the font browser (REQ-BRW-011, decision E.75
//! item 5): how the decoder chose the page's character sets, step by step,
//! drawn as the browser's own text, outside the page.

use eframe::egui::{RichText, Ui};
use neoteletext_app::pipe::PageDesignation;
use neoteletext_browser::charsets::{group_name, set_label};
use neoteletext_core::consts::DESIGNATION_GROUP_SHIFT;
use neoteletext_core::level1::{DesignationSource, NationalSource};

/// Bits of the national option index.
const NATIONAL_BITS: usize = DESIGNATION_GROUP_SHIFT as usize;
/// Bits of the group.
const GROUP_BITS: usize = 4;

const fn source_text(source: DesignationSource) -> &'static str {
    match source {
        DesignationSource::PageX28Format1 => "this page's packet X/28/0 Format 1",
        DesignationSource::PageX28Format4 => "this page's packet X/28/4",
        DesignationSource::MagazineM29Format0 => "its magazine's packet M/29/0",
        DesignationSource::MagazineM29Format4 => "its magazine's packet M/29/4",
        DesignationSource::Receiver => "the receiver's setting: the page sends none (Level 1)",
    }
}

const fn national_text(source: NationalSource) -> &'static str {
    match source {
        NationalSource::Header => "C12 to C14 of the page header",
        NationalSource::Designation => "bits 8 to 10 of the designation (Levels 2.5 and 3.5)",
        NationalSource::Fixed => "the receiver's fixed setting",
    }
}

/// The section, for the page's designation.
pub fn section(ui: &mut Ui, page: u16, d: &PageDesignation) {
    ui.label(RichText::new(format!("P{page:03X}: how its character sets were chosen")).strong());
    ui.label(format!("1  National option bits (X/0, C12 C13 C14): {:0NATIONAL_BITS$b}", d.header_national));
    ui.label(format!("2  Group {:0GROUP_BITS$b}, {}", d.group, group_name(d.group)));
    ui.label(RichText::new(format!("     from {}", source_text(d.source))).weak());
    let option = format!("{:0NATIONAL_BITS$b}", d.national);
    ui.label(format!("3  Table 32 entry {:0GROUP_BITS$b} {option}", d.group));
    if d.fallback {
        ui.label(RichText::new("     reserved: the fallback of REQ-CHR-003 chose").weak());
    }
    ui.label(format!("     → {} + {}", set_label(usize::from(d.g0)), set_label(usize::from(d.g2))));
    ui.label(RichText::new(format!("     national option from {}", national_text(d.national_source))).weak());
    let second = d.second.map_or("none", |second| set_label(usize::from(second)));
    ui.label(format!("4  Second G0 set (Table 33): {second}"));
    ui.label(format!("5  ESC in {} rows; X/26 designations applied: {}", d.esc_rows, d.x26_designations));
    let mut uses = vec![set_label(usize::from(d.g0)), set_label(usize::from(d.g2))];
    if let Some(second) = d.second {
        uses.push(set_label(usize::from(second)));
    }
    ui.label(RichText::new(format!("Sets this page uses: {}", uses.join(" · "))).strong());
}
