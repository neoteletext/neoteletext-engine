//! What the push connection carries (REQ-WEB-005): WebSocket frames and
//! server-sent events read into pages of TTI or T42.

use neoteletext_core::types::Instant;

use super::{Event, Failure, Output, PageBytes, PushProblem, Sink, Source, T42_PACKET, UserAgent, event};
use crate::bytes::copy_prefix;
use crate::consts::OPCODE_PONG;
use crate::http::MediaType;
use crate::port::RandomSource;
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
            Some(WsMessage::Text(len)) => {
                self.push.healthy = true;
                self.deliver_push_page(MediaType::Tti, len, sink);
            }
            Some(WsMessage::Binary(len)) => {
                if len % T42_PACKET == 0 {
                    self.push.healthy = true;
                    self.deliver_push_page(MediaType::T42, len, sink);
                } else {
                    event(sink, Event::PushLost { retry_in_us: 0, problem: Some(PushProblem::PacketLength) });
                }
            }
            Some(WsMessage::Ping) => {
                let mut payload = [0u8; crate::consts::WS_CONTROL_MAX];
                let len = copy_prefix(&mut payload, self.push.frames.control_payload());
                self.send_push_frame(OPCODE_PONG, payload.get(..len).unwrap_or(&[]), sink);
            }
            Some(WsMessage::Pong) => self.push.healthy = true,
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
            self.deliver_event(dispatched, sink);
        }
        if step.done {
            self.push_lost(now, None, true, sink);
            return None;
        }
        Some(step.consumed)
    }

    fn deliver_event(&mut self, dispatched: Option<SseEvent>, sink: &mut Sink<'_>) {
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
                    Ok(decoded) if decoded % T42_PACKET == 0 => {
                        self.push.healthy = true;
                        self.deliver_push_page(MediaType::T42, decoded, sink);
                    }
                    Ok(_) => event(sink, Event::PushLost { retry_in_us: 0, problem: Some(PushProblem::PacketLength) }),
                    Err(_) => event(sink, Event::PushLost { retry_in_us: 0, problem: Some(PushProblem::Base64) }),
                }
            }
            Some(SseEvent::Other) | None => {}
        }
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
