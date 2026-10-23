//! The relay's control text: `ready`, the phone's connection, and the PIN's
//! failures and the lock.

use super::{Event, MAX_PIN_ATTEMPTS, RECONNECT_FIRST, Screen};
use crate::KEEPALIVE;
use crate::crypto::Role;
use crate::json::Value;
use crate::pairing::Pairing;

impl Screen {
    /// Text from the relay: `ready`, and `peer` up or down. Anything else
    /// is ignored, so the relay can add messages ahead of screens.
    pub(super) fn control(&mut self, text: &str, events: &mut Vec<Event>) {
        if text == KEEPALIVE {
            return;
        }
        let Ok(message) = Value::parse(text.as_bytes()) else { return };
        match message.get("type").and_then(Value::as_str) {
            Some("ready") => {
                self.reconnect_delay = RECONNECT_FIRST;
                if self.channel.is_none() && !self.locked {
                    events.push(Event::Scan);
                }
            }
            Some("peer") if message.get("up").and_then(Value::as_bool) == Some(true) => self.peer_up(events),
            Some("peer") => {
                let abandoned = self.pairing.as_ref().is_some_and(Pairing::uses_pin);
                self.clear_channel(events);
                if abandoned {
                    self.pin_failed(events);
                }
                events.push(Event::Disconnected(message.get("reservedForMs").and_then(Value::as_u64)));
            }
            _ => {}
        }
    }

    fn peer_up(&mut self, events: &mut Vec<Event>) {
        self.clear_channel(events);
        if self.locked {
            return;
        }
        if self.config.pin {
            // Counted as the share goes: a phone that abandons the run has
            // still tested one PIN.
            if self.pin_attempts >= MAX_PIN_ATTEMPTS {
                self.lock(events);
                return;
            }
            self.pin_attempts = self.pin_attempts.saturating_add(1);
        }
        let Ok(pairing) = Pairing::new(Role::Device, &self.join.code, &self.secret, &self.hash) else { return };
        for frame in pairing.start() {
            self.send_binary(&frame);
        }
        self.pairing = Some(pairing);
    }

    pub(super) fn pin_failed(&mut self, events: &mut Vec<Event>) {
        events.push(Event::PinFailed(MAX_PIN_ATTEMPTS.saturating_sub(self.pin_attempts)));
        if self.pin_attempts >= MAX_PIN_ATTEMPTS {
            self.lock(events);
        }
    }

    pub(super) fn lock(&mut self, events: &mut Vec<Event>) {
        if !self.locked {
            self.locked = true;
            events.push(Event::Locked);
        }
    }

    pub(super) fn clear_channel(&mut self, events: &mut Vec<Event>) {
        for (key, _) in core::mem::take(&mut self.held) {
            events.push(Event::Up(key));
        }
        self.pairing = None;
        self.channel = None;
    }
}
