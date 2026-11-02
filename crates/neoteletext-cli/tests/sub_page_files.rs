//! `neottx` against a static channel whose page has its sub-pages in a folder
//! (REQ-1887, REQ-1888, DEC-2116): a sub-page is reached by its own file.
#![cfg(feature = "live")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::{Arc, Mutex};

use neoteletext_formats::encode::encode_page;
use neoteletext_formats::item::{Item, Reader};
use neoteletext_formats::tti::TtiReader;
use neoteletext_web::channel::SETTINGS_FILE;

/// A page written as TTI, as T42.
fn t42(tti: &str) -> Vec<u8> {
    let mut packets = Vec::new();
    let mut reader = TtiReader::new();
    let mut each = |item: Item<'_>| {
        if let Item::Page(page) = item {
            encode_page(page, &mut |packet| packets.extend_from_slice(&packet.bytes));
        }
    };
    reader.feed(tti.as_bytes(), &mut each);
    reader.finish(&mut each);
    packets
}

/// One request: its target goes in `asked`, and it is answered from `files`.
fn answer(mut stream: TcpStream, files: &BTreeMap<String, (&str, Vec<u8>)>, asked: &Mutex<Vec<String>>) {
    let mut reader = BufReader::new(stream.try_clone().expect("the stream"));
    let mut line = String::new();
    reader.read_line(&mut line).expect("a request line");
    let target = line.split_whitespace().nth(1).expect("a target").to_owned();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
            break;
        }
    }
    asked.lock().expect("the list").push(target.clone());
    let head = match files.get(&target) {
        Some((media, body)) => format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {media}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        ),
        None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),
    };
    let _ = stream.write_all(head.as_bytes());
    if let Some((_, body)) = files.get(&target) {
        let _ = stream.write_all(body);
    }
}

// Verifies REQ-1887, REQ-1888: selecting a sub-page of a page with a folder
// asks for the first part of its list, then for that sub-page's file alone.
#[test]
fn a_sub_page_is_fetched_from_its_pages_folder() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let port = listener.local_addr().expect("its address").port();
    let files = BTreeMap::from([
        (format!("/ch/{SETTINGS_FILE}"), ("application/json", br#"{"name":"Test"}"#.to_vec())),
        ("/ch/100.t42".to_owned(), ("application/octet-stream", t42("PN,10000\r\nSC,0000\r\nOL,1,Index\r\n"))),
        ("/ch/302.t42".to_owned(), ("application/octet-stream", t42("PN,30200\r\nSC,0001\r\nOL,1,First\r\n"))),
        ("/ch/302/0.sub".to_owned(), ("text/plain", b"2\tv\n0001\tv\n0002\tv\n".to_vec())),
        ("/ch/302/0002.t42".to_owned(), ("application/octet-stream", t42("PN,30200\r\nSC,0002\r\nOL,1,Second\r\n"))),
    ]);
    let asked = Arc::new(Mutex::new(Vec::new()));
    let served = Arc::clone(&asked);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            answer(stream, &files, &served);
        }
    });
    let output = Command::new(env!("CARGO_BIN_EXE_neottx"))
        .args(["probe", &format!("http://127.0.0.1:{port}/ch/"), "--allow-local", "127.0.0.1", "--no-sandbox"])
        .args(["--select", "302/0002"])
        .output()
        .expect("run neottx probe");
    let said = String::from_utf8_lossy(&output.stdout);
    let asked = asked.lock().expect("the list");
    assert!(said.contains("selection   302/0002: a page in"), "{said} {:?}: {asked:?}", output.stderr);
    let list = asked.iter().position(|target| target == "/ch/302/0.sub").expect("the first part was asked for");
    let file = asked.iter().position(|target| target == "/ch/302/0002.t42").expect("the sub-page was asked for");
    assert!(list < file, "{asked:?}");
    assert!(!asked.iter().any(|target| target.starts_with("/ch/302.")), "not the page's own file: {asked:?}");
}
