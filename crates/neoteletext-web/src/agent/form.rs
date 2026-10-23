//! Form submission (REQ-WEB-010, REQ-WEB-011): the destination and the
//! values, the viewer's confirmation (REQ-SEC-008, REQ-SEC-009), and the
//! response.

use super::{Event, Exchange, FormPhase, FormState, Output, PageBytes, Sink, Source, UserAgent, event};
use crate::bytes::{Overflow, Writer};
use crate::channel;
use crate::consts::{MAX_URL_LENGTH, STATUS_CLIENT_ERROR_FIRST, STATUS_REDIRECT_FIRST, STATUS_SUCCESS_FIRST};
use crate::cookie::Site;
use crate::descriptor::Descriptor;
use crate::form::{self, FormMethod, FormRefused, FormSubmission};
use crate::http::{MediaType, Method, ResponseHead};
use crate::port::RandomSource;
use crate::url::Url;
use neoteletext_core::policy;

impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    pub(super) fn submit(&mut self, submission: &FormSubmission<'_>, sink: &mut Sink<'_>) {
        if self.form.phase != FormPhase::Idle {
            event(sink, Event::SubmissionRefused(FormRefused::Busy));
            return;
        }
        let page = match Url::parse(submission.page_url) {
            Ok(page) => page,
            Err(error) => {
                event(sink, Event::SubmissionRefused(FormRefused::Address(error)));
                return;
            }
        };
        let permitted = self.descriptor.as_ref().map(Descriptor::form_origins).into_iter().flatten();
        let target = match form::destination(&page, submission.target, permitted) {
            Ok(target) => target,
            Err(refused) => {
                event(sink, Event::SubmissionRefused(refused));
                return;
            }
        };
        // A channel's form submits inside its folder, or to an origin its
        // settings list (REQ-WEB-026).
        if let Some(folder) = &self.channel {
            let listed = self.descriptor.as_ref().is_some_and(|d| d.form_origins().any(|o| o.same(&target.origin())));
            if !channel::is_inside(folder, &target) && !listed {
                event(sink, Event::SubmissionRefused(FormRefused::Origin));
                return;
            }
        }
        let mut writer = Writer::new(self.buffers.form);
        if form::encode(submission.fields, submission.token, submission.action, &mut writer).is_err() {
            event(sink, Event::SubmissionRefused(FormRefused::TooLarge));
            return;
        }
        let encoded_len = writer.len();
        let (url, method, body_len) = match submission.method {
            FormMethod::Post => (target, Method::Post, encoded_len),
            FormMethod::Get => {
                // The values replace the target's query.
                let mut buffer = [0u8; MAX_URL_LENGTH];
                let mut address = Writer::new(&mut buffer);
                let built = (|| -> Result<(), Overflow> {
                    address.put(target.scheme().name())?;
                    address.put(b"://")?;
                    address.put(target.authority())?;
                    address.put(target.path().as_bytes())?;
                    address.byte(b'?')?;
                    address.put(self.buffers.form.get(..encoded_len).unwrap_or(&[]))
                })();
                let parsed = built
                    .map_err(|_| FormRefused::TooLarge)
                    .and_then(|()| core::str::from_utf8(address.written()).map_err(|_| FormRefused::TooLarge))
                    .and_then(|text| Url::parse(text).map_err(FormRefused::Address));
                match parsed {
                    Ok(url) => (url, Method::Get, 0),
                    Err(refused) => {
                        event(sink, Event::SubmissionRefused(refused));
                        return;
                    }
                }
            }
        };
        let target_origin = url.origin();
        let site = if target_origin.same(&page.origin()) { Site::SameService } else { Site::CrossOriginForm };
        self.form = FormState { phase: FormPhase::Queued, target: Some(url), method, body_len, site };
        let plain_http = policy::needs_plain_http_warning(url.is_secure());
        let first_to_origin = self.config.confirm_first_submission
            && !self.confirmed.iter().flatten().any(|known| known.same(&target_origin));
        if plain_http || first_to_origin {
            self.form.phase = FormPhase::Confirming { record_origin: first_to_origin };
            event(
                sink,
                Event::ConfirmSubmission {
                    host: url.host_text(),
                    port: url.port(),
                    secure: url.is_secure(),
                    plain_http,
                    first_to_origin,
                },
            );
        }
    }

    pub(super) fn confirm(&mut self, confirmed: bool, sink: &mut Sink<'_>) {
        let FormPhase::Confirming { record_origin } = self.form.phase else { return };
        if !confirmed {
            self.form = FormState {
                phase: FormPhase::Idle,
                target: None,
                method: Method::Post,
                body_len: 0,
                site: Site::SameService,
            };
            event(sink, Event::SubmissionCancelled);
            return;
        }
        if record_origin && let Some(target) = &self.form.target {
            let origin = target.origin();
            if let Some(slot) = self.confirmed.iter_mut().find(|slot| slot.is_none()) {
                *slot = Some(origin);
            } else {
                // Full: forget the oldest, so the newest is remembered.
                self.confirmed.rotate_left(1);
                if let Some(last) = self.confirmed.last_mut() {
                    *last = Some(origin);
                }
            }
        }
        self.form.phase = FormPhase::Queued;
    }

    /// A form's response is complete (REQ-WEB-011): its TTI page, or the
    /// success or failure the `FP` pages show.
    pub(super) fn complete_form(&mut self, exchange: &Exchange, head: &ResponseHead, sink: &mut Sink<'_>) {
        let status = head.status;
        let body = self.buffers.body.get(..self.fetch.body_len).unwrap_or(&[]);
        let secure = exchange.url.is_secure();
        self.form.phase = FormPhase::Idle;
        let success = (STATUS_SUCCESS_FIRST..STATUS_REDIRECT_FIRST).contains(&status);
        if success && head.media == MediaType::Tti {
            sink(Output::PageBytes(PageBytes {
                source: Source::Form,
                media: head.media,
                url: exchange.url.as_str(),
                secure,
                bytes: body,
            }));
            event(sink, Event::SubmissionPage);
        } else if success {
            event(sink, Event::SubmissionSucceeded { status });
        } else if status >= STATUS_CLIENT_ERROR_FIRST && head.media == MediaType::Tti {
            // REQ-WEB-011 item 3: a TTI page is displayed whatever
            // the status; the error is still reported.
            sink(Output::PageBytes(PageBytes {
                source: Source::Form,
                media: head.media,
                url: exchange.url.as_str(),
                secure,
                bytes: body,
            }));
            event(sink, Event::SubmissionFailed { status: Some(status), failure: None });
        } else {
            event(sink, Event::SubmissionFailed { status: Some(status), failure: None });
        }
    }
}
