//! The fetcher process of web and live mode (REQ-SEC-018): it holds the
//! network and runs the web module's state machines over a std transport
//! with TLS, sending packets and page bytes to the decoder process on
//! standard output (decision 26 of IMPLEMENTATION-LOG.md).

use crate::pipe::{self, Frame, Notice};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant as StdInstant};
use tellytxt_core::policy::{ipv4_blocked, ipv6_blocked};
use tellytxt_core::types::{Instant, PageAddress, SubCode};
use tellytxt_web::port::{ConnectionId, LinkState, Pump, RandomSource, Received, Security, Transport};
use tellytxt_web::url::IpAddress;

/// Time allowed to connect and complete the TLS handshake.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the loop sleeps when nothing happened.
const IDLE: Duration = Duration::from_millis(10);
/// Bytes read from a connection at a time.
const READ_CHUNK: usize = 16 * 1024;

enum Stream {
    Plain(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Stream {
    fn socket(&self) -> &TcpStream {
        match self {
            Self::Plain(socket) => socket,
            Self::Tls(stream) => &stream.sock,
        }
    }
}

/// A blocking-connect, non-blocking-read transport over std sockets and
/// rustls, checking every resolved address against the policy before
/// connecting (decision 27).
pub struct StdTransport {
    streams: [Option<Stream>; 3],
    states: [LinkState; 3],
    tls: Arc<rustls::ClientConfig>,
}

fn slot(connection: ConnectionId) -> usize {
    match connection {
        ConnectionId::Fetch => 0,
        ConnectionId::Push => 1,
        ConnectionId::Live => 2,
    }
}

fn web_address(address: IpAddr) -> IpAddress {
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
    /// A transport trusting the Mozilla root certificates.
    #[must_use]
    pub fn new() -> Self {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
        Self { streams: [None, None, None], states: [LinkState::Closed; 3], tls: Arc::new(tls) }
    }

    fn open(&self, host: &str, port: u16, security: Security) -> Result<(Stream, IpAddr), String> {
        let candidates: Vec<SocketAddr> =
            (host, port).to_socket_addrs().map_err(|error| format!("{host}: {error}"))?.collect();
        let allowed = candidates
            .iter()
            .find(|address| !blocked(address.ip()))
            .ok_or_else(|| format!("{host}: every address is on a blocked local network (REQ-SEC-006)"))?;
        let socket =
            TcpStream::connect_timeout(allowed, CONNECT_TIMEOUT).map_err(|error| format!("{host}: {error}"))?;
        socket.set_nodelay(true).map_err(|error| error.to_string())?;
        socket.set_read_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| error.to_string())?;
        let stream = match security {
            Security::Plain => Stream::Plain(socket),
            Security::Tls => {
                let name = rustls::pki_types::ServerName::try_from(host.to_string())
                    .map_err(|error| format!("{host}: {error}"))?;
                let connection =
                    rustls::ClientConnection::new(self.tls.clone(), name).map_err(|error| error.to_string())?;
                let mut stream = rustls::StreamOwned::new(connection, socket);
                while stream.conn.is_handshaking() {
                    stream.conn.complete_io(&mut stream.sock).map_err(|error| format!("{host}: TLS: {error}"))?;
                }
                Stream::Tls(Box::new(stream))
            }
        };
        stream.socket().set_nonblocking(true).map_err(|error| error.to_string())?;
        Ok((stream, allowed.ip()))
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
        match self.open(host, port, security) {
            Ok((stream, peer)) => {
                self.streams[index] = Some(stream);
                self.states[index] = LinkState::Open(web_address(peer));
            }
            Err(message) => {
                let _ = send(&Frame::Notice(Notice::Message(message)));
                self.states[index] = LinkState::Failed;
            }
        }
        Ok(())
    }

    fn state(&mut self, connection: ConnectionId) -> LinkState {
        self.states[slot(connection)]
    }

    fn send(&mut self, connection: ConnectionId, bytes: &[u8]) -> Result<(), Self::Error> {
        let stream = self.streams[slot(connection)].as_mut().ok_or(embedded_io::ErrorKind::NotConnected)?;
        let blocking = |stream: &Stream, on: bool| stream.socket().set_nonblocking(!on);
        blocking(stream, true).map_err(|_| embedded_io::ErrorKind::Other)?;
        let result = match stream {
            Stream::Plain(socket) => socket.write_all(bytes).and_then(|()| socket.flush()),
            Stream::Tls(tls) => tls.write_all(bytes).and_then(|()| tls.flush()),
        };
        blocking(stream, false).map_err(|_| embedded_io::ErrorKind::Other)?;
        result.map_err(|_| embedded_io::ErrorKind::Other)
    }

    fn receive(&mut self, connection: ConnectionId, buffer: &mut [u8]) -> Result<Received, Self::Error> {
        let Some(stream) = self.streams[slot(connection)].as_mut() else { return Ok(Received::Closed) };
        let result = match stream {
            Stream::Plain(socket) => socket.read(buffer),
            Stream::Tls(tls) => tls.read(buffer),
        };
        match result {
            Ok(0) => Ok(Received::Closed),
            Ok(count) => Ok(Received::Data(count)),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(Received::Nothing),
            Err(_) => Ok(Received::Closed),
        }
    }

    fn close(&mut self, connection: ConnectionId) {
        let index = slot(connection);
        if let Some(stream) = self.streams[index].take() {
            let _ = stream.socket().shutdown(std::net::Shutdown::Both);
        }
        self.states[index] = LinkState::Closed;
    }
}

/// Random bytes from the TLS library's system generator.
struct SystemRandom;

impl RandomSource for SystemRandom {
    fn fill(&mut self, bytes: &mut [u8]) {
        let provider = rustls::crypto::ring::default_provider();
        let _ = provider.secure_random.fill(bytes);
    }
}

fn send(frame: &Frame) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    pipe::write(&mut out, frame)
}

/// Reads page requests from the decoder process.
fn requests() -> Receiver<(PageAddress, Option<SubCode>)> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        while let Ok(Some(Frame::Request(mpp, sub))) = pipe::read(&mut input) {
            if let Some(address) = PageAddress::from_mpp(mpp) {
                let subcode = (sub != u16::MAX).then(|| SubCode::new(sub));
                if sender.send((address, subcode)).is_err() {
                    break;
                }
            }
        }
    });
    receiver
}

/// Runs the fetcher for `address` until the decoder process goes away.
pub fn run(address: &str, no_sandbox: bool) -> Result<(), (u8, String)> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let requests = requests();
    // The fetcher may resolve names, which reads /etc, and use the network.
    crate::sandbox::apply(
        crate::sandbox::Policy { writable_directory: None, network: true, readable: &[std::path::Path::new("/etc")] },
        no_sandbox,
    )
    .map_err(|message| (crate::exit::INPUT, message))?;
    if let Some(name) = address.strip_prefix("zxnet:") {
        let channel = tellytxt_web::zxnet::Channel::from_short_name(name)
            .map_err(|error| (crate::exit::USAGE, format!("unknown live feed: {error:?}")))?;
        run_zxnet(&channel, &requests);
    } else if address.starts_with("wss://") {
        let channel = tellytxt_web::zxnet::Channel::from_identifier(address)
            .map_err(|error| (crate::exit::USAGE, format!("unknown live feed: {error:?}")))?;
        run_zxnet(&channel, &requests);
    } else {
        run_web(address, &requests);
    }
    Ok(())
}

fn clock(start: StdInstant) -> Instant {
    Instant(u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX))
}

fn run_zxnet(channel: &tellytxt_web::zxnet::Channel, requests: &Receiver<(PageAddress, Option<SubCode>)>) {
    use tellytxt_web::zxnet::{LiveEvent, LiveInput, LiveOutput, LiveStorage, ZxnetFeed};
    let mut storage = Box::new(LiveStorage::default());
    let mut feed = ZxnetFeed::new(*channel, storage.buffers(), SystemRandom);
    let mut pump = Pump::new(StdTransport::new());
    let start = StdInstant::now();
    let mut buffer = vec![0u8; READ_CHUNK];
    let broken = std::cell::Cell::new(false);
    let sink = |output: LiveOutput<'_>, pump: &mut Pump<StdTransport>| {
        if let Some(net) = output.net() {
            pump.execute(net);
            return;
        }
        let result = match output {
            LiveOutput::Packet(packet) => send(&Frame::Packet(packet.bytes)),
            LiveOutput::Event(LiveEvent::InitialPage { page, .. }) => {
                send(&Frame::Notice(Notice::InitialPage(page.mpp())))
            }
            LiveOutput::Event(LiveEvent::GaveUp) => {
                send(&Frame::Notice(Notice::Message("live feed: gave up reconnecting".into())))
            }
            LiveOutput::Event(LiveEvent::WrongName) => {
                send(&Frame::Notice(Notice::Message("live feed: wrong service name".into())))
            }
            _ => Ok(()),
        };
        if result.is_err() {
            broken.set(true);
        }
    };
    feed.handle(clock(start), LiveInput::Start, &mut |output| sink(output, &mut pump));
    let mut next_tick = clock(start);
    while !broken.get() {
        let mut busy = false;
        if let Some(input) = pump.poll(ConnectionId::Live, &mut buffer) {
            busy = true;
            feed.handle(clock(start), LiveInput::Net(input), &mut |output| sink(output, &mut pump));
        }
        while let Ok((page, subcode)) = requests.try_recv() {
            busy = true;
            feed.handle(clock(start), LiveInput::Request { page, subcode }, &mut |output| sink(output, &mut pump));
        }
        if clock(start) >= next_tick {
            let mut timer = None;
            feed.handle(clock(start), LiveInput::Tick, &mut |output| {
                if let LiveOutput::Timer(at) = output {
                    timer = Some(at);
                }
                sink(output, &mut pump);
            });
            next_tick = timer.unwrap_or(Instant(clock(start).0 + 200_000));
        }
        if !busy {
            std::thread::sleep(IDLE);
        }
    }
}

fn run_web(address: &str, requests: &Receiver<(PageAddress, Option<SubCode>)>) {
    use tellytxt_web::agent::{Event, Input, Output, Storage, UserAgent};
    use tellytxt_web::http::MediaType;
    /// Cookie slots and bytes of the jar: REQ-WEB-006's default total.
    const COOKIE_SLOTS: usize = 100;
    const COOKIE_BYTES: usize = 32 * 1024;
    let mut storage = Box::new(Storage::default());
    let mut agent: Box<UserAgent<'_, SystemRandom, COOKIE_SLOTS, COOKIE_BYTES>> =
        Box::new(UserAgent::new(tellytxt_web::agent::Config::default(), storage.buffers(), SystemRandom));
    let mut pump = Pump::new(StdTransport::new());
    let start = StdInstant::now();
    let mut buffer = vec![0u8; READ_CHUNK];
    let broken = std::cell::Cell::new(false);
    let mut next_tick: Option<Instant> = Some(Instant(0));
    let sink = |output: Output<'_>, pump: &mut Pump<StdTransport>, next_tick: &mut Option<Instant>| {
        if let Some(net) = output.net() {
            pump.execute(net);
            return;
        }
        let result = match output {
            Output::Timer { at } => {
                *next_tick = Some(next_tick.map_or(at, |current| current.min(at)));
                Ok(())
            }
            Output::PageBytes(page) => match page.media {
                MediaType::T42 => send(&Frame::T42(page.secure, page.bytes.to_vec())),
                _ => send(&Frame::Tti(page.secure, page.bytes.to_vec())),
            },
            Output::Event(Event::ServiceReady { initial_page, secure, .. }) => {
                let _ = if secure { Ok(()) } else { send(&Frame::Notice(Notice::Unencrypted)) };
                send(&Frame::Notice(Notice::InitialPage(initial_page.mpp())))
            }
            Output::Event(Event::Unencrypted { .. }) => send(&Frame::Notice(Notice::Unencrypted)),
            Output::Event(
                event @ (Event::DescriptorRejected(_)
                | Event::DescriptorFailed(_)
                | Event::Blocked { .. }
                | Event::DowngradeRefused { .. }
                | Event::FetchFailed { .. }),
            ) => send(&Frame::Notice(Notice::Message(format!("{event:?}")))),
            _ => Ok(()),
        };
        if result.is_err() {
            broken.set(true);
        }
    };
    agent.handle(clock(start), Input::OpenService(address), &mut |output| sink(output, &mut pump, &mut next_tick));
    while !broken.get() {
        let mut busy = false;
        for connection in [ConnectionId::Fetch, ConnectionId::Push] {
            if let Some(input) = pump.poll(connection, &mut buffer) {
                busy = true;
                agent.handle(clock(start), Input::Net(input), &mut |output| sink(output, &mut pump, &mut next_tick));
            }
        }
        while let Ok((page, subcode)) = requests.try_recv() {
            busy = true;
            agent.handle(clock(start), Input::FetchPage { page, subcode }, &mut |output| {
                sink(output, &mut pump, &mut next_tick);
            });
        }
        if next_tick.is_some_and(|at| clock(start) >= at) {
            next_tick = None;
            agent.handle(clock(start), Input::Tick, &mut |output| sink(output, &mut pump, &mut next_tick));
        }
        if !busy {
            std::thread::sleep(IDLE);
        }
    }
}
