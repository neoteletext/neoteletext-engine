//! The web half of the platform interface (layer L8, REQ-ARCH-009,
//! REQ-WEB-001): what a port supplies to the web module.
//!
//! - [`Transport`]: byte-stream connections, plain or with TLS, modelled on
//!   the `embedded-io` traits. The port does the encryption and the
//!   certificate checking (on by default), and reports the peer address it
//!   actually reached so the local-network rule can be applied to it
//!   (REQ-SEC-006).
//! - [`RandomSource`]: random bytes for WebSocket keys and masks. The
//!   library has no randomness of its own.
//! - [`CookieStorage`]: optional saving of cookies between runs, only when
//!   the port enables it (REQ-SEC-013).
//!
//! The push connection (REQ-RCH-014, REQ-RCH-015) is a connection of its
//! own, HTTP/1.1, never multiplexed with fetches. A port with TLS resumes
//! the session of an earlier connection to the same host where the server
//! allows, and never sends early data (TLS 0-RTT) or uses TCP Fast Open on
//! it: a key in either could be replayed.
//!
//! [`Pump`] connects a [`Transport`] to a protocol machine for ports that
//! want the simplest loop.

use crate::consts::PUSH_UNSENT_BOUND;
use crate::url::IpAddress;

/// The connections the web module uses. A port keeps one socket per
/// identifier.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ConnectionId {
    /// Descriptor and page requests, one at a time.
    Fetch,
    /// The push connection (WebSocket or server-sent events).
    Push,
    /// The zxnet live feed.
    Live,
}

impl ConnectionId {
    /// Every identifier, for ports that keep a table.
    pub const ALL: [Self; 3] = [Self::Fetch, Self::Push, Self::Live];
}

/// Whether a connection uses Transport Layer Security.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Security {
    /// Plain TCP.
    Plain,
    /// TLS, with the certificate checked against the host name.
    Tls,
}

/// The state of a connection, as the transport reports it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinkState {
    /// No connection.
    Closed,
    /// Connecting, or negotiating TLS.
    Connecting,
    /// Connected to this peer address, after name resolution.
    Open(IpAddress),
    /// The connection attempt failed.
    Failed,
}

/// The result of a non-blocking receive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Received {
    /// This many bytes were placed in the buffer.
    Data(usize),
    /// Nothing is available now.
    Nothing,
    /// The peer closed the connection.
    Closed,
}

/// A non-blocking byte-stream transport supplied by the port (REQ-WEB-001).
/// Errors follow `embedded-io`, so existing network stacks adapt directly.
pub trait Transport: embedded_io::ErrorType {
    /// Starts connecting `connection` to `host` and `port`. Returns at once;
    /// progress is read with [`Transport::state`]. A port that cannot offer
    /// TLS returns an error for [`Security::Tls`].
    fn connect(
        &mut self,
        connection: ConnectionId,
        host: &str,
        port: u16,
        security: Security,
    ) -> Result<(), Self::Error>;

    /// The connection's state.
    fn state(&mut self, connection: ConnectionId) -> LinkState;

    /// Queues all of `bytes` for sending, or fails. The port buffers what
    /// the network cannot take at once. Bytes given in one call go in one
    /// write where the transport allows, one TLS record up to its size
    /// (REQ-RCH-010): the agent gives the messages waiting at one moment
    /// together.
    fn send(&mut self, connection: ConnectionId, bytes: &[u8]) -> Result<(), Self::Error>;

    /// The bytes given to [`Transport::send`] on `connection` that have not
    /// yet left for the network: the port's own buffer, and where the
    /// platform tells it the socket's (Linux's `SIOCOUTQNSD`), 0 when it
    /// cannot see them. The agent writes a pointer's move only while the
    /// push connection's are under [`crate::consts::PUSH_UNSENT_BOUND`]
    /// (REQ-RCH-035); a port reports them as [`NetInput::Unsent`].
    fn unsent(&mut self, connection: ConnectionId) -> usize;

    /// Reads whatever has arrived, without blocking.
    fn receive(&mut self, connection: ConnectionId, buffer: &mut [u8]) -> Result<Received, Self::Error>;

    /// Closes the connection. Closing a closed connection does nothing.
    fn close(&mut self, connection: ConnectionId);
}

/// Random bytes for WebSocket handshake keys and frame masks (RFC 6455
/// sections 4.1 and 5.3). The port should use a cryptographic generator;
/// RFC 6455 requires masks an attacker cannot predict.
pub trait RandomSource {
    /// Fills `bytes` with random values.
    fn fill(&mut self, bytes: &mut [u8]);
}

/// Saving the cookie store between runs (REQ-SEC-013). A port enables this
/// only deliberately, and should store the data as a secret.
pub trait CookieStorage {
    /// The storage's error type.
    type Error;

    /// Replaces the saved data.
    fn save(&mut self, data: &[u8]) -> Result<(), Self::Error>;

    /// Reads the saved data into `buffer`, returning its length (0 when
    /// nothing is saved).
    fn load(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error>;
}

/// A connection-level input for a protocol machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetInput<'a> {
    /// The connection opened, to this peer.
    Connected(ConnectionId, IpAddress),
    /// The connection attempt failed.
    ConnectFailed(ConnectionId),
    /// Bytes arrived.
    Received(ConnectionId, &'a [u8]),
    /// The connection closed or broke.
    Closed(ConnectionId),
    /// The bytes sent on the connection not yet gone to the network
    /// ([`Transport::unsent`]). A port reports them when they reach
    /// [`crate::consts::PUSH_UNSENT_BOUND`] after a send, and when they fall
    /// under it again (where the platform has it, `TCP_NOTSENT_LOWAT` at the
    /// bound wakes it then); [`Pump`] does both (REQ-RCH-035).
    Unsent(ConnectionId, usize),
}

/// A connection-level output of a protocol machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetOutput<'a> {
    /// Open a connection.
    Connect {
        /// Which connection.
        connection: ConnectionId,
        /// The host name or address literal, without brackets.
        host: &'a str,
        /// The port.
        port: u16,
        /// Plain or TLS.
        security: Security,
    },
    /// Send bytes.
    Send(ConnectionId, &'a [u8]),
    /// Close the connection.
    Close(ConnectionId),
}

fn slot(connection: ConnectionId) -> usize {
    match connection {
        ConnectionId::Fetch => 0,
        ConnectionId::Push => 1,
        ConnectionId::Live => 2,
    }
}

/// Carries a protocol machine's network outputs out on a [`Transport`],
/// and turns what the transport reports into network inputs, each change
/// once. A port that wants the simplest loop uses it like this:
///
/// ```text
/// loop {
///     for connection in [ConnectionId::Fetch, ConnectionId::Push] {
///         if let Some(input) = pump.poll(connection, &mut buffer) {
///             agent.handle(now, Input::Net(input), &mut |output| match output.net() {
///                 Some(net) => pump.execute(net),
///                 None => application(output),
///             });
///         }
///     }
///     // Feed user requests and Input::Tick the same way.
/// }
/// ```
#[derive(Debug)]
pub struct Pump<T: Transport> {
    /// The port's transport.
    pub transport: T,
    connecting: [bool; 3],
    open: [bool; 3],
    failed: [bool; 3],
    /// Each connection's unsent bytes were last reported at or over
    /// [`PUSH_UNSENT_BOUND`].
    congested: [bool; 3],
}

impl<T: Transport> Pump<T> {
    /// A pump over `transport`, with no connections.
    pub const fn new(transport: T) -> Self {
        Self { transport, connecting: [false; 3], open: [false; 3], failed: [false; 3], congested: [false; 3] }
    }

    fn set(flags: &mut [bool; 3], connection: ConnectionId, value: bool) {
        if let Some(flag) = flags.get_mut(slot(connection)) {
            *flag = value;
        }
    }

    fn get(flags: [bool; 3], connection: ConnectionId) -> bool {
        flags.get(slot(connection)).copied().unwrap_or(false)
    }

    /// Carries out one network output. A failure is reported by the next
    /// [`Self::poll`] of that connection.
    pub fn execute(&mut self, output: NetOutput<'_>) {
        match output {
            NetOutput::Connect { connection, host, port, security } => {
                self.transport.close(connection);
                Self::set(&mut self.open, connection, false);
                Self::set(&mut self.congested, connection, false);
                let ok = self.transport.connect(connection, host, port, security).is_ok();
                Self::set(&mut self.connecting, connection, ok);
                Self::set(&mut self.failed, connection, !ok);
            }
            NetOutput::Send(connection, bytes) => {
                if self.transport.send(connection, bytes).is_err() {
                    self.transport.close(connection);
                    Self::set(&mut self.failed, connection, Self::get(self.open, connection));
                }
            }
            NetOutput::Close(connection) => {
                self.transport.close(connection);
                Self::set(&mut self.open, connection, false);
                Self::set(&mut self.congested, connection, false);
                Self::set(&mut self.connecting, connection, false);
                Self::set(&mut self.failed, connection, false);
            }
        }
    }

    /// Polls one connection without blocking. Received bytes are placed in
    /// `buffer`, whose size bounds one read.
    pub fn poll<'b>(&mut self, connection: ConnectionId, buffer: &'b mut [u8]) -> Option<NetInput<'b>> {
        if Self::get(self.failed, connection) {
            Self::set(&mut self.failed, connection, false);
            let was_open = Self::get(self.open, connection);
            Self::set(&mut self.open, connection, false);
            Self::set(&mut self.connecting, connection, false);
            return Some(if was_open { NetInput::Closed(connection) } else { NetInput::ConnectFailed(connection) });
        }
        if Self::get(self.connecting, connection) {
            return match self.transport.state(connection) {
                LinkState::Open(peer) => {
                    Self::set(&mut self.connecting, connection, false);
                    Self::set(&mut self.open, connection, true);
                    Some(NetInput::Connected(connection, peer))
                }
                LinkState::Failed | LinkState::Closed => {
                    Self::set(&mut self.connecting, connection, false);
                    Some(NetInput::ConnectFailed(connection))
                }
                LinkState::Connecting => None,
            };
        }
        if !Self::get(self.open, connection) {
            return None;
        }
        // REQ-RCH-035: the agent hears when the unsent bytes reach the
        // bound, and when they fall under it, and nothing in between.
        let unsent = self.transport.unsent(connection);
        if (unsent >= PUSH_UNSENT_BOUND) != Self::get(self.congested, connection) {
            Self::set(&mut self.congested, connection, unsent >= PUSH_UNSENT_BOUND);
            return Some(NetInput::Unsent(connection, unsent));
        }
        match self.transport.receive(connection, buffer) {
            Ok(Received::Data(0) | Received::Nothing) => None,
            Ok(Received::Data(n)) => Some(NetInput::Received(connection, buffer.get(..n.min(buffer.len()))?)),
            Ok(Received::Closed) | Err(_) => {
                Self::set(&mut self.open, connection, false);
                self.transport.close(connection);
                Some(NetInput::Closed(connection))
            }
        }
    }
}
