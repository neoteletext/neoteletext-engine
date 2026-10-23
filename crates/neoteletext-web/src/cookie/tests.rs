#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
//! The tests of the cookie store.

use super::store::default_cookie_path;
use super::*;

type Jar = CookieJar<120, 65536>;

const NOW: u64 = 1_791_590_400; // 2026-10-10 00:00:00 UTC

fn header(jar: &mut Jar, host: &str, path: &str, secure: bool, site: Site) -> std::string::String {
    let mut buffer = [0u8; 8192];
    let mut writer = Writer::new(&mut buffer);
    jar.header_for(host, path, secure, site, NOW, &mut writer);
    std::string::String::from_utf8(writer.written().to_vec()).unwrap()
}

fn set(jar: &mut Jar, text: &str, secure: bool) -> Result<(), CookieRejected> {
    jar.store(text.as_bytes(), "a.example", "/dir/page", secure, NOW, true)
}

#[test]
fn control_characters_ignore_the_header() {
    let mut jar = Jar::new();
    assert_eq!(set(&mut jar, "a=b\u{1}c", true), Err(CookieRejected::ControlCharacter));
    assert_eq!(set(&mut jar, "a=b; Path=/\u{7f}", true), Err(CookieRejected::ControlCharacter));
    assert_eq!(set(&mut jar, "a=b\tc", true), Ok(()), "tab is allowed");
}

#[test]
fn empty_name_and_value_is_ignored() {
    let mut jar = Jar::new();
    assert_eq!(set(&mut jar, "=", true), Err(CookieRejected::Empty));
    assert_eq!(set(&mut jar, " ; Secure", true), Err(CookieRejected::Empty));
    assert_eq!(set(&mut jar, "lonely", true), Ok(()));
    assert_eq!(header(&mut jar, "a.example", "/dir", true, Site::SameService), "lonely");
}

#[test]
fn size_limits() {
    let mut jar = Jar::new();
    let mut big = std::string::String::from("n=");
    big.push_str(&"v".repeat(4095));
    assert_eq!(set(&mut jar, &big, true), Ok(()), "exactly 4096 is accepted");
    big.push('v');
    assert_eq!(set(&mut jar, &big, true), Err(CookieRejected::TooLarge));
    jar.wipe();
    let long_path = std::format!("p=1; Path=/{}", "x".repeat(1024));
    assert_eq!(set(&mut jar, &long_path, true), Ok(()));
    // The over-long Path attribute was ignored, so the default path applies.
    assert_eq!(header(&mut jar, "a.example", "/dir/x", true, Site::SameService), "p=1");
}

/// `store` is public, and a port may give it any request path. The
/// default path of one longer than a URL the agent holds was copied
/// into a buffer of that size, and panicked.
#[test]
fn default_path_longer_than_a_url_is_stored() {
    let mut jar = Jar::new();
    let directory = std::format!("/{}", "d".repeat(crate::consts::MAX_URL_LENGTH));
    let request = std::format!("{directory}/page");
    assert_eq!(jar.store(b"long=1", "a.example", &request, true, NOW, true), Ok(()));
    assert_eq!(header(&mut jar, "a.example", &request, true, Site::SameService), "long=1");
    assert_eq!(header(&mut jar, "a.example", "/elsewhere", true, Site::SameService), "");
}

#[test]
fn domain_is_ignored_so_cookies_are_host_only() {
    let mut jar = Jar::new();
    set(&mut jar, "a=1; Domain=example; Path=/", true).unwrap();
    assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "a=1");
    assert_eq!(header(&mut jar, "b.a.example", "/", true, Site::SameService), "");
    assert_eq!(header(&mut jar, "example", "/", true, Site::SameService), "");
}

#[test]
fn lifetimes() {
    let mut jar = Jar::new();
    set(&mut jar, "a=1; Path=/; Max-Age=10; Expires=Wed, 21 Oct 2015 07:28:00 GMT", true).unwrap();
    assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "a=1", "Max-Age wins over a past Expires");
    set(&mut jar, "b=1; Path=/; Max-Age=999999999", true).unwrap();
    let record = jar.records.iter().flatten().find(|r| jar.parts(r).name == b"b").copied().unwrap();
    assert_eq!(record.expires, Some(NOW + 400 * 86_400), "capped at 400 days");
    set(&mut jar, "c=1; Path=/; Expires=Fri, 01 Jan 2100 00:00:00 GMT", true).unwrap();
    let record = jar.records.iter().flatten().find(|r| jar.parts(r).name == b"c").copied().unwrap();
    assert_eq!(record.expires, Some(NOW + 400 * 86_400), "Expires is capped too");
    assert_eq!(set(&mut jar, "a=1; Path=/; Max-Age=0", true), Err(CookieRejected::Expired));
    assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "b=1; c=1", "Max-Age=0 deletes");
    assert_eq!(set(&mut jar, "b=1; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT", true), Err(CookieRejected::Expired));
    assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "c=1", "a past Expires deletes");
    set(&mut jar, "s=1; Path=/", true).unwrap();
    let record = jar.records.iter().flatten().find(|r| jar.parts(r).name == b"s").copied().unwrap();
    assert_eq!(record.expires, None, "a session cookie");
}

#[test]
fn secure_cookies_and_plain_http() {
    let mut jar = Jar::new();
    assert_eq!(set(&mut jar, "s=1; Secure; Path=/", false), Err(CookieRejected::SecureOverPlain));
    set(&mut jar, "s=1; Secure; Path=/", true).unwrap();
    assert_eq!(set(&mut jar, "s=2; Path=/", false), Err(CookieRejected::WouldReplaceSecure));
    assert_eq!(set(&mut jar, "s=2; Path=/x", false), Err(CookieRejected::WouldReplaceSecure));
    assert_eq!(set(&mut jar, "s=; Path=/; Max-Age=0", false), Err(CookieRejected::WouldReplaceSecure));
    assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "s=1");
    assert_eq!(header(&mut jar, "a.example", "/", false, Site::SameService), "", "never sent over plain HTTP");
    set(&mut jar, "p=1; Path=/", false).unwrap();
    assert_eq!(header(&mut jar, "a.example", "/", false, Site::SameService), "p=1");
}

#[test]
fn prefixes() {
    let mut jar = Jar::new();
    let cases: [(&str, bool, bool); 14] = [
        ("__Secure-a=1; Secure", true, true),
        ("__Secure-a=1", true, false),
        ("__SECURE-a=1", true, false),
        ("__Secure-a=1; Secure", false, false),
        ("__Host-a=1; Secure; Path=/", true, true),
        ("__Host-a=1; Secure", true, false),
        ("__Host-a=1; Secure; Path=/; Domain=a.example", true, false),
        ("__host-a=1; Path=/", true, false),
        ("__Http-a=1; Secure; HttpOnly", true, true),
        ("__Http-a=1; Secure", true, false),
        ("__Host-Http-a=1; Secure; HttpOnly; Path=/", true, true),
        ("__Host-Http-a=1; Secure; HttpOnly", true, false),
        ("__Host-Http-a=1; Secure; Path=/", true, false),
        ("=__Host-a=1; Secure; Path=/", true, false),
    ];
    for (text, secure, accepted) in cases {
        let result = set(&mut jar, text, secure);
        if accepted {
            assert_eq!(result, Ok(()), "{text}");
        } else {
            assert!(matches!(result, Err(CookieRejected::Prefix | CookieRejected::SecureOverPlain)), "{text}");
        }
    }
}

#[test]
fn same_site_rules() {
    let mut jar = Jar::new();
    set(&mut jar, "strict=1; Path=/; Secure; SameSite=Strict", true).unwrap();
    set(&mut jar, "lax=1; Path=/; Secure; SameSite=Lax", true).unwrap();
    set(&mut jar, "none=1; Path=/; Secure; SameSite=None", true).unwrap();
    set(&mut jar, "default=1; Path=/; Secure; SameSite=bogus", true).unwrap();
    set(&mut jar, "absent=1; Path=/; Secure", true).unwrap();
    assert_eq!(set(&mut jar, "insecure=1; Path=/; SameSite=None", true), Err(CookieRejected::SameSiteNoneNotSecure));
    let record = jar.records.iter().flatten().find(|r| jar.parts(r).name == b"default").copied().unwrap();
    assert_eq!(record.same_site, SameSite::Default);
    assert_eq!(
        header(&mut jar, "a.example", "/", true, Site::SameService),
        "strict=1; lax=1; none=1; default=1; absent=1"
    );
    assert_eq!(
        header(&mut jar, "a.example", "/", true, Site::CrossServiceNavigation),
        "lax=1; none=1; default=1; absent=1",
        "default is enforced as Lax"
    );
    assert_eq!(header(&mut jar, "a.example", "/", true, Site::CrossOriginForm), "none=1");
}

#[test]
fn http_only_and_partitioned_are_stored() {
    let mut jar = Jar::new();
    set(&mut jar, "a=1; HttpOnly; Partitioned; Secure; Path=/", true).unwrap();
    let record = jar.records.iter().flatten().next().copied().unwrap();
    assert!(record.http_only && record.partitioned);
    assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "a=1");
}

#[test]
fn paths_and_order() {
    let mut jar = Jar::new();
    set(&mut jar, "root=1; Path=/", true).unwrap();
    set(&mut jar, "deep=1; Path=/dir/sub", true).unwrap();
    set(&mut jar, "dir=1", true).unwrap(); // default path /dir
    set(&mut jar, "other=1; Path=/other", true).unwrap();
    set(&mut jar, "relative=1; Path=dir", true).unwrap(); // bad Path: default applies
    assert_eq!(
        header(&mut jar, "a.example", "/dir/sub/page", true, Site::SameService),
        "deep=1; dir=1; relative=1; root=1"
    );
    assert_eq!(header(&mut jar, "a.example", "/dirt", true, Site::SameService), "root=1");
    assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "root=1");
    // Replacing keeps the creation order.
    set(&mut jar, "root=2; Path=/", true).unwrap();
    assert_eq!(header(&mut jar, "a.example", "/x", true, Site::SameService), "root=2");
    assert_eq!(default_cookie_path(b"/a/b/c"), b"/a/b");
    assert_eq!(default_cookie_path(b"/a"), b"/");
    assert_eq!(default_cookie_path(b""), b"/");
}

#[test]
fn values_are_opaque() {
    let mut jar = Jar::new();
    set(&mut jar, "session=v2|eyJ1c2VyIjoxfQ==|sig; Path=/", true).unwrap();
    assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "session=v2|eyJ1c2VyIjoxfQ==|sig");
}

#[test]
fn per_host_and_total_limits_evict_least_recently_used_expired_first() {
    let mut jar = Jar::new();
    jar.set_limits(3, 5);
    for name in ["a", "b", "c"] {
        jar.store(std::format!("{name}=1; Path=/").as_bytes(), "h1.example", "/", true, NOW, true).unwrap();
    }
    // Use "a" so "b" becomes least recently used.
    let mut buffer = [0u8; 64];
    let mut writer = Writer::new(&mut buffer);
    jar.header_for("h1.example", "/", true, Site::SameService, NOW, &mut writer);
    jar.store(b"a=2; Path=/", "h1.example", "/", true, NOW, true).unwrap();
    jar.store(b"d=1; Path=/", "h1.example", "/", true, NOW, true).unwrap();
    assert_eq!(header(&mut jar, "h1.example", "/", true, Site::SameService), "a=2; c=1; d=1");
    // An expired cookie goes first, even if recently used.
    jar.store(b"x=1; Path=/; Max-Age=5", "h2.example", "/", true, NOW, true).unwrap();
    jar.store(b"y=1; Path=/", "h2.example", "/", true, NOW, true).unwrap();
    assert_eq!(jar.len(), 5);
    jar.store(b"z=1; Path=/", "h3.example", "/", true, NOW + 10, true).unwrap();
    assert_eq!(jar.len(), 5);
    let names: std::vec::Vec<_> = jar.records.iter().flatten().map(|r| jar.parts(r).name.to_vec()).collect();
    assert!(!names.contains(&b"x".to_vec()), "{names:?}");
}

#[test]
fn arena_compaction_keeps_other_cookies_intact() {
    let mut jar: CookieJar<8, 64> = CookieJar::new();
    jar.store(b"a=11111111; Path=/", "h.example", "/", true, NOW, true).unwrap();
    jar.store(b"b=22222222; Path=/", "h.example", "/", true, NOW, true).unwrap();
    jar.store(b"c=33333333; Path=/", "h.example", "/", true, NOW, true).unwrap();
    // The arena is full: storing evicts the least recently used.
    jar.store(b"d=44444444; Path=/", "h.example", "/", true, NOW, true).unwrap();
    let mut buffer = [0u8; 128];
    let mut writer = Writer::new(&mut buffer);
    jar.header_for("h.example", "/", true, Site::SameService, NOW, &mut writer);
    assert_eq!(writer.written(), b"b=22222222; c=33333333; d=44444444");
    assert_eq!(jar.store(&[b'x'; 70], "h.example", "/", true, NOW, true), Err(CookieRejected::NoRoom));
}

#[test]
fn wipe_and_persistence() {
    let mut jar = Jar::new();
    set(&mut jar, "keep=1; Path=/; Max-Age=3600; Secure; SameSite=Strict", true).unwrap();
    set(&mut jar, "session=1; Path=/", true).unwrap();
    set(&mut jar, "soon=1; Path=/; Max-Age=1", true).unwrap();
    let mut buffer = [0u8; 1024];
    let mut writer = Writer::new(&mut buffer);
    assert_eq!(jar.export(NOW + 5, &mut writer).unwrap(), 1, "session and expired cookies are never saved");
    let saved = writer.written().to_vec();
    jar.wipe();
    assert!(jar.is_empty());
    assert_eq!(jar.import(&saved, NOW + 5), 1);
    assert_eq!(header(&mut jar, "a.example", "/", true, Site::SameService), "keep=1");
    assert_eq!(header(&mut jar, "a.example", "/", true, Site::CrossServiceNavigation), "", "SameSite survives");
    // Tampered data is refused.
    let mut tampered = saved.clone();
    let len = tampered.len();
    tampered[len - 1] = b'x';
    let mut fresh = Jar::new();
    assert_eq!(fresh.import(&tampered, NOW), 0);
    assert_eq!(fresh.import(b"neoteletext-cookies 2\n9999999999 1 1 1 3 1 1 1 a bx1/\n", NOW), 0, "a bad host");
    assert_eq!(fresh.import(b"neoteletext-cookies 2\n9999999999 1 1 9 3 1 1 1 a.bx1/\n", NOW), 1, "the same, good");
    assert_eq!(fresh.import(b"neoteletext-cookies 1\n9999999999 1 3 1 1 1 a.bx1/\n", NOW), 0, "version 1 is not read");
    assert_eq!(fresh.import(b"garbage", NOW), 0);
}

/// Decision E.104: each cookie's times, in seconds, are kept through a
/// replacement, a sending and a save; a port can list cookies, remove
/// those a rule chooses, and clear a host.
#[test]
fn times_listing_and_removal() {
    let mut jar = Jar::new();
    jar.store(b"a=1; Max-Age=3600", "a.example", "/", true, NOW, true).unwrap();
    jar.store(b"b=1", "b.example", "/", true, NOW + 10, true).unwrap();
    jar.store(b"a=2; Max-Age=3600", "a.example", "/", true, NOW + 20, true).unwrap();
    let mut buffer = [0u8; 64];
    let _ = jar.header_for("a.example", "/", true, Site::SameService, NOW + 30, &mut Writer::new(&mut buffer));
    let listed = jar.cookies().map(|cookie| (cookie.host, cookie.created_at, cookie.used_at, cookie.expires.is_some()));
    assert!(
        listed.eq([(&b"a.example"[..], NOW, NOW + 30, true), (&b"b.example"[..], NOW + 10, NOW + 10, false)]),
        "a replaced cookie keeps its creation"
    );
    assert_eq!(jar.cookies().map(|cookie| cookie.bytes).sum::<usize>(), "a.examplea2/".len() + "b.exampleb1/".len());

    let mut saved = [0u8; 256];
    let mut writer = Writer::new(&mut saved);
    jar.export(NOW + 40, &mut writer).unwrap();
    let mut restored = Jar::new();
    assert_eq!(restored.import(writer.written(), NOW + 50), 1);
    assert!(
        restored.cookies().map(|cookie| (cookie.created_at, cookie.used_at)).eq([(NOW, NOW + 30)]),
        "the times as saved"
    );

    assert_eq!(jar.remove_where(&|cookie| cookie.created_at >= NOW + 5), 1);
    assert_eq!(jar.len(), 1);
    jar.store(b"c=1", "a.example", "/", true, NOW, true).unwrap();
    jar.clear_host("a.example");
    assert!(jar.is_empty());
}

/// Decision E.104: the jar serves an agent through the `Cookies`
/// interface as it does directly.
#[test]
fn the_jar_is_a_cookies_store() {
    let mut jar = Jar::new();
    let store: &mut dyn Cookies = &mut jar;
    store.store_within(b"a=1", "a.example", "/channel/page", Some("/channel/"), true, NOW, true).unwrap();
    let mut buffer = [0u8; 64];
    let mut writer = Writer::new(&mut buffer);
    assert_eq!(store.header_for("a.example", "/channel/x", true, Site::SameService, NOW, &mut writer), (1, false));
    assert_eq!(writer.written(), b"a=1");
    assert!(store.reached_securely("a.example", NOW));
    store.clear_host("a.example");
    assert!(!store.reached_securely("a.example", NOW));
}

#[test]
fn cookie_dates() {
    let cases: [(&str, Option<u64>); 8] = [
        ("Wed, 21 Oct 2015 07:28:00 GMT", Some(1_445_412_480)),
        ("Sunday, 06-Nov-94 08:49:37 GMT", Some(784_111_777)),
        ("Sun Nov  6 08:49:37 1994", Some(784_111_777)),
        ("Thu, 01 Jan 1970 00:00:00 GMT", Some(0)),
        ("Sat, 10 Oct 2026 00:00:00 GMT", Some(NOW)),
        ("Wed, 30 Feb 2015 07:28:00 GMT", None),
        ("Wed, 21 Oct 2015 25:28:00 GMT", None),
        ("21 Oct 2015", None),
    ];
    for (text, expected) in cases {
        assert_eq!(parse_cookie_date(text.as_bytes()), expected, "{text}");
    }
}

#[test]
fn expires_needs_calendar_time() {
    let mut jar = Jar::new();
    jar.store(b"a=1; Path=/; Expires=Fri, 01 Jan 2100 00:00:00 GMT", "h.example", "/", true, 50, false).unwrap();
    let record = jar.records.iter().flatten().next().copied().unwrap();
    assert_eq!(record.expires, None, "kept as a session cookie");
}

#[test]
fn secure_history_for_downgrade() {
    let mut jar = Jar::new();
    set(&mut jar, "a=1; Path=/; Max-Age=60", true).unwrap();
    assert!(jar.reached_securely("a.example", NOW));
    assert!(!jar.reached_securely("a.example", NOW + 61));
    assert!(!jar.reached_securely("b.example", NOW));
}

/// Finding F-13 (decision E.114 item 1): an expired record is read past,
/// and the live record after it is restored. The first expired record
/// ended the whole import, and every later cookie was lost.
#[test]
fn an_import_skips_an_expired_record_and_goes_on() {
    let mut jar = Jar::new();
    jar.store(b"old=1; Path=/; Max-Age=10", "a.example", "/", true, NOW, true).unwrap();
    jar.store(b"live=1; Path=/; Max-Age=3600", "a.example", "/", true, NOW, true).unwrap();
    let mut buffer = [0u8; 512];
    let mut writer = Writer::new(&mut buffer);
    assert_eq!(jar.export(NOW, &mut writer).unwrap(), 2);
    let saved = writer.written().to_vec();
    let mut restored = Jar::new();
    assert_eq!(restored.import(&saved, NOW + 20), 1, "the expired record is skipped, the live one restored");
    assert_eq!(header(&mut restored, "a.example", "/", true, Site::SameService), "live=1");
}

/// Finding F-13 (decision E.114 item 1): a cookie whose path is longer
/// than the attribute limit comes back with that path. The path was
/// rebuilt into a `Path=` attribute, which the 1024-byte limit dropped,
/// so the cookie came back as `Path=/` and was sent everywhere on the host.
#[test]
fn an_imported_path_longer_than_an_attribute_keeps_its_folder() {
    let folder = std::format!("/{}", "f".repeat(1100));
    let mut jar = Jar::new();
    jar.store(b"deep=1; Max-Age=3600", "a.example", &std::format!("{folder}/page"), true, NOW, true).unwrap();
    assert_eq!(header(&mut jar, "a.example", &std::format!("{folder}/x"), true, Site::SameService), "deep=1");
    let mut buffer = [0u8; 2048];
    let mut writer = Writer::new(&mut buffer);
    assert_eq!(jar.export(NOW, &mut writer).unwrap(), 1);
    let saved = writer.written().to_vec();
    let mut restored = Jar::new();
    assert_eq!(restored.import(&saved, NOW), 1);
    assert_eq!(header(&mut restored, "a.example", &std::format!("{folder}/x"), true, Site::SameService), "deep=1");
    assert_eq!(header(&mut restored, "a.example", "/", true, Site::SameService), "", "not widened to the host");
    assert_eq!(header(&mut restored, "a.example", "/elsewhere", true, Site::SameService), "");
}

/// Finding F-13: a saved path is checked as the header it came from was,
/// and a refused record is read past.
#[test]
fn an_imported_path_with_a_control_character_is_refused() {
    let mut jar = Jar::new();
    let data = b"neoteletext-cookies 2\n9999999999 1 1 9 3 1 1 3 a.bx1/\r\n\n9999999999 1 1 9 3 1 1 1 a.by2/\n";
    assert_eq!(jar.import(data, NOW), 1, "the second record is restored");
    assert_eq!(header(&mut jar, "a.b", "/", true, Site::SameService), "y=2");
}
