//! The contents file of a channel folder (REQ-WEB-025's `contents.txt`,
//! REQ-TST-040): its size and line checks, then every entry the input's
//! lines hold.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_web::contents;

fuzz_target!(|data: &[u8]| {
    let Ok(entries) = contents::read(data) else { return };
    for entry in entries {
        core::hint::black_box(&entry);
    }
});
