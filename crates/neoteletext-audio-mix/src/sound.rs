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
    key: SoundKey,
    rate_hz: u32,
    samples: Vec<i16>,
    /// The receiver forgot it: it goes once no voice plays it.
    forgotten: bool,
    /// A live sound: `samples` is a ring, and `written` counts every
    /// sample ever given it.
    stream: bool,
    written: i64,
}

impl Sound {
    /// A sound of `length` samples at `rate_hz`, silent until they arrive.
    pub(crate) fn new(key: SoundKey, rate_hz: u32, length: usize) -> Self {
        Self { key, rate_hz, samples: alloc::vec![0; length], forgotten: false, stream: false, written: 0 }
    }

    /// A live sound at `rate_hz` with a ring of `ring` samples.
    pub(crate) fn live(key: SoundKey, rate_hz: u32, ring: usize) -> Self {
        Self { key, rate_hz, samples: alloc::vec![0; ring], forgotten: false, stream: true, written: 0 }
    }

    /// The sound's identity.
    pub(crate) fn key(&self) -> SoundKey {
        self.key
    }

    /// Whether it is a live sound.
    pub(crate) fn is_live(&self) -> bool {
        self.stream
    }

    /// Whether the receiver forgot it.
    pub(crate) fn is_forgotten(&self) -> bool {
        self.forgotten
    }

    /// The receiver forgot it: it goes once no voice plays it.
    pub(crate) fn forget(&mut self) {
        self.forgotten = true;
    }

    /// Its samples, or a live sound's ring, as a count.
    pub(crate) fn length(&self) -> i64 {
        i64::try_from(self.samples.len()).unwrap_or(i64::MAX)
    }

    /// Every sample ever given a live sound.
    pub(crate) fn written(&self) -> i64 {
        self.written
    }

    /// What a voice starting it needs: its rate and, for a live sound, the
    /// samples written so far.
    pub(crate) fn source(&self) -> (u32, Option<i64>) {
        (self.rate_hz, self.stream.then_some(self.written))
    }

    /// Copies samples in from sample `first`; samples beyond its end are
    /// dropped.
    pub(crate) fn copy_in(&mut self, first: usize, pcm: &[i16]) {
        let end = first.saturating_add(pcm.len()).min(self.samples.len());
        if let (Some(target), Some(source)) = (self.samples.get_mut(first..end), pcm.get(..end.saturating_sub(first))) {
            target.copy_from_slice(source);
        }
    }

    /// Writes a live sound's next samples into its ring.
    pub(crate) fn write_live(&mut self, pcm: &[i16]) {
        let length = i64::try_from(self.samples.len()).unwrap_or(i64::MAX);
        for &value in pcm {
            let slot = usize::try_from(self.written.checked_rem(length).unwrap_or(0)).unwrap_or(0);
            if let Some(place) = self.samples.get_mut(slot) {
                *place = value;
            }
            self.written = self.written.saturating_add(1);
        }
    }

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
