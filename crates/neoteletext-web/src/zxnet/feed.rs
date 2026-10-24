//! The feed's connection (REQ-INP-022): the WebSocket handshake through
//! the port's transport, the frames received, keepalive every 5 seconds,
//! and reconnection after 10 seconds, at most five times.

use neoteletext_core::types::Instant;

use super::{LiveEvent, LiveInput, LiveOutput, LiveSink, Malformed, Phase, Walk, ZxnetFeed, event};
use crate::bytes::Writer;
use crate::consts::{
    DEFAULT_CONNECT_TIMEOUT_US, DEFAULT_RESPONSE_TIMEOUT_US, OPCODE_PONG, ZXNET_INACTIVITY_US, ZXNET_KEEPALIVE_US,
    ZXNET_MAX_ATTEMPTS, ZXNET_RECONNECT_US,
};
use crate::http;
use crate::port::{ConnectionId, NetInput, RandomSource};
use crate::url::IpAddress;
use crate::ws::{self, FrameReader, WsMessage};

impl<R: RandomSource> ZxnetFeed<'_, R> {
    /// Handles one input. Outputs go to `sink` before this returns.
    pub fn handle(&mut self, now: Instant, input: LiveInput<'_>, sink: &mut LiveSink<'_>) {
        match input {
            LiveInput::Tick => self.tick(now, sink),
            LiveInput::Start => {
                if matches!(self.phase, Phase::Stopped | Phase::Waiting { .. }) {
                    self.attempts = 0;
                    self.connect(now, sink);
                }
            }
            LiveInput::Stop => {
                if !matches!(self.phase, Phase::Stopped | Phase::Waiting { .. }) {
                    sink(LiveOutput::Close);
                }
                self.phase = Phase::Stopped;
            }
            LiveInput::Request { page, subcode } => {
                self.display = Some((page, subcode));
                self.walk = None;
                if self.phase == Phase::Open {
                    self.page_search(page, subcode, sink);
                }
            }
            LiveInput::Walk(on) => {
                self.walk =
                    on.then_some(Walk { current: None, started: now, header_seen: false, rows: 0, last_row: None });
                if on {
                    self.walk_next(now, None, sink);
                }
            }
            LiveInput::Net(net) => match net {
                NetInput::Connected(ConnectionId::Live, peer) => self.connected(now, peer, sink),
                NetInput::ConnectFailed(ConnectionId::Live) | NetInput::Closed(ConnectionId::Live) => {
                    if !matches!(self.phase, Phase::Stopped | Phase::Waiting { .. }) {
                        self.dropped(now, false, sink);
                    }
                }
                NetInput::Received(ConnectionId::Live, bytes) => self.received(now, bytes, sink),
                _ => {}
            },
        }
        if let Some(at) = self.next_deadline() {
            sink(LiveOutput::Timer(at));
        }
    }

    fn connect(&mut self, now: Instant, sink: &mut LiveSink<'_>) {
        let ip = self.channel.address().ip();
        let allowed = ip.is_some_and(|ip| self.allowed_local == Some(ip));
        let blocked = ip.is_some_and(IpAddress::is_blocked)
            || self.proxied && crate::proxy::hidden_local(self.channel.address().host_text());
        if blocked && !allowed {
            self.phase = Phase::Stopped;
            event(sink, LiveEvent::Blocked);
            return;
        }
        self.phase = Phase::Connecting { deadline: Instant(now.0.saturating_add(DEFAULT_CONNECT_TIMEOUT_US)) };
        let address = *self.channel.address();
        sink(LiveOutput::Connect { host: self.channel.address().host_text(), port: address.port() });
    }

    fn connected(&mut self, now: Instant, peer: IpAddress, sink: &mut LiveSink<'_>) {
        if !matches!(self.phase, Phase::Connecting { .. }) {
            return;
        }
        if !self.proxied && peer.is_blocked() && self.allowed_local != Some(peer) {
            sink(LiveOutput::Close);
            self.phase = Phase::Stopped;
            event(sink, LiveEvent::Blocked);
            return;
        }
        self.key = ws::new_key(&mut self.random);
        self.head.reset();
        // The server sends messages the feed does not use, such as
        // "channelSettings" with the channel's clock face at about 88 KB;
        // they are skipped, not taken as a broken connection.
        self.frames = FrameReader::skipping_oversized();
        let mut writer = Writer::new(self.buffers.request);
        let written =
            ws::write_handshake_headers(&mut writer, self.channel.address(), &self.key, self.user_agent.as_bytes())
                .and_then(|()| writer.put(crate::consts::CRLF));
        if written.is_err() {
            self.dropped(now, true, sink);
            return;
        }
        self.phase = Phase::Handshake { deadline: Instant(now.0.saturating_add(DEFAULT_RESPONSE_TIMEOUT_US)) };
        let len = writer.len();
        sink(LiveOutput::Send(self.buffers.request.get(..len).unwrap_or(&[])));
    }

    /// The connection dropped or failed: wait 10 seconds and try again, at
    /// most five times (REQ-INP-022).
    fn dropped(&mut self, now: Instant, close: bool, sink: &mut LiveSink<'_>) {
        if close {
            sink(LiveOutput::Close);
        }
        self.attempts = self.attempts.saturating_add(1);
        if self.attempts >= ZXNET_MAX_ATTEMPTS {
            self.phase = Phase::Stopped;
            event(sink, LiveEvent::GaveUp);
            return;
        }
        self.phase = Phase::Waiting { until: Instant(now.0.saturating_add(ZXNET_RECONNECT_US)) };
        event(sink, LiveEvent::Reconnecting { attempt: self.attempts, in_us: ZXNET_RECONNECT_US });
    }

    fn received(&mut self, now: Instant, mut input: &[u8], sink: &mut LiveSink<'_>) {
        while !input.is_empty() {
            match self.phase {
                Phase::Handshake { .. } => {
                    let head_buffer = &mut *self.buffers.head;
                    let Ok((used, complete)) = self.head.feed(head_buffer, input) else {
                        self.dropped(now, true, sink);
                        return;
                    };
                    input = input.get(used..).unwrap_or(&[]);
                    if complete {
                        let head = self.buffers.head.get(..self.head.len()).unwrap_or(&[]);
                        let accepted = http::parse_head(head, &mut |_| {})
                            .is_ok_and(|head| ws::check_handshake(&head, &self.key).is_ok());
                        if !accepted {
                            self.dropped(now, true, sink);
                            return;
                        }
                        self.opened(now, sink);
                    }
                }
                Phase::Open => {
                    let (used, done) = match self.frames.feed(self.buffers.message, input) {
                        Ok(result) => result,
                        Err(error) => {
                            self.send_close(error.close_code(), sink);
                            self.dropped(now, true, sink);
                            return;
                        }
                    };
                    input = input.get(used..).unwrap_or(&[]);
                    match done {
                        None | Some(WsMessage::Pong) => {}
                        Some(WsMessage::Text(len)) => self.message(now, len, sink),
                        Some(WsMessage::Skipped) => event(sink, LiveEvent::Ignored(Malformed::TooLong)),
                        Some(WsMessage::Binary(_)) => event(sink, LiveEvent::Ignored(Malformed::Binary)),
                        Some(WsMessage::Ping) => {
                            let mut payload = [0u8; crate::consts::WS_CONTROL_MAX];
                            let ping = self.frames.control_payload();
                            let len = ping.len().min(payload.len());
                            payload.get_mut(..len).unwrap_or(&mut []).copy_from_slice(ping.get(..len).unwrap_or(&[]));
                            self.send_frame(OPCODE_PONG, payload.get(..len).unwrap_or(&[]), sink);
                        }
                        Some(WsMessage::Close(code)) => {
                            self.send_close(code.unwrap_or(crate::consts::CLOSE_NORMAL), sink);
                            self.dropped(now, true, sink);
                            return;
                        }
                    }
                    if self.phase != Phase::Open {
                        return;
                    }
                }
                Phase::Stopped | Phase::Waiting { .. } | Phase::Connecting { .. } => return,
            }
        }
    }

    fn tick(&mut self, now: Instant, sink: &mut LiveSink<'_>) {
        match self.phase {
            Phase::Waiting { until } if now >= until => self.connect(now, sink),
            Phase::Connecting { deadline } | Phase::Handshake { deadline } if now >= deadline => {
                self.dropped(now, true, sink);
            }
            Phase::Open => {
                if now.since(self.last_activity) >= ZXNET_INACTIVITY_US {
                    self.dropped(now, true, sink);
                    return;
                }
                if now >= self.next_keepalive {
                    self.next_keepalive = Instant(now.0.saturating_add(ZXNET_KEEPALIVE_US));
                    self.send_text(b"keepalive", sink);
                }
                self.tick_walk(now, sink);
            }
            _ => {}
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        match self.phase {
            Phase::Stopped => None,
            Phase::Waiting { until } => Some(until),
            Phase::Connecting { deadline } | Phase::Handshake { deadline } => Some(deadline),
            Phase::Open => {
                let inactive = Instant(self.last_activity.0.saturating_add(ZXNET_INACTIVITY_US));
                [Some(inactive), Some(self.next_keepalive), self.walk_deadline()].into_iter().flatten().min()
            }
        }
    }
}
