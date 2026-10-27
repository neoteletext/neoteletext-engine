//! Loss (REQ-AUD-013, `design/audio.md` section 8.2): a block that has not
//! arrived when it is due is silence (faded into and out of, `fade`), and
//! reported as `concealed`. A stored sound's play is followed by the
//! receiver's clock, so that each block not at the sink when the play
//! reached it is reported once.

use super::{MS_PER_SECOND, Receiver, bump};
use crate::port::{SoundId, VoiceSink};
use crate::store::Entry;
use crate::{CHANNELS, Codec};

/// A stored sound's play followed by the receiver's clock, so that a block
/// not at the sink when the play reaches it is reported as `concealed`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Progress {
    sound: SoundId,
    /// When the sink was told to start it, in the receiver's milliseconds.
    started_ms: u64,
    /// Blocks before this one have been judged.
    judged: u16,
}

impl Progress {
    /// A play of `entry` started at `now_ms`; `None` when nothing in it can
    /// be missing (a whole or a live sound) or its start is not known (a
    /// play after the current sound).
    pub(super) fn follow(entry: &Entry, after_current: bool, now_ms: u64) -> Option<Self> {
        (!entry.live && !entry.complete() && !after_current).then_some(Self {
            sound: entry.sound,
            started_ms: now_ms,
            judged: 0,
        })
    }
}

/// Samples of `entry` played by `elapsed_ms` after its start.
fn samples_by(entry: &Entry, elapsed_ms: u64) -> u64 {
    elapsed_ms.saturating_mul(u64::from(entry.rate_hz)).checked_div(u64::from(MS_PER_SECOND)).unwrap_or(0)
}

impl<const OBJECTS: usize, const SLOTS: usize> Receiver<'_, OBJECTS, SLOTS> {
    /// Whether block `index` of a stored sound is at the sink: a PCM or IMA
    /// ADPCM block once it has arrived, an Opus block once every block before
    /// it has too, since they are decoded in order.
    fn at_sink(&self, entry: &Entry, index: u16) -> bool {
        if entry.codec == Codec::Opus {
            return index < self.store.contiguous(entry.sound);
        }
        self.store.block(entry.sound, index).is_some()
    }

    /// Judges each followed play up to the receiver's time: every block whose
    /// start has passed and that is not at the sink is reported concealed.
    pub(super) fn judge(&mut self, sink: &mut dyn VoiceSink) {
        for index in 0..CHANNELS {
            let Some(progress) = self.progress.get(usize::from(index)).copied().flatten() else { continue };
            let entry = self.store.entries().find(|entry| entry.sound == progress.sound).copied();
            let Some(entry) = entry else {
                self.end_progress(index);
                continue;
            };
            let played = samples_by(&entry, self.now_ms.saturating_sub(progress.started_ms));
            let started = played.checked_div(u64::from(entry.samples_per_block())).unwrap_or(0).saturating_add(1);
            let due = u16::try_from(started.min(u64::from(entry.blocks))).unwrap_or(entry.blocks);
            for block in progress.judged..due {
                if !self.at_sink(&entry, block) {
                    bump(&mut self.statistics.concealed);
                    sink.concealed(entry.sound, block, entry.block_samples(block));
                }
            }
            if due >= entry.blocks {
                self.end_progress(index);
            } else if let Some(Some(slot)) = self.progress.get_mut(usize::from(index)) {
                slot.judged = due;
            }
        }
    }

    /// When the next followed block that is not at the sink falls due, in
    /// the receiver's milliseconds: the moment its first sample's time
    /// passes.
    pub(super) fn next_due_ms(&self) -> Option<u64> {
        self.progress
            .iter()
            .flatten()
            .filter_map(|progress| {
                let entry = self.store.entries().find(|entry| entry.sound == progress.sound)?;
                let block = (progress.judged..entry.blocks).find(|&block| !self.at_sink(entry, block))?;
                let first = u64::from(block).saturating_mul(u64::from(entry.samples_per_block()));
                let after_ms = first.saturating_mul(u64::from(MS_PER_SECOND)).div_ceil(u64::from(entry.rate_hz.max(1)));
                Some(progress.started_ms.saturating_add(after_ms))
            })
            .min()
    }

    /// Follows the play just started on channel `index`, or nothing.
    pub(super) fn set_progress(&mut self, index: u8, progress: Option<Progress>) {
        if let Some(slot) = self.progress.get_mut(usize::from(index)) {
            *slot = progress;
        }
    }

    /// Stops following channel `index`'s play: it was stopped or replaced.
    pub(super) fn end_progress(&mut self, index: u8) {
        self.set_progress(index, None);
    }
}
