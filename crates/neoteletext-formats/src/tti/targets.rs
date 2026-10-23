//! Link targets (Appendix E.1) and addresses as link targets (the
//! web-native concept section 3, decision E.88).

use neoteletext_core::types::{PageAddress, SubCode};

use crate::page::{DEFAULT_ADDRESS, LINK_ADDRESS_BYTES, LinkDestination, SERVICE_NAME_BYTES, WEB_MARK};
use crate::text::Text;

use super::fields::{page_address, service_name, subcode};

/// What starts an address as a link target, rather than a page or a named
/// service (the web-native concept section 3: "a target containing
/// `://` is an address").
const ADDRESS_MARK: &[u8] = b"://";
/// The schemes an address in a page may use.
const ADDRESS_SCHEMES: [&[u8]; 2] = [b"https://", b"http://"];

/// A link target: `mpp`, `mpp:ssss`, `name:mpp` or `name:mpp:ssss`
/// (Appendix E.1).
/// An address as a link target: `http` or `https`, printable ASCII
/// without spaces or commas, and short enough to keep; or `web:` and such
/// an address, kept whole with its mark (decision E.88).
fn link_address(text: &[u8]) -> Option<Text<LINK_ADDRESS_BYTES>> {
    let address = text.strip_prefix(WEB_MARK).unwrap_or(text);
    let lower_scheme =
        |scheme: &[u8]| address.get(..scheme.len()).is_some_and(|start| start.eq_ignore_ascii_case(scheme));
    let valid = ADDRESS_SCHEMES.iter().any(|scheme| lower_scheme(scheme))
        && text.iter().all(|&byte| byte.is_ascii_graphic() && byte != b',');
    let (url, truncated) = Text::from_bytes(text);
    (valid && !truncated).then_some(url)
}

/// A link target of Appendix E, or an address (concept section 3).
pub(super) fn link_destination(text: &[u8]) -> Option<LinkDestination> {
    let text = text.trim_ascii();
    if text.starts_with(WEB_MARK) || text.windows(ADDRESS_MARK.len()).any(|window| window == ADDRESS_MARK) {
        let url = link_address(text)?;
        return Some(LinkDestination { service: Text::new(), address: DEFAULT_ADDRESS, subcode: None, url });
    }
    let (service, address, subcode) = link_target(text)?;
    Some(LinkDestination { service, address, subcode, url: Text::new() })
}

fn link_target(text: &[u8]) -> Option<(Text<SERVICE_NAME_BYTES>, PageAddress, Option<SubCode>)> {
    let mut parts = text.trim_ascii().split(|&byte| byte == b':');
    let first = parts.next()?;
    let second = parts.next();
    let third = parts.next();
    if parts.next().is_some() {
        return None;
    }
    match (second, third) {
        (None, _) => Some((Text::new(), page_address(first)?, None)),
        (Some(second), None) => {
            if let (Some(address), Some(sub)) = (page_address(first), subcode(second)) {
                Some((Text::new(), address, Some(sub)))
            } else {
                Some((service_name(first)?, page_address(second)?, None))
            }
        }
        (Some(second), Some(third)) => Some((service_name(first)?, page_address(second)?, Some(subcode(third)?))),
    }
}
