//! Device Bound Session Credentials (REQ-WEB-017, decision E.110) against
//! a server played through the replay transport: registration with a
//! proof of a new key, cookies used while they last, a refresh with the
//! server's challenge when one lapses, before the request that needs it,
//! and the session's end.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic, missing_docs)]

mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::string::{String, ToString};
use std::vec::Vec;

use common::{Counter, Replay, Reply, header_value, response};
use neoteletext_core::types::{Instant, PageAddress};
use neoteletext_web::agent::{BoundChange, Config, Input, Output, Storage, UserAgent};
use neoteletext_web::dbsc::{COORDINATE_BYTES, DeviceKeys, KeyHandle, PublicKey, SIGNATURE_BYTES};
use neoteletext_web::port::{ConnectionId, Pump};
use neoteletext_web::url::IpAddress;

const PUBLIC: IpAddress = IpAddress::V4([93, 184, 216, 34]);
const SECOND: u64 = 1_000_000;
const HOST: &str = "news.example:443";
const TTI: &[u8] = b"PN,10000\r\nOL,1,HELLO\r\n";

/// What the keys did, shared with the test.
#[derive(Debug, Default)]
struct KeyLog {
    made: u32,
    signed: Vec<(KeyHandle, Vec<u8>)>,
    deleted: Vec<KeyHandle>,
}

/// The port's keys, played: a key's "signature" is its handle and the
/// message's length, so the test knows which key signed what.
#[derive(Debug)]
struct Keys(Rc<RefCell<KeyLog>>);

impl DeviceKeys for Keys {
    fn create(&mut self) -> Option<(KeyHandle, PublicKey)> {
        let mut log = self.0.borrow_mut();
        log.made += 1;
        let made = u8::try_from(log.made).unwrap();
        Some((KeyHandle(log.made), PublicKey { x: [made; COORDINATE_BYTES], y: [made; COORDINATE_BYTES] }))
    }
    fn sign(&mut self, key: KeyHandle, message: &[u8]) -> Option<[u8; SIGNATURE_BYTES]> {
        self.0.borrow_mut().signed.push((key, message.to_vec()));
        let mut signature = [0_u8; SIGNATURE_BYTES];
        signature[..4].copy_from_slice(&key.0.to_be_bytes());
        Some(signature)
    }
    fn delete(&mut self, key: KeyHandle) {
        self.0.borrow_mut().deleted.push(key);
    }
}

struct Harness {
    agent: UserAgent<'static, Counter, 100, 32768>,
    pump: Pump<Replay>,
    now: Instant,
    events: Vec<String>,
    keys: Rc<RefCell<KeyLog>>,
}

impl Harness {
    fn new() -> Self {
        let storage: &'static mut Storage = Box::leak(Box::new(Storage::new()));
        let mut agent = UserAgent::new(Config::default(), storage.buffers(), Counter(0));
        let keys = Rc::new(RefCell::new(KeyLog::default()));
        agent.set_device_keys(Box::leak(Box::new(Keys(Rc::clone(&keys)))));
        let replay = Replay::new(&[("news.example", PUBLIC)]);
        Self { agent, pump: Pump::new(replay), now: Instant(1_000 * SECOND), events: Vec::new(), keys }
    }

    fn input(&mut self, input: Input<'_>) {
        self.handle(input);
        let mut buffer = [0_u8; 4096];
        for _ in 0..10_000 {
            let Some(net) = self.pump.poll(ConnectionId::Fetch, &mut buffer) else { return };
            let (kind, peer, bytes) = match net {
                neoteletext_web::port::NetInput::Connected(_, peer) => (0, Some(peer), Vec::new()),
                neoteletext_web::port::NetInput::ConnectFailed(_) => (1, None, Vec::new()),
                neoteletext_web::port::NetInput::Received(_, bytes) => (2, None, bytes.to_vec()),
                neoteletext_web::port::NetInput::Closed(_) => (3, None, Vec::new()),
                neoteletext_web::port::NetInput::Unsent(..) => continue,
            };
            let net = match kind {
                0 => neoteletext_web::port::NetInput::Connected(ConnectionId::Fetch, peer.unwrap()),
                1 => neoteletext_web::port::NetInput::ConnectFailed(ConnectionId::Fetch),
                2 => neoteletext_web::port::NetInput::Received(ConnectionId::Fetch, &bytes),
                _ => neoteletext_web::port::NetInput::Closed(ConnectionId::Fetch),
            };
            self.handle(Input::Net(net));
        }
        panic!("the exchange did not settle");
    }

    fn handle(&mut self, input: Input<'_>) {
        let Self { agent, pump, now, events, .. } = self;
        agent.handle(*now, input, &mut |output| match output.net() {
            Some(net) => pump.execute(net),
            None => {
                if let Output::Event(event) = output {
                    events.push(format!("{event:?}"));
                }
            }
        });
    }

    fn reply(&mut self, reply: Vec<u8>) {
        self.pump.transport.reply(HOST, Reply::Bytes(reply));
    }

    fn requests(&self) -> Vec<Vec<u8>> {
        self.pump.transport.requests_to(HOST)
    }

    fn fetch(&mut self, mpp: u16) {
        self.input(Input::FetchPage { page: PageAddress::from_mpp(mpp).unwrap(), subcode: None });
        self.now = Instant(self.now.0 + neoteletext_web::consts::DEFAULT_FETCH_INTERVAL_US);
        self.input(Input::Tick);
    }

    fn advance(&mut self, microseconds: u64) {
        self.now = Instant(self.now.0 + microseconds);
        self.input(Input::Tick);
    }

    fn changes(&mut self) -> Vec<String> {
        core::mem::take(&mut self.events).into_iter().filter(|event| event.starts_with("BoundSession")).collect()
    }
}

fn change(session: &str, change: BoundChange) -> String {
    format!("BoundSession {{ session: {session:?}, change: {change:?} }}")
}

fn request_line(request: &[u8]) -> String {
    String::from_utf8_lossy(request).lines().next().unwrap_or_default().to_string()
}

fn unbase64url(text: &str) -> Vec<u8> {
    let mut standard: String = text
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    while !standard.len().is_multiple_of(4) {
        standard.push('=');
    }
    let mut out = std::vec![0_u8; standard.len()];
    let length = neoteletext_web::base64::decode(standard.as_bytes(), &mut out).unwrap();
    out.truncate(length);
    out
}

/// A proof's header and payload as JSON text, and its signature.
fn proof(request: &[u8]) -> (String, String, Vec<u8>) {
    let jws = header_value(request, "Secure-Session-Response").expect("a proof");
    let parts: Vec<&str> = jws.split('.').collect();
    assert_eq!(parts.len(), 3, "compact JWS");
    (
        String::from_utf8(unbase64url(parts[0])).unwrap(),
        String::from_utf8(unbase64url(parts[1])).unwrap(),
        unbase64url(parts[2]),
    )
}

const INSTRUCTIONS: &[u8] = br#"{"session_identifier":"s1","refresh_url":"/dbsc/refresh",
    "scope":{"include_site":false},"credentials":[{"type":"cookie","name":"auth"}]}"#;

/// Opens the service, whose descriptor asks for a session, and registers.
fn registered() -> Harness {
    let mut harness = Harness::new();
    let descriptor = br#"{"name":"News","initial_page":"100","page_template":"/p/{mpp}"}"#;
    harness.reply(response(
        "200 OK",
        &[
            "Content-Type: application/json",
            r#"Secure-Session-Registration: (RS256);path="/rsa", (ES256 RS256);path="/dbsc/start";challenge="c1""#,
        ],
        descriptor,
    ));
    harness.reply(response(
        "200 OK",
        &["Content-Type: application/json", "Set-Cookie: auth=one; Max-Age=60; Secure; Path=/"],
        INSTRUCTIONS,
    ));
    harness.input(Input::OpenService("https://news.example/d.json"));
    assert_eq!(harness.changes(), [change("s1", BoundChange::Registered)]);
    harness
}

#[test]
fn a_session_is_registered_with_a_proof_of_a_new_key() {
    let harness = registered();
    let requests = harness.requests();
    assert_eq!(request_line(&requests[1]), "POST /dbsc/start HTTP/1.1");
    assert_eq!(header_value(&requests[1], "Content-Length").as_deref(), Some("0"), "posted empty");
    assert_eq!(header_value(&requests[1], "Content-Type"), None);
    let (head, payload, signature) = proof(&requests[1]);
    let coordinate = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE";
    assert_eq!(
        head,
        format!(
            r#"{{"alg":"ES256","typ":"dbsc+jwt","jwk":{{"kty":"EC","crv":"P-256","x":"{coordinate}","y":"{coordinate}"}}}}"#
        )
    );
    assert_eq!(payload, r#"{"jti":"c1"}"#);
    assert_eq!(&signature[..4], &1_u32.to_be_bytes(), "signed by the new key");
    let log = harness.keys.borrow();
    assert_eq!(log.made, 1);
    // The key signed exactly the JWS signing input.
    let jws = header_value(&requests[1], "Secure-Session-Response").unwrap();
    let input = &jws[..jws.rfind('.').unwrap()];
    assert_eq!(log.signed, [(KeyHandle(1), input.as_bytes().to_vec())]);
    let sessions: Vec<_> =
        harness.agent.bound_sessions().iter().map(|(_, session)| session.id.as_str().to_string()).collect();
    assert_eq!(sessions, ["s1"]);
}

#[test]
fn a_lapsed_cookie_is_refreshed_with_the_servers_challenge_before_the_request() {
    let mut harness = registered();
    // While the bound cookie lasts, requests go at once.
    harness.reply(response("200 OK", &["Content-Type: text/x-teletext-tti"], TTI));
    harness.fetch(0x100);
    let requests = harness.requests();
    assert_eq!(request_line(&requests[2]), "GET /p/100 HTTP/1.1");
    assert_eq!(header_value(&requests[2], "Cookie").as_deref(), Some("auth=one"));
    assert_eq!(harness.changes(), Vec::<String>::new());

    // The cookie lapses: the refresh goes first, without a proof, as no
    // challenge is kept; the server's 403 gives one; the refresh goes again
    // with its proof; the server sets a new cookie; then the page.
    harness.advance(61 * SECOND);
    harness.reply(response("403 Forbidden", &[r#"Secure-Session-Challenge: "c2";id="s1""#], b""));
    harness.reply(response("200 OK", &["Set-Cookie: auth=two; Max-Age=60; Secure; Path=/"], b""));
    harness.reply(response("200 OK", &["Content-Type: text/x-teletext-tti"], TTI));
    harness.fetch(0x100);
    let requests = harness.requests();
    let lines: Vec<String> = requests[3..].iter().map(|request| request_line(request)).collect();
    assert_eq!(lines, ["POST /dbsc/refresh HTTP/1.1", "POST /dbsc/refresh HTTP/1.1", "GET /p/100 HTTP/1.1"]);
    assert_eq!(header_value(&requests[3], "Sec-Secure-Session-Id").as_deref(), Some("\"s1\""));
    assert_eq!(header_value(&requests[3], "Secure-Session-Response"), None, "no challenge yet");
    let (head, payload, signature) = proof(&requests[4]);
    assert_eq!(head, r#"{"alg":"ES256","typ":"dbsc+jwt"}"#, "no key on a refresh");
    assert_eq!(payload, r#"{"jti":"c2"}"#);
    assert_eq!(&signature[..4], &1_u32.to_be_bytes(), "the session's own key");
    assert_eq!(header_value(&requests[5], "Cookie").as_deref(), Some("auth=two"));
    assert_eq!(harness.changes(), [change("s1", BoundChange::Refreshed)]);
}

#[test]
fn a_refused_refresh_ends_the_session_and_deletes_its_key_and_the_request_still_goes() {
    let mut harness = registered();
    harness.advance(61 * SECOND);
    harness.reply(response("401 Unauthorized", &[], b""));
    harness.reply(response("200 OK", &["Content-Type: text/x-teletext-tti"], TTI));
    harness.fetch(0x100);
    let lines: Vec<String> = harness.requests()[2..].iter().map(|request| request_line(request)).collect();
    assert_eq!(lines, ["POST /dbsc/refresh HTTP/1.1", "GET /p/100 HTTP/1.1"]);
    assert_eq!(harness.changes(), [change("s1", BoundChange::Ended)]);
    assert_eq!(harness.keys.borrow().deleted, [KeyHandle(1)]);
    assert!(harness.agent.bound_sessions().iter().next().is_none());
    // With no session, a request goes alone.
    harness.reply(response("200 OK", &["Content-Type: text/x-teletext-tti"], TTI));
    harness.fetch(0x200);
    assert_eq!(request_line(harness.requests().last().unwrap()), "GET /p/200 HTTP/1.1");
}

#[test]
fn a_server_error_leaves_the_session_and_the_request_goes_on() {
    let mut harness = registered();
    harness.advance(61 * SECOND);
    harness.reply(response("503 Service Unavailable", &[], b""));
    harness.reply(response("200 OK", &["Content-Type: text/x-teletext-tti"], TTI));
    harness.fetch(0x100);
    assert_eq!(harness.changes(), [change("s1", BoundChange::RefreshFailed)]);
    assert_eq!(harness.agent.bound_sessions().iter().count(), 1);
    assert_eq!(request_line(harness.requests().last().unwrap()), "GET /p/100 HTTP/1.1");
}

#[test]
fn a_refused_registration_deletes_its_key() {
    let mut harness = Harness::new();
    harness.reply(response(
        "200 OK",
        &["Content-Type: application/json", r#"Secure-Session-Registration: (ES256);path="/dbsc/start""#],
        br#"{"name":"News","initial_page":"100","page_template":"/p/{mpp}"}"#,
    ));
    harness.reply(response("200 OK", &["Content-Type: application/json"], br#"{"session_identifier":"s1"}"#));
    harness.input(Input::OpenService("https://news.example/d.json"));
    assert_eq!(harness.changes(), [change("", BoundChange::RegistrationFailed)]);
    assert_eq!(harness.keys.borrow().deleted, [KeyHandle(1)]);
    let (_, payload, _) = proof(&harness.requests()[1]);
    assert_eq!(payload, "{}", "no challenge was given");
}

#[test]
fn plain_http_and_other_origins_never_register() {
    let mut harness = Harness::new();
    harness.pump.transport.resolve.insert("plain.example".into(), PUBLIC);
    harness.pump.transport.reply(
        "plain.example:80",
        Reply::Bytes(response(
            "200 OK",
            &["Content-Type: application/json", r#"Secure-Session-Registration: (ES256);path="/dbsc/start""#],
            br#"{"name":"P","initial_page":"100","page_template":"/p/{mpp}"}"#,
        )),
    );
    harness.input(Input::OpenService("http://plain.example/d.json"));
    assert_eq!(harness.pump.transport.requests_to("plain.example:80").len(), 1, "no registration over plain HTTP");
    let mut harness = Harness::new();
    harness.reply(response(
        "200 OK",
        &[
            "Content-Type: application/json",
            r#"Secure-Session-Registration: (ES256);path="https://elsewhere.example/r""#,
        ],
        br#"{"name":"News","initial_page":"100","page_template":"/p/{mpp}"}"#,
    ));
    harness.input(Input::OpenService("https://news.example/d.json"));
    assert_eq!(harness.requests().len(), 1, "no registration at another origin");
    assert_eq!(harness.keys.borrow().made, 0);
}

/// Finding F-91 (decision E.114 item 6): a session scoped to the whole
/// site is refused, so a session covers its own host alone; it was kept,
/// without the public suffix list it would need.
#[test]
fn a_session_for_the_whole_site_is_refused() {
    let mut harness = Harness::new();
    harness.reply(response(
        "200 OK",
        &["Content-Type: application/json", r#"Secure-Session-Registration: (ES256);path="/dbsc/start""#],
        br#"{"name":"News","initial_page":"100","page_template":"/p/{mpp}"}"#,
    ));
    let site =
        br#"{"session_identifier":"s1","scope":{"include_site":true},"credentials":[{"type":"cookie","name":"auth"}]}"#;
    harness.reply(response("200 OK", &["Content-Type: application/json"], site));
    harness.input(Input::OpenService("https://news.example/d.json"));
    assert_eq!(harness.changes(), [change("", BoundChange::RegistrationFailed)]);
    assert_eq!(harness.keys.borrow().deleted, [KeyHandle(1)]);
    assert!(harness.agent.bound_sessions().iter().next().is_none());
}

/// Finding F-91 (decision E.114 item 6): an origin may have the agent
/// register at most `MAX_REGISTRATIONS_PER_HOUR` times in an hour, so a
/// server cannot make it create and delete keys without bound.
#[test]
fn registrations_are_bounded_for_each_origin_in_each_hour() {
    use neoteletext_web::dbsc::MAX_REGISTRATIONS_PER_HOUR;
    const HOUR: u64 = 3600 * SECOND;
    let mut harness = Harness::new();
    harness.reply(response(
        "200 OK",
        &["Content-Type: application/json"],
        br#"{"name":"News","initial_page":"100","page_template":"/p/{mpp}"}"#,
    ));
    harness.input(Input::OpenService("https://news.example/d.json"));
    let asking = || {
        let ask = r#"Secure-Session-Registration: (ES256);path="/dbsc/start""#;
        response("200 OK", &["Content-Type: text/x-teletext-tti", ask], TTI)
    };
    let tries = usize::from(MAX_REGISTRATIONS_PER_HOUR) + 2;
    for page in 0..tries {
        harness.reply(asking());
        harness.reply(response("400 Bad Request", &[], b""));
        harness.fetch(0x100 + u16::try_from(page).unwrap());
    }
    assert_eq!(harness.keys.borrow().made, u32::from(MAX_REGISTRATIONS_PER_HOUR), "no more in the hour");
    harness.advance(HOUR);
    harness.reply(asking());
    harness.reply(response("400 Bad Request", &[], b""));
    harness.fetch(0x1FF);
    assert_eq!(harness.keys.borrow().made, u32::from(MAX_REGISTRATIONS_PER_HOUR) + 1, "a new hour");
}
