//! Opening a socket, each step within the connect limit: to an address the
//! caller's own rule chose, or to the proxy the user chose, which then
//! opens a tunnel to the host (REQ-WEB-032).

use neoteletext_web::consts::DEFAULT_CONNECT_TIMEOUT_US;
use neoteletext_web::proxy::Proxy;
use neoteletext_web::url::IpAddress;
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Time allowed to connect and to complete the TLS handshake; a write and
/// a read of the proxy's answer are held to it too.
pub const CONNECT_TIMEOUT: Duration = Duration::from_micros(DEFAULT_CONNECT_TIMEOUT_US);

/// The first address `host` resolves to.
///
/// # Errors
/// The name does not resolve, said for the user.
pub fn first_address(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("{host}: {error}"))?
        .next()
        .ok_or_else(|| format!("{host}: no address"))
}

/// A socket to `target`, whose reads and writes are held to
/// [`CONNECT_TIMEOUT`]. `name` is what the user knows the target as.
///
/// # Errors
/// The connection is refused or times out, said for the user.
pub fn connect_within_limit(target: &SocketAddr, name: &str) -> Result<TcpStream, String> {
    let socket = TcpStream::connect_timeout(target, CONNECT_TIMEOUT).map_err(|error| format!("{name}: {error}"))?;
    socket.set_read_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| error.to_string())?;
    socket.set_write_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| error.to_string())?;
    Ok(socket)
}

/// A socket through `proxy` that reaches `host` and `port`, with the
/// host's first bytes when the proxy's answer brought them, which the
/// caller hands on before anything it reads next (decision E.102). The
/// proxy's address may be local: the user chose it.
///
/// # Errors
/// The proxy cannot be reached or refuses, said for the user; nothing goes
/// direct instead.
pub fn open_tunnel(proxy: &Proxy, host: &str, port: u16) -> Result<(TcpStream, Vec<u8>), String> {
    let target = first_address(&proxy.host, proxy.port)?;
    let mut socket = connect_within_limit(&target, &proxy.host)?;
    let first = proxy.open_tunnel(&mut socket, host, port)?;
    Ok((socket, first))
}

/// The address the operating system gave, as the engine writes one.
#[must_use]
pub fn web_address(address: IpAddr) -> IpAddress {
    match address {
        IpAddr::V4(v4) => IpAddress::V4(v4.octets()),
        IpAddr::V6(v6) => IpAddress::V6(v6.segments()),
    }
}
