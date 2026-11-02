//! The push connection (REQ-WEB-005): connecting over WebSocket or
//! the handshake, the backoff and the heartbeat.

use neoteletext_core::types::Instant;

use super::cookies::{cookie_store, store_response_cookies, write_cookie_header};
use super::{Event, Failure, Output, PushPhase, PushProblem, PushStop, Sink, UserAgent, event};
use crate::bytes::{Overflow, Writer};
use crate::consts::{
    CRLF, OPCODE_PING, PUSH_PING_AFTER_US, PUSH_PONG_WITHIN_US, PUSH_RETRY_FIRST_US, PUSH_RETRY_MAX_US,
    PUSH_SUBPROTOCOL, ROUND_TRIP_EVERY_US, STATUS_CLIENT_ERROR_FIRST, STATUS_SERVER_ERROR_FIRST,
    STATUS_SWITCHING_PROTOCOLS, STATUS_TOO_MANY_REQUESTS,
};
use crate::cookie::Site;
use crate::http::{self, ResponseHead};
use crate::port::{ConnectionId, RandomSource, Security};
use crate::url::{IpAddress, Url};
use crate::ws::{self, FrameReader};

/// Why a handshake's answer means no attempt will ever do better (DOC-1920),
/// or `None` when it may: a client error but for a limit, or
/// an upgrade to something other than the push subprotocol, which is another
/// server or another version (DEC-1759 lesson 14).
fn handshake_stop(head: &ResponseHead) -> Option<PushStop> {
    match head.status {
        STATUS_SWITCHING_PROTOCOLS => {
            let named = head.websocket_protocol.as_ref().map(crate::bytes::TextBuf::as_bytes);
            (named != Some(PUSH_SUBPROTOCOL.as_bytes())).then_some(PushStop::WrongProtocol)
        }
        STATUS_TOO_MANY_REQUESTS => None,
        status => (STATUS_CLIENT_ERROR_FIRST..STATUS_SERVER_ERROR_FIRST)
            .contains(&status)
            .then_some(PushStop::Refused(status)),
    }
}

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    pub(super) fn push_connect(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let Some(url) = self.push.url else {
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
        self.forget_push_writes();
        let security = if url.is_secure() { Security::Tls } else { Security::Plain };
        sink(Output::Connect { connection: ConnectionId::Push, host: url.host_text(), port: url.port(), security });
    }

    pub(super) fn push_connected(&mut self, now: Instant, peer: IpAddress, sink: &mut Sink<'_>) {
        if !matches!(self.push.phase, PushPhase::Connecting { .. }) {
            return;
        }
        let Some(url) = self.push.url else { return };
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
        self.push.frames = FrameReader::new();
        // A channel's push connection may be anywhere on its origin, and
        // carries the channel's cookies, whose paths are its folder
        // (REQ-WEB-026).
        let folder = self.channel;
        let mut writer = Writer::new(self.buffers.push_request);
        self.push.key = ws::new_key(&mut self.random);
        let written = (|| -> Result<bool, Overflow> {
            let subprotocol = Some(PUSH_SUBPROTOCOL);
            ws::write_handshake_headers(&mut writer, &url, &self.push.key, self.named_as.as_bytes(), subprotocol)?;
            // Cookies go with the upgrade too (REQ-WEB-006).
            let path = folder.as_ref().map_or(url.path(), Url::path);
            let truncated = write_cookie_header(
                cookie_store(&mut self.jar, &mut self.port_cookies),
                &mut writer,
                &url,
                path,
                Site::SameService,
                seconds,
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

    /// The push connection failed or closed: wait, then reconnect, with a
    /// ceiling that doubles and a random delay under it (REQ-WEB-005,
    /// DOC-1920).
    pub(super) fn push_lost(&mut self, now: Instant, problem: Option<PushProblem>, close: bool, sink: &mut Sink<'_>) {
        if self.push.url.is_none() || self.push.phase == PushPhase::Idle && problem.is_none() {
            return;
        }
        if close {
            self.close_push(sink);
        }
        self.forget_push_writes();
        if self.push.healthy {
            self.push.delay_us = PUSH_RETRY_FIRST_US;
        }
        let ceiling = self.push.delay_us;
        self.push.delay_us = ceiling.saturating_mul(2).min(PUSH_RETRY_MAX_US);
        let delay = self.spread(ceiling);
        self.push.phase = PushPhase::Backoff { until: Instant(now.0.saturating_add(delay)) };
        self.push.healthy = false;
        event(sink, Event::PushLost { retry_in_us: delay, problem });
    }

    /// A delay in the upper half of `ceiling`, at random: viewers a restart
    /// sent away together do not all come back at once, and none comes back
    /// at once. At most the ceiling, so `RESUME_HOLD_MS`, which counts the
    /// ceilings of the first five retries, still covers them.
    fn spread(&mut self, ceiling: u64) -> u64 {
        let mut drawn = [0_u8; size_of::<u64>()];
        self.random.fill(&mut drawn);
        let half = ceiling / 2;
        let above = u64::from_le_bytes(drawn).checked_rem(ceiling.saturating_sub(half).saturating_add(1));
        half.saturating_add(above.unwrap_or(0))
    }

    /// The push connection ended for good (DOC-1920): no
    /// reconnection follows, and only opening the channel again starts one.
    pub(super) fn push_stop(&mut self, stop: PushStop, sink: &mut Sink<'_>) {
        self.close_push(sink);
        self.forget_push_writes();
        self.push.phase = PushPhase::Stopped;
        self.push.healthy = false;
        event(sink, Event::PushStopped(stop));
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
                    let used = self.push_frames(now, input, sink);
                    match used {
                        Some(used) if used > 0 => input = input.get(used..).unwrap_or(&[]),
                        _ => return,
                    }
                }
                PushPhase::Idle | PushPhase::Stopped | PushPhase::Backoff { .. } | PushPhase::Connecting { .. } => {
                    return;
                }
            }
        }
    }

    fn push_head_complete(&mut self, now: Instant, sink: &mut Sink<'_>) -> bool {
        let Some(url) = self.push.url else { return false };
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
        let accepted = ws::check_handshake(&head, &self.push.key, Some(PUSH_SUBPROTOCOL));
        if let Err(problem) = accepted.map_err(PushProblem::WebSocket) {
            match handshake_stop(&head) {
                Some(stop) => self.push_stop(stop, sink),
                None => self.push_lost(now, Some(problem), true, sink),
            }
            return false;
        }
        self.push.phase = PushPhase::Open { heard: now, pinged: None };
        self.push.invalid = 0;
        self.push.log_lines = 0;
        self.records.clear();
        event(sink, Event::PushConnected);
        self.send_subscribe(sink);
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
            PushPhase::Open { .. } => {
                if self.heartbeat_due().is_some_and(|due| now >= due) {
                    self.heartbeat(now, sink);
                }
                if self.round_trip_due().is_some_and(|due| now >= due) {
                    self.ping_round_trip(now, sink);
                }
            }
            _ => {}
        }
    }

    /// When the next ping that measures the round trip goes (REQ-RCH-016):
    /// while it is measured on an open WebSocket and no ping waits for its
    /// pong, a second after the last, or at once when none has gone.
    pub(super) fn round_trip_due(&self) -> Option<Instant> {
        let measured = self.push.round_trip.on
            && matches!(self.push.phase, PushPhase::Open { .. })
            && self.push.round_trip.waiting.is_none();
        measured.then(|| {
            self.push.round_trip.last.map_or(Instant(0), |last| Instant(last.0.saturating_add(ROUND_TRIP_EVERY_US)))
        })
    }

    /// Sends the ping that measures the round trip: its payload one byte,
    /// counted on, so its pong is told from the heartbeat's, which has none.
    fn ping_round_trip(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let sequence = self.push.round_trip.sequence.wrapping_add(1);
        if self.send_push_frame(OPCODE_PING, &[sequence], sink) {
            self.push.round_trip.sequence = sequence;
            self.push.round_trip.waiting = Some((sequence, now));
            self.push.round_trip.last = Some(now);
        }
    }

    /// A pong: when it answers the round trip's ping, the round trip
    /// (REQ-RCH-016).
    pub(super) fn round_trip_pong(&mut self, now: Instant, payload: &[u8], sink: &mut Sink<'_>) {
        if let Some((sequence, sent)) = self.push.round_trip.waiting
            && payload == [sequence]
        {
            self.push.round_trip.waiting = None;
            event(sink, Event::RoundTrip { microseconds: now.since(sent) });
        }
    }

    /// Turns the round trip's measurement on or off (REQ-RCH-016).
    pub(super) fn measure_round_trip(&mut self, on: bool) {
        self.push.round_trip.on = on;
        if !on {
            self.push.round_trip.waiting = None;
        }
    }

    /// When the heartbeat next acts on the open push connection
    /// (REQ-WEB-005): a ping after a silence, or the end of the wait for its
    /// pong.
    pub(super) fn heartbeat_due(&self) -> Option<Instant> {
        let PushPhase::Open { heard, pinged } = self.push.phase else { return None };
        let after = |at: Instant, wait: u64| Instant(at.0.saturating_add(wait));
        Some(pinged.map_or_else(|| after(heard, PUSH_PING_AFTER_US), |pinged| after(pinged, PUSH_PONG_WITHIN_US)))
    }

    /// A silent connection is pinged, and one that does not answer is
    /// reconnected with the backoff, which sends the subscription again.
    fn heartbeat(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let PushPhase::Open { heard, pinged } = self.push.phase else { return };
        if pinged.is_none() {
            self.send_push_frame(OPCODE_PING, b"", sink);
            self.push.phase = PushPhase::Open { heard, pinged: Some(now) };
            return;
        }
        self.push_lost(now, Some(PushProblem::Failure(Failure::Timeout)), true, sink);
    }
}
