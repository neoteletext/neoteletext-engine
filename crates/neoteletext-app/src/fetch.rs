//! The fetcher process of web and live mode (REQ-SEC-018): it holds the
//! network and runs the web module's state machines over a std transport
//! with TLS, sending packets and page bytes to the decoder process on
//! standard output (decision 26 of IMPLEMENTATION-LOG.md).

use crate::pipe::{self, Frame, Notice};
use neoteletext_core::policy::{ipv4_blocked, ipv6_blocked};
use neoteletext_core::types::{Instant, PageAddress, SubCode};
use neoteletext_web::port::{ConnectionId, LinkState, Pump, RandomSource, Received, Security, Transport};
use neoteletext_web::url::IpAddress;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant as StdInstant};

/// Time allowed to connect and complete the TLS handshake.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the loop sleeps when nothing happened.
const IDLE: Duration = Duration::from_millis(10);
/// Directories the fetcher may read to resolve names (decision 28).
const RESOLVER_PATHS: [&str; 6] = ["/etc", "/run/systemd/resolve", "/usr/lib", "/lib", "/usr/lib64", "/lib64"];
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
    allowed_local: Vec<IpAddr>,
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
    pub fn new(allowed_local: &[IpAddr]) -> Self {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
        Self {
            streams: [None, None, None],
            states: [LinkState::Closed; 3],
            tls: Arc::new(tls),
            allowed_local: allowed_local.to_vec(),
        }
    }

    fn open(&self, host: &str, port: u16, security: Security) -> Result<(Stream, IpAddr), String> {
        let candidates: Vec<SocketAddr> =
            (host, port).to_socket_addrs().map_err(|error| format!("{host}: {error}"))?.collect();
        let allowed = candidates
            .iter()
            .find(|address| !blocked(address.ip()) || self.allowed_local.contains(&address.ip()))
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

/// Reads the decoder process's requests: page requests, submissions,
/// confirmations and links to other services. Anything else is dropped.
fn requests() -> Receiver<Frame> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        while let Ok(Some(frame)) = pipe::read(&mut input) {
            let wanted = matches!(frame, Frame::Request(..) | Frame::Submit(_) | Frame::Confirm(_) | Frame::Follow(..));
            if wanted && sender.send(frame).is_err() {
                break;
            }
        }
    });
    receiver
}

fn page_request(mpp: u16, sub: u16) -> Option<(PageAddress, Option<SubCode>)> {
    PageAddress::from_mpp(mpp).map(|address| (address, (sub != u16::MAX).then(|| SubCode::new(sub))))
}

/// The prefix of a fetcher address that names a web-native site rather
/// than a descriptor: the browser's form (`docs/web-native-concept.md`
/// section 2).
pub const SITE_PREFIX: &str = "site:";

/// Runs the fetcher for `address` until the decoder process goes away.
/// `sandboxed_by_parent` says the process that started this one has
/// already confined it (the browser on macOS, `docs/desktop-browser.md`
/// section 12.3), so no sandbox of its own is applied.
pub fn run(
    address: &str,
    no_sandbox: bool,
    sandboxed_by_parent: bool,
    allow_local: &[IpAddr],
) -> Result<(), (u8, String)> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let requests = requests();
    if !sandboxed_by_parent {
        // Name resolution reads the resolver configuration and loads the C
        // library's name-service modules (decision 28 of
        // IMPLEMENTATION-LOG.md).
        let readable: Vec<&std::path::Path> =
            RESOLVER_PATHS.iter().map(std::path::Path::new).filter(|path| path.exists()).collect();
        crate::sandbox::apply(
            crate::sandbox::Policy { writable_directory: None, network: true, readable: &readable },
            no_sandbox,
        )
        .map_err(|message| (crate::exit::INPUT, message))?;
    }
    if let Some(name) = address.strip_prefix("zxnet:") {
        let channel = neoteletext_web::zxnet::Channel::from_short_name(name)
            .map_err(|error| (crate::exit::USAGE, format!("unknown live feed: {error:?}")))?;
        run_zxnet(&channel, &requests, allow_local);
    } else if address.starts_with("wss://") {
        let channel = neoteletext_web::zxnet::Channel::from_identifier(address)
            .map_err(|error| (crate::exit::USAGE, format!("unknown live feed: {error:?}")))?;
        run_zxnet(&channel, &requests, allow_local);
    } else {
        run_web(address, &requests, allow_local);
    }
    Ok(())
}

fn clock(start: StdInstant) -> Instant {
    Instant(u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX))
}

fn run_zxnet(channel: &neoteletext_web::zxnet::Channel, requests: &Receiver<Frame>, allow_local: &[IpAddr]) {
    use neoteletext_web::zxnet::{LiveEvent, LiveInput, LiveOutput, LiveStorage, ZxnetFeed};
    let mut storage = Box::new(LiveStorage::default());
    let mut feed = ZxnetFeed::new(*channel, storage.buffers(), SystemRandom);
    if let Some(&address) = allow_local.first() {
        feed.allow_local(web_address(address));
    }
    let mut pump = Pump::new(StdTransport::new(allow_local));
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
        while let Ok(frame) = requests.try_recv() {
            busy = true;
            if let Frame::Request(mpp, sub) = frame
                && let Some((page, subcode)) = page_request(mpp, sub)
            {
                feed.handle(clock(start), LiveInput::Request { page, subcode }, &mut |output| sink(output, &mut pump));
            }
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

// One event loop; splitting it would scatter the agent's sink and state.
#[allow(clippy::too_many_lines)]
fn run_web(address: &str, requests: &Receiver<Frame>, allow_local: &[IpAddr]) {
    use neoteletext_web::agent::{Event, Failure, Input, Output, Source as Came, Storage, UserAgent};
    use neoteletext_web::form::{FormMethod, FormSubmission};
    use neoteletext_web::http::MediaType;
    /// Cookie slots and bytes of the jar: REQ-WEB-006's default total.
    const COOKIE_SLOTS: usize = 100;
    const COOKIE_BYTES: usize = 32 * 1024;
    let mut storage = Box::new(Storage::default());
    let mut config = neoteletext_web::agent::Config::default();
    let site = address.strip_prefix(SITE_PREFIX);
    // A browser confirms every other site before showing it.
    config.redirects_within_site = site.is_some();
    for (slot, &address) in config.allowed_local.iter_mut().zip(allow_local) {
        *slot = Some(web_address(address));
    }
    let mut agent: Box<UserAgent<'_, SystemRandom, COOKIE_SLOTS, COOKIE_BYTES>> =
        Box::new(UserAgent::new(config, storage.buffers(), SystemRandom));
    let mut pump = Pump::new(StdTransport::new(allow_local));
    let start = StdInstant::now();
    let mut buffer = vec![0u8; READ_CHUNK];
    let broken = std::cell::Cell::new(false);
    let service_ready = std::cell::Cell::new(false);
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
            Output::PageBytes(page) => {
                let data = pipe::PageData {
                    secure: page.secure,
                    origin: match page.source {
                        Came::Page { .. } => pipe::Origin::Page,
                        Came::Push => pipe::Origin::Push,
                        Came::Form => pipe::Origin::Form,
                        Came::FrontDoor => pipe::Origin::FrontDoor,
                    },
                    url: page.url.to_string(),
                    bytes: page.bytes.to_vec(),
                };
                match page.media {
                    MediaType::T42 => send(&Frame::T42(data)),
                    _ => send(&Frame::Tti(data)),
                }
            }
            Output::Event(Event::ServiceReady { initial_page, name, .. }) => {
                service_ready.set(true);
                send(&Frame::Notice(Notice::ServiceName(name.to_string())))
                    .and_then(|()| send(&Frame::Notice(Notice::InitialPage(initial_page.mpp()))))
            }
            Output::Event(Event::RedirectToOtherSite { location }) => {
                send(&Frame::Notice(Notice::RedirectToOtherSite(location.to_string())))
            }
            Output::Event(Event::Unencrypted { .. }) => send(&Frame::Notice(Notice::Unencrypted)),
            Output::Event(Event::ConfirmSubmission { host, plain_http, first_to_origin, .. }) => {
                send(&Frame::Notice(Notice::Confirm(host.to_string(), plain_http, first_to_origin)))
            }
            Output::Event(Event::PageCurrent { page, .. }) => send(&Frame::Notice(Notice::Current(page.mpp()))),
            Output::Event(Event::FetchFailed { page, failure: Failure::Network | Failure::Timeout, .. }) => {
                send(&Frame::Notice(Notice::Offline(page.mpp())))
            }
            Output::Event(Event::SubmissionSucceeded { .. }) => send(&Frame::Notice(Notice::Submitted(true))),
            Output::Event(Event::SubmissionFailed { .. }) => send(&Frame::Notice(Notice::Submitted(false))),
            Output::Event(
                event @ (Event::DescriptorRejected(_)
                | Event::DescriptorFailed(_)
                | Event::Blocked { .. }
                | Event::DowngradeRefused { .. }
                | Event::FetchFailed { .. }
                | Event::SubmissionRefused(_)
                | Event::LinkUnknown),
            ) => send(&Frame::Notice(Notice::Message(format!("{event:?}")))),
            _ => Ok(()),
        };
        if result.is_err() {
            broken.set(true);
        }
    };
    let open = site.map_or(Input::OpenService(address), Input::OpenSite);
    agent.handle(clock(start), open, &mut |output| sink(output, &mut pump, &mut next_tick));
    while !broken.get() {
        let mut busy = false;
        for connection in [ConnectionId::Fetch, ConnectionId::Push] {
            if let Some(input) = pump.poll(connection, &mut buffer) {
                busy = true;
                agent.handle(clock(start), Input::Net(input), &mut |output| sink(output, &mut pump, &mut next_tick));
            }
        }
        if service_ready.replace(false) {
            // Tell the decoder about the (new) service's linked services
            // before any of its pages (Appendix E).
            let sites: Vec<(String, String)> = agent
                .descriptor()
                .map(|descriptor| {
                    descriptor
                        .services()
                        .map(|service| {
                            (
                                String::from_utf8_lossy(service.short_name.as_bytes()).into_owned(),
                                descriptor.service_address(service).to_string(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let aliases: Vec<(u16, String)> = agent
                .descriptor()
                .map(|descriptor| {
                    descriptor
                        .aliases()
                        .map(|alias| (alias.page.mpp(), descriptor.alias_target(alias).to_string()))
                        .collect()
                })
                .unwrap_or_default();
            // A service that pushes keeps its pages current itself.
            let refresh = agent
                .descriptor()
                .filter(|descriptor| descriptor.push().is_none())
                .and_then(|descriptor| descriptor.refresh_seconds);
            if let Some(seconds) = refresh
                && send(&Frame::Notice(Notice::Refresh(seconds))).is_err()
            {
                break;
            }
            let names = sites.iter().map(|(name, _)| name.clone()).collect();
            if send(&Frame::Notice(Notice::Service(names))).is_err()
                || send(&Frame::Notice(Notice::LinkedSites(sites))).is_err()
                || send(&Frame::Notice(Notice::Aliases(aliases))).is_err()
            {
                break;
            }
        }
        while let Ok(frame) = requests.try_recv() {
            busy = true;
            let now = clock(start);
            let mut out = |output: Output<'_>| sink(output, &mut pump, &mut next_tick);
            match frame {
                Frame::Request(mpp, sub) => {
                    if let Some((page, subcode)) = page_request(mpp, sub) {
                        agent.handle(now, Input::FetchPage { page, subcode }, &mut out);
                    }
                }
                Frame::Submit(submission) => {
                    let fields: Vec<(&str, &str)> =
                        submission.fields.iter().map(|(name, value)| (name.as_str(), value.as_str())).collect();
                    let form = FormSubmission {
                        page_url: &submission.page_url,
                        method: if submission.post { FormMethod::Post } else { FormMethod::Get },
                        target: &submission.target,
                        fields: &fields,
                        token: submission.token.as_deref(),
                        action: submission.action.as_deref(),
                    };
                    agent.handle(now, Input::Submit(form), &mut out);
                }
                Frame::Confirm(yes) => agent.handle(now, Input::ConfirmSubmission(yes), &mut out),
                Frame::Follow(service, mpp, sub) => {
                    if let Some((page, subcode)) = page_request(mpp, sub) {
                        agent.handle(now, Input::FollowLink { service: &service, page, subcode }, &mut out);
                    }
                }
                _ => {}
            }
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
