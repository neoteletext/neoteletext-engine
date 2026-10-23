//! Reading a RIFF WAVE file of 16-bit linear PCM.

/// WAVE format tag of linear PCM.
const WAVE_PCM: u16 = 1;

/// A WAVE file's samples, mixed down to mono.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Wave {
    rate_hz: u32,
    samples: Vec<i16>,
}

impl Wave {
    /// The sample rate in hertz.
    pub(super) fn rate_hz(&self) -> u32 {
        self.rate_hz
    }

    /// The samples, mono.
    pub(super) fn samples(&self) -> &[i16] {
        &self.samples
    }
}

/// Reads a RIFF WAVE file of 16-bit linear PCM, mono or stereo; stereo is
/// mixed down as (left + right) / 2, as tellyboy does.
pub(super) fn read_wave(bytes: &[u8]) -> Result<Wave, String> {
    let rest = bytes.strip_prefix(b"RIFF").ok_or("not a RIFF file")?;
    let rest = rest.get(4..).and_then(|rest| rest.strip_prefix(b"WAVE")).ok_or("not a WAVE file")?;
    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut chunks = rest;
    while let Some(&[a, b, c, d, size_0, size_1, size_2, size_3]) = chunks.first_chunk::<8>() {
        let size = usize::try_from(u32::from_le_bytes([size_0, size_1, size_2, size_3])).map_err(|_| "chunk size")?;
        let body = chunks.get(8..8_usize.saturating_add(size)).ok_or("a chunk runs past the end of the file")?;
        match &[a, b, c, d] {
            b"fmt " => {
                let field = |at: usize| {
                    body.get(at..)
                        .and_then(<[u8]>::first_chunk::<2>)
                        .map(|&b| u16::from_le_bytes(b))
                        .ok_or("short fmt chunk")
                };
                let rate = body
                    .get(4..)
                    .and_then(<[u8]>::first_chunk::<4>)
                    .map(|&b| u32::from_le_bytes(b))
                    .ok_or("short fmt chunk")?;
                format = Some((field(0)?, field(2)?, rate, field(14)?));
            }
            b"data" => {
                let (tag, channels, rate_hz, bits) = format.ok_or("the data comes before the fmt chunk")?;
                if tag != WAVE_PCM || bits != 16 || !(1..=2).contains(&channels) {
                    return Err(format!(
                        "16-bit PCM, mono or stereo, is needed: format {tag}, {bits} bits, {channels} channels"
                    ));
                }
                // Two bytes a sample, one sample a channel.
                let frame = if channels == 2 { 4 } else { 2 };
                let samples = body
                    .chunks_exact(frame)
                    .map(|frame| {
                        let sample = |at: usize| {
                            frame
                                .get(at..)
                                .and_then(<[u8]>::first_chunk::<2>)
                                .map(|&b| i32::from(i16::from_le_bytes(b)))
                        };
                        let left = sample(0).unwrap_or(0);
                        let right = sample(2).unwrap_or(left);
                        i16::try_from(i32::midpoint(left, right)).unwrap_or(0)
                    })
                    .collect();
                return Ok(Wave { rate_hz, samples });
            }
            _ => {}
        }
        // Chunks are padded to an even length.
        chunks = chunks.get(8_usize.saturating_add(size).saturating_add(size & 1)..).unwrap_or(&[]);
    }
    Err("no data chunk".to_owned())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, reason = "a test")]
mod tests {
    use super::*;

    fn wave(channels: u16, rate: u32, samples: &[i16]) -> Vec<u8> {
        let data: Vec<u8> = samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();
        let mut bytes = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0".to_vec();
        bytes.extend_from_slice(&WAVE_PCM.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"LIST\x03\0\0\0abc\0");
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&data);
        bytes
    }

    #[test]
    fn mono_and_stereo_files_are_read_and_stereo_mixed_down() {
        assert_eq!(read_wave(&wave(1, 16_000, &[1, -2, 3])), Ok(Wave { rate_hz: 16_000, samples: vec![1, -2, 3] }));
        assert_eq!(
            read_wave(&wave(2, 8_000, &[100, 300, -50, -150])),
            Ok(Wave { rate_hz: 8_000, samples: vec![200, -100] })
        );
    }

    #[test]
    fn other_files_are_refused() {
        assert!(read_wave(b"RIFX").is_err());
        let mut eight_bit = wave(1, 8_000, &[0]);
        eight_bit[34] = 8;
        assert!(read_wave(&eight_bit).unwrap_err().contains("16-bit"));
        let mut short = wave(1, 8_000, &[1, 2, 3]);
        short.truncate(short.len() - 2);
        assert_eq!(read_wave(&short), Err("a chunk runs past the end of the file".to_owned()));
    }
}
