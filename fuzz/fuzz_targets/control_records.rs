//! The input control record's reader, from arbitrary bytes (REQ-INT-111,
//! REQ-INT-114, REQ-TST-040): whatever it accepts is written back, carried
//! in a packet, and read again the same.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_input::control::{FRAGMENT_DATA, MAX_BODY, from_packet, packet_of, read_body, write_body};

fuzz_target!(|data: &[u8]| {
    if let Ok(control) = read_body(data) {
        let mut out = [0_u8; MAX_BODY];
        let length = write_body(&control, &mut out).expect("an accepted message is written");
        assert_eq!(read_body(&out[..length]), Ok(control));
        let packet = packet_of(&control, 0, 0).expect("and carried");
        let mut buffer = [0_u8; FRAGMENT_DATA];
        assert_eq!(from_packet(&packet, &mut buffer), Some(Ok(control)));
    }
});
