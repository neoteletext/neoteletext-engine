//! The addresses `neottx` opens live (DEC-1710, TODO-1657): a program asked
//! for a channel's folder fetches the channel's settings, and one asked for
//! anything else fetches that address as a descriptor.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};

/// Ends the program when the test does.
struct Killed(Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The path of the first request `neottx fetch ADDRESS` makes, where the
/// address is `path` on a server of this test.
fn first_request(path: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let port = listener.local_addr().expect("its address").port();
    let _fetcher = Killed(
        Command::new(env!("CARGO_BIN_EXE_neottx"))
            .args(["fetch", &format!("http://127.0.0.1:{port}{path}"), "--allow-local", "127.0.0.1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start the fetcher"),
    );
    let (stream, _) = listener.accept().expect("a connection");
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).expect("a request line");
    let _ = (&stream).write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    line.split_whitespace().nth(1).expect("its target").to_owned()
}

#[test]
fn a_folder_opens_as_a_channel() {
    assert_eq!(first_request("/tap/"), "/tap/channel.json");
}

#[test]
fn a_channels_own_settings_open_as_a_channel() {
    assert_eq!(first_request("/tap/channel.json"), "/tap/channel.json");
}

#[test]
fn any_other_address_is_a_descriptor() {
    assert_eq!(first_request("/service.json"), "/service.json");
}
