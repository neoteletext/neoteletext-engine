//! A proxy's answers, from arbitrary bytes (REQ-WEB-032, REQ-TST-040): the
//! `CONNECT` reply read in pieces a byte at a time and whole, and SOCKS5
//! replies; none panics, and a success never takes more than was given.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_web::http::HeadCollector;
use neoteletext_web::proxy::{read_connect_reply, read_socks_choice, read_socks_reply, socks_reply_length};

fuzz_target!(|data: &[u8]| {
    let mut buffer = [0_u8; 1024];
    let mut whole = HeadCollector::default();
    if let Ok(Some(used)) = read_connect_reply(&mut whole, &mut buffer, data) {
        assert!(used <= data.len());
    }
    let mut collector = HeadCollector::default();
    for byte in data {
        match read_connect_reply(&mut collector, &mut buffer, core::slice::from_ref(byte)) {
            Ok(None) => {}
            Ok(Some(used)) => {
                assert_eq!(used, 1);
                break;
            }
            Err(_) => break,
        }
    }
    if let [first, second, ..] = *data {
        let _ = read_socks_choice([first, second]);
    }
    if let Some(start) = data.get(..5).and_then(|start| <[u8; 5]>::try_from(start).ok())
        && let Ok(length) = socks_reply_length(start)
        && let Some(reply) = data.get(..length)
    {
        let _ = read_socks_reply(reply);
    }
    let _ = read_socks_reply(data);
});
