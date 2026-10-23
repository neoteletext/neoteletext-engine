//! Writing a page as an edit.tf address (REQ-INP-016).

use neoteletext_core::consts::{
    COLUMNS, LINK_NULL_PAGE, LINK_NULL_SUBCODE, MAGAZINE_ZERO_MEANS, PACKET_ROW_25, ROWS, TRIPLETS_PER_PACKET,
};
use neoteletext_core::types::hex_digit;

use crate::consts::{BASE64URL_ALPHABET, EDITTF_LAST_NATIONAL};
use crate::page::PageFile;

use super::base64::{pack_codes, write_hex};
use super::{BASE64_MASK, FILLER, NO_ROW_24_FLAG, ROW_24_FLAG, TRIPLET_SHIFTS};

/// Writes a page as an edit.tf address (REQ-INP-016): metadata, the 25
/// rows, and the `PN`, `SC`, `PS`, `X270`, `X26`, `X25` and `RE` fields the
/// page needs. Reading the result reproduces the page.
pub fn write(page: &PageFile, out: &mut dyn FnMut(&[u8])) {
    let national = page.control().national_index();
    let metadata = if national <= EDITTF_LAST_NATIONAL { national } else { 0 };
    out(&[hex_digit(metadata), b':']);
    let mut codes = [0u8; ROWS * COLUMNS];
    for (row, target) in codes.chunks_mut(COLUMNS).enumerate() {
        if let Some(source) = page.row(row) {
            target.copy_from_slice(source);
        }
    }
    pack_codes(&codes, out);
    out(b":PN=");
    out(&page.address.ascii());
    if page.subpage != 0 {
        let tens = page.subpage.checked_div(10).and_then(|tens| tens.checked_rem(10)).unwrap_or(0);
        let units = page.subpage.checked_rem(10).unwrap_or(0);
        out(&[b'0'.saturating_add(tens), b'0'.saturating_add(units)]);
    }
    out(b":SC=");
    write_hex(u32::from(page.subcode.value()), 4, out);
    out(b":PS=");
    write_hex(u32::from(page.status), 4, out);
    if page.fastext.iter().any(Option::is_some) {
        out(b":X270=");
        for link in page.fastext {
            let (mpp, subcode) = link
                .map_or((u16::from(MAGAZINE_ZERO_MEANS) << 8 | u16::from(LINK_NULL_PAGE), LINK_NULL_SUBCODE), |link| {
                    (link.address.mpp(), link.subcode.value())
                });
            write_hex(u32::from(mpp), 3, out);
            write_hex(u32::from(subcode), 4, out);
        }
        out(&[if page.fastext_row_24 { ROW_24_FLAG } else { NO_ROW_24_FLAG }]);
    }
    if !page.enhancements.is_empty() {
        out(b":X26=");
        let last = page.enhancements.as_slice().iter().map(|packet| packet.designation).max().unwrap_or(0);
        for designation in 0..=last {
            let packet = page.enhancements.as_slice().iter().find(|packet| packet.designation == designation);
            let triplets = packet.map_or([FILLER; TRIPLETS_PER_PACKET], |packet| packet.triplets);
            for triplet in triplets {
                let value = triplet.value();
                // Three characters, the most significant six bits first.
                for shift in TRIPLET_SHIFTS {
                    let index = usize::try_from((value >> shift) & BASE64_MASK).unwrap_or(0);
                    out(BASE64URL_ALPHABET.get(index..=index).unwrap_or(&[]));
                }
            }
        }
    }
    if page.has_row(usize::from(PACKET_ROW_25)) {
        out(b":X25=");
        pack_codes(page.row(usize::from(PACKET_ROW_25)).map_or(&[][..], |row| &row[..]), out);
    }
    if let Some(region) = page.region {
        out(b":RE=");
        out(&[hex_digit(region)]);
    }
}
