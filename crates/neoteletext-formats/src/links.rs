//! A page's links as a receiver follows them (Appendix E of the TTI Link
//! Extension, `docs/spec/requirements/tti-link-extension.md`): where each
//! coloured key, Index, page-number link and text link leads, with the
//! services the descriptor names resolved, and the row 24 the receiver
//! draws from the `LN` lines (REQ-NAV-011, decision E.108). One implementation for every
//! port: `neottx` through `neoteletext-app`, and NeoTeletext's decoder.
//!
//! Packet X/27/0, which the page encoder writes from the same lines
//! (Appendix E.2 rule 4), carries only the links to this service's pages;
//! the links to other services and to addresses are here.

use neoteletext_core::types::{PageAddress, SubCode};

use crate::encode::{LinkRow, Prompt, draw_link_row, key_colour};
use crate::page::{LinkDestination, LinkKey, PageFile, WEB_MARK};

/// The coloured keys and Index: red, green, yellow, cyan and Index, in
/// that order in [`ReceiverLinks::keys`].
pub const KEYS_WITH_INDEX: usize = 5;
/// Index's place in [`ReceiverLinks::keys`].
pub const INDEX_KEY: usize = 4;

/// Where a link leads, with its service resolved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Destination<'a> {
    /// A page of this service (`service` `None`), or of the service at that
    /// index among the descriptor's linked services.
    Page {
        /// The page.
        address: PageAddress,
        /// The sub-page, `None` for any.
        subcode: Option<SubCode>,
        /// The linked service's index, `None` for this service.
        service: Option<u8>,
    },
    /// Another site's page file, by its address as the page wrote it.
    Address(&'a [u8]),
    /// A web page, to open outside teletext (decision E.88), its `web:`
    /// mark kept.
    Web(&'a [u8]),
}

impl Destination<'_> {
    /// True when following it leaves this service: the receiver's "»"
    /// mark goes before its label (REQ-NAV-011).
    #[must_use]
    pub const fn leaves_service(&self) -> bool {
        !matches!(self, Self::Page { service: None, .. })
    }
}

/// Where `target` leads, or `None` when the link is null: it names a
/// service that `services`, the descriptor's linked services in order,
/// does not hold (Appendix E.2 rule 2), or it is a web page and `web` is
/// false, for a port that opens only teletext (decision E.88).
#[must_use]
pub fn resolve<'a, S: AsRef<str>>(target: &'a LinkDestination, services: &[S], web: bool) -> Option<Destination<'a>> {
    resolve_parts(target.service.as_bytes(), target.address, target.subcode, target.url.as_bytes(), services, web)
}

/// [`resolve`] of a destination's parts, as an `LN`, `LK` or `LT` line
/// holds them.
fn resolve_parts<'a, S: AsRef<str>>(
    service: &[u8],
    address: PageAddress,
    subcode: Option<SubCode>,
    url: &'a [u8],
    services: &[S],
    web: bool,
) -> Option<Destination<'a>> {
    if url.starts_with(WEB_MARK) {
        return web.then_some(Destination::Web(url));
    }
    if !url.is_empty() {
        return Some(Destination::Address(url));
    }
    let service = if service.is_empty() {
        None
    } else {
        let index = services.iter().position(|known| known.as_ref().as_bytes() == service)?;
        Some(u8::try_from(index).ok()?)
    };
    Some(Destination::Page { address, subcode, service })
}

/// The place of a key in [`ReceiverLinks::keys`].
const fn key_place(key: LinkKey) -> usize {
    match key {
        LinkKey::Red => 0,
        LinkKey::Green => 1,
        LinkKey::Yellow => 2,
        LinkKey::Cyan => 3,
        LinkKey::Index => INDEX_KEY,
    }
}

/// The links of a page's `LN` lines as a receiver follows them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReceiverLinks<'a> {
    /// Where each key leads: red, green, yellow, cyan, then Index; `None`
    /// for a key with no link or a null one.
    pub keys: [Option<Destination<'a>>; KEYS_WITH_INDEX],
    /// Row 24 as the receiver draws it (REQ-NAV-011): each coloured key's
    /// prompt in its key's colour, a null link's prompt blank, and the "»"
    /// mark before the label of a link that leaves the service, at the
    /// columns `marks` lists, written as `>` among the codes for a
    /// renderer that draws no mark of its own.
    pub row: LinkRow,
}

/// The receiver's links and row 24 of a page with valid `LN` lines, or
/// `None` when it has none or they are invalid (Appendix E.2 rule 1).
/// `services` and `web` are as [`resolve`] takes them.
#[must_use]
pub fn receiver_links<'a, S: AsRef<str>>(page: &'a PageFile, services: &[S], web: bool) -> Option<ReceiverLinks<'a>> {
    if !page.uses_link_lines() || !page.links_valid {
        return None;
    }
    let mut keys = [None; KEYS_WITH_INDEX];
    for line in page.links.as_slice() {
        if let Some(slot) = keys.get_mut(key_place(line.key)) {
            *slot =
                resolve_parts(line.service.as_bytes(), line.address, line.subcode, line.url.as_bytes(), services, web);
        }
    }
    let row = draw_link_row(page, |line| {
        let destination = keys.get(key_place(line.key)).copied().flatten()?;
        Some(Prompt { colour: key_colour(line.key), leaves: destination.leaves_service() })
    });
    Some(ReceiverLinks { keys, row })
}

#[cfg(all(test, feature = "tti"))]
mod tests;
