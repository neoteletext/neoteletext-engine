//! The excitation's quantisation indices (RFC 6716 §4.2.7.8): the rate level, the pulse counts
//! of each 16-sample shell block, their positions by the shell decoder, the extra LSBs, and the
//! signs.
//!
//! From the reference decoder's `silk/decode_pulses.c`, `shell_coder.c`, `code_signs.c`. Copyright
//! (c) 2006-2011, Skype Limited, BSD-3-Clause (the licence beside the crate).

use crate::range::EcDec;

use super::decode_below;
use super::layout::{MAX_FRAME_LENGTH, QuantOffsetType, SignalType};
use super::tables::{
    LSB_ICDF, PULSES_PER_BLOCK_ICDF, RATE_LEVELS_ICDF, SHELL_CODE_TABLE_OFFSETS, SHELL_CODE_TABLE0, SHELL_CODE_TABLE1,
    SHELL_CODE_TABLE2, SHELL_CODE_TABLE3, SIGN_ICDF,
};

/// `SHELL_CODEC_FRAME_LENGTH`: the samples of a shell block.
pub const SHELL_BLOCK: usize = 16;
/// `MAX_NB_SHELL_BLOCKS`.
const MAX_SHELL_BLOCKS: usize = MAX_FRAME_LENGTH / SHELL_BLOCK;
/// `SILK_MAX_PULSES`: a pulse count of one more signals an extra LSB (RFC 6716 §4.2.7.8.2).
const MAX_PULSES: i32 = 16;
/// The most extra LSBs a block can have: at 10 the table that can signal another is replaced.
const MAX_LSBS: i32 = 10;

/// `silk_decode_pulses`: decodes the excitation's pulses, signed, into `pulses`, which holds
/// `frame_length` rounded up to whole shell blocks.
pub fn decode_pulses(
    range: &mut EcDec,
    pulses: &mut [i16; MAX_FRAME_LENGTH],
    signal_type: SignalType,
    quant_offset_type: QuantOffsetType,
    frame_length: usize,
) {
    let mut sum_pulses = [0i32; MAX_SHELL_BLOCKS];
    let mut n_lshifts = [0i32; MAX_SHELL_BLOCKS];
    let blocks = frame_length.div_ceil(SHELL_BLOCK).min(MAX_SHELL_BLOCKS);
    let sum_pulses = sum_pulses.get_mut(..blocks).unwrap_or_default();
    let n_lshifts = n_lshifts.get_mut(..blocks).unwrap_or_default();

    // RFC 6716 §4.2.7.8.1: the rate level
    let rate_level_index = decode_below(range, signal_type.select_by_voicing(&RATE_LEVELS_ICDF));

    // RFC 6716 §4.2.7.8.2: the pulse count of each block, with an extra LSB for each escape
    let [.., escaped_icdf] = &PULSES_PER_BLOCK_ICDF;
    let pulse_count_icdf = PULSES_PER_BLOCK_ICDF.get(rate_level_index).unwrap_or(escaped_icdf);
    for (sum, lshifts) in sum_pulses.iter_mut().zip(n_lshifts.iter_mut()) {
        *sum = range.dec_icdf(pulse_count_icdf, 8);
        while *sum == MAX_PULSES + 1 {
            // At most `MAX_LSBS`, after which the table cannot signal another
            *lshifts = lshifts.wrapping_add(1);
            // When we've already got 10 LSBs, we shift the table to not allow (SILK_MAX_PULSES + 1)
            let icdf = if *lshifts == MAX_LSBS { &escaped_icdf[1..] } else { &escaped_icdf[..] };
            *sum = range.dec_icdf(icdf, 8);
        }
    }

    // RFC 6716 §4.2.7.8.3: the pulse positions
    let (blocks_of_pulses, _) = pulses.as_chunks_mut::<SHELL_BLOCK>();
    for (block, &sum) in blocks_of_pulses.iter_mut().zip(sum_pulses.iter()) {
        if sum > 0 {
            decode_shell_block(block, range, sum);
        } else {
            block.fill(0);
        }
    }

    // RFC 6716 §4.2.7.8.4: the extra LSBs
    for ((block, sum), &lshifts) in blocks_of_pulses.iter_mut().zip(sum_pulses.iter_mut()).zip(n_lshifts.iter()) {
        if lshifts > 0 {
            for pulse in block.iter_mut() {
                let mut abs_q = i32::from(*pulse);
                for _ in 0..lshifts {
                    abs_q = (abs_q << 1).wrapping_add(range.dec_icdf(&LSB_ICDF, 8));
                }
                *pulse = abs_q as i16;
            }
            // Mark the number of pulses non-zero for sign decoding.
            *sum |= lshifts << 5;
        }
    }

    decode_signs(range, blocks_of_pulses, signal_type, quant_offset_type, sum_pulses);
}

/// `decode_split`: the pulse counts of the two halves of a part holding `p` pulses.
#[inline]
fn decode_split(range: &mut EcDec, p: i16, shell_table: &[u8; 152]) -> [i16; 2] {
    // A part holds at most the block's 16 pulses, so the offsets table has an entry for it
    let icdf = usize::try_from(p)
        .ok()
        .filter(|&count| count > 0)
        .and_then(|count| SHELL_CODE_TABLE_OFFSETS.get(count))
        .and_then(|&offset| shell_table.get(usize::from(offset)..));
    if let Some(icdf) = icdf {
        let child1 = range.dec_icdf(icdf, 8) as i16;
        [child1, p.wrapping_sub(child1)]
    } else {
        [0, 0]
    }
}

/// `silk_shell_decoder`: the positions of `pulses4` pulses (at most 16) in a shell block, by
/// splitting it in halves recursively (RFC 6716 §4.2.7.8.3), into `pulses0`.
pub fn decode_shell_block(pulses0: &mut [i16; SHELL_BLOCK], range: &mut EcDec, pulses4: i32) {
    let halves = decode_split(range, pulses4 as i16, &SHELL_CODE_TABLE3);
    // Depth first, as the bits are coded: each half of 8 into quarters of 4, each of those into
    // pairs, each pair into samples
    for (&half, out) in halves.iter().zip(pulses0.as_chunks_mut::<8>().0) {
        let quarters = decode_split(range, half, &SHELL_CODE_TABLE2);
        for (&quarter, out) in quarters.iter().zip(out.as_chunks_mut::<4>().0) {
            let pairs = decode_split(range, quarter, &SHELL_CODE_TABLE1);
            for (&pair, out) in pairs.iter().zip(out.as_chunks_mut::<2>().0) {
                *out = decode_split(range, pair, &SHELL_CODE_TABLE0);
            }
        }
    }
}

/// `silk_decode_signs`: decodes the signs of the nonzero pulses (RFC 6716 §4.2.7.8.5), with a
/// probability that depends on the signal type, the quantisation offset type and the block's
/// pulse count.
fn decode_signs(
    range: &mut EcDec,
    blocks: &mut [[i16; SHELL_BLOCK]],
    signal_type: SignalType,
    quant_offset_type: QuantOffsetType,
    sum_pulses: &[i32],
) {
    let sign_icdf = quant_offset_type.select(signal_type.select(&SIGN_ICDF));
    for (block, &p) in blocks.iter_mut().zip(sum_pulses) {
        if let Ok(count @ 1..) = usize::try_from(p) {
            // The last entry serves every count from 6
            let [.., many] = *sign_icdf;
            let icdf = [sign_icdf.get(count & 0x1F).copied().unwrap_or(many), 0];
            for q in block.iter_mut().filter(|q| **q > 0) {
                // The C multiplies by `2 * sign - 1`: symbol 0 negates
                if range.dec_icdf(&icdf, 8) == 0 {
                    *q = q.wrapping_neg();
                }
            }
        }
    }
}
