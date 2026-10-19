//! The receiver: records in, voice operations out. It follows one audio
//! service, keeps sounds in its store, applies each channel's latest
//! command, and plays a page's sounds while that page is open and the user
//! allows page sounds (REQ-AUD-009 to REQ-AUD-013).

#[cfg(feature = "idl")]
use neoteletext_core::packet::Packet;

use crate::control::{Channel, Play, fade};
#[cfg(feature = "idl")]
use crate::idl::{Reassembler, fragment};
use crate::port::{SoundFormat, SoundId, Start, VoiceSink};
use crate::record::{Control, Operation, Record, parse};
use crate::store::{Defined, Entry, Store, Stored};
use crate::{CHANNELS, Codec, MAX_BLOCK_SAMPLES, PROGRESSIVE_BUFFER_MS};

/// Counts of what the receiver met, for diagnostics (REQ-AUD-021).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Statistics {
    /// Audio packets dropped: damaged or malformed.
    pub packets_dropped: u32,
    /// Records read.
    pub records: u32,
    /// Records refused: a bad frame, CRC or field.
    pub records_refused: u32,
    /// Objects or blocks in conflict with what was stored.
    pub conflicts: u32,
    /// Objects that did not fit the store.
    pub resource_limits: u32,
    /// Live sounds, which this receiver does not play yet.
    pub unsupported: u32,
    /// Commands ignored as repeats or late arrivals.
    pub stale_commands: u32,
    /// Blocks that could not be decoded.
    pub decode_failures: u32,
}

/// The receiver of one audio service.
pub struct Receiver<'a, const OBJECTS: usize, const SLOTS: usize> {
    store: Store<'a, OBJECTS>,
    #[cfg(feature = "idl")]
    reassembler: Reassembler<SLOTS>,
    channels: [Channel; CHANNELS as usize],
    service: u8,
    opened: Option<u16>,
    page_sounds: bool,
    statistics: Statistics,
}

impl<'a, const OBJECTS: usize, const SLOTS: usize> Receiver<'a, OBJECTS, SLOTS> {
    /// A receiver of audio service 0, keeping its sounds in `arena`, with
    /// page sounds on.
    pub fn new(arena: &'a mut [u8]) -> Self {
        Self {
            store: Store::new(arena),
            #[cfg(feature = "idl")]
            reassembler: Reassembler::new(),
            channels: [Channel::default(); CHANNELS as usize],
            service: 0,
            opened: None,
            page_sounds: true,
            statistics: Statistics::default(),
        }
    }

    /// What the receiver has met.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }

    /// The store, for diagnostics.
    #[must_use]
    pub fn store(&self) -> &Store<'a, OBJECTS> {
        &self.store
    }

    /// Follows broadcast audio service `service` (0 to 7) instead, forgetting
    /// everything of the one before.
    pub fn follow_service(&mut self, service: u8, sink: &mut dyn VoiceSink) {
        if service != self.service {
            self.service = service;
            self.reset(sink);
        }
    }

    /// Forgets every sound and command, as on a change of service
    /// (REQ-AUD-010), and stops every channel.
    pub fn reset(&mut self, sink: &mut dyn VoiceSink) {
        for channel in 0..CHANNELS {
            sink.stop(channel, crate::MIN_FADE_MS);
        }
        for entry in self.store.entries().filter(|entry| entry.at_sink) {
            sink.forget(entry.sound);
        }
        self.store.clear();
        #[cfg(feature = "idl")]
        self.reassembler.clear();
        self.channels = [Channel::default(); CHANNELS as usize];
    }

    /// The sink has dropped every sound and stopped every voice, as when
    /// another tab became the one heard: sounds are sent again before they
    /// next play.
    pub fn sink_lost(&mut self) {
        self.store.sink_lost();
        for channel in &mut self.channels {
            channel.playing_page = None;
        }
    }

    /// Reads one packet. Returns whether it was an audio packet, so the
    /// port can leave the rest to the teletext decoder.
    #[cfg(feature = "idl")]
    pub fn push_packet(&mut self, packet: &Packet, sink: &mut dyn VoiceSink) -> bool {
        let Some(result) = fragment(packet) else { return false };
        match result {
            Ok(fragment) if fragment.service == self.service => {
                if let Some((_, record)) = self.reassembler.push(&fragment) {
                    let mut copy = [0_u8; crate::MAX_RECORD];
                    let length = record.len().min(copy.len());
                    if let (Some(target), Some(source)) = (copy.get_mut(..length), record.get(..length)) {
                        target.copy_from_slice(source);
                    }
                    self.push_record(copy.get(..length).unwrap_or(&[]), sink);
                }
            }
            Ok(_) => {}
            Err(_) => bump(&mut self.statistics.packets_dropped),
        }
        true
    }

    /// Reads one whole record, from any carriage.
    pub fn push_record(&mut self, bytes: &[u8], sink: &mut dyn VoiceSink) {
        let Ok(record) = parse(bytes) else {
            bump(&mut self.statistics.records_refused);
            return;
        };
        bump(&mut self.statistics.records);
        match record {
            Record::Object(object) => {
                let channels = self.channels;
                let busy = |sound: SoundId| channels.iter().any(|channel| needs(channel, sound));
                match self.store.define(&object, &busy) {
                    Defined::New { replaced: Some(old), .. } if old.at_sink => sink.forget(old.sound),
                    Defined::New { .. } | Defined::Repeat | Defined::Ignored => {}
                    Defined::Conflict => bump(&mut self.statistics.conflicts),
                    Defined::ResourceLimit => bump(&mut self.statistics.resource_limits),
                    Defined::Unsupported => bump(&mut self.statistics.unsupported),
                }
                // An announced sound may be what a waiting play needs.
                self.start_waiting(sink);
            }
            Record::Data(data) => match self.store.put(&data) {
                Stored::Blocks { first, count } => {
                    if let Some(entry) = self.store.entry(data.handle).copied()
                        && entry.at_sink
                    {
                        for index in first..first.saturating_add(count) {
                            self.send_block(&entry, index, sink);
                        }
                    }
                    self.start_waiting(sink);
                }
                Stored::Conflict => bump(&mut self.statistics.conflicts),
                Stored::Repeat | Stored::Unknown | Stored::Malformed => {}
            },
            Record::Control(control) => self.command(&control, sink),
        }
    }

    /// The user opened `page`, or none: page sounds follow it (REQ-AUD-012).
    pub fn open_page(&mut self, page: Option<u16>, sink: &mut dyn VoiceSink) {
        if page == self.opened {
            return;
        }
        let left = self.opened;
        self.opened = page;
        for index in 0..CHANNELS {
            let Some(channel) = self.channels.get_mut(usize::from(index)) else { continue };
            if left.is_some() && channel.playing_page == left {
                channel.playing_page = None;
                sink.stop(index, crate::MIN_FADE_MS);
            }
            if channel.waiting.is_some_and(|play| play.page.is_some()) {
                channel.waiting = None;
            }
        }
        if self.page_sounds {
            self.start_armed(sink);
        }
    }

    /// The user's page sounds setting (REQ-AUD-012).
    pub fn set_page_sounds(&mut self, on: bool, sink: &mut dyn VoiceSink) {
        if on == self.page_sounds {
            return;
        }
        self.page_sounds = on;
        if on {
            self.start_armed(sink);
        } else {
            for index in 0..CHANNELS {
                if let Some(channel) = self.channels.get_mut(usize::from(index))
                    && channel.playing_page.is_some()
                {
                    channel.playing_page = None;
                    channel.waiting = None;
                    sink.stop(index, crate::MIN_FADE_MS);
                }
            }
        }
    }

    fn command(&mut self, control: &Control, sink: &mut dyn VoiceSink) {
        let index = control.channel;
        let Some(channel) = self.channels.get_mut(usize::from(index)) else { return };
        if !channel.accept(control.sequence) {
            bump(&mut self.statistics.stale_commands);
            return;
        }
        // Channels beyond the sink's are ignored (REQ-AUD-016).
        if index >= sink.channels() {
            return;
        }
        match control.operation {
            Operation::Play => {
                let play = Play::from_control(control);
                channel.waiting = None;
                if let Some(page) = play.page {
                    channel.armed = Some(play);
                    if self.opened == Some(page) && self.page_sounds {
                        self.try_start(index, play, sink);
                    }
                } else {
                    channel.armed = None;
                    self.try_start(index, play, sink);
                }
            }
            Operation::Stop => {
                channel.armed = None;
                channel.waiting = None;
                channel.playing_page = None;
                sink.stop(index, fade(control.fade_ms));
            }
            Operation::Gain => sink.gain(index, control.gain, fade(control.fade_ms)),
        }
    }

    /// Starts a play now if its sound is ready, or keeps it waiting.
    fn try_start(&mut self, index: u8, play: Play, sink: &mut dyn VoiceSink) {
        let ready = self
            .store
            .entry(play.sound.handle)
            .copied()
            .filter(|entry| entry.sound == play.sound && !entry.conflict)
            .filter(|entry| {
                entry.complete()
                    || (play.progressive && self.buffered_ms(entry) >= PROGRESSIVE_BUFFER_MS.min(duration_ms(entry)))
            });
        let Some(channel) = self.channels.get_mut(usize::from(index)) else { return };
        let Some(entry) = ready else {
            channel.waiting = Some(play);
            return;
        };
        channel.waiting = None;
        channel.playing_page = play.page;
        if !entry.at_sink {
            self.send_sound(&entry, sink);
        }
        sink.start(
            index,
            Start {
                sound: play.sound,
                gain: play.gain,
                fade_ms: play.fade_ms,
                looped: play.looped,
                duck_percent: play.duck_percent,
                after_current: play.after_current,
            },
        );
    }

    fn start_waiting(&mut self, sink: &mut dyn VoiceSink) {
        for index in 0..CHANNELS {
            if let Some(play) = self.channels.get(usize::from(index)).and_then(|channel| channel.waiting) {
                self.try_start(index, play, sink);
            }
        }
    }

    fn start_armed(&mut self, sink: &mut dyn VoiceSink) {
        let Some(opened) = self.opened else { return };
        for index in 0..CHANNELS.min(sink.channels()) {
            if let Some(play) = self.channels.get(usize::from(index)).and_then(|channel| channel.armed)
                && play.page == Some(opened)
            {
                self.try_start(index, play, sink);
            }
        }
    }

    /// Milliseconds of sound received contiguously from the start.
    fn buffered_ms(&self, entry: &Entry) -> u32 {
        let blocks = self.store.contiguous(entry.sound);
        let samples = (0..blocks).map(|index| entry.block_samples(index)).fold(0_u32, u32::saturating_add);
        samples.saturating_mul(1000).checked_div(entry.rate_hz).unwrap_or(0)
    }

    /// Defines a sound at the sink and sends every block received so far.
    fn send_sound(&mut self, entry: &Entry, sink: &mut dyn VoiceSink) {
        sink.define(entry.sound, SoundFormat { rate_hz: entry.rate_hz, samples: entry.samples });
        self.store.set_at_sink(entry.sound, true);
        for index in 0..entry.blocks {
            self.send_block(entry, index, sink);
        }
    }

    /// Decodes block `index` and sends its samples, if it has arrived.
    fn send_block(&mut self, entry: &Entry, index: u16, sink: &mut dyn VoiceSink) {
        let Some(block) = self.store.block(entry.sound, index) else { return };
        let mut pcm = [0_i16; MAX_BLOCK_SAMPLES];
        let decoded = match entry.codec {
            Codec::Pcm16 => {
                let mut count: usize = 0;
                for (sample, bytes) in pcm.iter_mut().zip(block.as_chunks::<2>().0) {
                    *sample = i16::from_le_bytes(*bytes);
                    count = count.saturating_add(1);
                }
                Some(count)
            }
            #[cfg(feature = "ima")]
            Codec::ImaAdpcm => crate::ima::decode_block(block, &mut pcm).ok(),
            #[cfg(not(feature = "ima"))]
            Codec::ImaAdpcm => None,
        };
        let Some(count) = decoded else {
            bump(&mut self.statistics.decode_failures);
            return;
        };
        // The final block's last code may carry a padding sample.
        let samples = usize::try_from(entry.block_samples(index)).unwrap_or(0).min(count);
        let first = u32::from(index).saturating_mul(entry.samples_per_block());
        sink.samples(entry.sound, first, pcm.get(..samples).unwrap_or(&[]));
    }
}

/// Counts one more.
fn bump(counter: &mut u32) {
    *counter = counter.saturating_add(1);
}

/// Whether a channel still needs a sound: playing, armed or waiting for it.
fn needs(channel: &Channel, sound: SoundId) -> bool {
    channel.armed.is_some_and(|play| play.sound == sound) || channel.waiting.is_some_and(|play| play.sound == sound)
}

/// A sound's whole length in milliseconds.
fn duration_ms(entry: &Entry) -> u32 {
    entry.samples.saturating_mul(1000).checked_div(entry.rate_hz).unwrap_or(0)
}
