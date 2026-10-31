//! The REDIRECT record (`neoteletext-common/INPUT.md` section 7.11,
//! REQ-1688, REQ-TST-040): whatever the reader accepts writes back to the
//! same record, and an address it passes on is printable ASCII within the
//! record's bound.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_web::records::redirect::{MAX_ADDRESS, RedirectRecord, encode, read};
use neoteletext_web::records::{ChannelRecord, Read, RecordReader};

/// A service any record may be written on.
const SERVICE: u8 = 3;

fuzz_target!(|data: &[u8]| {
    let Ok(redirect) = read(data) else { return };
    if let RedirectRecord::Address(address) = redirect {
        assert!(!address.is_empty() && address.len() <= MAX_ADDRESS);
        assert!(address.bytes().all(|byte| byte.is_ascii_graphic()));
    }
    let mut packets = Vec::new();
    encode(redirect, SERVICE, 1, &mut |packet| packets.push(packet)).unwrap();
    let mut reader = RecordReader::new();
    let mut read_back = false;
    for packet in &packets {
        if let Read::Record(ChannelRecord::Redirect(back)) = reader.push(packet) {
            assert_eq!(back, redirect);
            read_back = true;
        }
    }
    assert!(read_back);
});
