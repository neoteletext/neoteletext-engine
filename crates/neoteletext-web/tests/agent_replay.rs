//! The user agent driven through the in-memory replay transport
//! (REQ-WEB-001, REQ-WEB-013's exact replay), including channels
//! (REQ-WEB-018, REQ-WEB-019, REQ-WEB-024, REQ-WEB-026, REQ-WEB-027) and
//! redirects within a channel (REQ-WEB-021).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]

mod common;

use std::string::{String, ToString};
use std::vec::Vec;

use common::{Counter, Replay, Reply, header_value, response};
#[cfg(feature = "push")]
use common::{client_frame, server_frame};
use neoteletext_core::types::{Instant, PageAddress, SubCode};
use neoteletext_web::agent::{Config, Input, Output, Source, Storage, Subscription, UserAgent};
use neoteletext_web::bytes::Writer;
use neoteletext_web::cookie::{CookieJar, Cookies};
#[cfg(feature = "cookies")]
use neoteletext_web::cookie::{CookieRejected, Site};
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
        Self::keeping(config, replay, None)
    }

    /// A harness whose agent's cookies `cookies` keeps (decision E.104).
    fn keeping(config: Config, replay: Replay, cookies: Option<&'static mut dyn Cookies>) -> Self {
        let storage: &'static mut Storage = Box::leak(Box::new(Storage::new()));
        let agent = match cookies {
            Some(cookies) => UserAgent::with_cookies(config, storage.buffers(), Counter(0), cookies),
            None => UserAgent::new(config, storage.buffers(), Counter(0)),
        };
        Self { agent, pump: Pump::new(replay), now: Instant(1_000 * SECOND), out: Vec::new() }
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

    fn open_channel(&mut self, address: &str) -> Vec<Rec> {
        self.input(Input::OpenChannel(address));
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

/// Decision E.103: the port is told its timer when its deadline changes
/// and after a tick, not after every input. Before this every input was
/// followed by one while any deadline stood, waking the port's loop twice
/// for each.
#[test]
fn the_timer_is_told_on_a_change_and_after_a_tick() {
    let timers = |records: &[Rec]| records.iter().filter(|record| matches!(record, Rec::Timer(_))).count();
    let mut harness = plain_service();
    harness.replay().reply("tt.example:80", Reply::Bytes(response("500 Oops", &["Retry-After: 30"], b"")));
    assert!(has_event(&harness.fetch(0x101), "FetchFailed"));
    // Retry-After holds the next fetch back: a deadline stands.
    let records = harness.fetch(0x102);
    assert!(records.iter().any(|record| matches!(record, Rec::Timer(at) if at.0 >= harness.now.0 + 29 * SECOND)));
    // Inputs that move no deadline are told no timer.
    harness.input(Input::WallClock(1_700_000_000));
    harness.input(Input::WallClock(1_700_000_000));
    assert_eq!(timers(&harness.take()), 0, "nothing changed");
    // A tick spends the deadline the port had: it is told the next, once.
    harness.advance(SECOND);
    assert_eq!(timers(&harness.take()), 1, "told again after a tick");
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
    secure_service_keeping(None)
}

fn secure_service_keeping(cookies: Option<&'static mut dyn Cookies>) -> Harness {
    let mut replay = Replay::new(&[("secure.example", PUBLIC)]);
    let json = br#"{"name":"S","initial_page":"100","page_template":"/p/{mpp}"}"#;
    replay.reply("secure.example:443", json_response(json));
    let mut harness = Harness::keeping(Config::default(), replay, cookies);
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

/// Finding F-16 (decision E.112; the review's probe E): a form sent to a
/// permitted origin that answers 307 or 308 is not sent again to a third
/// origin, which would get the viewer's values and the token with no check
/// and no confirmation. A 307 within the target's origin is followed.
#[test]
#[cfg(feature = "forms")]
fn a_form_redirected_with_its_method_kept_stays_on_its_target_origin() {
    for status in ["307 Temporary Redirect", "308 Permanent Redirect"] {
        let mut harness = plain_service();
        harness.replay().resolve.insert("third.example".to_string(), PUBLIC);
        let away = response(status, &["Location: http://third.example/collect"], b"");
        harness.replay().reply("forms.example:80", Reply::Bytes(away));
        harness.replay().reply("third.example:80", Reply::Bytes(response("204 No Content", &[], b"")));
        let fields = [("a", "1")];
        harness.input(Input::Submit(submission("http://tt.example/p/400", "http://forms.example/in", &fields)));
        harness.input(Input::ConfirmSubmission(true));
        let records = harness.take();
        assert!(has_event(&records, "SubmissionRefused(Origin)"), "{status}: {records:?}");
        assert!(harness.replay().requests_to("third.example:80").is_empty(), "{status}: nothing to the third origin");
    }
    // Within the target's origin, the method and body are kept.
    let mut harness = plain_service();
    let along = response("307 Temporary Redirect", &["Location: /in/again"], b"");
    harness.replay().reply("forms.example:80", Reply::Bytes(along));
    harness.replay().reply("forms.example:80", Reply::Bytes(response("204 No Content", &[], b"")));
    let fields = [("a", "1")];
    harness.input(Input::Submit(submission("http://tt.example/p/400", "http://forms.example/in", &fields)));
    harness.input(Input::ConfirmSubmission(true));
    assert!(has_event(&harness.take(), "SubmissionSucceeded { status: 204 }"));
    let last = harness.replay().requests_to("forms.example:80").last().unwrap().clone();
    assert!(String::from_utf8_lossy(&last).starts_with("POST /in/again HTTP/1.1"));
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

/// What a port's cookie store was asked and given.
#[cfg(feature = "cookies")]
#[derive(Debug, Default)]
struct PortLog {
    asked: Vec<String>,
    set: Vec<String>,
    cleared: Vec<String>,
}

/// A port's cookie store: the engine's jar, recording each call.
#[cfg(feature = "cookies")]
#[derive(Debug)]
struct PortCookies {
    jar: CookieJar<100, 32768>,
    log: std::rc::Rc<std::cell::RefCell<PortLog>>,
}

#[cfg(feature = "cookies")]
impl Cookies for PortCookies {
    fn header_for(
        &mut self,
        host: &str,
        path: &str,
        secure: bool,
        site: Site,
        now: u64,
        out: &mut Writer<'_>,
    ) -> (usize, bool) {
        self.log.borrow_mut().asked.push(std::format!("{host}{path}"));
        self.jar.header_for(host, path, secure, site, now, out)
    }

    fn store_within(
        &mut self,
        header: &[u8],
        host: &str,
        request_path: &str,
        scope: Option<&str>,
        secure: bool,
        now: u64,
        wall_clock: bool,
    ) -> Result<(), CookieRejected> {
        self.log.borrow_mut().set.push(String::from_utf8_lossy(header).into_owned());
        self.jar.store_within(header, host, request_path, scope, secure, now, wall_clock)
    }

    fn reached_securely(&self, host: &str, now: u64) -> bool {
        self.jar.reached_securely(host, now)
    }

    fn clear_host(&mut self, host: &str) {
        self.log.borrow_mut().cleared.push(host.to_string());
        self.jar.clear_host(host);
    }

    fn wipe(&mut self) {
        self.jar.wipe();
    }
}

#[cfg(feature = "cookies")]
fn port_cookies() -> (&'static mut dyn Cookies, std::rc::Rc<std::cell::RefCell<PortLog>>) {
    let log = std::rc::Rc::new(std::cell::RefCell::new(PortLog::default()));
    let store: &'static mut PortCookies =
        Box::leak(Box::new(PortCookies { jar: CookieJar::new(), log: std::rc::Rc::clone(&log) }));
    (store, log)
}

/// Decision E.104: a port that keeps the cookies is asked for each
/// request's and given each response's; the agent keeps none.
#[test]
#[cfg(feature = "cookies")]
fn a_port_keeps_the_cookies() {
    let (store, log) = port_cookies();
    let mut harness = secure_service_keeping(Some(store));
    harness.replay().reply("secure.example:443", tti_response(&["Set-Cookie: id=1; Secure; Path=/"]));
    harness.replay().reply("secure.example:443", tti_response(&[]));
    harness.fetch(0x100);
    harness.advance(SECOND);
    harness.fetch(0x101);
    let last = harness.replay().requests_to("secure.example:443").pop().unwrap();
    assert_eq!(header_value(&last, "Cookie").as_deref(), Some("id=1"), "the port's header");
    assert_eq!(log.borrow().set, ["id=1; Secure; Path=/"]);
    assert!(log.borrow().asked.iter().any(|asked| asked == "secure.example/p/101"), "{:?}", log.borrow().asked);
    assert!(harness.agent.cookies().is_empty(), "the agent's own jar holds nothing");
}

/// Decision E.104 (W3C Clear Site Data section 4.2): `Clear-Site-Data:
/// "cookies"` over HTTPS clears the host's cookies before the same
/// response's are stored; over plain HTTP it is ignored.
#[test]
#[cfg(feature = "cookies")]
fn clear_site_data_clears_before_the_response_sets() {
    let (store, log) = port_cookies();
    let mut harness = secure_service_keeping(Some(store));
    harness.replay().reply("secure.example:443", tti_response(&["Set-Cookie: old=1; Secure; Path=/"]));
    harness.replay().reply(
        "secure.example:443",
        tti_response(&["Clear-Site-Data: \"cache\", \"cookies\"", "Set-Cookie: fresh=1; Secure; Path=/"]),
    );
    harness.replay().reply("secure.example:443", tti_response(&[]));
    harness.fetch(0x100);
    harness.advance(SECOND);
    harness.fetch(0x101);
    harness.advance(SECOND);
    harness.fetch(0x102);
    let last = harness.replay().requests_to("secure.example:443").pop().unwrap();
    assert_eq!(header_value(&last, "Cookie").as_deref(), Some("fresh=1"));
    assert_eq!(log.borrow().cleared, ["secure.example"]);

    // Over plain HTTP nothing is cleared.
    let mut replay = Replay::new(&[("plain.example", PUBLIC)]);
    let json = br#"{"name":"P","initial_page":"100","page_template":"/p/{mpp}"}"#;
    replay.reply("plain.example:80", json_response(json));
    let mut plain = Harness::new(Config::default(), replay);
    plain.open("http://plain.example/d.json");
    plain.replay().reply("plain.example:80", tti_response(&["Set-Cookie: kept=1; Path=/"]));
    plain.replay().reply("plain.example:80", tti_response(&["Clear-Site-Data: \"*\""]));
    plain.replay().reply("plain.example:80", tti_response(&[]));
    plain.fetch(0x100);
    plain.advance(SECOND);
    plain.fetch(0x101);
    plain.advance(SECOND);
    plain.fetch(0x102);
    let last = plain.replay().requests_to("plain.example:80").pop().unwrap();
    assert_eq!(header_value(&last, "Cookie").as_deref(), Some("kept=1"));
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

/// REQ-WEB-031: the port's product names the agent on every request and
/// on the push WebSocket's upgrade; with none, the engine names itself.
#[test]
#[cfg(feature = "push")]
fn the_user_agent_goes_on_requests_and_the_upgrade() {
    use neoteletext_web::user_agent::{Product, significant_version};
    let engine = significant_version(env!("CARGO_PKG_VERSION"));
    let teefax = Product { name: "Teefax", version: "1.4.2", family: "ESP-IDF", engine_own: false };
    for (product, expected) in
        [(Some(teefax), format!("Teefax/1 (ESP-IDF) neoteletext/{engine}")), (None, format!("neoteletext/{engine}"))]
    {
        let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
        let json = descriptor(r#","push":{"address":"ws://tt.example/push","method":"websocket"}"#);
        replay.reply("tt.example:80", json_response(&json));
        replay.reply("tt.example:80", Reply::WebSocket(Vec::new()));
        let mut harness = Harness::new(Config { product, ..Config::default() }, replay);
        assert!(has_event(&harness.open("http://tt.example/service.json"), "PushConnected"));
        let requests = harness.replay().requests_to("tt.example:80");
        assert_eq!(requests.len(), 2, "the descriptor and the upgrade");
        for request in &requests {
            assert_eq!(header_value(request, "User-Agent").as_deref(), Some(expected.as_str()));
        }
    }
}

/// REQ-WEB-032 with REQ-SEC-006: through a proxy, the peer is the proxy
/// and may be local, and a local name is refused before anything is sent.
#[test]
fn through_a_proxy_local_names_are_refused_and_the_proxy_may_be_local() {
    let proxied = Config { proxied: true, ..Config::default() };
    let replay = Replay::new(&[("printer.local", PUBLIC)]);
    let mut harness = Harness::new(proxied, replay);
    let records = harness.open("http://printer.local/service.json");
    assert!(has_event(&records, "Blocked"), "{records:?}");
    assert!(harness.replay().connects.is_empty(), "nothing was sent to the proxy");
    // Finding F-17 (decision E.113): a name of one label and a private-use
    // suffix are local names too.
    for local in ["router", "nas.lan", "x.home.arpa"] {
        let mut harness = Harness::new(proxied, Replay::new(&[(local, PUBLIC)]));
        let records = harness.open(&format!("http://{local}/service.json"));
        assert!(has_event(&records, "Blocked"), "{local}: {records:?}");
        assert!(harness.replay().connects.is_empty(), "{local}: nothing was sent to the proxy");
    }
    // The proxy on this network: the peer the transport reports.
    let mut replay = Replay::new(&[("tt.example", PRIVATE)]);
    replay.reply("tt.example:80", json_response(&descriptor("")));
    let mut harness = Harness::new(proxied, replay);
    assert!(has_event(&harness.open("http://tt.example/service.json"), "ServiceReady"));
    let mut replay = Replay::new(&[("tt.example", PRIVATE)]);
    replay.reply("tt.example:80", json_response(&descriptor("")));
    let mut harness = Harness::new(Config::default(), replay);
    assert!(
        has_event(&harness.open("http://tt.example/service.json"), "Blocked"),
        "without a proxy the peer is the host"
    );
}

/// REQ-WEB-033: each answered request is reported, with its method,
/// address, status and size, for a developer's view of the channel.
#[test]
fn each_exchange_is_reported() {
    let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
    replay.reply("tt.example:80", json_response(&descriptor("")));
    let mut harness = Harness::new(Config::default(), replay);
    let records = harness.open("http://tt.example/service.json");
    let exchanged: Vec<&str> = events(&records).into_iter().filter(|event| event.starts_with("Exchanged")).collect();
    assert_eq!(exchanged.len(), 1, "{records:?}");
    assert!(
        exchanged[0]
            .starts_with("Exchanged { method: Get, url: \"http://tt.example/service.json\", status: 200, bytes: "),
        "{exchanged:?}"
    );
    harness.replay().reply("tt.example:80", tti_response(&[]));
    let records = harness.fetch(0x100);
    assert!(
        events(&records)
            .iter()
            .any(|event| event.starts_with("Exchanged { method: Get, url: \"http://tt.example/p/100\", status: 200")),
        "{records:?}"
    );
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
    let mut jar = CookieJar::<4, 1024>::new();
    jar.store(b"keep=1; Max-Age=3600", "tt.example", "/", false, UNIX_NOW, true).unwrap();
    let mut saved = [0u8; 1024];
    let mut writer = Writer::new(&mut saved);
    assert_eq!(jar.export(UNIX_NOW, &mut writer), Ok(1));
    let mut storage = MemoryStorage(writer.written().to_vec());
    let mut scratch = [0u8; 4096];
    assert_eq!(harness.agent.load_cookies(harness.now, &mut storage, &mut scratch), Ok(0));
    assert!(harness.agent.cookies().is_empty());
    assert_eq!(harness.agent.save_cookies(harness.now, &mut storage, &mut scratch), Ok(0));
}

// ---------------------------------------------------------------------
// Channels (decision E.70, REQ-WEB-018 to REQ-WEB-027)
// ---------------------------------------------------------------------

fn site_harness(config: Config) -> Harness {
    let replay = Replay::new(&[("site.example", PUBLIC), ("other.example", PUBLIC_2)]);
    Harness::new(config, replay)
}

fn not_found() -> Reply {
    Reply::Bytes(response("404 Not Found", &["Content-Type: text/html"], b"<p>no</p>"))
}

fn request_path(request: &[u8]) -> String {
    String::from_utf8_lossy(request).split(' ').nth(1).unwrap_or_default().to_string()
}

fn paths(harness: &mut Harness, host: &str) -> Vec<String> {
    harness.replay().requests_to(host).iter().map(|request| request_path(request)).collect()
}

/// One T42 packet with a valid address: row 0 of magazine 1.
fn packet() -> Vec<u8> {
    let mut packet = std::vec![0x15, 0x15];
    packet.extend_from_slice(&[0x20; 40]);
    packet
}

fn ok(media: &str, body: &[u8]) -> Reply {
    let header = format!("Content-Type: {media}");
    Reply::Bytes(response("200 OK", &[header.as_str()], body))
}

#[test]
fn a_channel_with_settings_opens_from_its_folder() {
    let mut harness = site_harness(Config::default());
    harness.replay().reply("site.example:80", json_response(br#"{"name":"Test"}"#));
    harness.replay().reply("site.example:80", not_found());
    let records = harness.open_channel("http://site.example/news/");
    assert!(has_event(&records, "ServiceReady { name: \"Test\""), "{records:?}");
    assert!(!has_event(&records, "ChannelDefaults"));
    assert!(!has_event(&records, "BundleFailed"), "a missing bundle is not a failure: {records:?}");
    assert_eq!(paths(&mut harness, "site.example:80"), ["/news/channel.json", "/news/index.t42"]);
}

#[test]
fn a_channel_without_settings_uses_the_defaults_its_bundle_and_page_files() {
    let mut harness = site_harness(Config::default());
    harness.replay().reply("site.example:80", not_found());
    harness.replay().reply("site.example:80", ok("application/octet-stream", &packet()));
    let records = harness.open_channel("http://site.example/news/");
    assert!(has_event(&records, "ChannelDefaults"), "{records:?}");
    assert!(has_event(&records, "ServiceReady { name: \"site.example/news\""), "{records:?}");
    let Rec::Page { source, url, media, .. } = pages(&records)[0] else { unreachable!() };
    assert_eq!(
        (*source, url.as_str(), *media),
        (Source::FrontDoor, "http://site.example/news/index.t42", MediaType::T42)
    );
    // A page: `NNN.tti` first, then `NNN.t42`.
    harness.replay().reply("site.example:80", not_found());
    harness.replay().reply("site.example:80", ok("text/plain", &packet()));
    let records = harness.fetch(0x140);
    let Rec::Page { url, media, .. } = pages(&records)[0] else { unreachable!() };
    assert_eq!((url.as_str(), *media), ("http://site.example/news/140.t42", MediaType::T42));
    assert_eq!(paths(&mut harness, "site.example:80")[2..], ["/news/140.tti", "/news/140.t42"]);
    // The extension that answered is tried first next time.
    harness.replay().reply("site.example:80", ok("application/x-teletext-t42", &packet()));
    let records = harness.fetch(0x150);
    assert_eq!(pages(&records).len(), 1, "{records:?}");
    assert_eq!(paths(&mut harness, "site.example:80")[4..], ["/news/150.t42"]);
    // Neither file: not found.
    harness.replay().reply("site.example:80", not_found());
    harness.replay().reply("site.example:80", Reply::Bytes(response("410 Gone", &[], b"")));
    let records = harness.fetch(0x160);
    assert!(has_event(&records, "PageNotInService"), "{records:?}");
}

#[test]
fn a_broken_settings_file_is_not_replaced_by_the_defaults() {
    // Only a missing file means "no settings": a server error, an invalid
    // file, a template or an HTML page must not change how the channel is
    // read.
    let cases: [(Reply, &str); 3] = [
        (Reply::Bytes(response("500 Oops", &["Content-Type: text/plain"], b"")), "DescriptorFailed(Status(500))"),
        (json_response(br#"{"page_template":"/{mpp}.t42"}"#), "DescriptorRejected(NotForChannels(PageTemplate))"),
        (ok("text/html", br#"{"name":"Test"}"#), "DescriptorFailed(MediaType)"),
    ];
    for (reply, expected) in cases {
        let mut harness = site_harness(Config::default());
        harness.replay().reply("site.example:80", reply);
        let records = harness.open_channel("http://site.example/");
        assert!(has_event(&records, expected), "{expected}: {records:?}");
        assert!(!has_event(&records, "ChannelDefaults"));
        assert!(!has_event(&records, "ServiceReady"));
    }
}

#[test]
fn settings_may_come_as_plain_text_and_link_channels() {
    let mut harness = site_harness(Config::default());
    let settings = br#"{"services":[{"name":"w","display_name":"W","channel":"http://other.example/w/"}]}"#;
    harness.replay().reply("site.example:80", ok("text/plain; charset=utf-8", settings));
    let records = harness.open_channel("http://site.example/");
    assert!(has_event(&records, "ServiceReady"), "{records:?}");
    let descriptor = harness.agent.descriptor().unwrap();
    let linked = descriptor.service("w").unwrap();
    assert!(linked.channel);
    assert_eq!(descriptor.service_address(linked), "http://other.example/w/");
}

#[test]
fn page_files_must_be_what_their_names_say() {
    let mut harness = site_harness(Config::default());
    harness.replay().reply("site.example:80", not_found());
    harness.replay().reply("site.example:80", not_found());
    harness.open_channel("http://site.example/");
    // A Template Toolkit file named 100.tti: refused, and 100.t42 is not
    // then tried.
    harness.replay().reply("site.example:80", ok("text/plain", b"[% INCLUDE header %]\n"));
    let records = harness.fetch(0x100);
    assert!(
        has_event(
            &records,
            "FetchFailed { page: PageAddress { magazine: Magazine(1), page: PageNumber(0) }, subcode: None, failure: Content }"
        ),
        "{records:?}"
    );
    assert_eq!(paths(&mut harness, "site.example:80").last().unwrap(), "/100.tti");
    // An HTML page answered with 200 for a missing file.
    harness.replay().reply("site.example:80", ok("text/html", TTI));
    let records = harness.fetch(0x110);
    assert!(
        has_event(
            &records,
            "FetchFailed { page: PageAddress { magazine: Magazine(1), page: PageNumber(16) }, subcode: None, failure: MediaType }"
        ),
        "{records:?}"
    );
    // A bundle that is not T42 is reported, and the channel works without
    // it.
    let mut harness = site_harness(Config::default());
    harness.replay().reply("site.example:80", not_found());
    harness.replay().reply("site.example:80", ok("application/octet-stream", b"not packets"));
    let records = harness.open_channel("http://site.example/");
    assert!(has_event(&records, "BundleFailed(Content)"), "{records:?}");
    assert!(has_event(&records, "ServiceReady"), "{records:?}");
}

#[test]
fn redirects_that_leave_the_channel_are_reported_not_followed() {
    let config = Config { redirects_within_channel: true, ..Config::default() };
    for location in [
        "http://other.example/news/channel.json",
        "http://site.example/sport/channel.json",
        "https://site.example:8443/news/channel.json",
    ] {
        let mut harness = site_harness(config);
        let header = format!("Location: {location}");
        harness.replay().reply("site.example:80", Reply::Bytes(response("302 Found", &[header.as_str()], b"")));
        let records = harness.open_channel("http://site.example/news/");
        let expected = format!("RedirectToOtherChannel {{ location: \"{location}\" }}");
        assert!(has_event(&records, &expected), "{location}: {records:?}");
        assert!(harness.replay().requests_to("other.example:80").is_empty(), "nothing is fetched elsewhere");
        assert_eq!(harness.replay().requests.len(), 1, "{location}");
    }
}

#[test]
fn redirects_inside_the_channel_and_upgrades_are_followed() {
    let config = Config { redirects_within_channel: true, ..Config::default() };
    let mut harness = site_harness(config);
    harness.replay().reply(
        "site.example:80",
        Reply::Bytes(response("301 Moved", &["Location: https://site.example/news/channel.json"], b"")),
    );
    harness.replay().reply("site.example:443", json_response(br#"{"name":"Secure"}"#));
    let records = harness.open_channel("http://site.example/news/");
    assert!(!has_event(&records, "RedirectToOtherChannel"), "{records:?}");
    assert!(has_event(&records, "ServiceReady { name: \"Secure\""), "{records:?}");
}

#[test]
fn neottx_still_follows_redirects_to_other_hosts() {
    let mut harness = site_harness(Config::default());
    harness
        .replay()
        .reply("site.example:80", Reply::Bytes(response("302 Found", &["Location: http://other.example/d.json"], b"")));
    harness.replay().reply("other.example:80", json_response(&descriptor("")));
    let records = harness.open("http://site.example/d.json");
    assert!(has_event(&records, "ServiceReady"), "{records:?}");
}

#[test]
fn a_service_redirected_to_another_port_leaves_its_channel() {
    let config = Config { redirects_within_channel: true, ..Config::default() };
    let mut harness = site_harness(config);
    harness.replay().reply(
        "site.example:80",
        Reply::Bytes(response("301 Moved", &["Location: https://site.example:8443/d.json"], b"")),
    );
    let records = harness.open("http://site.example/d.json");
    assert!(has_event(&records, "RedirectToOtherChannel"), "{records:?}");
}

#[test]
fn contents_are_fetched_on_request() {
    let mut harness = site_harness(Config::default());
    harness.replay().reply("site.example:80", not_found());
    harness.replay().reply("site.example:80", not_found());
    harness.open_channel("http://site.example/news/");
    harness.replay().reply("site.example:80", ok("text/plain", b"p\t100\tIndex\n"));
    harness.input(Input::FetchContents);
    let records = harness.take();
    assert!(has_event(&records, "Contents([112, 9, 49, 48, 48"), "{records:?}");
    assert_eq!(paths(&mut harness, "site.example:80").last().unwrap(), "/news/contents.txt");
    harness.replay().reply("site.example:80", not_found());
    harness.input(Input::FetchContents);
    assert!(has_event(&harness.take(), "ContentsMissing"));
    harness.replay().reply("site.example:80", ok("text/html", b"p\t100\tIndex\n"));
    harness.input(Input::FetchContents);
    assert!(has_event(&harness.take(), "ContentsFailed(MediaType)"));
}

#[test]
#[cfg(feature = "forms")]
fn a_channel_form_submits_only_inside_its_folder() {
    let mut harness = site_harness(Config::default());
    harness.replay().reply("site.example:80", json_response(br#"{"form_origins":["http://other.example"]}"#));
    harness.replay().reply("site.example:80", not_found());
    harness.open_channel("http://site.example/one/");
    let fields = [("a", "1")];
    harness.input(Input::Submit(submission("http://site.example/one/100.tti", "/two/send", &fields)));
    assert!(has_event(&harness.take(), "SubmissionRefused(Origin)"), "another folder of the same origin");
    harness.replay().reply("site.example:80", Reply::Bytes(response("204 No Content", &[], b"")));
    harness.input(Input::Submit(submission("http://site.example/one/100.tti", "/one/api/send", &fields)));
    harness.input(Input::ConfirmSubmission(true));
    assert!(has_event(&harness.take(), "SubmissionSucceeded { status: 204 }"), "inside the folder");
    harness.replay().reply("other.example:80", Reply::Bytes(response("204 No Content", &[], b"")));
    harness.input(Input::Submit(submission("http://site.example/one/100.tti", "http://other.example/in", &fields)));
    harness.input(Input::ConfirmSubmission(true));
    assert!(has_event(&harness.take(), "SubmissionSucceeded { status: 204 }"), "a listed origin");
}

#[test]
#[cfg(feature = "cookies")]
fn a_channel_cookie_set_for_the_whole_host_stays_in_its_folder() {
    let mut harness = site_harness(Config::default());
    harness
        .replay()
        .reply("site.example:80", Reply::Bytes(response("404 Not Found", &["Set-Cookie: s=1; Path=/"], b"")));
    harness.replay().reply("site.example:80", not_found());
    harness.open_channel("http://site.example/one/");
    harness.replay().reply("site.example:80", tti_response(&[]));
    harness.fetch(0x100);
    let last = harness.replay().requests_to("site.example:80").last().unwrap().clone();
    assert_eq!(header_value(&last, "Cookie").as_deref(), Some("s=1"), "sent inside the folder");
    let mut store = CookieJar::<8, 1024>::new();
    let out = &mut [0u8; 64];
    store.store_within(b"s=1; Path=/", "h.example", "/one/x", Some("/one/"), false, 0, false).unwrap();
    let mut writer = Writer::new(out);
    let (count, _) = store.header_for("h.example", "/two/100.tti", false, Site::SameService, 0, &mut writer);
    assert_eq!(count, 0, "never sent to another folder");
}

/// Finding F-15 (REQ-WEB-026; the review's probe A): a cookie a channel's
/// push handshake sets for the whole host stays in the channel's folder,
/// as one a page sets does. It was stored with no scope, so `Path=/`
/// reached every other channel on the host.
#[test]
#[cfg(all(feature = "push", feature = "cookies"))]
fn a_push_handshake_cookie_stays_in_its_channel_folder() {
    let mut harness = site_harness(Config::default());
    let settings = br#"{"push":{"address":"http://site.example/a/events","method":"sse"}}"#;
    harness.replay().reply("site.example:80", json_response(settings));
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nSet-Cookie: s=1; Path=/\r\n\
                Transfer-Encoding: chunked\r\n\r\n";
    harness.replay().reply("site.example:80", not_found());
    harness.replay().reply("site.example:80", Reply::Bytes(head.as_bytes().to_vec()));
    let records = harness.open_channel("http://site.example/a/");
    assert!(has_event(&records, "PushConnected(ServerSentEvents)"), "{records:?}");
    harness.replay().reply("site.example:80", not_found());
    harness.replay().reply("site.example:80", not_found());
    harness.open_channel("http://site.example/b/");
    harness.replay().reply("site.example:80", tti_response(&[]));
    harness.fetch(0x100);
    let last = harness.replay().requests_to("site.example:80").last().unwrap().clone();
    assert_eq!(request_path(&last), "/b/100.tti");
    assert_eq!(header_value(&last, "Cookie"), None, "not sent to another channel's folder");
}

/// Finding F-42 (decision E.114 item 2): a response from outside the
/// channel's folder, here a permitted form target on another origin,
/// stores no cookies; it was stored under the channel's folder on that
/// origin. A response after an upgrade to `https` on the channel's host
/// and folder is inside the channel, and stores its cookies.
#[test]
#[cfg(all(feature = "forms", feature = "cookies"))]
fn a_response_from_outside_the_channel_stores_no_cookies() {
    let mut harness = site_harness(Config::default());
    harness.replay().reply("site.example:80", json_response(br#"{"form_origins":["http://other.example"]}"#));
    harness.replay().reply("site.example:80", not_found());
    harness.open_channel("http://site.example/one/");
    let set = response("204 No Content", &["Set-Cookie: t=1; Path=/"], b"");
    harness.replay().reply("other.example:80", Reply::Bytes(set));
    let fields = [("a", "1")];
    harness.input(Input::Submit(submission("http://site.example/one/100.tti", "http://other.example/in", &fields)));
    harness.input(Input::ConfirmSubmission(true));
    assert!(has_event(&harness.take(), "SubmissionSucceeded { status: 204 }"));
    assert!(
        harness.agent.cookies().cookies().all(|cookie| cookie.host != b"other.example"),
        "nothing stored from outside the channel"
    );
    let upgrade = response("301 Moved", &["Location: https://site.example/one/100.tti"], b"");
    harness.replay().reply("site.example:80", Reply::Bytes(upgrade));
    harness.replay().reply("site.example:443", tti_response(&["Set-Cookie: u=1; Path=/"]));
    harness.fetch(0x100);
    assert!(harness.agent.cookies().cookies().any(|cookie| cookie.host == b"site.example"), "the upgrade stores");
}

#[cfg(feature = "push")]
fn input_service(input: bool) -> Harness {
    let mut replay = Replay::new(&[("tt.example", PUBLIC)]);
    let extra = format!(r#","push":{{"address":"ws://tt.example/push","method":"websocket","input":{input}}}"#);
    replay.reply("tt.example:80", json_response(&descriptor(&extra)));
    replay.reply("tt.example:80", Reply::WebSocket(Vec::new()));
    Harness::new(Config::default(), replay)
}

/// The push connection's frames sent after the handshake.
#[cfg(feature = "push")]
fn push_frames(harness: &mut Harness) -> Vec<(u8, Vec<u8>)> {
    harness
        .replay()
        .sent
        .iter()
        .filter(|(c, _)| *c == ConnectionId::Push)
        .skip(1)
        .map(|(_, f)| client_frame(f))
        .collect()
}

/// Decision E.90: a key record goes as one binary message on the push
/// WebSocket of a channel that takes input, and nowhere else.
#[test]
#[cfg(feature = "push")]
fn input_goes_on_the_push_websocket_only_when_declared() {
    let record = [1u8, 1, 0, 1, 0, 0, 0, 9, 0, 0x61, 0, 0, 0, 0];
    let mut harness = input_service(true);
    assert!(has_event(&harness.open("http://tt.example/service.json"), "PushConnected(WebSocket)"));
    harness.input(Input::Send(&record));
    assert_eq!(push_frames(&mut harness).last(), Some(&(0x2, record.to_vec())));
    assert!(!has_event(&harness.take(), "InputDropped"));
    // Too long for the input profile.
    harness.input(Input::Send(&[0; neoteletext_web::consts::INPUT_MESSAGE_MAX + 1]));
    assert!(has_event(&harness.take(), "InputDropped"));
    // A channel that does not take input sends nothing.
    let mut harness = input_service(false);
    harness.open("http://tt.example/service.json");
    let before = push_frames(&mut harness).len();
    harness.input(Input::Send(&record));
    assert!(has_event(&harness.take(), "InputDropped"));
    assert_eq!(push_frames(&mut harness).len(), before);
}

/// REQ-WEB-005's heartbeat: a silent WebSocket is pinged after 25 seconds,
/// and lost when no pong comes within 10 more.
#[test]
#[cfg(feature = "push")]
fn a_silent_websocket_is_pinged_then_given_up() {
    let mut harness = input_service(false);
    harness.open("http://tt.example/service.json");
    harness.advance(24 * SECOND);
    assert!(push_frames(&mut harness).iter().all(|(opcode, _)| *opcode != 0x9), "no ping yet");
    harness.advance(SECOND);
    assert_eq!(push_frames(&mut harness).last(), Some(&(0x9, Vec::new())), "pinged");
    harness.advance(9 * SECOND);
    assert!(!has_event(&harness.take(), "PushLost"));
    harness.advance(SECOND);
    assert!(has_event(&harness.take(), "PushLost { retry_in_us: 1000000, problem: Some(Failure(Timeout)) }"));
}

/// A silent event stream is made again after a minute.
#[test]
#[cfg(feature = "push")]
fn a_silent_event_stream_is_made_again() {
    let mut harness = push_service("sse", "http://tt.example/events");
    let head = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
    harness.replay().reply("tt.example:80", Reply::Bytes(head));
    assert!(has_event(&harness.open("http://tt.example/service.json"), "PushConnected(ServerSentEvents)"));
    harness.advance(59 * SECOND);
    assert!(!has_event(&harness.take(), "PushLost"));
    harness.advance(SECOND);
    assert!(has_event(&harness.take(), "PushLost { retry_in_us: 1000000, problem: Some(Failure(Timeout)) }"));
}
