//! Objects and the pages that carry them (area OBJ, layer L3; clauses 10.5,
//! 10.6 and 13; decisions E.51 to E.53).
//!
//! This module finds objects for the enhancement processor:
//!
//! - [`links()`]: the enhancement pages a page uses. Its own X/27/4 and, at
//!   Level 3.5, X/27/5 take precedence role by role; the Magazine
//!   Organization Table of its magazine gives the rest, the fallback bits
//!   and the default objects (REQ-OBJ-030, REQ-OBJ-031, REQ-OBJ-032,
//!   REQ-OBJ-040);
//! - [`find_local`] and [`find_public`]: an object's definition in the
//!   page's own packets X/26 (REQ-OBJ-003), or through the pointer tables of
//!   a Public or Global Public Object Page (REQ-OBJ-004), checked for type
//!   and usage (REQ-OBJ-005);
//! - [`prescan`]: an object's body, where it ends and the last position it
//!   addresses, before it runs (REQ-OBJ-006, REQ-OBJ-020);
//! - [`Body::fetch`]: the triplets of local enhancement data and of object
//!   bodies, one at a time.
//!
//! It reads stored pages only through the read-only lookup of the
//! acquisition layer (REQ-ACQ-020), and decodes their bytes only with the
//! packet layer's decoders and [`TripletFunction`](crate::packet::TripletFunction) (REQ-PKT-010,
//! REQ-PKT-017, REQ-PKT-018). Nothing here allocates; every loop is bounded
//! by the 507 triplets of an object page or the 208 of a page's X/26.

use crate::config::PresentationLevel;
use crate::consts::DEFAULT_OBJECTS;
use crate::packet::{ObjectKind, ObjectNumber, ObjectSource};
use crate::types::{FallbackFields, Magazine, PageNumber};

mod body;
mod definition;
mod links;

pub use body::{Body, Fetch};
pub use definition::{Absent, Extent, ObjectRef, find_local, find_public, prescan, row_of};
pub use links::links;

/// A link to an object page: its magazine and page number. The
/// sub-page is named by each invocation (decision E.52).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct PageLink {
    /// The magazine of the linked page.
    pub magazine: Magazine,
    /// The page number.
    pub page: PageNumber,
}

/// A default object of the Magazine Organization Table (clause 13.8).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct DefaultInvocation {
    /// GPOP or POP.
    pub source: ObjectSource,
    /// The object type.
    pub kind: ObjectKind,
    /// The object number, with N8 = 0 (REQ-OBJ-040).
    pub number: ObjectNumber,
    /// The object page: the table's own GPOP or POP link, within whose
    /// sub-pages clause 13.8 says the object exists.
    pub page: PageLink,
}

/// The enhancement pages a displayed page uses (REQ-OBJ-030, REQ-OBJ-031).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct EnhancementLinks {
    /// The Global Public Object Page.
    pub gpop: Option<PageLink>,
    /// The Public Object Page.
    pub pop: Option<PageLink>,
    /// The fallback bits of the page's POP link in the table, for Black
    /// Background Colour Substitution; `None` without a POP link (decision
    /// E.53).
    pub fallback: Option<FallbackFields>,
    /// The default objects in the order of Annex G.5: GPOP 1, GPOP 2, POP 1,
    /// POP 2 (decision E.53).
    pub default_objects: [Option<DefaultInvocation>; DEFAULT_OBJECTS],
    /// The level the page is processed at. A Level 3.5 decoder processes a
    /// page at Level 2.5 when the table lacks X/22 or X/24 (clause 10.6.1).
    pub level: PresentationLevel,
}

impl EnhancementLinks {
    /// No enhancement pages, at a level.
    #[must_use]
    pub const fn none(level: PresentationLevel) -> Self {
        Self { gpop: None, pop: None, fallback: None, default_objects: [None; DEFAULT_OBJECTS], level }
    }

    /// The object page of a public or global source.
    #[must_use]
    pub const fn object_page(&self, source: ObjectSource) -> Option<PageLink> {
        match source {
            ObjectSource::Public => self.pop,
            ObjectSource::Global => self.gpop,
            ObjectSource::Local | ObjectSource::Illegal => None,
        }
    }
}

#[cfg(test)]
mod tests;
