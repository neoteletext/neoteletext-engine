//! The teletext audio profile (area AUD, `docs/spec/design/audio.md`):
//! sounds carried as records over EN 300 708 independent data lines, a
//! network or a file; a store that keeps them compressed; and control by
//! channels, where the latest command wins and a sound can belong to a page.
//!
//! The crate decides what plays and when; it mixes nothing and drives no
//! hardware. Its output is voice operations on a [`port::VoiceSink`], which
//! the port supplies (REQ-AUD-018): a software mixer on a desktop, or the
//! tellyboy driver on a microcontroller.
//!
//! Everything here is `no_std` and allocation-free; every buffer has a size
//! fixed by its type. Only the `encode` feature, for publishing tools,
//! allocates.
#![no_std]
// REQ-SEC-015: only the C interface crate may contain unsafe code.
#![forbid(unsafe_code)]
// REQ-SEC-003: everything here reads untrusted input.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

#[cfg(feature = "encode")]
extern crate alloc;
#[cfg(test)]
extern crate std;

pub mod control;
pub mod crc;
#[cfg(feature = "encode")]
pub mod encode;
#[cfg(feature = "idl")]
pub mod idl;
#[cfg(feature = "ima")]
pub mod ima;
pub mod port;
pub mod receiver;
pub mod record;
pub mod store;

/// The profile version every record carries (REQ-AUD-003).
pub const PROFILE_VERSION: u8 = 2;
/// The largest record, header and CRC included (REQ-AUD-003): 1 KB keeps
/// the reassembly buffer of a microcontroller small.
pub const MAX_RECORD: usize = 1024;
/// Record header: version, type and a two-byte body length.
pub const RECORD_HEADER: usize = 4;
/// The CRC-32 after the body.
pub const RECORD_CRC: usize = 4;
/// The largest body.
pub const MAX_BODY: usize = MAX_RECORD - RECORD_HEADER - RECORD_CRC;
/// Channels, each with one voice (REQ-AUD-010).
pub const CHANNELS: u8 = 4;
/// No level changes faster than this (REQ-AUD-015): tellyboy measured a
/// level step as a click and glides every change over 5 ms
/// (`TB_AUDIO_DECLICK_MS`).
pub const MIN_FADE_MS: u16 = 5;
/// The largest fade a command may ask for, ten seconds.
pub const MAX_FADE_MS: u16 = 10_000;
/// Gain is a whole percentage (REQ-AUD-015).
pub const MAX_GAIN: u8 = 100;
/// The page field's value for a command that belongs to the whole service.
pub const SERVICE_SCOPE: u16 = 0xFFFF;
/// A progressive start waits for this many milliseconds of contiguous
/// sound (REQ-AUD-013).
pub const PROGRESSIVE_BUFFER_MS: u32 = 40;

/// A sound's codec (REQ-AUD-001).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Codec {
    /// Linear PCM, signed 16-bit little-endian.
    Pcm16,
    /// IMA ADPCM in WAVE blocks (format tag 0x0011).
    ImaAdpcm,
}

impl Codec {
    /// The codec of a code, or `None` for a reserved code, whose object a
    /// receiver ignores.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Pcm16),
            1 => Some(Self::ImaAdpcm),
            _ => None,
        }
    }

    /// The code that names the codec.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Pcm16 => 0,
            Self::ImaAdpcm => 1,
        }
    }

    /// Whether `bytes` is a block size the codec allows (REQ-AUD-005).
    #[must_use]
    pub const fn allows_block(self, bytes: u16) -> bool {
        match self {
            Self::ImaAdpcm => bytes == IMA_BLOCK_SMALL || bytes == IMA_BLOCK_LARGE,
            Self::Pcm16 => bytes >= 2 && bytes <= MAX_PCM_BLOCK && bytes.is_multiple_of(2),
        }
    }

    /// Samples in a whole block of `bytes`.
    #[must_use]
    pub const fn samples_per_block(self, bytes: u16) -> u32 {
        match self {
            // The header's predictor is the first sample; each byte after
            // the 4-byte header holds two (IMA 1992; WAVE format 0x0011).
            Self::ImaAdpcm => (bytes as u32).saturating_sub(IMA_HEADER as u32).saturating_mul(2).saturating_add(1),
            Self::Pcm16 => bytes as u32 / 2,
        }
    }

    /// The bytes of a block holding `samples` samples, at most a whole block.
    #[must_use]
    pub const fn block_bytes_for(self, samples: u32) -> u32 {
        match self {
            // The first sample is the header; the rest are packed two a byte.
            Self::ImaAdpcm => samples.saturating_sub(1).div_ceil(2).saturating_add(IMA_HEADER as u32),
            Self::Pcm16 => samples.saturating_mul(2),
        }
    }
}

/// The IMA ADPCM block header: predictor (2 bytes), step index, reserved.
pub const IMA_HEADER: usize = 4;
/// The two IMA ADPCM block sizes allowed (REQ-AUD-005). 1 024 bytes, ffmpeg's
/// default, would not fit a DATA record.
pub const IMA_BLOCK_SMALL: u16 = 256;
/// The larger IMA ADPCM block.
pub const IMA_BLOCK_LARGE: u16 = 512;
/// The largest PCM block, which leaves room in a DATA record's body.
pub const MAX_PCM_BLOCK: u16 = 1000;
/// The most samples in any block: a 512-byte IMA ADPCM block.
pub const MAX_BLOCK_SAMPLES: usize = 1 + 2 * (IMA_BLOCK_LARGE as usize - IMA_HEADER);

/// The sample rates, by code (REQ-AUD-002): the common rates from 8 to 48 kHz.
pub const SAMPLE_RATES: [u32; 8] = [8_000, 11_025, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000];

/// The rate of a sample-rate code.
#[must_use]
pub fn sample_rate(code: u8) -> Option<u32> {
    SAMPLE_RATES.get(usize::from(code)).copied()
}

/// The code of a sample rate, if the profile allows it.
#[must_use]
pub fn rate_code(hz: u32) -> Option<u8> {
    SAMPLE_RATES.iter().position(|&rate| rate == hz).and_then(|index| u8::try_from(index).ok())
}

/// The attenuations of the duck codes in whole per cent of full level
/// (REQ-AUD-015): EN 300 708 Table 19's 0, 4, 8, 12, 18, 24 and 30 dB, each
/// the percentage nearest to 10^(−dB/20).
pub const DUCK_PERCENT: [u8; 7] = [100, 63, 40, 25, 13, 6, 3];

/// A sequence number newer than another by RFC 1982 serial arithmetic over
/// 16 bits: the difference is positive and less than half the range. Two
/// numbers exactly half the range apart are neither (REQ-AUD-010).
#[must_use]
pub const fn serial_newer(candidate: u16, than: u16) -> bool {
    let difference = candidate.wrapping_sub(than);
    difference != 0 && difference < 0x8000
}
