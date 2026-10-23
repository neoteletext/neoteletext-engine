//! Requests on the fetch connection (REQ-WEB-008, REQ-WEB-014): what is
//! refused before connecting (REQ-SEC-006, REQ-SEC-007), the scheduler
//! with the rate limit and the queue, and the request written.

use neoteletext_core::types::Instant;

#[cfg(feature = "dbsc")]
use super::bound;
use super::cookies::{cookie_store, write_cookie_header};
use super::{Event, Exchange, Failure, FormPhase, Link, Output, PageFile, Purpose, Sink, Target, UserAgent, event};
use crate::bytes::{Overflow, Writer};
use crate::channel::{self, ChannelFile};
use crate::consts::{ACCEPT_PAGES, CRLF, IDENTITY, header, media};
use crate::cookie::Site;
use crate::http::{self, Method};
use crate::port::{ConnectionId, RandomSource, Security};
use crate::url::{IpAddress, Url};

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    /// The first refusal that applies to a request to `url`, before any
    /// connection is made.
    pub(super) fn refusal(&self, now: Instant, url: &Url) -> Option<Failure> {
        if url.is_secure() && !self.config.tls_available {
            return Some(Failure::TlsUnavailable);
        }
        if self.is_downgrade(now, url) {
            return Some(Failure::Downgrade);
        }
        if url.ip().is_some_and(|ip| self.blocked(ip)) {
            return Some(Failure::Blocked);
        }
        // Through a proxy, names are resolved there: a local name is
        // refused before the proxy is asked (REQ-WEB-032, REQ-SEC-006).
        let allowed = url.ip().is_some_and(|ip| self.config.allowed_local.contains(&Some(ip)));
        if self.config.proxied && !allowed && crate::proxy::hidden_local(url.host_text()) {
            return Some(Failure::Blocked);
        }
        None
    }

    fn blocked(&self, address: IpAddress) -> bool {
        address.is_blocked() && !self.config.allowed_local.contains(&Some(address))
    }

    /// Whether the peer a connection reached is refused: never through a
    /// proxy, whose address it is (REQ-WEB-032).
    pub(super) fn peer_blocked(&self, peer: IpAddress) -> bool {
        !self.config.proxied && self.blocked(peer)
    }

    pub(super) fn report_refusal(connection: ConnectionId, url: &Url, failure: Failure, sink: &mut Sink<'_>) {
        match failure {
            Failure::Blocked => event(sink, Event::Blocked { connection, host: url.host_text() }),
            Failure::Downgrade => event(sink, Event::DowngradeRefused { host: url.host_text() }),
            _ => {}
        }
    }

    /// Starts the next request when the fetch connection is free.
    pub(super) fn dispatch(&mut self, now: Instant, sink: &mut Sink<'_>) {
        while self.fetch.exchange.is_none() {
            let (purpose, url, method, site) = if let Some(bound) = self.bound_next(sink) {
                bound
            } else if let Some((url, via_link, channel)) = self.pending_descriptor.take() {
                let site = if via_link { Site::CrossServiceNavigation } else { Site::SameService };
                (Purpose::Descriptor { via_link, channel }, url, Method::Get, site)
            } else if let Some(url) = self.front_door.take() {
                (Purpose::FrontDoor, url, Method::Get, Site::SameService)
            } else if let Some((page, subcode, t42)) = self.second_try.take() {
                // The other file of a page whose first answered 404 or 410
                // (REQ-WEB-019), without waiting for the rate limit: it is
                // the same request, completed.
                let Some(folder) = self.channel else { continue };
                match channel::file_url(&folder, ChannelFile::Page { page, t42 }) {
                    Ok(url) => (
                        Purpose::Page { page, subcode, file: PageFile::Channel { t42, second: true } },
                        url,
                        Method::Get,
                        Site::SameService,
                    ),
                    Err(error) => {
                        event(sink, Event::FetchFailed { page, subcode, failure: Failure::Address(error) });
                        continue;
                    }
                }
            } else if let Some(url) = self.contents.take() {
                (Purpose::Contents, url, Method::Get, Site::SameService)
            } else if cfg!(feature = "forms") && self.form.phase == FormPhase::Queued {
                let Some(url) = self.form.target else {
                    self.form.phase = FormPhase::Idle;
                    continue;
                };
                self.form.phase = FormPhase::InFlight;
                (Purpose::Form, url, self.form.method, self.form.site)
            } else if self.queue.iter().any(Option::is_some) && now >= self.next_fetch_at {
                let Some((page, subcode)) = self.take_queued() else { return };
                let Some(descriptor) = &self.descriptor else {
                    event(sink, Event::FetchFailed { page, subcode, failure: Failure::NoService });
                    continue;
                };
                if self
                    .cache_entry(page, subcode)
                    .is_some_and(|entry| entry.fresh_until.is_some_and(|until| now < until))
                {
                    event(sink, Event::PageCurrent { page, subcode });
                    continue;
                }
                let (address, file) = match self.channel {
                    Some(folder) => {
                        // The file that answered last time is asked again,
                        // so its validators apply; otherwise the extension
                        // that last answered in the channel.
                        let t42 = self.cache_entry(page, subcode).map_or(self.t42_first, |entry| entry.t42);
                        (
                            channel::file_url(&folder, ChannelFile::Page { page, t42 }),
                            PageFile::Channel { t42, second: false },
                        )
                    }
                    None => (descriptor.page_url(page, subcode), PageFile::Template),
                };
                let url = match address {
                    Ok(url) => url,
                    Err(error) => {
                        event(sink, Event::FetchFailed { page, subcode, failure: Failure::Address(error) });
                        continue;
                    }
                };
                let site = if core::mem::take(&mut self.navigation_pending) {
                    Site::CrossServiceNavigation
                } else {
                    Site::SameService
                };
                (Purpose::Page { page, subcode, file }, url, Method::Get, site)
            } else {
                return;
            };
            // REQ-WEB-017: a request that would go without a bound
            // session's cookie waits for the session's refresh.
            if self.bound_defer(now, purpose, &url, method, site) {
                continue;
            }
            self.next_fetch_at = Instant(now.0.saturating_add(self.config.fetch_interval_us));
            if let Some(failure) = self.refusal(now, &url) {
                Self::report_refusal(ConnectionId::Fetch, &url, failure, sink);
                self.finish_purpose(purpose, Err(failure), sink);
                continue;
            }
            self.fetch.exchange = Some(Exchange {
                purpose,
                url,
                method,
                redirects: 0,
                site,
                deadline: Instant(now.0.saturating_add(self.config.response_timeout_us)),
                sent: false,
                reused: false,
                retried: false,
                started: now,
            });
            self.start_exchange(now, sink);
        }
    }

    /// Sends the current exchange's request, connecting first if needed.
    pub(super) fn start_exchange(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let Some(exchange) = self.fetch.exchange else { return };
        let reusable =
            self.fetch.link == Link::Open && self.fetch.target.is_some_and(|target| target.matches(&exchange.url));
        if reusable {
            self.send_request(now, sink);
            return;
        }
        if self.fetch.link != Link::Closed {
            sink(Output::Close { connection: ConnectionId::Fetch });
        }
        let target = Target::of(&exchange.url);
        self.fetch.target = Some(target);
        self.fetch.link = Link::Connecting;
        self.fetch.used = false;
        self.fetch.deadline = Some(Instant(now.0.saturating_add(self.config.connect_timeout_us)));
        let security = if target.secure { Security::Tls } else { Security::Plain };
        sink(Output::Connect {
            connection: ConnectionId::Fetch,
            host: exchange.url.host_text(),
            port: target.port,
            security,
        });
    }

    fn write_request(&mut self, now: Instant, exchange: &Exchange, sink: &mut Sink<'_>) -> Result<usize, Overflow> {
        let (seconds, _) = self.seconds(now);
        // Only a form's request has a body; the bound sessions' requests
        // are posted empty (REQ-WEB-017).
        let body_len =
            if exchange.method == Method::Post && exchange.purpose == Purpose::Form { self.form.body_len } else { 0 };
        let mut writer = Writer::new(self.buffers.fetch_request);
        http::write_request_line(&mut writer, exchange.method, exchange.url.target())?;
        http::write_host(&mut writer, &exchange.url)?;
        http::write_header(&mut writer, header::USER_AGENT, self.named_as.as_bytes())?;
        http::write_header(&mut writer, header::ACCEPT, ACCEPT_PAGES)?;
        http::write_header(&mut writer, header::ACCEPT_ENCODING, IDENTITY)?;
        if let Purpose::Page { page, subcode, .. } = exchange.purpose
            && exchange.redirects == 0
            && let Some(entry) = self.cache.iter().flatten().find(|e| e.page == page && e.subcode == subcode)
        {
            if let Some(etag) = &entry.etag {
                http::write_header(&mut writer, header::IF_NONE_MATCH, etag.as_bytes())?;
            }
            if let Some(date) = &entry.last_modified {
                http::write_header(&mut writer, header::IF_MODIFIED_SINCE, date.as_bytes())?;
            }
        }
        if exchange.method == Method::Post {
            if exchange.purpose == Purpose::Form {
                http::write_header(&mut writer, header::CONTENT_TYPE, media::FORM_URLENCODED)?;
            }
            writer.put(header::CONTENT_LENGTH)?;
            writer.put(b": ")?;
            writer.decimal(body_len as u64)?;
            writer.put(CRLF)?;
        }
        #[cfg(feature = "dbsc")]
        bound::write_headers(&mut self.bound, self.device_keys.as_deref_mut(), exchange, &mut writer)?;
        let path = exchange.url.path();
        if write_cookie_header(
            cookie_store(&mut self.jar, &mut self.port_cookies),
            &mut writer,
            &exchange.url,
            path,
            exchange.site,
            seconds,
            body_len,
        )? {
            event(sink, Event::CookiesTruncated);
        }
        writer.put(CRLF)?;
        writer.put(self.buffers.form.get(..body_len).unwrap_or(&[]))?;
        Ok(writer.len())
    }

    pub(super) fn send_request(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let Some(exchange) = self.fetch.exchange else { return };
        match self.write_request(now, &exchange, sink) {
            Ok(len) => {
                if let Some(current) = &mut self.fetch.exchange {
                    current.sent = true;
                    current.reused = self.fetch.used;
                }
                self.fetch.head.reset();
                self.fetch.response = None;
                self.fetch.body = None;
                self.fetch.body_len = 0;
                self.fetch.started = false;
                self.fetch.deadline = Some(exchange.deadline);
                let bytes = self.buffers.fetch_request.get(..len).unwrap_or(&[]);
                sink(Output::Send { connection: ConnectionId::Fetch, bytes });
            }
            Err(Overflow) => self.fail_exchange(Failure::RequestTooLarge, sink),
        }
    }
}
