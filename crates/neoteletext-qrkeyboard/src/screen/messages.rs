//! The phone's binary frames: pairing, then sealed application messages
//! and their acknowledgements.

use super::{ACKNOWLEDGED, Event, Screen};
use crate::message::{self, FromPhone};
use crate::pairing::Outcome;
use std::time::Instant;

impl Screen {
    pub(super) fn binary(&mut self, frame: &[u8], events: &mut Vec<Event>) {
        if let Some(pairing) = &mut self.pairing {
            let (out, outcome) = pairing.receive(frame);
            for frame in out {
                self.send_binary(&frame);
            }
            match outcome {
                None => {}
                Some(Outcome::WrongPin) => {
                    self.pairing = None;
                    self.pin_failed(events);
                }
                Some(Outcome::Paired(channel)) => {
                    self.pairing = None;
                    self.channel = Some(*channel);
                    self.pin_attempts = 0;
                    self.keepalive = Instant::now();
                    events.push(Event::Connected);
                }
            }
            return;
        }
        let Some(plaintext) = self.channel.as_mut().and_then(|(_, opener)| opener.open(frame)) else { return };
        let Some(message) = message::read(&plaintext) else { return };
        if let Some((_, ok)) = self.acknowledged.iter().find(|(id, _)| *id == message.id).cloned() {
            self.acknowledge(&message.id, ok);
            return;
        }
        let ok = match message.body {
            FromPhone::Finish => {
                let _ = self.finish();
                events.push(Event::Finished);
                return;
            }
            FromPhone::Text(text) if self.config.layout.has_text() => {
                events.push(Event::Text(text));
                true
            }
            FromPhone::Special { key, count } if self.config.layout.has_text() => {
                events.push(Event::Special { key, count });
                true
            }
            FromPhone::Down(key) if self.config.layout.has_key(&key) => {
                self.held.retain(|(held, _)| *held != key);
                self.held.push((key.clone(), Instant::now()));
                events.push(Event::Down(key));
                true
            }
            FromPhone::Up(key) if self.config.layout.has_key(&key) => {
                // Only a key the screen holds is released (decision E.116
                // item 1): releases are bounded by presses, and a release
                // of anything else is acknowledged and dropped.
                let before = self.held.len();
                self.held.retain(|(held, _)| *held != key);
                if self.held.len() < before {
                    events.push(Event::Up(key));
                }
                true
            }
            // Keys outside the layout are ignored, as the SDK ignores them.
            FromPhone::Down(_) | FromPhone::Up(_) => return,
            FromPhone::Text(_) | FromPhone::Special { .. } | FromPhone::Other => false,
        };
        self.acknowledged.push_back((message.id.clone(), ok));
        if self.acknowledged.len() > ACKNOWLEDGED {
            self.acknowledged.pop_front();
        }
        self.acknowledge(&message.id, ok);
    }

    fn acknowledge(&mut self, id: &str, ok: bool) {
        let Ok(plaintext) = message::pack(&message::ack(id, ok)) else { return };
        let Some(frame) = self.channel.as_mut().and_then(|(sealer, _)| sealer.seal(&plaintext)) else { return };
        self.send_binary(&frame);
    }
}
