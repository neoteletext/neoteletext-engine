//! The agent's one entry point, [`UserAgent::handle`]: each [`Input`]
//! given to the part it concerns, then the requests dispatched and the
//! port told its next timer (decision E.103).

use neoteletext_core::types::Instant;

use super::{Event, Failure, Input, Output, PushPhase, Sink, UserAgent, event};
use crate::channel;
use crate::form::FormRefused;
use crate::port::{ConnectionId, NetInput, RandomSource};
use crate::url::{Url, UrlError};

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    /// Handles one input. Outputs go to `sink` before this returns.
    pub fn handle(&mut self, now: Instant, input: Input<'_>, sink: &mut Sink<'_>) {
        let ticked = matches!(input, Input::Tick);
        match input {
            Input::Tick => self.tick(now, sink),
            Input::Net(net) => match net {
                NetInput::Connected(ConnectionId::Fetch, peer) => self.fetch_connected(now, peer, sink),
                NetInput::ConnectFailed(ConnectionId::Fetch) => self.fetch_lost(now, Failure::Network, sink),
                NetInput::Received(ConnectionId::Fetch, bytes) => self.fetch_received(now, bytes, sink),
                NetInput::Closed(ConnectionId::Fetch) => self.fetch_closed(now, sink),
                NetInput::Connected(ConnectionId::Push, peer) if cfg!(feature = "push") => {
                    self.push_connected(now, peer, sink);
                }
                NetInput::ConnectFailed(ConnectionId::Push) | NetInput::Closed(ConnectionId::Push)
                    if cfg!(feature = "push") =>
                {
                    let live = matches!(
                        self.push.phase,
                        PushPhase::Connecting { .. } | PushPhase::Handshake { .. } | PushPhase::Open { .. }
                    );
                    if live {
                        self.push_lost(now, None, false, sink);
                    }
                }
                NetInput::Received(ConnectionId::Push, bytes) if cfg!(feature = "push") => {
                    self.push_received(now, bytes, sink);
                }
                // The live feed has its own machine, and a build without
                // push never opens a push connection.
                NetInput::Connected(ConnectionId::Push | ConnectionId::Live, _)
                | NetInput::ConnectFailed(ConnectionId::Push | ConnectionId::Live)
                | NetInput::Received(ConnectionId::Push | ConnectionId::Live, _)
                | NetInput::Closed(ConnectionId::Push | ConnectionId::Live) => {}
            },
            Input::OpenService(address) => match Url::parse(address) {
                Ok(url) if !url.scheme().is_websocket() => {
                    self.leave_channel();
                    self.pending_descriptor = Some((url, false, false));
                }
                Ok(_) => event(sink, Event::DescriptorFailed(Failure::Address(UrlError::Scheme))),
                Err(error) => event(sink, Event::DescriptorFailed(Failure::Address(error))),
            },
            Input::OpenChannel(address) => match channel::parse_address(address) {
                Ok(found) => self.open_channel(&found.folder, false),
                Err(channel::ChannelError::Address(error)) => {
                    event(sink, Event::DescriptorFailed(Failure::Address(error)));
                }
                Err(_) => event(sink, Event::DescriptorFailed(Failure::Address(UrlError::Path))),
            },
            Input::FetchContents => self.fetch_contents(sink),
            Input::FollowLink { service, page, subcode } => self.follow_link(service, page, subcode, sink),
            Input::FetchPage { page, subcode } => self.enqueue(page, subcode, sink),
            Input::Subscribe(subscription) => self.subscribe(now, subscription, sink),
            Input::Submit(submission) => {
                if cfg!(feature = "forms") {
                    self.submit(&submission, sink);
                } else {
                    event(sink, Event::SubmissionRefused(FormRefused::NotCompiledIn));
                }
            }
            // Without forms nothing ever waits for a confirmation.
            Input::ConfirmSubmission(confirmed) => {
                if cfg!(feature = "forms") {
                    self.confirm(confirmed, sink);
                }
            }
            Input::WipeCookies => self.wipe_cookies(),
            Input::WallClock(unix) => self.wall_clock = Some((now, unix)),
            Input::Send(message) => self.send_input(message, sink),
        }
        self.dispatch(now, sink);
        // The port is told its timer when the deadline changes, and again
        // after a tick, which spent the one it had; not after every input,
        // which woke the port twice for each (decision E.103).
        let deadline = self.next_deadline();
        if ticked || deadline != self.timer_told {
            self.timer_told = deadline;
            if let Some(at) = deadline {
                sink(Output::Timer { at });
            }
        }
    }

    fn tick(&mut self, now: Instant, sink: &mut Sink<'_>) {
        if let Some(deadline) = self.fetch.deadline
            && now >= deadline
        {
            self.abandon_fetch(Failure::Timeout, sink);
        }
        self.push_tick(now, sink);
    }

    fn next_deadline(&self) -> Option<Instant> {
        let push = match self.push.phase {
            PushPhase::Backoff { until } => Some(until),
            PushPhase::Connecting { deadline } | PushPhase::Handshake { deadline } => Some(deadline),
            PushPhase::Open { .. } => self.heartbeat_due(),
            PushPhase::Idle => None,
        };
        let queue =
            (self.fetch.exchange.is_none() && self.queue.iter().any(Option::is_some)).then_some(self.next_fetch_at);
        [self.fetch.deadline, push, queue].into_iter().flatten().min()
    }
}
