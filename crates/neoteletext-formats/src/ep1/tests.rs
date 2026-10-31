//! EP1 reader tests, with files built here in the documented layout.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]

use super::*;
use crate::testing::{diagnostics, pages, same_in_any_chunks};
use neoteletext_core::consts::{MODE_G0_NO_DIACRITIC, MODE_TERMINATION};
use std::vec::Vec;

/// A stored enhancement packet: designation and 13 × (address, mode, data).
pub(crate) type StoredPacket = (u8, [(u8, u8, u8); 13]);

/// An EP1 file: 25 rows of text and, optionally, enhancement packets.
pub(crate) fn ep1_file(language: u8, packets: &[StoredPacket]) -> Vec<u8> {
    let mut file = std::vec![0xFE, 0x01, language];
    if packets.is_empty() {
        file.extend_from_slice(&[0x00, 0x00, 0x00]);
    } else {
        let length = (packets.len() * EP1_PACKET_BYTES) as u16;
        let offset = length + EP1_ENHANCEMENT_HEADER_BYTES as u16;
        file.push(EP1_ENHANCED);
        file.extend_from_slice(&offset.to_le_bytes());
        file.extend_from_slice(&EP1_BLOCK_MARKER);
        file.extend_from_slice(&length.to_le_bytes());
        for (designation, triplets) in packets {
            file.push(*designation);
            for (address, mode, data) in triplets {
                file.extend_from_slice(&[*address, *mode, *data]);
            }
        }
    }
    for row in 0..25u8 {
        let mut text = [b' '; 40];
        text[0] = 0x80 | b'A'.wrapping_add(row);
        file.extend_from_slice(&text);
    }
    file.extend_from_slice(&[0, 0]);
    file
}

#[test]
fn reads_a_plain_file_of_1008_bytes() {
    let file = ep1_file(0x0D, &[]);
    assert_eq!(file.len(), 1008);
    let items = same_in_any_chunks(Ep1Reader::new, &file);
    assert_eq!(diagnostics(&items), []);
    let page = pages(&items)[0].clone();
    assert_eq!(page.control().national_index(), 1);
    assert_eq!(page.region, None);
    assert_eq!(page.row(0).unwrap()[0], b'A');
    assert_eq!(page.row(24).unwrap()[0], b'Y');
    assert!(page.has_row(24));
}

#[test]
fn reads_enhancements_and_the_stored_terminator() {
    let mut triplets = [(EP1_TERMINATOR_ADDRESS, MODE_TERMINATION, 0); 13];
    triplets[0] = (41, 4, 0);
    triplets[1] = (2, MODE_G0_NO_DIACRITIC, b'q');
    let file = ep1_file(0x14, &[(0, triplets), (1, triplets)]);
    let items = same_in_any_chunks(Ep1Reader::new, &file);
    let page = pages(&items)[0].clone();
    assert_eq!(page.enhancements.len(), 2);
    let stored = page.enhancements.as_slice()[1];
    assert_eq!(stored.designation, 1);
    assert_eq!(stored.triplets[1], Triplet { address: 2, mode: MODE_G0_NO_DIACRITIC, data: b'q' });
    assert_eq!(stored.triplets[2], Triplet { address: ROW_ADDRESS_LAST, mode: MODE_TERMINATION, data: 0 });
    // Polish is group 1, national option 0.
    assert_eq!(page.region, Some(1));
    assert_eq!(page.control().national_index(), 0);
}

#[test]
fn reports_bad_files() {
    let mut file = ep1_file(0x09, &[]);
    file[0] = 0;
    let items = same_in_any_chunks(Ep1Reader::new, &file);
    assert_eq!(diagnostics(&items)[0].kind, DiagnosticKind::BadSignature);
    let file = ep1_file(0x42, &[]);
    let items = same_in_any_chunks(Ep1Reader::new, &file[..900]);
    let found = diagnostics(&items);
    assert_eq!(found[0].kind, DiagnosticKind::UnmappedLanguage { code: 0x42 });
    assert_eq!(found[1].kind, DiagnosticKind::BadSize { size: 900 });
    assert_eq!(pages(&items), []);
}
