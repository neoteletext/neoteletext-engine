//! The terminal side of the viewer: the wait for its events or the
//! source's, and the guard that restores it however the viewer exits.

use crossterm::event::{DisableBracketedPaste, DisableFocusChange, Event, PopKeyboardEnhancementFlags};
use crossterm::{cursor, execute, terminal};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

/// What ends the viewer's wait.
pub(super) enum Wakened {
    /// The terminal reported an event.
    Terminal(Event),
    /// The source has something for its next poll.
    Source,
}

/// Waits up to `wait` for the terminal or the source, and returns the
/// terminal's event if one ended the wait; the source only ends it early.
pub(super) fn next_event(wakes: &Receiver<Wakened>, wait: Duration) -> Option<Event> {
    match wakes.recv_timeout(wait) {
        Ok(Wakened::Terminal(event)) => Some(event),
        Ok(Wakened::Source) | Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => None,
    }
}

/// The longest the viewer waits before it looks at the source again and
/// draws flash and the ticker; a key or a page wakes it at once.
pub(super) const TICK: Duration = Duration::from_millis(40);

/// Restores the terminal however the viewer exits; `enhanced` when the
/// Kitty keyboard protocol's flags were pushed.
pub(super) struct RawGuard {
    pub(super) enhanced: bool,
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        let mut out = std::io::stdout();
        if self.enhanced {
            let _ = execute!(out, PopKeyboardEnhancementFlags);
        }
        let _ = execute!(out, DisableFocusChange, DisableBracketedPaste, cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
