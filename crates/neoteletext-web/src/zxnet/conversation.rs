//! The feed's side of the conversation with the server (REQ-INP-022):
//! the client messages it sends as WebSocket frames, and what it does with
//! each server message.

use neoteletext_core::consts::{PACKET_HEADER, PACKET_SERVICE, SERVICE_MAGAZINE};
use neoteletext_core::types::{Instant, Magazine, PageAddress, PageNumber, SubCode};

use super::message::{Message, cross_check, parse_message, rebuild};
use super::walk::page_index;
use super::{LiveEvent, LiveOutput, LiveSink, MAX_CLIENT_MESSAGE, Phase, ZxnetFeed, event};
use crate::bytes::Writer;
use crate::consts::{OPCODE_TEXT, ZXNET_API_VERSION, ZXNET_DISPLAY_SLOT, ZXNET_KEEPALIVE_US};
use crate::port::RandomSource;
use crate::ws;

impl<R: RandomSource> ZxnetFeed<'_, R> {
    /// Sends one client message as a text frame.
    pub(super) fn send_text(&mut self, message: &[u8], sink: &mut LiveSink<'_>) {
        self.send_frame(OPCODE_TEXT, message, sink);
    }

    /// Writes one frame with `opcode` and sends it.
    pub(super) fn send_frame(&mut self, opcode: u8, payload: &[u8], sink: &mut LiveSink<'_>) {
        let mut writer = Writer::new(self.buffers.request);
        if ws::write_frame(&mut writer, opcode, payload, &mut self.random).is_ok() {
            let len = writer.len();
            sink(LiveOutput::Send(self.buffers.request.get(..len).unwrap_or(&[])));
        }
    }

    /// Writes a close frame with `code` and sends it.
    pub(super) fn send_close(&mut self, code: u16, sink: &mut LiveSink<'_>) {
        let mut writer = Writer::new(self.buffers.request);
        if ws::write_close(&mut writer, code, &mut self.random).is_ok() {
            let len = writer.len();
            sink(LiveOutput::Send(self.buffers.request.get(..len).unwrap_or(&[])));
        }
    }

    /// `pagesearch` on slot 0: slot, magazine, page number, sub-code (16255
    /// for any) and three flags, as the viewer sends for the displayed page.
    pub(super) fn page_search(&mut self, page: PageAddress, subcode: Option<SubCode>, sink: &mut LiveSink<'_>) {
        let mut text = [0u8; MAX_CLIENT_MESSAGE];
        let mut writer = Writer::new(&mut text);
        let subcode = subcode.unwrap_or(SubCode::ANY).value();
        let built = (|| {
            writer.put(b"pagesearch,")?;
            writer.decimal(u64::from(ZXNET_DISPLAY_SLOT))?;
            writer.byte(b',')?;
            writer.decimal(u64::from(page.magazine.number()))?;
            writer.byte(b',')?;
            writer.decimal(u64::from(page.page.0))?;
            writer.byte(b',')?;
            writer.decimal(u64::from(subcode))?;
            writer.put(b",true,false,false")
        })();
        if built.is_ok() {
            let len = writer.len();
            self.send_text(text.get(..len).unwrap_or(&[]), sink);
        }
    }

    pub(super) fn opened(&mut self, now: Instant, sink: &mut LiveSink<'_>) {
        self.phase = Phase::Open;
        self.last_activity = now;
        self.next_keepalive = Instant(now.0.saturating_add(ZXNET_KEEPALIVE_US));
        let mut text = [0u8; MAX_CLIENT_MESSAGE];
        let mut writer = Writer::new(&mut text);
        if writer.put(b"service,").and_then(|()| writer.put(self.channel.identifier().as_bytes())).is_ok() {
            let len = writer.len();
            self.send_text(text.get(..len).unwrap_or(&[]), sink);
        }
        self.send_text(b"ttx,true", sink);
        let wanted = self.walk.and_then(|walk| walk.current.map(|page| (page, None))).or(self.display);
        if let Some((page, subcode)) = wanted {
            self.page_search(page, subcode, sink);
        }
        event(sink, LiveEvent::Connected);
    }

    /// Handles one server message held in the message buffer.
    pub(super) fn message(&mut self, now: Instant, len: usize, sink: &mut LiveSink<'_>) {
        let parsed = parse_message(self.buffers.message.get(..len).unwrap_or(&[]));
        let message = match parsed {
            Ok(Some(message)) => message,
            Ok(None) => return,
            Err(malformed) => {
                event(sink, LiveEvent::Ignored(malformed));
                return;
            }
        };
        match message {
            Message::ApiVersion(version) => {
                event(sink, LiveEvent::ApiVersion(version));
                let mut text = [0u8; 16];
                let mut writer = Writer::new(&mut text);
                if writer.put(b"apiver,").and_then(|()| writer.decimal(u64::from(ZXNET_API_VERSION))).is_ok() {
                    let len = writer.len();
                    self.send_text(text.get(..len).unwrap_or(&[]), sink);
                }
            }
            Message::WrongName => {
                sink(LiveOutput::Close);
                self.phase = Phase::Stopped;
                event(sink, LiveEvent::WrongName);
            }
            Message::Header { slot, data, control, magazine, subcode, page } => {
                self.last_activity = now;
                // A healthy connection: failed attempts count from zero.
                self.attempts = 0;
                let packet = rebuild(magazine, PACKET_HEADER, &data);
                sink(LiveOutput::Packet(packet));
                cross_check(&packet, control, subcode, page, sink);
                let address = PageAddress { magazine, page: PageNumber(page) };
                if slot == ZXNET_DISPLAY_SLOT
                    && let Some(walk) = &mut self.walk
                    && walk.current == Some(address)
                {
                    walk.header_seen = true;
                }
            }
            Message::Row { slot, magazine, number, data } => {
                sink(LiveOutput::Packet(rebuild(magazine, number, &data)));
                if slot == ZXNET_DISPLAY_SLOT
                    && let Some(walk) = &mut self.walk
                    && walk.header_seen
                {
                    walk.rows = walk.rows.saturating_add(1);
                    walk.last_row = Some(now);
                }
            }
            Message::Bsdp(data) => {
                self.last_activity = now;
                let service = Magazine::new(SERVICE_MAGAZINE).unwrap_or(Magazine::from_bits(0));
                sink(LiveOutput::Packet(rebuild(service, PACKET_SERVICE, &data)));
            }
            Message::Clock(clock, len) => {
                self.last_activity = now;
                event(sink, LiveEvent::Clock(clock.get(..len).unwrap_or(&[])));
            }
            Message::PageExists(address) => {
                let index = page_index(address);
                if let Some(byte) = self.pages.get_mut(index / 8) {
                    *byte |= 1 << (index % 8);
                }
                event(sink, LiveEvent::PageExists(address));
            }
            Message::InitialPage { page, subcode, status, len } => {
                event(sink, LiveEvent::InitialPage { page, subcode, status: status.get(..len).unwrap_or(&[]) });
            }
        }
    }
}
