//! Real broadcast captures (REQ-TST-022): Dan Farrimond's recovered T42
//! recordings, read from `$NEOTELETEXT_CORPUS` or `~/devel/jellytext/dan/`.
//! The files are never copied into the repository. Without them the tests
//! print a note and pass.
//!
//! The page checksum test settles the byte and bit order of the X/27/0
//! checksum (REQ-ECC-007): for every received page that carries X/27/0, the
//! checksum of the page as received is compared with bytes 44 and 45 under
//! each candidate order.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::print_stdout)]

use std::path::{Path, PathBuf};

use neoteletext_core::consts::{DATA_BYTES, HEADER_DISPLAY_BYTES, MAGAZINES, PACKET_ROW_25, SPACE};
use neoteletext_core::ecc::{PageChecksum, reverse_bits, with_odd_parity};
use neoteletext_core::packet::{Decoded, Packet, decode};
use neoteletext_formats::encode::{CHECKSUM_HEADER_BYTES, checksum_word, page_checksum};
use neoteletext_formats::item::{Item, Reader};
use neoteletext_formats::t42::{T42Reader, T42Variant};

fn corpus_directory() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("NEOTELETEXT_CORPUS") {
        return Some(PathBuf::from(path));
    }
    let home = std::env::var("HOME").ok()?;
    let path = Path::new(&home).join("devel/jellytext/dan");
    path.is_dir().then_some(path)
}

/// Every `.t42` file under the corpus directory, up to six levels deep (the
/// teletext-restoration layout is year/date/channel/restored/page).
fn t42_files() -> Vec<PathBuf> {
    let Some(root) = corpus_directory() else { return Vec::new() };
    let mut files = Vec::new();
    let mut directories = vec![root];
    for _ in 0..6 {
        let mut next = Vec::new();
        for directory in directories {
            let Ok(entries) = std::fs::read_dir(&directory) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    next.push(path);
                } else if path.extension().is_some_and(|extension| extension == "t42") {
                    files.push(path);
                }
            }
        }
        directories = next;
    }
    files.sort();
    files
}

fn read_packets(path: &Path) -> Vec<Packet> {
    let data = std::fs::read(path).unwrap();
    let mut reader = T42Reader::new(T42Variant::Plain, None);
    let mut packets = Vec::new();
    let mut out = |item: Item<'_>| {
        if let Item::Packet { packet, .. } = item {
            packets.push(packet);
        }
    };
    for chunk in data.chunks(65_536) {
        reader.feed(chunk, &mut out);
    }
    reader.finish(&mut out);
    packets
}

/// A page being received in one magazine.
#[derive(Clone)]
struct Received {
    display: [u8; HEADER_DISPLAY_BYTES],
    rows: [Option<[u8; DATA_BYTES]>; PACKET_ROW_25 as usize],
    transmitted: Option<[u8; 2]>,
}

/// The candidate orders of the checksum, each a function of the register
/// computed with bits applied b8 first (`forward`) or b1 first
/// (`backward`).
const ORDERS: [&str; 8] = [
    "b8 first, big-endian",
    "b8 first, little-endian",
    "b8 first, big-endian, bytes bit-reversed",
    "b8 first, little-endian, bytes bit-reversed",
    "b1 first, big-endian",
    "b1 first, little-endian",
    "b1 first, big-endian, bytes bit-reversed",
    "b1 first, little-endian, bytes bit-reversed",
];

fn candidates(forward: u16, backward: u16) -> [[u8; 2]; 8] {
    let reversed = |bytes: [u8; 2]| bytes.map(reverse_bits);
    [
        forward.to_be_bytes(),
        forward.to_le_bytes(),
        reversed(forward.to_be_bytes()),
        reversed(forward.to_le_bytes()),
        backward.to_be_bytes(),
        backward.to_le_bytes(),
        reversed(backward.to_be_bytes()),
        reversed(backward.to_le_bytes()),
    ]
}

/// The register with each byte applied b1 first.
fn backward_checksum(display: &[u8; HEADER_DISPLAY_BYTES], rows: &[Option<[u8; DATA_BYTES]>]) -> u16 {
    let mut checksum = PageChecksum::new();
    for &byte in &display[..CHECKSUM_HEADER_BYTES] {
        checksum.update(reverse_bits(byte));
    }
    for row in rows {
        let bytes = row.unwrap_or([with_odd_parity(SPACE); DATA_BYTES]);
        for byte in bytes {
            checksum.update(reverse_bits(byte));
        }
    }
    checksum.value()
}

#[derive(Default)]
struct Tally {
    with_x27: usize,
    pages: usize,
    matches: [usize; 8],
    matches_without_row_25: [usize; 8],
    encoder_matches: usize,
}

fn finish_page(page: &Received, tally: &mut Tally) {
    let Some(transmitted) = page.transmitted else { return };
    tally.with_x27 += 1;
    // Recovered captures carry 00 00 where no checksum was kept; a real
    // checksum is 0 only once in 65536 pages.
    if transmitted == [0, 0] {
        return;
    }
    tally.pages += 1;
    let forward = page_checksum(&page.display, &page.rows);
    let backward = backward_checksum(&page.display, &page.rows);
    for (count, candidate) in tally.matches.iter_mut().zip(candidates(forward, backward)) {
        *count += usize::from(candidate == transmitted);
    }
    let mut without = page.rows;
    without[usize::from(PACKET_ROW_25) - 1] = None;
    let forward = page_checksum(&page.display, &without);
    let backward = backward_checksum(&page.display, &without);
    for (count, candidate) in tally.matches_without_row_25.iter_mut().zip(candidates(forward, backward)) {
        *count += usize::from(candidate == transmitted);
    }
    let encoded = checksum_word(page_checksum(&page.display, &page.rows)).to_be_bytes();
    tally.encoder_matches += usize::from(encoded == transmitted);
}

fn tally_file(packets: &[Packet], tally: &mut Tally) {
    let mut current: [Option<Received>; MAGAZINES] = Default::default();
    for packet in packets {
        match decode(packet) {
            Ok(Decoded::Header(header)) => {
                let finished: Vec<usize> = if header.serial() == Some(true) {
                    (0..MAGAZINES).collect()
                } else {
                    vec![header.magazine.index()]
                };
                for index in finished {
                    if let Some(page) = current[index].take() {
                        finish_page(&page, tally);
                    }
                }
                if header.is_complete() && header.page.is_some_and(|page| !page.is_time_filling()) {
                    current[header.magazine.index()] = Some(Received {
                        display: header.display,
                        rows: [None; PACKET_ROW_25 as usize],
                        transmitted: None,
                    });
                }
            }
            Ok(Decoded::Row(row)) => {
                if let Some(page) = current[row.magazine.index()].as_mut() {
                    page.rows[usize::from(row.row) - 1] = Some(row.bytes);
                }
            }
            Ok(Decoded::Links(links)) => {
                if let Some(page) = current[links.magazine.index()].as_mut() {
                    page.transmitted = Some(links.checksum_bytes);
                }
            }
            _ => {}
        }
    }
    for page in current.iter().flatten() {
        finish_page(page, tally);
    }
}

#[test]
fn corpus_packets_and_page_checksum_order() {
    let files = t42_files();
    if files.is_empty() {
        println!("note: no corpus found (set NEOTELETEXT_CORPUS); skipping the real-data test");
        return;
    }
    let mut total = Tally::default();
    for path in &files {
        let packets = read_packets(path);
        let valid = packets.iter().filter(|packet| neoteletext_core::packet::decode_address(packet).is_ok()).count();
        let mut tally = Tally::default();
        tally_file(&packets, &mut tally);
        println!(
            "{}: {} packets, {} with valid addresses; {} pages with X/27/0, {} with a checksum; matches {:?}; without X/25 {:?}",
            path.file_name().unwrap().to_string_lossy(),
            packets.len(),
            valid,
            tally.with_x27,
            tally.pages,
            tally.matches,
            tally.matches_without_row_25
        );
        assert!(valid > 0, "{}", path.display());
        total.pages += tally.pages;
        total.with_x27 += tally.with_x27;
        total.encoder_matches += tally.encoder_matches;
        for index in 0..ORDERS.len() {
            total.matches[index] += tally.matches[index];
            total.matches_without_row_25[index] += tally.matches_without_row_25[index];
        }
    }
    println!("pages with X/27/0: {}; with a non-zero checksum: {}", total.with_x27, total.pages);
    for (index, order) in ORDERS.iter().enumerate() {
        println!("  {order}: {} match ({} without X/25)", total.matches[index], total.matches_without_row_25[index]);
    }
    println!("  the encoder's order: {} match", total.encoder_matches);
    // The encoder must use the order the broadcasts use: it has to match on
    // most pages that carry a checksum.
    if total.pages == 0 {
        println!("note: no page in the corpus carries a checksum; the order is not tested");
        return;
    }
    assert!(
        total.encoder_matches * 2 > total.pages,
        "encoder order matched {} of {}",
        total.encoder_matches,
        total.pages
    );
}

/// The national option index of a page in a transport stream recording
/// from the corpus, read through the transport stream reader and the
/// acquisition decoder. `None` (with a note) when the file is absent.
fn national_index_in_recording(name: &str, mpp: u16) -> Option<u8> {
    use neoteletext_core::acq::{Decoder, StoredPage};
    use neoteletext_core::config::Config;
    use neoteletext_core::types::{Instant, PageAddress, SubCode};
    use neoteletext_formats::detect::{AutoConfig, AutoReader};

    let path = corpus_directory().map(|directory| directory.join(name)).filter(|path| path.is_file());
    let Some(path) = path else {
        eprintln!("SKIPPED: corpus file {name} not found; set NEOTELETEXT_CORPUS");
        return None;
    };
    let data = std::fs::read(path).unwrap();
    let mut slots = vec![StoredPage::EMPTY; 64];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut reader = Box::new(AutoReader::new(AutoConfig::default()));
    let mut count = 0u64;
    let mut out = |item: Item<'_>| {
        if let Item::Packet { packet, meta } = item {
            decoder.push(&packet, &meta, Instant(count * 64), &mut |_| {});
            count += 1;
        }
    };
    for chunk in data.chunks(65_536) {
        reader.feed(chunk, &mut out);
    }
    reader.finish(&mut out);
    let address = PageAddress::from_mpp(mpp).unwrap();
    let page = decoder.store().get(address, SubCode::ANY);
    assert!(page.is_some(), "page {mpp:x} not received from {name}");
    page.map(|page| page.control.national_index())
}

/// REQ-TST-012: the order of decision E.21, pinned by two off-air
/// recordings whose language is known from their descriptors. The reversed
/// order would give German and Czech.
#[test]
fn national_option_order_matches_broadcast_languages() {
    if let Some(index) = national_index_in_recording("ffmpeg-France2.m2ts", 0x888) {
        assert_eq!(index, 4, "France 2 subtitles are French (Table 32 index 4)");
    }
    if let Some(index) = national_index_in_recording("ffmpeg-teletextsubtitles.ts", 0x777) {
        assert_eq!(index, 3, "Italian subtitles (Table 32 index 3)");
    }
}
