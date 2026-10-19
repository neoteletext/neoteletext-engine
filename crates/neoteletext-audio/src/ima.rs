//! IMA ADPCM in WAVE blocks (format tag 0x0011), mono (REQ-AUD-001): the
//! arithmetic of the IMA's 1992 *Recommended Practices for Enhancing Digital
//! Audio Compatibility in Multimedia Systems*. Each block starts with its
//! own predictor and step index, so every block decodes on its own.

use crate::IMA_HEADER;

/// The step-size table: 89 steps, each about 1.1 times the last.
pub const STEP_SIZES: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107,
    118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963,
    1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894,
    6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];

/// How each code moves the step index.
const INDEX_ADJUST: [i8; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

/// The highest step index.
const MAX_INDEX: u8 = 88;
/// A code's sign bit and its three magnitude bits.
const SIGN: u8 = 0b1000;
const MAGNITUDE_4: u8 = 0b0100;
const MAGNITUDE_2: u8 = 0b0010;
const MAGNITUDE_1: u8 = 0b0001;
const NIBBLE: u8 = 0x0F;
const NIBBLE_BITS: u8 = 4;

/// Why a block could not be decoded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImaError {
    /// Shorter than its header.
    Truncated,
    /// A step index beyond the table.
    StepIndex(u8),
    /// The output has no room for the block's samples.
    OutputTooSmall,
}

/// The decoder's state between samples.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct State {
    /// The last sample.
    pub predictor: i16,
    /// The index into [`STEP_SIZES`].
    pub index: u8,
}

impl State {
    /// Decodes one 4-bit code and returns the sample.
    pub fn decode(&mut self, code: u8) -> i16 {
        let step = STEP_SIZES.get(usize::from(self.index)).copied().unwrap_or(0);
        let mut difference = step >> 3;
        if code & MAGNITUDE_4 != 0 {
            difference = difference.saturating_add(step);
        }
        if code & MAGNITUDE_2 != 0 {
            difference = difference.saturating_add(step >> 1);
        }
        if code & MAGNITUDE_1 != 0 {
            difference = difference.saturating_add(step >> 2);
        }
        let predictor = i32::from(self.predictor);
        let next =
            if code & SIGN != 0 { predictor.saturating_sub(difference) } else { predictor.saturating_add(difference) };
        self.predictor = i16::try_from(next.clamp(i32::from(i16::MIN), i32::from(i16::MAX))).unwrap_or(0);
        let adjust = INDEX_ADJUST.get(usize::from(code & NIBBLE)).copied().unwrap_or(0);
        self.index = self.index.saturating_add_signed(adjust).min(MAX_INDEX);
        self.predictor
    }
}

/// Decodes one block into `out` and returns how many samples it held: the
/// header's predictor, then two samples a byte, low nibble first.
///
/// # Errors
/// A block shorter than its header, a step index beyond the table, or an
/// output with no room.
pub fn decode_block(block: &[u8], out: &mut [i16]) -> Result<usize, ImaError> {
    let header = block.get(..IMA_HEADER).ok_or(ImaError::Truncated)?;
    let predictor = i16::from_le_bytes([header.first().copied().unwrap_or(0), header.get(1).copied().unwrap_or(0)]);
    let index = header.get(2).copied().unwrap_or(0);
    if index > MAX_INDEX {
        return Err(ImaError::StepIndex(index));
    }
    let codes = block.get(IMA_HEADER..).unwrap_or(&[]);
    let total = codes.len().saturating_mul(2).saturating_add(1);
    if out.len() < total {
        return Err(ImaError::OutputTooSmall);
    }
    let mut state = State { predictor, index };
    let mut samples = out.iter_mut();
    if let Some(first) = samples.next() {
        *first = predictor;
    }
    for &byte in codes {
        for code in [byte & NIBBLE, byte >> NIBBLE_BITS] {
            if let Some(sample) = samples.next() {
                *sample = state.decode(code);
            }
        }
    }
    Ok(total)
}

/// Encodes one sample's difference from the state's prediction as a 4-bit
/// code, and advances the state as a decoder would.
#[cfg(feature = "encode")]
pub fn encode(state: &mut State, sample: i16) -> u8 {
    let step = STEP_SIZES.get(usize::from(state.index)).copied().unwrap_or(0);
    let mut difference = i32::from(sample).saturating_sub(i32::from(state.predictor));
    let mut code = 0;
    if difference < 0 {
        code = SIGN;
        difference = difference.saturating_neg();
    }
    let mut threshold = step;
    for bit in [MAGNITUDE_4, MAGNITUDE_2, MAGNITUDE_1] {
        if difference >= threshold {
            code |= bit;
            difference = difference.saturating_sub(threshold);
        }
        threshold >>= 1;
    }
    // Decoding the code keeps the encoder's prediction equal to the decoder's.
    state.decode(code);
    code
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    // Tests of REQ-AUD-001.
    use super::*;

    #[test]
    fn a_block_decodes_from_its_own_header() {
        // Predictor 1000, step index 10 (step 19); codes 0x7 then 0xF.
        let block = [0xE8, 0x03, 10, 0, 0xF7];
        let mut out = [0_i16; 3];
        assert_eq!(decode_block(&block, &mut out), Ok(3));
        // 0x7: 19/8 + 19 + 9 + 4 = 34 up; index +8 to 18 (step 41).
        // 0xF: 41/8 + 41 + 20 + 10 = 76 down.
        assert_eq!(out, [1000, 1034, 958]);
    }

    #[test]
    fn bad_blocks_are_refused() {
        assert_eq!(decode_block(&[0, 0, 0], &mut [0; 4]), Err(ImaError::Truncated));
        assert_eq!(decode_block(&[0, 0, 89, 0], &mut [0; 4]), Err(ImaError::StepIndex(89)));
        assert_eq!(decode_block(&[0, 0, 0, 0, 0], &mut [0; 2]), Err(ImaError::OutputTooSmall));
    }

    #[test]
    fn the_predictor_saturates_rather_than_wraps() {
        let mut state = State { predictor: i16::MAX - 1, index: MAX_INDEX };
        assert_eq!(state.decode(0x7), i16::MAX);
        let mut state = State { predictor: i16::MIN + 1, index: MAX_INDEX };
        assert_eq!(state.decode(0xF), i16::MIN);
    }

    #[cfg(feature = "encode")]
    #[test]
    fn encoding_tracks_a_tone_closely() {
        let mut encoder = State { predictor: 0, index: 0 };
        let mut decoder = encoder;
        let mut worst = 0_i32;
        // After the step size has adapted, the error stays well within the
        // signal's swing.
        // A triangle wave between -16 000 and 16 000, 800 a sample: no
        // jump faster than ADPCM's step can follow.
        for n in 0..2000_i32 {
            let phase = n % 80;
            let sample = i16::try_from(if phase < 40 { phase * 800 - 16_000 } else { 48_000 - phase * 800 }).unwrap();
            let output = decoder.decode(encode(&mut encoder, sample));
            if n > 100 {
                worst = worst.max((i32::from(sample) - i32::from(output)).abs());
            }
        }
        assert!(worst < 1000, "error {worst}");
    }
}
