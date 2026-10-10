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
