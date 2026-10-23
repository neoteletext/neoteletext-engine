//! The page a turn shows: found and its state told to the viewer, its
//! form made active, and its grid built with row 24 and the form.

use super::ticker::origin_ticker;
use super::{Shown, ViewState};
use neoteletext_app::extras::{self, PageExtras};
use neoteletext_app::interact::{ActiveForm, origin_of};
use neoteletext_app::session::Session;
use neoteletext_core::acq::StoredPage;
use neoteletext_core::forms::FormModel;
use neoteletext_core::fsm::PageState;
use neoteletext_core::level1::Grid;
use neoteletext_core::types::{PageAddress, SubCode};
use neoteletext_core::viewer::{ComposeContext, Composed, ViewerEvent, ViewerState};

impl ViewState {
    /// The page shown, once the viewer has been told it is present and
    /// whether it is stale.
    pub(super) fn page_shown<'s>(
        &mut self,
        session: &'s Session,
        requested: PageAddress,
        subcode: SubCode,
    ) -> Option<&'s StoredPage> {
        if self.viewer.state() == ViewerState::Searching && session.page(requested, subcode).is_some() {
            self.viewer.event(ViewerEvent::PagePresent);
        }
        let page = match self.viewer.state() {
            ViewerState::Searching | ViewerState::Starting => None,
            _ => session.page(requested, subcode),
        };
        // REQ-FSM-005: the status area says when the page shown has not
        // been received, or confirmed current, recently.
        match page.map(|page| page.state) {
            Some(PageState::Stale) => {
                let _ = self.viewer.event(ViewerEvent::PageStale);
            }
            Some(_) => {
                let _ = self.viewer.event(ViewerEvent::PageUpdated);
            }
            None => {}
        }
        page
    }

    /// A form on the page shown becomes active, keeping values when
    /// returning; leaving for another page drops it.
    pub(super) fn activate_form(&mut self, session: &Session, shown: &Shown<'_>) {
        match (shown.extras, shown.page) {
            (Some(PageExtras { form: Some(spec), url, .. }), Some(page)) => {
                let same = self
                    .form
                    .as_ref()
                    .is_some_and(|form| form.page == page.key.address && form.subcode == page.key.subcode);
                if !same {
                    let mut probe = Grid::EMPTY;
                    let magazine = session.decoder().magazine_data(page.key.address.magazine);
                    let store = session.decoder().store();
                    let rolling = session.rolling_header();
                    let context = ComposeContext {
                        magazine: &magazine,
                        config: shown.config,
                        rolling_header: &rolling,
                        store,
                        row24: None,
                        lookup: session.decoder().enhancement_lookup(),
                    };
                    let _ = self.viewer.compose(&mut probe, Some(page), &context);
                    self.form = FormModel::new(*spec, probe.designation.g0).ok().map(|model| ActiveForm {
                        page: page.key.address,
                        subcode: page.key.subcode,
                        model,
                        url: url.clone(),
                        focused: true,
                    });
                }
            }
            (_, Some(page)) if self.form.as_ref().is_some_and(|form| form.page != page.key.address) => {
                self.form = None;
            }
            _ => {}
        }
    }

    /// Builds the grid of the page shown, with row 24's prompt or links,
    /// and the active form over it with its origin ticker.
    pub(super) fn compose(&mut self, session: &Session, requested: PageAddress, shown: &Shown<'_>) {
        let rolling = session.rolling_header();
        let magazine = session.decoder().magazine_data(requested.magazine);
        let store = session.decoder().store();
        let row24 = self.prompt.or_else(|| shown.extras.and_then(|extras| extras.row24.as_ref()).map(|(row, _)| *row));
        let lookup = session.decoder().enhancement_lookup();
        let context = ComposeContext {
            magazine: &magazine,
            config: shown.config,
            rolling_header: &rolling,
            store,
            row24,
            lookup,
        };
        if let Composed::Built(report) = self.viewer.compose(&mut self.grid, shown.page, &context) {
            self.unrecorded = report;
        }
        if self.prompt.is_none()
            && let Some((_, marks)) = shown.extras.and_then(|extras| extras.row24.as_ref())
        {
            extras::draw_marks(&mut self.grid, marks);
        }
        if let Some(active) = &self.form {
            active.model.draw(&mut self.grid);
            if active.focused && self.pending.is_none() {
                origin_ticker(&mut self.grid, &origin_of(&active.url), shown.now);
            }
        }
    }
}
