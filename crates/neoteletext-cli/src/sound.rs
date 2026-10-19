//! `neottx sound --page MPP INPUT.wav OUTPUT.t42` (REQ-CLI-011,
//! REQ-AUD-020): a sound as the audio profile's broadcast packets, with the
//! command that plays it, ready to add to a T42 stream. With `--live`, the
//! sound is sent as a live one (REQ-AUD-014), each block at the moment a
//! player of the file reaches it.

#[cfg(feature = "sound")]
use crate::font::read_bounded;
use neoteletext_app::Failure;
#[cfg(feature = "sound")]
use neoteletext_app::exit;
#[cfg(feature = "sound")]
use neoteletext_app::sandbox::{self, Policy};
#[cfg(feature = "sound")]
use neoteletext_audio::Codec;
#[cfg(feature = "sound")]
use neoteletext_audio::encode::{Command, Sound, packets};
#[cfg(feature = "sound")]
use std::fs::File;
#[cfg(feature = "sound")]
use std::io::Write;
use std::path::Path;

/// The largest WAV file read: 32 MB, almost three minutes of 48 kHz stereo.
#[cfg(feature = "sound")]
const WAV_MAX_FILE_BYTES: usize = 32 * 1024 * 1024;
/// WAVE format tag of linear PCM.
#[cfg(feature = "sound")]
const WAVE_PCM: u16 = 1;
/// The operation code of a play command (REQ-AUD-006).
#[cfg(feature = "sound")]
const PLAY: u8 = 1;
/// The command's flag for a looping sound.
#[cfg(feature = "sound")]
const LOOP: u8 = neoteletext_audio::record::FLAG_LOOP;
/// The pace at which the browser plays a T42 file: 64 packets every 0.2
/// seconds, the engine's `DEFAULT_FETCH_INTERVAL_US`.
pub const PLAYER_PACKETS_PER_SECOND: u32 = 64 * 5;

/// What `neottx sound` writes. A build without `sound` refuses the command
/// before reading any of it.
#[derive(Clone, Debug)]
#[cfg_attr(not(feature = "sound"), allow(dead_code))]
pub struct SoundOptions {
    /// The page the sound belongs to, 100 to 8FF.
    pub page: String,
    /// The channel, 0 to 3.
    pub channel: u8,
    /// The gain, 0 to 100 per cent.
    pub gain: u8,
    /// `ima` or `pcm`.
    pub codec: String,
    /// The sound's handle.
    pub handle: u8,
    /// Repeat until stopped.
    pub looped: bool,
    /// Times the whole sequence is written, as a carousel repeats it.
    pub repeat: u8,
    /// Send the sound live, paced for a player.
    pub live: bool,
    /// The packets a second the player reads.
    pub pace: u32,
}

/// `neottx sound`.
#[cfg(feature = "sound")]
pub fn sound(options: &SoundOptions, input: &Path, output: &Path, no_sandbox: bool) -> Result<(), Failure> {
    let page = u16::from_str_radix(&options.page, 16)
        .ok()
        .filter(|&mpp| neoteletext_core::types::PageAddress::from_mpp(mpp).is_some())
        .ok_or_else(|| (exit::USAGE, format!("{}: not a page number 100 to 8FF", options.page)))?;
    let codec = match options.codec.as_str() {
        "ima" => Codec::ImaAdpcm,
        "pcm" => Codec::Pcm16,
        other => return Err((exit::USAGE, format!("{other}: the codec is ima or pcm"))),
    };
    if options.channel >= neoteletext_audio::CHANNELS
        || options.gain > neoteletext_audio::MAX_GAIN
        || options.repeat == 0
    {
        return Err((exit::USAGE, "the channel is 0 to 3, the gain 0 to 100 and the repeat at least 1".to_owned()));
    }
    let bytes = read_bounded(input, WAV_MAX_FILE_BYTES)?;
    let mut out = File::create(output).map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
    // Every file is open (REQ-SEC-017).
    sandbox::apply(Policy { writable_directory: None, network: false, readable: &[] }, no_sandbox)
        .map_err(|message| (exit::INPUT, message))?;
    let wave = read_wave(&bytes).map_err(|message| (exit::INPUT, format!("{}: {message}", input.display())))?;
    let block = match codec {
        Codec::ImaAdpcm => neoteletext_audio::IMA_BLOCK_LARGE,
        Codec::Pcm16 => neoteletext_audio::MAX_PCM_BLOCK,
    };
    let refused = |error| {
        (
            exit::INPUT,
            format!("{}: {error:?}; the rate must be one of {:?}", input.display(), neoteletext_audio::SAMPLE_RATES),
        )
    };
    let command = Command {
        channel: options.channel,
        sequence: 1,
        operation: PLAY,
        handle: options.handle,
        generation: 1,
        gain: options.gain,
        fade_ms: 0,
        flags: if options.looped && !options.live { LOOP } else { 0 },
        duck: 0,
        page: Some(page),
    };
    if options.live {
        // A live sound's blocks are small, so each goes as soon as it exists.
        let block = match codec {
            Codec::ImaAdpcm => neoteletext_audio::IMA_BLOCK_SMALL,
            Codec::Pcm16 => block,
        };
        let sound = Sound::live(options.handle, 1, codec, wave.rate_hz, block).map_err(refused)?;
        let packets = live_packets(&sound, &command, &wave.samples, options.repeat, options.pace)
            .map_err(|message| (exit::INPUT, format!("{}: {message}", input.display())))?;
        for packet in &packets {
            out.write_all(&packet.bytes).map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
        }
        out.flush().map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
        eprintln!(
            "{}: {} samples at {} Hz sent live {} times, {} packets at {} a second for page {page:03X}",
            output.display(),
            wave.samples.len(),
            wave.rate_hz,
            options.repeat,
            packets.len(),
            options.pace
        );
        return Ok(());
    }
    let sound = Sound::new(options.handle, 1, codec, wave.rate_hz, block, &wave.samples).map_err(refused)?;
    let mut records = vec![sound.object_record().map_err(|error| (exit::INPUT, format!("{error:?}")))?];
    records.extend(sound.data_records());
    records.push(command.record());
    let mut written = 0_usize;
    let mut record_id = 0;
    for _ in 0..options.repeat {
        // A repeat keeps its record ids: the same records again.
        let (packets, _) = packets(0, record_id, &records);
        for packet in &packets {
            out.write_all(&packet.bytes).map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
        }
        written = written.saturating_add(packets.len());
        record_id = 0;
    }
    out.flush().map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
    eprintln!(
        "{}: {} samples at {} Hz, {} bytes of {}, {written} packets for page {page:03X}",
        output.display(),
        wave.samples.len(),
        wave.rate_hz,
        sound.data.len(),
        options.codec
    );
    Ok(())
}

/// A live sound's packets as a player reading `pace` packets a second
/// meets them: the announcement and the play first, then each block when
/// its last sample has been captured, `repeat` runs of `samples` in a row.
/// The packets between are repeats of the announcement and of the latest
/// block, as a carousel repeats records, which also mends a lost packet.
#[cfg(feature = "sound")]
fn live_packets(
    sound: &Sound,
    command: &Command,
    samples: &[i16],
    repeat: u8,
    pace: u32,
) -> Result<Vec<neoteletext_core::packet::Packet>, String> {
    let announce = vec![sound.object_record().map_err(|error| format!("{error:?}"))?, command.record()];
    let (announcement, mut record_id) = packets(0, 0, &announce);
    let per_block = usize::try_from(sound.samples_per_block()).map_err(|_| "block size")?;
    let all: Vec<i16> =
        samples.iter().copied().cycle().take(samples.len().saturating_mul(usize::from(repeat))).collect();
    let mut out = announcement.clone();
    let mut latest: Vec<neoteletext_core::packet::Packet> = Vec::new();
    let mut filler = 0_usize;
    for (index, block) in (0_u16..).zip(all.chunks(per_block)) {
        // The block is whole once its last sample is captured.
        let captured =
            u64::try_from((usize::from(index).saturating_add(1)).saturating_mul(per_block)).unwrap_or(u64::MAX);
        let due = usize::try_from(captured.saturating_mul(u64::from(pace)) / u64::from(sound.rate_hz.max(1)))
            .unwrap_or(usize::MAX);
        while out.len() < due {
            let cycle = announcement.len().saturating_add(latest.len()).max(1);
            let at = filler % cycle;
            let packet = announcement.get(at).or_else(|| latest.get(at.saturating_sub(announcement.len())));
            out.extend(packet.copied());
            filler = filler.saturating_add(1);
        }
        let (block_packets, next) = packets(0, record_id, &sound.live_records(index, block));
        if out.len().saturating_sub(due) > block_packets.len().saturating_mul(2) {
            return Err(format!(
                "{} Hz in {}-byte blocks needs more than {pace} packets a second",
                sound.rate_hz, sound.block_bytes
            ));
        }
        record_id = next;
        out.extend_from_slice(&block_packets);
        latest = block_packets;
    }
    Ok(out)
}

/// `neottx sound` in a build without it.
#[cfg(not(feature = "sound"))]
pub fn sound(_: &SoundOptions, _: &Path, _: &Path, _: bool) -> Result<(), Failure> {
    Err(neoteletext_app::not_in_build("writing sounds", "sound"))
}

/// A WAVE file's samples, mixed down to mono.
#[cfg(feature = "sound")]
#[derive(Debug, PartialEq, Eq)]
struct Wave {
    rate_hz: u32,
    samples: Vec<i16>,
}

/// Reads a RIFF WAVE file of 16-bit linear PCM, mono or stereo; stereo is
/// mixed down as (left + right) / 2, as tellyboy does.
#[cfg(feature = "sound")]
fn read_wave(bytes: &[u8]) -> Result<Wave, String> {
    let rest = bytes.strip_prefix(b"RIFF").ok_or("not a RIFF file")?;
    let rest = rest.get(4..).and_then(|rest| rest.strip_prefix(b"WAVE")).ok_or("not a WAVE file")?;
    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut chunks = rest;
    while let Some(header) = chunks.get(..8) {
        let size = usize::try_from(u32::from_le_bytes([header[4], header[5], header[6], header[7]]))
            .map_err(|_| "chunk size")?;
        let body = chunks.get(8..8_usize.saturating_add(size)).ok_or("a chunk runs past the end of the file")?;
        match &header[..4] {
            b"fmt " => {
                let field =
                    |at: usize| body.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or("short fmt chunk");
                let rate =
                    body.get(4..8).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or("short fmt chunk")?;
                format = Some((field(0)?, field(2)?, rate, field(14)?));
            }
            b"data" => {
                let (tag, channels, rate_hz, bits) = format.ok_or("the data comes before the fmt chunk")?;
                if tag != WAVE_PCM || bits != 16 || !(1..=2).contains(&channels) {
                    return Err(format!(
                        "16-bit PCM, mono or stereo, is needed: format {tag}, {bits} bits, {channels} channels"
                    ));
                }
                let frame = usize::from(channels) * 2;
                let samples = body
                    .chunks_exact(frame)
                    .map(|frame| {
                        let left = i32::from(i16::from_le_bytes([frame[0], frame[1]]));
                        let right = frame.get(2..4).map_or(left, |b| i32::from(i16::from_le_bytes([b[0], b[1]])));
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

#[cfg(all(test, feature = "sound"))]
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
