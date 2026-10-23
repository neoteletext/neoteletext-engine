//! The enhancement pages a page uses (REQ-OBJ-030 to REQ-OBJ-040): its own
//! compositional links role by role, then the Magazine Organization Table of
//! its magazine, its fallback bits and default objects.

use super::{DefaultInvocation, EnhancementLinks, PageLink};
use crate::acq::{EnhancementLookup, StoredPage};
use crate::config::PresentationLevel;
use crate::consts::{
    COMPOSITIONAL_LINKS, DEFAULT_OBJECTS, LEVEL_BIT_25, LEVEL_BIT_35, LINK_FUNCTION_GPOP, LINK_FUNCTION_POP,
    LINK_NULL_PAGE, LINKS_FIXED_ROLES, LINKS_X27_5_USED, MOT_LEVEL_35_LAST_PACKET, MOT_OBJECT_LINKS_LEVEL_25,
    MOT_OBJECT_LINKS_LEVEL_35, MOT_OBJECT_LINKS_PER_PACKET,
};
use crate::packet::{CompositionalLink, ObjectNumber, ObjectSource, mot_object_link, mot_page_entry, mot_position};
use crate::types::{FallbackFields, Magazine, PageNumber};

/// Roles of the links used, in the order of their function codes (Table
/// 15): GPOP and POP. Links with the DRCS functions that follow are not
/// used (decision E.95).
const ROLES: usize = 2;

/// True when a link of X/27/4 or X/27/5 may be used at a level: Format 1
/// markers as Table 15 requires, not null, and a page validity that
/// includes the level (decision E.52).
fn usable(link: CompositionalLink, level: PresentationLevel) -> bool {
    let bit = if level.is_level35() { LEVEL_BIT_35 } else { LEVEL_BIT_25 };
    link.is_format_1() && !link.is_null() && link.validity & bit != 0
}

/// The roles that the page's own X/27/4 and X/27/5 give, indexed by link
/// function (REQ-OBJ-030, decision E.52).
fn compositional(page: &StoredPage, level: PresentationLevel) -> [Option<PageLink>; ROLES] {
    let mut roles = [None; ROLES];
    let mut accept = |link: &CompositionalLink| {
        if !usable(*link, level) {
            return;
        }
        // Table 16 note: a duplicate setting is invalid, so the first wins.
        if let Some(slot) = roles.get_mut(usize::from(link.function)) {
            slot.get_or_insert(PageLink { magazine: link.magazine, page: link.page });
        }
    };
    let [x27_4, x27_5] = page.compositional_links;
    if let Some(links) = x27_4 {
        // Links 0 to 3 have fixed roles; a link whose function bits do not
        // match its position is ignored (Table 16).
        for (position, link) in links.iter().enumerate().take(LINKS_FIXED_ROLES) {
            if let Some(link) = link
                && usize::from(link.function) == position
            {
                accept(link);
            }
        }
        if level.is_level35() {
            for link in links.iter().skip(LINKS_FIXED_ROLES).take(COMPOSITIONAL_LINKS).flatten() {
                accept(link);
            }
        }
    }
    if level.is_level35()
        && let Some(links) = x27_5
    {
        for link in links.iter().take(LINKS_X27_5_USED).flatten() {
            accept(link);
        }
    }
    roles
}

/// A table link to page FF is not used (Tables 21 and 23).
fn table_link(magazine: Magazine, page: PageNumber) -> Option<PageLink> {
    (page.0 != LINK_NULL_PAGE).then_some(PageLink { magazine, page })
}

/// What the Magazine Organization Table gives a page.
struct TableLinks {
    roles: [Option<PageLink>; ROLES],
    fallback: Option<FallbackFields>,
    defaults: [Option<DefaultInvocation>; DEFAULT_OBJECTS],
    level: PresentationLevel,
}

/// Reads the Magazine Organization Table of the page's magazine for the
/// page (REQ-OBJ-031, decision E.52).
fn table(lookup: &EnhancementLookup<'_>, magazine: Magazine, page: PageNumber, level: PresentationLevel) -> TableLinks {
    let mut out = TableLinks { roles: [None; ROLES], fallback: None, defaults: [None; DEFAULT_OBJECTS], level };
    let Some(mot) = lookup.magazine_organization_table(magazine) else { return out };
    // Clause 10.6.1: a Level 3.5 decoder uses X/22 to X/24, and with X/22 or
    // X/24 missing uses X/19 to X/21 and processes the page at Level 2.5.
    let [first_35, second_35] = MOT_OBJECT_LINKS_LEVEL_35;
    let [first_25, second_25] = MOT_OBJECT_LINKS_LEVEL_25;
    let level_35 = level.is_level35() && mot.row(first_35).is_some() && mot.row(MOT_LEVEL_35_LAST_PACKET).is_some();
    if level.is_level35() && !level_35 {
        out.level = PresentationLevel::Level25;
    }
    let (first, second) = if level_35 {
        // Decision E.52: with only X/23 missing, POP links 4 to 7 come from
        // the Level 2.5 links of X/20.
        let second = if mot.row(second_35).is_some() { second_35 } else { second_25 };
        (first_35, second)
    } else {
        (first_25, second_25)
    };
    // The byte pair of page xFF is ignored (decision E.52).
    if page.0 == LINK_NULL_PAGE {
        return out;
    }
    let position = mot_position(page);
    let Some(entry) = mot.row(position.packet).and_then(|row| mot_page_entry(row, position.index)) else {
        return out;
    };
    let object_link = |number: usize| {
        let (packet, link) = if number < MOT_OBJECT_LINKS_PER_PACKET {
            (first, number)
        } else {
            (second, number.saturating_sub(MOT_OBJECT_LINKS_PER_PACKET))
        };
        mot.row(packet).and_then(|row| mot_object_link(row, link))
    };
    // Decision E.52: the global flag is an acquisition hint; the GPOP is
    // reached through the magazine's link whatever it says.
    let gpop = object_link(0);
    let pop = (entry.pop_link != 0).then(|| object_link(usize::from(entry.pop_link))).flatten();

    let mut defaults = out.defaults.iter_mut();
    for (link, source) in [(gpop, ObjectSource::Global), (pop, ObjectSource::Public)] {
        let Some(link) = link else { continue };
        let Some(page_link) = table_link(link.magazine, link.page) else { continue };
        for object in link.default_objects {
            let invocation = object.map(|object| DefaultInvocation {
                source,
                kind: object.kind,
                number: ObjectNumber(u16::from(object.number)),
                page: page_link,
            });
            if let Some(slot) = defaults.next() {
                *slot = invocation;
            }
        }
        if source == ObjectSource::Public {
            // Decision E.53: the fallback bits come from the page's POP link.
            out.fallback = Some(link.fallback);
        }
    }
    out.roles = [
        gpop.and_then(|link| table_link(link.magazine, link.page)),
        pop.and_then(|link| table_link(link.magazine, link.page)),
    ];
    out
}

/// The enhancement pages a page uses (REQ-OBJ-030 to REQ-OBJ-032,
/// REQ-OBJ-040). Each role comes from the page's own X/27/4 or X/27/5 when
/// they give it, whatever the table's X/27/4,5 flag says, and otherwise
/// from the Magazine Organization Table of the page's magazine. The
/// fallback bits and the default objects come from the table.
#[must_use]
pub fn links(page: &StoredPage, lookup: &EnhancementLookup<'_>, level: PresentationLevel) -> EnhancementLinks {
    let address = page.key.address;
    let table = table(lookup, address.magazine, address.page, level);
    let own = compositional(page, table.level);
    let role = |function: u8| {
        let index = usize::from(function);
        own.get(index).copied().flatten().or_else(|| table.roles.get(index).copied().flatten())
    };
    EnhancementLinks {
        gpop: role(LINK_FUNCTION_GPOP),
        pop: role(LINK_FUNCTION_POP),
        fallback: table.fallback,
        default_objects: table.defaults,
        level: table.level,
    }
}
