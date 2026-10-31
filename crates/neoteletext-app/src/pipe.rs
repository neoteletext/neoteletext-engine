//! The framed messages between the fetcher and decoder processes
//! (REQ-SEC-018, decision 26 of IMPLEMENTATION-LOG.md). Each frame is a
//! kind byte, a 32-bit big-endian length and the payload. Both sides bound
//! the length, parse strictly and reject unknown kinds.

mod codec;

pub use codec::{read, write};

/// The longest input message a frame carries (`neoteletext-common/INPUT.md`
/// section 5.1): the web module's `INPUT_MESSAGE_MAX` and the input crate's
/// `MAX_MESSAGE`, which this crate may be built without. The command-line
/// program asserts that they agree.
pub const MAX_INPUT_MESSAGE: usize = 8192;
/// The largest payload: one page response (REQ-WEB-008 default).
pub const MAX_FRAME: usize = 256 * 1024;

/// Where page bytes came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    /// A page fetch.
    Page,
    /// The push connection.
    Push,
    /// The front door of a site without a descriptor: any number of the
    /// site's pages.
    FrontDoor,
}

/// Page bytes from the fetcher.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageData {
    /// Came over an encrypted connection.
    pub secure: bool,
    /// Where the bytes came from.
    pub origin: Origin,
    /// TTI or T42 bytes.
    pub bytes: Vec<u8>,
}

/// A message on the pipe.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Frame {
    /// Fetcher to decoder: one 42-byte packet.
    Packet([u8; neoteletext_core::consts::PACKET_BYTES]),
    /// Fetcher to decoder: TTI page bytes.
    Tti(PageData),
    /// Fetcher to decoder: T42 page bytes.
    T42(PageData),
    /// Fetcher to decoder: a notice for the viewer.
    Notice(Notice),
    /// Decoder to fetcher: request a page (`0xMPP`, sub-code or 0xFFFF).
    Request(u16, u16),
    /// Decoder to fetcher: follow a link to another service.
    Follow(String, u16, u16),
    /// Decoder to fetcher: one input message for the channel's push
    /// WebSocket (area INT, REQ-INT-101), at most [`MAX_INPUT_MESSAGE`]
    /// bytes.
    Input(Vec<u8>),
}

/// Notices the fetcher sends.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Notice {
    /// The service's initial page, `0xMPP`.
    InitialPage(u16),
    /// The page on screen came over plain HTTP (REQ-SEC-008).
    Unencrypted,
    /// A message for the user.
    Message(String),
    /// A new service is ready; its linked services' short names in
    /// descriptor order (Appendix E).
    Service(Vec<String>),
    /// The server confirmed page `0xMPP` unchanged (REQ-WEB-004): the
    /// stored copy is current (REQ-WEB-009).
    Current(u16),
    /// Fetching page `0xMPP` failed for want of a network (REQ-WEB-009).
    Offline(u16),
    /// A request was redirected out of the channel, or the channel's
    /// program sent the viewer to another channel, and it was not followed:
    /// its address (REQ-WEB-021, REQ-1688).
    RedirectToOtherChannel(String),
    /// The channel's program sent the viewer to page `0xMPP`, at this
    /// sub-code (`INPUT.md` section 7.11).
    PageRedirect(u16, u16),
    /// The service's name, from its descriptor or its host.
    ServiceName(String),
    /// The descriptor's aliases: page numbers, `0xMPP`, and their targets,
    /// `name:mpp` or an address (concept section 5 item 4).
    Aliases(Vec<(u16, String)>),
    /// Ask for the page on screen again this often, in seconds: the
    /// descriptor's refresh interval for a service without push.
    Refresh(u32),
    /// A bundle of pages arrived (a file, or a site's front door); its
    /// first page, `0xMPP`, is shown if the page asked for is not in it
    /// and the user has not moved.
    Bundle(u16),
    /// Keys may go to the channel now (true) or not (false): its push
    /// WebSocket opened or closed, and its descriptor declares input.
    Input(bool),
}

#[cfg(test)]
mod tests;
