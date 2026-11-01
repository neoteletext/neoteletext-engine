//! A connection against peers played on this computer: the reader thread's
//! bound, the waits, the end of a socket, TLS both ways and through its
//! failures, and the tunnel.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "a test"
)]

use super::connection::READS_AHEAD;
use super::*;
use neoteletext_web::port::Received;
use neoteletext_web::proxy::{Proxy, ProxyKind};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// How long a test gives what should happen at once.
const LIMIT: Duration = Duration::from_secs(5);
/// The size of a read in the tests that do not care.
const READ_BYTES: usize = 64;
/// A wait that must be seen to last.
const SHORT_WAIT: Duration = Duration::from_millis(50);

/// Two ends of a connection on this computer: this side's, and the peer's.
fn socket_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (peer, _) = listener.accept().unwrap();
    (client, peer)
}

fn plain(socket: TcpStream, first: Vec<u8>) -> Connection {
    Connection::start(socket, None, first, READ_BYTES, || {}).unwrap()
}

/// Takes from `connection` until `wanted` bytes have come or it closes.
fn take_all(connection: &mut Connection, wanted: usize) -> Vec<u8> {
    let started = Instant::now();
    let mut seen = Vec::new();
    let mut buffer = [0_u8; READ_BYTES];
    while seen.len() < wanted && started.elapsed() < LIMIT {
        match connection.take(&mut buffer, Some(Instant::now() + LIMIT), write_records).unwrap() {
            Received::Data(count) => seen.extend_from_slice(&buffer[..count]),
            Received::Nothing => {}
            Received::Closed => break,
        }
    }
    seen
}

#[test]
fn the_tunnels_first_bytes_come_before_the_sockets() {
    let (socket, mut peer) = socket_pair();
    let mut connection = plain(socket, b"first".to_vec());
    peer.write_all(b"next").unwrap();
    assert_eq!(take_all(&mut connection, b"firstnext".len()), b"firstnext");
}

#[test]
fn no_moment_looks_without_waiting() {
    let (socket, _peer) = socket_pair();
    let mut connection = plain(socket, Vec::new());
    let started = Instant::now();
    assert_eq!(connection.take(&mut [0_u8; 8], None, write_records).unwrap(), Received::Nothing);
    assert!(started.elapsed() < SHORT_WAIT);
}

#[test]
fn a_wait_lasts_to_its_moment_and_reports_nothing() {
    let (socket, _peer) = socket_pair();
    let mut connection = plain(socket, Vec::new());
    let started = Instant::now();
    assert_eq!(connection.take(&mut [0_u8; 8], Some(started + SHORT_WAIT), write_records).unwrap(), Received::Nothing);
    assert!(started.elapsed() >= SHORT_WAIT);
}

#[test]
fn a_wait_ends_when_bytes_arrive() {
    let (socket, mut peer) = socket_pair();
    let mut connection = plain(socket, Vec::new());
    std::thread::spawn(move || {
        std::thread::sleep(SHORT_WAIT);
        peer.write_all(b"x").unwrap();
        // Until the owner has read: closing here would race its read.
        let _ = peer.read(&mut [0_u8; 1]);
    });
    let started = Instant::now();
    let mut buffer = [0_u8; 8];
    assert_eq!(connection.take(&mut buffer, Some(started + LIMIT), write_records).unwrap(), Received::Data(1));
    assert!(started.elapsed() < LIMIT);
}

#[test]
fn the_end_of_the_socket_closes_the_connection_and_wakes_its_owner() {
    let (socket, peer) = socket_pair();
    let wakes = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&wakes);
    let mut connection = Connection::start(socket, None, Vec::new(), READ_BYTES, move || {
        counted.fetch_add(1, Ordering::SeqCst);
    })
    .unwrap();
    drop(peer);
    assert_eq!(connection.take(&mut [0_u8; 8], Some(Instant::now() + LIMIT), write_records).unwrap(), Received::Closed);
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
}

/// A peer that sends far more than the owner takes fills the reader
/// thread's channel, and then the operating system's buffer: the thread
/// holds no more than its bound.
#[test]
fn the_reader_thread_stays_within_its_bound() {
    const READS: usize = 64;
    let (socket, mut peer) = socket_pair();
    let wakes = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&wakes);
    let mut connection = Connection::start(socket, None, Vec::new(), READ_BYTES, move || {
        counted.fetch_add(1, Ordering::SeqCst);
    })
    .unwrap();
    peer.write_all(&vec![0_u8; READS * READ_BYTES]).unwrap();
    let started = Instant::now();
    while wakes.load(Ordering::SeqCst) < READS_AHEAD && started.elapsed() < LIMIT {
        std::thread::yield_now();
    }
    std::thread::sleep(SHORT_WAIT);
    assert_eq!(wakes.load(Ordering::SeqCst), READS_AHEAD, "woken for each read taken, and none beyond the bound");
    assert_eq!(take_all(&mut connection, READS * READ_BYTES).len(), READS * READ_BYTES, "and nothing was lost");
}

#[test]
fn shutting_down_ends_the_reader_thread() {
    let (socket, _peer) = socket_pair();
    let mut connection = plain(socket, Vec::new());
    connection.shutdown();
    assert_eq!(connection.take(&mut [0_u8; 8], Some(Instant::now() + LIMIT), write_records).unwrap(), Received::Closed);
}

/// The test's server: a certificate for `localhost` made for this test
/// alone, valid to 2126, and its key.
struct Server {
    config: Arc<rustls::ServerConfig>,
    roots: rustls::RootCertStore,
}

fn server() -> Server {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    let certificate = CertificateDer::from(include_bytes!("testdata/localhost-certificate.der").to_vec());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(include_bytes!("testdata/localhost-key.der").to_vec()));
    let mut roots = rustls::RootCertStore::empty();
    roots.add(certificate.clone()).unwrap();
    let config =
        rustls::ServerConfig::builder().with_no_client_auth().with_single_cert(vec![certificate], key).unwrap();
    Server { config: Arc::new(config), roots }
}

/// Plays `script` on the server's side of a completed handshake; this side
/// starts TLS and hands back the connection.
fn tls_with<F>(server: Server, script: F) -> (Connection, std::thread::JoinHandle<()>)
where
    F: FnOnce(&mut rustls::ServerConnection, &mut TcpStream) + Send + 'static,
{
    let (mut socket, mut peer) = socket_pair();
    let config = server.config;
    let played = std::thread::spawn(move || {
        let mut connection = rustls::ServerConnection::new(config).unwrap();
        while connection.is_handshaking() {
            connection.complete_io(&mut peer).unwrap();
        }
        script(&mut connection, &mut peer);
    });
    let tls = Arc::new(client_config(server.roots));
    let started = start_tls(tls, "localhost", &[], &mut socket).unwrap();
    (Connection::start(socket, Some(Box::new(started)), Vec::new(), READ_BYTES, || {}).unwrap(), played)
}

/// Reads from the client until `wanted` bytes have been decrypted.
fn server_reads(connection: &mut rustls::ServerConnection, socket: &mut TcpStream, wanted: usize) -> Vec<u8> {
    let mut asked = vec![0_u8; wanted];
    let mut taken = 0;
    while taken < wanted {
        connection.complete_io(socket).unwrap();
        taken += connection
            .reader()
            .read(&mut asked[taken..])
            .or_else(|error| if error.kind() == ErrorKind::WouldBlock { Ok(0) } else { Err(error) })
            .unwrap();
    }
    asked
}

fn flush(connection: &mut rustls::ServerConnection, socket: &mut TcpStream) {
    while connection.wants_write() {
        connection.complete_io(socket).unwrap();
    }
}

/// More than a record: the pump takes several reads to give it back, and
/// a read of rustls takes less than the reader thread's chunk.
const LONG_ANSWER: usize = 64 * 1024;

#[test]
fn tls_carries_bytes_both_ways_and_a_close_notify_closes() {
    let (mut connection, played) = tls_with(server(), |tls, socket| {
        assert_eq!(server_reads(tls, socket, b"ping".len()), b"ping");
        tls.writer().write_all(&vec![7_u8; LONG_ANSWER]).unwrap();
        tls.send_close_notify();
        flush(tls, socket);
        let _ = socket.read(&mut [0_u8; 1]);
    });
    connection.write_all(b"ping").unwrap();
    assert_eq!(take_all(&mut connection, LONG_ANSWER + 1), vec![7_u8; LONG_ANSWER]);
    assert_eq!(connection.take(&mut [0_u8; 8], Some(Instant::now() + LIMIT), write_records).unwrap(), Received::Closed);
    connection.shutdown();
    played.join().unwrap();
}

/// The owner's way of writing records is the one used: the browser's goes
/// through its writer thread. A key update needs an answer.
#[test]
fn an_answer_tls_needs_goes_the_owners_way() {
    let (mut connection, played) = tls_with(server(), |tls, socket| {
        tls.refresh_traffic_keys().unwrap();
        tls.writer().write_all(b"after").unwrap();
        flush(tls, socket);
        let _ = socket.read(&mut [0_u8; 64]);
    });
    let mut answers = 0;
    let mut seen = Vec::new();
    let mut buffer = [0_u8; READ_BYTES];
    let started = Instant::now();
    while seen.len() < b"after".len() && started.elapsed() < LIMIT {
        let until = Some(Instant::now() + LIMIT);
        let took = connection.take(&mut buffer, until, |tls, socket| {
            answers += 1;
            write_records(tls, socket)
        });
        if let Received::Data(count) = took.unwrap() {
            seen.extend_from_slice(&buffer[..count]);
        }
    }
    assert_eq!(seen, b"after");
    assert!(answers > 0, "the records went through the owner's answer");
    connection.shutdown();
    played.join().unwrap();
}

#[test]
fn bytes_that_are_not_records_fail_tls_and_say_so() {
    let (mut connection, played) = tls_with(server(), |tls, socket| {
        // The ticket a TLS 1.3 server sends first, read before the garbage.
        flush(tls, socket);
        socket.write_all(&[0xFF_u8; READ_BYTES]).unwrap();
        let _ = socket.read(&mut [0_u8; READ_BYTES]);
    });
    let mut buffer = [0_u8; READ_BYTES];
    let until = Instant::now() + LIMIT;
    let failed = loop {
        match connection.take(&mut buffer, Some(until), write_records) {
            Ok(Received::Nothing) if Instant::now() < until => {}
            Ok(other) => panic!("not a failure: {other:?}"),
            Err(error) => break error,
        }
    };
    assert_eq!(failed.kind(), ErrorKind::InvalidData);
    assert!(failed.get_ref().is_some_and(<dyn std::error::Error + Send + Sync>::is::<rustls::Error>), "{failed}");
    connection.shutdown();
    played.join().unwrap();
}

#[test]
fn a_certificate_refused_fails_the_handshake() {
    let (mut socket, mut peer) = socket_pair();
    let config = server().config;
    let played = std::thread::spawn(move || {
        let mut connection = rustls::ServerConnection::new(config).unwrap();
        while connection.is_handshaking() {
            if connection.complete_io(&mut peer).is_err() {
                break;
            }
        }
    });
    let tls = Arc::new(client_config(mozilla_roots()));
    let failure = start_tls(tls, "localhost", &[], &mut socket).expect_err("Mozilla's roots do not hold the test's");
    let TlsFailure::Handshake(error) = failure else { panic!("{failure}") };
    let inner = error.get_ref().and_then(|inner| inner.downcast_ref::<rustls::Error>());
    assert!(matches!(inner, Some(rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer))));
    played.join().unwrap();
}

#[test]
fn a_host_no_certificate_can_name_is_refused_before_any_byte() {
    let (mut socket, mut peer) = socket_pair();
    let tls = Arc::new(client_config(mozilla_roots()));
    assert!(matches!(start_tls(tls, "not a name", &[], &mut socket), Err(TlsFailure::Name(_))));
    peer.set_nonblocking(true).unwrap();
    assert_eq!(peer.read(&mut [0_u8; 1]).unwrap_err().kind(), ErrorKind::WouldBlock);
}

#[test]
fn bytes_from_the_tunnel_are_read_as_the_servers() {
    let (mut socket, _peer) = socket_pair();
    let tls = Arc::new(client_config(mozilla_roots()));
    let failure = start_tls(tls, "localhost", &[0xFF; READ_BYTES], &mut socket).expect_err("not TLS");
    assert!(matches!(failure, TlsFailure::Records(_)), "{failure}");
}

/// REQ-WEB-035, REQ-RCH-014: sessions kept to resume, no early data.
#[test]
fn the_configuration_resumes_and_sends_no_early_data() {
    let config = client_config(mozilla_roots());
    assert!(!config.enable_early_data, "no 0-RTT: an input message could be replayed");
    assert!(format!("{:?}", config.resumption).contains("MemoryCache"), "{:?}", config.resumption);
}

/// REQ-WEB-032: a proxy on this computer, which the user chose, answers
/// with the host's first bytes in the same write.
fn played_proxy(answer: &'static [u8]) -> (Proxy, std::thread::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut heard = Vec::new();
        let mut byte = [0_u8; 1];
        while !heard.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            heard.push(byte[0]);
        }
        stream.write_all(answer).unwrap();
        heard
    });
    (Proxy { kind: ProxyKind::Http, host: "127.0.0.1".into(), port }, handle)
}

#[test]
fn a_tunnel_gives_the_hosts_first_bytes_with_its_socket() {
    let (proxy, played) = played_proxy(b"HTTP/1.1 200 Connection established\r\n\r\nHOST");
    let (socket, first) = open_tunnel(&proxy, "news.example", 443).unwrap();
    assert_eq!(first, b"HOST");
    assert_eq!(socket.read_timeout().unwrap(), Some(CONNECT_TIMEOUT));
    assert_eq!(socket.write_timeout().unwrap(), Some(CONNECT_TIMEOUT));
    assert!(played.join().unwrap().starts_with(b"CONNECT news.example:443 HTTP/1.1\r\n"));
}

#[test]
fn a_refusing_proxy_is_said_for_the_user() {
    let (proxy, played) = played_proxy(b"HTTP/1.1 403 Forbidden\r\n\r\n");
    let refused = open_tunnel(&proxy, "news.example", 443).unwrap_err();
    assert!(refused.contains("the proxy refused the connection to news.example"), "{refused}");
    played.join().unwrap();
}

#[test]
fn a_connect_failure_names_what_the_user_knows() {
    // A port just given up, which nothing listens on.
    let target = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let failure = connect_within_limit(&target, "my.proxy").unwrap_err();
    assert!(failure.starts_with("my.proxy: "), "{failure}");
}

#[test]
fn addresses_are_written_as_the_engine_writes_them() {
    use neoteletext_web::url::IpAddress;
    assert_eq!(web_address([127, 0, 0, 1].into()), IpAddress::V4([127, 0, 0, 1]));
    assert_eq!(web_address(std::net::Ipv6Addr::LOCALHOST.into()), IpAddress::V6([0, 0, 0, 0, 0, 0, 0, 1]));
    assert!(web_address([127, 0, 0, 1].into()).is_blocked());
}
