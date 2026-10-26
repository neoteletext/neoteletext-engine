//! The SILK decoder (RFC 6716 §4.2), for the profile: mono, one 20 ms frame a packet, output
//! at 48 kHz. The Opus layer hands it each received frame with its internal rate, or asks
//! it to conceal a lost one; it writes the frame at the output rate.
//!
//! One module per stage: the side information (`side_info`), the excitation (`excitation`),
//! the parameters they give (`parameters`, with `nlsf` and `lpc`), the reconstruction
//! (`synthesis`), concealment (`plc`) and comfort noise (`cng`), the resampler, and the
//! fixed-point operations (`macros`) and tables they share.
//!
//! From the reference decoder. Copyright (c) 2006-2011, Skype Limited, BSD-3-Clause (the licence
//! beside the crate).

mod cng;
mod decoder;
mod excitation;
mod layout;
mod lpc;
mod macros;
mod nlsf;
mod parameters;
mod plc;
mod resampler;
mod scratch;
mod side_info;
mod synthesis;
mod tables;

pub(crate) use decoder::{Frame, SilkDecoder};
pub(crate) use layout::{FrameDuration, InternalRate};

use crate::range::EcDec;

/// A symbol decoded with an 8-bit inverse CDF (RFC 6716 §4.1.3.3) that SILK uses as an index
/// into its tables.
#[inline]
#[allow(
    clippy::cast_sign_loss,
    reason = "the symbol is a position in `icdf`, and every table SILK decodes with is nonempty, so it is 0 or more"
)]
fn decode_index(range: &mut EcDec<'_>, icdf: &[u8]) -> usize {
    range.dec_icdf(icdf, 8) as usize
}

/// `decode_index` with a table of `N` entries, below `N`: what the symbol always is (a range
/// decoder in error answers the last), said so that indexing a table of `N` rows by it needs no
/// bounds check.
#[inline]
fn decode_below<const N: usize>(range: &mut EcDec<'_>, icdf: &[u8; N]) -> usize {
    decode_index(range, icdf).min(N.saturating_sub(1))
}
