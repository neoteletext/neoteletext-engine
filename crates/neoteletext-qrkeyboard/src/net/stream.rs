//! A connection to the relay, plain or over TLS, directly or through the
//! proxy (REQ-WEB-032).

use super::{CONNECT_TIMEOUT, tls_config};
use crate::Error;
use neoteletext_web::proxy::Proxy;
use neoteletext_web::url::Url;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// A connection, plain or over TLS.
pub enum Stream {
    /// Plain TCP: only for a relay on this computer, for testing.
    Plain(TcpStream),
    /// TLS.
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Stream {
    /// Connects to `url`'s host, with TLS for `https` and `wss`.
    ///
    /// # Errors
    /// When the name does not resolve, the connection is refused or times
    /// out, or TLS fails.
    pub fn connect(url: &Url, proxy: Option<&Proxy>) -> Result<Self, Error> {
        let host = url.host_text();
        let socket = if let Some(proxy) = proxy {
            // Through the proxy the port's setting names (REQ-WEB-032),
            // whose address the user chose.
            let target = (proxy.host.as_str(), proxy.port)
                .to_socket_addrs()
                .map_err(|error| Error::Network(format!("{}: {error}", proxy.host)))?
                .next()
                .ok_or_else(|| Error::Network(format!("{}: no address", proxy.host)))?;
            let mut socket = TcpStream::connect_timeout(&target, CONNECT_TIMEOUT)
                .map_err(|error| Error::Network(format!("{}: {error}", proxy.host)))?;
            socket.set_read_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| Error::Network(error.to_string()))?;
            socket.set_write_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| Error::Network(error.to_string()))?;
            let early = proxy.open_tunnel(&mut socket, host, url.port()).map_err(Error::Network)?;
            // The relay speaks only when spoken to, over HTTP and TLS
            // alike: bytes of its before this side's first are a broken
            // protocol, not data to keep.
            if !early.is_empty() {
                return Err(Error::Protocol("the relay spoke through the proxy before it was asked"));
            }
            socket
        } else {
            let target = (host, url.port())
                .to_socket_addrs()
                .map_err(|error| Error::Network(format!("{host}: {error}")))?
                .next()
                .ok_or_else(|| Error::Network(format!("{host}: no address")))?;
            TcpStream::connect_timeout(&target, CONNECT_TIMEOUT)
                .map_err(|error| Error::Network(format!("{host}: {error}")))?
        };
        socket.set_nodelay(true).map_err(|error| Error::Network(error.to_string()))?;
        socket.set_read_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| Error::Network(error.to_string()))?;
        socket.set_write_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| Error::Network(error.to_string()))?;
        if !url.is_secure() {
            return Ok(Self::Plain(socket));
        }
        let name =
            rustls::pki_types::ServerName::try_from(host.to_owned()).map_err(|error| Error::Tls(error.to_string()))?;
        let connection =
            rustls::ClientConnection::new(tls_config(), name).map_err(|error| Error::Tls(error.to_string()))?;
        let mut stream = rustls::StreamOwned::new(connection, socket);
        while stream.conn.is_handshaking() {
            stream.conn.complete_io(&mut stream.sock).map_err(|error| Error::Tls(error.to_string()))?;
        }
        Ok(Self::Tls(Box::new(stream)))
    }

    fn socket(&self) -> &TcpStream {
        match self {
            Self::Plain(socket) => socket,
            Self::Tls(stream) => &stream.sock,
        }
    }

    /// How long a read waits for data.
    ///
    /// # Errors
    /// When the socket refuses the setting.
    pub fn set_read_timeout(&self, timeout: Duration) -> Result<(), Error> {
        self.socket().set_read_timeout(Some(timeout)).map_err(|error| Error::Network(error.to_string()))
    }

    /// Writes all of `bytes`.
    ///
    /// # Errors
    /// When the connection breaks.
    pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let written = match self {
            Self::Plain(socket) => socket.write_all(bytes).and_then(|()| socket.flush()),
            Self::Tls(stream) => stream.write_all(bytes).and_then(|()| stream.flush()),
        };
        written.map_err(|error| Error::Network(error.to_string()))
    }

    /// Reads what has arrived: `Ok(0)` when the peer closed, `Ok(None)`
    /// when nothing came within the read timeout.
    ///
    /// # Errors
    /// When the connection breaks.
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, Error> {
        let read = match self {
            Self::Plain(socket) => socket.read(buffer),
            Self::Tls(stream) => stream.read(buffer),
        };
        match read {
            Ok(count) => Ok(Some(count)),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Ok(None),
            // A TLS peer that closes without close_notify has still closed.
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => Ok(Some(0)),
            Err(error) => Err(Error::Network(error.to_string())),
        }
    }

    /// Reads what arrives within `wait`, as [`Stream::read`] does. A zero
    /// wait looks without waiting: the operating system refuses a zero
    /// read timeout, so the socket is made non-blocking for that one read.
    ///
    /// # Errors
    /// When the connection breaks or the socket refuses the setting.
    pub fn read_within(&mut self, buffer: &mut [u8], wait: Duration) -> Result<Option<usize>, Error> {
        if !wait.is_zero() {
            self.set_read_timeout(wait)?;
            return self.read(buffer);
        }
        let blocking = |stream: &Self, on: bool| {
            stream.socket().set_nonblocking(!on).map_err(|error| Error::Network(error.to_string()))
        };
        blocking(self, false)?;
        let read = self.read(buffer);
        blocking(self, true)?;
        read
    }

    /// Closes the connection.
    pub fn close(&self) {
        let _ = self.socket().shutdown(Shutdown::Both);
    }
}
