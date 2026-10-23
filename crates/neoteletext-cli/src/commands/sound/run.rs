//! `neottx sound` in a build with it: the options checked, the WAVE file
//! read, and the packets written, paced for a player when live.

use super::SoundOptions;
use super::wave::read_wave;
use crate::bounded_file::read_bounded;
use crate::commands::{io_failure, page_number, sandbox_closed};
use neoteletext_app::{Failure, exit};
use neoteletext_audio::Codec;
use neoteletext_audio::encode::{Command, Sound, packets};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// The largest WAV file read: 32 MB, almost three minutes of 48 kHz stereo.
#[cfg(feature = "sound")]
const WAV_MAX_FILE_BYTES: usize = 32 * 1024 * 1024;
/// The operation code of a play command (REQ-AUD-006).
#[cfg(feature = "sound")]
const PLAY: u8 = 1;
/// The command's flag for a looping sound.
#[cfg(feature = "sound")]
const LOOP: u8 = neoteletext_audio::record::FLAG_LOOP;

/// `neottx sound`.
pub fn sound(options: &SoundOptions, input: &Path, output: &Path, no_sandbox: bool) -> Result<(), Failure> {
    let page = page_number(&options.page)?;
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
    let mut out = File::create(output).map_err(|error| io_failure(output, error))?;
    // Every file is open (REQ-SEC-017).
    sandbox_closed(no_sandbox)?;
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
            out.write_all(&packet.bytes).map_err(|error| io_failure(output, error))?;
        }
        out.flush().map_err(|error| io_failure(output, error))?;
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
            out.write_all(&packet.bytes).map_err(|error| io_failure(output, error))?;
        }
        written = written.saturating_add(packets.len());
        record_id = 0;
    }
    out.flush().map_err(|error| io_failure(output, error))?;
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
        let due = usize::try_from(
            captured.saturating_mul(u64::from(pace)).checked_div(u64::from(sound.rate_hz.max(1))).unwrap_or(0),
        )
        .unwrap_or(usize::MAX);
        while out.len() < due {
            let cycle = announcement.len().saturating_add(latest.len()).max(1);
            let at = filler.checked_rem(cycle).unwrap_or(0);
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
