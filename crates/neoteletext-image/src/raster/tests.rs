#![allow(clippy::indexing_slicing, clippy::unwrap_used, reason = "a test")]

use alloc::vec::Vec;

use neoteletext_core::consts::{ALPHA_YELLOW, COLUMNS, PACKET_BYTES, SPACE, STATUS_COLUMNS};
use neoteletext_core::types::PageAddress;
use neoteletext_formats::encode::encode_page;
use neoteletext_formats::page::PageFile;

use super::{grid_of_packets, picture, picture_size};

const PAGE: u16 = 0x302;
const STATUS: [u8; STATUS_COLUMNS] = *b"P302   ";
/// Bytes of the PNG signature and of the IHDR chunk's length and kind, after
/// which its width comes (PNG specification sections 5.2 and 11.2.2).
const WIDTH_AT: usize = 16;

fn address() -> PageAddress {
    PageAddress::from_mpp(PAGE).unwrap()
}

/// The packets of a page whose row 1 holds `text` in yellow.
fn packets(text: &[u8]) -> Vec<[u8; PACKET_BYTES]> {
    let mut page = PageFile::new();
    page.address = address();
    let mut row = [SPACE; COLUMNS];
    row[0] = ALPHA_YELLOW;
    row[1..=text.len()].copy_from_slice(text);
    page.set_row(1, &row);
    let mut packets = Vec::new();
    encode_page(&page, &mut |packet| packets.push(packet.bytes));
    packets
}

/// The grid of a page's packets holds what a receiver shows of it.
#[test]
fn a_pages_packets_make_the_grid_a_receiver_shows() {
    let glyph = |text: &[u8], column: usize| {
        alloc::format!("{:?}", grid_of_packets(&packets(text), address(), STATUS).unwrap().cells[1][column].glyph)
    };
    assert_eq!(glyph(b"HELLO", 1), glyph(b"HELP", 1), "both begin with H");
    assert_ne!(glyph(b"HELLO", 4), glyph(b"HELP", 4), "L and P differ");
}

/// Packets of another page make no grid: nothing draws a page it was not given.
#[test]
fn packets_of_another_page_make_no_grid() {
    assert!(grid_of_packets(&packets(b"HELLO"), PageAddress::from_mpp(0x303).unwrap(), STATUS).is_none());
    assert!(grid_of_packets(&[], address(), STATUS).is_none());
}

/// REQ-1728 item 4: the picture is a PNG of the size given, and it is of the
/// page: two pages' pictures differ.
#[test]
fn a_picture_is_a_png_of_the_page() {
    let picture_of = |text: &[u8]| picture(&grid_of_packets(&packets(text), address(), STATUS).unwrap()).unwrap();
    let png = picture_of(b"HELLO");
    assert_eq!(png[..8], [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let dimension = |at: usize| usize::try_from(u32::from_be_bytes(png[at..at + 4].try_into().unwrap())).unwrap();
    assert_eq!((dimension(WIDTH_AT), dimension(WIDTH_AT + 4)), picture_size());
    assert_ne!(png, picture_of(b"WORLD"));
}
