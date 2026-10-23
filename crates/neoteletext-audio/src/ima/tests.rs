#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]

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

/// The recommendation's own worked examples, Appendix D sections 6.1
/// (page 30) and 6.2 (page 32): previous sample 0x8700 (-30976), step
/// index 24 (step 73), code 0x3 gives 0x873F (-30913) and index 23
/// (step 66); and sample 0x873F predicted from 0x8700 encodes as 0x3.
#[test]
fn the_recommendations_worked_example_decodes_and_encodes_as_printed() {
    const PREVIOUS: i16 = -30976;
    const PRINTED: i16 = -30913;
    let before = State { predictor: PREVIOUS, index: 24 };
    assert_eq!(STEP_SIZES[24], 73);
    let mut state = before;
    assert_eq!(state.decode(0x3), PRINTED);
    assert_eq!(state, State { predictor: PRINTED, index: 23 });
    assert_eq!(STEP_SIZES[23], 66);
    #[cfg(feature = "encode")]
    {
        let mut state = before;
        assert_eq!(encode(&mut state, PRINTED), 0x3);
        assert_eq!(state.index, 23);
    }
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
