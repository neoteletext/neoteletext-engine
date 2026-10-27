//! The receiver: records in, voice operations out. It follows one audio
//! service, keeps sounds in its store, applies each channel's latest
//! command, and plays a page's sounds while that page is open and the user
//! allows page sounds (REQ-AUD-009 to REQ-AUD-013).

use crate::CHANNELS;
use crate::control::{Channel, Play, fade};
#[cfg(feature = "idl")]
use crate::idl::Reassembler;
use crate::port::{SoundId, VoiceSink};
use crate::record::{Control, Operation, Record, parse};
use crate::store::{Defined, Entry, Store, Stored};

#[cfg(feature = "idl")]
mod carriage;
#[cfg(any(feature = "pcm16", feature = "ima", feature = "opus"))]
mod fade;
mod file;
mod loss;
#[cfg(feature = "opus")]
mod opus;
mod play;
mod send;

#[cfg(feature = "idl")]
pub use carriage::MessageError;

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
    /// Blocks given as silence, or concealed by their decoder, because they
    /// had not arrived when due (REQ-AUD-013): a live sound's block given up
    /// once three newer have come, and a stored sound's block not at the
    /// sink when its play reached it by the receiver's clock. Each is also
    /// reported to the sink ([`VoiceSink::concealed`]).
    pub concealed: u32,
    /// Commands ignored as repeats or late arrivals.
    pub stale_commands: u32,
    /// Blocks that could not be decoded.
    pub decode_failures: u32,
    /// OBJECT records of a codec this receiver does not decode, a reserved
    /// one or one its build leaves out (decision E.131): each is ignored,
    /// and counted at each repeat.
    pub unsupported: u32,
    /// WebSocket messages refused whole: not a whole number of packets
    /// (REQ-AUD-008).
    pub messages_refused: u32,
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
    /// The port's time, in milliseconds (REQ-AUD-014).
    now_ms: u64,
    /// Opus sounds' decoders (decision E.124).
    #[cfg(feature = "opus")]
    opus: opus::Lanes,
    /// Each channel's stored-sound play, followed for the `concealed`
    /// report (REQ-AUD-013).
    progress: [Option<loss::Progress>; CHANNELS as usize],
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
            now_ms: 0,
            #[cfg(feature = "opus")]
            opus: opus::Lanes::new(),
            progress: [None; CHANNELS as usize],
        }
    }

    /// The port's time, in milliseconds from any start: a live sound with no
    /// new block for [`crate::LIVE_STALL_MS`] fades out, and its play waits
    /// for it to resume (REQ-AUD-014).
    pub fn tick(&mut self, now_ms: u64, sink: &mut dyn VoiceSink) {
        self.now_ms = now_ms;
        // One pass over the store: a port ticks on every message it
        // handles, so this is paid constantly (decision E.103).
        let mut stalled: [Option<Entry>; OBJECTS] = [None; OBJECTS];
        for (slot, entry) in stalled.iter_mut().zip(self.store.entries()) {
            if entry.live && entry.at_sink && now_ms.saturating_sub(entry.arrived_ms) >= crate::LIVE_STALL_MS {
                *slot = Some(*entry);
            }
        }
        for entry in stalled.iter().flatten() {
            for index in 0..CHANNELS {
                let Some(channel) = self.channels.get_mut(usize::from(index)) else { continue };
                if let Some(play) = channel.playing.filter(|play| play.sound == entry.sound) {
                    sink.stop(index, crate::LIVE_STALL_FADE_MS);
                    channel.playing = None;
                    channel.waiting = Some(play);
                }
            }
            sink.forget(entry.sound);
            self.store.set_at_sink(entry.sound, false);
            #[cfg(feature = "opus")]
            self.opus.release(entry.sound);
        }
        self.judge(sink);
    }

    /// When the receiver next needs [`Receiver::tick`], in its milliseconds:
    /// the moment the live sound at the sink whose last block came longest
    /// ago would stall (REQ-AUD-014), or a stored sound's play reaches a
    /// block that has not arrived, to report it concealed (REQ-AUD-013); or
    /// `None` while neither can happen, when it needs no tick at all. A port
    /// sleeps until then, rather than ticking on a fixed interval (decision
    /// E.98).
    #[must_use]
    pub fn next_tick_ms(&self) -> Option<u64> {
        let stall = self
            .store
            .entries()
            .filter(|entry| entry.live && entry.at_sink)
            .map(|entry| entry.arrived_ms.saturating_add(crate::LIVE_STALL_MS))
            .min();
        [stall, self.next_due_ms()].into_iter().flatten().min()
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
        #[cfg(feature = "opus")]
        self.opus.clear();
        self.channels = [Channel::default(); CHANNELS as usize];
        self.progress = [None; CHANNELS as usize];
    }

    /// The sink has dropped every sound and stopped every voice, as when
    /// another tab became the one heard: sounds are sent again before they
    /// next play.
    pub fn sink_lost(&mut self) {
        self.store.sink_lost();
        #[cfg(feature = "opus")]
        self.opus.clear();
        for channel in &mut self.channels {
            channel.playing_page = None;
            channel.playing = None;
        }
        self.progress = [None; CHANNELS as usize];
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
                    Defined::New { replaced: Some(old), .. } if old.at_sink => {
                        sink.forget(old.sound);
                        #[cfg(feature = "opus")]
                        self.opus.release(old.sound);
                    }
                    Defined::New { .. } | Defined::Repeat => {}
                    Defined::Ignored => bump(&mut self.statistics.unsupported),
                    Defined::Conflict => bump(&mut self.statistics.conflicts),
                    Defined::ResourceLimit => bump(&mut self.statistics.resource_limits),
                }
                // An announced sound may be what a waiting play needs.
                self.start_waiting(sink);
            }
            Record::Data(data) => match self.store.put(&data) {
                Stored::Blocks { first, count } => {
                    if let Some(entry) = self.store.entry(data.handle).copied() {
                        if entry.live {
                            self.store.set_live_cursor(entry.sound, entry.next, self.now_ms);
                            if entry.at_sink {
                                self.send_live(entry.sound, sink);
                            }
                        } else if entry.at_sink {
                            self.send_blocks(&entry, first, count, sink);
                        }
                    }
                    self.start_waiting(sink);
                }
                Stored::Conflict => bump(&mut self.statistics.conflicts),
                Stored::Repeat | Stored::Unknown | Stored::Malformed => {}
            },
            Record::Control(control) => self.command(&control, sink),
        }
        self.judge(sink);
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
                if let Some(progress) = self.progress.get_mut(usize::from(index)) {
                    *progress = None;
                }
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
                    if let Some(progress) = self.progress.get_mut(usize::from(index)) {
                        *progress = None;
                    }
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
                channel.playing = None;
                sink.stop(index, fade(control.fade_ms));
                self.end_progress(index);
            }
            Operation::Gain => sink.gain(index, control.gain, fade(control.fade_ms)),
        }
    }
}

/// Counts one more.
fn bump(counter: &mut u32) {
    *counter = counter.saturating_add(1);
}

/// Whether a channel still needs a sound: playing, armed or waiting for it.
fn needs(channel: &Channel, sound: SoundId) -> bool {
    [channel.armed, channel.waiting, channel.playing].iter().flatten().any(|play| play.sound == sound)
}

/// Milliseconds in a second, for a rate's samples.
const MS_PER_SECOND: u32 = 1000;

/// A sound's whole length in milliseconds.
fn duration_ms(entry: &Entry) -> u32 {
    entry.samples.saturating_mul(MS_PER_SECOND).checked_div(entry.rate_hz).unwrap_or(0)
}

#[cfg(test)]
mod tests;
