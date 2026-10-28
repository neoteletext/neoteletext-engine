//! What wakes the fetcher loop, and how it waits for the next wake
//! (REQ-INT-008, decision E.97).

use crate::pipe::Frame;
use neoteletext_core::types::Instant;
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

/// What ends the fetcher loop's wait. It sleeps until one of these comes,
/// or until the state machine's next timer, never on a fixed interval, so a
/// key goes and a pushed page comes back at once (REQ-INT-008, decision
/// E.97).
pub enum Wake {
    /// A request from the decoder process.
    Request(Frame),
    /// A connection has bytes, or has ended.
    Network,
    /// The decoder process closed the pipe.
    Ended,
}

/// The loop's wakes: the requests not yet acted on, and whether the
/// decoder process has gone.
pub(super) struct Wakes {
    receiver: Receiver<Wake>,
    requests: VecDeque<Frame>,
    ended: bool,
    /// A socket woke the loop since its turn began: its bytes are looked
    /// at before the loop sleeps again.
    network: bool,
    /// How many times the loop has slept, for the tests to count.
    #[cfg(test)]
    waits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Wakes {
    pub(super) fn new(receiver: Receiver<Wake>) -> Self {
        Self {
            receiver,
            requests: VecDeque::new(),
            ended: false,
            network: false,
            #[cfg(test)]
            waits: std::sync::Arc::default(),
        }
    }

    /// The count of the loop's sleeps, shared, for the tests to read while
    /// the loop runs.
    #[cfg(test)]
    pub(super) fn waits(&self) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
        self.waits.clone()
    }

    fn take(&mut self, wake: Wake) {
        match wake {
            Wake::Request(frame) => self.requests.push_back(frame),
            // The sockets are looked at on the loop's next turn, which then
            // comes at once.
            Wake::Network => self.network = true,
            Wake::Ended => self.ended = true,
        }
    }

    /// The next request that has arrived, without waiting.
    pub(super) fn next_request(&mut self) -> Option<Frame> {
        while let Ok(wake) = self.receiver.try_recv() {
            self.take(wake);
        }
        self.requests.pop_front()
    }

    /// Whether the decoder's next request, already here, is an input
    /// message: the messages waiting at one moment are written together
    /// (REQ-RCH-010).
    pub(super) fn next_is_input(&self) -> bool {
        matches!(self.requests.front(), Some(Frame::Input(_)))
    }

    /// A turn of the loop begins: the sockets are about to be looked at,
    /// so a wake that came before is answered.
    pub(super) fn turn(&mut self) {
        self.network = false;
    }

    /// Whether the decoder process has gone.
    pub(super) fn ended(&self) -> bool {
        self.ended
    }

    /// Sleeps until a request, a socket or the pipe's end wakes the loop,
    /// or until `deadline`, the state machine's next timer; not at all when
    /// a socket woke it since its turn began.
    pub(super) fn wait(&mut self, now: Instant, deadline: Option<Instant>) {
        if self.network {
            return;
        }
        #[cfg(test)]
        self.waits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let woken = match deadline {
            Some(at) => match self.receiver.recv_timeout(Duration::from_micros(at.0.saturating_sub(now.0))) {
                Ok(wake) => Some(wake),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => Some(Wake::Ended),
            },
            None => Some(self.receiver.recv().unwrap_or(Wake::Ended)),
        };
        if let Some(wake) = woken {
            self.take(wake);
        }
    }
}
