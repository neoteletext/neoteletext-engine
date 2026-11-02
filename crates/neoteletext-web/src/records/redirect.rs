//! The REDIRECT record (DOC-1960,
//! DEC-1689, REQ-1688, DEC-1730): a channel's program moving a viewer to
//! another page of the channel, or to another channel. It alone moves a
//! viewer: a cursor record never does.
//!
//! Its body is one address, as an HTTP `Location` is: a place of this
//! channel, `302`, `302/0002` or either with a query, or a whole `http` or
//! `https` address, of printable ASCII with no fragment. The receiver's own
//! address rules decide whether a whole address is followed.

use neoteletext_audio::MAX_FRAMED_BODY;
use neoteletext_core::limits::{MAX_URL_LENGTH, is_reserved_page};
use neoteletext_core::packet::Packet;
use neoteletext_core::types::{PageAddress, SubCode};

use super::{RecordFault, TYPE_REDIRECT, write_packets};
use crate::bytes::Writer;
use crate::map::Place;
use crate::url::{Scheme, Url};

/// The longest address a record carries: the longest of all (REQ-1661).
pub const MAX_ADDRESS: usize = MAX_URL_LENGTH;
/// The longest body: a place, `302/0002`, and the longest query a selection
/// may carry, as long as any address, kept through a redirect (TODO-1731).
pub const MAX_BODY: usize = MAX_PLACE_TEXT + 1 + MAX_ADDRESS;
/// The longest place: a page and a sub-code.
const MAX_PLACE_TEXT: usize = 8;
/// What begins a fragment, which a record's address never has (DEC-1694).
const FRAGMENT: u8 = b'#';
const _: () = assert!(MAX_BODY <= MAX_FRAMED_BODY);

/// A REDIRECT record, checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RedirectRecord<'a> {
    /// Another page of this channel: `subcode` is [`SubCode::ANY`] for the
    /// page, or one of its sub-pages.
    Page {
        /// The page.
        page: PageAddress,
        /// The sub-code, or [`SubCode::ANY`].
        subcode: SubCode,
        /// The query the place keeps, without its `?`, when the program
        /// moved a selection that had one (TODO-1731).
        parameters: Option<&'a str>,
    },
    /// Another channel's address. The receiver's own address rules decide
    /// whether it is followed.
    Address(&'a str),
}

/// Whether `text` is a whole address the record may carry.
fn check_address(text: &str) -> bool {
    !text.as_bytes().contains(&FRAGMENT)
        && text.bytes().all(|byte| byte.is_ascii_graphic())
        && Url::parse(text).is_ok_and(|url| matches!(url.scheme(), Scheme::Http | Scheme::Https))
}

/// Reads a REDIRECT record's body.
///
/// # Errors
/// A body that is neither a place of this channel, on a page that is not
/// reserved, nor an address the record may carry.
pub fn read(body: &[u8]) -> Result<RedirectRecord<'_>, RecordFault> {
    if body.is_empty() || body.len() > MAX_BODY {
        return Err(RecordFault::Length);
    }
    let text = core::str::from_utf8(body).map_err(|_| RecordFault::BadValue("redirect address"))?;
    if let Some(Place::Page { page, subcode, parameters }) = Place::read(text) {
        let [magazine, number] = page.mpp().to_be_bytes();
        if is_reserved_page(magazine, number) {
            return Err(RecordFault::BadValue("redirect page"));
        }
        return Ok(RedirectRecord::Page { page, subcode: subcode.unwrap_or(SubCode::ANY), parameters });
    }
    if text.len() <= MAX_ADDRESS && check_address(text) {
        return Ok(RedirectRecord::Address(text));
    }
    Err(RecordFault::BadValue("redirect address"))
}

/// Writes a REDIRECT record as packets 4/31 of audio service `service`,
/// record id `record_id`. Returns the packets written.
///
/// # Errors
/// What the reader would refuse.
pub fn encode(
    redirect: RedirectRecord<'_>,
    service: u8,
    record_id: u8,
    out: &mut dyn FnMut(Packet),
) -> Result<usize, RecordFault> {
    let mut buffer = [0_u8; MAX_BODY];
    let body = match redirect {
        RedirectRecord::Page { page, subcode, parameters } => {
            let mut writer = Writer::new(&mut buffer);
            let named = (!subcode.is_any()).then_some(subcode);
            let place = Place::Page { page, subcode: named, parameters };
            place.write(&mut writer).map_err(|_| RecordFault::Length)?;
            let length = writer.written().len();
            buffer.get(..length).ok_or(RecordFault::Length)?
        }
        RedirectRecord::Address(address) if address.len() > MAX_ADDRESS => return Err(RecordFault::Length),
        RedirectRecord::Address(address) => address.as_bytes(),
    };
    // The writer checks with the reader, so the two cannot disagree.
    if read(body)? != redirect {
        return Err(RecordFault::BadValue("does not read back"));
    }
    write_packets(TYPE_REDIRECT, body, service, record_id, out).ok_or(RecordFault::Length)
}

#[cfg(test)]
mod tests;
