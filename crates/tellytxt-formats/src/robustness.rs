//! Property tests of REQ-GEN-008 and REQ-INP-001: arbitrary input never
//! panics or overflows (tests build with overflow checks), and the items
//! do not depend on how the input is split.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use proptest::prelude::*;
use std::vec::Vec;

use crate::detect::{AutoConfig, AutoReader, Format};
use crate::dump::DumpReader;
use crate::edittf::EditTfReader;
use crate::encode::encode_page;
use crate::ep1::Ep1Reader;
use crate::item::Reader;
use crate::pes::PesReader;
use crate::sliced::{ByteOrder, SlicedReader};
use crate::t42::{T42Reader, T42Variant};
use crate::testing::{Owned, run};
use crate::transport::{TsConfig, TsReader};
use crate::tti::TtiReader;

/// Prefixes that lead each reader past its first checks.
const PREFIXES: [&[u8]; 6] = [
    b"",
    b"PN,10000\nOL,1,",
    b"0:QIECBAgQ",
    &[0x47, 0x40, 0x00, 0x10, 0x00, 0x00, 0xB0, 0x0D],
    &[0x00, 0x00, 0x01, 0xBD, 0x00, 0x40, 0x80, 0x80, 0x05],
    &[0xFE, 0x01, 0x09, 0xCA, 0x2C, 0x00, 0xC2, 0x00],
];

fn readers() -> Vec<std::boxed::Box<dyn Fn() -> std::boxed::Box<dyn Reader>>> {
    std::vec![
        std::boxed::Box::new(
            || std::boxed::Box::new(T42Reader::new(T42Variant::RunIn, None)) as std::boxed::Box<dyn Reader>
        ),
        std::boxed::Box::new(|| std::boxed::Box::new(TsReader::new(TsConfig::default()))),
        std::boxed::Box::new(|| std::boxed::Box::new(TsReader::new(TsConfig { pid: Some(0), ..TsConfig::default() }))),
        std::boxed::Box::new(|| std::boxed::Box::new(PesReader::new(None))),
        std::boxed::Box::new(|| std::boxed::Box::new(SlicedReader::new(ByteOrder::Big, None))),
        std::boxed::Box::new(|| std::boxed::Box::new(TtiReader::new())),
        std::boxed::Box::new(|| std::boxed::Box::new(Ep1Reader::new())),
        std::boxed::Box::new(|| std::boxed::Box::new(EditTfReader::new())),
        std::boxed::Box::new(|| std::boxed::Box::new(DumpReader::new())),
        std::boxed::Box::new(|| std::boxed::Box::new(AutoReader::new(AutoConfig {
            encode_pages: true,
            ..AutoConfig::default()
        }))),
        std::boxed::Box::new(|| {
            let config = AutoConfig { format: Some(Format::Tti), encode_pages: true, ..AutoConfig::default() };
            std::boxed::Box::new(AutoReader::new(config))
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    #[test]
    fn any_input_in_any_split(prefix in 0..PREFIXES.len(), body in proptest::collection::vec(any::<u8>(), 0..3000), chunk in 1usize..400) {
        let mut input = PREFIXES[prefix].to_vec();
        input.extend_from_slice(&body);
        for make in readers() {
            let whole = run(&mut *make(), &input, input.len());
            let split = run(&mut *make(), &input, chunk);
            prop_assert_eq!(&whole, &split);
            for item in &whole {
                if let Owned::Page(page) = item {
                    encode_page(page, &mut |_| {});
                }
            }
        }
    }

    #[test]
    fn any_tti_lines(lines in proptest::collection::vec(proptest::collection::vec(prop_oneof![Just(b','), Just(b':'), Just(0x1B), 0x20u8..0x7F, any::<u8>()], 0..60), 0..40)) {
        let mut input = b"PN,10000\n".to_vec();
        let commands: [&[u8]; 14] = [b"OL,", b"FD,", b"OPTION,", b"LN,", b"FL,", b"FS,", b"FP,", b"PS,", b"SC,", b"PF,", b"RE,", b"CK,", b"OL,26,", b"OL,27,"];
        for (index, line) in lines.iter().enumerate() {
            input.extend_from_slice(commands[index % commands.len()]);
            input.extend_from_slice(line);
            input.push(b'\n');
        }
        let whole = run(&mut TtiReader::new(), &input, input.len());
        prop_assert_eq!(&whole, &run(&mut TtiReader::new(), &input, 1));
        for item in &whole {
            if let Owned::Page(page) = item {
                let mut check = Vec::new();
                crate::check::check_page(page, &crate::check::CheckOptions::default(), &mut |finding| check.push(finding));
                encode_page(page, &mut |_| {});
            }
        }
    }
}

/// The fuzzing target `tti_enhancement` (REQ-TST-042) as a property test,
/// for machines without `cargo fuzz`: the same page heads, commands and
/// checks, over a deterministic pseudo-random corpus.
#[cfg(feature = "std")]
mod enhancement_lines {
    use super::*;
    use std::boxed::Box;
    use tellytxt_core::packet::decode;

    use crate::Item;
    use crate::page::PageFile;
    use crate::write::write_tti;

    /// As in `fuzz/fuzz_targets/tti_enhancement.rs`.
    const HEADS: [&[u8]; 5] =
        [b"PN,1A000\nPF,3,2\n", b"PN,1B000\nPF,5,0\n", b"PN,1FE00\n", b"PN,1FF00\n", b"PN,10000\n"];
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
        write_tti(&references, &mut out).unwrap();
        out
    }

    /// The body of the fuzzing target.
    fn run(data: &[u8]) {
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
                let _ = decode(&packet);
            });
        }
        let written = write(&pages);
        assert_eq!(write(&read(&written)), written, "{}", std::string::String::from_utf8_lossy(&written));
    }

    /// Characters a packed line or a row is made of, with the bytes that
    /// break lines and escapes.
    fn character() -> impl Strategy<Value = u8> {
        prop_oneof![0x40u8..0x80, Just(b'\n'), Just(0x1B), Just(b','), any::<u8>()]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// REQ-TST-042 and REQ-INP-023: any enhancement lines are read,
        /// encoded and written back stably, without a panic.
        #[test]
        fn any_enhancement_lines(data in proptest::collection::vec(character(), 0..600)) {
            run(&data);
        }
    }

    /// REQ-TST-042: the corpus of the authored pages, cut and damaged.
    #[test]
    fn authored_pages_cut_and_damaged() {
        let text = crate::author::streams::stream().unwrap().tti().unwrap();
        let mut seed = 0x2545_F491u32;
        for _ in 0..64 {
            // A fixed linear congruential sequence (Numerical Recipes), so
            // the corpus is the same on every run.
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let start = seed as usize % text.len();
            let length = (seed >> 8) as usize % 2000;
            let mut piece = std::vec![seed.to_le_bytes()[3]];
            piece.extend_from_slice(&text[start..(start + length).min(text.len())]);
            if let Some(byte) = piece.get_mut(length / 2) {
                *byte ^= seed.to_le_bytes()[1];
            }
            run(&piece);
        }
    }
}

/// Memory of each reader is fixed by its type (REQ-INP-001); the sizes are
/// printed for the resource documentation of REQ-GEN-009.
#[test]
fn reader_sizes() {
    use core::mem::size_of;
    let sizes = [
        ("T42Reader", size_of::<T42Reader>()),
        ("TsReader", size_of::<TsReader>()),
        ("PesReader", size_of::<PesReader>()),
        ("SlicedReader", size_of::<SlicedReader>()),
        ("TtiReader", size_of::<TtiReader>()),
        ("Ep1Reader", size_of::<Ep1Reader>()),
        ("EditTfReader", size_of::<EditTfReader>()),
        ("DumpReader", size_of::<DumpReader>()),
        ("AutoReader", size_of::<AutoReader>()),
        ("PageFile", size_of::<crate::page::PageFile>()),
    ];
    for (name, size) in sizes {
        std::println!("{name}: {size} bytes");
        assert!(size < 64 * 1024, "{name}");
    }
}
