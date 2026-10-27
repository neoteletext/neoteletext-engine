//! The source's notices to the viewer: the service's pages, the channel's
//! input, and messages.

use super::ViewState;
use crate::clock::now_ms;
use neoteletext_app::interact::message_row;
use neoteletext_app::pipe::Notice;
use neoteletext_app::source::Source;
use neoteletext_core::types::PageAddress;
use neoteletext_core::viewer::{SecurityIndicator, Viewer, ViewerEvent};

impl ViewState {
    /// Acts on one of the source's notices.
    pub(super) fn take_notice(&mut self, notice: Notice, source: &mut dyn Source) {
        match notice {
            Notice::Unencrypted => self.viewer.set_indicator(SecurityIndicator::Unencrypted),
            Notice::InitialPage(mpp) => {
                if let Some(address) = PageAddress::from_mpp(mpp) {
                    let untouched = self.viewer.requested().0 == self.initial && !self.viewer.history().can_go_back();
                    self.initial = address;
                    if untouched && address != self.viewer.requested().0 {
                        self.viewer = Viewer::new(address);
                        self.action = self.viewer.event(ViewerEvent::Started);
                    }
                }
            }
            Notice::Service(_) => self.confirmed_services.clear(),
            Notice::Input(open) => {
                let clock_ms = now_ms(self.start);
                self.typing.connection(source, open, clock_ms);
            }
            Notice::Message(text) => self.messages.push(text),
            // REQ-WEB-009: a page that is not stored cannot be shown
            // while the network is away.
            Notice::Offline(mpp) if PageAddress::from_mpp(mpp) == Some(self.viewer.requested().0) => {
                self.prompt = Some(message_row("Page not available offline"));
            }
            Notice::Offline(_)
            | Notice::Current(_)
            | Notice::ServiceName(_)
            | Notice::Bundle(_)
            | Notice::Aliases(_)
            | Notice::Refresh(_) => {}
            Notice::RedirectToOtherChannel(address) => self.messages.push(format!("redirected to {address}")),
        }
    }
}
