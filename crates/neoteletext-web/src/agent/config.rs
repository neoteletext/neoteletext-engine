//! The agent's configuration and the buffers it works in, supplied by the
//! caller (REQ-GEN-003, REQ-SEC-020).

use crate::consts::{
    ALLOWED_LOCAL, DEFAULT_CONNECT_TIMEOUT_US, DEFAULT_COOKIES_PER_HOST, DEFAULT_COOKIES_TOTAL,
    DEFAULT_DESCRIPTOR_LIMIT, DEFAULT_FETCH_INTERVAL_US, DEFAULT_HEADER_LIMIT, DEFAULT_PAGE_LIMIT, DEFAULT_PUSH_LIMIT,
    DEFAULT_RESPONSE_TIMEOUT_US,
};
use crate::url::IpAddress;

/// The agent's configuration. Every security setting defaults to the safer
/// choice (REQ-SEC-020).
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent yes-or-no setting named by a requirement, not a state"
)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Largest page response body (REQ-WEB-008), at most the body buffer.
    pub page_limit: usize,
    /// Largest push message (REQ-WEB-008), at most the push buffer.
    pub push_limit: usize,
    /// Largest descriptor (REQ-WEB-008), at most the body buffer.
    pub descriptor_limit: usize,
    /// Largest response head (REQ-WEB-008), at most the head buffers.
    pub header_limit: usize,
    /// Shortest interval between fetches (REQ-WEB-004).
    pub fetch_interval_us: u64,
    /// Time limit for opening a connection (REQ-SEC-011).
    pub connect_timeout_us: u64,
    /// Time limit for a request and its response (REQ-SEC-011).
    pub response_timeout_us: u64,
    /// The port can offer TLS. Without it, HTTPS addresses are reported
    /// unreachable (REQ-WEB-008).
    pub tls_available: bool,
    /// Local-network addresses explicitly allowed (REQ-SEC-006). None by
    /// default.
    pub allowed_local: [Option<IpAddress>; ALLOWED_LOCAL],
    /// Cookies kept per host (REQ-WEB-006).
    pub cookies_per_host: usize,
    /// Cookies kept in total (REQ-WEB-006).
    pub cookies_total: usize,
    /// The port saves cookies between runs (REQ-SEC-013). Off by default.
    pub persist_cookies: bool,
    /// Refuse redirects of descriptor, page, bundle and contents requests
    /// that leave the channel, reporting [`Event::RedirectToOtherChannel`](super::Event::RedirectToOtherChannel)
    /// instead, so that the viewer can confirm another channel before
    /// showing its content (REQ-WEB-021). An upgrade from `http` to `https`
    /// on the same host and folder, between the default ports, keeps the
    /// channel. For a service opened by its descriptor, the channel is its
    /// origin. Off by default, which keeps `neottx`'s behaviour.
    pub redirects_within_channel: bool,
    /// The program the agent names itself as, in its `User-Agent`
    /// (REQ-WEB-031); `None` names the engine.
    pub product: Option<crate::user_agent::Product>,
    /// The port's transport goes through a proxy (REQ-WEB-032), so the
    /// peer it reports is the proxy, which the user chose: the local
    /// network is then refused by what the agent can see of a host before
    /// it connects ([`crate::proxy::hidden_local`]), not by the peer.
    pub proxied: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            page_limit: DEFAULT_PAGE_LIMIT,
            push_limit: DEFAULT_PUSH_LIMIT,
            descriptor_limit: DEFAULT_DESCRIPTOR_LIMIT,
            header_limit: DEFAULT_HEADER_LIMIT,
            fetch_interval_us: DEFAULT_FETCH_INTERVAL_US,
            connect_timeout_us: DEFAULT_CONNECT_TIMEOUT_US,
            response_timeout_us: DEFAULT_RESPONSE_TIMEOUT_US,
            tls_available: true,
            allowed_local: [None; ALLOWED_LOCAL],
            cookies_per_host: DEFAULT_COOKIES_PER_HOST,
            cookies_total: DEFAULT_COOKIES_TOTAL,
            persist_cookies: false,
            redirects_within_channel: false,
            product: None,
            proxied: false,
        }
    }
}

/// The byte buffers the agent works in, supplied by the caller
/// (REQ-GEN-003). Their lengths bound the corresponding limits.
#[derive(Debug)]
pub struct Buffers<'m> {
    /// Response heads on the fetch connection.
    pub fetch_head: &'m mut [u8],
    /// Requests on the fetch connection.
    pub fetch_request: &'m mut [u8],
    /// Page, descriptor, bundle and contents response bodies.
    pub body: &'m mut [u8],
    /// The push connection's handshake response head.
    pub push_head: &'m mut [u8],
    /// Requests and frames on the push connection.
    pub push_request: &'m mut [u8],
    /// One push message.
    pub push_message: &'m mut [u8],
}

/// Buffers of the default sizes, for callers that do not need to choose.
#[derive(Debug)]
pub struct Storage {
    fetch_head: [u8; DEFAULT_HEADER_LIMIT],
    fetch_request: [u8; crate::consts::DEFAULT_REQUEST_LIMIT],
    body: [u8; DEFAULT_PAGE_LIMIT],
    push_head: [u8; DEFAULT_HEADER_LIMIT],
    push_request: [u8; crate::consts::DEFAULT_REQUEST_LIMIT],
    push_message: [u8; DEFAULT_PUSH_LIMIT],
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage {
    /// Zeroed buffers. Large: the caller places them in static memory or
    /// on the heap.
    #[must_use]
    // The arrays are the buffers themselves; building them is the point.
    #[allow(clippy::large_stack_arrays)]
    pub const fn new() -> Self {
        Self {
            fetch_head: [0; DEFAULT_HEADER_LIMIT],
            fetch_request: [0; crate::consts::DEFAULT_REQUEST_LIMIT],
            body: [0; DEFAULT_PAGE_LIMIT],
            push_head: [0; DEFAULT_HEADER_LIMIT],
            push_request: [0; crate::consts::DEFAULT_REQUEST_LIMIT],
            push_message: [0; DEFAULT_PUSH_LIMIT],
        }
    }

    /// The buffers, borrowed for an agent.
    pub fn buffers(&mut self) -> Buffers<'_> {
        Buffers {
            fetch_head: &mut self.fetch_head,
            fetch_request: &mut self.fetch_request,
            body: &mut self.body,
            push_head: &mut self.push_head,
            push_request: &mut self.push_request,
            push_message: &mut self.push_message,
        }
    }
}
