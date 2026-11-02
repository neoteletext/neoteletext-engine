#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]

// Tests of REQ-AUD-005, REQ-AUD-009 and REQ-AUD-014.
use super::*;
use crate::record::Object;
use std::vec::Vec;

fn object(handle: u8, generation: u16, samples: u32) -> Object<'static> {
    // PCM in 100-byte blocks: 50 samples a block.
    Object {
        handle,
        generation,
        id: u32::from(handle),
        codec: Some(Codec::Pcm16),
        rate_hz: 8_000,
        block_bytes: 100,
        samples,
        blocks: u16::try_from(samples.div_ceil(50)).unwrap(),
        label: None,
        language: None,
    }
}

const IDLE: &dyn Fn(SoundId) -> bool = &|_| false;

// Verifies REQ-AUD-005.
#[test]
fn blocks_are_stored_and_a_sound_completes() {
    let mut arena = [0_u8; 1000];
    let mut store = Store::<4>::new(&mut arena);
    assert!(matches!(store.define(&object(1, 1, 120), IDLE), Defined::New { replaced: None, .. }));
    // Three blocks: 50, 50 and a final 20 samples (40 bytes).
    let blocks: Vec<u8> = (0..240_u8).collect();
    let sound = SoundId { handle: 1, generation: 1 };
    assert_eq!(
        store.put(&Data { handle: 1, generation: 1, first_block: 1, blocks: &blocks[100..] }),
        Stored::Blocks { first: 1, count: 2 }
    );
    assert_eq!(store.contiguous(sound), 0);
    assert_eq!(
        store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &blocks[..100] }),
        Stored::Blocks { first: 0, count: 1 }
    );
    assert!(store.entry(1).unwrap().complete());
    assert_eq!(store.block(sound, 2), Some(&blocks[200..]));
    // A repeat changes nothing; a different block is a conflict.
    assert_eq!(store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &blocks[..100] }), Stored::Repeat);
    let mut changed = blocks[..100].to_vec();
    changed[0] ^= 1;
    assert_eq!(store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &changed }), Stored::Conflict);
    assert!(store.entry(1).unwrap().conflict);
}

// Verifies REQ-AUD-005.
#[test]
fn wrong_lengths_and_generations_are_discarded() {
    let mut arena = [0_u8; 1000];
    let mut store = Store::<4>::new(&mut arena);
    store.define(&object(1, 1, 120), IDLE);
    assert_eq!(store.put(&Data { handle: 1, generation: 2, first_block: 0, blocks: &[0; 100] }), Stored::Unknown);
    assert_eq!(store.put(&Data { handle: 2, generation: 1, first_block: 0, blocks: &[0; 100] }), Stored::Unknown);
    assert_eq!(store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &[0; 99] }), Stored::Malformed);
    assert_eq!(store.put(&Data { handle: 1, generation: 1, first_block: 3, blocks: &[0; 40] }), Stored::Malformed);
}

/// A snapshot of what a refused record must leave as it was.
fn state(store: &Store<'_, 4>, sound: SoundId) -> (Option<Vec<u8>>, u16, u16, bool) {
    let entry = store.entry(sound.handle).unwrap();
    (store.block(sound, 0).map(<[u8]>::to_vec), store.contiguous(sound), entry.received, entry.conflict)
}

// Verifies REQ-AUD-003 and REQ-AUD-005: a record is refused whole, before
// anything in it is stored, so the good record completes the sound.
#[test]
fn a_refused_record_stores_nothing_and_the_good_record_completes() {
    let sound = SoundId { handle: 1, generation: 1 };
    let good = [7_u8; 200];
    for bad in [&[7_u8; 150][..], &[7_u8; 300][..]] {
        let mut arena = [0_u8; 1000];
        let mut store = Store::<4>::new(&mut arena);
        store.define(&object(1, 1, 100), IDLE);
        let before = state(&store, sound);
        assert_eq!(store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: bad }), Stored::Malformed);
        assert_eq!(state(&store, sound), before, "a record of {} bytes", bad.len());
        assert_eq!(
            store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &good }),
            Stored::Blocks { first: 0, count: 2 }
        );
        assert!(store.entry(1).unwrap().complete());
    }
}

// Verifies REQ-AUD-005 and REQ-AUD-009: a discarded record cannot put a
// sound in conflict, however its first block compares.
#[test]
fn a_refused_record_cannot_raise_a_conflict() {
    let mut arena = [0_u8; 1000];
    let mut store = Store::<4>::new(&mut arena);
    let sound = SoundId { handle: 1, generation: 1 };
    store.define(&object(1, 1, 100), IDLE);
    assert_eq!(
        store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &[1; 100] }),
        Stored::Blocks { first: 0, count: 1 }
    );
    let before = state(&store, sound);
    let mut bad = [2_u8; 150];
    bad[99] = 1;
    assert_eq!(store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &bad }), Stored::Malformed);
    assert_eq!(state(&store, sound), before);
}

// Verifies REQ-AUD-009: the walk compares every block already stored before
// it writes, so a conflict part way through stores none of the record.
#[test]
fn a_conflict_part_way_through_a_record_stores_none_of_it() {
    let mut arena = [0_u8; 1000];
    let mut store = Store::<4>::new(&mut arena);
    let sound = SoundId { handle: 1, generation: 1 };
    store.define(&object(1, 1, 150), IDLE);
    store.put(&Data { handle: 1, generation: 1, first_block: 1, blocks: &[1; 100] });
    let mut blocks = [0_u8; 200];
    blocks[100..].fill(2);
    assert_eq!(store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &blocks }), Stored::Conflict);
    let entry = store.entry(1).unwrap();
    assert!(entry.conflict);
    assert_eq!((store.block(sound, 0), entry.received), (None, 1));
}

// Verifies REQ-AUD-009 and REQ-AUD-014: the same order in a live sound's ring.
#[test]
fn a_conflict_part_way_through_a_live_record_stores_none_of_it() {
    let mut arena = [0_u8; 3_300];
    let mut store = Store::<2>::new(&mut arena);
    let sound = SoundId { handle: 1, generation: 1 };
    store.define(&Object { samples: 0, blocks: 0, ..object(1, 1, 0) }, IDLE);
    store.put(&live(6, &[1; 100]));
    let mut blocks = [0_u8; 200];
    blocks[100..].fill(2);
    assert_eq!(store.put(&live(5, &blocks)), Stored::Conflict);
    assert_eq!((store.live_block(sound, 5), store.entry(1).unwrap().received), (None, 1));
}

// Verifies REQ-AUD-009.
#[test]
fn generations_replace_and_conflicts_are_refused() {
    let mut arena = [0_u8; 1000];
    let mut store = Store::<4>::new(&mut arena);
    store.define(&object(1, 5, 100), IDLE);
    assert_eq!(store.define(&object(1, 5, 100), IDLE), Defined::Repeat);
    assert_eq!(store.define(&object(1, 5, 150), IDLE), Defined::Conflict, "same generation, different definition");
    assert_eq!(store.define(&object(1, 4, 100), IDLE), Defined::Conflict, "an older generation");
    assert!(matches!(
        store.define(&object(1, 6, 100), IDLE),
        Defined::New { replaced: Some(Entry { sound: SoundId { generation: 5, .. }, .. }), .. }
    ));
}

// Verifies REQ-AUD-009.
#[test]
fn the_oldest_idle_sound_is_evicted_and_a_busy_one_never() {
    // Each 100-sample sound takes 200 bytes and a 1-byte bitmap.
    let mut arena = [0_u8; 450];
    let mut store = Store::<4>::new(&mut arena);
    store.define(&object(1, 1, 100), IDLE);
    store.define(&object(2, 1, 100), IDLE);
    let busy_one: &dyn Fn(SoundId) -> bool = &|sound| sound.handle == 1;
    assert!(matches!(store.define(&object(3, 1, 100), busy_one), Defined::New { .. }));
    assert!(store.entry(1).is_some() && store.entry(2).is_none() && store.entry(3).is_some());
    let all_busy: &dyn Fn(SoundId) -> bool = &|_| true;
    assert_eq!(store.define(&object(4, 1, 100), all_busy), Defined::ResourceLimit);
    assert_eq!(store.define(&object(5, 1, 1000), IDLE), Defined::ResourceLimit, "larger than the arena");
}

// Verifies REQ-AUD-009: a store that refuses a sound has changed nothing.
#[test]
fn a_refusal_evicts_nothing() {
    // Two idle sounds and a busy one of 200 bytes and a 1-byte bitmap each.
    let mut arena = [0_u8; 610];
    let mut store = Store::<4>::new(&mut arena);
    store.define(&object(1, 1, 100), IDLE);
    store.define(&object(2, 1, 100), IDLE);
    store.define(&object(3, 1, 100), IDLE);
    let busy_last: &dyn Fn(SoundId) -> bool = &|sound| sound.handle == 3;
    // 500 bytes do not fit beside the busy sound, so nothing is evicted for it.
    assert_eq!(store.define(&object(4, 1, 250), busy_last), Defined::ResourceLimit);
    assert!(store.entry(1).is_some() && store.entry(2).is_some() && store.entry(3).is_some());
}

// Verifies REQ-AUD-009 and DEC-2113: a newer generation that cannot be
// admitted leaves the older one in place, and one that fits only in the
// older one's room replaces it.
#[test]
fn a_newer_generation_that_does_not_fit_leaves_the_older_one() {
    let mut arena = [0_u8; 201];
    let mut store = Store::<4>::new(&mut arena);
    store.define(&object(1, 1, 100), IDLE);
    assert_eq!(store.define(&object(1, 2, 500), IDLE), Defined::ResourceLimit);
    assert_eq!(store.entry(1).unwrap().sound.generation, 1);
    assert!(matches!(
        store.define(&object(1, 3, 100), IDLE),
        Defined::New { replaced: Some(Entry { sound: SoundId { generation: 1, .. }, .. }), .. }
    ));
    assert_eq!(store.entry(1).unwrap().sound.generation, 3);
}

// Verifies REQ-AUD-009: a store out of slots evicts as it does when it is
// out of bytes.
#[test]
fn a_store_out_of_slots_evicts_the_oldest() {
    let mut arena = [0_u8; 1000];
    let mut store = Store::<2>::new(&mut arena);
    store.define(&object(1, 1, 10), IDLE);
    store.define(&object(2, 1, 10), IDLE);
    assert!(matches!(store.define(&object(3, 1, 10), IDLE), Defined::New { .. }));
    assert!(store.entry(1).is_none() && store.entry(2).is_some() && store.entry(3).is_some());
}

#[test]
fn a_reserved_codec_is_not_stored() {
    let mut arena = [0_u8; 100];
    let mut store = Store::<2>::new(&mut arena);
    assert_eq!(store.define(&Object { codec: None, ..object(1, 1, 10) }, IDLE), Defined::Ignored);
}

/// A DATA record of a live block at `index`.
fn live(index: u16, block: &[u8]) -> Data<'_> {
    Data { handle: 1, generation: 1, first_block: index, blocks: block }
}

// Verifies REQ-AUD-014.
#[test]
fn a_live_sound_keeps_its_newest_blocks_in_a_ring() {
    // A ring of 32 blocks of 100 bytes, its bitmap and its index table.
    let mut arena = [0_u8; 3_300];
    let mut store = Store::<2>::new(&mut arena);
    let sound = SoundId { handle: 1, generation: 1 };
    assert!(matches!(store.define(&Object { samples: 0, blocks: 0, ..object(1, 1, 0) }, IDLE), Defined::New { .. }));
    let entry = *store.entry(1).unwrap();
    assert!(entry.live && entry.blocks == crate::LIVE_RING_BLOCKS && entry.newest.is_none());
    let block = |value: u8| [value; 100];
    assert_eq!(store.put(&live(5, &block(5))), Stored::Blocks { first: 5, count: 1 });
    assert_eq!(store.live_block(sound, 5), Some(&block(5)[..]));
    // Block 37 takes block 5's slot: 5 is gone and 37 is the newest.
    assert_eq!(store.put(&live(37, &block(37))), Stored::Blocks { first: 37, count: 1 });
    assert_eq!((store.live_block(sound, 5), store.live_block(sound, 37)), (None, Some(&block(37)[..])));
    assert_eq!(store.entry(1).unwrap().newest, Some(37));
    // A block older than the ring is dropped; an older one inside it is kept.
    assert_eq!(store.put(&live(5, &block(5))), Stored::Repeat);
    assert_eq!(store.live_block(sound, 37), Some(&block(37)[..]));
    assert_eq!(store.put(&live(30, &block(30))), Stored::Blocks { first: 30, count: 1 });
    assert_eq!(store.entry(1).unwrap().newest, Some(37), "an older block is not the newest");
    // The index wraps: 65 535 then 0.
    assert_eq!(store.put(&live(u16::MAX, &block(1))), Stored::Repeat, "far older by serial arithmetic");
    // A part block is malformed: live blocks are always whole.
    assert_eq!(store.put(&live(38, &[0; 40])), Stored::Malformed);
    // The same block with other bytes is a conflict.
    assert_eq!(store.put(&live(37, &block(9))), Stored::Conflict);
}

// Verifies REQ-AUD-014.
#[test]
fn a_live_index_wraps_from_the_largest_to_zero() {
    let mut arena = [0_u8; 3_300];
    let mut store = Store::<2>::new(&mut arena);
    let sound = SoundId { handle: 1, generation: 1 };
    store.define(&Object { samples: 0, blocks: 0, ..object(1, 1, 0) }, IDLE);
    store.put(&live(u16::MAX, &[1; 100]));
    assert_eq!(store.put(&live(0, &[2; 100])), Stored::Blocks { first: 0, count: 1 });
    assert_eq!(store.entry(1).unwrap().newest, Some(0));
    assert_eq!(store.live_block(sound, u16::MAX), Some(&[1; 100][..]));
}
