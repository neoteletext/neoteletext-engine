//! Tests of the Foundation types: addresses, sub-codes, control bits,
//! colours, font glyphs and the fixed-capacity vector.
#![allow(clippy::unwrap_used)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

#[test]
fn magazine_zero_is_eight() {
    assert_eq!(Magazine::from_bits(0).number(), 8);
    assert_eq!(Magazine::from_bits(3).number(), 3);
    assert_eq!(Magazine::new(8).unwrap().bits(), 0);
    assert!(Magazine::new(0).is_none());
    assert!(Magazine::new(9).is_none());
}

#[test]
fn page_addresses_round_trip() {
    let address = PageAddress::from_mpp(0x8FF).unwrap();
    assert_eq!(address.mpp(), 0x8FF);
    assert_eq!(&address.ascii(), b"8FF");
    assert!(!address.page.is_decimal());
    assert!(PageAddress::from_mpp(0x100).unwrap().page.is_decimal());
    assert!(PageAddress::from_mpp(0x900).is_none());
    assert!(PageAddress::from_mpp(0x0A0).is_none());
}

#[test]
fn national_index_follows_e21() {
    // France 2 page 888: C12=1, C13=0, C14=0 gives index 4 (French).
    assert_eq!(ControlBits(1 << 12).national_index(), 4);
    // Italian subtitles: C12=0, C13=1, C14=1 gives index 3.
    assert_eq!(ControlBits((1 << 13) | (1 << 14)).national_index(), 3);
}

#[test]
fn fixed_vec_basics() {
    let mut vec: FixedVec<u8, 3> = FixedVec::new(0);
    assert!(vec.push(1) && vec.push(2) && vec.push(3));
    assert!(!vec.push(4));
    assert_eq!(vec.remove(0), Some(1));
    assert_eq!(vec.as_slice(), &[2, 3]);
    assert_eq!(vec.pop(), Some(3));
    assert_eq!(vec.len(), 1);
}

/// REQ-GEN-002: two vectors are equal when their items are, whatever the
/// unused slots hold; an emptied vector is empty again.
#[test]
fn fixed_vec_equality_ignores_unused_slots() {
    let mut zeros: FixedVec<u8, 3> = FixedVec::new(0);
    let mut nines: FixedVec<u8, 3> = FixedVec::new(9);
    assert!(zeros.is_empty());
    assert_eq!(zeros, nines);
    assert!(zeros.push(1) && nines.push(1));
    assert_eq!(zeros, nines);
    assert!(nines.push(2));
    assert_ne!(zeros, nines);
    nines.clear();
    assert!(nines.is_empty() && !zeros.is_empty());
}
