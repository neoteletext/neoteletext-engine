//! Web addresses: parsing and resolving references (REQ-TST-040,
//! REQ-WEB-016).
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_web::url::Url;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = core::str::from_utf8(data) else { return };
    // The first line is an address; the rest a reference against it.
    let (first, reference) = text.split_once('\n').unwrap_or((text, ""));
    if let Ok(url) = Url::parse(first) {
        core::hint::black_box((url.host_text(), url.port(), url.path(), url.query(), url.origin()));
        if let Ok(joined) = url.join(reference) {
            core::hint::black_box(joined.as_str());
        }
    }
    let _ = Url::parse_host_only(first);
});
