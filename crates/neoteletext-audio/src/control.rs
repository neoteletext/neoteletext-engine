//! A channel's command state (REQ-AUD-010 to REQ-AUD-012): the latest
//! command wins, in constant memory.

use crate::port::SoundId;
use crate::record::{Control, FLAG_AFTER_CURRENT, FLAG_LOOP, FLAG_PROGRESSIVE};
use crate::{DUCK_PERCENT, MIN_FADE_MS, serial_newer};

/// A play command, kept until its sound is ready or its page is shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Play {
    /// The sound.
    pub sound: SoundId,
    /// 0 to 100 per cent.
    pub gain: u8,
    /// The ramp, at least [`MIN_FADE_MS`].
    pub fade_ms: u16,
    /// Start before the whole sound has arrived.
    pub progressive: bool,
    /// Repeat until stopped.
    pub looped: bool,
    /// Wait for the current sound to end.
    pub after_current: bool,
    /// The other channels' level while this plays, in per cent.
    pub duck_percent: u8,
    /// The page the play belongs to, or `None` for the whole service.
    pub page: Option<u16>,
}

impl Play {
    /// The play a CONTROL record asks for.
    #[must_use]
    pub fn from_control(control: &Control) -> Self {
        Self {
            sound: SoundId { handle: control.handle, generation: control.generation },
            gain: control.gain,
            fade_ms: fade(control.fade_ms),
            progressive: control.flags & FLAG_PROGRESSIVE != 0,
            looped: control.flags & FLAG_LOOP != 0,
            after_current: control.flags & FLAG_AFTER_CURRENT != 0,
            duck_percent: DUCK_PERCENT.get(usize::from(control.duck)).copied().unwrap_or(100),
            page: control.page,
        }
    }
}

/// A ramp no shorter than [`MIN_FADE_MS`] (REQ-AUD-015).
#[must_use]
pub fn fade(milliseconds: u16) -> u16 {
    milliseconds.max(MIN_FADE_MS)
}

/// One channel's state.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Channel {
    /// The sequence number of the last command accepted.
    pub last: Option<u16>,
    /// A play that belongs to a page, waiting for that page to be shown.
    pub armed: Option<Play>,
    /// A play accepted but not started: its sound is not ready.
    pub waiting: Option<Play>,
    /// The page whose sound the channel is playing, if it plays one.
    pub playing_page: Option<u16>,
    /// The play the channel last started, while it may still sound.
    pub playing: Option<Play>,
}

impl Channel {
    /// Accepts `sequence` if it is newer than the last command's, by RFC 1982
    /// arithmetic; a repeat or a late command is refused.
    pub fn accept(&mut self, sequence: u16) -> bool {
        let newer = self.last.is_none_or(|last| serial_newer(sequence, last));
        if newer {
            self.last = Some(sequence);
        }
        newer
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    // Tests of REQ-AUD-010 and REQ-AUD-015.
    use super::*;

    #[test]
    fn the_latest_command_wins_and_repeats_do_nothing() {
        let mut channel = Channel::default();
        assert!(channel.accept(5), "the first command seen");
        assert!(!channel.accept(5), "a repeat");
        assert!(!channel.accept(4), "an older command arriving late");
        assert!(channel.accept(6));
        // Across the wrap.
        let mut channel = Channel { last: Some(0xFFFE), ..Channel::default() };
        assert!(channel.accept(1));
        assert!(!channel.accept(0xFFFF));
        // Exactly half the range apart is neither older nor newer.
        let mut channel = Channel { last: Some(0), ..Channel::default() };
        assert!(!channel.accept(0x8000));
    }

    #[test]
    fn fades_are_never_shorter_than_five_milliseconds() {
        assert_eq!(fade(0), 5);
        assert_eq!(fade(200), 200);
    }
}
