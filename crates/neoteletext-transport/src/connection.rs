//! An open connection's bytes: the reader thread that blocks on its socket
//! and wakes the owner when bytes arrive, what the owner takes from it,
//! plain or through TLS, and the blocking writes.

use neoteletext_web::port::Received;
use std::collections::VecDeque;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::time::{Duration, Instant};

/// Bytes a reader thread takes from its socket at a time, where the owner
/// has no smaller bound of its own.
pub const SOCKET_READ: usize = 16 * 1024;
/// Reads a reader thread may be ahead of its owner before it waits: with
/// the size of a read, what one connection holds in memory at most, beyond
/// the operating system's own buffer.
pub(crate) const READS_AHEAD: usize = 4;

/// One open connection. A thread of its own reads the socket, blocking, so
/// its owner waits on nothing but its own wakes.
pub struct Connection {
    /// For writing and for shutting down; the reader thread reads a clone.
    socket: TcpStream,
    tls: Option<Box<rustls::ClientConnection>>,
    /// What the reader thread read; the channel ends when the socket does.
    chunks: Receiver<Vec<u8>>,
    /// Bytes taken from `chunks` and not yet passed on.
    pending: VecDeque<u8>,
}

/// A connection's reader thread: passes on what the socket gives, waking
/// the owner each time, until the socket ends or nothing takes the bytes.
fn read_socket(mut socket: TcpStream, chunks: &SyncSender<Vec<u8>>, read_bytes: usize, wake: &impl Fn()) {
    let mut buffer = vec![0_u8; read_bytes];
    loop {
        let count = match socket.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let Some(bytes) = buffer.get(..count) else { break };
        if chunks.send(bytes.to_vec()).is_err() {
            return;
        }
        wake();
    }
    // The owner sees the end as the channel's: this thread's sender is the
    // only one.
    wake();
}

/// Whether bytes from the reader thread are pending.
enum Refill {
    Ready,
    Waiting,
    Ended,
}

/// Takes the reader thread's next bytes when none are pending, waiting for
/// them until `until`. No moment, or one already past, looks without
/// waiting.
fn refill(pending: &mut VecDeque<u8>, chunks: &Receiver<Vec<u8>>, until: Option<Instant>) -> Refill {
    if !pending.is_empty() {
        return Refill::Ready;
    }
    let left = until.map_or(Duration::ZERO, |until| until.saturating_duration_since(Instant::now()));
    let taken = if left.is_zero() {
        match chunks.try_recv() {
            Ok(bytes) => bytes,
            Err(TryRecvError::Empty) => return Refill::Waiting,
            Err(TryRecvError::Disconnected) => return Refill::Ended,
        }
    } else {
        match chunks.recv_timeout(left) {
            Ok(bytes) => bytes,
            Err(RecvTimeoutError::Timeout) => return Refill::Waiting,
            Err(RecvTimeoutError::Disconnected) => return Refill::Ended,
        }
    };
    pending.extend(taken);
    Refill::Ready
}

/// Writes whatever records rustls has ready, blocking, within the socket's
/// write limit.
///
/// # Errors
/// When the socket fails.
pub fn write_records(tls: &mut rustls::ClientConnection, mut socket: &TcpStream) -> io::Result<()> {
    while tls.wants_write() {
        tls.write_tls(&mut socket)?;
    }
    socket.flush()
}

impl Connection {
    /// Starts the connection's reader thread, reading `read_bytes` at a
    /// time and calling `wake` when bytes arrive and when the socket ends.
    /// `first` is what the socket's next bytes come after: the host's, when
    /// the proxy's answer brought them on a plain connection.
    ///
    /// From here the reader thread waits on the socket for as long as it is
    /// open; the owner's own timers decide when it has been silent too long,
    /// and [`Connection::shutdown`] ends the wait.
    ///
    /// # Errors
    /// When the socket refuses the setting or cannot be cloned.
    pub fn start(
        socket: TcpStream,
        tls: Option<Box<rustls::ClientConnection>>,
        first: Vec<u8>,
        read_bytes: usize,
        wake: impl Fn() + Send + 'static,
    ) -> io::Result<Self> {
        socket.set_read_timeout(None)?;
        let reader = socket.try_clone()?;
        let (sender, chunks) = mpsc::sync_channel(READS_AHEAD);
        std::thread::spawn(move || read_socket(reader, &sender, read_bytes, &wake));
        Ok(Self { socket, tls, chunks, pending: first.into() })
    }

    /// Takes what has arrived into `buffer`, decrypted when the connection
    /// is TLS, waiting for it until `until`; with no moment, or one already
    /// past, it looks without waiting. `answer` writes the records TLS has
    /// ready: an alert or a key update may need an answer. A server's
    /// `close_notify` and the socket's end are [`Received::Closed`].
    ///
    /// # Errors
    /// When TLS fails or `answer` does.
    pub fn take(
        &mut self,
        buffer: &mut [u8],
        until: Option<Instant>,
        mut answer: impl FnMut(&mut rustls::ClientConnection, &TcpStream) -> io::Result<()>,
    ) -> io::Result<Received> {
        let Self { socket, tls, chunks, pending } = self;
        let Some(tls) = tls.as_deref_mut() else {
            return Ok(match refill(pending, chunks, until) {
                Refill::Waiting => Received::Nothing,
                Refill::Ended => Received::Closed,
                Refill::Ready => pending.read(buffer).map_or(Received::Closed, Received::Data),
            });
        };
        loop {
            match tls.reader().read(buffer) {
                // The server's close_notify.
                Ok(0) => return Ok(Received::Closed),
                Ok(count) => return Ok(Received::Data(count)),
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
            // Nothing decrypted waits: more records, if the socket has any.
            match refill(pending, chunks, until) {
                Refill::Ready => {}
                Refill::Waiting => return Ok(Received::Nothing),
                Refill::Ended => return Ok(Received::Closed),
            }
            // `pending` is not empty, which to rustls would be the end; a
            // read that takes nothing from it would never finish.
            if tls.read_tls(pending)? == 0 {
                return Ok(Received::Closed);
            }
            tls.process_new_packets().map_err(|error| io::Error::new(ErrorKind::InvalidData, error))?;
            answer(tls, socket)?;
        }
    }

    /// Writes all of `bytes`, encrypted over TLS, blocking, within the
    /// socket's write limit.
    ///
    /// # Errors
    /// When the connection breaks.
    pub fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut socket = &self.socket;
        match self.tls.as_deref_mut() {
            None => socket.write_all(bytes).and_then(|()| socket.flush()),
            Some(tls) => tls.writer().write_all(bytes).and_then(|()| write_records(tls, socket)),
        }
    }

    /// The TLS side of the connection, for an owner that writes its records
    /// itself.
    pub fn tls_mut(&mut self) -> Option<&mut rustls::ClientConnection> {
        self.tls.as_deref_mut()
    }

    /// Ends the reader thread's wait; the bytes it still holds are dropped
    /// with the channel.
    pub fn shutdown(&self) {
        let _ = self.socket.shutdown(Shutdown::Both);
    }
}
