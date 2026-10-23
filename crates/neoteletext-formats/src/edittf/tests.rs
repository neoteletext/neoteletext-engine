//! edit.tf address tests: a known empty page, the 24-row form and the
//! write-read round trip (REQ-TST-020).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]

use super::base64::pack_codes;
use super::*;
use crate::consts::{EDITTF_CHARACTERS_24, EDITTF_CHARACTERS_25};
use crate::item::DiagnosticKind;
use crate::page::{EnhancementPacket, FastextLink, PageFile};
use crate::testing::{diagnostics, pages, same_in_any_chunks};
use neoteletext_core::consts::{COLUMNS, MODE_TERMINATION, ROW_ADDRESS_LAST, ROWS, SPACE, TRIPLETS_PER_PACKET};
use neoteletext_core::types::{PageAddress, SubCode};
use std::vec::Vec;

pub(crate) fn written(page: &PageFile) -> Vec<u8> {
    let mut out = Vec::new();
    write(page, &mut |bytes| out.extend_from_slice(bytes));
    out
}

#[test]
fn an_empty_page_is_the_familiar_address() {
    let mut page = PageFile::new();
    for row in 0..ROWS {
        page.set_row(row, &[SPACE; COLUMNS]);
    }
    page.address = PageAddress::from_mpp(0x100).unwrap();
    let text = written(&page);
    // Spaces, 0100000 repeated, give the base64 cycle Q I E C B A g.
    assert!(text.starts_with(b"0:QIECBAgQIECBAgQIEC"), "{}", core::str::from_utf8(&text).unwrap());
    let data_end = text.iter().skip(2).position(|&byte| byte == b':').unwrap() + 2;
    assert_eq!(data_end - 2, EDITTF_CHARACTERS_25);
}

#[test]
fn reads_the_24_row_form_after_a_web_address() {
    let mut page = PageFile::new();
    page.set_row(0, b"Header");
    page.set_row(23, b"Last row");
    let mut codes = Vec::new();
    for row in 0..24 {
        codes.extend_from_slice(page.row(row).unwrap());
    }
    let mut text = b"https://edit.tf/#3:".to_vec();
    pack_codes(&codes, &mut |bytes| text.extend_from_slice(bytes));
    assert_eq!(text.len() - 19, EDITTF_CHARACTERS_24);
    text.extend_from_slice(b":unknown=1\n");
    let items = same_in_any_chunks(EditTfReader::new, &text);
    assert_eq!(diagnostics(&items), []);
    let read = pages(&items)[0];
    assert_eq!(&read.row(23).unwrap()[..8], b"Last row");
    assert!(!read.has_row(24));
    // Metadata 3 is Italian, national option index 3.
    assert_eq!(read.control().national_index(), 3);
}

#[test]
fn write_then_read_reproduces_the_page() {
    let mut page = PageFile::new();
    for row in 0..ROWS {
        let mut codes = [0u8; COLUMNS];
        for (column, code) in codes.iter_mut().enumerate() {
            *code = ((row * 7 + column * 3) % 128) as u8;
        }
        page.set_row(row, &codes);
    }
    page.set_row(25, b"Row twenty-five");
    page.address = PageAddress::from_mpp(0x3A5).unwrap();
    page.subpage = 12;
    page.subcode = SubCode::new(0x0012);
    page.status = 0x8000 | 0x0002;
    page.set_national_index(2);
    page.region = Some(4);
    page.fastext[0] = Some(FastextLink { address: PageAddress::from_mpp(0x101).unwrap(), subcode: SubCode::ANY });
    page.fastext[5] = Some(FastextLink { address: PageAddress::from_mpp(0x800).unwrap(), subcode: SubCode::new(1) });
    page.fastext_row_24 = false;
    let mut triplets = [Triplet { address: ROW_ADDRESS_LAST, mode: MODE_TERMINATION, data: 0 }; TRIPLETS_PER_PACKET];
    triplets[0] = Triplet { address: 41, mode: 4, data: 0 };
    let _ = page.enhancements.push(EnhancementPacket { designation: 0, triplets });
    let text = written(&page);
    let items = same_in_any_chunks(EditTfReader::new, &text);
    assert_eq!(diagnostics(&items), []);
    assert_eq!(pages(&items).as_slice(), core::slice::from_ref(&page));
}

#[test]
fn reports_bad_data() {
    let items = same_in_any_chunks(EditTfReader::new, b"0:QIEC\n0:\n");
    let found = diagnostics(&items);
    assert_eq!(found[0], Diagnostic::at_line(0, 1, DiagnosticKind::BadSize { size: 4 }));
    assert_eq!(found[1].line, Some(2));
    let mut text = b"0:".to_vec();
    text.extend(std::iter::repeat_n(b'*', EDITTF_CHARACTERS_25));
    let items = same_in_any_chunks(EditTfReader::new, &text);
    assert_eq!(diagnostics(&items)[0].kind, DiagnosticKind::BadBase64);
}
