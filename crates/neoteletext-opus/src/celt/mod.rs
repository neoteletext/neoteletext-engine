//! CELT (RFC 6716 §4.3), the transform layer of Opus, as the profile meets it: mono, the 48 kHz
//! mode's 21 bands, frames of 2.5 ms (LM 0: the silence frame that ends CELT at a Hybrid to SILK
//! change), 5 ms (LM 1: redundant frames and transition concealment) and 20 ms (LM 3), out at 48
//! or 16 kHz.
//!
//! The frame decode (`decoder`) reads the energies (`energy`), the bit allocation (`alloc`) and
//! the band shapes (`bands`, over the pyramid vector quantiser of `pvq`), then synthesises them
//! (`synthesis`, over the inverse MDCT of `transform`) and post-filters them (`postfilter`). A
//! lost frame is concealed (`plc`) from the decoder's history.

mod alloc;
mod bands;
mod decoder;
mod energy;
mod mode;
mod plc;
mod postfilter;
mod pvq;
mod synthesis;
mod tables;
mod transform;

pub(crate) use decoder::CeltDecoder;
pub(crate) use mode::{BITRES, FrameDuration, MAX_BAND, MAX_FRAME, NB_BANDS, OVERLAP, SHORT_BLOCK};
pub(crate) use tables::WINDOW;
