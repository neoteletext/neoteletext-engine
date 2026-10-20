//! Each capability of decision E.71 compiled out (REQ-GEN-020): its page
//! data is ignored as the decision says, never an error. Each test runs
//! only in the builds that leave its capability out; the builds with it
//! test what it does in `level25.rs` and `objects.rs`.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]
// The helpers serve only the tests of capabilities compiled out, so in a
// build with every capability they are unused.
#![cfg_attr(
    all(feature = "objects", feature = "side-panels", feature = "colours32", feature = "animation"),
    allow(dead_code, unused_imports, reason = "every capability is compiled in, so no test here runs")
)]

mod common;

use common::{display_page, local_data, set_position};
use neoteletext_core::packet::{Packet, TripletFunction};

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

/// Without side panels, a page is always 40 columns (decision E.71).
#[cfg(not(feature = "side-panels"))]
#[test]
fn side_panels_compiled_out_are_never_shown() {
    use common::{build_35, x28};
    use neoteletext_core::colour::SidePanels;
    use neoteletext_core::types::{PresentationFields, SidePanelFields};
    let panels = PresentationFields {
        side_panels: Some(SidePanelFields { left: true, right: true, status_flag: true, left_columns: 8 }),
        ..PresentationFields::default()
    };
    let grid = build_35(&display_page(0, &[(1, b"A")], vec![x28(0, &panels)]));
    assert_eq!(grid.side_panels, SidePanels { left: 0, right: 0 });
}

/// Without the 32-colour Colour Map, nothing redefines a CLUT, and CLUTs 2
/// and 3 are copies of CLUTs 0 and 1 (decision E.71).
#[cfg(not(feature = "colours32"))]
#[test]
fn colours32_compiled_out_keeps_the_default_sixteen() {
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

/// Without animation, Additional Flash Functions change nothing, and the
/// Level 1 flash is unchanged (decision E.71).
#[cfg(not(feature = "animation"))]
#[test]
fn animation_compiled_out_leaves_level_1_flash() {
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
        (FlashMode::Normal, FlashRate::Slow)
    );
}
