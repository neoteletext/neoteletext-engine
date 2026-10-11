//! The user agent driven through the in-memory replay transport
//! (REQ-WEB-001, REQ-WEB-013's exact replay).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]

mod common;

use std::string::{String, ToString};
use std::vec::Vec;

use common::{Counter, Replay, Reply, header_value, response};
#[cfg(feature = "push")]
use common::{client_frame, server_frame};
use neoteletext_core::types::{Instant, PageAddress, SubCode};
use neoteletext_web::agent::{Config, Input, Output, Source, Storage, Subscription, UserAgent};
use neoteletext_web::form::{FormMethod, FormSubmission};
use neoteletext_web::http::MediaType;
use neoteletext_web::port::{ConnectionId, Pump, Security};
use neoteletext_web::url::IpAddress;

const PUBLIC: IpAddress = IpAddress::V4([93, 184, 216, 34]);
const PUBLIC_2: IpAddress = IpAddress::V4([93, 184, 216, 35]);
const PRIVATE: IpAddress = IpAddress::V4([192, 168, 1, 10]);
const SECOND: u64 = 1_000_000;

const TTI: &[u8] = b"PN,10000\r\nOL,1,HELLO\r\n";

#[derive(Debug, Clone, PartialEq)]
enum Rec {
    Page { source: Source, media: MediaType, url: String, secure: bool, bytes: Vec<u8> },
    Event(String),
    Timer(Instant),
}

struct Harness {
    agent: UserAgent<'static, Counter, 100, 32768>,
    pump: Pump<Replay>,
    now: Instant,
    out: Vec<Rec>,
}

fn record(output: Output<'_>) -> Rec {
    match output {
        Output::PageBytes(page) => Rec::Page {
            source: page.source,
            media: page.media,
            url: page.url.to_string(),
            secure: page.secure,
            bytes: page.bytes.to_vec(),
        },
        Output::Event(event) => Rec::Event(format!("{event:?}")),
        Output::Timer { at } => Rec::Timer(at),
        other => panic!("network output reached the application: {other:?}"),
    }
}

impl Harness {
    fn new(config: Config, replay: Replay) -> Self {
        let storage: &'static mut Storage = Box::leak(Box::new(Storage::new()));
        Self {
            agent: UserAgent::new(config, storage.buffers(), Counter(0)),
            pump: Pump::new(replay),
            now: Instant(1_000 * SECOND),
            out: Vec::new(),
        }
    }

    fn handle(&mut self, input: Input<'_>) {
        let Self { agent, pump, out, now } = self;
        agent.handle(*now, input, &mut |output| match output.net() {
            Some(net) => pump.execute(net),
            None => out.push(record(output)),
        });
    }

    fn settle(&mut self) {
        let mut buffer = [0u8; 4096];
        for _ in 0..10_000 {
            let mut progressed = false;
            for connection in [ConnectionId::Fetch, ConnectionId::Push] {
                if let Some(input) = self.pump.poll(connection, &mut buffer) {
                    let input = copy_net(input);
                    self.handle(Input::Net(input.as_net()));
                    progressed = true;
                }
            }
            if !progressed {
                return;
            }
        }
        panic!("the exchange did not settle");
    }

    fn input(&mut self, input: Input<'_>) {
        self.handle(input);
        self.settle();
    }

    fn advance(&mut self, microseconds: u64) {
        self.now = Instant(self.now.0 + microseconds);
        self.input(Input::Tick);
    }

    fn take(&mut self) -> Vec<Rec> {
        std::mem::take(&mut self.out)
    }

    fn replay(&mut self) -> &mut Replay {
        &mut self.pump.transport
    }

    fn open(&mut self, address: &str) -> Vec<Rec> {
        self.input(Input::OpenService(address));
        self.take()
    }

    fn open_site(&mut self, address: &str) -> Vec<Rec> {
        self.input(Input::OpenSite(address));
        self.take()
    }

    /// Requests a page and lets the default rate limit's interval pass.
    fn fetch(&mut self, mpp: u16) -> Vec<Rec> {
        self.input(Input::FetchPage { page: PageAddress::from_mpp(mpp).unwrap(), subcode: None });
        self.advance(neoteletext_web::consts::DEFAULT_FETCH_INTERVAL_US);
        self.take()
    }
}

/// An owned copy of a network input, so the receive buffer is free while
/// the agent handles it.
struct NetCopy {
    kind: u8,
    connection: ConnectionId,
    peer: Option<IpAddress>,
    bytes: Vec<u8>,
}

fn copy_net(input: neoteletext_web::port::NetInput<'_>) -> NetCopy {
    use neoteletext_web::port::NetInput;
    match input {
        NetInput::Connected(connection, peer) => NetCopy { kind: 0, connection, peer: Some(peer), bytes: Vec::new() },
        NetInput::ConnectFailed(connection) => NetCopy { kind: 1, connection, peer: None, bytes: Vec::new() },
        NetInput::Received(connection, bytes) => NetCopy { kind: 2, connection, peer: None, bytes: bytes.to_vec() },
        NetInput::Closed(connection) => NetCopy { kind: 3, connection, peer: None, bytes: Vec::new() },
    }
}

impl NetCopy {
    fn as_net(&self) -> neoteletext_web::port::NetInput<'_> {
        use neoteletext_web::port::NetInput;
        match self.kind {
            0 => NetInput::Connected(self.connection, self.peer.unwrap()),
            1 => NetInput::ConnectFailed(self.connection),
            2 => NetInput::Received(self.connection, &self.bytes),
            _ => NetInput::Closed(self.connection),
        }
    }
}

fn events(records: &[Rec]) -> Vec<&str> {
    records.iter().filter_map(|r| if let Rec::Event(e) = r { Some(e.as_str()) } else { None }).collect()
}

fn has_event(records: &[Rec], prefix: &str) -> bool {
    events(records).iter().any(|e| e.starts_with(prefix))
}

fn pages(records: &[Rec]) -> Vec<&Rec> {
    records.iter().filter(|r| matches!(r, Rec::Page { .. })).collect()
}

fn descriptor(extra: &str) -> Vec<u8> {
    format!(
        r#"{{"name":"Test","initial_page":"100","page_template":"/p/{{mpp}}","form_origins":["http://forms.example"],
            "services":[{{"name":"other","display_name":"Other","descriptor":"http://other.example/d.json"}}]{extra}}}"#
    )
    .into_bytes()
}

fn json_response(body: &[u8]) -> Reply {
    Reply::Bytes(response("200 OK", &["Content-Type: application/json"], body))
}

fn tti_response(headers: &[&str]) -> Reply {
    let mut all = std::vec!["Content-Type: text/x-teletext-tti"];
    all.extend_from_slice(headers);
    Reply::Bytes(response("200 OK", &all, TTI))
}

/// REQ-WEB-014: every request carries only the permitted headers, never
/// `Referer`, always `Accept-Encoding: identity`.
fn assert_profile_headers(replay: &Replay) {
    const PERMITTED: [&str; 16] = [
        "host",
        "user-agent",
        "accept",
        "accept-encoding",
        "cookie",
        "if-none-match",
        "if-modified-since",
        "content-type",
        "content-length",
        "last-event-id",
        "upgrade",
        "connection",
        "sec-websocket-key",
        "sec-websocket-version",
        "sec-websocket-extensions",
        "sec-websocket-protocol",
    ];
    assert_ne!(replay.requests.len(), 0);
    for (_, request) in &replay.requests {
        let text = String::from_utf8_lossy(request);
        let head = text.split("\r\n\r\n").next().unwrap();
        let mut lines = head.split("\r\n");
        let request_line = lines.next().unwrap();
        assert!(request_line.starts_with("GET ") || request_line.starts_with("POST "), "{request_line}");
        assert!(request_line.ends_with(" HTTP/1.1"));
        let mut names = Vec::new();
        for line in lines {
            let name = line.split(':').next().unwrap().to_ascii_lowercase();
            assert!(PERMITTED.contains(&name.as_str()), "header {name} is outside the profile");
            names.push(name);
        }
        assert!(names.contains(&"host".to_string()) && names.contains(&"user-agent".to_string()));
        if !names.contains(&"upgrade".to_string()) {
            assert_eq!(header_value(request, "Accept-Encoding").as_deref(), Some("identity"));
        }
        let is_post = request_line.starts_with("POST ");
        assert_eq!(names.contains(&"content-type".to_string()), is_post, "Content-Type only on form submissions");
    }
}

fn plain_service() -> Harness {
    let mut replay = Replay::new(&[("tt.example", PUBLIC), ("forms.example", PUBLIC_2), ("evil.example", PRIVATE)]);
    replay.reply("tt.example:80", json_response(&descriptor("")));
    let mut harness = Harness::new(Config::default(), replay);
    let records = harness.open("http://tt.example/service.json");
    assert!(has_event(&records, "ServiceReady { name: \"Test\""), "{records:?}");
    harness
}

#[test]
fn descriptor_page_and_conditional_request() {
    let mut harness = plain_service();
    harness.replay().reply(
        "tt.example:80",
        tti_response(&["ETag: \"v1\"", "Set-Cookie: session=v1-abc; Path=/; HttpOnly", "Cache-Control: no-cache"]),
    );
    harness
        .replay()
        .reply("tt.example:80", Reply::Bytes(b"HTTP/1.1 304 Not Modified\r\nETag: \"v1\"\r\n\r\n".to_vec()));
    let records = harness.fetch(0x100);
    let got = pages(&records);
    assert_eq!(got.len(), 1);
    let Rec::Page { source, media, url, secure, bytes } = got[0] else { unreachable!() };
    assert_eq!(*media, MediaType::Tti);
    assert_eq!(url, "http://tt.example/p/100");
    assert!(!secure, "came over plain HTTP: the indicator must show");
    assert_eq!(bytes, TTI);
    assert!(matches!(source, Source::Page { .. }));

    // The same page again: one conditional request, no body.
    harness.advance(SECOND);
    let records = harness.fetch(0x100);
    assert_eq!(pages(&records), Vec::<&Rec>::new());
    assert!(has_event(&records, "PageCurrent"), "{records:?}");
    let requests = harness.replay().requests_to("tt.example:80");
    let last = requests.last().unwrap();
    assert_eq!(header_value(last, "If-None-Match").as_deref(), Some("\"v1\""));
    #[cfg(feature = "cookies")]
    assert_eq!(header_value(last, "Cookie").as_deref(), Some("session=v1-abc"));
    // One persistent connection served all three requests.
    assert_eq!(harness.replay().connects.len(), 1);
    assert_profile_headers(harness.replay());
}

#[test]
fn unencrypted_connection_is_reported() {
    let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
    replay.reply("tt.example:80", json_response(&descriptor("")));
    let mut harness = Harness::new(Config::default(), replay);
    let records = harness.open("http://tt.example/service.json");
    assert!(has_event(&records, "Unencrypted { connection: Fetch, host: \"tt.example\" }"));
}

#[test]
fn fresh_pages_are_not_fetched_again() {
    let mut harness = plain_service();
    harness.replay().reply("tt.example:80", tti_response(&["Cache-Control: max-age=60"]));
    harness.fetch(0x100);
    harness.advance(10 * SECOND);
    let records = harness.fetch(0x100);
    assert!(has_event(&records, "PageCurrent"));
    assert_eq!(harness.replay().requests_to("tt.example:80").len(), 2, "no request while fresh");
}

#[test]
fn not_found_and_failures_leave_pages_alone() {
    let mut harness = plain_service();
    harness.replay().reply("tt.example:80", Reply::Bytes(response("404 Not Found", &[], b"")));
    harness.replay().reply("tt.example:80", Reply::Bytes(response("500 Oops", &["Retry-After: 30"], b"")));
    harness.replay().reply(
        "tt.example:80",
        Reply::Bytes(response("200 OK", &["Content-Type: text/x-teletext-tti", "Content-Encoding: gzip"], b"x")),
    );
    harness.replay().reply("tt.example:80", Reply::Bytes(response("200 OK", &["Content-Type: text/html"], b"<p>")));
    assert!(has_event(&harness.fetch(0x100), "PageNotInService"));
    harness.advance(SECOND);
    assert!(has_event(
        &harness.fetch(0x101),
        "FetchFailed { page: PageAddress { magazine: Magazine(1), page: PageNumber(1) }, subcode: None, failure: Status(500)"
    ));
    // Retry-After holds the next fetch back.
    let records = harness.fetch(0x102);
    assert!(pages(&records).is_empty() && !has_event(&records, "FetchFailed"));
    assert!(
        records.contains(&Rec::Timer(Instant(harness.now.0 + 29 * SECOND)))
            || records.iter().any(|r| matches!(r, Rec::Timer(at) if at.0 >= harness.now.0 + 29 * SECOND))
    );
    harness.advance(30 * SECOND);
    assert!(has_event(
        &harness.take(),
        "FetchFailed { page: PageAddress { magazine: Magazine(1), page: PageNumber(2) }, subcode: None, failure: Http(Compressed)"
    ));
    harness.advance(SECOND);
    assert!(has_event(
        &harness.fetch(0x103),
        "FetchFailed { page: PageAddress { magazine: Magazine(1), page: PageNumber(3) }, subcode: None, failure: MediaType"
    ));
}

#[test]
fn size_limit_abandons_the_response() {
    let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
    replay.reply("tt.example:80", json_response(&descriptor("")));
    replay.reply(
        "tt.example:80",
        Reply::Bytes(response("200 OK", &["Content-Type: text/x-teletext-tti"], &[b'x'; 2000])),
    );
    let config = Config { page_limit: 1000, ..Config::default() };
    let mut harness = Harness::new(config, replay);
    harness.open("http://tt.example/service.json");
    assert!(has_event(
        &harness.fetch(0x100),
        "FetchFailed { page: PageAddress { magazine: Magazine(1), page: PageNumber(0) }, subcode: None, failure: Http(BodyTooLarge)"
    ));
}

#[test]
fn time_limit() {
    let mut harness = plain_service();
    harness.replay().reply("tt.example:80", Reply::Silent);
    let records = harness.fetch(0x100);
    assert!(records.iter().any(|r| matches!(r, Rec::Timer(_))));
    harness.advance(31 * SECOND);
    assert!(has_event(
        &harness.take(),
        "FetchFailed { page: PageAddress { magazine: Magazine(1), page: PageNumber(0) }, subcode: None, failure: Timeout"
    ));
}

#[test]
fn rate_limit() {
    let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
    replay.reply("tt.example:80", json_response(&descriptor("")));
    replay.reply("tt.example:80", tti_response(&[]));
    replay.reply("tt.example:80", tti_response(&[]));
    let config = Config { fetch_interval_us: SECOND, ..Config::default() };
    let mut harness = Harness::new(config, replay);
    harness.open("http://tt.example/service.json");
    harness.advance(SECOND);
    harness.input(Input::FetchPage { page: PageAddress::from_mpp(0x100).unwrap(), subcode: None });
    harness.input(Input::FetchPage { page: PageAddress::from_mpp(0x101).unwrap(), subcode: None });
    let records = harness.take();
    assert_eq!(pages(&records).len(), 1, "the second waits");
    assert!(records.contains(&Rec::Timer(Instant(harness.now.0 + SECOND))));
    harness.advance(SECOND / 2);
    assert_eq!(pages(&harness.take()), Vec::<&Rec>::new());
    harness.advance(SECOND / 2);
    assert_eq!(pages(&harness.take()).len(), 1);
}

#[test]
fn redirects_are_followed_up_to_five() {
    let mut harness = plain_service();
    for (status, location) in [("301 Moved", "/a"), ("302 Found", "http://tt.example/b"), ("307 Temporary", "c")] {
        harness
            .replay()
            .reply("tt.example:80", Reply::Bytes(response(status, &[&format!("Location: {location}")], b"moved")));
    }
    harness.replay().reply("tt.example:80", tti_response(&[]));
    let records = harness.fetch(0x100);
    let Rec::Page { url, .. } = pages(&records)[0] else { unreachable!() };
    assert_eq!(url, "http://tt.example/c");

    for _ in 0..6 {
        harness.replay().reply("tt.example:80", Reply::Bytes(response("302 Found", &["Location: /loop"], b"")));
    }
    harness.advance(SECOND);
    assert!(has_event(
        &harness.fetch(0x101),
        "FetchFailed { page: PageAddress { magazine: Magazine(1), page: PageNumber(1) }, subcode: None, failure: TooManyRedirects"
    ));
}

#[test]
fn redirects_to_blocked_addresses_are_refused() {
    let mut harness = plain_service();
    // A name that resolves to a private address: checked on the peer.
    harness
        .replay()
        .reply("tt.example:80", Reply::Bytes(response("302 Found", &["Location: http://evil.example/x"], b"")));
    let records = harness.fetch(0x100);
    assert!(has_event(&records, "Blocked { connection: Fetch, host: \"evil.example\" }"), "{records:?}");
    assert!(has_event(
        &records,
        "FetchFailed { page: PageAddress { magazine: Magazine(1), page: PageNumber(0) }, subcode: None, failure: Blocked"
    ));
    assert!(harness.replay().requests_to("evil.example:80").is_empty(), "no request reached it");
    // Address literals are refused before connecting.
    for literal in [
        "http://127.0.0.1/x",
        "http://10.1.2.3/",
        "http://[::1]/",
        "http://[::ffff:192.168.0.1]/",
        "http://169.254.169.254/",
    ] {
        harness.advance(SECOND);
        harness
            .replay()
            .reply("tt.example:80", Reply::Bytes(response("302 Found", &[&format!("Location: {literal}")], b"")));
        let records = harness.fetch(0x101);
        assert!(has_event(&records, "Blocked"), "{literal} {records:?}");
        harness.input(Input::Tick);
    }
    assert!(!harness.replay().connects.iter().any(|(_, host, _, _)| host == "127.0.0.1" || host == "10.1.2.3"));
}

#[test]
fn allowed_local_service() {
    let mut replay = Replay::new(&[("tt.local", PRIVATE)]);
    replay.reply("tt.local:80", json_response(&descriptor("")));
    let mut config = Config::default();
    config.allowed_local[0] = Some(PRIVATE);
    let mut harness = Harness::new(config, replay);
    assert!(has_event(&harness.open("http://tt.local/service.json"), "ServiceReady"));
    let mut replay = Replay::new(&[("tt.local", PRIVATE)]);
    replay.reply("tt.local:80", json_response(&descriptor("")));
    let mut harness = Harness::new(Config::default(), replay);
    assert!(has_event(&harness.open("http://tt.local/service.json"), "DescriptorFailed(Blocked)"));
}

fn secure_service() -> Harness {
    let mut replay = Replay::new(&[("secure.example", PUBLIC)]);
    let json = br#"{"name":"S","initial_page":"100","page_template":"/p/{mpp}"}"#;
    replay.reply("secure.example:443", json_response(json));
    let mut harness = Harness::new(Config::default(), replay);
    let records = harness.open("https://secure.example/d.json");
    assert!(has_event(&records, "ServiceReady { name: \"S\""), "{records:?}");
    assert_eq!(harness.replay().connects[0].3, Security::Tls);
    harness
}

#[test]
fn no_downgrade_after_https() {
    let mut harness = secure_service();
    // A redirect from HTTPS to plain HTTP, even on another host.
    harness
        .replay()
        .reply("secure.example:443", Reply::Bytes(response("301 Moved", &["Location: http://tt.example/p"], b"")));
    let records = harness.fetch(0x100);
    assert!(has_event(&records, "DowngradeRefused { host: \"tt.example\" }"), "{records:?}");
    // The host itself, over plain HTTP, for the rest of the session.
    harness.advance(SECOND);
    let records = harness.open("http://secure.example/d.json");
    assert!(has_event(&records, "DowngradeRefused { host: \"secure.example\" }"));
    assert!(has_event(&records, "DescriptorFailed(Downgrade)"));
    assert!(!harness.replay().connects.iter().any(|(_, _, port, _)| *port == 80));
}

#[test]
fn tls_unavailable() {
    let config = Config { tls_available: false, ..Config::default() };
    let mut harness = Harness::new(config, Replay::new(&[("secure.example", PUBLIC)]));
    assert!(has_event(&harness.open("https://secure.example/d.json"), "DescriptorFailed(TlsUnavailable)"));
    assert_eq!(harness.replay().connects, []);
}

#[test]
fn descriptor_rejections() {
    let mut replay = Replay::new(&[("tt.example", PUBLIC), ("secure.example", PUBLIC_2)]);
    replay.reply("tt.example:80", json_response(br#"{"name":"S","initial_page":"100","page_template":"/{page}"}"#));
    replay.reply("tt.example:80", Reply::Bytes(response("200 OK", &["Content-Type: text/html"], b"{}")));
    replay.reply(
        "secure.example:443",
        json_response(br#"{"name":"S","initial_page":"100","page_template":"http://secure.example/{mpp}"}"#),
    );
    let mut harness = Harness::new(Config::default(), replay);
    assert!(has_event(&harness.open("http://tt.example/a.json"), "DescriptorRejected(Placeholder)"));
    harness.advance(SECOND);
    assert!(has_event(&harness.open("http://tt.example/b.json"), "DescriptorFailed(MediaType)"));
    harness.advance(SECOND);
    assert!(has_event(&harness.open("https://secure.example/c.json"), "DescriptorRejected(Downgrade(PageTemplate))"));
    assert!(has_event(&harness.open("ftp://x.example/"), "DescriptorFailed(Address(Scheme))"));
}

fn submission<'a>(page_url: &'a str, target: &'a str, fields: &'a [(&'a str, &'a str)]) -> FormSubmission<'a> {
    FormSubmission {
        page_url,
        method: FormMethod::Post,
        target,
        fields,
        token: Some("5f2c9a17e0"),
        action: Some("send_contact"),
    }
}

#[test]
#[cfg(feature = "forms")]
fn form_over_plain_http_needs_the_warning_every_time() {
    let mut harness = plain_service();
    harness.replay().reply("tt.example:80", Reply::Bytes(response("200 OK", &["Content-Type: text/plain"], b"ok")));
    harness.replay().reply("tt.example:80", Reply::Bytes(response("500 Error", &[], b"")));
    let fields = [("name", "Ann Smith"), ("email", "ann@example.org")];
    harness.input(Input::Submit(submission("http://tt.example/p/400", "/contact", &fields)));
    let records = harness.take();
    assert!(
        has_event(
            &records,
            "ConfirmSubmission { host: \"tt.example\", port: 80, secure: false, plain_http: true, first_to_origin: true }"
        ),
        "{records:?}"
    );
    assert!(harness.replay().requests_to("tt.example:80").len() == 1, "nothing sent before confirmation");
    // Only one submission at a time.
    harness.input(Input::Submit(submission("http://tt.example/p/400", "/contact", &fields)));
    assert!(has_event(&harness.take(), "SubmissionRefused(Busy)"));
    harness.input(Input::ConfirmSubmission(true));
    assert!(has_event(&harness.take(), "SubmissionSucceeded { status: 200 }"));
    let post = harness.replay().requests_to("tt.example:80").pop().unwrap();
    let text = String::from_utf8(post.clone()).unwrap();
    assert!(text.starts_with("POST /contact HTTP/1.1\r\n"));
    assert_eq!(header_value(&post, "Content-Type").as_deref(), Some("application/x-www-form-urlencoded"));
    assert!(
        text.ends_with("\r\n\r\nname=Ann+Smith&email=ann%40example.org&ttx_token=5f2c9a17e0&ttx_action=send_contact"),
        "{text}"
    );
    // The second submission to the same plain origin still warns.
    harness.input(Input::Submit(submission("http://tt.example/p/400", "/contact", &fields)));
    assert!(has_event(
        &harness.take(),
        "ConfirmSubmission { host: \"tt.example\", port: 80, secure: false, plain_http: true, first_to_origin: false }"
    ));
    harness.input(Input::ConfirmSubmission(true));
    assert!(has_event(&harness.take(), "SubmissionFailed { status: Some(500), failure: None }"));
    // Declining sends nothing.
    harness.input(Input::Submit(submission("http://tt.example/p/400", "/contact", &fields)));
    harness.input(Input::ConfirmSubmission(false));
    assert!(has_event(&harness.take(), "SubmissionCancelled"));
    assert_profile_headers(harness.replay());
}

#[test]
#[cfg(feature = "forms")]
fn form_destinations_are_restricted() {
    let mut harness = plain_service();
    let fields = [("a", "1")];
    harness.input(Input::Submit(submission("http://tt.example/p/400", "http://evil.example/steal", &fields)));
    assert!(has_event(&harness.take(), "SubmissionRefused(Origin)"));
    harness.input(Input::Submit(submission("http://tt.example/p/400", "http://tt.example:8080/x", &fields)));
    assert!(has_event(&harness.take(), "SubmissionRefused(Origin)"));
    // An origin the descriptor lists.
    harness.replay().reply("forms.example:80", Reply::Bytes(response("204 No Content", &[], b"")));
    harness.input(Input::Submit(submission("http://tt.example/p/400", "http://forms.example/in", &fields)));
    harness.input(Input::ConfirmSubmission(true));
    assert!(has_event(&harness.take(), "SubmissionSucceeded { status: 204 }"));
}

#[test]
#[cfg(feature = "forms")]
fn form_responses() {
    let mut replay = Replay::new(&[("secure.example", PUBLIC)]);
    replay
        .reply("secure.example:443", json_response(br#"{"name":"S","initial_page":"100","page_template":"/p/{mpp}"}"#));
    replay.reply(
        "secure.example:443",
        Reply::Bytes(response("303 See Other", &["Location: /thanks", "Set-Cookie: s=2; Secure; Path=/"], b"")),
    );
    replay.reply("secure.example:443", tti_response(&[]));
    replay.reply("secure.example:443", Reply::Close);
    let mut harness = Harness::new(Config::default(), replay);
    harness.open("https://secure.example/d.json");
    let fields = [("q", "x")];
    harness.input(Input::Submit(submission("https://secure.example/p/400", "/f", &fields)));
    assert!(has_event(
        &harness.take(),
        "ConfirmSubmission { host: \"secure.example\", port: 443, secure: true, plain_http: false, first_to_origin: true }"
    ));
    harness.input(Input::ConfirmSubmission(true));
    let records = harness.take();
    assert!(has_event(&records, "SubmissionPage"), "{records:?}");
    let Rec::Page { source, url, .. } = pages(&records)[0] else { unreachable!() };
    assert_eq!(*source, Source::Form);
    assert_eq!(url, "https://secure.example/thanks");
    let requests = harness.replay().requests_to("secure.example:443");
    assert!(String::from_utf8_lossy(requests.last().unwrap()).starts_with("GET /thanks HTTP/1.1"), "303 becomes GET");
    // Second submission to a confirmed HTTPS origin: no confirmation. The
    // server closes without answering: a network failure.
    harness.input(Input::Submit(submission("https://secure.example/p/400", "/f", &fields)));
    let records = harness.take();
    assert!(!has_event(&records, "ConfirmSubmission"));
    assert!(has_event(&records, "SubmissionFailed { status: None, failure: Some(Network) }"), "{records:?}");
}

#[test]
#[cfg(feature = "forms")]
fn get_form_puts_values_in_the_query() {
    let mut harness = plain_service();
    harness.replay().reply("tt.example:80", tti_response(&[]));
    let fields = [("q", "a b")];
    let form =
        FormSubmission { method: FormMethod::Get, ..submission("http://tt.example/p/400", "/search?old=1", &fields) };
    harness.input(Input::Submit(form));
    harness.input(Input::ConfirmSubmission(true));
    let request = harness.replay().requests_to("tt.example:80").pop().unwrap();
    assert!(
        String::from_utf8_lossy(&request)
            .starts_with("GET /search?q=a+b&ttx_token=5f2c9a17e0&ttx_action=send_contact HTTP/1.1")
    );
    assert_eq!(header_value(&request, "Content-Length"), None);
}

#[test]
#[cfg(all(feature = "forms", feature = "cookies"))]
fn cross_origin_form_sends_only_same_site_none_secure_cookies() {
    let mut replay = Replay::new(&[("secure.example", PUBLIC), ("forms.example", PUBLIC_2)]);
    replay.reply(
        "secure.example:443",
        json_response(
            br#"{"name":"S","initial_page":"100","page_template":"/p/{mpp}","form_origins":["https://forms.example"]}"#,
        ),
    );
    replay.reply(
        "forms.example:443",
        Reply::Bytes(response(
            "200 OK",
            &[
                "Set-Cookie: lax=1; Secure; Path=/",
                "Set-Cookie: none=1; Secure; SameSite=None; Path=/",
                "Content-Type: text/plain",
            ],
            b"",
        )),
    );
    replay.reply("forms.example:443", Reply::Bytes(response("200 OK", &["Content-Type: text/plain"], b"")));
    replay.reply("forms.example:443", Reply::Bytes(response("200 OK", &["Content-Type: text/plain"], b"")));
    let mut harness = Harness::new(Config::default(), replay);
    harness.open("https://secure.example/d.json");
    let fields = [("a", "1")];
    // Same-origin submission on the forms host sets its cookies.
    harness.input(Input::Submit(submission("https://forms.example/form", "/in", &fields)));
    harness.input(Input::ConfirmSubmission(true));
    harness.take();
    harness.input(Input::Submit(submission("https://forms.example/form", "/in", &fields)));
    let same = harness.replay().requests_to("forms.example:443").pop().unwrap();
    assert_eq!(header_value(&same, "Cookie").as_deref(), Some("lax=1; none=1"));
    // Cross-origin, from the service's page.
    harness.input(Input::Submit(submission("https://secure.example/p/400", "https://forms.example/in", &fields)));
    harness.input(Input::ConfirmSubmission(true));
    let cross = harness.replay().requests_to("forms.example:443").pop().unwrap();
    assert_eq!(header_value(&cross, "Cookie").as_deref(), Some("none=1"));
}

#[test]
fn links_to_other_services() {
    let mut harness = plain_service();
    harness.input(Input::FollowLink { service: "nowhere", page: PageAddress::from_mpp(0x100).unwrap(), subcode: None });
    assert!(has_event(&harness.take(), "LinkUnknown"));
    harness.replay().resolve.insert("other.example".to_string(), PUBLIC_2);
    harness.replay().reply(
        "other.example:80",
        json_response(br#"{"name":"Other","initial_page":"200","page_template":"/x/{mpp}/{sub}"}"#),
    );
    harness.replay().reply("other.example:80", tti_response(&[]));
    harness.input(Input::FollowLink {
        service: "other",
        page: PageAddress::from_mpp(0x234).unwrap(),
        subcode: Some(SubCode::new(2)),
    });
    harness.advance(SECOND);
    let records = harness.take();
    assert!(has_event(&records, "ServiceReady { name: \"Other\""), "{records:?}");
    let Rec::Page { url, .. } = pages(&records)[0] else { unreachable!() };
    assert_eq!(url, "http://other.example/x/234/0002");
}

#[cfg(feature = "push")]
fn push_service(method: &str, address: &str) -> Harness {
    let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
    let extra = format!(r#","push":{{"address":"{address}","method":"{method}"}}"#);
    replay.reply("tt.example:80", json_response(&descriptor(&extra)));
    Harness::new(Config::default(), replay)
}

#[test]
#[cfg(feature = "push")]
fn websocket_push() {
    let mut harness = push_service("websocket", "ws://tt.example/push");
    let packet = [0x15u8; 42];
    let mut frames = server_frame(0x2, &[packet, packet].concat());
    frames.extend(server_frame(0x1, TTI));
    frames.extend(server_frame(0x9, b"hi"));
    harness.replay().reply("tt.example:80", Reply::WebSocket(frames));
    let records = harness.open("http://tt.example/service.json");
    assert!(has_event(&records, "PushConnected(WebSocket)"), "{records:?}");
    let delivered = pages(&records);
    assert_eq!(delivered.len(), 2);
    assert!(
        matches!(delivered[0], Rec::Page { source: Source::Push, media: MediaType::T42, bytes, .. } if bytes.len() == 84)
    );
    assert!(
        matches!(delivered[1], Rec::Page { source: Source::Push, media: MediaType::Tti, bytes, .. } if bytes == TTI)
    );
    let push_frames: Vec<_> = harness
        .replay()
        .sent
        .iter()
        .filter(|(c, _)| *c == ConnectionId::Push)
        .skip(1)
        .map(|(_, f)| client_frame(f))
        .collect();
    assert_eq!(push_frames[0], (0x1, b"subscribe all".to_vec()));
    assert_eq!(push_frames[1], (0xA, b"hi".to_vec()), "ping answered with pong");
    let handshake = &harness.replay().requests_to("tt.example:80")[1];
    assert_eq!(header_value(handshake, "Sec-WebSocket-Version").as_deref(), Some("13"));
    // Changing the subscription.
    let wanted = [PageAddress::from_mpp(0x100).unwrap(), PageAddress::from_mpp(0x1A0).unwrap()];
    harness.input(Input::Subscribe(Subscription::Pages(&wanted)));
    let (_, last) = harness.replay().sent.last().unwrap().clone();
    assert_eq!(client_frame(&last), (0x1, b"subscribe 100 1A0".to_vec()));
    assert_profile_headers(harness.replay());
}

#[test]
#[cfg(feature = "push")]
fn websocket_push_reconnects_with_doubling_delays() {
    let mut harness = push_service("websocket", "ws://tt.example/push");
    harness.replay().reply("tt.example:80", Reply::WebSocket(server_frame(0x8, &1000u16.to_be_bytes())));
    harness.replay().reply("tt.example:80", Reply::Close);
    harness.replay().reply("tt.example:80", Reply::Close);
    let records = harness.open("http://tt.example/service.json");
    assert!(has_event(&records, "PushLost { retry_in_us: 1000000, problem: None }"), "{records:?}");
    let (_, close) = harness.replay().sent.last().unwrap().clone();
    assert_eq!(client_frame(&close), (0x8, 1000u16.to_be_bytes().to_vec()), "close answered");
    harness.advance(SECOND);
    assert!(has_event(&harness.take(), "PushLost { retry_in_us: 2000000"));
    harness.advance(2 * SECOND);
    assert!(has_event(&harness.take(), "PushLost { retry_in_us: 4000000"));
    for _ in 0..8 {
        harness.replay().reply("tt.example:80", Reply::Close);
        harness.advance(60 * SECOND);
    }
    assert!(has_event(&harness.take(), "PushLost { retry_in_us: 60000000"), "capped at 60 seconds");
}

#[test]
#[cfg(feature = "push")]
fn websocket_push_protocol_errors() {
    let mut harness = push_service("websocket", "ws://tt.example/push");
    let mut frames = server_frame(0x2, &[0u8; 41]);
    frames.extend([0x82, 0x85, 0, 0, 0, 0, 1, 2, 3, 4, 5]); // masked server frame
    harness.replay().reply("tt.example:80", Reply::WebSocket(frames));
    let records = harness.open("http://tt.example/service.json");
    assert!(has_event(&records, "PushLost { retry_in_us: 0, problem: Some(PacketLength)"));
    assert!(has_event(&records, "PushLost { retry_in_us: 1000000, problem: Some(WebSocket(Protocol))"), "{records:?}");
    let (_, close) = harness.replay().sent.last().unwrap().clone();
    assert_eq!(client_frame(&close), (0x8, 1002u16.to_be_bytes().to_vec()));
    // A wrong accept value fails the handshake.
    harness.replay().reply("tt.example:80", Reply::BadWebSocket);
    harness.advance(SECOND);
    assert!(has_event(&harness.take(), "PushLost { retry_in_us: 2000000, problem: Some(WebSocket(Handshake))"));
}

#[test]
#[cfg(feature = "push")]
fn websocket_push_oversize_message() {
    let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
    replay.reply(
        "tt.example:80",
        json_response(&descriptor(r#","push":{"address":"ws://tt.example/push","method":"websocket"}"#)),
    );
    replay.reply("tt.example:80", Reply::WebSocket(server_frame(0x2, &[0u8; 420])));
    let config = Config { push_limit: 200, ..Config::default() };
    let mut harness = Harness::new(config, replay);
    let records = harness.open("http://tt.example/service.json");
    assert!(has_event(&records, "PushLost { retry_in_us: 1000000, problem: Some(WebSocket(TooBig))"), "{records:?}");
}

#[test]
#[cfg(feature = "push")]
fn server_sent_events_push_and_resumption() {
    let mut harness = push_service("sse", "http://tt.example/events?x=1");
    let packets = [0x15u8; 42];
    let mut encoded = [0u8; 56];
    let n = neoteletext_web::base64::encode(&packets, &mut encoded).unwrap();
    let body = format!(
        "id: 5\nevent: page\ndata: PN,10000\ndata: OL,1,HELLO\n\nid: 6\nevent: packets\ndata: {}\n\n",
        core::str::from_utf8(&encoded[..n]).unwrap()
    );
    let mut stream =
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
    stream.extend(format!("{:x}\r\n{body}\r\n", body.len()).as_bytes());
    harness.replay().reply("tt.example:80", Reply::BytesThenClose(stream));
    harness.replay().reply("tt.example:80", Reply::Silent);
    let records = harness.open("http://tt.example/service.json");
    assert!(has_event(&records, "PushConnected(ServerSentEvents)"));
    let delivered = pages(&records);
    assert_eq!(delivered.len(), 2, "{records:?}");
    assert!(matches!(delivered[0], Rec::Page { media: MediaType::Tti, bytes, .. } if bytes == b"PN,10000\nOL,1,HELLO"));
    assert!(matches!(delivered[1], Rec::Page { media: MediaType::T42, bytes, .. } if bytes == &packets));
    assert!(has_event(&records, "PushLost { retry_in_us: 1000000"));
    let first = &harness.replay().requests_to("tt.example:80")[1];
    assert!(String::from_utf8_lossy(first).starts_with("GET /events?x=1&subscribe=all HTTP/1.1"));
    assert_eq!(header_value(first, "Accept").as_deref(), Some("text/event-stream"));
    assert_eq!(header_value(first, "Last-Event-ID"), None);
    harness.advance(SECOND);
    let second = harness.replay().requests_to("tt.example:80").pop().unwrap();
    assert_eq!(header_value(&second, "Last-Event-ID").as_deref(), Some("6"), "resumed");
    assert_profile_headers(harness.replay());
}

#[test]
#[cfg(feature = "push")]
fn push_to_blocked_peer_is_refused() {
    let mut replay = Replay::new(&[("tt.example", PUBLIC), ("push.example", PRIVATE)]);
    replay.reply(
        "tt.example:80",
        json_response(&descriptor(r#","push":{"address":"ws://push.example/p","method":"websocket"}"#)),
    );
    let mut harness = Harness::new(Config::default(), replay);
    let records = harness.open("http://tt.example/service.json");
    assert!(has_event(&records, "Blocked { connection: Push, host: \"push.example\" }"), "{records:?}");
    assert_eq!(harness.replay().requests_to("push.example:80"), Vec::<Vec<u8>>::new());
}

#[test]
#[cfg(feature = "cookies")]
fn cookies_round_trip_and_wipe() {
    let mut harness = secure_service();
    harness.replay().reply(
        "secure.example:443",
        tti_response(&["Set-Cookie: id=1; Secure; Path=/; Max-Age=60", "Set-Cookie: __Host-x=1; Secure; Path=/"]),
    );
    harness.fetch(0x100);
    harness.advance(SECOND);
    harness.replay().reply("secure.example:443", tti_response(&["Set-Cookie: id=evil; Path=/"]));
    harness.replay().reply("secure.example:443", tti_response(&[]));
    harness.fetch(0x101);
    harness.advance(SECOND);
    harness.fetch(0x102);
    let last = harness.replay().requests_to("secure.example:443").pop().unwrap();
    assert_eq!(
        header_value(&last, "Cookie").as_deref(),
        Some("id=evil; __Host-x=1"),
        "over HTTPS a new cookie replaces it"
    );
    assert!(harness.agent.cookies().reached_securely("secure.example", harness.now.0 / SECOND));
    harness.input(Input::WipeCookies);
    assert!(harness.agent.cookies().is_empty());
}

#[test]
#[cfg(all(feature = "push", feature = "cookies"))]
fn cookies_go_with_websocket_upgrades() {
    let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
    let json = descriptor(r#","push":{"address":"ws://tt.example/push","method":"websocket"}"#);
    replay.reply(
        "tt.example:80",
        Reply::Bytes(response("200 OK", &["Content-Type: application/json", "Set-Cookie: s=v1; Path=/"], &json)),
    );
    replay.reply("tt.example:80", Reply::WebSocket(Vec::new()));
    let mut harness = Harness::new(Config::default(), replay);
    assert!(has_event(&harness.open("http://tt.example/service.json"), "PushConnected"));
    let handshake = &harness.replay().requests_to("tt.example:80")[1];
    assert_eq!(header_value(handshake, "Upgrade").as_deref(), Some("websocket"));
    assert_eq!(header_value(handshake, "Cookie").as_deref(), Some("s=v1"));
}

#[test]
fn stale_kept_alive_connection_is_retried_once_for_a_get() {
    let mut harness = plain_service();
    harness.replay().reply("tt.example:80", Reply::Close);
    harness.replay().reply("tt.example:80", tti_response(&[]));
    let records = harness.fetch(0x100);
    assert_eq!(pages(&records).len(), 1, "{records:?}");
    assert_eq!(harness.replay().connects.len(), 2, "a fresh connection for the retry");
}

#[derive(Default)]
struct MemoryStorage(Vec<u8>);

impl neoteletext_web::port::CookieStorage for MemoryStorage {
    type Error = ();

    fn save(&mut self, data: &[u8]) -> Result<(), ()> {
        self.0 = data.to_vec();
        Ok(())
    }

    fn load(&mut self, buffer: &mut [u8]) -> Result<usize, ()> {
        buffer[..self.0.len()].copy_from_slice(&self.0);
        Ok(self.0.len())
    }
}

#[test]
#[cfg(feature = "cookies")]
fn cookies_are_saved_only_when_enabled_and_never_session_ones() {
    const UNIX_NOW: u64 = 1_791_590_400;
    let setup = |persist: bool| {
        let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
        replay.reply("tt.example:80", json_response(&descriptor("")));
        replay.reply(
            "tt.example:80",
            tti_response(&["Set-Cookie: keep=1; Path=/; Max-Age=3600", "Set-Cookie: session=1; Path=/"]),
        );
        let mut harness = Harness::new(Config { persist_cookies: persist, ..Config::default() }, replay);
        harness.input(Input::WallClock(UNIX_NOW));
        harness.open("http://tt.example/service.json");
        harness.fetch(0x100);
        harness
    };
    let mut scratch = [0u8; 4096];
    let mut storage = MemoryStorage::default();
    let off = setup(false);
    assert_eq!(off.agent.save_cookies(off.now, &mut storage, &mut scratch), Ok(0), "not saved by default");
    assert_eq!(storage.0, Vec::<u8>::new());
    let on = setup(true);
    assert_eq!(on.agent.save_cookies(on.now, &mut storage, &mut scratch), Ok(1), "the session cookie is not saved");
    let mut fresh = Harness::new(Config { persist_cookies: true, ..Config::default() }, Replay::new(&[]));
    fresh.input(Input::WallClock(UNIX_NOW));
    assert_eq!(fresh.agent.load_cookies(fresh.now, &mut storage, &mut scratch), Ok(1));
    assert_eq!(fresh.agent.cookies().len(), 1);
}

// Decision E.61: a build without a feature answers whatever needs it with a
// value that says so, and never acts as if the missing part had worked.

#[test]
#[cfg(not(feature = "push"))]
fn push_not_compiled_in_is_reported_and_never_connects() {
    for (method, address, reported) in [
        ("websocket", "ws://tt.example/push", "PushNotCompiledIn(WebSocket)"),
        ("sse", "http://tt.example/events", "PushNotCompiledIn(ServerSentEvents)"),
    ] {
        let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
        let extra = format!(r#","push":{{"address":"{address}","method":"{method}"}}"#);
        replay.reply("tt.example:80", json_response(&descriptor(&extra)));
        let mut harness = Harness::new(Config::default(), replay);
        let records = harness.open("http://tt.example/service.json");
        assert!(has_event(&records, "ServiceReady"), "{records:?}");
        assert!(has_event(&records, reported), "{records:?}");
        assert_eq!(records.last(), Some(&Rec::Event(reported.to_string())), "no timer for a reconnection follows");
        harness.input(Input::Subscribe(Subscription::All));
        harness.advance(120 * SECOND);
        assert!(!has_event(&harness.take(), "Push"), "nothing more about push");
        assert!(
            !harness.replay().connects.iter().any(|(connection, ..)| *connection == ConnectionId::Push),
            "no push connection is opened"
        );
    }
}

#[test]
#[cfg(not(feature = "forms"))]
fn form_submission_not_compiled_in_is_refused_and_sends_nothing() {
    let mut harness = plain_service();
    let before = harness.replay().requests.len();
    let fields = [("name", "Ann Smith")];
    harness.input(Input::Submit(submission("http://tt.example/p/400", "/contact", &fields)));
    harness.input(Input::ConfirmSubmission(true));
    let records = harness.take();
    assert_eq!(events(&records), ["SubmissionRefused(NotCompiledIn)"]);
    assert_eq!(harness.replay().requests.len(), before, "no request was made");
}

#[test]
#[cfg(not(feature = "cookies"))]
fn cookies_not_compiled_in_are_reported_and_never_kept() {
    const UNIX_NOW: u64 = 1_791_590_400;
    let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
    replay.reply("tt.example:80", json_response(&descriptor("")));
    replay.reply("tt.example:80", tti_response(&["Set-Cookie: keep=1; Path=/; Max-Age=3600"]));
    replay.reply("tt.example:80", tti_response(&[]));
    let config = Config { persist_cookies: true, ..Config::default() };
    let mut harness = Harness::new(config, replay);
    harness.input(Input::WallClock(UNIX_NOW));
    harness.open("http://tt.example/service.json");
    let records = harness.fetch(0x100);
    assert_eq!(pages(&records).len(), 1, "the page still arrives");
    assert!(has_event(&records, "CookieRejected(NotCompiledIn)"), "{records:?}");
    harness.advance(SECOND);
    harness.fetch(0x101);
    for request in harness.replay().requests_to("tt.example:80") {
        assert_eq!(header_value(&request, "Cookie"), None);
    }
    assert!(harness.agent.cookies().is_empty());

    // Cookies saved by a build that has them are not restored.
    let mut jar = neoteletext_web::cookie::CookieJar::<4, 1024>::new();
    jar.store(b"keep=1; Max-Age=3600", "tt.example", "/", false, UNIX_NOW, true).unwrap();
    let mut saved = [0u8; 1024];
    let mut writer = neoteletext_web::bytes::Writer::new(&mut saved);
    assert_eq!(jar.export(UNIX_NOW, &mut writer), Ok(1));
    let mut storage = MemoryStorage(writer.written().to_vec());
    let mut scratch = [0u8; 4096];
    assert_eq!(harness.agent.load_cookies(harness.now, &mut storage, &mut scratch), Ok(0));
    assert!(harness.agent.cookies().is_empty());
    assert_eq!(harness.agent.save_cookies(harness.now, &mut storage, &mut scratch), Ok(0));
}

// ---------------------------------------------------------------------
// Web-native sites (docs/web-native-concept.md section 2)
// ---------------------------------------------------------------------

fn site_harness(config: Config) -> Harness {
    let replay = Replay::new(&[("site.example", PUBLIC), ("other.example", PUBLIC_2)]);
    Harness::new(config, replay)
}

fn not_found() -> Reply {
    Reply::Bytes(response("404 Not Found", &["Content-Type: text/plain"], b"no"))
}

fn request_path(request: &[u8]) -> String {
    String::from_utf8_lossy(request).split(' ').nth(1).unwrap_or_default().to_string()
}

#[test]
fn a_site_with_a_descriptor_is_opened_from_the_well_known_address() {
    let mut harness = site_harness(Config::default());
    harness.replay().reply("site.example:80", json_response(&descriptor("")));
    let records = harness.open_site("http://site.example/");
    assert!(has_event(&records, "ServiceReady { name: \"Test\""), "{records:?}");
    assert!(!has_event(&records, "SiteDefaults"));
    let requests = harness.replay().requests_to("site.example:80");
    assert_eq!(request_path(&requests[0]), "/.well-known/teletext.json");
}

#[test]
fn a_site_without_a_descriptor_uses_the_defaults_and_its_front_door() {
    let mut harness = site_harness(Config::default());
    harness.replay().reply("site.example:80", not_found());
    harness.replay().reply("site.example:80", tti_response(&[]));
    let records = harness.open_site("http://site.example/anything");
    assert!(has_event(&records, "SiteDefaults"), "{records:?}");
    assert!(has_event(&records, "ServiceReady { name: \"site.example\""), "{records:?}");
    let front = pages(&records);
    assert_eq!(front.len(), 1, "{records:?}");
    let Rec::Page { source, url, .. } = front[0] else { unreachable!() };
    assert_eq!(*source, Source::FrontDoor);
    assert_eq!(url, "http://site.example/index.t42");
    // A page of the site is its own file.
    harness.replay().reply("site.example:80", tti_response(&[]));
    let records = harness.fetch(0x140);
    let Rec::Page { url, .. } = pages(&records)[0] else { unreachable!() };
    assert_eq!(url, "http://site.example/140.t42");
}

#[test]
fn a_site_without_a_front_door_still_opens() {
    let mut harness = site_harness(Config::default());
    harness.replay().reply("site.example:80", not_found());
    harness.replay().reply("site.example:80", not_found());
    let records = harness.open_site("http://site.example/");
    assert!(has_event(&records, "ServiceReady"), "{records:?}");
    assert_eq!(pages(&records), Vec::<&Rec>::new());
}

#[test]
fn a_broken_descriptor_is_not_replaced_by_the_defaults() {
    // Only a missing descriptor means "no descriptor": a server error or
    // an invalid one must not silently change how the site is read.
    let mut harness = site_harness(Config::default());
    harness.replay().reply("site.example:80", Reply::Bytes(response("500 Oops", &["Content-Type: text/plain"], b"")));
    let records = harness.open_site("http://site.example/");
    assert!(has_event(&records, "DescriptorFailed(Status(500))"), "{records:?}");
    assert!(!has_event(&records, "SiteDefaults"));
}

#[test]
fn redirects_to_another_site_are_reported_not_followed_in_a_browser() {
    let config = Config { redirects_within_site: true, ..Config::default() };
    let mut harness = site_harness(config);
    harness.replay().reply(
        "site.example:80",
        Reply::Bytes(response("302 Found", &["Location: http://other.example/.well-known/teletext.json"], b"")),
    );
    let records = harness.open_site("http://site.example/");
    assert!(
        has_event(&records, "RedirectToOtherSite { location: \"http://other.example/.well-known/teletext.json\" }"),
        "{records:?}"
    );
    assert!(harness.replay().requests_to("other.example:80").is_empty(), "nothing is fetched from the other site");
}

#[test]
fn an_upgrade_to_https_on_the_same_host_is_not_another_site() {
    let config = Config { redirects_within_site: true, ..Config::default() };
    let mut harness = site_harness(config);
    harness.replay().reply(
        "site.example:80",
        Reply::Bytes(response("301 Moved", &["Location: https://site.example/.well-known/teletext.json"], b"")),
    );
    harness.replay().reply(
        "site.example:443",
        json_response(br#"{"name":"Secure","initial_page":"100","page_template":"/{mpp}.t42"}"#),
    );
    let records = harness.open_site("http://site.example/");
    assert!(!has_event(&records, "RedirectToOtherSite"), "{records:?}");
    assert!(has_event(&records, "ServiceReady"), "{records:?}");
}

#[test]
fn ttx_still_follows_redirects_to_other_hosts() {
    let mut harness = site_harness(Config::default());
    harness
        .replay()
        .reply("site.example:80", Reply::Bytes(response("302 Found", &["Location: http://other.example/d.json"], b"")));
    harness.replay().reply("other.example:80", json_response(&descriptor("")));
    let records = harness.open("http://site.example/d.json");
    assert!(has_event(&records, "ServiceReady"), "{records:?}");
}
