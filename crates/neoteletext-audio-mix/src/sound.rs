//! A sound held for playing: its samples, or a live sound's ring of them,
//! and the targets a live sound is buffered to (REQ-AUD-014).

use alloc::vec::Vec;

/// A sound's identity: the audio receiver's handle and generation.
pub type SoundKey = (u8, u16);
/// How much of a live sound a voice keeps buffered ahead of it, in
/// milliseconds: enough to ride out blocks arriving in bursts. A port may
/// hand over packets in pieces (the browser, every 200 ms), and a 256-byte
/// IMA ADPCM block at 8 kHz holds 63 ms, so samples can come 263 ms apart.
pub const STREAM_TARGET_MS: u32 = 400;
/// A live sound's ring of samples, in milliseconds: four times the target,
/// so a burst never overwrites what the voice has yet to play.
pub const STREAM_RING_MS: u32 = 4 * STREAM_TARGET_MS;
/// The furthest a live sound's playback rate moves from its nominal rate
/// to follow the sender's clock, in parts per million: ±5 per cent, where
/// REQ-AUD-014 asks for at least ±1.
pub const STREAM_TRIM_PPM: i64 = 50_000;
/// Parts in a million.
pub(crate) const PPM: i64 = 1_000_000;

/// A sound held for playing.
pub(crate) struct Sound {
    pub(crate) key: SoundKey,
    pub(crate) rate_hz: u32,
    pub(crate) samples: Vec<i16>,
    /// The receiver forgot it: it goes once no voice plays it.
    pub(crate) forgotten: bool,
    /// A live sound: `samples` is a ring, and `written` counts every
    /// sample ever given it.
    pub(crate) stream: bool,
    pub(crate) written: i64,
}

impl Sound {
    /// Sample `index` of the sound; silence outside it, or outside the part
    /// of a live sound its ring still holds.
    pub(crate) fn at(&self, index: i64) -> i16 {
        let length = i64::try_from(self.samples.len()).unwrap_or(i64::MAX);
        let index = if self.stream {
            if index >= self.written || index < self.written.saturating_sub(length) {
                return 0;
            }
            index.checked_rem(length).unwrap_or(0)
        } else {
            index
        };
        usize::try_from(index).ok().and_then(|index| self.samples.get(index)).copied().unwrap_or(0)
    }

    /// Samples of a live sound a voice at `whole` has yet to play.
    pub(crate) fn ahead(&self, whole: i64) -> i64 {
        self.written.saturating_sub(whole)
    }

    /// A live sound's target and ring, in samples.
    pub(crate) fn target(&self) -> i64 {
        i64::from(self.rate_hz.saturating_mul(STREAM_TARGET_MS) / 1000)
    }
}
