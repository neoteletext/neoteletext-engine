//! What feeds a viewer: a file read whole at start, or a live source that
//! keeps delivering pages and takes the viewer's requests.

use crate::extras::PageExtras;
use crate::pipe::{Notice, Submission};
use crate::session::Session;
use neoteletext_core::types::{Instant, PageAddress, SubCode};

/// Something that feeds packets into the session while the viewer runs.
pub trait Source {
    /// Feeds whatever is available now. Returns false at the end of input.
    fn poll(&mut self, session: &mut Session, now: Instant) -> bool;

    /// Asks for a page, for sources that fetch on demand.
    fn request(&mut self, _page: PageAddress, _subcode: SubCode) {}

    /// Asks for a page the shown page depends on, without making it the
    /// page kept fresh.
    fn prefetch(&mut self, _page: PageAddress) {}

    /// Notices for the viewer since the last call.
    fn notices(&mut self) -> Vec<Notice> {
        Vec::new()
    }

    /// Extension data of a page: links, the drawn link row, a form.
    fn extras(&self, _page: PageAddress, _subcode: SubCode) -> Option<&PageExtras> {
        None
    }

    /// Submits a form.
    fn submit(&mut self, _submission: Submission) {}

    /// Answers a confirmation request.
    fn confirm(&mut self, _yes: bool) {}
    /// Sends a key record to the channel being typed into (area INT).
    fn send_key(&mut self, _record: &[u8]) {}

    /// Follows a link to another service.
    fn follow(&mut self, _service: u8, _page: PageAddress, _subcode: SubCode) {}

    /// The short name of a linked service.
    fn service_name(&self, _service: u8) -> Option<String> {
        None
    }
}

/// A source with nothing more to give: the input was read at start.
pub struct Finished;

impl Source for Finished {
    fn poll(&mut self, _session: &mut Session, _now: Instant) -> bool {
        false
    }
}
