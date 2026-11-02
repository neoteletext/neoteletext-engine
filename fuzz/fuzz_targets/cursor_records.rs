//! The cursor record reader, from arbitrary packets and bodies
//! (REQ-INT-005, REQ-TST-040): whatever it accepts it writes back the same,
//! but for what decision E.134 left ignored when read and written 0.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_core::packet::Packet;
use neoteletext_input::cursor::{BLOCK, BODY, from_body, from_packet};

/// The body's reserved byte and its flags (the layout in `cursor.rs`).
const RESERVED: usize = 6;
const FLAGS: usize = 7;

fuzz_target!(|data: &[u8]| {
    if let Ok(bytes) = <[u8; 42]>::try_from(data.get(..42).unwrap_or(&[]))
        && let Some(Ok(cursor)) = from_packet(&Packet { bytes })
    {
        assert!(cursor.record().is_some());
    }
    if let Ok(body) = <[u8; BODY]>::try_from(data.get(..BODY).unwrap_or(&[]))
        && let Ok(cursor) = from_body(body)
    {
        let mut written = body;
        written[RESERVED] = 0;
        written[FLAGS] &= BLOCK;
        assert_eq!(cursor.body(), written);
    }
});
