//! Each capability of decisions E.71 and E.105 compiled out (REQ-GEN-020): its page
//! data is ignored as the decision says, never an error. Each test runs
//! only in the builds that leave its capability out; the builds with it
//! test what it does in `level25.rs` and `objects.rs`.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]
// The helpers serve only the tests of capabilities compiled out, so in a
// build with every capability they are unused.
#![cfg_attr(
    all(feature = "objects", feature = "level35-32colors", feature = "additional-flash-functions"),
    allow(dead_code, unused_imports, reason = "every capability is compiled in, so no test here runs")
)]

mod common;

use common::{display_page, local_data, set_position};
use neoteletext_core::packet::{Packet, TripletFunction};

/// Decision E.105: the default build is Levels 1.0 and 1.5, the non-spacing
/// attributes and the Colour Map of Levels 2.5 and 3.5, and nothing more of
/// Levels 2.5 and 3.5; read from the manifest, whatever this build's
/// features.
#[test]
fn the_default_features_are_decision_e105() {
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    let start = manifest.find("\ndefault = [").unwrap();
    let list = &manifest[start..start + manifest[start..].find(']').unwrap()];
    let mut features: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
    features.sort_unstable();
    let mut expected = vec![
        "level15",
        "level25",
        "nonspacing-attributes",
        "level25-32colors",
        "level35-32colors",
        "render",
        "bedstead",
        "viewer",
        "extended-sets",
    ];
    expected.sort_unstable();
    assert_eq!(features, expected);
}

/// REQ-1644: no feature adds columns; read from the manifest, whatever this
/// build's features.
#[test]
fn no_feature_adds_columns() {
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    assert!(!manifest.contains("side-panels"), "DEC-1636: side panels are never built");
}

/// Page 100 with Level 1 rows and local enhancement data.
fn page(rows: &[(u8, &[u8])], local: &[TripletFunction]) -> Vec<Packet> {
    display_page(0, rows, local_data(local))
}

/// Without objects, an invocation places nothing (decision E.71).
#[cfg(not(feature = "objects"))]
#[test]
fn objects_compiled_out_are_ignored() {
    use common::{build, char_of, define, end, g0, invoke, local_number, x26};
    use neoteletext_core::packet::{ObjectKind, ObjectSource};
    let object = [
        define(ObjectKind::Passive, 0b11, local_number(1, 0)),
        g0(0, b'P'),
        TripletFunction::Termination { kind: 0b101 },
    ];
    let mut data = vec![set_position(3, 4), invoke(ObjectSource::Local, ObjectKind::Passive, local_number(1, 0))];
    data.push(end());
    let packets = display_page(0, &[], vec![x26(0, &data), x26(1, &object)]);
    let (grid, _) = build(&packets, neoteletext_core::config::PresentationLevel::Level35);
    assert_ne!(char_of(&grid.cells[3][4]), 'P');
}

/// Without the Colour Map of Level 2.5, nothing redefines a CLUT, and CLUTs
/// 2 and 3 are copies of CLUTs 0 and 1 (decision E.105).
#[cfg(not(feature = "level25-32colors"))]
#[test]
fn level25_32colors_compiled_out_keeps_the_default_sixteen() {
    use common::{build_35, x28};
    use neoteletext_core::colour::{ColourMap, colour_index};
    use neoteletext_core::types::{PresentationFields, Rgb12};
    let words = core::array::from_fn(|entry| Rgb12::new(9, entry as u8, 0));
    let fields = PresentationFields { colour_words: Some(words), ..PresentationFields::default() };
    let grid = build_35(&display_page(0, &[(1, b"A")], vec![x28(0, &fields), x28(4, &fields)]));
    for clut in 0..2u8 {
        for entry in 0..8u8 {
            let low = colour_index(clut, entry);
            let high = colour_index(clut + 2, entry);
            assert_eq!(grid.colour_map.entry(low), ColourMap::DEFAULT.entry(low), "CLUT {clut} entry {entry}");
            assert_eq!(grid.colour_map.entry(high), ColourMap::DEFAULT.entry(low), "CLUT {} entry {entry}", clut + 2);
        }
    }
}

/// With the Colour Map of Level 2.5 and without that of Level 3.5, X/28/0
/// redefines CLUTs 2 and 3, and X/28/4 redefines nothing: CLUTs 0 and 1
/// keep their defaults even at Level 3.5 (decision E.105).
#[cfg(all(feature = "level25-32colors", not(feature = "level35-32colors")))]
#[test]
fn level35_32colors_compiled_out_keeps_cluts_0_and_1() {
    use common::{build_35, x28};
    use neoteletext_core::colour::{ColourMap, colour_index};
    use neoteletext_core::types::{PresentationFields, Rgb12};
    let words = core::array::from_fn(|entry| Rgb12::new(9, entry as u8, 0));
    let fields = PresentationFields { colour_words: Some(words), ..PresentationFields::default() };
    let grid = build_35(&display_page(0, &[(1, b"A")], vec![x28(0, &fields), x28(4, &fields)]));
    assert_eq!(grid.colour_map.entry(colour_index(2, 3)), Rgb12::new(9, 3, 0), "CLUT 2 from X/28/0");
    for entry in 0..16u8 {
        assert_eq!(grid.colour_map.entry(entry), ColourMap::DEFAULT.entry(entry), "CLUT 0 and 1 entry {entry}");
    }
}

/// Without the Additional Flash Functions, their triplet changes nothing,
/// and the Level 1 flash is unchanged (decision E.105).
#[cfg(not(feature = "additional-flash-functions"))]
#[test]
fn additional_flash_functions_compiled_out_leave_level_1_flash() {
    use common::build_25;
    use neoteletext_core::consts::FLASH;
    use neoteletext_core::level1::{FlashMode, FlashRate};
    let local =
        [set_position(1, 0), TripletFunction::AdditionalFlash { column: 3, mode: 0b01, rate: 0b010, reserved: 0 }];
    let grid = build_25(&page(&[(1, &[b'A'; 40]), (2, &[FLASH, b'A'])], &local));
    assert!(!grid.cells[1][3].flash, "the triplet is ignored");
    let level_1 = grid.cells[2][1];
    assert!(level_1.flash);
    assert_eq!(
        (level_1.attributes.flash_mode(), level_1.attributes.flash_rate()),
        (FlashMode::Normal, FlashRate::Level1)
    );
}
