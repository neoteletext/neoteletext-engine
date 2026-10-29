//! The std transport: sockets, TLS and the proxy, each connection read by
//! a thread of its own that wakes the loop when bytes arrive (decision 27,
//! REQ-WEB-032).

use super::decoder_link::send;
use super::wakes::Wake;
use crate::pipe::{Frame, Notice};
use neoteletext_core::policy::{ipv4_blocked, ipv6_blocked};
use neoteletext_web::port::{ConnectionId, LinkState, RandomSource, Received, Security, Transport};
use neoteletext_web::proxy::Proxy;
use neoteletext_web::url::IpAddress;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
use std::time::Duration;

/// Time allowed to connect and complete the TLS handshake; a write is held
/// to it too.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Bytes read from a connection at a time, by its reader thread and by the
/// state machines.
pub(super) const READ_CHUNK: usize = 16 * 1024;
/// Reads a connection's reader thread may be ahead of the loop before it
/// waits: with `READ_CHUNK`, what one connection holds in memory at most,
/// beyond the operating system's own buffer.
const READS_AHEAD: usize = 4;

/// One open connection. A thread of its own reads the socket, blocking,
/// and wakes the loop when bytes arrive.
struct Connection {
    /// For writing and for shutting down; the reader thread reads a clone.
    socket: TcpStream,
    tls: Option<Box<rustls::ClientConnection>>,
    /// What the reader thread read; the channel ends when the socket does.
    incoming: Receiver<Vec<u8>>,
    /// Bytes taken from `incoming` and not yet passed on.
    pending: VecDeque<u8>,
}

/// The TLS configuration of every connection (REQ-RCH-014): one, shared,
/// so a reconnection to a host resumes the session of the last connection
/// where the server allows, from rustls's own in-memory store of sessions,
/// saving a round trip; and no early data, which could replay an input
/// message. (TCP Fast Open is never used: `std` does not offer it.)
fn tls_config(roots: rustls::RootCertStore) -> rustls::ClientConfig {
    let mut config = rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
    config.resumption = rustls::client::Resumption::default();
    config.enable_early_data = false;
    config
}

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

pub(super) fn web_address(address: IpAddr) -> IpAddress {
    match address {
        IpAddr::V4(v4) => IpAddress::V4(v4.octets()),
        IpAddr::V6(v6) => IpAddress::V6(v6.segments()),
    }
}

fn blocked(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => ipv4_blocked(v4.octets()),
        IpAddr::V6(v6) => ipv6_blocked(v6.segments()),
    }
}

impl StdTransport {
    /// A transport trusting the Mozilla root certificates, waking the loop
    /// through `wake`.
    #[must_use]
    pub fn new(allowed_local: &[IpAddr], wake: Sender<Wake>, proxy: Option<Proxy>) -> Self {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = tls_config(roots);
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
        let mut early = Vec::new();
        let socket = if let Some(proxy) = &self.proxy {
            // The proxy's address may be local: the user chose it.
            let address = (proxy.host.as_str(), proxy.port)
                .to_socket_addrs()
                .map_err(|error| format!("{}: {error}", proxy.host))?
                .next()
                .ok_or_else(|| format!("{}: no address", proxy.host))?;
            let mut socket = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)
                .map_err(|error| format!("{}: {error}", proxy.host))?;
            socket.set_read_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| error.to_string())?;
            socket.set_write_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| error.to_string())?;
            early = proxy.open_tunnel(&mut socket, host, port)?;
            socket
        } else {
            let candidates: Vec<SocketAddr> =
                (host, port).to_socket_addrs().map_err(|error| format!("{host}: {error}"))?.collect();
            let allowed = candidates
                .iter()
                .find(|address| !blocked(address.ip()) || self.allowed_local.contains(&address.ip()))
                .ok_or_else(|| format!("{host}: every address is on a blocked local network (REQ-SEC-006)"))?;
            let socket =
                TcpStream::connect_timeout(allowed, CONNECT_TIMEOUT).map_err(|error| format!("{host}: {error}"))?;
            socket.set_read_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| error.to_string())?;
            socket.set_write_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| error.to_string())?;
            socket
        };
        socket.set_nodelay(true).map_err(|error| error.to_string())?;
        let peer = socket.peer_addr().map_err(|error| error.to_string())?.ip();
        Ok((socket, peer, early))
    }

    fn open(&self, host: &str, port: u16, security: Security) -> Result<(Connection, IpAddr), String> {
        let (mut socket, peer, early) = self.reach(host, port)?;
        let mut pending = VecDeque::new();
        let tls = match security {
            Security::Plain => {
                pending.extend(early);
                None
            }
            Security::Tls => {
                let name = rustls::pki_types::ServerName::try_from(host.to_string())
                    .map_err(|error| format!("{host}: {error}"))?;
                let mut connection =
                    rustls::ClientConnection::new(self.tls.clone(), name).map_err(|error| error.to_string())?;
                // What came through the tunnel with the proxy's answer is
                // the server's, read before the socket's next bytes.
                let mut records = early.as_slice();
                while !records.is_empty() {
                    let taken = connection.read_tls(&mut records).map_err(|error| format!("{host}: TLS: {error}"))?;
                    connection.process_new_packets().map_err(|error| format!("{host}: TLS: {error}"))?;
                    if taken == 0 {
                        break;
                    }
                }
                while connection.is_handshaking() {
                    connection.complete_io(&mut socket).map_err(|error| format!("{host}: TLS: {error}"))?;
                }
                Some(Box::new(connection))
            }
        };
        // From here the reader thread waits on the socket for as long as it
        // is open; the state machines' own timers decide when it has been
        // silent too long, and closing it ends the wait.
        socket.set_read_timeout(None).map_err(|error| error.to_string())?;
        let reader = socket.try_clone().map_err(|error| error.to_string())?;
        let (chunks, incoming) = mpsc::sync_channel(READS_AHEAD);
        let wake = self.wake.clone();
        std::thread::spawn(move || read_socket(reader, &chunks, &wake));
        Ok((Connection { socket, tls, incoming, pending }, peer))
    }
}

/// A connection's reader thread: passes on what the socket gives until it
/// ends, or until the connection is closed and nothing takes the bytes.
fn read_socket(mut socket: TcpStream, chunks: &SyncSender<Vec<u8>>, wake: &Sender<Wake>) {
    let mut buffer = vec![0u8; READ_CHUNK];
    loop {
        let count = match socket.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let Some(bytes) = buffer.get(..count) else { break };
        if chunks.send(bytes.to_vec()).is_err() {
            return;
        }
        let _ = wake.send(Wake::Network);
    }
    // The loop sees the end as the channel's: this thread's sender is the
    // only one.
    let _ = wake.send(Wake::Network);
}

/// Whether bytes from the reader thread are pending.
enum Refill {
    Ready,
    Waiting,
    Ended,
}

/// Takes the reader thread's next bytes when none are pending.
fn refill(pending: &mut VecDeque<u8>, incoming: &Receiver<Vec<u8>>) -> Refill {
    if !pending.is_empty() {
        return Refill::Ready;
    }
    match incoming.try_recv() {
        Ok(bytes) => {
            pending.extend(bytes);
            Refill::Ready
        }
        Err(TryRecvError::Empty) => Refill::Waiting,
        Err(TryRecvError::Disconnected) => Refill::Ended,
    }
}

/// Writes whatever records rustls has ready, blocking, within the write
/// limit.
fn write_records(tls: &mut rustls::ClientConnection, mut socket: &TcpStream) -> std::io::Result<()> {
    while tls.wants_write() {
        tls.write_tls(&mut socket)?;
    }
    socket.flush()
}

impl Connection {
    fn receive(&mut self, buffer: &mut [u8]) -> Received {
        let Self { socket, tls, incoming, pending } = self;
        let Some(tls) = tls.as_deref_mut() else {
            return match refill(pending, incoming) {
                Refill::Waiting => Received::Nothing,
                Refill::Ended => Received::Closed,
                Refill::Ready => pending.read(buffer).map_or(Received::Closed, Received::Data),
            };
        };
        loop {
            match tls.reader().read(buffer) {
                // The server's close_notify.
                Ok(0) => return Received::Closed,
                Ok(count) => return Received::Data(count),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => return Received::Closed,
            }
            // Nothing decrypted waits: more records, if the socket has any.
            match refill(pending, incoming) {
                Refill::Ready => {}
                Refill::Waiting => return Received::Nothing,
                Refill::Ended => return Received::Closed,
            }
            // `pending` is not empty, which to rustls would be the end; a
            // read that takes nothing from it would never finish.
            if !matches!(tls.read_tls(pending), Ok(taken) if taken > 0) || tls.process_new_packets().is_err() {
                return Received::Closed;
            }
            // An alert or a key update may need an answer.
            if write_records(tls, socket).is_err() {
                return Received::Closed;
            }
        }
    }
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
        let mut socket = &open.socket;
        let result = match open.tls.as_deref_mut() {
            None => socket.write_all(bytes).and_then(|()| socket.flush()),
            Some(tls) => tls.writer().write_all(bytes).and_then(|()| write_records(tls, socket)),
        };
        result.map_err(|_| embedded_io::ErrorKind::Other)
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
        Ok(open.map_or(Received::Closed, |open| open.receive(buffer)))
    }

    fn close(&mut self, connection: ConnectionId) {
        let index = slot(connection);
        // Ends the reader thread's wait; the bytes it still holds are
        // dropped with the channel.
        if let Some(open) = self.streams.get_mut(index).and_then(Option::take) {
            let _ = open.socket.shutdown(std::net::Shutdown::Both);
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
