//! Tests of the numbers of area NUM (REQ-NUM-031): each count is found by
//! enumerating every page number or every 16-bit value, and compared with
//! the constant and with the figure the standard gives.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

extern crate std;
// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use std::collections::{BTreeMap, BTreeSet};
use std::vec::Vec;

/// Every page address of a service as (magazine, page number).
fn every_page() -> impl Iterator<Item = (u8, u8)> {
    (FIRST_MAGAZINE..=MAGAZINE_ZERO_MEANS).flat_map(|magazine| (0..=u8::MAX).map(move |page| (magazine, page)))
}

/// Every sub-code, in ascending order: the 16-bit values with no bit
/// outside the thirteen.
fn every_sub_code() -> Vec<u16> {
    (0..=u16::MAX).filter(|&value| is_sub_code(value)).collect()
}

/// REQ-NUM-003, REQ-NUM-004, REQ-NUM-010, REQ-NUM-015: the page numbers.
#[test]
fn page_numbers_are_counted_by_enumeration() {
    assert_eq!(every_page().count(), PAGE_NUMBERS);
    assert_eq!(PAGE_NUMBERS, 2048);
    assert_eq!(MAGAZINES, 8);
    let usable = every_page().filter(|&(magazine, page)| !is_reserved_page(magazine, page)).count();
    assert_eq!(usable, USABLE_PAGE_NUMBERS);
    assert_eq!(usable, 2022);
    let keyable: Vec<u16> = every_page()
        .filter(|&(magazine, page)| is_keyable_page(magazine, page))
        .map(|(magazine, page)| (u16::from(magazine) << MPP_MAGAZINE_SHIFT) | u16::from(page))
        .collect();
    assert_eq!(keyable.len(), KEYABLE_PAGES);
    assert_eq!(keyable.len(), 800);
    assert_eq!(keyable.iter().min(), Some(&FIRST_KEYABLE_PAGE));
    assert_eq!(keyable.iter().max(), Some(&LAST_KEYABLE_PAGE));
    assert_eq!((FIRST_KEYABLE_PAGE, LAST_KEYABLE_PAGE), (0x100, 0x899));
    let every: Vec<u16> =
        every_page().map(|(magazine, page)| (u16::from(magazine) << MPP_MAGAZINE_SHIFT) | u16::from(page)).collect();
    assert_eq!((every.iter().min(), every.iter().max()), (Some(&FIRST_PAGE_ADDRESS), Some(&LAST_PAGE_ADDRESS)));
    assert_eq!((FIRST_PAGE_ADDRESS, LAST_PAGE_ADDRESS), (0x100, 0x8FF));
    // No keyable page is reserved: every reserved number has a hexadecimal
    // digit.
    assert!(every_page().all(|(m, p)| !(is_keyable_page(m, p) && is_reserved_page(m, p))));
}

/// REQ-NUM-008, REQ-NUM-010, REQ-NUM-013: the reserved page numbers are
/// Annex A.10's, and the recommended ones of Annex E.15 are not reserved.
#[test]
fn reserved_and_recommended_page_numbers_are_the_standards() {
    let reserved: BTreeSet<u16> = every_page()
        .filter(|&(magazine, page)| is_reserved_page(magazine, page))
        .map(|(magazine, page)| (u16::from(magazine) << MPP_MAGAZINE_SHIFT) | u16::from(page))
        .collect();
    let mut expected = BTreeSet::from([0x1BE, 0x1F0]);
    for magazine in 1..=8u16 {
        expected.extend([0xFD, 0xFE, 0xFF].map(|page| (magazine << 8) | page));
    }
    assert_eq!(reserved, expected);
    assert_eq!((PAGE_MIP, PAGE_MOT, PAGE_TIME_FILLING), (0xFD, 0xFE, 0xFF));
    for page in [PAGE_HOME, PAGE_EPG, PAGE_TRIGGER, PAGES_TOP_DATA[0], PAGES_TOP_DATA[1]] {
        assert!(!is_reserved_page((page >> MPP_MAGAZINE_SHIFT) as u8, page as u8), "{page:03X}");
    }
    assert_eq!([PAGE_HOME, PAGE_EPG, PAGE_TRIGGER], [0x100, 0x1DF, 0x1E7]);
    assert_eq!(PAGES_TOP_DATA, [0x1F1, 0x1F4]);
}

/// REQ-NUM-005, REQ-NUM-007, REQ-NUM-018, REQ-NUM-020, REQ-NUM-016,
/// REQ-NUM-017: every count of sub-codes, by enumerating all 65,536
/// values.
#[test]
fn sub_codes_are_counted_by_enumeration() {
    let all = every_sub_code();
    assert_eq!(all.len(), SUBCODE_VALUES);
    assert_eq!(all.len(), 8192);
    assert_eq!((all[0], *all.last().unwrap()), (SUBCODE_NONE, SUBCODE_ANY));
    assert_eq!((SUBCODE_MASK, SUBCODE_ANY, LINK_NULL_SUBCODE), (0x3F7F, 0x3F7F, 0x3F7F));
    assert_eq!(NULL_ADDRESS, (0xFF, 0x3F7F));
    let sub_pages: Vec<u16> = all.iter().copied().filter(|&c| c != SUBCODE_NONE && c != SUBCODE_ANY).collect();
    assert_eq!(sub_pages.len(), MAX_SUBPAGES);
    assert_eq!(sub_pages.len(), 8190);
    assert_eq!((sub_pages[0], *sub_pages.last().unwrap()), (FIRST_SUBPAGE_CODE, LAST_SUBPAGE_CODE));
    let decimal = all.iter().filter(|&&c| c != SUBCODE_NONE && is_decimal_sub_code(c)).count();
    assert_eq!(decimal, DECIMAL_SUBCODES);
    assert_eq!(decimal, 3199);
    let above_set = sub_pages.iter().filter(|&&c| c > SET_LAST_SUBCODE).count();
    assert_eq!(above_set, SUBCODES_ABOVE_SET);
    assert_eq!(above_set, 8069);
    let decimal_above_set = all.iter().filter(|&&c| c > SET_LAST_SUBCODE && is_decimal_sub_code(c)).count();
    assert_eq!(decimal_above_set, DECIMAL_SUBCODES_ABOVE_SET);
    assert_eq!(decimal_above_set, 3120);
    assert_eq!(above_set - decimal_above_set, 4949);
    // The first set: 0001 to 0079 are exactly the decimal sub-codes up to 79.
    let set = all.iter().filter(|&&c| c != SUBCODE_NONE && c <= SET_LAST_SUBCODE && is_decimal_sub_code(c)).count();
    assert_eq!(set, usize::from(SET_SUBPAGES));
    assert_eq!(SUBCODE_BITS, 13);
}

/// REQ-NUM-005: the four elements and their widths; no value with a bit
/// outside them is a sub-code.
#[test]
fn elements_round_trip_for_every_sub_code() {
    for code in every_sub_code() {
        assert_eq!(sub_code(s4(code), s3(code), s2(code), s1(code)), code);
        assert!(s1(code) <= 0xF && s2(code) <= 7 && s3(code) <= 0xF && s4(code) <= 3);
    }
    for bit in [7, 14, 15] {
        assert!(!is_sub_code(1 << bit));
    }
    assert_eq!(sub_code(3, 0xF, 7, 0xF), 0x3F7F);
}

/// The rank keeps the order of the sub-codes and is a bijection onto 0 to
/// 8,191.
#[test]
fn rank_numbers_the_sub_codes_in_order() {
    for (position, code) in every_sub_code().into_iter().enumerate() {
        assert_eq!(usize::from(rank(code)), position);
        assert_eq!(from_rank(rank(code)), code);
    }
}

/// REQ-NUM-016, REQ-NUM-019: the numbering at every boundary, and its
/// inverse for every sub-code.
#[test]
fn sub_code_of_follows_the_code_of_practice() {
    assert_eq!(sub_code_of(1, 1), Some(0x0000));
    assert_eq!(sub_code_of(1, 2), Some(0x0001));
    assert_eq!(sub_code_of(2, 2), Some(0x0002));
    assert_eq!(sub_code_of(10, 79), Some(0x0010));
    assert_eq!(sub_code_of(79, 79), Some(0x0079));
    assert_eq!(sub_code_of(1, 80), Some(0x0100));
    assert_eq!(sub_code_of(80, 80), Some(0x0179));
    assert_eq!(sub_code_of(3120, 8069), Some(0x3979));
    assert_eq!(sub_code_of(3121, 8069), Some(0x007A));
    assert_eq!(sub_code_of(8069, 8069), Some(0x3F7E));
    for (place, count) in [(0, 1), (2, 1), (0, 79), (80, 79), (1, 0), (1, 8070), (8070, 8070)] {
        assert_eq!(sub_code_of(place, count), None, "{place} of {count}");
    }
    let every = every_sub_code();
    for count in [1u16, 2, 3, 78, 79, 80, 81, 3120, 3121, 8068, 8069] {
        let codes: Vec<u16> = (1..=count).map(|place| sub_code_of(place, count).unwrap()).collect();
        // Unique, never 3F7F, and 0000 only for a single page.
        assert_eq!(codes.iter().collect::<BTreeSet<_>>().len(), usize::from(count));
        assert!(!codes.contains(&SUBCODE_ANY));
        assert_eq!(codes.contains(&SUBCODE_NONE), count == 1);
        if count > SET_SUBPAGES {
            assert!(codes.iter().all(|&code| code > SET_LAST_SUBCODE), "{count}");
        } else if count > 1 {
            assert!(codes.iter().all(|&code| is_decimal_sub_code(code) && code <= SET_LAST_SUBCODE));
        }
        // Decimal ones first, each run ascending.
        let decimal_run = codes.iter().take_while(|&&code| is_decimal_sub_code(code)).count();
        assert!(codes[decimal_run..].iter().all(|&code| !is_decimal_sub_code(code)));
        assert!(codes[..decimal_run].windows(2).all(|pair| pair[0] < pair[1]));
        assert!(codes[decimal_run..].windows(2).all(|pair| pair[0] < pair[1]));
        // The inverse, for every sub-code.
        let places: BTreeMap<u16, u16> = codes.iter().zip(1..).map(|(&code, place)| (code, place)).collect();
        for &code in &every {
            assert_eq!(place_of(code, count), places.get(&code).copied(), "{code:04X} of {count}");
        }
    }
    // The largest set uses every sub-code above 0079 but 3F7F.
    let largest: BTreeSet<u16> = (1..=8069).map(|place| sub_code_of(place, 8069).unwrap()).collect();
    let expected: BTreeSet<u16> =
        every.iter().copied().filter(|&code| code > SET_LAST_SUBCODE && code != SUBCODE_ANY).collect();
    assert_eq!(largest, expected);
}

/// REQ-NUM-017: entry accepts exactly the 3,199 decimal sub-codes.
#[test]
fn entry_accepts_exactly_the_decimal_sub_codes() {
    let mut accepted = BTreeSet::new();
    for d4 in 0..=0xF {
        for d3 in 0..=0xF {
            for d2 in 0..=0xF {
                for d1 in 0..=0xF {
                    if let Some(code) = decimal_sub_code([d4, d3, d2, d1]) {
                        assert_eq!((s4(code), s3(code), s2(code), s1(code)), (d4, d3, d2, d1));
                        assert!(accepted.insert(code));
                    }
                }
            }
        }
    }
    assert_eq!(accepted.len(), DECIMAL_SUBCODES);
    let decimal: BTreeSet<u16> =
        every_sub_code().into_iter().filter(|&code| code != SUBCODE_NONE && is_decimal_sub_code(code)).collect();
    assert_eq!(accepted, decimal);
    assert_eq!(decimal_sub_code([3, 9, 7, 9]), Some(0x3979));
    assert_eq!(decimal_sub_code([0, 0, 0, 0]), None);
    assert_eq!(decimal_sub_code([0, 0, 8, 0]), None);
    assert_eq!(decimal_sub_code([4, 0, 0, 0]), None);
}

/// REQ-NUM-022: the nnXs layout of an enhancement data page, Annex A.1.
#[test]
fn enhancement_sub_codes_are_nn_x_s() {
    for code in every_sub_code() {
        let (last, update, sub_page) =
            (enhancement_last_packet(code), enhancement_update(code), enhancement_sub_page(code));
        assert_eq!(enhancement_sub_code(last, update, sub_page), code);
        assert_eq!(last, (s4(code) << 4) | s3(code));
        assert_eq!((update, sub_page), (s2(code), s1(code)));
    }
    // nn 41 is X/26/15: S4 2, S3 9.
    assert_eq!(enhancement_sub_code(41, 0, 0), 0x2900);
    assert_eq!(ENHANCEMENT_LAST_PACKET_LAST, 25 + 16);
    assert_eq!((ENHANCEMENT_SUBPAGES, TABLE_SUBPAGES), (16, 1));
}

/// REQ-NUM-024: the sub-code of a data broadcasting page, EN 300 708
/// clause 4.3.1.2; 3F7F never occurs.
#[test]
fn data_broadcasting_sub_codes_number_no_sub_pages() {
    assert_eq!(DATA_STREAMS, 15);
    for stream in 0..=DATA_STREAM_RESERVED {
        for continuity in 0..DATA_CONTINUITY_MODULUS {
            for last in DATA_LAST_PACKET_FIRST..=DATA_LAST_PACKET_LAST {
                let code = data_sub_code(stream, continuity, last);
                assert_eq!(
                    (data_stream(code), data_continuity(code), data_last_packet(code)),
                    (stream, continuity, last)
                );
                assert_ne!(code, SUBCODE_ANY);
            }
        }
    }
    // Table 2: even streams apply the 20 ms rule, odd ones do not.
    assert!(data_stream_has_20ms_rule(data_sub_code(0, 0, 1)));
    assert!(!data_stream_has_20ms_rule(data_sub_code(1, 0, 1)));
    // S2 and S4: 25 is S4 3, S2 1.
    assert_eq!(data_sub_code(0, 0, 25), 0x3010);
}
