//! A complete response: acted on by its purpose, or redirected
//! (REQ-WEB-003, REQ-WEB-021, REQ-SEC-007).

use neoteletext_core::types::Instant;

use super::{Event, Exchange, Failure, FormPhase, Link, Output, Purpose, Sink, UserAgent, event};
use crate::channel::{self, ChannelFile};
use crate::consts::{
    MAX_REDIRECTS, MAX_RETRY_AFTER_US, MICROSECONDS_PER_SECOND, STATUS_FOUND, STATUS_MOVED_PERMANENTLY, STATUS_OK,
    STATUS_PERMANENT_REDIRECT, STATUS_SEE_OTHER, STATUS_TEMPORARY_REDIRECT,
};
use crate::cookie::Site;
use crate::descriptor::Descriptor;
use crate::form::FormRefused;
use crate::http::{MediaType, Method, ResponseHead};
use crate::port::{ConnectionId, RandomSource};
use crate::url::{Url, UrlError};

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    pub(super) fn complete(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let (Some(exchange), Some(head)) = (self.fetch.exchange, self.fetch.response) else { return };
        self.fetch.deadline = None;
        self.fetch.used = true;
        self.fetch.body = None;
        self.fetch.response = None;
        if head.close || core::mem::take(&mut self.close_fetch_after) {
            sink(Output::Close { connection: ConnectionId::Fetch });
            self.fetch.link = Link::Closed;
        }
        if let Some(seconds) = head.retry_after {
            let delay = u64::from(seconds).saturating_mul(MICROSECONDS_PER_SECOND).min(MAX_RETRY_AFTER_US);
            self.next_fetch_at = self.next_fetch_at.max(Instant(now.0.saturating_add(delay)));
        }
        let status = head.status;
        self.report_exchange(now, &exchange, status, sink);
        // A bound session's exchange is never redirected (REQ-WEB-017).
        if matches!(exchange.purpose, Purpose::Register | Purpose::Refresh { .. }) {
            self.fetch.exchange = None;
            self.bound_complete(now, &exchange, &head, sink);
            return;
        }
        let redirect = matches!(
            status,
            STATUS_MOVED_PERMANENTLY
                | STATUS_FOUND
                | STATUS_SEE_OTHER
                | STATUS_TEMPORARY_REDIRECT
                | STATUS_PERMANENT_REDIRECT
        );
        if redirect {
            self.redirect(now, exchange, &head, sink);
            return;
        }
        self.fetch.exchange = None;
        if self.complete_channel(now, &exchange, &head, sink) {
            return;
        }
        let body = self.buffers.body.get(..self.fetch.body_len).unwrap_or(&[]);
        match exchange.purpose {
            // complete_channel handled every one of these, and
            // bound_complete the bound sessions'.
            Purpose::FrontDoor | Purpose::Contents | Purpose::Register | Purpose::Refresh { .. } => {}
            Purpose::Descriptor { via_link, channel } => {
                // A channel's settings may come as any type REQ-WEB-027
                // accepts for them; a service's descriptor only as JSON.
                let media_accepted = head.media == MediaType::Json
                    || channel && matches!(head.media, MediaType::TextPlain | MediaType::OctetStream);
                if status != STATUS_OK {
                    event(sink, Event::DescriptorFailed(Failure::Status(status)));
                } else if !media_accepted {
                    event(sink, Event::DescriptorFailed(Failure::MediaType));
                } else {
                    let parsed = if channel {
                        Descriptor::parse_channel(body, &exchange.url)
                    } else {
                        Descriptor::parse(body, &exchange.url)
                    };
                    match parsed {
                        Ok(descriptor) => {
                            self.open_descriptor(now, descriptor, via_link, sink);
                            if channel {
                                self.front_door = self
                                    .channel
                                    .and_then(|folder| channel::file_url(&folder, ChannelFile::Bundle).ok());
                            }
                        }
                        Err(error) => event(sink, Event::DescriptorRejected(error)),
                    }
                }
            }
            Purpose::Page { page, subcode, file } => {
                self.complete_page(now, &exchange, &head, (page, subcode, file), sink);
            }
            // Only a build with forms makes a form request.
            Purpose::Form if !cfg!(feature = "forms") => {}
            Purpose::Form => self.complete_form(&exchange, &head, sink),
        }
    }

    fn redirect(&mut self, now: Instant, mut exchange: Exchange, head: &ResponseHead, sink: &mut Sink<'_>) {
        self.fetch.exchange = None;
        if exchange.redirects >= MAX_REDIRECTS {
            self.finish_purpose(exchange.purpose, Err(Failure::TooManyRedirects), sink);
            return;
        }
        // An empty or non-UTF-8 `Location` leads nowhere: it would resolve
        // to the request's own address (decision E.114 item 4, F-44).
        let location = head.location.as_ref().and_then(|location| core::str::from_utf8(location.as_bytes()).ok());
        let Some(location) = location.filter(|location| !location.is_empty()) else {
            self.finish_purpose(exchange.purpose, Err(Failure::BadRedirect), sink);
            return;
        };
        let next = match exchange.url.join(location) {
            Ok(url) if !url.scheme().is_websocket() => url,
            Ok(_) => {
                self.finish_purpose(exchange.purpose, Err(Failure::Address(UrlError::Scheme)), sink);
                return;
            }
            Err(error) => {
                self.finish_purpose(exchange.purpose, Err(Failure::Address(error)), sink);
                return;
            }
        };
        // REQ-SEC-007: a redirect from HTTPS to plain HTTP is refused,
        // whichever host it names.
        let downgrade = exchange.url.is_secure() && !next.is_secure();
        let failure = if downgrade { Some(Failure::Downgrade) } else { self.refusal(now, &next) };
        if let Some(failure) = failure {
            Self::report_refusal(ConnectionId::Fetch, &next, failure, sink);
            self.finish_purpose(exchange.purpose, Err(failure), sink);
            return;
        }
        // RFC 9110 section 15.4: 303 always becomes GET, and 301 and 302
        // do for POST, as browsers do; 307 and 308 keep the method.
        let status = head.status;
        // The same address with the same method is the same request again.
        let method_changes = status == STATUS_SEE_OTHER
            || ((status == STATUS_MOVED_PERMANENTLY || status == STATUS_FOUND) && exchange.method == Method::Post);
        let next_method = if method_changes { Method::Get } else { exchange.method };
        if next.as_str() == exchange.url.as_str() && next_method == exchange.method {
            self.finish_purpose(exchange.purpose, Err(Failure::BadRedirect), sink);
            return;
        }
        // A form whose method and body a 307 or 308 would keep is not sent
        // to any origin but its target's: the viewer's values and token
        // would reach an origin neither checked nor confirmed (REQ-SEC-009,
        // REQ-WEB-008, decision E.112, finding F-16).
        let method_kept = status == STATUS_TEMPORARY_REDIRECT || status == STATUS_PERMANENT_REDIRECT;
        if exchange.purpose == Purpose::Form
            && method_kept
            && !self.form.target.is_some_and(|target| next.origin().same(&target.origin()))
        {
            self.form.phase = FormPhase::Idle;
            event(sink, Event::SubmissionRefused(FormRefused::Origin));
            return;
        }
        if method_changes {
            exchange.method = Method::Get;
            self.form.body_len = 0;
        }
        // Another channel is confirmed before anything from it is shown
        // (REQ-WEB-021). Forms keep their own rule below.
        if self.config.redirects_within_channel
            && exchange.purpose != Purpose::Form
            && !self.redirect_keeps_channel(&exchange.url, &next)
        {
            event(sink, Event::RedirectToOtherChannel { location: next.as_str() });
            self.finish_purpose(exchange.purpose, Err(Failure::RedirectToOtherChannel), sink);
            return;
        }
        // A redirect to another origin makes a form submission cross-site.
        if exchange.purpose == Purpose::Form && !next.origin().same(&exchange.url.origin()) {
            exchange.site = Site::CrossOriginForm;
        }
        exchange.url = next;
        exchange.redirects = exchange.redirects.saturating_add(1);
        exchange.sent = false;
        exchange.retried = false;
        exchange.started = now;
        self.fetch.exchange = Some(exchange);
        self.start_exchange(now, sink);
    }

    /// A redirect from `from` to `next` keeps the channel (REQ-WEB-021): for
    /// a channel, inside its folder; for a service opened by its descriptor,
    /// on the same origin. Either way, an upgrade from `http` to `https` on
    /// the same host between the default ports keeps it.
    fn redirect_keeps_channel(&self, from: &Url, next: &Url) -> bool {
        if let Some(folder) = &self.channel {
            return channel::redirect_keeps_channel(folder, next);
        }
        let upgrade = !from.is_secure()
            && next.is_secure()
            && next.host_text() == from.host_text()
            && from.port() == crate::url::Scheme::Http.default_port()
            && next.port() == crate::url::Scheme::Https.default_port();
        next.origin().same(&from.origin()) || upgrade
    }
}
