//! Connections to the relay: HTTP/1.1 requests and a WebSocket, plain or
//! over TLS (rustls, Mozilla's roots). HTTP heads and WebSocket frames are
//! read and written by the engine's own bounded code (`neoteletext_web`'s
//! `http` and `ws`). Every HTTP read has a time limit; a WebSocket is read
//! by a thread of its own, which wakes its owner as bytes arrive.

use std::sync::{Arc, OnceLock, mpsc};
use std::time::{Duration, Instant};

use neoteletext_web::port::RandomSource;
use neoteletext_web::url::Url;

use crate::Error;

mod http;
mod stream;
mod websocket;

pub use http::{MAX_BODY, request};
pub use stream::Stream;
pub use websocket::WebSocket;

/// How long a connection and its TLS handshake may take.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long an HTTP answer may take.
pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(15);
/// The longest response head read.
pub(super) const MAX_HEAD: usize = 16 * 1024;
/// Bytes read from the socket at a time.
pub(super) const READ_CHUNK: usize = 4096;

/// Random bytes for WebSocket masks and keys.
pub struct OsRandom;

impl RandomSource for OsRandom {
    fn fill(&mut self, bytes: &mut [u8]) {
        // A mask needs no secrecy; on the impossible failure it stays zero.
        let _ = crate::random(bytes);
    }
}

pub(super) fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots: rustls::RootCertStore = webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect();
            Arc::new(rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth())
        })
        .clone()
}

/// Parses an `http`, `https`, `ws` or `wss` address.
///
/// # Errors
/// Any other address.
pub fn parse(address: &str) -> Result<Url, Error> {
    Url::parse(address).map_err(|_| Error::Address)
}

/// What a WebSocket delivered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Incoming {
    /// A text message.
    Text(String),
    /// A binary message.
    Binary(Vec<u8>),
    /// The peer closed, with this code when it gave one.
    Closed(Option<u16>),
}

/// Why a wait woke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wake {
    /// A socket's reader thread passed on bytes, or saw its socket end.
    Network,
    /// Someone holding a [`Waker`] asked the wait to end.
    Asked,
}

/// Ends a wait from another thread (decision E.106): a [`Screen`]'s wait
/// returns at once when its waker wakes it, or when bytes arrive on a
/// [`WebSocket`] connected with it.
///
/// [`Screen`]: crate::screen::Screen
#[derive(Clone, Debug)]
pub struct Waker {
    sender: mpsc::Sender<Wake>,
    kind: Wake,
}

impl Waker {
    pub(crate) const fn new(sender: mpsc::Sender<Wake>, kind: Wake) -> Self {
        Self { sender, kind }
    }

    /// Ends the wait now, or the next one if none is waiting (REQ-INT-009).
    /// Waking a wait whose owner has gone does nothing.
    pub fn wake(&self) {
        let _ = self.sender.send(self.kind);
    }
}

/// The longest wait taken as itself: a wait the clock cannot add, such as
/// `Duration::MAX` for "until something happens", is a day, after which
/// the caller only looks again.
const LONGEST_WAIT: Duration = Duration::from_hours(24);

/// The moment `wait` from now.
pub(crate) fn later(wait: Duration) -> Instant {
    let now = Instant::now();
    now.checked_add(wait).or_else(|| now.checked_add(LONGEST_WAIT)).unwrap_or(now)
}
