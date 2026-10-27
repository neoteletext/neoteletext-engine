//! The fetch connection's events: connected, closed, lost, bytes
//! received into the head and the body, and an exchange failed or
//! reported (REQ-WEB-033).

use neoteletext_core::types::Instant;

use super::cookies::{cookie_store, store_response_cookies};
use super::{BOUND_INSTRUCTIONS_LIMIT, Event, Exchange, Failure, Link, Output, Purpose, Sink, UserAgent, event};
use crate::bytes::copy_prefix;
use crate::consts::STATUS_SUCCESS_FIRST;
use crate::http::{self, BodyDecoder, HttpError};
use crate::port::{ConnectionId, RandomSource};
use crate::url::{IpAddress, Url};

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    pub(super) fn fetch_connected(&mut self, now: Instant, peer: IpAddress, sink: &mut Sink<'_>) {
        if self.fetch.link != Link::Connecting {
            return;
        }
        let Some(target) = self.fetch.target else { return };
        if self.peer_blocked(peer) {
            sink(Output::Close { connection: ConnectionId::Fetch });
            self.fetch.link = Link::Closed;
            event(sink, Event::Blocked { connection: ConnectionId::Fetch, host: target.host.as_str() });
            self.fail_exchange(Failure::Blocked, sink);
            return;
        }
        self.fetch.link = Link::Open;
        if target.secure {
            self.remember_secure(target.host.as_str());
        } else {
            event(sink, Event::Unencrypted { connection: ConnectionId::Fetch, host: target.host.as_str() });
        }
        self.send_request(now, sink);
    }

    pub(super) fn fetch_closed(&mut self, now: Instant, sink: &mut Sink<'_>) {
        self.fetch.link = Link::Closed;
        let Some(exchange) = self.fetch.exchange else { return };
        if let Some(body) = &mut self.fetch.body {
            match body.finish_at_close() {
                Ok(()) => {
                    self.complete(now, sink);
                    return;
                }
                Err(error) => {
                    self.fail_exchange(Failure::Http(error), sink);
                    return;
                }
            }
        }
        // A kept-alive connection the server closed before answering:
        // retry a GET once on a new connection (RFC 9112 section 9.3.1).
        // A bound session's POST is never repeated (RFC 9110 section
        // 9.2.2).
        let idempotent = !matches!(exchange.purpose, Purpose::Register | Purpose::Refresh { .. });
        if idempotent && exchange.sent && exchange.reused && !exchange.retried && !self.fetch.started {
            if let Some(current) = &mut self.fetch.exchange {
                current.retried = true;
                current.sent = false;
            }
            self.start_exchange(now, sink);
            return;
        }
        self.fail_exchange(Failure::Network, sink);
    }

    pub(super) fn fetch_lost(&mut self, _now: Instant, failure: Failure, sink: &mut Sink<'_>) {
        self.fetch.link = Link::Closed;
        self.fail_exchange(failure, sink);
    }

    pub(super) fn fetch_received(&mut self, now: Instant, mut input: &[u8], sink: &mut Sink<'_>) {
        if self.fetch.exchange.is_none() || self.fetch.link != Link::Open {
            // Bytes nobody asked for: the connection cannot be trusted.
            if !input.is_empty() {
                sink(Output::Close { connection: ConnectionId::Fetch });
                self.fetch.link = Link::Closed;
            }
            return;
        }
        self.fetch.started = true;
        while !input.is_empty() {
            if self.fetch.response.is_none() {
                let limit = self.config.header_limit.min(self.buffers.fetch_head.len());
                let buffer = self.buffers.fetch_head.get_mut(..limit).unwrap_or(&mut []);
                match self.fetch.head.feed(buffer, input) {
                    Ok((used, complete)) => {
                        input = input.get(used..).unwrap_or(&[]);
                        let head = self.buffers.fetch_head.get(..self.fetch.head.len()).unwrap_or(&[]);
                        if complete && http::is_interim(head) {
                            // RFC 9110 section 15.2: the final head follows.
                            self.fetch.head.reset();
                        } else if complete && !self.fetch_head_complete(now, sink) {
                            return;
                        }
                    }
                    Err(error) => {
                        self.abandon_fetch(Failure::Http(error), sink);
                        return;
                    }
                }
                continue;
            }
            let Some(body) = &mut self.fetch.body else { break };
            match body.feed(input) {
                Ok(step) => {
                    let end = self.fetch.body_len.saturating_add(step.data.len());
                    if let Some(target) = self.buffers.body.get_mut(self.fetch.body_len..end) {
                        copy_prefix(target, step.data);
                        self.fetch.body_len = end;
                    } else {
                        self.abandon_fetch(Failure::Http(HttpError::BodyTooLarge), sink);
                        return;
                    }
                    input = input.get(step.consumed..).unwrap_or(&[]);
                    if step.done {
                        break;
                    }
                    if step.consumed == 0 {
                        break;
                    }
                }
                Err(error) => {
                    self.abandon_fetch(Failure::Http(error), sink);
                    return;
                }
            }
        }
        if self.fetch.body.is_some_and(|body| body.is_done()) {
            if !input.is_empty() {
                // Bytes after the response: no pipelining is used, so the
                // connection is out of step and is closed.
                self.close_fetch_after = true;
            }
            self.complete(now, sink);
        }
    }

    /// Parses a complete head. Returns false when the exchange ended.
    fn fetch_head_complete(&mut self, now: Instant, sink: &mut Sink<'_>) -> bool {
        let Some(exchange) = self.fetch.exchange else { return false };
        let (seconds, calendar) = self.seconds(now);
        let head_len = self.fetch.head.len();
        let Self { buffers, jar, port_cookies, channel, .. } = self;
        let head_bytes = buffers.fetch_head.get(..head_len).unwrap_or(&[]);
        let host = exchange.url.host_text();
        let path = exchange.url.path();
        let secure = exchange.url.is_secure();
        // A channel's cookies stay in its folder (REQ-WEB-026), and only a
        // response from inside it, or from its upgrade to `https`, sets or
        // clears any: a redirect elsewhere touches none (decision E.114
        // item 2, finding F-42).
        let inside =
            channel.as_ref().is_none_or(|folder| crate::channel::redirect_keeps_channel(folder, &exchange.url));
        let scope = channel.as_ref().map(Url::path);
        let parsed = if inside {
            let store = cookie_store(jar, port_cookies);
            store_response_cookies(store, head_bytes, host, path, scope, secure, seconds, calendar, sink)
        } else {
            http::parse_head(head_bytes, &mut |_| {})
        };
        let head = match parsed {
            Ok(head) => head,
            Err(error) => {
                self.abandon_fetch(Failure::Http(error), sink);
                return false;
            }
        };
        if head.status < STATUS_SUCCESS_FIRST {
            self.abandon_fetch(Failure::Status(head.status), sink);
            return false;
        }
        let limit = match exchange.purpose {
            Purpose::Descriptor { .. } => self.config.descriptor_limit,
            // A front door is bounded like a page: a bundled site larger
            // than the page limit publishes page files instead.
            Purpose::Page { .. } | Purpose::FrontDoor => self.config.page_limit,
            Purpose::Contents => crate::contents::MAX_BYTES,
            Purpose::Register | Purpose::Refresh { .. } => BOUND_INSTRUCTIONS_LIMIT,
        }
        .min(self.buffers.body.len());
        self.bound_head(now, &exchange, &head, sink);
        match BodyDecoder::new(head.framing, limit) {
            Ok(decoder) => {
                self.fetch.response = Some(head);
                self.fetch.body = Some(decoder);
                true
            }
            Err(error) => {
                self.abandon_fetch(Failure::Http(error), sink);
                false
            }
        }
    }

    /// Closes the fetch connection and fails the exchange.
    pub(super) fn abandon_fetch(&mut self, failure: Failure, sink: &mut Sink<'_>) {
        sink(Output::Close { connection: ConnectionId::Fetch });
        self.fetch.link = Link::Closed;
        self.fail_exchange(failure, sink);
    }

    pub(super) fn fail_exchange(&mut self, failure: Failure, sink: &mut Sink<'_>) {
        self.fetch.deadline = None;
        self.fetch.response = None;
        self.fetch.body = None;
        if let Some(exchange) = self.fetch.exchange.take() {
            self.finish_purpose(exchange.purpose, Err(failure), sink);
        }
    }

    pub(super) fn finish_purpose(&mut self, purpose: Purpose, result: Result<(), Failure>, sink: &mut Sink<'_>) {
        let Err(failure) = result else { return };
        match purpose {
            Purpose::Descriptor { .. } => event(sink, Event::DescriptorFailed(failure)),
            Purpose::FrontDoor => event(sink, Event::BundleFailed(failure)),
            Purpose::Contents => event(sink, Event::ContentsFailed(failure)),
            Purpose::Page { page, subcode, .. } => event(sink, Event::FetchFailed { page, subcode, failure }),
            Purpose::Register | Purpose::Refresh { .. } => self.bound_failed(purpose, sink),
        }
    }

    /// A response is complete: act on it.
    /// REQ-WEB-033: each exchange reported as it completes, for a
    /// developer's view of the channel.
    pub(super) fn report_exchange(&self, now: Instant, exchange: &Exchange, status: u16, sink: &mut Sink<'_>) {
        event(
            sink,
            Event::Exchanged {
                method: exchange.method,
                url: exchange.url.as_str(),
                status,
                bytes: self.fetch.body_len,
                elapsed_us: now.0.saturating_sub(exchange.started.0),
            },
        );
    }
}
