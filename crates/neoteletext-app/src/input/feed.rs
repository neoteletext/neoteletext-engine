//! The live feed: which inputs are live, and how a live source is opened
//! and collected for a session (REQ-CLI-002, REQ-SEC-018).

use super::{Options, config, load};
use crate::Failure;
use crate::session::Session;
use crate::source::{Finished, Source};
use neoteletext_core::fsm::PageState;
use neoteletext_core::types::PageAddress;

/// True for inputs that are live feeds or web services.
#[must_use]
pub fn is_live(input: &str) -> bool {
    input.starts_with("zxnet:")
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
        source.request(page, neoteletext_core::types::SubCode::ANY);
    }
    let mut asked_initial = wanted.is_some();
    while start.elapsed() < limit {
        let now = neoteletext_core::types::Instant(u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX));
        let alive = source.poll(&mut session, now);
        for notice in source.notices() {
            match notice {
                crate::pipe::Notice::InitialPage(mpp) if !asked_initial => {
                    if let Some(page) = PageAddress::from_mpp(mpp) {
                        source.request(page, neoteletext_core::types::SubCode::ANY);
                        asked_initial = true;
                    }
                }
                crate::pipe::Notice::Message(text) => eprintln!("neottx: {text}"),
                _ => {}
            }
        }
        let done = wanted
            .and_then(|page| session.page(page, neoteletext_core::types::SubCode::ANY))
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
