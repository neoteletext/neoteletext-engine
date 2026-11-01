//! The channel URL map (neoteletext-common's REQ-1693, decided by DEC-1694
//! version 3; written out in `SERVER.md` section 4): the one place that reads
//! and writes what an address says inside a channel.
//!
//! An address is a channel's folder, ending in `/`, followed by its place:
//! `302` a page, `302/0002` a sub-page, either with a `?name=value` query,
//! `contents` the contents and `wall` the wall. The same text is what SELECT
//! carries (DEC-1730), so an address and a selection cannot disagree. Each
//! thing has one spelling: other spellings are normalised to it or refused,
//! never repaired, and a fragment on an `http` or `https` address is refused.

use neoteletext_core::types::{PageAddress, SubCode};

use crate::bytes::{Overflow, Writer};
use crate::channel::{ChannelError, ChannelFile, PAGE_DIGITS, page_of, parse_address};
use crate::subpages::{SUBCODE_DIGITS, parse_subcode, write_subcode};
use crate::url::{FRAGMENT_MARK, QUERY_MARK, Url, check_fragment};

/// The place that opens the channel's contents (REQ-1693 item 1).
pub const CONTENTS: &str = "contents";
/// The place that opens the wall of pages (REQ-1693 item 1).
pub const WALL: &str = "wall";
/// Between a page and its sub-page in a place (REQ-1693 item 1).
const SUBPAGE_MARK: u8 = b'/';
/// The length of `://` in an address.
const SCHEME_MARK_LENGTH: usize = 3;
/// Between the pairs of page parameters (REQ-1693 item 5).
const PAIR_MARK: u8 = b'&';
/// Between a parameter's name and its value (REQ-1693 item 5).
const VALUE_MARK: u8 = b'=';

/// The map's conformance table: every address, what it opens or why it is
/// refused, and what a server answers (REQ-1693's Verification). Every
/// program that follows the map runs this one file, which says how to read it.
pub const CONFORMANCE: &str = include_str!("map/conformance.tsv");

/// What an address opens inside a channel (REQ-1693 item 1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place<'a> {
    /// A page, or one sub-page of it.
    Page {
        /// The page.
        page: PageAddress,
        /// The sub-page, when the address names one.
        subcode: Option<SubCode>,
        /// The page parameters as written, without their `?`: they are not
        /// normalised (REQ-1693 item 3).
        parameters: Option<&'a str>,
    },
    /// The channel's contents.
    Contents,
    /// The wall of pages, with no page selected.
    Wall,
}

impl<'a> Place<'a> {
    /// Reads a place: the end of an address, or what SELECT carries, or a
    /// `file:` address's fragment without its `#`. Digits may be in either
    /// case, and [`Place::write`] gives them as capitals. `None` for
    /// anything else the map does not list (REQ-1693 item 4).
    #[must_use]
    pub fn read(place: &'a str) -> Option<Self> {
        match place {
            CONTENTS => return Some(Self::Contents),
            WALL => return Some(Self::Wall),
            _ => {}
        }
        check_fragment(place.as_bytes()).ok()?;
        let (selection, parameters) = match before(place, QUERY_MARK) {
            Some((selection, parameters)) => (selection, Some(page_parameters(parameters)?)),
            None => (place, None),
        };
        let (page, subcode) = match before(selection, SUBPAGE_MARK) {
            Some((page, subcode)) => (page, Some(subcode_of(subcode)?)),
            None => (selection, None),
        };
        Some(Self::Page { page: page_of(&capitals::<PAGE_DIGITS>(page)?)?, subcode, parameters })
    }

    /// Whether this is one sub-page of a page.
    const fn names_sub_page(&self) -> bool {
        matches!(self, Self::Page { subcode: Some(_), .. })
    }

    /// This place with the query of its address. A query goes only with a
    /// page or a sub-page (REQ-1693 item 4).
    fn with_query(self, query: Option<&'a str>) -> Result<Self, MapError> {
        match (self, query) {
            (place, None) => Ok(place),
            (Self::Page { page, subcode, .. }, parameters) => Ok(Self::Page { page, subcode, parameters }),
            _ => Err(MapError::Place),
        }
    }

    /// Writes the place's canonical text.
    pub fn write(&self, writer: &mut Writer<'_>) -> Result<(), Overflow> {
        match *self {
            Self::Page { page, subcode, parameters } => {
                writer.put(&page.ascii())?;
                if let Some(subcode) = subcode {
                    writer.byte(SUBPAGE_MARK)?;
                    write_subcode(writer, subcode)?;
                }
                if let Some(parameters) = parameters {
                    writer.byte(QUERY_MARK)?;
                    writer.put(parameters.as_bytes())?;
                }
                Ok(())
            }
            Self::Contents => writer.put(CONTENTS.as_bytes()),
            Self::Wall => writer.put(WALL.as_bytes()),
        }
    }
}

/// The folder and the place an address names by its end (REQ-1693 item 1,
/// DEC-1694 rule 1), and `None` when it names none. A final slash after a
/// place is ignored, so a folder named as a place belongs to the channel
/// above it (rule 2).
fn named_place(location: &str) -> Option<(&str, Place<'_>)> {
    let after_scheme = location.find("://")?.checked_add(SCHEME_MARK_LENGTH)?;
    let path = location.get(after_scheme..)?.find('/')?.checked_add(after_scheme)?;
    let trimmed = location.strip_suffix('/').unwrap_or(location);
    let last = trimmed.rfind('/').filter(|&slash| slash >= path)?;
    let last_start = last.checked_add(1)?;
    // A sub-page is two segments, so it is tried before its last alone.
    if let Some(previous) = trimmed.get(..last)?.rfind('/').filter(|&slash| slash >= path) {
        let start = previous.checked_add(1)?;
        let place = Place::read(trimmed.get(start..)?).filter(Place::names_sub_page);
        if let Some(place) = place {
            return Some((location.get(..start)?, place));
        }
    }
    Some((location.get(..last_start)?, Place::read(trimmed.get(last_start..)?)?))
}

/// `text` split around the first `mark`, which is left out of both. Scanned
/// byte by byte, as the address parser does (REQ-GEN-019).
fn before(text: &str, mark: u8) -> Option<(&str, &str)> {
    let at = text.as_bytes().iter().position(|&byte| byte == mark)?;
    let (head, tail) = text.split_at_checked(at)?;
    Some((head, tail.get(1..)?))
}

/// `digits` as capitals, when there are exactly `N` of them.
fn capitals<const N: usize>(digits: &str) -> Option<[u8; N]> {
    let digits: [u8; N] = digits.as_bytes().try_into().ok()?;
    Some(digits.map(|digit| digit.to_ascii_uppercase()))
}

/// A sub-page in a place: 3F7F means no particular sub-page, so it
/// cannot name one (REQ-1693 item 4).
fn subcode_of(digits: &str) -> Option<SubCode> {
    parse_subcode(&capitals::<SUBCODE_DIGITS>(digits)?).filter(|subcode| !subcode.is_any())
}

/// `name=value` pairs joined by `&` (REQ-1693 item 5). A value may be empty,
/// and a name may not.
fn page_parameters(parameters: &str) -> Option<&str> {
    parameters
        .as_bytes()
        .split(|&byte| byte == PAIR_MARK)
        .all(|pair| pair.iter().position(|&byte| byte == VALUE_MARK).is_some_and(|at| at > 0))
        .then_some(parameters)
}

/// Why an address is outside the map.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapError {
    /// It does not name a channel.
    Channel(ChannelError),
    /// It has a fragment, or a place or query the map does not list
    /// (REQ-1693 item 4).
    Place,
    /// It names a file only receivers use (REQ-1693 item 2).
    ReceiverFile,
}

/// An address of the map, read and normalised (REQ-1693 items 1 and 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapAddress<'a> {
    /// The channel's folder.
    pub folder: Url,
    /// The query of an address that names no place, as written, without its
    /// `?`. It goes with the selection the arrival makes (DEC-1730).
    pub query: Option<&'a str>,
    /// Where inside the channel it goes, with its query; `None` arrives at
    /// the initial page.
    pub place: Option<Place<'a>>,
}

impl<'a> MapAddress<'a> {
    /// Reads an `http` or `https` address. `<channel>302.tti` and
    /// `<channel>302.t42` read as `<channel>302`, `<channel>302-0002.t42` as
    /// `<channel>302/0002`, and a final slash after a place is removed. A
    /// file only receivers use is refused: a viewer never sees one
    /// (REQ-1693 item 2).
    pub fn read(text: &'a str) -> Result<Self, MapError> {
        Self::read_with(text, false)
    }

    /// Reads an address a program was told to open, which differs from
    /// [`MapAddress::read`] in one way: a receiver's file with nothing after
    /// it opens its channel (DEC-1710), as `neottx https://h.example/a/channel.json`
    /// does. A fragment the map refuses is refused here as well, so no
    /// address opens with one ignored (REQ-1693 item 4).
    pub fn read_opened(text: &'a str) -> Result<Self, MapError> {
        Self::read_with(text, true)
    }

    fn read_with(text: &'a str, opens_receiver_files: bool) -> Result<Self, MapError> {
        if text.as_bytes().contains(&FRAGMENT_MARK) {
            return Err(MapError::Place);
        }
        let (location, query) = before(text, QUERY_MARK).map_or((text, None), |(head, tail)| (head, Some(tail)));
        if query.is_some_and(|query| page_parameters(query).is_none()) {
            return Err(MapError::Place);
        }
        // The whole address obeys the strict parser and the general limit.
        Url::parse(text).map_err(|error| MapError::Channel(ChannelError::Address(error)))?;
        if let Some((folder, place)) = named_place(location) {
            let found = parse_address(folder).map_err(MapError::Channel)?;
            return Ok(Self { folder: found.folder, query: None, place: Some(place.with_query(query)?) });
        }
        let found = parse_address(location).map_err(MapError::Channel)?;
        let place = match found.file {
            None => None,
            Some(ChannelFile::Page { page, .. }) => Some(Place::Page { page, subcode: None, parameters: query }),
            Some(ChannelFile::SubPage { page, subcode }) => {
                Some(Place::Page { page, subcode: Some(subcode), parameters: query })
            }
            Some(_) if opens_receiver_files => None,
            Some(_) => return Err(MapError::ReceiverFile),
        };
        let query = if place.is_some() { None } else { query };
        Ok(Self { folder: found.folder, query, place })
    }

    /// Writes the channel's own address, and the query of an address that
    /// names no place.
    pub fn write_channel(&self, writer: &mut Writer<'_>) -> Result<(), Overflow> {
        writer.put(self.folder.as_str().as_bytes())?;
        if let Some(query) = self.query {
            writer.byte(QUERY_MARK)?;
            writer.put(query.as_bytes())?;
        }
        Ok(())
    }

    /// Writes the canonical address (REQ-1693 item 3).
    pub fn write(&self, writer: &mut Writer<'_>) -> Result<(), Overflow> {
        self.write_channel(writer)?;
        self.place.as_ref().map_or(Ok(()), |place| place.write(writer))
    }
}

#[cfg(test)]
mod tests;
