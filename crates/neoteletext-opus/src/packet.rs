//! The profile's packets (REQ-AUD-022): Opus mono, one 20 ms frame a packet, at most `MAX_PACKET`
//! bytes.
//! The frame comes bare (RFC 6716 §3.2.2, TOC code 0) or, from an encoder at a constant bit
//! rate, with padding (RFC 6716 §3.2.5, code 3 with a count of 1). Anything else is refused
//! before the decoder's state is touched.

/// The largest packet accepted: four times the 40 bytes a 20 ms frame averages at 16 kbps,
/// room for the encoder's variable rate (peaks of 77 bytes measured at 16 kbps).
pub const MAX_PACKET: usize = 160;

/// Which codec a frame uses (RFC 6716 §3.1, Table 2).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// SILK alone: speech up to wideband.
    Silk,
    /// SILK below 8 kHz and CELT above it.
    Hybrid,
    /// CELT alone.
    Celt,
}

/// The audio bandwidth a frame codes (RFC 6716 §2, Table 1).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bandwidth {
    Narrowband,
    Mediumband,
    Wideband,
    Superwideband,
    Fullband,
}

impl Bandwidth {
    /// The CELT band above the last one this bandwidth codes (RFC 6716 §4.3, Table 55).
    pub(crate) const fn end_band(self) -> usize {
        match self {
            Self::Narrowband => 13,
            Self::Mediumband | Self::Wideband => 17,
            Self::Superwideband => 19,
            Self::Fullband => 21,
        }
    }
}

/// The TOC byte's frame-count code (RFC 6716 §3.1): one frame, or a count in the next byte.
const CODE_MASK: u8 = 0x03;
const ONE_FRAME: u8 = 0;
const SIGNALLED_FRAMES: u8 = 3;
/// The count byte of a code 3 packet (RFC 6716 §3.2.5, Figure 5): padding flag and count.
const PADDING_FLAG: u8 = 0x40;
const COUNT_MASK: u8 = 0x3F;
/// A padding length byte of 255 adds 254 bytes and says another length byte follows.
const PADDING_CONTINUES: u8 = 255;

/// The one frame of a code 3 packet: after the count byte and any padding lengths, less the
/// padding at the end (RFC 6716 §3.2.5). One frame needs no frame lengths, whether the packet
/// is marked constant or variable rate.
fn unpad(bytes: &[u8]) -> Result<&[u8], PacketError> {
    let (&count, mut rest) = bytes.split_first().ok_or(PacketError::Malformed)?;
    match count & COUNT_MASK {
        0 => return Err(PacketError::Malformed),
        1 => {}
        _ => return Err(PacketError::SeveralFrames),
    }
    let mut padding = 0usize;
    if count & PADDING_FLAG != 0 {
        loop {
            let (&length, after) = rest.split_first().ok_or(PacketError::Malformed)?;
            rest = after;
            padding = padding.saturating_add(usize::from(length.min(PADDING_CONTINUES - 1)));
            if length != PADDING_CONTINUES {
                break;
            }
        }
    }
    let frame_length = rest.len().checked_sub(padding).ok_or(PacketError::Malformed)?;
    rest.get(..frame_length).ok_or(PacketError::Malformed)
}

/// Why a packet was refused.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PacketError {
    /// No bytes at all: a loss is told with `Decoder::conceal`.
    Empty,
    /// More than `MAX_PACKET` bytes.
    TooLong,
    /// A stereo packet.
    Stereo,
    /// More than one frame in the packet (TOC code 1 or 2, or code 3 with a count over 1).
    SeveralFrames,
    /// A frame count of 0, or padding longer than the packet (RFC 6716 §3.4, R5 and R7).
    Malformed,
    /// A frame other than 20 ms.
    NotTwentyMs,
}

/// A packet that fits the profile: its frame's mode and bandwidth, and the frame's bytes.
pub(crate) struct Packet<'a> {
    pub(crate) mode: Mode,
    pub(crate) bandwidth: Bandwidth,
    /// The frame after the TOC byte; 0 or 1 bytes is a frame left out (silence suppression).
    pub(crate) frame: &'a [u8],
}

impl<'a> Packet<'a> {
    /// Read `bytes` as a profile packet (RFC 6716 §3.1).
    pub(crate) fn parse(bytes: &'a [u8]) -> Result<Self, PacketError> {
        let (&toc, frame) = bytes.split_first().ok_or(PacketError::Empty)?;
        if bytes.len() > MAX_PACKET {
            return Err(PacketError::TooLong);
        }
        if toc & 0x04 != 0 {
            return Err(PacketError::Stereo);
        }
        let frame = match toc & CODE_MASK {
            ONE_FRAME => frame,
            SIGNALLED_FRAMES => unpad(frame)?,
            _ => return Err(PacketError::SeveralFrames),
        };
        let config = toc >> 3;
        let (mode, bandwidth, twenty_ms) = match config {
            // SILK: NB, MB, WB, each 10, 20, 40, 60 ms.
            0..=11 => {
                let bandwidth = match config / 4 {
                    0 => Bandwidth::Narrowband,
                    1 => Bandwidth::Mediumband,
                    _ => Bandwidth::Wideband,
                };
                (Mode::Silk, bandwidth, config % 4 == 1)
            }
            // Hybrid: SWB, FB, each 10, 20 ms.
            12..=15 => {
                let bandwidth = if config < 14 { Bandwidth::Superwideband } else { Bandwidth::Fullband };
                (Mode::Hybrid, bandwidth, config % 2 == 1)
            }
            // CELT: NB, WB, SWB, FB, each 2.5, 5, 10, 20 ms.
            _ => {
                let bandwidth = match config.wrapping_sub(16) / 4 {
                    0 => Bandwidth::Narrowband,
                    1 => Bandwidth::Wideband,
                    2 => Bandwidth::Superwideband,
                    _ => Bandwidth::Fullband,
                };
                (Mode::Celt, bandwidth, config % 4 == 3)
            }
        };
        if !twenty_ms {
            return Err(PacketError::NotTwentyMs);
        }
        Ok(Self { mode, bandwidth, frame })
    }
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "tests may panic"
)]
mod tests {
    use super::{MAX_PACKET, Mode, Packet, PacketError};

    /// TOC bytes (RFC 6716 §3.1, Table 2): Hybrid fullband 20 ms, mono, code 0 and code 3.
    const HYBRID_FB_20: u8 = 15 << 3;
    const CODE_3: u8 = 3;

    fn refusal(bytes: &[u8]) -> Option<PacketError> {
        Packet::parse(bytes).err()
    }

    #[test]
    fn a_bare_frame_is_the_bytes_after_the_toc() {
        let packet = Packet::parse(&[HYBRID_FB_20, 1, 2, 3]).ok().expect("profile packet");
        assert!(packet.mode == Mode::Hybrid && packet.frame == [1, 2, 3]);
    }

    #[test]
    fn a_padded_frame_loses_its_count_lengths_and_padding() {
        // Count 1 with 3 bytes of padding, and count 1 without.
        let packet = Packet::parse(&[HYBRID_FB_20 | CODE_3, 0x41, 3, 7, 8, 0, 0, 0]).ok().expect("profile packet");
        assert_eq!(packet.frame, [7, 8]);
        let packet = Packet::parse(&[HYBRID_FB_20 | CODE_3, 0x01, 7, 8]).ok().expect("profile packet");
        assert_eq!(packet.frame, [7, 8]);
        // A length byte of 255 is 254 bytes and another length: more than a packet can hold.
        let mut long = [0u8; MAX_PACKET];
        long[..4].copy_from_slice(&[HYBRID_FB_20 | CODE_3, 0x41, 255, 0]);
        assert!(refusal(&long) == Some(PacketError::Malformed));
    }

    /// REQ-AUD-022: every refusal.
    #[test]
    fn everything_outside_the_profile_is_refused() {
        assert!(refusal(&[]) == Some(PacketError::Empty));
        assert!(refusal(&[HYBRID_FB_20; MAX_PACKET + 1]) == Some(PacketError::TooLong));
        assert!(refusal(&[HYBRID_FB_20 | 0x04, 0]) == Some(PacketError::Stereo));
        assert!(refusal(&[HYBRID_FB_20 | 1, 0, 0]) == Some(PacketError::SeveralFrames));
        assert!(refusal(&[HYBRID_FB_20 | 2, 0, 0]) == Some(PacketError::SeveralFrames));
        assert!(refusal(&[HYBRID_FB_20 | CODE_3, 0x02, 0, 0]) == Some(PacketError::SeveralFrames));
        assert!(refusal(&[HYBRID_FB_20 | CODE_3]) == Some(PacketError::Malformed));
        assert!(refusal(&[HYBRID_FB_20 | CODE_3, 0x00, 0]) == Some(PacketError::Malformed));
        assert!(refusal(&[HYBRID_FB_20 | CODE_3, 0x41]) == Some(PacketError::Malformed));
        assert!(refusal(&[HYBRID_FB_20 | CODE_3, 0x41, 5, 0]) == Some(PacketError::Malformed));
        assert!(refusal(&[HYBRID_FB_20 | CODE_3, 0x41, 255]) == Some(PacketError::Malformed));
        // Every configuration but the 20 ms ones (RFC 6716 §3.1, Table 2).
        for config in 0..32u8 {
            let twenty_ms = matches!(config, 1 | 5 | 9 | 13 | 15 | 19 | 23 | 27 | 31);
            let refused = refusal(&[config << 3, 0, 0]);
            assert!(refused == if twenty_ms { None } else { Some(PacketError::NotTwentyMs) }, "configuration {config}");
        }
    }
}
