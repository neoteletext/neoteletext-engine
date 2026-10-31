//! What the agent sends on the push connection (REQ-WEB-005): the
//! subscription, its WebSocket frames and close, and the input profile's
//! messages (decision E.90).
//!
//! Every frame is written into the push request buffer after those already
//! waiting, and what waits is sent as one [`Output::Send`] when the input
//! being handled is done, so the messages of one moment leave in one write
//! and one TLS record (REQ-WEB-035, REQ-RCH-010), with nothing held for a
//! timer. A pointer's move is held instead while the socket is congested
//! (REQ-RCH-035).

use neoteletext_core::types::Instant;

use super::{Event, HeldMove, InputMessage, Output, PushPhase, Sink, Subscription, UserAgent, event};
use crate::bytes::{Overflow, Writer, copy_prefix};
use crate::consts::{
    INPUT_MESSAGE_MAX, MAX_SUBSCRIBED_PAGES, OPCODE_BINARY, OPCODE_CLOSE, OPCODE_TEXT, PUSH_MOVE_MAX, PUSH_MOVE_SLOTS,
    PUSH_UNSENT_BOUND, SUBSCRIBE, SUBSCRIBE_ALL,
};
use crate::descriptor::{Descriptor, PushMethod};
use crate::port::{ConnectionId, RandomSource};
use crate::ws;

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    pub(super) fn subscribe(&mut self, now: Instant, subscription: Subscription<'_>, sink: &mut Sink<'_>) {
        self.subscribed = [None; MAX_SUBSCRIBED_PAGES];
        match subscription {
            Subscription::All => self.subscribe_all = true,
            Subscription::Pages(pages) => {
                self.subscribe_all = false;
                for (slot, page) in self.subscribed.iter_mut().zip(pages) {
                    *slot = Some(*page);
                }
            }
        }
        if cfg!(feature = "push") && matches!(self.push.phase, PushPhase::Open { .. }) {
            match self.push.method {
                PushMethod::WebSocket => self.send_subscribe(sink),
                PushMethod::ServerSentEvents => {
                    // The subscription is in the address: reconnect at once.
                    self.close_push(sink);
                    self.push.phase = PushPhase::Idle;
                    self.push_connect(now, sink);
                }
            }
        }
    }

    /// Writes the subscription: `all`, or page numbers separated by
    /// `separator`.
    pub(super) fn write_subscription(&self, writer: &mut Writer<'_>, separator: u8) -> Result<(), Overflow> {
        if self.subscribe_all {
            return writer.put(SUBSCRIBE_ALL);
        }
        for (index, page) in self.subscribed.iter().flatten().enumerate() {
            if index > 0 {
                writer.byte(separator)?;
            }
            writer.put(&page.ascii())?;
        }
        Ok(())
    }

    pub(super) fn send_subscribe(&mut self, sink: &mut Sink<'_>) {
        let mut text = [0u8; 4 * MAX_SUBSCRIBED_PAGES + 16];
        let mut writer = Writer::new(&mut text);
        let built = writer
            .put(SUBSCRIBE)
            .and_then(|()| writer.byte(b' '))
            .and_then(|()| self.write_subscription(&mut writer, b' '));
        if built.is_err() {
            return;
        }
        let len = writer.len();
        self.send_push_frame(OPCODE_TEXT, text.get(..len).unwrap_or(&[]), sink);
    }

    /// Writes one frame on the push WebSocket after those waiting, to be
    /// sent with them; false when it does not fit the request buffer, even
    /// once what waits is sent.
    pub(super) fn send_push_frame(&mut self, opcode: u8, payload: &[u8], sink: &mut Sink<'_>) -> bool {
        if self.write_push_frame(opcode, payload) {
            return true;
        }
        self.flush_push(sink);
        self.write_push_frame(opcode, payload)
    }

    /// Writes one frame after those waiting; false when there is no room.
    fn write_push_frame(&mut self, opcode: u8, payload: &[u8]) -> bool {
        let rest = self.buffers.push_request.get_mut(self.push.out..).unwrap_or(&mut []);
        let mut frame = Writer::new(rest);
        if ws::write_frame(&mut frame, opcode, payload, &mut self.random).is_err() {
            return false;
        }
        self.push.out = self.push.out.saturating_add(frame.len());
        true
    }

    /// Sends the frames waiting, as one send (REQ-RCH-010).
    pub(super) fn flush_push(&mut self, sink: &mut Sink<'_>) {
        let length = core::mem::take(&mut self.push.out);
        if length > 0 {
            sink(Output::Send {
                connection: ConnectionId::Push,
                bytes: self.buffers.push_request.get(..length).unwrap_or(&[]),
            });
        }
    }

    /// Sends what waits, then closes the push connection.
    pub(super) fn close_push(&mut self, sink: &mut Sink<'_>) {
        self.flush_push(sink);
        sink(Output::Close { connection: ConnectionId::Push });
    }

    /// The push connection is new or gone: nothing written for the last one
    /// waits, no move is held, and the round trip starts again.
    pub(super) fn forget_push_writes(&mut self) {
        self.push.out = 0;
        self.push.unsent = 0;
        self.push.held = [None; PUSH_MOVE_SLOTS];
        self.push.round_trip.waiting = None;
        self.push.round_trip.last = None;
    }

    /// Writes a close frame with `code` on the push WebSocket, after what
    /// waits (RFC 6455 section 5.5.1: the code, big-endian).
    pub(super) fn send_push_close(&mut self, code: u16, sink: &mut Sink<'_>) {
        self.send_push_frame(OPCODE_CLOSE, &code.to_be_bytes(), sink);
    }

    /// Sends one message of the input profile on the push WebSocket
    /// (decision E.90), or drops it. A move waits in its contact's slot
    /// while the socket is congested; any other message first writes the
    /// moves held, then itself (REQ-RCH-035).
    pub(super) fn send_input(&mut self, message: InputMessage<'_>, sink: &mut Sink<'_>) {
        let open = cfg!(feature = "push")
            && matches!(self.push.phase, PushPhase::Open { .. })
            && self.push.method == PushMethod::WebSocket
            && self.descriptor.as_ref().is_some_and(Descriptor::input);
        if !open || message.bytes.len() > INPUT_MESSAGE_MAX {
            event(sink, Event::InputDropped);
            return;
        }
        if let Some(contact) = message.move_of {
            let slot = self.push.held.get_mut(usize::from(contact));
            let length = message.bytes.len();
            let (Some(slot), true) = (slot, length <= PUSH_MOVE_MAX) else {
                event(sink, Event::InputDropped);
                return;
            };
            if self.push.unsent >= PUSH_UNSENT_BOUND {
                let mut held = HeldMove { bytes: [0; PUSH_MOVE_MAX], length };
                copy_prefix(&mut held.bytes, message.bytes);
                *slot = Some(held);
                return;
            }
        }
        self.write_held_moves(sink);
        if !self.send_push_frame(OPCODE_BINARY, message.bytes, sink) {
            event(sink, Event::InputDropped);
        }
    }

    /// Writes every move held, in contact order (REQ-RCH-035).
    fn write_held_moves(&mut self, sink: &mut Sink<'_>) {
        for contact in 0..PUSH_MOVE_SLOTS {
            let Some(held) = self.push.held.get_mut(contact).and_then(Option::take) else { continue };
            if !self.send_push_frame(OPCODE_BINARY, held.bytes.get(..held.length).unwrap_or(&[]), sink) {
                event(sink, Event::InputDropped);
            }
        }
    }

    /// The port reports the push socket's unsent bytes: under the bound,
    /// the moves held are written (REQ-RCH-035).
    pub(super) fn push_unsent(&mut self, unsent: usize, sink: &mut Sink<'_>) {
        self.push.unsent = unsent;
        if unsent < PUSH_UNSENT_BOUND {
            self.write_held_moves(sink);
        }
    }
}
