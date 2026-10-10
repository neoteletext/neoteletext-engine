//! Enhancement faults in the decoder's statistics (REQ-L25-051,
//! REQ-ARCH-007).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

mod common;

use common::{
    DISPLAY, Receiver, damage, display_page, feed, g0, invoke, local_data, public_number, row_0, set_position,
};
use tellytxt_core::acq::{Decoder, EnhancementFaults, PageBuild};
use tellytxt_core::config::{Config, PresentationLevel};
use tellytxt_core::consts::{HEADER_DISPLAY_BYTES, OBJECT_INVOCATIONS_PER_TYPE, SPACE};
use tellytxt_core::enhance::EnhancementReport;
use tellytxt_core::fsm::PageKey;
use tellytxt_core::level1::Grid;
use tellytxt_core::packet::{ObjectKind, ObjectSource, Packet, TripletFunction};
use tellytxt_core::types::{PageAddress, SubCode};
use tellytxt_core::viewer::{ComposeContext, Composed, Viewer, ViewerEvent};

/// The second feed starts after the first has finished and timed out.
const SECOND_FEED_US: u64 = 2_000_000_000;

/// Page 100 with one fault of most kinds in its local enhancement data,
/// and, after its first triplet, as many absent public objects as the
/// Annex A.8 limit lets through, and one more.
fn faulty_page() -> Vec<Packet> {
    let absent = invoke(ObjectSource::Public, ObjectKind::Active, public_number(0, 0));
    let mut local = vec![
        set_position(1, 0),
        // Damaged below: one damaged triplet.
        g0(9, b'Z'),
        // A reserved designation (REQ-L25-012).
        TripletFunction::ModifiedDesignation { column: 2, designation: 0b010_1000 },
        // Undefined G3 and G2 positions (decision E.50).
        TripletFunction::G3Character { column: 3, code: 0x6E, level_15_form: false },
        TripletFunction::G2Character { column: 4, code: 0x59 },
        // No DRCS page is linked, so its data is missing.
        TripletFunction::DrcsCharacter { column: 5, normal: true, character: 1 },
        // Row 0 columns 0 to 7 are the receiver's (REQ-L25-050).
        row_0(0, 0),
        g0(3, b'Q'),
    ];
    local.extend(core::iter::repeat_n(absent, usize::from(OBJECT_INVOCATIONS_PER_TYPE) + 1));
    let mut packets = local_data(&local);
    packets[0] = damage(packets[0], 1);
    display_page(0, &[(1, b"LEVEL ONE")], packets)
}

/// The faults [`faulty_page`] has.
const FAULTY_PAGE: EnhancementFaults = EnhancementFaults {
    damaged_triplets: 1,
    forbidden_invocations: 0,
    absent_objects: OBJECT_INVOCATIONS_PER_TYPE as u32,
    limited_invocations: 1,
    discarded_writes: 1,
    ignored_designations: 1,
    undefined_characters: 2,
    drcs_missing: 1,
    drcs_invalid: 0,
};

fn address() -> PageAddress {
    PageAddress::from_mpp(DISPLAY).unwrap()
}

/// Composes page 100 as the viewer does, and returns its report.
fn compose(decoder: &Decoder<'_>, config: &Config) -> Option<EnhancementReport> {
    let mut viewer = Viewer::new(address());
    let _ = viewer.event(ViewerEvent::Started);
    let _ = viewer.event(ViewerEvent::PagePresent);
    let magazine = decoder.magazine_data(address().magazine);
    let rolling = [SPACE; HEADER_DISPLAY_BYTES];
    let context = ComposeContext {
        magazine: &magazine,
        config,
        rolling_header: &rolling,
        store: decoder.store(),
        row24: None,
        lookup: decoder.enhancement_lookup(),
    };
    let page = decoder.store().get(address(), SubCode::ANY);
    let mut grid = Box::new(Grid::EMPTY);
    match viewer.compose(&mut grid, page, &context) {
        Composed::Built(report) => report,
        Composed::Held => panic!("the viewer is not holding"),
    }
}

/// Composes page 100 and records the report, as a port does on a redraw.
fn redraw(decoder: &mut Decoder<'_>, config: &Config) -> Option<EnhancementReport> {
    let report = compose(decoder, config);
    if let Some(report) = report {
        decoder.record_enhancement(report.build, &report.faults);
    }
    report
}

fn config(level: PresentationLevel) -> Config {
    Config { level, ..Config::default() }
}

/// REQ-L25-051, REQ-ARCH-007: every kind of fault the report counts is
/// added to its own count in the statistics.
#[test]
fn every_kind_of_fault_reaches_the_statistics() {
    let mut pages = vec![tellytxt_core::acq::StoredPage::EMPTY; 1];
    let mut decoder = Decoder::new(&mut pages, Config::default());
    // A different count for each kind, so that a field added to the wrong
    // count shows.
    let faults = EnhancementFaults {
        damaged_triplets: 1,
        forbidden_invocations: 2,
        absent_objects: 3,
        limited_invocations: 4,
        discarded_writes: 5,
        ignored_designations: 6,
        undefined_characters: 7,
        drcs_missing: 8,
        drcs_invalid: 9,
    };
    let key = PageKey { address: address(), subcode: SubCode::new(0) };
    let build = PageBuild { key, receptions: 1, level: PresentationLevel::Level25 };
    assert!(decoder.record_enhancement(build, &faults));
    assert_eq!(decoder.statistics().enhancement, faults);
    let next = PageBuild { receptions: 2, ..build };
    assert!(decoder.record_enhancement(next, &faults));
    let mut twice = faults;
    twice.add(&faults);
    assert_eq!(decoder.statistics().enhancement, twice);
    assert_eq!(twice.drcs_invalid, 18);
    decoder.reset_statistics();
    assert_eq!(decoder.statistics().enhancement, EnhancementFaults::NONE);
}

/// REQ-L25-051: the faults a real build meets reach the statistics through
/// the viewer, while the Level 1 page is still shown.
#[test]
fn faults_of_a_built_page_reach_the_statistics() {
    let config = config(PresentationLevel::Level25);
    Receiver::new().run_mut(&faulty_page(), config, |decoder| {
        assert_eq!(decoder.statistics().enhancement, EnhancementFaults::NONE, "nothing counted before a build");
        let report = redraw(decoder, &config).unwrap();
        assert_eq!(report.faults, FAULTY_PAGE);
        assert_eq!(decoder.statistics().enhancement, FAULTY_PAGE);
    });
}

/// The decoder counts a build once however often the viewer redraws it,
/// and again for a new reception or a new level.
#[test]
fn redraws_do_not_count_again() {
    let level_25 = config(PresentationLevel::Level25);
    let packets = faulty_page();
    Receiver::new().run_mut(&packets, level_25, |decoder| {
        for _ in 0..3 {
            let _ = redraw(decoder, &level_25);
        }
        assert_eq!(decoder.statistics().enhancement, FAULTY_PAGE, "three redraws, one build");
        let mut twice = FAULTY_PAGE;
        twice.add(&FAULTY_PAGE);
        // The page received again is a new build.
        feed(decoder, &packets, SECOND_FEED_US);
        let _ = redraw(decoder, &level_25);
        let _ = redraw(decoder, &level_25);
        assert_eq!(decoder.statistics().enhancement, twice, "a new reception");
        // So is the same reception at another level.
        let level_35 = config(PresentationLevel::Level35);
        let _ = redraw(decoder, &level_35);
        twice.add(&FAULTY_PAGE);
        assert_eq!(decoder.statistics().enhancement, twice, "another level");
        // At Levels 1 and 1.5 there is no report to record.
        assert!(compose(decoder, &config(PresentationLevel::Level15)).is_none());
    });
}
