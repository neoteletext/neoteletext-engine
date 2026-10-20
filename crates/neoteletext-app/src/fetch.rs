//! The fetcher process of web and live mode (REQ-SEC-018): it holds the
//! network and runs the web module's state machines over a std transport
//! with TLS, sending packets and page bytes to the decoder process on
//! standard output (decision 26 of IMPLEMENTATION-LOG.md).

use crate::pipe::{self, Frame, Notice};
use neoteletext_core::policy::{ipv4_blocked, ipv6_blocked};
use neoteletext_core::types::{Instant, PageAddress, SubCode};
use neoteletext_web::port::{ConnectionId, LinkState, Pump, RandomSource, Received, Security, Transport};
use neoteletext_web::url::IpAddress;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError};
use std::time::{Duration, Instant as StdInstant};

/// Time allowed to connect and complete the TLS handshake; a write is held
/// to it too.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Directories the fetcher may read to resolve names (decision 28).
const RESOLVER_PATHS: [&str; 6] = ["/etc", "/run/systemd/resolve", "/usr/lib", "/lib", "/usr/lib64", "/lib64"];
/// Bytes read from a connection at a time, by its reader thread and by the
/// state machines.
const READ_CHUNK: usize = 16 * 1024;
/// Reads a connection's reader thread may be ahead of the loop before it
/// waits: with `READ_CHUNK`, what one connection holds in memory at most,
/// beyond the operating system's own buffer.
const READS_AHEAD: usize = 4;

/// What ends the fetcher loop's wait. It sleeps until one of these comes,
/// or until the state machine's next timer, never on a fixed interval, so a
/// key goes and a pushed page comes back at once (REQ-INT-008, decision
/// E.97).
pub enum Wake {
    /// A request from the decoder process.
    Request(Frame),
    /// A connection has bytes, or has ended.
    Network,
    /// The decoder process closed the pipe.
    Ended,
}

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

/// A blocking-connect transport over std sockets and rustls, checking every
/// resolved address against the policy before connecting (decision 27),
/// whose connections each wake the loop when bytes arrive.
pub struct StdTransport {
    streams: [Option<Connection>; 3],
    states: [LinkState; 3],
    tls: Arc<rustls::ClientConfig>,
    allowed_local: Vec<IpAddr>,
    wake: Sender<Wake>,
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
    /// A transport trusting the Mozilla root certificates, waking the loop
    /// through `wake`.
    #[must_use]
    pub fn new(allowed_local: &[IpAddr], wake: Sender<Wake>) -> Self {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
        Self {
            streams: [None, None, None],
            states: [LinkState::Closed; 3],
            tls: Arc::new(tls),
            allowed_local: allowed_local.to_vec(),
            wake,
        }
    }

    fn open(&self, host: &str, port: u16, security: Security) -> Result<(Connection, IpAddr), String> {
        let candidates: Vec<SocketAddr> =
            (host, port).to_socket_addrs().map_err(|error| format!("{host}: {error}"))?.collect();
        let allowed = candidates
            .iter()
            .find(|address| !blocked(address.ip()) || self.allowed_local.contains(&address.ip()))
            .ok_or_else(|| format!("{host}: every address is on a blocked local network (REQ-SEC-006)"))?;
        let mut socket =
            TcpStream::connect_timeout(allowed, CONNECT_TIMEOUT).map_err(|error| format!("{host}: {error}"))?;
        socket.set_nodelay(true).map_err(|error| error.to_string())?;
        socket.set_read_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| error.to_string())?;
        socket.set_write_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| error.to_string())?;
        let tls = match security {
            Security::Plain => None,
            Security::Tls => {
                let name = rustls::pki_types::ServerName::try_from(host.to_string())
                    .map_err(|error| format!("{host}: {error}"))?;
                let mut connection =
                    rustls::ClientConnection::new(self.tls.clone(), name).map_err(|error| error.to_string())?;
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
        Ok((Connection { socket, tls, incoming, pending: VecDeque::new() }, allowed.ip()))
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
        let open = self.streams[slot(connection)].as_mut().ok_or(embedded_io::ErrorKind::NotConnected)?;
        // Written whole, blocking, within the write limit: a request or a
        // input message is small, and the socket drains it at once.
        let mut socket = &open.socket;
        let result = match open.tls.as_deref_mut() {
            None => socket.write_all(bytes).and_then(|()| socket.flush()),
            Some(tls) => tls.writer().write_all(bytes).and_then(|()| write_records(tls, socket)),
        };
        result.map_err(|_| embedded_io::ErrorKind::Other)
    }

    fn receive(&mut self, connection: ConnectionId, buffer: &mut [u8]) -> Result<Received, Self::Error> {
        Ok(self.streams[slot(connection)].as_mut().map_or(Received::Closed, |open| open.receive(buffer)))
    }

    fn close(&mut self, connection: ConnectionId) {
        let index = slot(connection);
        // Ends the reader thread's wait; the bytes it still holds are
        // dropped with the channel.
        if let Some(open) = self.streams[index].take() {
            let _ = open.socket.shutdown(std::net::Shutdown::Both);
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

/// Reads the requests sent to the fetcher: page requests, submissions,
/// confirmations, links to other services and keys. Anything else is
/// dropped. The loop is woken by each, and by the pipe's end.
fn requests(wake: Sender<Wake>) {
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        while let Ok(Some(frame)) = pipe::read(&mut input) {
            let wanted = matches!(
                frame,
                Frame::Request(..) | Frame::Submit(_) | Frame::Confirm(_) | Frame::Follow(..) | Frame::Input(_)
            );
            if wanted && wake.send(Wake::Request(frame)).is_err() {
                return;
            }
        }
        let _ = wake.send(Wake::Ended);
    });
}

/// The loop's wakes: the requests not yet acted on, and whether the
/// decoder process has gone.
struct Wakes {
    receiver: Receiver<Wake>,
    requests: VecDeque<Frame>,
    ended: bool,
    /// How many times the loop has slept, for the tests to count.
    #[cfg(test)]
    waits: Arc<std::sync::atomic::AtomicUsize>,
}

impl Wakes {
    fn new(receiver: Receiver<Wake>) -> Self {
        Self {
            receiver,
            requests: VecDeque::new(),
            ended: false,
            #[cfg(test)]
            waits: Arc::default(),
        }
    }

    fn take(&mut self, wake: Wake) {
        match wake {
            Wake::Request(frame) => self.requests.push_back(frame),
            // The sockets are looked at on the loop's next turn.
            Wake::Network => {}
            Wake::Ended => self.ended = true,
        }
    }

    /// The next request that has arrived, without waiting.
    fn next_request(&mut self) -> Option<Frame> {
        while let Ok(wake) = self.receiver.try_recv() {
            self.take(wake);
        }
        self.requests.pop_front()
    }

    /// Sleeps until a request, a socket or the pipe's end wakes the loop,
    /// or until `deadline`, the state machine's next timer.
    fn wait(&mut self, now: Instant, deadline: Option<Instant>) {
        #[cfg(test)]
        self.waits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let woken = match deadline {
            Some(at) => match self.receiver.recv_timeout(Duration::from_micros(at.0.saturating_sub(now.0))) {
                Ok(wake) => Some(wake),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => Some(Wake::Ended),
            },
            None => Some(self.receiver.recv().unwrap_or(Wake::Ended)),
        };
        if let Some(wake) = woken {
            self.take(wake);
        }
    }
}

fn page_request(mpp: u16, sub: u16) -> Option<(PageAddress, Option<SubCode>)> {
    PageAddress::from_mpp(mpp).map(|address| (address, (sub != u16::MAX).then(|| SubCode::new(sub))))
}

/// The prefix of a fetcher address that names a channel's folder rather
/// than a descriptor (REQ-WEB-018).
pub const CHANNEL_PREFIX: &str = "channel:";

/// Runs the fetcher for `address` until the decoder process goes away.
pub fn run(address: &str, no_sandbox: bool, allow_local: &[IpAddr]) -> Result<(), (u8, String)> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (wake, receiver) = mpsc::channel();
    requests(wake.clone());
    let mut wakes = Wakes::new(receiver);
    // Name resolution reads the resolver configuration and loads the C
    // library's name-service modules (decision 28 of IMPLEMENTATION-LOG.md).
    let readable: Vec<&std::path::Path> =
        RESOLVER_PATHS.iter().map(std::path::Path::new).filter(|path| path.exists()).collect();
    crate::sandbox::apply(
        crate::sandbox::Policy { writable_directory: None, network: true, readable: &readable },
        no_sandbox,
    )
    .map_err(|message| (crate::exit::INPUT, message))?;
    if let Some(name) = address.strip_prefix("zxnet:") {
        let channel = neoteletext_web::zxnet::Channel::from_short_name(name)
            .map_err(|error| (crate::exit::USAGE, format!("unknown live feed: {error:?}")))?;
        run_zxnet(&channel, &mut wakes, wake, allow_local);
    } else if address.starts_with("wss://") {
        let channel = neoteletext_web::zxnet::Channel::from_identifier(address)
            .map_err(|error| (crate::exit::USAGE, format!("unknown live feed: {error:?}")))?;
        run_zxnet(&channel, &mut wakes, wake, allow_local);
    } else {
        run_web(address, &mut wakes, wake, allow_local);
    }
    Ok(())
}

fn clock(start: StdInstant) -> Instant {
    Instant(u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX))
}

fn run_zxnet(channel: &neoteletext_web::zxnet::Channel, wakes: &mut Wakes, wake: Sender<Wake>, allow_local: &[IpAddr]) {
    live_loop(channel, wakes, StdTransport::new(allow_local, wake), allow_local, &mut |frame| send(frame));
}

/// The live feed's loop over any transport, sending what it receives
/// through `emit` until that fails or the decoder process goes.
fn live_loop<T: Transport>(
    channel: &neoteletext_web::zxnet::Channel,
    wakes: &mut Wakes,
    transport: T,
    allow_local: &[IpAddr],
    emit: &mut dyn FnMut(&Frame) -> std::io::Result<()>,
) {
    use neoteletext_web::zxnet::{LiveEvent, LiveInput, LiveOutput, LiveStorage, ZxnetFeed};
    let mut storage = Box::new(LiveStorage::default());
    let mut feed = ZxnetFeed::new(*channel, storage.buffers(), SystemRandom);
    if let Some(&address) = allow_local.first() {
        feed.allow_local(web_address(address));
    }
    let mut pump = Pump::new(transport);
    let start = StdInstant::now();
    let mut buffer = vec![0u8; READ_CHUNK];
    let broken = std::cell::Cell::new(false);
    // The feed says, after every input, when it next needs a tick, and
    // says nothing once it has stopped: the latest it said is the deadline.
    let deadline: std::cell::Cell<Option<Instant>> = std::cell::Cell::new(None);
    let mut sink = |output: LiveOutput<'_>, pump: &mut Pump<T>| {
        if let Some(net) = output.net() {
            pump.execute(net);
            return;
        }
        let result = match output {
            LiveOutput::Timer(at) => {
                deadline.set(Some(at));
                Ok(())
            }
            LiveOutput::Packet(packet) => emit(&Frame::Packet(packet.bytes)),
            LiveOutput::Event(LiveEvent::InitialPage { page, .. }) => {
                emit(&Frame::Notice(Notice::InitialPage(page.mpp())))
            }
            LiveOutput::Event(LiveEvent::GaveUp) => {
                emit(&Frame::Notice(Notice::Message("live feed: gave up reconnecting".into())))
            }
            LiveOutput::Event(LiveEvent::WrongName) => {
                emit(&Frame::Notice(Notice::Message("live feed: wrong service name".into())))
            }
            _ => Ok(()),
        };
        if result.is_err() {
            broken.set(true);
        }
    };
    let mut step = |input: LiveInput<'_>, pump: &mut Pump<T>| {
        deadline.set(None);
        feed.handle(clock(start), input, &mut |output| sink(output, pump));
        deadline.get()
    };
    let mut next_tick = step(LiveInput::Start, &mut pump);
    while !broken.get() && !wakes.ended {
        let mut busy = false;
        if let Some(input) = pump.poll(ConnectionId::Live, &mut buffer) {
            busy = true;
            next_tick = step(LiveInput::Net(input), &mut pump);
        }
        while let Some(frame) = wakes.next_request() {
            busy = true;
            if let Frame::Request(mpp, sub) = frame
                && let Some((page, subcode)) = page_request(mpp, sub)
            {
                next_tick = step(LiveInput::Request { page, subcode }, &mut pump);
            }
        }
        if next_tick.is_some_and(|at| clock(start) >= at) {
            // What a tick asks for, a reconnection above all, is taken up
            // on the next turn, not after another wait.
            busy = true;
            next_tick = step(LiveInput::Tick, &mut pump);
        }
        if !busy {
            wakes.wait(clock(start), next_tick);
        }
    }
}

// One event loop; splitting it would scatter the agent's sink and state.
#[allow(clippy::too_many_lines)]
fn run_web(address: &str, wakes: &mut Wakes, wake: Sender<Wake>, allow_local: &[IpAddr]) {
    use neoteletext_web::agent::{Event, Failure, Input, Output, Source as Came, Storage, UserAgent};
    use neoteletext_web::form::{FormMethod, FormSubmission};
    use neoteletext_web::http::MediaType;
    /// Cookie slots and bytes of the jar: REQ-WEB-006's default total.
    const COOKIE_SLOTS: usize = 100;
    const COOKIE_BYTES: usize = 32 * 1024;
    let mut storage = Box::new(Storage::default());
    let mut config = neoteletext_web::agent::Config::default();
    let channel = address.strip_prefix(CHANNEL_PREFIX);
    // An address naming a channel's folder keeps redirects inside it
    // (REQ-WEB-021).
    config.redirects_within_channel = channel.is_some();
    for (slot, &address) in config.allowed_local.iter_mut().zip(allow_local) {
        *slot = Some(web_address(address));
    }
    let mut agent: Box<UserAgent<'_, SystemRandom, COOKIE_SLOTS, COOKIE_BYTES>> =
        Box::new(UserAgent::new(config, storage.buffers(), SystemRandom));
    let mut pump = Pump::new(StdTransport::new(allow_local, wake));
    let start = StdInstant::now();
    let mut buffer = vec![0u8; READ_CHUNK];
    let broken = std::cell::Cell::new(false);
    let service_ready = std::cell::Cell::new(false);
    // The push connection opened (true) or was lost (false), for the
    // decoder to know when keys may go (area INT).
    let push_state: std::cell::Cell<Option<bool>> = std::cell::Cell::new(None);
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
            Output::Event(Event::RedirectToOtherChannel { location }) => {
                send(&Frame::Notice(Notice::RedirectToOtherChannel(location.to_string())))
            }
            Output::Event(Event::BundleFailed(failure)) => {
                send(&Frame::Notice(Notice::Message(format!("index.t42 was not used: {failure:?}"))))
            }
            Output::Event(Event::Unencrypted { .. }) => send(&Frame::Notice(Notice::Unencrypted)),
            Output::Event(Event::PushConnected(_)) => {
                push_state.set(Some(true));
                Ok(())
            }
            // A bad message keeps the connection and says so with no wait.
            Output::Event(Event::PushLost { retry_in_us, .. }) if retry_in_us > 0 => {
                push_state.set(Some(false));
                Ok(())
            }
            Output::Event(Event::ConfirmSubmission { host, plain_http, first_to_origin, .. }) => {
                send(&Frame::Notice(Notice::Confirm(host.to_string(), plain_http, first_to_origin)))
            }
            Output::Event(Event::PageCurrent { page, .. }) => send(&Frame::Notice(Notice::Current(page.mpp()))),
            Output::Event(Event::FetchFailed { page, failure: Failure::Network | Failure::Timeout, .. }) => {
                send(&Frame::Notice(Notice::Offline(page.mpp())))
            }
            Output::Event(Event::SubmissionSucceeded { .. }) => send(&Frame::Notice(Notice::Submitted(true))),
            Output::Event(Event::SubmissionFailed { .. }) => send(&Frame::Notice(Notice::Submitted(false))),
            Output::Event(Event::FetchFailed { page, failure, .. }) => send(&Frame::Notice(Notice::Message(format!(
                "Page {} not used: {failure:?}",
                String::from_utf8_lossy(&page.ascii())
            )))),
            Output::Event(
                event @ (Event::DescriptorRejected(_)
                | Event::DescriptorFailed(_)
                | Event::Blocked { .. }
                | Event::DowngradeRefused { .. }
                | Event::SubmissionRefused(_)
                | Event::LinkUnknown),
            ) => send(&Frame::Notice(Notice::Message(format!("{event:?}")))),
            _ => Ok(()),
        };
        if result.is_err() {
            broken.set(true);
        }
    };
    let open = channel.map_or(Input::OpenService(address), Input::OpenChannel);
    agent.handle(clock(start), open, &mut |output| sink(output, &mut pump, &mut next_tick));
    while !broken.get() && !wakes.ended {
        let mut busy = false;
        for connection in [ConnectionId::Fetch, ConnectionId::Push] {
            if let Some(input) = pump.poll(connection, &mut buffer) {
                busy = true;
                agent.handle(clock(start), Input::Net(input), &mut |output| sink(output, &mut pump, &mut next_tick));
            }
        }
        if let Some(open) = push_state.take()
            && agent.descriptor().is_some_and(neoteletext_web::descriptor::Descriptor::input)
            && send(&Frame::Notice(Notice::Input(open))).is_err()
        {
            break;
        }
        if service_ready.replace(false) {
            // Tell the decoder about the (new) service's linked services
            // before any of its pages (Appendix E).
            let names: Vec<String> = agent
                .descriptor()
                .map(|descriptor| {
                    descriptor
                        .services()
                        .map(|service| String::from_utf8_lossy(service.short_name.as_bytes()).into_owned())
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
            if send(&Frame::Notice(Notice::Service(names))).is_err()
                || send(&Frame::Notice(Notice::Aliases(aliases))).is_err()
            {
                break;
            }
        }
        while let Some(frame) = wakes.next_request() {
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
                Frame::Input(message) => agent.handle(now, Input::Send(&message), &mut out),
                Frame::Follow(service, mpp, sub) => {
                    if let Some((page, subcode)) = page_request(mpp, sub) {
                        agent.handle(now, Input::FollowLink { service: &service, page, subcode }, &mut out);
                    }
                }
                _ => {}
            }
        }
        if next_tick.is_some_and(|at| clock(start) >= at) {
            // What a tick asks for, a reconnection above all, is taken up
            // on the next turn, not after another wait.
            busy = true;
            next_tick = None;
            agent.handle(clock(start), Input::Tick, &mut |output| sink(output, &mut pump, &mut next_tick));
        }
        if !busy {
            wakes.wait(clock(start), next_tick);
        }
    }
}

#[cfg(test)]
mod tests {
    //! The live feed's loop against a server played in memory: the feed's
    //! own deadlines decide when the loop wakes (decision E.97).
    #![allow(clippy::unwrap_used, clippy::expect_used, reason = "a test")]

    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::Ordering;

    /// How long the tests give the loop.
    const LIMIT: Duration = Duration::from_secs(12);
    /// A public address for the server, so no local-network rule applies.
    const PEER: IpAddress = IpAddress::V4([93, 184, 216, 34]);
    /// RFC 6455 section 5.2: a final text frame.
    const TEXT_FRAME: u8 = 0x81;

    /// What the server does once the WebSocket is open.
    #[derive(Clone, Copy, PartialEq)]
    enum Script {
        /// Says nothing more.
        Silent,
        /// Refuses the service name, which stops the feed.
        WrongName,
    }

    #[derive(Default)]
    struct Server {
        open: bool,
        to_client: VecDeque<u8>,
        opened_at: Option<StdInstant>,
        closed_at: Option<StdInstant>,
    }

    struct Fake {
        server: Arc<Mutex<Server>>,
        script: Script,
        wake: Sender<Wake>,
    }

    impl embedded_io::ErrorType for Fake {
        type Error = embedded_io::ErrorKind;
    }

    impl Transport for Fake {
        fn connect(&mut self, _: ConnectionId, _: &str, _: u16, _: Security) -> Result<(), Self::Error> {
            self.server.lock().unwrap().open = true;
            Ok(())
        }

        fn state(&mut self, _: ConnectionId) -> LinkState {
            if self.server.lock().unwrap().open { LinkState::Open(PEER) } else { LinkState::Closed }
        }

        fn send(&mut self, _: ConnectionId, bytes: &[u8]) -> Result<(), Self::Error> {
            let mut server = self.server.lock().unwrap();
            if server.opened_at.is_some() {
                return Ok(());
            }
            // The upgrade request: answer it, then follow the script.
            let request = String::from_utf8_lossy(bytes);
            let key = request
                .lines()
                .find_map(|line| {
                    line.split_once(':').filter(|(name, _)| name.eq_ignore_ascii_case("sec-websocket-key"))
                })
                .map(|(_, value)| value.trim().to_owned())
                .unwrap();
            let accept = neoteletext_web::ws::accept_for(key.as_bytes());
            let head = format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
                String::from_utf8_lossy(&accept)
            );
            server.to_client.extend(head.bytes());
            if self.script == Script::WrongName {
                let message = br#"["wrongname"]"#;
                server.to_client.extend([TEXT_FRAME, u8::try_from(message.len()).unwrap()]);
                server.to_client.extend(message);
            }
            server.opened_at = Some(StdInstant::now());
            let _ = self.wake.send(Wake::Network);
            Ok(())
        }

        fn receive(&mut self, _: ConnectionId, buffer: &mut [u8]) -> Result<Received, Self::Error> {
            let mut server = self.server.lock().unwrap();
            if !server.open {
                return Ok(Received::Closed);
            }
            let count = server.to_client.read(buffer).unwrap_or(0);
            Ok(if count == 0 { Received::Nothing } else { Received::Data(count) })
        }

        fn close(&mut self, _: ConnectionId) {
            let mut server = self.server.lock().unwrap();
            server.open = false;
            // The slot is closed once before anything opens; only a close
            // of the open WebSocket counts.
            if server.opened_at.is_some() {
                server.closed_at.get_or_insert_with(StdInstant::now);
            }
        }
    }

    /// Runs the loop against the script on a thread; returns the server,
    /// the loop's count of sleeps, and the sender that ends it.
    fn start(script: Script) -> (Arc<Mutex<Server>>, Arc<std::sync::atomic::AtomicUsize>, Sender<Wake>) {
        let (wake, receiver) = mpsc::channel();
        let mut wakes = Wakes::new(receiver);
        let waits = wakes.waits.clone();
        let server = Arc::new(Mutex::new(Server::default()));
        let fake = Fake { server: server.clone(), script, wake: wake.clone() };
        let channel = neoteletext_web::zxnet::Channel::from_address("wss://feed.example/live", "test").unwrap();
        std::thread::spawn(move || live_loop(&channel, &mut wakes, fake, &[], &mut |_| Ok(())));
        (server, waits, wake)
    }

    /// Waits for the server to see the connection close.
    fn closed(server: &Mutex<Server>) -> (StdInstant, StdInstant) {
        let started = StdInstant::now();
        loop {
            let state = server.lock().unwrap();
            if let (Some(opened), Some(closed)) = (state.opened_at, state.closed_at) {
                return (opened, closed);
            }
            drop(state);
            assert!(started.elapsed() < LIMIT, "the connection was never closed");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_silent_feed_is_dropped_when_its_inactivity_deadline_passes() {
        // The feed's inactivity deadline, 5 s after the handshake, was lost
        // behind the handshake's 30 s one: the silence went unnoticed, and
        // the first keepalive went, about 25 s late.
        let (server, _, end) = start(Script::Silent);
        let (opened, closed) = closed(&server);
        let silence = closed.duration_since(opened);
        let inactivity = Duration::from_micros(neoteletext_web::consts::ZXNET_INACTIVITY_US);
        assert!(silence >= inactivity && silence < inactivity * 3 / 2, "dropped after {silence:?}");
        let _ = end.send(Wake::Ended);
    }

    #[test]
    fn a_stopped_feed_sleeps_until_something_happens() {
        // A feed that has stopped asks for no tick; the loop slept 200 ms
        // and looked again, five times a second, for ever, once its stale
        // handshake deadline had passed: run 32 s after the stop on 21
        // October 2026, the old loop woke 5 times in the second counted.
        let (server, waits, end) = start(Script::WrongName);
        closed(&server);
        std::thread::sleep(Duration::from_millis(100));
        let before = waits.load(Ordering::Relaxed);
        std::thread::sleep(Duration::from_secs(1));
        let woken = waits.load(Ordering::Relaxed) - before;
        assert!(woken <= 1, "the stopped loop woke {woken} times in a second");
        let _ = end.send(Wake::Ended);
    }
}
