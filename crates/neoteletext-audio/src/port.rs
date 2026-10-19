//! The voice sink: what a port supplies to play sound (REQ-AUD-018). The
//! receiver tells it which sounds exist, gives it their samples as 16-bit
//! PCM, and starts, stops and ramps voices on numbered channels. The sink
//! mixes and plays; it decides nothing.

/// A sound as the sink knows it: its handle and generation. A newer
/// generation of a handle is a different sound.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct SoundId {
    /// The handle of the OBJECT record.
    pub handle: u8,
    /// Its generation.
    pub generation: u16,
}

/// A sound's shape: what the sink needs to hold its samples.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SoundFormat {
    /// Samples a second: one of [`crate::SAMPLE_RATES`].
    pub rate_hz: u32,
    /// Samples in the whole sound.
    pub samples: u32,
}

/// Starting a sound on a channel (REQ-AUD-011, REQ-AUD-015).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Start {
    /// The sound.
    pub sound: SoundId,
    /// 0 to 100 per cent.
    pub gain: u8,
    /// The ramp in and, for the sound it replaces, out; at least
    /// [`crate::MIN_FADE_MS`].
    pub fade_ms: u16,
    /// Repeat until stopped.
    pub looped: bool,
    /// While this sound plays, every other channel plays at this per cent of
    /// its gain: one of [`crate::DUCK_PERCENT`].
    pub duck_percent: u8,
    /// Wait for the channel's current sound to end instead of replacing it.
    /// A channel holds one such waiting sound; a newer one replaces it.
    pub after_current: bool,
}

/// What a port supplies to play sound.
pub trait VoiceSink {
    /// How many channels the sink plays, at most [`crate::CHANNELS`]. The
    /// receiver plays the lowest-numbered channels and ignores the rest
    /// (REQ-AUD-016).
    fn channels(&self) -> u8 {
        crate::CHANNELS
    }

    /// A sound exists: the sink makes room for its samples, silent until
    /// they arrive.
    fn define(&mut self, sound: SoundId, format: SoundFormat);

    /// Samples of a sound, starting at sample `first`.
    fn samples(&mut self, sound: SoundId, first: u32, pcm: &[i16]);

    /// The sound is gone. A voice already playing it plays to its end; a
    /// live sound's voice plays what it holds, then ends.
    fn forget(&mut self, sound: SoundId);

    /// A live sound exists (REQ-AUD-014): its samples come without end, in
    /// order, through [`VoiceSink::stream_samples`]. The sink buffers them,
    /// starts a voice once enough are held, and follows the sender's clock.
    fn stream(&mut self, sound: SoundId, rate_hz: u32);

    /// The next samples of a live sound, in order.
    fn stream_samples(&mut self, sound: SoundId, pcm: &[i16]);

    /// Starts a sound on a channel, ramping out what the channel played.
    fn start(&mut self, channel: u8, start: Start);

    /// Ramps the channel to silence and stops it, dropping any sound waiting
    /// to follow.
    fn stop(&mut self, channel: u8, fade_ms: u16);

    /// Ramps the channel to a new gain.
    fn gain(&mut self, channel: u8, gain: u8, fade_ms: u16);
}
