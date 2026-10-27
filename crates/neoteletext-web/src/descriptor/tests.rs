#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
//! The tests of the service descriptor and a channel's settings.

use super::*;
use neoteletext_core::types::SubCode;

const VALID: &str = r#"{
        "name": "Example Teletext",
        "initial_page": "100",
        "page_template": "/ttx/pages/{mpp}/{sub}?m={m}&p={pp}",
        "push": { "address": "wss://example.org/ttx/push", "method": "websocket" },
        "region": 2,
        "services": [
            { "name": "weather", "display_name": "Weather é", "descriptor": "https://weather.example/ttx.json" },
            { "name": "news-2", "display_name": "News", "descriptor": "/other/ttx.json" }
        ]
    }"#;

fn base() -> Url {
    Url::parse("https://example.org/ttx/service.json").unwrap()
}

#[test]
fn valid_descriptor() {
    let descriptor = Descriptor::parse(VALID.as_bytes(), &base()).unwrap();
    assert_eq!(descriptor.name.as_str(), "Example Teletext");
    assert_eq!(descriptor.initial_page.mpp(), 0x100);
    assert_eq!(descriptor.region, Some(2));
    let page = PageAddress::from_mpp(0x8A5).unwrap();
    assert_eq!(
        descriptor.page_url(page, Some(SubCode::new(0x12))).unwrap().as_str(),
        "https://example.org/ttx/pages/8A5/0012?m=8&p=A5"
    );
    assert_eq!(descriptor.page_url(page, None).unwrap().as_str(), "https://example.org/ttx/pages/8A5/0000?m=8&p=A5");
    let (push, method) = descriptor.push().unwrap();
    assert_eq!((push.as_str(), *method), ("wss://example.org/ttx/push", PushMethod::WebSocket));
    let weather = descriptor.service("weather").unwrap();
    assert_eq!(weather.display_name.as_str(), "Weather \u{e9}");
    assert_eq!(descriptor.service_address(weather), "https://weather.example/ttx.json");
    let news = descriptor.service("news-2").unwrap();
    assert_eq!(descriptor.service_address(news), "https://example.org/other/ttx.json");
    assert!(descriptor.service("sport").is_none());
}

#[test]
fn minimal_descriptor() {
    let json = br#"{"name":"S","initial_page":"1A0","page_template":"http://s.example/{mpp}"}"#;
    let base = Url::parse("http://s.example/d.json").unwrap();
    let descriptor = Descriptor::parse(json, &base).unwrap();
    assert!(descriptor.push().is_none());
    assert_eq!(descriptor.region, None);
}

#[test]
fn malformed_descriptors() {
    let plain = Url::parse("http://s.example/d.json").unwrap();
    let cases: [(&str, DescriptorError); 20] = [
        ("[]", DescriptorError::Invalid(Member::Document)),
        (r#"{"initial_page":"100","page_template":"/{mpp}"}"#, DescriptorError::Missing(Member::Name)),
        (r#"{"name":"S","page_template":"/{mpp}"}"#, DescriptorError::Missing(Member::InitialPage)),
        (r#"{"name":"S","initial_page":"100"}"#, DescriptorError::Missing(Member::PageTemplate)),
        (
            r#"{"name":"S","name":"T","initial_page":"100","page_template":"/{mpp}"}"#,
            DescriptorError::Duplicate(Member::Name),
        ),
        (r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","x":1}"#, DescriptorError::Unknown),
        (
            r#"{"name":"S","initial_page":"900","page_template":"/{mpp}"}"#,
            DescriptorError::Invalid(Member::InitialPage),
        ),
        (r#"{"name":"S","initial_page":100,"page_template":"/{mpp}"}"#, DescriptorError::Invalid(Member::InitialPage)),
        (r#"{"name":"","initial_page":"100","page_template":"/{mpp}"}"#, DescriptorError::Invalid(Member::Name)),
        (r#"{"name":"S","initial_page":"100","page_template":"/{page}"}"#, DescriptorError::Placeholder),
        (r#"{"name":"S","initial_page":"100","page_template":"/{mpp"}"#, DescriptorError::Placeholder),
        (r#"{"name":"S","initial_page":"100","page_template":"/mpp}"}"#, DescriptorError::Placeholder),
        (r#"{"name":"S","initial_page":"100","page_template":"http://1.2.3.{pp}/x"}"#, DescriptorError::Placeholder),
        (r#"{"name":"S","initial_page":"100","page_template":"//p{m}.example/x"}"#, DescriptorError::Placeholder),
        (
            r#"{"name":"S","initial_page":"100","page_template":"ftp://s.example/{mpp}"}"#,
            DescriptorError::Address(Member::PageTemplate, UrlError::Scheme),
        ),
        (
            r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","region":16}"#,
            DescriptorError::Invalid(Member::Region),
        ),
        (
            r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","push":{"address":"/p","method":"websocket"}}"#,
            DescriptorError::Invalid(Member::PushAddress),
        ),
        (
            r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","push":{"address":"ws://s.example/p","method":"poll"}}"#,
            DescriptorError::Invalid(Member::PushMethod),
        ),
        (
            r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","services":[{"name":"a b","display_name":"A","descriptor":"/a"}]}"#,
            DescriptorError::Invalid(Member::ServiceName),
        ),
        (
            r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","services":[{"name":"a","descriptor":"/a"}]}"#,
            DescriptorError::Missing(Member::ServiceDisplayName),
        ),
    ];
    for (json, error) in cases {
        assert_eq!(Descriptor::parse(json.as_bytes(), &plain).map(|d| d.initial_page), Err(error), "{json}");
    }
    assert!(matches!(Descriptor::parse(b"{\"name\":", &plain), Err(DescriptorError::Json(_))));
}

/// Decision E.90: `push.input` says the channel takes keys back, and
/// only a WebSocket can carry them.
#[test]
fn input_needs_a_websocket() {
    let plain = Url::parse("http://s.example/service.json").unwrap();
    let with =
        |push: &str| std::format!(r#"{{"name":"S","initial_page":"100","page_template":"/{{mpp}}","push":{push}}}"#);
    let parse = |push: &str| Descriptor::parse(with(push).as_bytes(), &plain).map(|descriptor| descriptor.input());
    assert_eq!(parse(r#"{"address":"ws://s.example/p","method":"websocket","input":true}"#), Ok(true));
    assert_eq!(parse(r#"{"address":"ws://s.example/p","method":"websocket","input":false}"#), Ok(false));
    assert_eq!(parse(r#"{"address":"ws://s.example/p","method":"websocket"}"#), Ok(false));
    assert_eq!(
        parse(r#"{"address":"http://s.example/p","method":"sse","input":true}"#),
        Err(DescriptorError::Invalid(Member::PushInput))
    );
    assert_eq!(
        parse(r#"{"address":"ws://s.example/p","method":"websocket","input":"yes"}"#),
        Err(DescriptorError::Invalid(Member::PushInput))
    );
    assert_eq!(
        parse(r#"{"address":"ws://s.example/p","method":"websocket","input":true,"input":true}"#),
        Err(DescriptorError::Duplicate(Member::PushInput))
    );
    assert!(!Descriptor::parse(VALID.as_bytes(), &base()).unwrap().input());
}

/// Finding F-90 (INPUT.md section 4): keys go back only to a push
/// WebSocket on the descriptor's own origin. A service's descriptor could
/// send them to any origin; a channel's push was already held to its own.
#[test]
fn input_goes_only_to_the_descriptors_own_origin() {
    let plain = Url::parse("http://s.example/service.json").unwrap();
    let with =
        |push: &str| std::format!(r#"{{"name":"S","initial_page":"100","page_template":"/{{mpp}}","push":{push}}}"#);
    let parse = |push: &str| Descriptor::parse(with(push).as_bytes(), &plain).map(|descriptor| descriptor.input());
    assert_eq!(
        parse(r#"{"address":"ws://keys.example/p","method":"websocket","input":true}"#),
        Err(DescriptorError::Invalid(Member::PushInput))
    );
    assert_eq!(
        parse(r#"{"address":"ws://s.example:8080/p","method":"websocket","input":true}"#),
        Err(DescriptorError::Invalid(Member::PushInput)),
        "another port is another origin"
    );
    assert_eq!(parse(r#"{"address":"ws://keys.example/p","method":"websocket"}"#), Ok(false), "push alone may");
}

#[test]
fn https_descriptor_cannot_name_plain_addresses() {
    let cases: [(&str, Member); 3] = [
        (r#"{"name":"S","initial_page":"100","page_template":"http://example.org/{mpp}"}"#, Member::PageTemplate),
        (
            r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","push":{"address":"ws://example.org/p","method":"websocket"}}"#,
            Member::PushAddress,
        ),
        (
            r#"{"name":"S","initial_page":"100","page_template":"/{mpp}","services":[{"name":"a","display_name":"A","descriptor":"http://a.example/"}]}"#,
            Member::ServiceDescriptor,
        ),
    ];
    for (json, member) in cases {
        assert_eq!(
            Descriptor::parse(json.as_bytes(), &base()).map(|d| d.initial_page),
            Err(DescriptorError::Downgrade(member)),
            "{json}"
        );
    }
}

/// the web-native concept section 5 item 4: aliases lead a page
/// number to a named service's page or to an address.
#[test]
fn aliases() {
    let json = r#"{"name":"A","initial_page":"100","page_template":"/{mpp}.t42",
            "aliases":{"300":"weather:300","400":"https://sport.example/400.t42"}}"#;
    let descriptor = Descriptor::parse(json.as_bytes(), &base()).unwrap();
    let target = |mpp| descriptor.alias(PageAddress::from_mpp(mpp).unwrap()).map(|a| descriptor.alias_target(a));
    assert_eq!(target(0x300), Some("weather:300"));
    assert_eq!(target(0x400), Some("https://sport.example/400.t42"));
    assert_eq!(target(0x500), None);
}

#[test]
fn refresh_interval() {
    let head = r#"{"name":"A","initial_page":"100","page_template":"/{mpp}.t42","refresh":"#;
    let parse = |value: &str| Descriptor::parse(std::format!("{head}{value}}}").as_bytes(), &base());
    assert_eq!(parse("60").unwrap().refresh_seconds, Some(60));
    for bad in ["1", "100000", "\"60\"", "-5", "6.5"] {
        assert_eq!(parse(bad).map(|_| ()), Err(DescriptorError::Invalid(Member::Refresh)), "{bad}");
    }
}

#[test]
fn bad_aliases_reject_the_descriptor() {
    let head = r#"{"name":"A","initial_page":"100","page_template":"/{mpp}.t42","aliases":"#;
    for (aliases, error) in [
        (r#"{"30":"w:300"}"#, DescriptorError::Invalid(Member::Aliases)),
        (r#"{"300":"w:3000"}"#, DescriptorError::Invalid(Member::Aliases)),
        (r#"{"300":"w w:300"}"#, DescriptorError::Invalid(Member::Aliases)),
        (r#"{"300":"http://plain.example/300.t42"}"#, DescriptorError::Downgrade(Member::Aliases)),
        (r#"{"300":"wss://x.example/300.t42"}"#, DescriptorError::Invalid(Member::Aliases)),
        (r#"{"300":"w:300","300":"w:301"}"#, DescriptorError::Duplicate(Member::Aliases)),
        (r#"["300"]"#, DescriptorError::Invalid(Member::Aliases)),
        // Without `://` a target is `name:mpp`; a path alone is neither.
        (r#"{"300":"/local/300.t42"}"#, DescriptorError::Invalid(Member::Aliases)),
    ] {
        let json = std::format!("{head}{aliases}}}");
        assert_eq!(Descriptor::parse(json.as_bytes(), &base()).map(|_| ()), Err(error), "{aliases}");
    }
    let many: std::vec::Vec<std::string::String> =
        (0..=MAX_ALIASES).map(|n| std::format!(r#""{:03X}":"w:100""#, 0x100 + n)).collect();
    let json = std::format!("{head}{{{}}}}}", many.join(","));
    assert_eq!(Descriptor::parse(json.as_bytes(), &base()).map(|_| ()), Err(DescriptorError::TooMany(Member::Aliases)));
}

/// Decision E.111: `font`, which named a channel's font, is withdrawn,
/// and refused as every unknown member is (REQ-WEB-024).
#[test]
fn a_channel_font_is_an_unknown_member() {
    let base = Url::parse("https://example.org/ceefax/channel.json").unwrap();
    let sha = "5ee6246b7f9bd197f47ce0741ea799a55ac08af775869a08c085803e02a3f14d";
    let json = std::format!(r#"{{"font":{{"address":"teletext.bdf","sha256":"{sha}"}}}}"#);
    assert_eq!(Descriptor::parse_channel(json.as_bytes(), &base).err(), Some(DescriptorError::Unknown));
}

/// REQ-WEB-024: every member optional, no template, linked channels by
/// folder, push on the channel's own origin.
#[test]
fn channel_settings() {
    let base = Url::parse("https://example.org/ceefax/channel.json").unwrap();
    let empty = Descriptor::parse_channel(b"{}", &base).unwrap();
    assert!(empty.is_channel());
    assert_eq!(empty.initial_page.mpp(), 0x100);
    assert_eq!(empty.name.as_str(), "example.org/ceefax");
    assert_eq!(
        Descriptor::parse_channel(br#"{"page_template":"/{mpp}.t42"}"#, &base).err(),
        Some(DescriptorError::NotForChannels(Member::PageTemplate))
    );
    assert_eq!(
        Descriptor::parse_channel(br#"{"push":{"address":"wss://other.example/push","method":"websocket"}}"#, &base)
            .err(),
        Some(DescriptorError::Invalid(Member::PushAddress))
    );
    assert!(
        Descriptor::parse_channel(br#"{"push":{"address":"wss://example.org/live/push","method":"websocket"}}"#, &base)
            .is_ok(),
        "push may be in any folder of the origin"
    );
    let both = br#"{"services":[{"name":"w","display_name":"W","channel":"https://w.example/","descriptor":"https://w.example/d.json"}]}"#;
    assert!(Descriptor::parse_channel(both, &base).is_err());
    let file = br#"{"services":[{"name":"w","display_name":"W","channel":"https://w.example/100.tti"}]}"#;
    assert_eq!(Descriptor::parse_channel(file, &base).err(), Some(DescriptorError::Invalid(Member::ServiceChannel)));
    assert_eq!(Descriptor::parse_channel(br#"{"colour":"red"}"#, &base).err(), Some(DescriptorError::Unknown));
    // A service's descriptor may not name a channel this way.
    let named = br#"{"name":"S","initial_page":"100","page_template":"/{mpp}","services":[{"name":"w","display_name":"W","channel":"https://w.example/"}]}"#;
    assert_eq!(Descriptor::parse(named, &base).err(), Some(DescriptorError::Unknown));
}
