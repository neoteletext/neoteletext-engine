//! The channel URL map (neoteletext-common's REQ-1693, decided by DEC-1694;
//! written out in `SERVER.md` section 4): the one place that reads and
//! writes what an address says inside a channel.
//!
//! A channel's identity is its address up to `#`, and everything after `#`
//! is inside the channel and is never requested: `#302` a page, `#302/0002`
//! a sub-page, either with `?name=value` page parameters, `#contents` the
//! contents and `#wall` the wall. Each thing has one spelling: other
//! spellings are normalised to it (item 3) or refused, never repaired
//! (item 4).

use neoteletext_core::types::{PageAddress, SubCode};

use crate::bytes::{Overflow, Writer};
use crate::channel::{ChannelError, ChannelFile, PAGE_DIGITS, page_of, parse_address};
use crate::subpages::{SUBCODE_DIGITS, parse_subcode, write_subcode};
use crate::url::{FRAGMENT_MARK, QUERY_MARK, Url, check_fragment};

/// The fragment that opens the channel's contents (REQ-1693 item 1).
pub const CONTENTS: &str = "contents";
/// The fragment that opens the wall of pages (REQ-1693 item 1).
pub const WALL: &str = "wall";
/// Between a page and its sub-page in a fragment (REQ-1693 item 1).
const SUBPAGE_MARK: u8 = b'/';
/// Between the pairs of page parameters (REQ-1693 item 5).
const PAIR_MARK: u8 = b'&';
/// Between a parameter's name and its value (REQ-1693 item 5).
const VALUE_MARK: u8 = b'=';

/// What a fragment opens inside a channel (REQ-1693 item 1).
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
    /// Reads a fragment, without its `#`. Digits may be in either case, and
    /// [`Place::write`] gives them as capitals. `None` for anything else the
    /// map does not list (REQ-1693 item 4).
    #[must_use]
    pub fn read(fragment: &'a str) -> Option<Self> {
        match fragment {
            CONTENTS => return Some(Self::Contents),
            WALL => return Some(Self::Wall),
            _ => {}
        }
        check_fragment(fragment.as_bytes()).ok()?;
        let (selection, parameters) = match before(fragment, QUERY_MARK) {
            Some((selection, parameters)) => (selection, Some(page_parameters(parameters)?)),
            None => (fragment, None),
        };
        let (page, subcode) = match before(selection, SUBPAGE_MARK) {
            Some((page, subcode)) => (page, Some(subcode_of(subcode)?)),
            None => (selection, None),
        };
        Some(Self::Page { page: page_of(&capitals::<PAGE_DIGITS>(page)?)?, subcode, parameters })
    }

    /// Writes the place's canonical fragment, without its `#`.
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

/// A sub-page in a fragment: 3F7F means no particular sub-page, so it
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
    /// Its fragment is not one the map lists (REQ-1693 item 4).
    Place,
    /// A page's file name and a fragment both say where to go.
    TwoPlaces,
    /// It names a file only receivers use (REQ-1693 item 2).
    ReceiverFile,
}

/// An address of the map, read and normalised (REQ-1693 items 1 and 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapAddress<'a> {
    /// The channel's folder.
    pub folder: Url,
    /// The channel parameters as written, without their `?`.
    pub query: Option<&'a str>,
    /// Where inside the channel it goes; `None` arrives at the initial page.
    pub place: Option<Place<'a>>,
}

impl<'a> MapAddress<'a> {
    /// Reads an `http` or `https` address. `<channel>302.tti` and
    /// `<channel>302.t42` read as `<channel>#302`, and
    /// `<channel>302-0002.t42` as `<channel>#302/0002`. A file only receivers
    /// use is refused: a viewer never sees one (REQ-1693 item 2).
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
        let (identity, fragment) = before(text, FRAGMENT_MARK).map_or((text, None), |(head, tail)| (head, Some(tail)));
        let (location, query) =
            before(identity, QUERY_MARK).map_or((identity, None), |(head, tail)| (head, Some(tail)));
        let found = parse_address(location).map_err(MapError::Channel)?;
        // The whole address obeys the strict parser and the general limit.
        Url::parse(text).map_err(|error| MapError::Channel(ChannelError::Address(error)))?;
        let chosen = fragment.map(|fragment| Place::read(fragment).ok_or(MapError::Place)).transpose()?;
        let place = match (found.file, chosen) {
            (None, chosen) => chosen,
            (Some(ChannelFile::Page { page, .. }), None) => Some(Place::Page { page, subcode: None, parameters: None }),
            (Some(ChannelFile::SubPage { page, subcode }), None) => {
                Some(Place::Page { page, subcode: Some(subcode), parameters: None })
            }
            (Some(ChannelFile::Page { .. } | ChannelFile::SubPage { .. }), Some(_)) => {
                return Err(MapError::TwoPlaces);
            }
            (Some(_), None) if opens_receiver_files => None,
            (Some(_), _) => return Err(MapError::ReceiverFile),
        };
        Ok(Self { folder: found.folder, query, place })
    }

    /// Writes the channel's identity, the address up to `#`.
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
        if let Some(place) = &self.place {
            writer.byte(FRAGMENT_MARK)?;
            place.write(writer)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
