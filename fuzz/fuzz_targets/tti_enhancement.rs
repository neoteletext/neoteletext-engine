//! The enhancement lines of TTI files and the pages of Levels 2.5 and 3.5
//! (REQ-TST-042, REQ-INP-023): packed `OL,26` to `OL,29` lines, `PF`
//! lines and the rows of object, pattern and table pages, under each page
//! coding. Every page read is encoded and its packets decoded, and writing
//! the pages and reading them back must be stable.
//!
//! The first byte chooses the page and so its coding; the rest is lines,
//! each a selector byte that chooses the command and then its text.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_core::packet::decode;
use neoteletext_formats::Item;
use neoteletext_formats::encode::encode_page;
use neoteletext_formats::item::Reader;
use neoteletext_formats::page::PageFile;
use neoteletext_formats::tti::TtiReader;
use neoteletext_formats::write::write_tti;

/// Pages that select each row coding: an object page (010), a page with
/// the DRCS page function (000), which is not decoded (decision E.95), a
/// table without `PF` (011), a carrier of magazine packets and a
/// display page.
const HEADS: [&[u8]; 5] = [b"PN,1A000\nPF,3,2\n", b"PN,1B000\nPF,5,0\n", b"PN,1FE00\n", b"PN,1FF00\n", b"PN,10000\n"];
/// The commands a line may start with.
const COMMANDS: [&[u8]; 9] =
    [b"OL,26,", b"OL,27,", b"OL,28,", b"OL,29,", b"OL,1,", b"OL,3,", b"OL,24,", b"PF,", b"PN,1A001\n"];

fn read(text: &[u8]) -> Vec<Box<PageFile>> {
    let mut pages = Vec::new();
    let mut reader = Box::new(TtiReader::new());
    let mut collect = |item: Item<'_>| {
        if let Item::Page(page) = item {
            pages.push(Box::new(*page));
        }
    };
    reader.feed(text, &mut collect);
    reader.finish(&mut collect);
    pages
}

fn write(pages: &[Box<PageFile>]) -> Vec<u8> {
    let references: Vec<&PageFile> = pages.iter().map(|page| &**page).collect();
    let mut out = Vec::new();
    write_tti(&references, &mut out).expect("writing to memory");
    out
}

fuzz_target!(|data: &[u8]| {
    let [head, body @ ..] = data else { return };
    let mut text = HEADS[usize::from(*head) % HEADS.len()].to_vec();
    for line in body.split(|&byte| byte == b'\n') {
        let [selector, rest @ ..] = line else { continue };
        text.extend_from_slice(COMMANDS[usize::from(*selector) % COMMANDS.len()]);
        text.extend_from_slice(rest);
        text.push(b'\n');
    }
    let pages = read(&text);
    for page in &pages {
        encode_page(page, &mut |packet| {
            core::hint::black_box(decode(&packet).ok());
        });
    }
    let written = write(&pages);
    assert_eq!(write(&read(&written)), written, "writing what was read back changes nothing");
});
