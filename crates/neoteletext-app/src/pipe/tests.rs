#![allow(clippy::unwrap_used, reason = "a test")]
use super::*;

#[test]
fn frames_round_trip_and_bad_frames_are_rejected() {
    let frames = [
        Frame::Packet([7; 42]),
        Frame::Tti(PageData {
            secure: true,
            origin: Origin::Form,
            url: "https://a/b".into(),
            bytes: b"PN,10000\n".to_vec(),
        }),
        Frame::T42(PageData { secure: false, origin: Origin::Push, url: String::new(), bytes: vec![1, 2, 3] }),
        Frame::Notice(Notice::InitialPage(0x100)),
        Frame::Notice(Notice::Unencrypted),
        Frame::Notice(Notice::Input(true)),
        Frame::Input(vec![0x61, 0xFF, 0x01, 0x04, 0x02]),
        Frame::Notice(Notice::Message("hello".into())),
        Frame::Notice(Notice::Service(vec!["other".into(), "weather".into()])),
        Frame::Notice(Notice::Confirm("example.org".into(), true, false)),
        Frame::Notice(Notice::Submitted(true)),
        Frame::Notice(Notice::ShowPage(0x301)),
        Frame::Notice(Notice::Current(0x100)),
        Frame::Notice(Notice::Offline(0x8FF)),
        Frame::Request(0x123, 0xFFFF),
        Frame::Submit(Submission {
            page_url: "http://x/p".into(),
            post: true,
            target: "/contact".into(),
            token: Some("t".into()),
            action: None,
            fields: vec![("name".into(), "Ann".into())],
        }),
        Frame::Confirm(false),
        Frame::Follow("other".into(), 0x400, 0xFFFF),
        Frame::Tti(PageData { secure: true, origin: Origin::FrontDoor, url: "h".into(), bytes: vec![] }),
        Frame::Notice(Notice::RedirectToOtherChannel("https://elsewhere.example/".into())),
        Frame::Notice(Notice::ServiceName("Test".into())),
        Frame::Notice(Notice::Bundle(0x199)),
        Frame::Notice(Notice::Aliases(vec![(0x300, "weather:300".into())])),
        Frame::Notice(Notice::Refresh(60)),
    ];
    let mut buffer = Vec::new();
    for frame in &frames {
        write(&mut buffer, frame).unwrap();
    }
    let mut reader = buffer.as_slice();
    for frame in &frames {
        assert_eq!(read(&mut reader).unwrap().as_ref(), Some(frame));
    }
    assert_eq!(read(&mut reader).unwrap(), None);
    for bad in [
        &[b'T', 0xFF, 0xFF, 0xFF, 0xFF][..],
        &[b'X', 0, 0, 0, 0],
        &[b'P', 0, 0, 0, 1, 0],
        &[b'S', 0, 0, 0, 2, 0, 9],
        &[b'L', 0, 0, 0, 8, 1, 0, 0xFF, 0xFF, 0, 0, 0, 9],
        &[b'C', 0, 0, 0, 2, 1, 1],
        // Retired kinds are unknown now: the glyph choice, and the
        // browser's input and screen.
        &[b'Z', 0, 0, 0, 1, 0],
        &[b'k', 0, 0, 0, 5, 0, 0, 0, 0, 1],
        &[b'Y', 0, 0, 0, 6, 0, 1, 0, 1, 0, 0],
    ] {
        assert!(read(&mut &bad[..]).is_err(), "{bad:?}");
    }
}
