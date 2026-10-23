//! Form submission (REQ-WEB-011, Appendix C): the encoding of the values
//! and the destination check. The form model, focus, editing and
//! validation belong to the viewer in the core (layer L6); this module
//! receives the validated values. Together they are the forms of
//! REQ-WEB-010.

use neoteletext_core::policy;

use crate::bytes::{Overflow, Writer};
use crate::consts::{FIELD_ACTION, FIELD_TOKEN};
use crate::url::{Origin, Url, UrlError};

/// The hexadecimal digits after `%` in a percent-encoded byte (RFC 3986
/// section 2.1).
const PERCENT_DIGITS: u32 = 2;
// `Writer::hex` writes at most this many (finding F-92).
const _: () = assert!(PERCENT_DIGITS <= crate::bytes::U32_HEX_DIGITS);

/// The method of the `FS` command.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FormMethod {
    /// Values in the query of a GET request.
    Get,
    /// Values in the body of a POST request.
    Post,
}

/// A form submission handed over by the viewer.
#[derive(Clone, Copy, Debug)]
pub struct FormSubmission<'a> {
    /// The address of the page holding the form, as delivered with its
    /// bytes. Its origin is the form's origin.
    pub page_url: &'a str,
    /// The `FS` method.
    pub method: FormMethod,
    /// The `FS` address, absolute or relative to the page.
    pub target: &'a str,
    /// Field names and values, already validated (Appendix C.5).
    pub fields: &'a [(&'a str, &'a str)],
    /// The `FT` token, sent as `ttx_token`.
    pub token: Option<&'a str>,
    /// The pressed button's action, sent as `ttx_action`.
    pub action: Option<&'a str>,
}

/// Why a form may not be submitted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FormRefused {
    /// The page or target address failed the strict parser.
    Address(UrlError),
    /// The target is not the page's origin nor a permitted origin
    /// (REQ-WEB-008, REQ-SEC-009).
    Origin,
    /// The target is a WebSocket address.
    Scheme,
    /// A submission of this form is already in progress (REQ-WEB-011).
    Busy,
    /// The encoded values do not fit.
    TooLarge,
    /// This build has no form submission (the `forms` feature, decision
    /// E.61).
    NotCompiledIn,
}

/// Resolves the target and checks that the form may submit there: the
/// page's own origin or one the descriptor lists, as the security policy
/// decides.
pub fn destination<'o>(
    page_url: &Url,
    target: &str,
    permitted: impl Iterator<Item = &'o Origin>,
) -> Result<Url, FormRefused> {
    let url = page_url.join(target).map_err(FormRefused::Address)?;
    if url.scheme().is_websocket() {
        return Err(FormRefused::Scheme);
    }
    let page_origin = page_url.origin();
    let target_origin = url.origin();
    let mut allowed = [page_origin.parts(); crate::consts::MAX_FORM_ORIGINS];
    let mut count = 0usize;
    for (slot, origin) in allowed.iter_mut().zip(permitted) {
        *slot = origin.parts();
        count = count.saturating_add(1);
    }
    if policy::may_submit(page_origin.parts(), target_origin.parts(), allowed.get(..count).unwrap_or(&[])) {
        Ok(url)
    } else {
        Err(FormRefused::Origin)
    }
}

/// Writes one name or value in `application/x-www-form-urlencoded` form
/// (WHATWG URL Standard section 5.2): `*-._`, digits and letters as they
/// are, space as `+`, every other byte percent-encoded in upper case.
pub fn encode_component(text: &[u8], out: &mut Writer<'_>) -> Result<(), Overflow> {
    for &byte in text {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'*' | b'-' | b'.' | b'_') {
            out.byte(byte)?;
        } else if byte == b' ' {
            out.byte(b'+')?;
        } else {
            out.byte(b'%')?;
            out.hex(u32::from(byte), PERCENT_DIGITS)?;
        }
    }
    Ok(())
}

/// Encodes the fields, then `ttx_token` and `ttx_action` when present
/// (REQ-WEB-011 item 2).
pub fn encode(
    fields: &[(&str, &str)],
    token: Option<&str>,
    action: Option<&str>,
    out: &mut Writer<'_>,
) -> Result<(), Overflow> {
    let extras = [(FIELD_TOKEN, token), (FIELD_ACTION, action)];
    let all = fields
        .iter()
        .map(|(name, value)| (name.as_bytes(), value.as_bytes()))
        .chain(extras.iter().filter_map(|(name, value)| value.map(|v| (*name, v.as_bytes()))));
    for (index, (name, value)) in all.enumerate() {
        if index > 0 {
            out.byte(b'&')?;
        }
        encode_component(name, out)?;
        out.byte(b'=')?;
        encode_component(value, out)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;

    fn encoded(fields: &[(&str, &str)], token: Option<&str>, action: Option<&str>) -> std::string::String {
        let mut buffer = [0u8; 256];
        let mut writer = Writer::new(&mut buffer);
        encode(fields, token, action, &mut writer).unwrap();
        std::string::String::from_utf8(writer.written().to_vec()).unwrap()
    }

    #[test]
    fn urlencoding() {
        assert_eq!(
            encoded(
                &[("name", "Ann Smith"), ("email", "a+b@c.example"), ("note", "50% & more=\u{e9}")],
                Some("5f2c9a17e0"),
                Some("send_contact")
            ),
            "name=Ann+Smith&email=a%2Bb%40c.example&note=50%25+%26+more%3D%C3%A9&ttx_token=5f2c9a17e0&ttx_action=send_contact"
        );
        assert_eq!(encoded(&[], None, None), "");
        assert_eq!(encoded(&[("a", "")], None, Some("go")), "a=&ttx_action=go");
        assert_eq!(encoded(&[("x", "*-._~")], None, None), "x=*-._%7E");
    }

    #[test]
    fn overflow_is_reported() {
        let mut buffer = [0u8; 8];
        let mut writer = Writer::new(&mut buffer);
        assert_eq!(encode(&[("name", "long value")], None, None, &mut writer), Err(Overflow));
    }

    #[test]
    fn destinations() {
        let page = Url::parse("https://example.org/ttx/40001").unwrap();
        let forms = Url::parse("https://forms.example.org/").unwrap().origin();
        assert_eq!(
            destination(&page, "/contact", core::iter::empty()).unwrap().as_str(),
            "https://example.org/contact"
        );
        assert_eq!(destination(&page, "https://evil.example/steal", core::iter::empty()), Err(FormRefused::Origin));
        assert_eq!(destination(&page, "http://example.org/contact", core::iter::empty()), Err(FormRefused::Origin));
        assert_eq!(destination(&page, "https://example.org:444/x", core::iter::empty()), Err(FormRefused::Origin));
        assert_eq!(
            destination(&page, "https://forms.example.org/submit", [forms].iter()).unwrap().as_str(),
            "https://forms.example.org/submit"
        );
        assert_eq!(destination(&page, "wss://example.org/x", core::iter::empty()), Err(FormRefused::Scheme));
        assert_eq!(
            destination(&page, "https://user@example.org/", core::iter::empty()),
            Err(FormRefused::Address(UrlError::UserInfo))
        );
        let plain = Url::parse("http://plain.example/p").unwrap();
        assert_eq!(destination(&plain, "/f", core::iter::empty()).unwrap().as_str(), "http://plain.example/f");
    }
}
