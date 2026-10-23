//! Reading one edit.tf or zxnet address into a page: the metadata, the
//! rows and the `key=value` fields.

use neoteletext_core::consts::{
    COLUMNS, LINK_NULL_PAGE, LINK_NULL_SUBCODE, NIBBLE_MASK, PACKET_ROW_25, ROWS, TRIPLET_DATA_MASK,
    TRIPLETS_PER_PACKET,
};
use neoteletext_core::packet::Triplet;
use neoteletext_core::types::{PageAddress, SubCode};

use crate::consts::{
    BASE64_BITS, EDITTF_CHARACTERS_24, EDITTF_CHARACTERS_25, EDITTF_LAST_NATIONAL, EDITTF_LINK_CHARACTERS,
    EDITTF_ROW_CHARACTERS, EDITTF_TRIPLET_CHARACTERS,
};
use crate::item::DiagnosticKind;
use crate::page::{EnhancementPacket, FASTEXT_LINKS, FastextLink, PageFile};

use super::base64::{base64_value, hex_value, unpack_codes};
use super::{FILLER, METADATA_SET_MASK, NO_ROW_24_FLAG};

/// Reads one address into `page`.
pub fn parse(text: &[u8], page: &mut PageFile) -> Result<(), DiagnosticKind> {
    page.clear();
    let text = text.trim_ascii();
    let text = text
        .iter()
        .rposition(|&byte| byte == b'#')
        .map_or(text, |index| text.get(index.saturating_add(1)..).unwrap_or(&[]));
    let mut parts = text.split(|&byte| byte == b':');
    let metadata = parts.next().unwrap_or(&[]);
    // Unreadable metadata counts as 0, as in edit.tf.
    let metadata = hex_value(metadata).and_then(|value| u8::try_from(value).ok()).unwrap_or(0);
    let data = parts.next().ok_or(DiagnosticKind::BadSignature)?;
    let rows = match data.len() {
        EDITTF_CHARACTERS_25 => ROWS,
        EDITTF_CHARACTERS_24 => ROWS.saturating_sub(1),
        other => return Err(DiagnosticKind::BadSize { size: u32::try_from(other).unwrap_or(u32::MAX) }),
    };
    let mut codes = [0u8; ROWS * COLUMNS];
    if !unpack_codes(data, &mut codes) {
        return Err(DiagnosticKind::BadBase64);
    }
    for (row, row_codes) in codes.chunks(COLUMNS).take(rows).enumerate() {
        page.set_row(row, row_codes);
    }
    let national = metadata & METADATA_SET_MASK;
    let mut status_given = false;
    let mut result = Ok(());
    for field in parts {
        let mut pair = field.splitn(2, |&byte| byte == b'=');
        let key = pair.next().unwrap_or(&[]);
        let value = pair.next().unwrap_or(&[]);
        status_given |= key == b"PS";
        if !field_value(page, key, value) {
            result = Err(DiagnosticKind::BadArgument {
                command: [key.first().copied().unwrap_or(0), key.get(1).copied().unwrap_or(0)],
            });
        }
    }
    if !status_given {
        if national <= EDITTF_LAST_NATIONAL {
            page.set_national_index(national);
        } else {
            result = result.and(Err(DiagnosticKind::UnmappedLanguage { code: national }));
        }
    }
    result
}

/// Applies one `key=value` field. Unknown keys are ignored. Returns false
/// when a known field could not be read.
fn field_value(page: &mut PageFile, key: &[u8], value: &[u8]) -> bool {
    match key {
        b"PN" => {
            let address = value
                .get(..3)
                .and_then(hex_value)
                .and_then(|mpp| u16::try_from(mpp).ok())
                .and_then(PageAddress::from_mpp);
            let subpage = match value.get(3..) {
                None | Some([]) => Some(0),
                Some(digits) => core::str::from_utf8(digits).ok().and_then(|digits| digits.parse::<u8>().ok()),
            };
            if let (Some(address), Some(subpage)) = (address, subpage) {
                page.address = address;
                page.subpage = subpage;
                true
            } else {
                false
            }
        }
        b"SC" => hex_value(value)
            .and_then(|value| u16::try_from(value).ok())
            .map(|value| page.subcode = SubCode::new(value))
            .is_some(),
        b"PS" => {
            hex_value(value).and_then(|value| u16::try_from(value).ok()).map(|value| page.status = value).is_some()
        }
        b"RE" => hex_value(value)
            .and_then(|value| u8::try_from(value).ok())
            .filter(|&value| value <= NIBBLE_MASK)
            .map(|value| page.region = Some(value))
            .is_some(),
        b"X270" => fastext_field(page, value),
        b"X26" => enhancement_field(page, value),
        b"X25" => {
            let mut codes = [0u8; COLUMNS];
            if value.len() != EDITTF_ROW_CHARACTERS || !unpack_codes(value, &mut codes) {
                return false;
            }
            page.set_row(usize::from(PACKET_ROW_25), &codes);
            true
        }
        _ => true,
    }
}

/// `X270`: six links of three hexadecimal page digits and four sub-code
/// digits, and an optional flag character for "display row 24".
fn fastext_field(page: &mut PageFile, value: &[u8]) -> bool {
    let (links, flag) = value.split_at(value.len().min(FASTEXT_LINKS * EDITTF_LINK_CHARACTERS));
    if links.len() != FASTEXT_LINKS * EDITTF_LINK_CHARACTERS {
        return false;
    }
    for (slot, link) in page.fastext.iter_mut().zip(links.chunks(EDITTF_LINK_CHARACTERS)) {
        let (Some(mpp), Some(sub)) = (link.get(..3).and_then(hex_value), link.get(3..).and_then(hex_value)) else {
            return false;
        };
        let mpp = u16::try_from(mpp).unwrap_or(0);
        let subcode = SubCode::new(u16::try_from(sub).unwrap_or(0));
        let null = (mpp as u8) == LINK_NULL_PAGE && subcode.value() == LINK_NULL_SUBCODE;
        *slot = if null { None } else { PageAddress::from_mpp(mpp).map(|address| FastextLink { address, subcode }) };
    }
    page.fastext_row_24 = flag.first() != Some(&NO_ROW_24_FLAG);
    true
}

/// `X26`: triplets of three base64 characters, most significant first,
/// filling packets of thirteen in designation order.
fn enhancement_field(page: &mut PageFile, value: &[u8]) -> bool {
    let (triplets, rest) = value.as_chunks::<EDITTF_TRIPLET_CHARACTERS>();
    if !rest.is_empty() {
        return false;
    }
    for (index, packet_triplets) in triplets.chunks(TRIPLETS_PER_PACKET).enumerate() {
        let Ok(designation) = u8::try_from(index) else { return false };
        let mut packet = EnhancementPacket { designation, triplets: [FILLER; TRIPLETS_PER_PACKET] };
        for (slot, characters) in packet.triplets.iter_mut().zip(packet_triplets) {
            let mut bits = 0u32;
            for &character in characters {
                let Some(value) = base64_value(character) else { return false };
                bits = (bits << BASE64_BITS) | value;
            }
            *slot = Triplet::from_value(bits & TRIPLET_DATA_MASK);
        }
        if !page.enhancements.push(packet) {
            return false;
        }
    }
    true
}
