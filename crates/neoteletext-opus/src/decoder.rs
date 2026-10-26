//! The Opus layer: one 20 ms mono frame a packet, decoded by SILK, by CELT, or by both (Hybrid),
//! with the redundant CELT frames, state resets and cross-fades of a change between them
//! (RFC 6716 §4.5), and the concealment of lost packets.
//!
//! Ported from libopus 1.5.2: Copyright (c) 2010 Xiph.Org Foundation, Skype Limited,
//! BSD-3-Clause (see `LICENSE-libopus`).

use crate::celt::{CeltDecoder, FrameDuration as CeltDuration, OVERLAP, WINDOW};
use crate::fixed::{Q15ONE, add32, mac16_16, mult16_16, mult16_16_q15, sat16, shr32};
use crate::packet::{Bandwidth, MAX_PACKET, Mode, Packet, PacketError};
use crate::range::EcDec;
use crate::silk::{Frame as SilkFrame, FrameDuration as SilkDuration, InternalRate, SilkDecoder};

/// The output rate, the profile's only one.
pub const SAMPLE_RATE: u32 = 48_000;
/// Samples in a frame: 20 ms at `SAMPLE_RATE`.
pub const FRAME_SAMPLES: usize = 960;
/// Samples in 10 ms: the shortest SILK frame, which a 5 ms concealment draws on.
const TEN_MS: usize = FRAME_SAMPLES / 2;
/// Samples in 5 ms: a redundant CELT frame (RFC 6716 §4.5.1), and a transition's concealment.
const FIVE_MS: usize = FRAME_SAMPLES / 4;
/// Samples in 2.5 ms: a cross-fade, over CELT's window (RFC 6716 §4.5.1.4).
const FADE: usize = FRAME_SAMPLES / 8;
const _: () = assert!(FADE == OVERLAP && FADE == WINDOW.len());

/// The first CELT band a Hybrid frame codes: SILK codes up to 8 kHz (RFC 6716 §3.1).
const HYBRID_START_BAND: usize = 17;
/// RFC 6716 §4.5.1.1: redundancy is possible only with at least 17 bits left after SILK, and in
/// a Hybrid frame 37: 20 more for the flag and the size.
const REDUNDANCY_MIN_BITS: i32 = 17;
const HYBRID_REDUNDANCY_EXTRA_BITS: i32 = 20;
/// RFC 6716 §4.5.1.1, Table 64: the Hybrid redundancy flag is 1 with probability 1/4096.
const REDUNDANCY_FLAG_LOGP: u32 = 12;
/// RFC 6716 §4.5.1.2, Table 65: the position flag is uniform.
const POSITION_FLAG_LOGP: u32 = 1;
/// RFC 6716 §4.5.1.3: a Hybrid redundant frame is 2 bytes plus a value below 256.
const HYBRID_REDUNDANCY_MIN_BYTES: usize = 2;
const HYBRID_REDUNDANCY_SIZE_RANGE: u32 = 256;
/// A 2.5 ms CELT frame of silence: all-ones bytes set its silence flag (RFC 6716 §4.3,
/// Table 56), so it adds only what is left in CELT's overlap (RFC 6716 §4.5).
static SILENCE: [u8; 2] = [0xFF, 0xFF];

/// The bits in `bytes` bytes of a frame, which `MAX_PACKET` keeps far inside `i32`.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "at most 8 × MAX_PACKET, asserted below"
)]
const fn bits_in(bytes: usize) -> i32 {
    bytes.saturating_mul(8) as i32
}
const _: () = assert!(8 * MAX_PACKET < i32::MAX as usize);

/// Why a packet produced no frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// The packet is outside the profile; nothing was decoded and the decoder is unchanged.
    Packet(PacketError),
    /// The frame read past its end. The decoder has moved on as if it had been decoded, and the
    /// output holds what was decoded up to the error.
    Corrupt,
}

/// The mode of the last frame, which decides the next frame's transition (RFC 6716 §4.5).
#[derive(Clone, Copy)]
struct Previous {
    /// `None` until the first frame.
    mode: Option<Mode>,
    /// The last frame ended in a redundant CELT frame: CELT continues, without a reset, in the
    /// next frame (RFC 6716 §4.5.2).
    redundant_end: bool,
}

impl Previous {
    const NONE: Self = Self { mode: None, redundant_end: false };
}

/// The two codecs, and the SILK output a 5 ms concealment adds to CELT's.
#[derive(Clone)]
struct Layers {
    silk: SilkDecoder,
    celt: CeltDecoder,
    silk_scratch: [i16; TEN_MS],
}

/// An Opus decoder for the profile: mono, 20 ms frames, 48 kHz out.
#[derive(Clone)]
pub struct Decoder {
    layers: Layers,
    previous: Previous,
    /// The frame's redundant CELT frame or its transition concealment, never both: a frame with
    /// redundancy needs no concealment (RFC 6716 §4.5).
    side: [i16; FIVE_MS],
    final_range: u32,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

/// The SILK rate of a SILK-only frame's bandwidth; a Hybrid frame's SILK is always wideband
/// (RFC 6716 §3.1).
const fn silk_rate(mode: Mode, bandwidth: Bandwidth) -> InternalRate {
    match (mode, bandwidth) {
        (Mode::Silk, Bandwidth::Narrowband) => InternalRate::Narrowband,
        (Mode::Silk, Bandwidth::Mediumband) => InternalRate::Mediumband,
        _ => InternalRate::Wideband,
    }
}

/// One sample of a cross-fade at `window`, CELT's window, squared to be power-complementary.
#[inline]
#[allow(
    clippy::cast_possible_truncation,
    reason = "the weights sum to one in Q15, so the mix stays within the inputs' range"
)]
fn mix(window: i16, outgoing: i16, incoming: i16) -> i16 {
    let weight = mult16_16_q15(i32::from(window), i32::from(window));
    let mixed =
        mac16_16(mult16_16(weight, i32::from(incoming)), i32::from(Q15ONE).wrapping_sub(weight), i32::from(outgoing));
    shr32(mixed, 15) as i16
}

/// Fade `pcm` out into `incoming`.
fn fade_out(pcm: &mut [i16; FADE], incoming: &[i16; FADE]) {
    for ((sample, &next), &window) in pcm.iter_mut().zip(incoming).zip(&WINDOW) {
        *sample = mix(window, *sample, next);
    }
}

/// Fade `outgoing` out into `pcm`.
fn fade_in(outgoing: &[i16; FADE], pcm: &mut [i16; FADE]) {
    for ((sample, &last), &window) in pcm.iter_mut().zip(outgoing).zip(&WINDOW) {
        *sample = mix(window, last, *sample);
    }
}

/// Start a frame with `side`: its first half as is, its second faded into the frame's own
/// (RFC 6716 §4.5.1.4: a redundant frame at the beginning; also a transition's concealment).
fn begin_with(side: &[i16; FIVE_MS], pcm: &mut [i16; FRAME_SAMPLES]) {
    if let (Some((first, second)), Some((start, rest))) =
        (side.split_first_chunk::<FADE>(), pcm.split_first_chunk_mut::<FADE>())
    {
        *start = *first;
        if let (Some(second), Some(target)) = (second.first_chunk::<FADE>(), rest.first_chunk_mut::<FADE>()) {
            fade_in(second, target);
        }
    }
}

impl Layers {
    /// Conceal a lost frame of `out.len()` samples (20 or 5 ms) in the mode of the last one:
    /// SILK and CELT each conceal their part.
    #[cold]
    #[inline(never)]
    fn conceal(&mut self, previous: &mut Previous, out: &mut [i16]) {
        // A frame that ended in a redundant CELT frame is continued by CELT (RFC 6716 §4.5.2).
        let mode = if previous.redundant_end { Some(Mode::Celt) } else { previous.mode };
        let duration = match out.len() {
            FRAME_SAMPLES => Some(CeltDuration::TwentyMs),
            FIVE_MS => Some(CeltDuration::FiveMs),
            _ => None,
        };
        let (Some(mode), Some(duration)) = (mode, duration) else {
            out.fill(0);
            return;
        };
        // SILK's 20 ms concealment goes straight to the output and CELT's is added to it; a 5 ms
        // one takes the start of a 10 ms SILK frame, added to CELT's.
        let silk_first = out.len() >= TEN_MS;
        if mode != Mode::Celt {
            // The mode is the last frame's, which had SILK, so SILK's state needs no reset.
            let (silk_duration, silk_out) = if silk_first {
                (SilkDuration::Ms20, &mut *out)
            } else {
                (SilkDuration::Ms10, &mut self.silk_scratch[..])
            };
            if self.silk.decode(SilkFrame::Lost { duration: silk_duration }, silk_out).is_err() {
                silk_out.fill(0);
            }
        }
        self.celt.set_start_band(if mode == Mode::Celt { 0 } else { HYBRID_START_BAND });
        if mode == Mode::Silk {
            if !silk_first {
                out.fill(0);
            }
        } else {
            // Without data CELT conceals, which cannot fail.
            // CELT adds to SILK's concealment only; alone, it writes the frame.
            let _ = self.celt.decode_with_ec(None, out, duration, None, silk_first && mode != Mode::Celt);
        }
        if mode != Mode::Celt && !silk_first {
            for (sample, &silk) in out.iter_mut().zip(&self.silk_scratch) {
                *sample = sat16(add32(i32::from(*sample), i32::from(silk)));
            }
        }
        *previous = Previous { mode: Some(mode), redundant_end: false };
    }
}

impl Decoder {
    /// A decoder before its first packet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            layers: Layers { silk: SilkDecoder::new(), celt: CeltDecoder::new(), silk_scratch: [0; TEN_MS] },
            previous: Previous::NONE,
            side: [0; FIVE_MS],
            final_range: 0,
        }
    }

    /// Return to the state before the first packet.
    pub fn reset(&mut self) {
        self.layers.silk.reset();
        self.layers.celt.reset();
        self.previous = Previous::NONE;
        self.final_range = 0;
    }

    /// The range decoder's final state for the last packet, which matches the encoder's
    /// (RFC 6716 §4.1.6.1); 0 after a concealed frame.
    #[must_use]
    pub fn final_range(&self) -> u32 {
        self.final_range
    }

    /// Conceal a lost packet.
    pub fn conceal(&mut self, pcm: &mut [i16; FRAME_SAMPLES]) {
        self.layers.conceal(&mut self.previous, pcm);
        self.final_range = 0;
    }

    /// Decode `packet` into `pcm`. A packet outside the profile is refused before anything
    /// changes. A frame of a byte or less is one the encoder left out, and is concealed
    /// (RFC 6716 §3.2.1).
    pub fn decode(&mut self, packet: &[u8], pcm: &mut [i16; FRAME_SAMPLES]) -> Result<(), DecodeError> {
        let packet = Packet::parse(packet).map_err(DecodeError::Packet)?;
        if packet.frame.len() <= 1 {
            self.conceal(pcm);
            return Ok(());
        }
        self.decode_frame(&packet, pcm)
    }

    /// Decode a received frame of two bytes or more.
    fn decode_frame(&mut self, packet: &Packet<'_>, pcm: &mut [i16; FRAME_SAMPLES]) -> Result<(), DecodeError> {
        let Self { layers, previous, side, final_range } = self;
        let mode = packet.mode;
        let frame = packet.frame;
        let mut range = EcDec::new(frame);

        // A change between CELT-only and a mode with SILK, without redundancy, is bridged by
        // concealing 5 ms in the old mode and fading it into the new (RFC 6716 §4.5).
        let mut transition = previous.mode.is_some_and(|before| {
            (mode == Mode::Celt && before != Mode::Celt && !previous.redundant_end)
                || (mode != Mode::Celt && before == Mode::Celt)
        });
        if transition && mode == Mode::Celt {
            layers.conceal(previous, side);
        }

        if mode != Mode::Celt {
            // RFC 6716 §4.5.2: SILK is reset at every change from CELT-only.
            if previous.mode == Some(Mode::Celt) {
                layers.silk.reset();
            }
            // SILK writes straight into the output, and CELT adds to it.
            let rate = silk_rate(mode, packet.bandwidth);
            if layers.silk.decode(SilkFrame::Received { range: &mut range, rate }, pcm).is_err() {
                return Err(DecodeError::Corrupt);
            }
        }

        // RFC 6716 §4.5.1: a frame with SILK may carry a redundant 5 ms CELT frame at its end;
        // the position flag says whether it belongs at the frame's beginning (RFC 6716 §4.5.1.2).
        let mut length = frame.len();
        let mut redundancy: Option<(&[u8], bool)> = None;
        let hybrid = mode == Mode::Hybrid;
        let needed = if hybrid { REDUNDANCY_MIN_BITS + HYBRID_REDUNDANCY_EXTRA_BITS } else { REDUNDANCY_MIN_BITS };
        if mode != Mode::Celt
            && range.tell().saturating_add(needed) <= bits_in(length)
            && (!hybrid || range.dec_bit_logp(REDUNDANCY_FLAG_LOGP))
        {
            let at_beginning = range.dec_bit_logp(POSITION_FLAG_LOGP);
            let bytes = if hybrid {
                usize::try_from(range.dec_uint(HYBRID_REDUNDANCY_SIZE_RANGE))
                    .map_or(usize::MAX, |size| size.saturating_add(HYBRID_REDUNDANCY_MIN_BYTES))
            } else {
                length.saturating_sub(usize::try_from(range.tell().saturating_add(7) >> 3).unwrap_or(length))
            };
            // RFC 6716 §4.5.1.3: a size beyond what the frame holds makes it invalid; what was
            // decoded is kept, and the rest is read as an empty frame.
            match length.checked_sub(bytes).filter(|&rest| bits_in(rest) >= range.tell()) {
                Some(rest) => {
                    range.shrink_storage(bytes);
                    redundancy = frame.get(rest..length).map(|data| (data, at_beginning));
                    length = rest;
                }
                None => length = 0,
            }
        }
        if redundancy.is_some() {
            transition = false;
        }
        if transition && mode != Mode::Celt {
            layers.conceal(previous, side);
        }

        layers.celt.set_end_band(packet.bandwidth.end_band());
        let mut redundant_range = 0;
        if let Some((data, true)) = redundancy {
            // RFC 6716 §4.5.2: CELT is not reset for a redundant frame at the beginning.
            layers.celt.set_start_band(0);
            let _ = layers.celt.decode_with_ec(Some(data), side, CeltDuration::FiveMs, None, false);
            redundant_range = layers.celt.final_range();
        }
        layers.celt.set_start_band(if mode == Mode::Celt { 0 } else { HYBRID_START_BAND });

        let mut result = Ok(());
        if mode == Mode::Silk {
            // RFC 6716 §4.5: from Hybrid to SILK-only, a silent frame adds CELT's overlap, unless
            // CELT goes on from a redundant frame at the last frame's end into one at this one's
            // beginning.
            let continued = matches!(redundancy, Some((_, true))) && previous.redundant_end;
            if previous.mode == Some(Mode::Hybrid) && !continued {
                layers.celt.set_start_band(0);
                let _ = layers.celt.decode_with_ec(Some(&SILENCE), pcm, CeltDuration::TwoAndAHalfMs, None, true);
            }
        } else {
            // RFC 6716 §4.5.2: CELT is reset at a change of mode, except one with redundancy.
            if previous.mode.is_some_and(|before| before != mode) && !previous.redundant_end {
                layers.celt.reset();
            }
            result = layers
                .celt
                .decode_with_ec(frame.get(..length), pcm, CeltDuration::TwentyMs, Some(&mut range), mode != Mode::Celt)
                .map_err(|_| DecodeError::Corrupt);
        }

        match redundancy {
            Some((data, false)) => {
                // RFC 6716 §4.5.2: CELT is reset for a redundant frame at the end, whose second
                // half the frame's end fades into (RFC 6716 §4.5.1.4).
                layers.celt.reset();
                layers.celt.set_start_band(0);
                let _ = layers.celt.decode_with_ec(Some(data), side, CeltDuration::FiveMs, None, false);
                redundant_range = layers.celt.final_range();
                if let (Some(end), Some(incoming)) = (pcm.last_chunk_mut::<FADE>(), side.last_chunk::<FADE>()) {
                    fade_out(end, incoming);
                }
            }
            Some((_, true)) if previous.mode != Some(Mode::Silk) || previous.redundant_end => begin_with(side, pcm),
            _ => {}
        }
        if transition {
            begin_with(side, pcm);
        }

        *final_range = if length <= 1 { 0 } else { range.rng() ^ redundant_range };
        *previous = Previous { mode: Some(mode), redundant_end: matches!(redundancy, Some((_, false))) };
        result
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, reason = "tests may panic")]
mod tests {
    use super::{Decoder, FRAME_SAMPLES};

    /// TOC bytes of 20 ms mono frames (RFC 6716 §3.1, Table 2): CELT fullband, Hybrid fullband
    /// and SILK wideband.
    const TOCS: [u8; 3] = [31 << 3, 15 << 3, 9 << 3];

    /// Pseudo-random packets of every mode, each followed by a loss now and then: whatever their
    /// bytes, they are frames the decoder must decode or conceal.
    fn packets() -> impl Iterator<Item = Option<[u8; 41]>> {
        let mut state = 0x1234_5678u32;
        (0..300).map(move |n| {
            let mut packet = [0u8; 41];
            for byte in &mut packet {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *byte = (state >> 24) as u8;
            }
            packet[0] = TOCS[(n / 20) % TOCS.len()];
            (n % 7 != 6).then_some(packet)
        })
    }

    /// REQ-AUD-023: a frame's output does not depend on what the caller's buffer held: SILK writes
    /// it, and CELT adds to SILK's frame only, never to what was there before (RFC 6716 §4.5).
    #[test]
    fn the_output_never_depends_on_the_buffers_old_contents() {
        let (mut clean, mut dirty) = (Decoder::new(), Decoder::new());
        for (n, packet) in packets().enumerate() {
            let mut zeroed = [0i16; FRAME_SAMPLES];
            let mut filled = [0x5A5Ai16; FRAME_SAMPLES];
            if let Some(packet) = packet {
                let (a, b) = (clean.decode(&packet, &mut zeroed), dirty.decode(&packet, &mut filled));
                assert!(a == b, "frame {n}: results differ");
            } else {
                clean.conceal(&mut zeroed);
                dirty.conceal(&mut filled);
            }
            assert!(zeroed == filled, "frame {n}: the output kept the buffer's old contents");
        }
    }
}
