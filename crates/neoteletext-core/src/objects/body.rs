//! Where a run of triplets comes from, and how one is fetched.

use crate::acq::{EnhancementPage, StoredPage};
use crate::consts::{DESIGNATION_CODES, LOCAL_TRIPLETS, OBJECT_BODY_TRIPLETS, TRIPLET_BYTES, TRIPLETS_PER_PACKET};
use crate::ecc::hamming2418;
use crate::packet::{Triplet, TripletLocation, triplet_location};

/// Where a run of triplets comes from: the page's own packets X/26, or an
/// object page's packets 3 to 25 and X/26.
#[derive(Clone, Copy, Debug)]
pub enum Body<'s> {
    /// The displayed page's packets X/26: triplet i is triplet i mod 13 of
    /// the packet with designation code i / 13.
    Local,
    /// An object page sub-page: triplet i is the absolute triplet number of
    /// clause 10.5.1.2.
    Page(EnhancementPage<'s>),
}

/// One triplet of a body.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fetch {
    /// A triplet that passed its Hamming 24/18 check.
    Triplet(Triplet),
    /// An uncorrectable triplet.
    Damaged,
    /// The packet holding it was not received.
    Missing,
}

/// The triplet of a stored packet: data byte 0 is the Hamming 8/4 byte 6,
/// then thirteen triplets of three bytes.
fn stored_triplet(bytes: &[u8], triplet: u8) -> Fetch {
    let start = usize::from(triplet).saturating_mul(TRIPLET_BYTES).saturating_add(1);
    match bytes.get(start..start.saturating_add(TRIPLET_BYTES)) {
        Some(&[a, b, c]) => {
            hamming2418([a, b, c]).map_or(Fetch::Damaged, |value| Fetch::Triplet(Triplet::from_value(value)))
        }
        _ => Fetch::Damaged,
    }
}

impl Body<'_> {
    /// The number of triplets the body can address: 208 for X/26, 507 for
    /// an object page.
    #[must_use]
    pub const fn capacity(&self) -> u16 {
        match self {
            Self::Local => LOCAL_TRIPLETS,
            Self::Page(_) => OBJECT_BODY_TRIPLETS,
        }
    }

    /// True for the local enhancement data and local objects.
    #[must_use]
    pub const fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }

    /// Triplet `index` of the body.
    #[must_use]
    pub fn fetch(&self, page: &StoredPage, index: u16) -> Fetch {
        let per_packet = TRIPLETS_PER_PACKET as u16;
        match self {
            Self::Local => {
                let designation = usize::from(index.checked_div(per_packet).unwrap_or(0));
                let triplet = usize::from(index.checked_rem(per_packet).unwrap_or(0));
                let received =
                    page.enhancements_present.checked_shr(designation as u32).is_some_and(|bits| bits & 1 == 1);
                if designation >= DESIGNATION_CODES || !received {
                    return Fetch::Missing;
                }
                match page.enhancements.get(designation).and_then(|packet| packet.get(triplet)) {
                    Some(Some(triplet)) => Fetch::Triplet(*triplet),
                    Some(None) => Fetch::Damaged,
                    None => Fetch::Missing,
                }
            }
            Self::Page(sub_page) => {
                let bytes = match triplet_location(index) {
                    Some(TripletLocation::Row { packet, triplet }) => {
                        sub_page.row(packet).map(|bytes| (bytes, triplet))
                    }
                    Some(TripletLocation::Enhancement { designation, triplet }) => {
                        sub_page.enhancement(designation).map(|bytes| (bytes, triplet))
                    }
                    None => None,
                };
                bytes.map_or(Fetch::Missing, |(bytes, triplet)| stored_triplet(bytes, triplet))
            }
        }
    }
}
