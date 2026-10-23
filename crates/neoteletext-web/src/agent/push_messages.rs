//! What the agent sends on the push connection (REQ-WEB-005): the
//! subscription, its WebSocket frames and close, and the input profile's
//! messages (decision E.90).

use neoteletext_core::types::Instant;

use super::{Event, Output, PushPhase, Sink, Subscription, UserAgent, event};
use crate::bytes::{Overflow, Writer};
use crate::consts::{INPUT_MESSAGE_MAX, MAX_SUBSCRIBED_PAGES, OPCODE_BINARY, OPCODE_TEXT, SUBSCRIBE, SUBSCRIBE_ALL};
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
                    sink(Output::Close { connection: ConnectionId::Push });
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

    /// Writes one frame on the push WebSocket and sends it; false when it
    /// does not fit the request buffer.
    pub(super) fn send_push_frame(&mut self, opcode: u8, payload: &[u8], sink: &mut Sink<'_>) -> bool {
        let mut frame = Writer::new(self.buffers.push_request);
        if ws::write_frame(&mut frame, opcode, payload, &mut self.random).is_err() {
            return false;
        }
        let len = frame.len();
        sink(Output::Send {
            connection: ConnectionId::Push,
            bytes: self.buffers.push_request.get(..len).unwrap_or(&[]),
        });
        true
    }

    /// Writes a close frame with `code` on the push WebSocket and sends it.
    pub(super) fn send_push_close(&mut self, code: u16, sink: &mut Sink<'_>) {
        let mut frame = Writer::new(self.buffers.push_request);
        if ws::write_close(&mut frame, code, &mut self.random).is_ok() {
            let len = frame.len();
            sink(Output::Send {
                connection: ConnectionId::Push,
                bytes: self.buffers.push_request.get(..len).unwrap_or(&[]),
            });
        }
    }

    /// Sends one message of the input profile on the push WebSocket
    /// (decision E.90), or drops it.
    pub(super) fn send_input(&mut self, message: &[u8], sink: &mut Sink<'_>) {
        let open = cfg!(feature = "push")
            && matches!(self.push.phase, PushPhase::Open { .. })
            && self.push.method == PushMethod::WebSocket
            && self.descriptor.as_ref().is_some_and(Descriptor::input);
        if !open || message.len() > INPUT_MESSAGE_MAX {
            event(sink, Event::InputDropped);
            return;
        }
        if !self.send_push_frame(OPCODE_BINARY, message, sink) {
            event(sink, Event::InputDropped);
        }
    }
}
