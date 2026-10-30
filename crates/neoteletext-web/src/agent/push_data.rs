//! What the push connection carries (REQ-WEB-005): WebSocket frames and
//! server-sent events read into pages of T42, every packet checked and the
//! invalid ones discarded and counted (REQ-1462), and the channel's
//! records read from among them (INPUT.md section 7).

use neoteletext_core::consts::PACKET_HEADER;
use neoteletext_core::packet::{Packet, decode_address, is_valid_stream_packet};
use neoteletext_core::types::Instant;

use super::cookies::cookie_store;
use super::{Event, Failure, Output, PageBytes, PushProblem, Sink, Source, T42_PACKET, UserAgent, event};
use crate::bytes::copy_prefix;
use crate::consts::OPCODE_PONG;
use crate::cookie::CookieRejected;
use crate::http::MediaType;
use crate::port::RandomSource;
use crate::records::index::Step;
use crate::records::log::MAX_LOG_LINES_PER_PAGE;
use crate::records::{ChannelRecord, Read};
use crate::sse::SseEvent;
use crate::url::Url;
use crate::ws::WsMessage;

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    /// Reads WebSocket frames. Returns the bytes used, or `None` when the
    /// connection ended.
    pub(super) fn push_frames(&mut self, now: Instant, input: &[u8], sink: &mut Sink<'_>) -> Option<usize> {
        let limit = self.config.push_limit.min(self.buffers.push_message.len());
        let message = self.buffers.push_message.get_mut(..limit).unwrap_or(&mut []);
        let (used, done) = match self.push.frames.feed(message, input) {
            Ok(result) => result,
            Err(error) => {
                self.send_push_close(error.close_code(), sink);
                self.push_lost(now, Some(PushProblem::WebSocket(error)), true, sink);
                return None;
            }
        };
        match done {
            // The push reader is strict, so it never skips a message.
            None | Some(WsMessage::Skipped) => {}
            // DEC-1459: a server sends a channel's receiver T42 alone, so a
            // text message is discarded and counted, never read as a page.
            Some(WsMessage::Text(_)) => self.count_invalid(1, sink),
            Some(WsMessage::Binary(len)) => self.push_packets(now, len, sink),
            Some(WsMessage::Ping) => {
                let mut payload = [0u8; crate::consts::WS_CONTROL_MAX];
                let len = copy_prefix(&mut payload, self.push.frames.control_payload());
                self.send_push_frame(OPCODE_PONG, payload.get(..len).unwrap_or(&[]), sink);
            }
            Some(WsMessage::Pong) => {
                self.push.healthy = true;
                let mut payload = [0u8; crate::consts::WS_CONTROL_MAX];
                let len = copy_prefix(&mut payload, self.push.frames.control_payload());
                self.round_trip_pong(now, payload.get(..len).unwrap_or(&[]), sink);
            }
            Some(WsMessage::Close(code)) => {
                // RFC 6455 section 5.5.1: answer with a close, then end.
                self.send_push_close(code.unwrap_or(crate::consts::CLOSE_NORMAL), sink);
                self.push_lost(now, None, true, sink);
                return None;
            }
        }
        Some(used)
    }

    /// Reads the event stream. Returns the bytes used, or `None` when the
    /// connection ended.
    pub(super) fn push_events(&mut self, now: Instant, input: &[u8], sink: &mut Sink<'_>) -> Option<usize> {
        let body = self.push.body.as_mut()?;
        let step = match body.feed(input) {
            Ok(step) => step,
            Err(error) => {
                self.push_lost(now, Some(PushProblem::Failure(Failure::Http(error))), true, sink);
                return None;
            }
        };
        // The final step can carry data too: it is read before the stream
        // ends (finding F-77).
        let mut data = step.data;
        while !data.is_empty() {
            let limit = self.config.push_limit.min(self.buffers.push_message.len());
            let message = self.buffers.push_message.get_mut(..limit).unwrap_or(&mut []);
            let (used, dispatched) = match self.push.sse.feed(message, data) {
                Ok(result) => result,
                Err(error) => {
                    self.push_lost(now, Some(PushProblem::EventStream(error)), true, sink);
                    return None;
                }
            };
            data = data.get(used..).unwrap_or(&[]);
            self.deliver_event(now, dispatched, sink);
        }
        if step.done {
            self.push_lost(now, None, true, sink);
            return None;
        }
        Some(step.consumed)
    }

    fn deliver_event(&mut self, now: Instant, dispatched: Option<SseEvent>, sink: &mut Sink<'_>) {
        match dispatched {
            Some(SseEvent::Page(len)) => {
                self.push.healthy = true;
                self.deliver_push_page(MediaType::Tti, len, sink);
            }
            Some(SseEvent::Packets(len)) => {
                let message = self.buffers.push_message.get_mut(..len).unwrap_or(&mut []);
                // Data lines are joined with line feeds; base64 has none.
                let mut kept = 0usize;
                for index in 0..message.len() {
                    let byte = message.get(index).copied().unwrap_or(b'\n');
                    if byte != b'\n'
                        && let Some(slot) = message.get_mut(kept)
                    {
                        *slot = byte;
                        kept = kept.saturating_add(1);
                    }
                }
                match crate::base64::decode_in_place(message, kept) {
                    Ok(decoded) => self.push_packets(now, decoded, sink),
                    Err(_) => event(sink, Event::PushLost { retry_in_us: 0, problem: Some(PushProblem::Base64) }),
                }
            }
            Some(SseEvent::Other) | None => {}
        }
    }

    /// Counts `packets` invalid packets (REQ-1462) and tells the port the
    /// new total.
    fn count_invalid(&mut self, packets: usize, sink: &mut Sink<'_>) {
        if packets == 0 {
            return;
        }
        self.push.invalid = self.push.invalid.saturating_add(u64::try_from(packets).unwrap_or(u64::MAX));
        event(sink, Event::InvalidPackets { total: self.push.invalid });
    }

    /// Reads the first `len` bytes of the push message as T42 packets: a
    /// message that is not whole packets is discarded whole; each packet
    /// the stream may not carry is discarded (REQ-1462); the channel's
    /// records among them are read; and the packets kept are passed on.
    /// None of this closes the connection.
    fn push_packets(&mut self, now: Instant, len: usize, sink: &mut Sink<'_>) {
        if !len.is_multiple_of(T42_PACKET) {
            self.count_invalid(len.div_ceil(T42_PACKET), sink);
            return;
        }
        let mut invalid: usize = 0;
        let mut kept: usize = 0;
        let mut page = false;
        for start in (0..len).step_by(T42_PACKET) {
            let end = start.saturating_add(T42_PACKET);
            let Some(bytes) = self.buffers.push_message.get(start..end).and_then(|bytes| bytes.try_into().ok()) else {
                break;
            };
            let packet = Packet { bytes };
            if !is_valid_stream_packet(&packet) {
                invalid = invalid.saturating_add(1);
                continue;
            }
            if decode_address(&packet).is_ok_and(|address| address.number == PACKET_HEADER) {
                // A page frame begins: its LOG records are counted afresh
                // (INPUT.md section 7.6).
                page = true;
                self.push.log_lines = 0;
            }
            invalid = invalid.saturating_add(self.read_record(now, &packet, sink));
            self.buffers.push_message.copy_within(start..end, kept);
            kept = kept.saturating_add(T42_PACKET);
        }
        if kept > 0 {
            self.push.healthy = true;
            self.deliver_push_page(MediaType::T42, kept, sink);
        }
        if page && core::mem::take(&mut self.connecting) {
            event(sink, Event::ChannelLive);
        }
        self.count_invalid(invalid, sink);
    }

    /// Reads one packet into the channel's records and acts on a record it
    /// completes (INPUT.md sections 7.5 to 7.7). Returns the packets found
    /// invalid.
    fn read_record(&mut self, now: Instant, packet: &Packet, sink: &mut Sink<'_>) -> usize {
        let (seconds, calendar) = self.seconds(now);
        let record = match self.records.push(packet) {
            Read::Nothing => return 0,
            Read::Invalid(packets) => return packets,
            Read::Record(record) => record,
        };
        match record {
            ChannelRecord::Log(log) => {
                if self.push.log_lines < MAX_LOG_LINES_PER_PAGE {
                    self.push.log_lines = self.push.log_lines.saturating_add(1);
                    event(sink, Event::Log { line: log.line, page: log.page });
                }
            }
            ChannelRecord::Index(index) => match self.index.note(&index) {
                Step::Refused => return 1,
                Step::Partial => event(sink, Event::Index(index)),
                Step::Complete => {
                    event(sink, Event::Index(index));
                    event(sink, Event::IndexComplete { generation: index.generation });
                }
            },
            ChannelRecord::SetCookie(line) => {
                if !cfg!(feature = "cookies") {
                    event(sink, Event::CookieRejected(CookieRejected::NotCompiledIn));
                    return 0;
                }
                let Some(url) = self.push.url else { return 0 };
                // As the push handshake's own cookies: host-only, in the
                // channel's folder (REQ-WEB-026, INPUT.md section 7.7).
                let scope = self.channel.as_ref().map(Url::path);
                let store = cookie_store(&mut self.jar, &mut self.port_cookies);
                if let Err(rejected) =
                    store.store_within(line, url.host_text(), url.path(), scope, url.is_secure(), seconds, calendar)
                {
                    event(sink, Event::CookieRejected(rejected));
                }
            }
        }
        0
    }

    /// Delivers the first `len` bytes of the push message as a pushed page
    /// of `media`.
    fn deliver_push_page(&self, media: MediaType, len: usize, sink: &mut Sink<'_>) {
        let url = self.push.url;
        let url_text = url.as_ref().map_or("", Url::as_str);
        let secure = url.as_ref().is_some_and(Url::is_secure);
        let bytes = self.buffers.push_message.get(..len).unwrap_or(&[]);
        sink(Output::PageBytes(PageBytes { source: Source::Push, media, url: url_text, secure, bytes }));
    }
}
