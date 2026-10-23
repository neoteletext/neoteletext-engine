//! Starting plays: a play starts once its sound is ready, or waits for it
//! (REQ-AUD-011 to REQ-AUD-013).

use super::{Receiver, duration_ms};
use crate::control::Play;
use crate::port::{Start, VoiceSink};
use crate::store::Entry;
use crate::{CHANNELS, PROGRESSIVE_BUFFER_MS};

impl<const OBJECTS: usize, const SLOTS: usize> Receiver<'_, OBJECTS, SLOTS> {
    /// Starts a play now if its sound is ready, or keeps it waiting.
    pub(super) fn try_start(&mut self, index: u8, play: Play, sink: &mut dyn VoiceSink) {
        let ready = self
            .store
            .entry(play.sound.handle)
            .copied()
            .filter(|entry| entry.sound == play.sound && !entry.conflict)
            .filter(|entry| {
                if entry.live {
                    // A live sound plays while its blocks keep coming.
                    return entry.newest.is_some()
                        && self.now_ms.saturating_sub(entry.arrived_ms) < crate::LIVE_STALL_MS;
                }
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
        channel.playing = Some(play);
        if entry.live {
            if !entry.at_sink {
                // Join at the newest block: a live sound is heard as it is now.
                sink.stream(entry.sound, entry.rate_hz);
                self.store.set_at_sink(entry.sound, true);
                self.store.set_live_cursor(entry.sound, entry.newest.unwrap_or(0), entry.arrived_ms);
                self.send_live(entry.sound, sink);
            }
        } else if !entry.at_sink {
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

    pub(super) fn start_waiting(&mut self, sink: &mut dyn VoiceSink) {
        for index in 0..CHANNELS {
            if let Some(play) = self.channels.get(usize::from(index)).and_then(|channel| channel.waiting) {
                self.try_start(index, play, sink);
            }
        }
    }

    pub(super) fn start_armed(&mut self, sink: &mut dyn VoiceSink) {
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
}
