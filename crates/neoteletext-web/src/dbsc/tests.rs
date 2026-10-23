#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, reason = "a test")]
//! The tests of Device Bound Session Credentials' records: the headers,
//! the instructions, the proofs and the session store.

use super::*;
use crate::bytes::{TextBuf, Writer};
use crate::url::Url;
use std::string::String;
use std::vec::Vec;

#[test]
fn registration_offers_are_read() {
    let read = read_registration(br#"(ES256 RS256);path="/dbsc/start";challenge="c-1";authorization="code""#).unwrap();
    assert_eq!(read.path.as_str(), "/dbsc/start");
    assert_eq!(read.challenge.unwrap().as_str(), "c-1");
    assert_eq!(read.authorization.unwrap().as_str(), "code");
    // The first member offering ES256 is taken; others are read past.
    let read = read_registration(br#"(RS256);path="/rsa", (ES256);path="/ec";x=?1;n=5"#).unwrap();
    assert_eq!(read.path.as_str(), "/ec");
    assert_eq!(read.challenge, None);
    let escaped = read_registration(br#"(ES256);path="/a";challenge="q\"b\\c""#).unwrap();
    assert_eq!(escaped.challenge.unwrap().as_str(), r#"q"b\c"#);
    for refused in [
        &b"(RS256);path=\"/rsa\""[..],
        b"(ES256)",
        b"(ES256);path=/bare",
        b"ES256;path=\"/x\"",
        b"(ES256);path=\"/x\",",
        b"(ES256);path=\"unterminated",
        b"(ES256);path=\"tab\there\"",
        b"(ES256);challenge=\"\\x\";path=\"/x\"",
    ] {
        assert_eq!(read_registration(refused), None, "{}", String::from_utf8_lossy(refused));
    }
    let long = [b"(ES256);path=\"".as_slice(), &[b'a'; MAX_SESSION_URL + 1], b"\""].concat();
    assert_eq!(read_registration(&long), None, "a path longer than kept");
}

#[test]
fn challenges_name_their_session() {
    let read = read_challenge(br#""fresh";id="s1""#).unwrap();
    assert_eq!((read.challenge.as_str(), read.session.as_str()), ("fresh", "s1"));
    assert_eq!(read_challenge(br#""fresh""#), None, "no id");
    assert_eq!(read_challenge(br#""fresh";id=s1"#), None, "a token id");
    assert_eq!(read_challenge(br#""a", "b";id="s""#), None, "a list");
}

const INSTRUCTIONS: &[u8] = br#"{
        "session_identifier": "s1",
        "refresh_url": "/dbsc/refresh",
        "scope": {"origin": "https://news.example", "include_site": false,
                  "scope_specification": [{"type": "exclude", "domain": "news.example", "path": "/public"}]},
        "credentials": [{"type": "cookie", "name": "auth", "attributes": "Domain=news.example; Secure"}],
        "allowed_refresh_initiators": ["*.news.example"],
        "newer_member": {"ignored": [1, 2, 3]}
    }"#;

#[test]
fn instructions_are_read() {
    let Ok(Answer::Continue(read)) = read_instructions(INSTRUCTIONS) else { panic!() };
    assert_eq!(read.id.as_str(), "s1");
    assert_eq!(read.refresh_url.unwrap().as_str(), "/dbsc/refresh");
    assert_eq!(read.origin.unwrap().as_str(), "https://news.example");
    assert!(!read.include_site);
    let rule = read.rules[0].unwrap();
    assert_eq!((rule.include, rule.domain.as_str(), rule.path.as_str()), (false, "news.example", "/public"));
    assert_eq!(read.credentials[0].unwrap().as_str(), "auth");
    assert_eq!(read_instructions(br#"{"continue": false}"#), Ok(Answer::End));
}

#[test]
fn instructions_outside_the_profile_are_refused() {
    let base =
        |members: &str| std::format!(r#"{{"session_identifier":"s","scope":{{"include_site":true}},{members}}}"#);
    for (body, why) in [
        (String::from("{"), InstructionsError::Malformed),
        (String::from(r#"{"continue":true} trailing"#), InstructionsError::Malformed),
        (base(r#""credentials":[]"#), InstructionsError::Missing),
        (base(r#""credentials":[{"type":"token","name":"a"}]"#), InstructionsError::Credential),
        (base(r#""credentials":[{"type":"cookie","name":""}]"#), InstructionsError::Credential),
        (
            base(r#""credentials":[{"type":"cookie","name":"a","attributes":"Secure; Partitioned"}]"#),
            InstructionsError::Credential,
        ),
        (
            r#"{"scope":{"include_site":true},"credentials":[{"type":"cookie","name":"a"}]}"#.into(),
            InstructionsError::Missing,
        ),
        (
            r#"{"session_identifier":"s","scope":{},"credentials":[{"type":"cookie","name":"a"}]}"#.into(),
            InstructionsError::Missing,
        ),
        (
            base(&std::format!(
                r#""credentials":[{}]"#,
                [r#"{"type":"cookie","name":"a"}"#; MAX_CREDENTIALS + 1].join(",")
            )),
            InstructionsError::TooLarge,
        ),
    ] {
        assert_eq!(read_instructions(body.as_bytes()), Err(why), "{body}");
    }
    let long = std::format!(r#"{{"session_identifier":"{}"}}"#, "a".repeat(MAX_SESSION_ID + 1));
    assert_eq!(read_instructions(long.as_bytes()), Err(InstructionsError::TooLarge));
}

/// A key store that signs with a stand-in signature and records what
/// it signed.
#[derive(Debug)]
struct Keys {
    signed: Vec<Vec<u8>>,
}

impl DeviceKeys for Keys {
    fn create(&mut self) -> Option<(KeyHandle, PublicKey)> {
        Some((KeyHandle(7), PublicKey { x: [1; COORDINATE_BYTES], y: [2; COORDINATE_BYTES] }))
    }
    fn sign(&mut self, key: KeyHandle, message: &[u8]) -> Option<[u8; SIGNATURE_BYTES]> {
        assert_eq!(key, KeyHandle(7));
        self.signed.push(message.to_vec());
        Some([0xAB; SIGNATURE_BYTES])
    }
    fn delete(&mut self, _: KeyHandle) {}
}

fn unbase64url(text: &[u8]) -> Vec<u8> {
    let mut standard: Vec<u8> = text
        .iter()
        .map(|&byte| match byte {
            b'-' => b'+',
            b'_' => b'/',
            other => other,
        })
        .collect();
    while !standard.len().is_multiple_of(4) {
        standard.push(b'=');
    }
    let mut out = std::vec![0_u8; standard.len()];
    let length = crate::base64::decode(&standard, &mut out).unwrap();
    out.truncate(length);
    out
}

#[test]
fn proofs_are_compact_jws_signed_by_the_port() {
    let mut keys = Keys { signed: Vec::new() };
    let (key, public) = keys.create().unwrap();
    let mut buffer = [0_u8; 2048];
    let mut out = Writer::new(&mut buffer);
    out.put(b"Secure-Session-Response: ").unwrap();
    let start = out.len();
    write_proof(&mut keys, key, Some(&public), Some(br#"c"1"#), Some(b"code"), &mut out).unwrap();
    let jws = out.written()[start..].to_vec();
    let parts: Vec<&[u8]> = jws.split(|&byte| byte == b'.').collect();
    assert_eq!(parts.len(), 3);
    let header = String::from_utf8(unbase64url(parts[0])).unwrap();
    let x = String::from_utf8(parts[0].to_vec()).unwrap();
    assert!(!x.contains('=') && !x.contains('+') && !x.contains('/'), "base64url without padding");
    assert_eq!(
        header,
        std::format!(
            r#"{{"alg":"ES256","typ":"dbsc+jwt","jwk":{{"kty":"EC","crv":"P-256","x":"{x}","y":"{y}"}}}}"#,
            x = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE",
            y = "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI"
        )
    );
    assert_eq!(String::from_utf8(unbase64url(parts[1])).unwrap(), r#"{"jti":"c\"1","authorization":"code"}"#);
    assert_eq!(unbase64url(parts[2]), [0xAB; SIGNATURE_BYTES]);
    // What was signed is the JWS signing input (RFC 7515 section 5.1).
    assert_eq!(keys.signed, [[parts[0], b".", parts[1]].concat()]);

    let mut buffer = [0_u8; 512];
    let mut out = Writer::new(&mut buffer);
    write_proof(&mut keys, key, None, Some(b"r"), None, &mut out).unwrap();
    let parts: Vec<Vec<u8>> = out.written().split(|&byte| byte == b'.').map(<[u8]>::to_vec).collect();
    assert_eq!(String::from_utf8(unbase64url(&parts[0])).unwrap(), r#"{"alg":"ES256","typ":"dbsc+jwt"}"#);
    assert_eq!(String::from_utf8(unbase64url(&parts[1])).unwrap(), r#"{"jti":"r"}"#);
}

fn session(include_site: bool, rules: &[(bool, &str, &str)]) -> Session {
    let mut kept = [None; MAX_SCOPE_RULES];
    for (slot, &(include, domain, path)) in kept.iter_mut().zip(rules) {
        *slot = Some(ScopeRule {
            include,
            domain: TextBuf::from_bytes(domain.as_bytes()).unwrap(),
            path: TextBuf::from_bytes(path.as_bytes()).unwrap(),
        });
    }
    let mut credentials = [None; MAX_CREDENTIALS];
    credentials[0] = TextBuf::from_bytes(b"auth");
    Session {
        id: TextBuf::from_bytes(b"s1").unwrap(),
        key: KeyHandle(7),
        refresh_url: TextBuf::from_bytes(b"https://news.example/dbsc/refresh").unwrap(),
        origin: Url::parse("https://news.example/").unwrap().origin(),
        include_site,
        rules: kept,
        credentials,
        challenge: None,
        expires: 100,
    }
}

#[test]
fn scope_is_the_origin_or_the_site_less_what_rules_exclude() {
    let covers = |session: &Session, url: &str| session.covers(&Url::parse(url).unwrap());
    let origin = session(false, &[]);
    assert!(covers(&origin, "https://news.example/p/100.tti"));
    assert!(!covers(&origin, "https://www.news.example/p/100.tti"), "another origin");
    assert!(!covers(&origin, "http://news.example/p/100.tti"), "plain HTTP");
    assert!(!covers(&origin, "https://news.example/dbsc/refresh"), "the refresh address itself");
    let site = session(true, &[(false, "*", "/public"), (true, "*", "/public/mine")]);
    assert!(covers(&site, "https://www.news.example/p/100.tti"));
    assert!(!covers(&site, "https://othernews.example/p"), "not below the site");
    assert!(covers(&site, "https://news.example/public/mine/1"), "the last matching rule decides");
    assert!(!covers(&site, "https://news.example/public/other"));
    let excluded = session(true, &[(false, "*.news.example", "/")]);
    assert!(!covers(&excluded, "https://cdn.news.example/a") && covers(&excluded, "https://news.example/a"));
}

#[test]
fn a_missing_cookie_is_found_by_name() {
    let session = session(false, &[]);
    assert!(!session.missing(b"theme=dark; auth=xyz"));
    assert!(session.missing(b"theme=dark"));
    assert!(session.missing(b"author=1; xauth=2"));
    assert!(session.missing(b""));
}

#[test]
fn the_store_is_bounded_and_replaces_by_identifier() {
    let mut sessions = Sessions::new();
    let mut first = session(false, &[]);
    assert_eq!(sessions.insert(first), Ok(None));
    first.key = KeyHandle(8);
    assert_eq!(sessions.insert(first), Ok(Some(KeyHandle(7))), "the replaced key is the caller's to delete");
    for index in 1..MAX_SESSIONS {
        let mut other = session(false, &[]);
        other.id = TextBuf::from_bytes(std::format!("s{index}x").as_bytes()).unwrap();
        assert_eq!(sessions.insert(other), Ok(None));
    }
    let mut more = session(false, &[]);
    more.id = TextBuf::from_bytes(b"over").unwrap();
    more.key = KeyHandle(9);
    assert_eq!(sessions.insert(more), Err(KeyHandle(9)));
    let mut ended = Vec::new();
    sessions.expire(100, |session| ended.push(session.key));
    assert_eq!(ended.len(), MAX_SESSIONS);
    assert!(sessions.iter().next().is_none());
}
