//! Set-Cookie headers, cookie dates and the cookie jar's import format
//! (REQ-TST-040, REQ-WEB-008).
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_web::cookie::{CookieJar, parse_cookie_date};

/// A small jar, so that limits and eviction are reached.
type Jar = CookieJar<4, 512>;

fuzz_target!(|data: &[u8]| {
    let mut jar = Jar::new();
    for header in data.split(|&byte| byte == b'\n') {
        let _ = jar.store(header, "example.org", "/teletext/page", true, 1_000, true);
        let _ = parse_cookie_date(header);
    }
    let mut imported = Jar::new();
    imported.import(data, 1_000);
    // A saved cookie's times are never later than now, nor used before made
    // (decision E.104).
    for cookie in imported.cookies() {
        assert!(cookie.created_at <= cookie.used_at && cookie.used_at <= 1_000);
    }
    core::hint::black_box(imported.len());
});
