//! The decoder's working memory: buffers that live only within one call, kept in the decoder
//! rather than on the stack, and shared by the stages that never run together.

use super::layout::{MAX_FRAME_LENGTH, MAX_LPC_ORDER, MAX_LTP_MEMORY, MAX_SUBFRAME_LENGTH};
use super::resampler::RESAMPLER_BUFFER;

/// The 16-bit buffer: the whitened LTP history of reconstruction and concealment, the
/// excitation energies of concealment, and the resampler's upsampled signal.
const SAMPLES: usize = if MAX_LTP_MEMORY > RESAMPLER_BUFFER { MAX_LTP_MEMORY } else { RESAMPLER_BUFFER };

#[derive(Clone)]
pub struct Scratch {
    /// The excitation pulses, from their decoding to the reconstruction.
    pub pulses: [i16; MAX_FRAME_LENGTH],
    pub samples: [i16; SAMPLES],
    /// The LTP state of reconstruction and concealment, and the comfort noise signal.
    pub wide: [i32; MAX_LTP_MEMORY + MAX_FRAME_LENGTH],
    /// The LPC state and one subframe, in reconstruction.
    pub lpc_q14: [i32; MAX_LPC_ORDER + MAX_SUBFRAME_LENGTH],
}

impl Scratch {
    /// Each user writes what it reads before it reads it, so the contents never matter.
    pub const ZERO: Self = Self {
        pulses: [0; MAX_FRAME_LENGTH],
        samples: [0; SAMPLES],
        wide: [0; MAX_LTP_MEMORY + MAX_FRAME_LENGTH],
        lpc_q14: [0; MAX_LPC_ORDER + MAX_SUBFRAME_LENGTH],
    };
}
