//! The REDIRECT record (`neoteletext-common/INPUT.md` section 7.11,
//! DEC-1689, REQ-1688, DEC-1730): a channel's program moving a viewer to
//! another page of the channel, or to another channel. It alone moves a
//! viewer: a cursor record never does.
//!
//! Its body is one address, as an HTTP `Location` is: a place of this
//! channel, `302` or `302/0002`, or a whole `http` or `https` address, 1 to
//! `MAX_ADDRESS` bytes of printable ASCII with no fragment. The receiver's
//! own address rules decide whether a whole address is followed.

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
/// What begins a fragment, which a record's address never has (DEC-1694).
const FRAGMENT: u8 = b'#';
const _: () = assert!(MAX_ADDRESS <= MAX_FRAMED_BODY);

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
/// A body that is neither a place of this channel with no parameters, on a
/// page that is not reserved, nor an address the record may carry.
pub fn read(body: &[u8]) -> Result<RedirectRecord<'_>, RecordFault> {
    if body.is_empty() || body.len() > MAX_ADDRESS {
        return Err(RecordFault::Length);
    }
    let text = core::str::from_utf8(body).map_err(|_| RecordFault::BadValue("redirect address"))?;
    if let Some(Place::Page { page, subcode, parameters: None }) = Place::read(text) {
        let [magazine, number] = page.mpp().to_be_bytes();
        if is_reserved_page(magazine, number) {
            return Err(RecordFault::BadValue("redirect page"));
        }
        return Ok(RedirectRecord::Page { page, subcode: subcode.unwrap_or(SubCode::ANY) });
    }
    if check_address(text) {
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
    let mut buffer = [0_u8; MAX_ADDRESS];
    let body = match redirect {
        RedirectRecord::Page { page, subcode } => {
            let mut writer = Writer::new(&mut buffer);
            let named = (!subcode.is_any()).then_some(subcode);
            let place = Place::Page { page, subcode: named, parameters: None };
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
