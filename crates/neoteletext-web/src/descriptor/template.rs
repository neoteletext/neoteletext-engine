//! The page address template (REQ-WEB-003): its placeholders checked once,
//! and expanded for a page and sub-code.

use super::{Descriptor, DescriptorError};
use crate::bytes::Writer;
use crate::consts::MAX_URL_LENGTH;
use crate::url::{Url, UrlError};
use neoteletext_core::types::{PageAddress, SubCode};

/// Characters of `{sub}`: four hexadecimal digits.
const SUBCODE_DIGITS: u32 = 4;
// `Writer::hex` writes at most this many (finding F-92).
const _: () = assert!(SUBCODE_DIGITS <= crate::bytes::U32_HEX_DIGITS);

/// Checks a template's placeholders: only `{m}`, `{pp}`, `{mpp}` and
/// `{sub}`, no other brace, and none before the path. Placeholders become
/// hexadecimal digits, which are valid anywhere in a path or query, so one
/// expansion that parses proves they all do; in the host they would not
/// (`1.2.3.{pp}` is an address for some pages and a name for others).
pub(super) fn check_template(template: &[u8]) -> Result<(), DescriptorError> {
    let authority_start = match template.windows(3).position(|w| w == b"://") {
        Some(index) => Some(index.saturating_add(3)),
        None => template.starts_with(b"//").then_some(2),
    };
    if let Some(start) = authority_start {
        let after = template.get(start..).unwrap_or(&[]);
        let end = after.iter().position(|&b| b == b'/' || b == b'?').unwrap_or(after.len());
        if template.get(..start.saturating_add(end)).unwrap_or(&[]).contains(&b'{') {
            return Err(DescriptorError::Placeholder);
        }
    }
    let mut rest = template;
    while let Some(open) = rest.iter().position(|&b| b == b'{' || b == b'}') {
        if rest.get(open) == Some(&b'}') {
            return Err(DescriptorError::Placeholder);
        }
        let after = rest.get(open.saturating_add(1)..).unwrap_or(&[]);
        let close = after.iter().position(|&b| b == b'}').ok_or(DescriptorError::Placeholder)?;
        let name = after.get(..close).unwrap_or(&[]);
        if !matches!(name, b"m" | b"pp" | b"mpp" | b"sub") {
            return Err(DescriptorError::Placeholder);
        }
        rest = after.get(close.saturating_add(1)..).unwrap_or(&[]);
    }
    Ok(())
}

/// Expands a template for a page and optional sub-code (REQ-WEB-003). With
/// no sub-code, `{sub}` becomes `0000`.
pub fn expand_template(
    template: &[u8],
    page: PageAddress,
    subcode: Option<SubCode>,
    out: &mut Writer<'_>,
) -> Result<(), UrlError> {
    let digits = page.ascii();
    let too_long = |_| UrlError::TooLong;
    let mut rest = template;
    while let Some(open) = rest.iter().position(|&b| b == b'{') {
        out.put(rest.get(..open).unwrap_or(&[])).map_err(too_long)?;
        let after = rest.get(open.saturating_add(1)..).unwrap_or(&[]);
        let close = after.iter().position(|&b| b == b'}').ok_or(UrlError::Path)?;
        match after.get(..close).unwrap_or(&[]) {
            b"m" => out.put(digits.get(..1).unwrap_or(&[])),
            b"pp" => out.put(digits.get(1..).unwrap_or(&[])),
            b"mpp" => out.put(&digits),
            b"sub" => out.hex(u32::from(subcode.map_or(0, SubCode::value)), SUBCODE_DIGITS),
            _ => return Err(UrlError::Path),
        }
        .map_err(too_long)?;
        rest = after.get(close.saturating_add(1)..).unwrap_or(&[]);
    }
    out.put(rest).map_err(too_long)
}

impl Descriptor {
    /// The address of a page (REQ-WEB-003): the template expanded and
    /// resolved against the descriptor's address.
    pub fn page_url(&self, page: PageAddress, subcode: Option<SubCode>) -> Result<Url, UrlError> {
        let mut buffer = [0u8; MAX_URL_LENGTH];
        let mut writer = Writer::new(&mut buffer);
        expand_template(self.template.as_bytes(), page, subcode, &mut writer)?;
        let expanded = core::str::from_utf8(writer.written()).map_err(|_| UrlError::BadCharacter)?;
        self.base.join(expanded)
    }
}
