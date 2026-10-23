//! Reassembling records from their fragments (REQ-AUD-007): a few records
//! in flight, the oldest incomplete one replaced by a further record.

use super::{FRAGMENT_DATA, Fragment, MAX_FRAGMENTS};
use crate::record::framed_length;

/// Bytes of a reassembly slot.
const SLOT_BYTES: usize = MAX_FRAGMENTS * FRAGMENT_DATA;

#[derive(Clone, Copy)]
struct Slot {
    service: u8,
    record: u8,
    /// One bit for each fragment received.
    received: u32,
    /// The fragments the record needs, once fragment 0 has said.
    needed: Option<u8>,
    /// Fragments received for this slot, for choosing the oldest.
    stamp: u32,
    bytes: [u8; SLOT_BYTES],
}

/// Reassembles records from fragments: at most `SLOTS` records in flight;
/// a fragment of a further record replaces the oldest incomplete one
/// (REQ-AUD-007).
pub struct Reassembler<const SLOTS: usize> {
    slots: [Option<Slot>; SLOTS],
    clock: u32,
}

impl<const SLOTS: usize> Default for Reassembler<SLOTS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SLOTS: usize> Reassembler<SLOTS> {
    /// An empty reassembler.
    #[must_use]
    pub const fn new() -> Self {
        Self { slots: [None; SLOTS], clock: 0 }
    }

    /// Forgets every record in flight.
    pub fn clear(&mut self) {
        self.slots = [None; SLOTS];
    }

    /// Adds a fragment; returns the service and the whole record when this
    /// fragment completes one. The record is valid until the next call.
    pub fn push(&mut self, fragment: &Fragment) -> Option<(u8, &[u8])> {
        self.clock = self.clock.wrapping_add(1);
        let index = self.slot_for(fragment.service, fragment.record)?;
        let slot = self.slots.get_mut(index)?.as_mut()?;
        slot.stamp = self.clock;
        let start = usize::from(fragment.index).saturating_mul(FRAGMENT_DATA);
        slot.bytes.get_mut(start..start.saturating_add(FRAGMENT_DATA))?.copy_from_slice(&fragment.data);
        slot.received |= 1_u32.checked_shl(u32::from(fragment.index)).unwrap_or(0);
        if fragment.index == 0 {
            let length = framed_length(&fragment.data).ok()?;
            slot.needed = u8::try_from(length.div_ceil(FRAGMENT_DATA)).ok();
        }
        let needed = slot.needed?;
        if fragment.last && fragment.index.checked_add(1) != Some(needed) {
            // A last flag where the length says otherwise: start again.
            *self.slots.get_mut(index)? = None;
            return None;
        }
        let all = 1_u32.checked_shl(u32::from(needed)).map_or(u32::MAX, |bit| bit.wrapping_sub(1));
        if slot.received & all != all {
            return None;
        }
        let length = framed_length(&slot.bytes).ok()?;
        let service = slot.service;
        // The slot is free for the next record; its bytes stay readable
        // until then.
        slot.received = 0;
        slot.needed = None;
        let bytes = self.slots.get(index)?.as_ref()?.bytes.get(..length)?;
        Some((service, bytes))
    }

    /// The slot of a record, starting one if it is new, replacing the
    /// oldest incomplete record when every slot is busy.
    fn slot_for(&mut self, service: u8, record: u8) -> Option<usize> {
        if let Some(found) = self.slots.iter().position(|slot| {
            slot.as_ref().is_some_and(|slot| slot.service == service && slot.record == record && slot.received != 0)
        }) {
            return Some(found);
        }
        // A new record of a service replaces that service's unfinished one
        // with a different id, which was abandoned by the transmitter.
        let index = self
            .slots
            .iter()
            .position(|slot| slot.as_ref().is_none_or(|slot| slot.received == 0 || slot.service == service))
            .or_else(|| {
                self.slots
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, slot)| slot.as_ref().map_or(0, |slot| slot.stamp))
                    .map(|(index, _)| index)
            })?;
        *self.slots.get_mut(index)? =
            Some(Slot { service, record, received: 0, needed: None, stamp: self.clock, bytes: [0; SLOT_BYTES] });
        Some(index)
    }
}
