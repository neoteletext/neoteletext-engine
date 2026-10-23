//! Why something failed: the network, TLS, the relay, or a limit.

/// Why something failed.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The network: a name that did not resolve, a refused or broken
    /// connection, a time limit.
    Network(String),
    /// TLS failed, a certificate among the causes: a hard stop.
    Tls(String),
    /// The relay answered with this status.
    Status(u16),
    /// The relay broke HTTP, WebSocket or qrkeyboard's protocol, and how.
    Protocol(&'static str),
    /// The relay closed the WebSocket with this code (`CLOSE` in
    /// `protocol.ts`): 4001 to 4011.
    Closed(u16),
    /// An address that is not `http` or `https`.
    Address,
    /// Something this side was asked for is outside a limit.
    Limit(&'static str),
}

impl core::fmt::Display for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Network(why) => write!(formatter, "network: {why}"),
            Self::Tls(why) => write!(formatter, "TLS: {why}"),
            Self::Status(status) => write!(formatter, "the relay answered {status}"),
            Self::Protocol(what) => write!(formatter, "the relay broke the protocol: {what}"),
            Self::Closed(code) => write!(formatter, "the relay closed the connection ({code})"),
            Self::Address => write!(formatter, "the relay's address must be http or https"),
            Self::Limit(what) => write!(formatter, "outside a limit: {what}"),
        }
    }
}

impl std::error::Error for Error {}
