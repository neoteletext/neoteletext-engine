//! Stored pages back to page files, for `ttx convert --to tti`.

use tellytxt_core::acq::StoredPage;
use tellytxt_core::config::ParityPolicy;
use tellytxt_core::consts::{CODE_MASK_7BIT, COLUMNS, HEADER_DISPLAY_COLUMN, SPACE, STORED_ROWS};
use tellytxt_formats::page::{EnhancementPacket, FastextLink, PageFile};

/// Fastext keys carried by a stored X/27/0: links 0 to 5.
const LINKS: usize = 6;

/// A page file from a stored page: rows as 7-bit codes (parity failures as
/// spaces, decision E.8), control bits, X/26 packets and X/27/0 links.
#[must_use]
pub fn page_file(page: &StoredPage) -> PageFile {
    let mut file = PageFile { address: page.key.address, subcode: page.key.subcode, ..PageFile::default() };
    file.set_control(page.control);
    for row in 0..STORED_ROWS {
        if row > 0 && !page.has_row(row as u8) {
            continue;
        }
        let mut codes = [SPACE; COLUMNS];
        let first = if row == 0 { HEADER_DISPLAY_COLUMN } else { 0 };
        for (column, code) in codes.iter_mut().enumerate().skip(first) {
            let (byte, passed) = page.byte(row, column, ParityPolicy::KeepLastGood);
            *code = if passed { byte & CODE_MASK_7BIT } else { SPACE };
        }
        file.set_row(row, &codes);
        file.rows_present |= 1 << row;
    }
    for (designation, triplets) in page.enhancements.iter().enumerate() {
        if page.enhancements_present & (1 << designation) != 0 {
            let mut packet = EnhancementPacket { designation: designation as u8, triplets: Default::default() };
            for (slot, triplet) in packet.triplets.iter_mut().zip(triplets) {
                *slot = triplet.unwrap_or_default();
            }
            let _ = file.enhancements.push(packet);
        }
    }
    if let Some(links) = page.links {
        for (slot, link) in file.fastext.iter_mut().zip(links.links.iter()).take(LINKS) {
            *slot = link.and_then(|link| {
                let mpp = (u16::from(link.magazine.number()) << 8) | u16::from(link.page.0);
                tellytxt_core::types::PageAddress::from_mpp(mpp)
                    .map(|address| FastextLink { address, subcode: link.subcode })
            });
        }
        file.fastext_row_24 = links.display_row_24;
    }
    file
}
