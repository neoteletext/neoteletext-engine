//! The zxnet live feed reader (REQ-INP-022) driven through the in-memory
//! replay transport, with message samples from a live capture.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]

mod common;

use std::string::{String, ToString};
use std::vec::Vec;

use common::{Counter, Replay, Reply, client_frame, server_frame};
use neoteletext_core::packet::{Decoded, Packet, decode};
use neoteletext_core::types::{Instant, PageAddress};
use neoteletext_web::port::{ConnectionId, NetInput, Pump};
use neoteletext_web::url::IpAddress;
use neoteletext_web::zxnet::{Channel, ChannelError, KNOWN_CHANNELS, LiveInput, LiveOutput, LiveStorage, ZxnetFeed};

const SECOND: u64 = 1_000_000;
const ZXNET: IpAddress = IpAddress::V4([46, 235, 226, 40]);

/// Samples from a live capture of Teefax (research/answers-teletext-
/// sources.md section 2.1): the 8/30 packet of 2026-10-09 22:47:44 UTC,
/// a header of page 168 and a row 1.
const BSDP: &str = r#"["bsdp", "FRUV6urqXgAAhQckMzNYVRUVFRVURUVGwVggICAgICAgICAgICAgIA=="]"#;
const HEADER: &str =
    r#"["header", 0, "0Dhe0BUVFRVURUVGwVggMbY4ICBG8ukgILkgT+P0IIMys7o0Ny80sw==", false, 16, 1, 3, 104]"#;
const ROW: &str = r#"["row", 0, 1, 1, "kSBwcHCScHBwlHBwcJJ8fCwsLOwsLOwsLOwsLOy8LCzsvCx8LOx8fA=="]"#;
const CLOCK: &str = r#"["clock", 0, "MrO6NDcvNLU="]"#;

#[derive(Debug, Clone, PartialEq)]
enum Rec {
    Packet(Packet),
    Event(String),
    Timer(Instant),
}

struct Harness {
    feed: ZxnetFeed<'static, Counter>,
    pump: Pump<Replay>,
    now: Instant,
    out: Vec<Rec>,
}

impl Harness {
    fn new(channel: &Channel, replay: Replay) -> Self {
        let storage: &'static mut LiveStorage = Box::leak(Box::new(LiveStorage::new()));
        Self {
            feed: ZxnetFeed::new(*channel, storage.buffers(), Counter(0)),
            pump: Pump::new(replay),
            now: Instant(SECOND),
            out: Vec::new(),
        }
    }

    fn handle(&mut self, input: LiveInput<'_>) {
        let Self { feed, pump, out, now } = self;
        feed.handle(*now, input, &mut |output| match output.net() {
            Some(net) => pump.execute(net),
            None => out.push(match output {
                LiveOutput::Packet(packet) => Rec::Packet(packet),
                LiveOutput::Event(event) => Rec::Event(format!("{event:?}")),
                LiveOutput::Timer(at) => Rec::Timer(at),
                other => panic!("{other:?}"),
            }),
        });
    }

    fn settle(&mut self) {
        let mut buffer = [0u8; 4096];
        // Enough reads for the 88 KB message of the oversized test in the
        // transport's small chunks; the bound only stops a feed that never
        // settles.
        for _ in 0..100_000 {
            let Some(input) = self.pump.poll(ConnectionId::Live, &mut buffer) else { return };
            let (kind, peer, bytes) = match input {
                NetInput::Connected(_, peer) => (0, Some(peer), Vec::new()),
                NetInput::ConnectFailed(_) => (1, None, Vec::new()),
                NetInput::Received(_, bytes) => (2, None, bytes.to_vec()),
                NetInput::Closed(_) => (3, None, Vec::new()),
            };
            let input = match kind {
                0 => NetInput::Connected(ConnectionId::Live, peer.unwrap()),
                1 => NetInput::ConnectFailed(ConnectionId::Live),
                2 => NetInput::Received(ConnectionId::Live, &bytes),
                _ => NetInput::Closed(ConnectionId::Live),
            };
            self.handle(LiveInput::Net(input));
        }
        panic!("did not settle");
    }

    fn input(&mut self, input: LiveInput<'_>) {
        self.handle(input);
        self.settle();
    }

    fn advance(&mut self, microseconds: u64) {
        self.now = Instant(self.now.0 + microseconds);
        self.input(LiveInput::Tick);
    }

    /// The server sends text messages.
    fn server(&mut self, messages: &[&str]) {
        for message in messages {
            self.pump.transport.inject(ConnectionId::Live, &server_frame(0x1, message.as_bytes()));
        }
        self.settle();
    }

    fn take(&mut self) -> Vec<Rec> {
        std::mem::take(&mut self.out)
    }

    /// Text messages the client sent, after the handshake.
    fn client_messages(&self) -> Vec<String> {
        self.pump
            .transport
            .sent
            .iter()
            .filter(|(c, bytes)| *c == ConnectionId::Live && !bytes.starts_with(b"GET "))
            .map(|(_, frame)| client_frame(frame))
            .filter(|(opcode, _)| *opcode == 0x1)
            .map(|(_, payload)| String::from_utf8(payload).unwrap())
            .collect()
    }
}

fn events(records: &[Rec]) -> Vec<&str> {
    records.iter().filter_map(|r| if let Rec::Event(e) = r { Some(e.as_str()) } else { None }).collect()
}

fn has_event(records: &[Rec], prefix: &str) -> bool {
    events(records).iter().any(|e| e.starts_with(prefix))
}

fn packets(records: &[Rec]) -> Vec<Packet> {
    records.iter().filter_map(|r| if let Rec::Packet(p) = r { Some(*p) } else { None }).collect()
}

fn connected(channel: &str) -> Harness {
    let mut replay = Replay::new(&[("zxnet.co.uk", ZXNET)]);
    for _ in 0..8 {
        replay.reply("zxnet.co.uk:443", Reply::WebSocket(Vec::new()));
    }
    let mut harness = Harness::new(&Channel::from_short_name(channel).unwrap(), replay);
    harness.input(LiveInput::Start);
    assert!(has_event(&harness.take(), "Connected"));
    harness
}

#[test]
fn channel_identifiers() {
    let expected = [
        ("teefax", "teefax", "wss://zxnet.co.uk/ws/teefax"),
        ("nms-ceefax", "nms-ceefax", "wss://internal.nathanmediaservices.co.uk/websockets/ceefax"),
        ("chunkytext", "chunkytext", "wss://zxnet.co.uk/ws/chunkytext"),
        ("spark", "spark", "wss://zxnet.co.uk/ws/spark"),
        ("ch4", "channel4-capture", "wss://zxnet.co.uk/ws/ch4"),
        ("test", "test", "wss://zxnet.co.uk/ws/test"),
    ];
    assert_eq!(KNOWN_CHANNELS.len(), expected.len());
    for (short, service, address) in expected {
        let channel = Channel::from_short_name(short).unwrap();
        assert_eq!(channel.service(), service);
        assert_eq!(channel.address().as_str(), address);
        let rebuilt = Channel::from_address(address, service).unwrap();
        assert_eq!(rebuilt.identifier(), channel.identifier(), "{short}");
    }
    assert_eq!(Channel::from_short_name("bbc1").map(|_| ()), Err(ChannelError::UnknownName));
    assert_eq!(Channel::from_identifier("not base64!").map(|_| ()), Err(ChannelError::Base64));
    // ["a","b"] and ["a","evil.example@x","/p"] and a non-array.
    assert_eq!(Channel::from_identifier("WyJhIiwiYiJd").map(|_| ()), Err(ChannelError::Json));
    assert_eq!(
        Channel::from_identifier("WyJhIiwiZXZpbC5leGFtcGxlQHgiLCIvcCJd").map(|_| ()),
        Err(ChannelError::Address)
    );
    assert_eq!(Channel::from_identifier("eyJhIjoxfQ==").map(|_| ()), Err(ChannelError::Json));
    assert_eq!(Channel::from_address("ws://zxnet.co.uk/ws/x", "x").map(|_| ()), Err(ChannelError::Address));
}

/// The server began sending "channelSettings" with the channel's clock face
/// (88,311 bytes on 10 October 2026), more than the message buffer holds.
/// It is skipped, and the page data after it still arrives.
#[test]
fn oversized_messages_are_skipped_not_fatal() {
    let mut harness = connected("teefax");
    let settings = format!(r#"["channelSettings", {{"clock": "{}"}}]"#, "x".repeat(88_311));
    harness.server(&[&settings, r#"["secondTick"]"#, HEADER, ROW]);
    let records = harness.take();
    assert!(has_event(&records, "Ignored(TooLong)"), "{records:?}");
    assert!(!has_event(&records, "Reconnecting"), "{records:?}");
    assert_eq!(packets(&records).len(), 2);
}

#[test]
fn bsdp_becomes_packet_8_30() {
    let mut harness = connected("teefax");
    harness.server(&[BSDP]);
    let records = harness.take();
    let got = packets(&records);
    assert_eq!(got.len(), 1);
    let Ok(Decoded::Service(service)) = decode(&got[0]) else { panic!("{:?}", decode(&got[0])) };
    let address = neoteletext_core::packet::decode_address(&got[0]).unwrap();
    assert_eq!((address.magazine.number(), address.number), (8, 30));
    let info = neoteletext_core::service::interpret(&service);
    let utc = info.utc.unwrap();
    assert_eq!((utc.year, utc.month, utc.day, utc.hour, utc.minute, utc.second), (2026, 10, 9, 22, 47, 44));
}

#[test]
fn header_and_row_are_rebuilt_and_cross_checked() {
    let mut harness = connected("teefax");
    harness.server(&[HEADER, ROW, CLOCK]);
    let records = harness.take();
    let got = packets(&records);
    assert_eq!(got.len(), 2);
    let Ok(Decoded::Header(header)) = decode(&got[0]) else { panic!() };
    assert_eq!(header.magazine.number(), 1);
    assert_eq!(header.page.map(|p| p.0), Some(104));
    assert_eq!(header.subcode.map(neoteletext_core::types::SubCode::value), Some(3));
    assert_eq!(header.control.map(|c| c.0), Some(16));
    assert!(!has_event(&records, "HeaderMismatch"), "{records:?}");
    let Ok(Decoded::Row(row)) = decode(&got[1]) else { panic!() };
    assert_eq!((row.magazine.number(), row.row), (1, 1));
    assert!(has_event(&records, "Clock("));
    // A header whose server fields disagree with its bytes is reported.
    harness.server(&[
        r#"["header", 0, "0Dhe0BUVFRVURUVGwVggMbY4ICBG8ukgILkgT+P0IIMys7o0Ny80sw==", false, 48, 1, 3, 105]"#,
    ]);
    let records = harness.take();
    assert!(has_event(&records, "HeaderMismatch { field: Page, server: 105, decoded: Some(104) }"), "{records:?}");
    assert!(has_event(&records, "HeaderMismatch { field: Control, server: 48, decoded: Some(16) }"));
}

#[test]
fn client_messages() {
    let mut harness = connected("ch4");
    harness.server(&[r#"["apiver", 1]"#]);
    harness.input(LiveInput::Request { page: PageAddress::from_mpp(0x8FF).unwrap(), subcode: None });
    let messages = harness.client_messages();
    let identifier = Channel::from_short_name("ch4").unwrap().identifier().to_string();
    assert_eq!(
        messages,
        [
            format!("service,{identifier}"),
            "ttx,true".to_string(),
            "apiver,1".to_string(),
            "pagesearch,0,8,255,16255,true,false,false".to_string()
        ]
    );
    assert!(has_event(&harness.take(), "ApiVersion(1)"));
    // Keepalive every 5 seconds, while clock messages keep it alive.
    for _ in 0..3 {
        harness.now = Instant(harness.now.0 + SECOND * 5 / 2);
        harness.server(&[CLOCK]);
        harness.advance(0);
    }
    assert_eq!(harness.client_messages().iter().filter(|m| *m == "keepalive").count(), 1);
    // The handshake request is a plain WebSocket upgrade with no cookies.
    let handshake = &harness.pump.transport.requests_to("zxnet.co.uk:443")[0];
    assert!(String::from_utf8_lossy(handshake).starts_with("GET /ws/ch4 HTTP/1.1\r\nHost: zxnet.co.uk\r\n"));
}

#[test]
fn unknown_and_malformed_messages_are_ignored() {
    let mut harness = connected("teefax");
    harness.server(&[
        r#"["mystery", 1, 2, {"x": [1]}]"#,
        r#"["channelSettings", {"image": {"url": "x"}}]"#,
        r#"["secondTick"]"#,
        r#"["row", 0, 1, 0, "kSBwcHCScHBwlHBwcJJ8fCwsLOwsLOwsLOwsLOy8LCzsvCx8LOx8fA=="]"#,
        r#"["row", 0, 9, 1, "kSBwcHCScHBwlHBwcJJ8fCwsLOwsLOwsLOwsLOy8LCzsvCx8LOx8fA=="]"#,
        r#"["row", 0, 1, 1, "kSBw"]"#,
        r#"["bsdp", 7]"#,
        "not json",
        r#"{"a": 1}"#,
        r#"["row", 0, 1, 1, "kSBwcHCScHBwlHBwcJJ8fCwsLOwsLOwsLOwsLOy8LCzsvCx8LOx8fA==", "extra", 5]"#,
    ]);
    harness.pump.transport.inject(ConnectionId::Live, &server_frame(0x2, b"\x00\x01"));
    harness.settle();
    let records = harness.take();
    assert_eq!(packets(&records).len(), 1, "only the last row, whose extra fields are ignored");
    let ignored = events(&records);
    assert_eq!(ignored.iter().filter(|e| **e == "Ignored(Fields)").count(), 3, "{ignored:?}");
    assert_eq!(ignored.iter().filter(|e| **e == "Ignored(PacketData)").count(), 1);
    assert_eq!(ignored.iter().filter(|e| **e == "Ignored(Json)").count(), 2);
    assert_eq!(ignored.iter().filter(|e| **e == "Ignored(Binary)").count(), 1);
}

#[test]
fn inactivity_reconnects_then_gives_up() {
    let mut harness = connected("teefax");
    harness.advance(5 * SECOND);
    assert!(has_event(&harness.take(), "Reconnecting { attempt: 1, in_us: 10000000 }"));
    harness.advance(10 * SECOND);
    assert!(has_event(&harness.take(), "Connected"));
    // A header resets the count of failed attempts.
    harness.server(&[HEADER]);
    for attempt in 1..=4 {
        harness.advance(5 * SECOND);
        assert!(has_event(&harness.take(), &format!("Reconnecting {{ attempt: {attempt},")), "attempt {attempt}");
        harness.advance(10 * SECOND);
    }
    harness.advance(5 * SECOND);
    assert!(has_event(&harness.take(), "GaveUp"));
    let connects = harness.pump.transport.connects.len();
    harness.advance(60 * SECOND);
    assert_eq!(harness.pump.transport.connects.len(), connects, "no more attempts");
}

#[test]
fn wrong_name_stops_reconnection() {
    let mut harness = connected("teefax");
    harness.server(&[r#"["wrongname"]"#]);
    assert!(has_event(&harness.take(), "WrongName"));
    let connects = harness.pump.transport.connects.len();
    harness.advance(30 * SECOND);
    assert_eq!(harness.pump.transport.connects.len(), connects);
}

#[test]
fn blocked_peer() {
    let mut replay = Replay::new(&[("zxnet.co.uk", IpAddress::V4([10, 0, 0, 1]))]);
    replay.reply("zxnet.co.uk:443", Reply::WebSocket(Vec::new()));
    let mut harness = Harness::new(&Channel::from_short_name("teefax").unwrap(), replay);
    harness.input(LiveInput::Start);
    assert!(has_event(&harness.take(), "Blocked"));
    assert_eq!(harness.pump.transport.requests, []);
}

#[test]
fn page_walk_holds_each_page_until_its_rows_arrive() {
    let mut harness = connected("teefax");
    harness.server(&[r#"["pageExists", 0, 1, 0]"#, r#"["pageExists", 0, 1, 104]"#, r#"["pageExists", 0, 8, 136]"#]);
    harness.input(LiveInput::Walk(true));
    assert!(has_event(&harness.take(), "WalkPage(PageAddress { magazine: Magazine(1), page: PageNumber(0) })"));
    assert_eq!(harness.client_messages().last().unwrap(), "pagesearch,0,1,0,16255,true,false,false");
    // A header for another page, then page 100's header and rows.
    harness.server(&[HEADER]);
    harness.server(&[
        r#"["header", 0, "FRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFQ==", false, 0, 1, 0, 0]"#,
        ROW,
        ROW,
    ]);
    harness.advance(SECOND / 2);
    assert!(!has_event(&harness.take(), "WalkPageDone"), "held while rows may still come");
    harness.server(&[CLOCK]);
    harness.advance(SECOND);
    let records = harness.take();
    assert!(
        has_event(
            &records,
            "WalkPageDone { page: PageAddress { magazine: Magazine(1), page: PageNumber(0) }, rows: 2, timed_out: false }"
        ),
        "{records:?}"
    );
    assert!(has_event(&records, "WalkPage(PageAddress { magazine: Magazine(1), page: PageNumber(104) })"));
    // No rows for 1/104: it is left after the timeout. Clock messages keep
    // the connection alive meanwhile.
    for _ in 0..12 {
        harness.server(&[CLOCK]);
        harness.advance(SECOND);
    }
    let records = harness.take();
    assert!(
        has_event(
            &records,
            "WalkPageDone { page: PageAddress { magazine: Magazine(1), page: PageNumber(104) }, rows: 0, timed_out: true }"
        ),
        "{records:?}"
    );
    assert!(
        has_event(&records, "WalkPage(PageAddress { magazine: Magazine(8), page: PageNumber(136) })"),
        "magazine 8 last"
    );
    for _ in 0..13 {
        harness.server(&[CLOCK]);
        harness.advance(SECOND);
    }
    assert!(has_event(&harness.take(), "WalkComplete"));
    assert!(harness.feed.page_exists(PageAddress::from_mpp(0x888).unwrap()));
}

/// Replays a recorded session from the corpus when one is present
/// (REQ-TST-063; REQ-TST-022 keeps captures out of the repository). Each
/// line of `$NEOTELETEXT_CORPUS/zxnet/*.jsonl` is one server message.
#[test]
fn recorded_sessions_from_the_corpus() {
    let Ok(corpus) = std::env::var("NEOTELETEXT_CORPUS") else { return };
    let Ok(entries) = std::fs::read_dir(std::path::Path::new(&corpus).join("zxnet")) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let mut harness = connected("teefax");
        let mut expected = 0;
        for line in text.lines() {
            if line.starts_with("[\"header\"") || line.starts_with("[\"row\"") || line.starts_with("[\"bsdp\"") {
                expected += 1;
            }
            harness.pump.transport.inject(ConnectionId::Live, &server_frame(0x1, line.as_bytes()));
            harness.settle();
        }
        let records = harness.take();
        let got = packets(&records);
        let mismatches = events(&records).iter().filter(|e| e.starts_with("HeaderMismatch")).count();
        let ignored = events(&records).iter().filter(|e| e.starts_with("Ignored")).count();
        std::println!(
            "{}: {} packets of {expected}, {mismatches} header mismatches, {ignored} ignored",
            path.display(),
            got.len()
        );
        assert_eq!(got.len(), expected, "{}", path.display());
        assert!(got.iter().all(|packet| decode(packet).is_ok()));
    }
}
