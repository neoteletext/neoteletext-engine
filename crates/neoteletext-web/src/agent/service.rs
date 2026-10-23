//! Opening a service or a channel (REQ-WEB-002, REQ-WEB-018 to
//! REQ-WEB-027): its descriptor or settings, its bundle and contents, the
//! links to other services, and the checks of a channel's files.

use neoteletext_core::types::{Instant, PageAddress, SubCode};

use super::{Event, Exchange, Failure, Output, PageBytes, Purpose, PushPhase, Sink, Source, UserAgent, event};
use crate::channel::{self, ChannelFile};
use crate::consts::{CACHE_ENTRIES, PAGE_QUEUE, PUSH_RETRY_FIRST_US, STATUS_GONE, STATUS_NOT_FOUND, STATUS_OK};
use crate::descriptor::{Descriptor, PushMethod};
use crate::http::{MediaType, ResponseHead};
use crate::port::{ConnectionId, RandomSource};
use crate::sse::SseParser;
use crate::url::Url;

/// Checks a channel's page file or bundle (REQ-WEB-027): a media type
/// accepted for its extension, and content of the format its name says.
/// Returns the format, which the caller need not detect again.
pub(super) fn channel_file_media(t42: bool, media: MediaType, body: &[u8]) -> Result<MediaType, Failure> {
    let (accepted, content, format) = if t42 {
        (
            matches!(media, MediaType::T42 | MediaType::OctetStream | MediaType::TextPlain),
            channel::is_t42(body),
            MediaType::T42,
        )
    } else {
        (
            matches!(media, MediaType::Tti | MediaType::TextPlain | MediaType::OctetStream),
            channel::is_tti(body),
            MediaType::Tti,
        )
    };
    if !accepted {
        return Err(Failure::MediaType);
    }
    if !content {
        return Err(Failure::Content);
    }
    Ok(format)
}

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    /// Forgets the open channel: what follows is a service opened by its
    /// descriptor.
    pub(super) fn leave_channel(&mut self) {
        self.channel = None;
        self.front_door = None;
        self.contents = None;
        self.second_try = None;
    }

    /// Starts opening a channel: its settings first (REQ-WEB-018).
    pub(super) fn open_channel(&mut self, folder: &Url, via_link: bool) {
        self.leave_channel();
        self.t42_first = false;
        // The folder is checked, so its settings' address always joins.
        if let Ok(settings) = channel::file_url(folder, ChannelFile::Settings) {
            self.pending_descriptor = Some((settings, via_link, true));
        }
        self.channel = Some(*folder);
    }

    /// The open service's folder: its channel's, or the folder holding its
    /// descriptor (REQ-WEB-018).
    fn folder(&self) -> Option<Url> {
        self.channel.or_else(|| self.descriptor.as_ref().and_then(|d| d.address().join(".").ok()))
    }

    pub(super) fn fetch_contents(&mut self, sink: &mut Sink<'_>) {
        match self.folder().map(|folder| channel::file_url(&folder, ChannelFile::Contents)) {
            Some(Ok(url)) => self.contents = Some(url),
            Some(Err(error)) => event(sink, Event::ContentsFailed(Failure::Address(error))),
            None => event(sink, Event::ContentsFailed(Failure::NoService)),
        }
    }

    pub(super) fn follow_link(
        &mut self,
        service: &str,
        page: PageAddress,
        subcode: Option<SubCode>,
        sink: &mut Sink<'_>,
    ) {
        let linked =
            self.descriptor.as_ref().and_then(|d| d.service(service).map(|s| (d.service_address(s), s.channel)));
        let address = linked.map(|(address, _)| address);
        let to_channel = linked.is_some_and(|(_, channel)| channel);
        match address.map(Url::parse) {
            Some(Ok(url)) => {
                if to_channel {
                    self.open_channel(&url, true);
                } else {
                    self.leave_channel();
                    self.pending_descriptor = Some((url, true, false));
                }
                self.queue = [None; PAGE_QUEUE];
                if let Some(slot) = self.queue.first_mut() {
                    *slot = Some((page, subcode));
                }
            }
            _ => event(sink, Event::LinkUnknown),
        }
    }

    /// The responses that belong to a channel (REQ-WEB-018): settings that
    /// are missing mean the defaults, the bundle's pages are delivered,
    /// and the contents are passed on. Returns true when the response was
    /// one of these.
    pub(super) fn complete_channel(
        &mut self,
        now: Instant,
        exchange: &Exchange,
        head: &ResponseHead,
        sink: &mut Sink<'_>,
    ) -> bool {
        let status = head.status;
        let missing = status == STATUS_NOT_FOUND || status == STATUS_GONE;
        let body = self.buffers.body.get(..self.fetch.body_len).unwrap_or(&[]);
        match exchange.purpose {
            Purpose::Descriptor { channel: true, via_link } if missing => {
                let Some(folder) = self.channel else { return true };
                event(sink, Event::ChannelDefaults);
                self.open_descriptor(now, Descriptor::channel_defaults(&folder), via_link, sink);
                self.front_door = channel::file_url(&folder, ChannelFile::Bundle).ok();
                true
            }
            Purpose::FrontDoor => {
                // A channel without a bundle is still a channel: its pages
                // are fetched one by one.
                if missing {
                } else if status != STATUS_OK {
                    event(sink, Event::BundleFailed(Failure::Status(status)));
                } else {
                    match channel_file_media(true, head.media, body) {
                        Ok(media) => sink(Output::PageBytes(PageBytes {
                            source: Source::FrontDoor,
                            media,
                            url: exchange.url.as_str(),
                            secure: exchange.url.is_secure(),
                            bytes: body,
                        })),
                        Err(failure) => event(sink, Event::BundleFailed(failure)),
                    }
                }
                true
            }
            Purpose::Contents => {
                if missing {
                    event(sink, Event::ContentsMissing);
                } else if status != STATUS_OK {
                    event(sink, Event::ContentsFailed(Failure::Status(status)));
                } else if !matches!(head.media, MediaType::TextPlain | MediaType::OctetStream) {
                    event(sink, Event::ContentsFailed(Failure::MediaType));
                } else {
                    event(sink, Event::Contents(body));
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn open_descriptor(
        &mut self,
        now: Instant,
        descriptor: Descriptor,
        via_link: bool,
        sink: &mut Sink<'_>,
    ) {
        self.cache = [None; CACHE_ENTRIES];
        self.navigation_pending = via_link;
        if self.push.phase != PushPhase::Idle {
            sink(Output::Close { connection: ConnectionId::Push });
        }
        self.push.phase = PushPhase::Idle;
        self.push.delay_us = PUSH_RETRY_FIRST_US;
        self.push.sse = SseParser::new();
        self.push.url = descriptor.push().map(|(url, _)| *url);
        self.push.method = descriptor.push().map_or(PushMethod::WebSocket, |(_, method)| *method);
        let descriptor = self.descriptor.insert(descriptor);
        event(
            sink,
            Event::ServiceReady {
                name: descriptor.name.as_str(),
                initial_page: descriptor.initial_page,
                region: descriptor.region,
                secure: descriptor.address().is_secure(),
            },
        );
        if self.push.url.is_some() {
            if cfg!(feature = "push") {
                self.push_connect(now, sink);
            } else {
                event(sink, Event::PushNotCompiledIn(self.push.method));
            }
        }
    }
}
