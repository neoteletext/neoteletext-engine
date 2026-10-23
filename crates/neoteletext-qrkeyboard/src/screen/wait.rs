//! The screen's wait: it sleeps on its socket, its wakers and its own next
//! deadline, then takes one turn (REQ-INT-009, decision E.106).

use super::{CLOSE_AUTH_FAILED, CLOSE_FINISHED, CLOSE_NO_INSTANCE, Event, MAX_HOLD, RECONNECT_MAX, Screen};
use crate::net::{Incoming, Wake, later};
use crate::{Error, KEEPALIVE, KEEPALIVE_INTERVAL_MS};
use std::sync::mpsc;
use std::time::{Duration, Instant};

impl Screen {
    /// Waits until there is something to report, a [`Waker`](crate::net::Waker) wakes it, or
    /// `limit` passes (`None` waits for one of the first two), and acts on
    /// what the relay sends: the keepalive, pairing, the phone's messages
    /// and their acknowledgements, and reconnection. It sleeps on nothing
    /// but its socket, its wakers and its own next deadline (the keepalive,
    /// a reconnection, a held key's release; REQ-INT-009, decision E.106).
    /// A woken wait may return no events.
    ///
    /// # Errors
    /// Only a failure to reconnect is not an error; anything else that
    /// breaks the session is reported as [`Event::Finished`].
    pub fn wait(&mut self, limit: Option<Duration>) -> Result<Vec<Event>, Error> {
        let give_up = limit.map(later);
        let mut events = Vec::new();
        let mut woken = false;
        loop {
            if self.finished {
                return Ok(events);
            }
            self.turn(&mut events);
            let now = Instant::now();
            if !events.is_empty() || woken || give_up.is_some_and(|at| now >= at) {
                return Ok(events);
            }
            let until = [give_up, self.next_deadline()].into_iter().flatten().min();
            let wake = match until {
                Some(at) => match self.wakes.recv_timeout(at.saturating_duration_since(now)) {
                    Ok(wake) => Some(wake),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    // The screen holds a sender: never.
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(events),
                },
                None => match self.wakes.recv() {
                    Ok(wake) => Some(wake),
                    Err(mpsc::RecvError) => return Ok(events),
                },
            };
            // Every wake already sent is answered by this turn.
            woken |= wake == Some(Wake::Asked);
            while let Ok(wake) = self.wakes.try_recv() {
                woken |= wake == Wake::Asked;
            }
        }
    }

    /// The soonest moment this screen has work without the relay: the
    /// keepalive while paired, a reconnection, and a held key's release.
    fn next_deadline(&self) -> Option<Instant> {
        let keepalive = self
            .channel
            .as_ref()
            .and_then(|_| self.keepalive.checked_add(Duration::from_millis(KEEPALIVE_INTERVAL_MS)));
        let reconnect = if self.socket.is_none() { Some(self.reconnect_at.unwrap_or_else(Instant::now)) } else { None };
        let release = self.held.iter().filter_map(|(_, since)| since.checked_add(MAX_HOLD)).min();
        [keepalive, reconnect, release].into_iter().flatten().min()
    }

    /// One turn without waiting: what is due, and everything the relay has
    /// sent.
    fn turn(&mut self, events: &mut Vec<Event>) {
        self.release_stale(events);
        if self.socket.is_none() {
            if self.reconnect_at.is_some_and(|at| Instant::now() < at) {
                return;
            }
            if self.connect().is_err() {
                self.reconnect_delay = self.reconnect_delay.saturating_mul(2).min(RECONNECT_MAX);
                self.reconnect_at = Some(later(self.reconnect_delay));
                return;
            }
        }
        if self.channel.is_some() && self.keepalive.elapsed() >= Duration::from_millis(KEEPALIVE_INTERVAL_MS) {
            self.keepalive = Instant::now();
            self.send_text(KEEPALIVE);
        }
        while !self.finished {
            let incoming = match self.socket.as_mut().map(|socket| socket.receive(Duration::ZERO)) {
                Some(Ok(Some(incoming))) => incoming,
                Some(Ok(None)) | None => return,
                Some(Err(_)) => Incoming::Closed(None),
            };
            match incoming {
                Incoming::Text(text) => self.control(&text, events),
                Incoming::Binary(frame) => self.binary(&frame, events),
                Incoming::Closed(code) => {
                    self.socket = None;
                    self.clear_channel(events);
                    if matches!(code, Some(CLOSE_AUTH_FAILED | CLOSE_NO_INSTANCE | CLOSE_FINISHED)) {
                        self.finished = true;
                        events.push(Event::Finished);
                    } else {
                        self.reconnect_at = Some(later(self.reconnect_delay));
                    }
                    return;
                }
            }
        }
    }

    /// Releases keys held past [`MAX_HOLD`], whose "up" was lost.
    fn release_stale(&mut self, events: &mut Vec<Event>) {
        let (stale, held): (Vec<_>, Vec<_>) =
            core::mem::take(&mut self.held).into_iter().partition(|(_, since)| since.elapsed() >= MAX_HOLD);
        self.held = held;
        events.extend(stale.into_iter().map(|(key, _)| Event::Up(key)));
    }
}
