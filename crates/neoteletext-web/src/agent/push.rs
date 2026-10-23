//! The push connection (REQ-WEB-005): the subscription, connecting over
//! WebSocket or server-sent events, the handshake, the backoff, the
//! heartbeat, and the input profile's messages (decision E.90).

use neoteletext_core::types::Instant;

use super::cookies::{cookie_store, store_response_cookies, write_cookie_header};
use super::{Event, Failure, Output, PushPhase, PushProblem, Sink, Subscription, UserAgent, event};
use crate::bytes::{Overflow, Writer};
use crate::consts::{
    ACCEPT_EVENT_STREAM, CRLF, IDENTITY, INPUT_MESSAGE_MAX, MAX_SUBSCRIBED_PAGES, MAX_URL_LENGTH, OPCODE_BINARY,
    OPCODE_PING, OPCODE_TEXT, PUSH_EVENTS_SILENCE_US, PUSH_PING_AFTER_US, PUSH_PONG_WITHIN_US, PUSH_RETRY_FIRST_US,
    PUSH_RETRY_MAX_US, STATUS_OK, SUBSCRIBE, SUBSCRIBE_ALL, SUBSCRIBE_QUERY, header,
};
use crate::cookie::Site;
use crate::descriptor::{Descriptor, PushMethod};
use crate::http::{self, BodyDecoder, MediaType, Method};
use crate::port::{ConnectionId, RandomSource, Security};
use crate::url::{IpAddress, Url};
use crate::ws::{self, FrameReader};

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
    fn write_subscription(&self, writer: &mut Writer<'_>, separator: u8) -> Result<(), Overflow> {
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

    fn send_subscribe(&mut self, sink: &mut Sink<'_>) {
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

    /// The push address with the server-sent events subscription query.
    fn push_address(&self) -> Option<Url> {
        let url = self.push.url?;
        if self.push.method == PushMethod::WebSocket {
            return Some(url);
        }
        let mut buffer = [0u8; MAX_URL_LENGTH];
        let mut writer = Writer::new(&mut buffer);
        writer.put(url.as_str().as_bytes()).ok()?;
        writer.byte(if url.query().is_some() { b'&' } else { b'?' }).ok()?;
        writer.put(SUBSCRIBE_QUERY).ok()?;
        self.write_subscription(&mut writer, b',').ok()?;
        Url::parse(core::str::from_utf8(writer.written()).ok()?).ok()
    }

    pub(super) fn push_connect(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let Some(url) = self.push_address() else {
            self.push.phase = PushPhase::Idle;
            return;
        };
        if let Some(failure) = self.refusal(now, &url) {
            Self::report_refusal(ConnectionId::Push, &url, failure, sink);
            self.push_lost(now, Some(PushProblem::Failure(failure)), false, sink);
            return;
        }
        self.push.phase =
            PushPhase::Connecting { deadline: Instant(now.0.saturating_add(self.config.connect_timeout_us)) };
        self.push.healthy = false;
        let security = if url.is_secure() { Security::Tls } else { Security::Plain };
        sink(Output::Connect { connection: ConnectionId::Push, host: url.host_text(), port: url.port(), security });
    }

    pub(super) fn push_connected(&mut self, now: Instant, peer: IpAddress, sink: &mut Sink<'_>) {
        if !matches!(self.push.phase, PushPhase::Connecting { .. }) {
            return;
        }
        let Some(url) = self.push_address() else { return };
        if self.peer_blocked(peer) {
            event(sink, Event::Blocked { connection: ConnectionId::Push, host: url.host_text() });
            self.push_lost(now, Some(PushProblem::Failure(Failure::Blocked)), true, sink);
            return;
        }
        if url.is_secure() {
            self.remember_secure(url.host_text());
        } else {
            event(sink, Event::Unencrypted { connection: ConnectionId::Push, host: url.host_text() });
        }
        let (seconds, _) = self.seconds(now);
        self.push.head.reset();
        self.push.body = None;
        self.push.frames = FrameReader::new();
        self.push.sse.restart();
        // A channel's push connection may be anywhere on its origin, and
        // carries the channel's cookies, whose paths are its folder
        // (REQ-WEB-026).
        let folder = self.channel;
        let mut writer = Writer::new(self.buffers.push_request);
        if self.push.method == PushMethod::WebSocket {
            self.push.key = ws::new_key(&mut self.random);
        }
        let written = (|| -> Result<bool, Overflow> {
            match self.push.method {
                PushMethod::WebSocket => {
                    ws::write_handshake_headers(&mut writer, &url, &self.push.key, self.named_as.as_bytes())?;
                }
                PushMethod::ServerSentEvents => {
                    http::write_request_line(&mut writer, Method::Get, url.target())?;
                    http::write_host(&mut writer, &url)?;
                    http::write_header(&mut writer, header::USER_AGENT, self.named_as.as_bytes())?;
                    http::write_header(&mut writer, header::ACCEPT, ACCEPT_EVENT_STREAM)?;
                    http::write_header(&mut writer, header::ACCEPT_ENCODING, IDENTITY)?;
                    let last = self.push.sse.last_event_id();
                    if !last.is_empty() {
                        http::write_header(&mut writer, header::LAST_EVENT_ID, last)?;
                    }
                }
            }
            // Cookies go with upgrades and event streams too (REQ-WEB-006).
            let path = folder.as_ref().map_or(url.path(), Url::path);
            let truncated = write_cookie_header(
                cookie_store(&mut self.jar, &mut self.port_cookies),
                &mut writer,
                &url,
                path,
                Site::SameService,
                seconds,
                0,
            )?;
            writer.put(CRLF)?;
            Ok(truncated)
        })();
        if written == Ok(true) {
            event(sink, Event::CookiesTruncated);
        }
        if written.is_err() {
            self.push_lost(now, Some(PushProblem::Failure(Failure::RequestTooLarge)), true, sink);
            return;
        }
        self.push.phase =
            PushPhase::Handshake { deadline: Instant(now.0.saturating_add(self.config.response_timeout_us)) };
        let len = writer.len();
        let bytes = self.buffers.push_request.get(..len).unwrap_or(&[]);
        sink(Output::Send { connection: ConnectionId::Push, bytes });
    }

    /// The push connection failed or closed: wait, then reconnect with a
    /// doubling delay (REQ-WEB-005).
    pub(super) fn push_lost(&mut self, now: Instant, problem: Option<PushProblem>, close: bool, sink: &mut Sink<'_>) {
        if self.push.url.is_none() || self.push.phase == PushPhase::Idle && problem.is_none() {
            return;
        }
        if close {
            sink(Output::Close { connection: ConnectionId::Push });
        }
        if self.push.healthy {
            self.push.delay_us = PUSH_RETRY_FIRST_US;
        }
        let delay = self.push.delay_us;
        self.push.delay_us = delay.saturating_mul(2).min(PUSH_RETRY_MAX_US);
        self.push.phase = PushPhase::Backoff { until: Instant(now.0.saturating_add(delay)) };
        self.push.healthy = false;
        event(sink, Event::PushLost { retry_in_us: delay, problem });
    }

    pub(super) fn push_received(&mut self, now: Instant, mut input: &[u8], sink: &mut Sink<'_>) {
        while !input.is_empty() {
            match self.push.phase {
                PushPhase::Handshake { .. } => {
                    let limit = self.config.header_limit.min(self.buffers.push_head.len());
                    let buffer = self.buffers.push_head.get_mut(..limit).unwrap_or(&mut []);
                    match self.push.head.feed(buffer, input) {
                        Ok((used, complete)) => {
                            input = input.get(used..).unwrap_or(&[]);
                            let head = self.buffers.push_head.get(..self.push.head.len()).unwrap_or(&[]);
                            if complete && http::is_interim(head) {
                                // RFC 9110 section 15.2: the final head follows.
                                self.push.head.reset();
                            } else if complete && !self.push_head_complete(now, sink) {
                                return;
                            }
                        }
                        Err(error) => {
                            self.push_lost(now, Some(PushProblem::Failure(Failure::Http(error))), true, sink);
                            return;
                        }
                    }
                }
                PushPhase::Open { .. } => {
                    // Anything heard is a sign of life (REQ-WEB-005).
                    self.push.phase = PushPhase::Open { heard: now, pinged: None };
                    let used = match self.push.method {
                        PushMethod::WebSocket => self.push_frames(now, input, sink),
                        PushMethod::ServerSentEvents => self.push_events(now, input, sink),
                    };
                    match used {
                        Some(used) if used > 0 => input = input.get(used..).unwrap_or(&[]),
                        _ => return,
                    }
                }
                PushPhase::Idle | PushPhase::Backoff { .. } | PushPhase::Connecting { .. } => return,
            }
        }
    }

    fn push_head_complete(&mut self, now: Instant, sink: &mut Sink<'_>) -> bool {
        let Some(url) = self.push_address() else { return false };
        let (seconds, calendar) = self.seconds(now);
        let head_len = self.push.head.len();
        let Self { buffers, jar, port_cookies, channel, .. } = self;
        let head_bytes = buffers.push_head.get(..head_len).unwrap_or(&[]);
        // A channel's cookies stay in its folder, those its push handshake
        // sets as much as a page's (REQ-WEB-026, finding F-15).
        let scope = channel.as_ref().map(Url::path);
        let store = cookie_store(jar, port_cookies);
        let parsed = store_response_cookies(
            store,
            head_bytes,
            url.host_text(),
            url.path(),
            scope,
            url.is_secure(),
            seconds,
            calendar,
            sink,
        );
        let head = match parsed {
            Ok(head) => head,
            Err(error) => {
                self.push_lost(now, Some(PushProblem::Failure(Failure::Http(error))), true, sink);
                return false;
            }
        };
        let accepted = match self.push.method {
            PushMethod::WebSocket => ws::check_handshake(&head, &self.push.key).map_err(PushProblem::WebSocket),
            PushMethod::ServerSentEvents => {
                if head.status != STATUS_OK {
                    Err(PushProblem::Failure(Failure::Status(head.status)))
                } else if head.media != MediaType::EventStream {
                    Err(PushProblem::Failure(Failure::MediaType))
                } else {
                    // Any framing is accepted for a stream; its total size
                    // is unbounded, each event is limited by the parser.
                    BodyDecoder::new(head.framing, usize::MAX)
                        .map(|decoder| self.push.body = Some(decoder))
                        .map_err(|error| PushProblem::Failure(Failure::Http(error)))
                }
            }
        };
        if let Err(problem) = accepted {
            self.push_lost(now, Some(problem), true, sink);
            return false;
        }
        self.push.phase = PushPhase::Open { heard: now, pinged: None };
        event(sink, Event::PushConnected(self.push.method));
        if self.push.method == PushMethod::WebSocket {
            self.send_subscribe(sink);
        }
        true
    }

    /// The push half of [`Self::tick`]: the backoff, the set-up's time
    /// limits and the heartbeat.
    pub(super) fn push_tick(&mut self, now: Instant, sink: &mut Sink<'_>) {
        if !cfg!(feature = "push") {
            return;
        }
        match self.push.phase {
            PushPhase::Backoff { until } if now >= until => {
                self.push.phase = PushPhase::Idle;
                self.push_connect(now, sink);
            }
            PushPhase::Connecting { deadline } | PushPhase::Handshake { deadline } if now >= deadline => {
                self.push_lost(now, Some(PushProblem::Failure(Failure::Timeout)), true, sink);
            }
            PushPhase::Open { .. } if self.heartbeat_due().is_some_and(|due| now >= due) => {
                self.heartbeat(now, sink);
            }
            _ => {}
        }
    }

    /// When the heartbeat next acts on the open push connection
    /// (REQ-WEB-005): a ping after a silence on a WebSocket, the end of the
    /// wait for its pong, or the end of a silent event stream.
    pub(super) fn heartbeat_due(&self) -> Option<Instant> {
        let PushPhase::Open { heard, pinged } = self.push.phase else { return None };
        let after = |at: Instant, wait: u64| Instant(at.0.saturating_add(wait));
        Some(match (self.push.method, pinged) {
            (PushMethod::WebSocket, None) => after(heard, PUSH_PING_AFTER_US),
            (PushMethod::WebSocket, Some(pinged)) => after(pinged, PUSH_PONG_WITHIN_US),
            (PushMethod::ServerSentEvents, _) => after(heard, PUSH_EVENTS_SILENCE_US),
        })
    }

    /// A silent WebSocket is pinged; one that does not answer, and a silent
    /// event stream, are reconnected with the backoff, which sends the
    /// subscription again.
    fn heartbeat(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let PushPhase::Open { heard, pinged } = self.push.phase else { return };
        if self.push.method == PushMethod::WebSocket && pinged.is_none() {
            self.send_push_frame(OPCODE_PING, b"", sink);
            self.push.phase = PushPhase::Open { heard, pinged: Some(now) };
            return;
        }
        self.push_lost(now, Some(PushProblem::Failure(Failure::Timeout)), true, sink);
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
