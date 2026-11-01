//! The std transport: the engine's network port over `neoteletext-transport`,
//! each connection read by a thread of its own that wakes the loop when bytes
//! arrive (decision 27, REQ-WEB-032).

use super::decoder_link::send;
use super::wakes::Wake;
use crate::pipe::{Frame, Notice};
use neoteletext_transport::{
    Connection, SOCKET_READ, TlsFailure, client_config, connect_within_limit, mozilla_roots, open_tunnel, start_tls,
    web_address, write_records,
};
use neoteletext_web::port::{ConnectionId, LinkState, RandomSource, Received, Security, Transport};
use neoteletext_web::proxy::Proxy;
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::mpsc::Sender;

/// A blocking-connect transport over std sockets and rustls, checking every
/// resolved address against the policy before connecting (decision 27),
/// whose connections each wake the loop when bytes arrive.
pub struct StdTransport {
    streams: [Option<Connection>; 3],
    states: [LinkState; 3],
    tls: Arc<rustls::ClientConfig>,
    allowed_local: Vec<IpAddr>,
    wake: Sender<Wake>,
    /// Every connection goes through this proxy (REQ-WEB-032).
    proxy: Option<Proxy>,
}

fn slot(connection: ConnectionId) -> usize {
    match connection {
        ConnectionId::Fetch => 0,
        ConnectionId::Push => 1,
        ConnectionId::Live => 2,
    }
}

impl StdTransport {
    /// A transport trusting the Mozilla root certificates, waking the loop
    /// through `wake`.
    #[must_use]
    pub fn new(allowed_local: &[IpAddr], wake: Sender<Wake>, proxy: Option<Proxy>) -> Self {
        let tls = client_config(mozilla_roots());
        Self {
            streams: [None, None, None],
            states: [LinkState::Closed; 3],
            tls: Arc::new(tls),
            allowed_local: allowed_local.to_vec(),
            wake,
            proxy,
        }
    }

    /// A socket that reaches `host` and `port`: directly, to an address the
    /// local-network rule allows, or through the proxy, whose address the
    /// user chose, after its handshake (REQ-WEB-032). With it, the host's
    /// first bytes when the proxy's answer brought them.
    fn reach(&self, host: &str, port: u16) -> Result<(TcpStream, IpAddr, Vec<u8>), String> {
        let (socket, early) = if let Some(proxy) = &self.proxy {
            open_tunnel(proxy, host, port)?
        } else {
            let candidates: Vec<SocketAddr> =
                (host, port).to_socket_addrs().map_err(|error| format!("{host}: {error}"))?.collect();
            let allowed = candidates
                .iter()
                .find(|address| !web_address(address.ip()).is_blocked() || self.allowed_local.contains(&address.ip()))
                .ok_or_else(|| format!("{host}: every address is on a blocked local network (REQ-SEC-006)"))?;
            (connect_within_limit(allowed, host)?, Vec::new())
        };
        socket.set_nodelay(true).map_err(|error| error.to_string())?;
        let peer = socket.peer_addr().map_err(|error| error.to_string())?.ip();
        Ok((socket, peer, early))
    }

    fn open(&self, host: &str, port: u16, security: Security) -> Result<(Connection, IpAddr), String> {
        let (mut socket, peer, early) = self.reach(host, port)?;
        let (tls, first) = match security {
            Security::Plain => (None, early),
            Security::Tls => {
                let connection =
                    start_tls(self.tls.clone(), host, &early, &mut socket).map_err(|failure| match failure {
                        TlsFailure::Name(error) => format!("{host}: {error}"),
                        TlsFailure::Start(error) => error.to_string(),
                        TlsFailure::Read(_) | TlsFailure::Records(_) | TlsFailure::Handshake(_) => {
                            format!("{host}: TLS: {failure}")
                        }
                    })?;
                (Some(Box::new(connection)), Vec::new())
            }
        };
        let wake = self.wake.clone();
        let wake = move || {
            let _ = wake.send(Wake::Network);
        };
        let connection = Connection::start(socket, tls, first, SOCKET_READ, wake).map_err(|error| error.to_string())?;
        Ok((connection, peer))
    }
}

/// What has arrived on `connection`, looking without waiting: any failure
/// is the connection's end.
fn receive(connection: &mut Connection, buffer: &mut [u8]) -> Received {
    connection.take(buffer, None, write_records).unwrap_or(Received::Closed)
}

impl embedded_io::ErrorType for StdTransport {
    type Error = embedded_io::ErrorKind;
}

impl Transport for StdTransport {
    fn connect(
        &mut self,
        connection: ConnectionId,
        host: &str,
        port: u16,
        security: Security,
    ) -> Result<(), Self::Error> {
        let index = slot(connection);
        let (state, stream) = match self.open(host, port, security) {
            Ok((stream, peer)) => (LinkState::Open(web_address(peer)), Some(stream)),
            Err(message) => {
                let _ = send(&Frame::Notice(Notice::Message(message)));
                (LinkState::Failed, None)
            }
        };
        if let (Some(slot_state), Some(slot_stream)) = (self.states.get_mut(index), self.streams.get_mut(index)) {
            *slot_state = state;
            if stream.is_some() {
                *slot_stream = stream;
            }
        }
        Ok(())
    }

    fn state(&mut self, connection: ConnectionId) -> LinkState {
        self.states.get(slot(connection)).copied().unwrap_or(LinkState::Closed)
    }

    fn send(&mut self, connection: ConnectionId, bytes: &[u8]) -> Result<(), Self::Error> {
        let open = self
            .streams
            .get_mut(slot(connection))
            .and_then(Option::as_mut)
            .ok_or(embedded_io::ErrorKind::NotConnected)?;
        // Written whole, blocking, within the write limit: a request or a
        // input message is small, and the socket drains it at once.
        open.write_all(bytes).map_err(|_| embedded_io::ErrorKind::Other)
    }

    /// Nothing waits here: a send is written whole, blocking, to the
    /// kernel, and what the kernel has not sent cannot be read without
    /// unsafe code (REQ-SEC-015), so moves are never held for this
    /// transport (REQ-RCH-035; an rdm to-do).
    fn unsent(&mut self, _: ConnectionId) -> usize {
        0
    }

    fn receive(&mut self, connection: ConnectionId, buffer: &mut [u8]) -> Result<Received, Self::Error> {
        let open = self.streams.get_mut(slot(connection)).and_then(Option::as_mut);
        Ok(open.map_or(Received::Closed, |open| receive(open, buffer)))
    }

    fn close(&mut self, connection: ConnectionId) {
        let index = slot(connection);
        if let Some(open) = self.streams.get_mut(index).and_then(Option::take) {
            open.shutdown();
        }
        if let Some(state) = self.states.get_mut(index) {
            *state = LinkState::Closed;
        }
    }
}

/// Random bytes from the TLS library's system generator.
pub(super) struct SystemRandom;

impl RandomSource for SystemRandom {
    fn fill(&mut self, bytes: &mut [u8]) {
        let provider = rustls::crypto::ring::default_provider();
        let _ = provider.secure_random.fill(bytes);
    }
}

#[cfg(test)]
mod tests;
