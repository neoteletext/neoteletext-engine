//! The REDIRECT record (`neoteletext-common/INPUT.md` section 7.11,
//! DEC-1689, REQ-1688): a channel's program moving a viewer to another page
//! of the channel, or to another channel. It alone moves a viewer: a cursor
//! record never does.
//!
//! ```text
//! byte 0       form: 0 a page of this channel, 1 an address
//! page form:
//! bytes 1-2    page: 0x100 to 0x8FF, never reserved
//! bytes 3-4    sub: a sub-code, or 0x3F7F for the page
//! address form:
//! bytes 1-end  address: 1 to MAX_ADDRESS bytes of printable ASCII, an
//!              http or https address, and after it, optionally, # and
//!              what follows, which the receiver's address rules judge
//! ```

use neoteletext_audio::MAX_BODY;
use neoteletext_core::limits::{LAST_SUBPAGE_CODE, SUBCODE_ANY, SUBCODE_MASK, is_reserved_page};
use neoteletext_core::packet::Packet;
use neoteletext_core::types::{PageAddress, SubCode};

use super::{RecordFault, TYPE_REDIRECT, take_u16, write_packets};
use crate::url::{Scheme, Url};

/// The `form` byte of a page of this channel.
const FORM_PAGE: u8 = 0;
/// The `form` byte of an address.
const FORM_ADDRESS: u8 = 1;
/// The `form` byte's length.
const FORM: usize = 1;
/// The page form's body: the form, the page and the sub-code.
const PAGE_BODY: usize = FORM + 2 + 2;
/// The longest address a record carries: the frame's largest body less the
/// `form` byte. An address may be longer elsewhere; a server refuses one
/// too long for this record.
pub const MAX_ADDRESS: usize = MAX_BODY - FORM;
/// What ends an address and begins its fragment (RFC 3986 section 3.5).
const FRAGMENT: u8 = b'#';
const _: () = assert!(PAGE_BODY <= MAX_BODY);

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

const fn check_subcode(subcode: u16) -> bool {
    subcode == SUBCODE_ANY || (subcode <= LAST_SUBPAGE_CODE && subcode & !SUBCODE_MASK == 0)
}

/// Reads the page form after its `form` byte.
fn read_page(rest: &[u8]) -> Result<RedirectRecord<'_>, RecordFault> {
    let (mpp, rest) = take_u16(rest)?;
    let (subcode, rest) = take_u16(rest)?;
    if !rest.is_empty() {
        return Err(RecordFault::Length);
    }
    let [magazine, number] = mpp.to_be_bytes();
    let page = PageAddress::from_mpp(mpp)
        .filter(|_| !is_reserved_page(magazine, number))
        .ok_or(RecordFault::BadValue("redirect page"))?;
    if !check_subcode(subcode) {
        return Err(RecordFault::BadValue("redirect sub-code"));
    }
    Ok(RedirectRecord::Page { page, subcode: SubCode::new(subcode) })
}

/// Whether `bytes` is an address the record may carry.
fn check_address(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.len() > MAX_ADDRESS || !bytes.iter().all(u8::is_ascii_graphic) {
        return false;
    }
    let (address, fragment) = match bytes.iter().position(|&byte| byte == FRAGMENT) {
        Some(at) => bytes.split_at(at),
        None => (bytes, &[][..]),
    };
    if fragment.len() == 1 {
        return false;
    }
    let Ok(text) = core::str::from_utf8(address) else { return false };
    Url::parse(text).is_ok_and(|url| matches!(url.scheme(), Scheme::Http | Scheme::Https))
}

/// Reads a REDIRECT record's body.
///
/// # Errors
/// A form that is not 0 or 1, a page or sub-code outside its range, an
/// address the record may not carry, or bytes after the last field.
pub fn read(body: &[u8]) -> Result<RedirectRecord<'_>, RecordFault> {
    let (&form, rest) = body.split_first().ok_or(RecordFault::Length)?;
    match form {
        FORM_PAGE => read_page(rest),
        FORM_ADDRESS if check_address(rest) => core::str::from_utf8(rest)
            .map(RedirectRecord::Address)
            .map_err(|_| RecordFault::BadValue("redirect address")),
        FORM_ADDRESS => Err(RecordFault::BadValue("redirect address")),
        _ => Err(RecordFault::BadValue("redirect form")),
    }
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
    let mut body = [0_u8; MAX_BODY];
    let length = match redirect {
        RedirectRecord::Page { page, subcode } => {
            let ([page_high, page_low], [sub_high, sub_low]) =
                (page.mpp().to_be_bytes(), subcode.value().to_be_bytes());
            let target = body.get_mut(..PAGE_BODY).ok_or(RecordFault::Length)?;
            target.copy_from_slice(&[FORM_PAGE, page_high, page_low, sub_high, sub_low]);
            PAGE_BODY
        }
        RedirectRecord::Address(address) => {
            let length =
                FORM.checked_add(address.len()).filter(|&length| length <= MAX_BODY).ok_or(RecordFault::Length)?;
            let target = body.get_mut(..length).ok_or(RecordFault::Length)?;
            let (form, rest) = target.split_at_mut(FORM);
            form.copy_from_slice(&[FORM_ADDRESS]);
            rest.copy_from_slice(address.as_bytes());
            length
        }
    };
    let body = body.get(..length).ok_or(RecordFault::Length)?;
    // The writer checks with the reader, so the two cannot disagree.
    if read(body)? != redirect {
        return Err(RecordFault::BadValue("does not read back"));
    }
    write_packets(TYPE_REDIRECT, body, service, record_id, out).ok_or(RecordFault::Length)
}

#[cfg(test)]
mod tests;
