//! The audio profile's constants (REQ-AUD-003 to REQ-AUD-015): the record
//! frame's sizes, the channels, the gain and fade bounds, and the live
//! sound's timing.

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
/// Blocks a live sound keeps (REQ-AUD-014): a ring, the newest replacing the
/// oldest. 32 blocks of 512 bytes are 16 KB, about 2 seconds of 8 kHz IMA
/// ADPCM.
pub const LIVE_RING_BLOCKS: u16 = 32;
/// A live sound fades out after this long with no new block (REQ-AUD-014),
/// the mute of EN 300 708 clause 6.7.1.1 at the time this profile sets.
pub const LIVE_STALL_MS: u64 = 2_000;
/// How a stalled live sound fades out.
pub const LIVE_STALL_FADE_MS: u16 = 100;
/// A live block this many blocks behind the newest that has still not
/// arrived is given up as silence, so a lost block delays nothing
/// (REQ-AUD-014).
pub const LIVE_LOOKAHEAD_BLOCKS: u16 = 3;
/// A progressive start waits for this many milliseconds of contiguous
/// sound (REQ-AUD-013).
pub const PROGRESSIVE_BUFFER_MS: u32 = 40;
/// The Opus decoders a receiver keeps (decision E.124), about 28 KB each:
/// one for a live sound and one for a stored sound arriving at once. A third
/// Opus sound waits for one to free; a stored sound frees its decoder once
/// every block is decoded.
pub const OPUS_DECODERS: usize = 2;
