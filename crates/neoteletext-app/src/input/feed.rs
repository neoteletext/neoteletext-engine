//! The live feed: which inputs are live, and how a live source is opened
//! and collected for a session (REQ-CLI-002, REQ-SEC-018).

use super::{Options, config, load};
use crate::Failure;
use crate::session::Session;
use crate::source::{Finished, Source};
use neoteletext_core::fsm::PageState;
use neoteletext_core::types::{PageAddress, SubCode};

/// What begins a link from outside: the address it wraps follows
/// (DEC-1663, DEC-1666).
const LINK_SCHEME: &str = "neoteletext://";
/// The schemes a link may wrap (REQ-1672 item 2).
const LINK_TARGETS: [&str; 2] = ["http://", "https://"];

/// Whether `text` starts with `prefix`, whatever the letters' case, as a
/// URI's scheme may (RFC 3986 section 3.1); and what follows it.
fn after_prefix<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let start = text.get(..prefix.len())?;
    start.eq_ignore_ascii_case(prefix).then(|| text.get(prefix.len()..)).flatten()
}

/// The address to open for `input`: a `neoteletext://` link's wrapped
/// address, which then goes as a plain address does, and any other input
/// as it is (REQ-1672, TODO-1679).
///
/// # Errors
/// A link that wraps no `http` or `https` address.
pub fn link_address(input: &str) -> Result<&str, String> {
    let Some(wrapped) = after_prefix(input, LINK_SCHEME) else { return Ok(input) };
    if LINK_TARGETS.iter().any(|scheme| after_prefix(wrapped, scheme).is_some()) {
        Ok(wrapped)
    } else {
        Err(format!("{LINK_SCHEME} wraps an http or https address, as in {LINK_SCHEME}https://example.org/channel/"))
    }
}

/// Where an address goes inside its channel (REQ-1693), for a viewer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Opening {
    /// The page and sub-code it names; `None` arrives at the channel's
    /// initial page, as `#contents` and `#wall` do in a viewer that shows
    /// neither.
    pub page: Option<(PageAddress, SubCode)>,
    /// The page parameters that go with that selection, as written.
    pub parameters: String,
}

/// Where `input` goes inside its channel. A channel's address follows the
/// map, normalised as it says; anything else that opens names no place.
///
/// # Errors
/// A channel's address the map refuses (REQ-1693 item 4), or a link that
/// wraps no `http` or `https` address.
#[cfg(feature = "live")]
pub fn opening(input: &str) -> Result<Opening, String> {
    use neoteletext_web::channel::is_channel_address;
    use neoteletext_web::map::{MapAddress, Place};
    let address = link_address(input)?;
    if !is_channel_address(address) {
        return Ok(Opening::default());
    }
    let read = MapAddress::read_opened(address).map_err(|_| format!("{address:?} is not an address of a channel"))?;
    Ok(match read.place {
        Some(Place::Page { page, subcode, parameters }) => Opening {
            page: Some((page, subcode.unwrap_or(SubCode::ANY))),
            parameters: parameters.unwrap_or_default().to_owned(),
        },
        Some(Place::Contents | Place::Wall) | None => Opening::default(),
    })
}

/// A build without the `live` feature opens no address.
///
/// # Errors
/// Never.
#[cfg(not(feature = "live"))]
pub fn opening(_input: &str) -> Result<Opening, String> {
    Ok(Opening::default())
}

/// True for inputs that are live feeds or web services.
#[must_use]
pub fn is_live(input: &str) -> bool {
    input.starts_with("zxnet:")
        || after_prefix(input, LINK_SCHEME).is_some()
        || input.starts_with("wss://")
        || input.starts_with("http://")
        || input.starts_with("https://")
}

/// Starts the fetcher process for a live input (REQ-SEC-018).
#[cfg(feature = "live")]
fn live_source(options: &Options) -> Result<Box<dyn Source>, Failure> {
    crate::live::open(&options.input, options.no_sandbox, &options.allow_local, options.proxy.as_deref())
}

/// A build without the `live` feature has no web module to fetch with.
#[cfg(not(feature = "live"))]
fn live_source(_options: &Options) -> Result<Box<dyn Source>, Failure> {
    Err(crate::not_in_build("live feeds and web services", "live"))
}

/// Collects a live source for `--duration` seconds, or until `wanted` is
/// stored and complete (REQ-CLI-002).
pub(super) fn load_live(options: &Options, wanted: Option<PageAddress>) -> Result<Session, Failure> {
    let config = config(options)?;
    let mut session = Session::new(config, options.pages);
    let mut source = live_source(options)?;
    let start = std::time::Instant::now();
    let limit = std::time::Duration::from_secs(options.duration);
    if let Some(page) = wanted {
        source.request(page, SubCode::ANY);
    }
    let mut asked_initial = wanted.is_some();
    while start.elapsed() < limit {
        let now = neoteletext_core::types::Instant(u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX));
        let alive = source.poll(&mut session, now);
        for notice in source.notices() {
            match notice {
                crate::pipe::Notice::InitialPage(mpp) if !asked_initial => {
                    if let Some(page) = PageAddress::from_mpp(mpp) {
                        source.request(page, SubCode::ANY);
                        asked_initial = true;
                    }
                }
                crate::pipe::Notice::Message(text) => eprintln!("neottx: {text}"),
                _ => {}
            }
        }
        let done = wanted
            .and_then(|page| session.page(page, SubCode::ANY))
            .is_some_and(|page| page.state == PageState::Complete);
        if done || !alive {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    session.finish();
    Ok(session)
}

/// Opens an input for a viewer: a file read whole, or a live feed.
pub fn open_live(options: &Options) -> Result<(Session, Box<dyn Source>), Failure> {
    if is_live(&options.input) {
        let config = config(options)?;
        let session = Session::new(config, options.pages);
        let source = live_source(options)?;
        return Ok((session, source));
    }
    Ok((load(options)?, Box::new(Finished)))
}
