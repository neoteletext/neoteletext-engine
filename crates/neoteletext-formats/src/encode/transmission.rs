//! The order in which a set of pages is sent (Annex B.7, REQ-INP-024).

use neoteletext_core::consts::{
    PACKET_MAGAZINE_ENHANCEMENT, PAGE_FUNCTION_BASIC, PAGE_FUNCTION_GPOP, PAGE_FUNCTION_MOT, PAGE_FUNCTION_POP,
    PAGE_MOT, PAGE_TIME_FILLING,
};

use crate::page::PageFile;

/// The order in which a set of pages is sent (Annex B.7, REQ-INP-024):
/// magazine packets, then the Magazine Organization Table, the object
/// pages, other data pages, and last the Level 1 pages.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum TransmissionClass {
    /// A page mFF that carries magazine packets M/29.
    MagazinePackets,
    /// Page mFE, or a page whose function is the Magazine Organization
    /// Table.
    OrganizationTable,
    /// A Public or Global Public Object Page.
    Objects,
    /// Any other page for further processing.
    OtherData,
    /// A Level 1 page.
    Display,
}

/// The class of a page in the order of Annex B.7.
#[must_use]
pub fn transmission_class(page: &PageFile) -> TransmissionClass {
    let carries_magazine_packets = page.address.page.0 == PAGE_TIME_FILLING
        && page.raw_packets.as_slice().iter().any(|raw| raw.number == PACKET_MAGAZINE_ENHANCEMENT);
    if carries_magazine_packets {
        return TransmissionClass::MagazinePackets;
    }
    if page.address.page.0 == PAGE_MOT {
        return TransmissionClass::OrganizationTable;
    }
    match page.function.map(|function| function.function) {
        None | Some(PAGE_FUNCTION_BASIC) => TransmissionClass::Display,
        Some(PAGE_FUNCTION_MOT) => TransmissionClass::OrganizationTable,
        Some(PAGE_FUNCTION_GPOP | PAGE_FUNCTION_POP) => TransmissionClass::Objects,
        Some(_) => TransmissionClass::OtherData,
    }
}
