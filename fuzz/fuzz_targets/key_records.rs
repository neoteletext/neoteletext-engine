//! The key record reader, from arbitrary bytes (REQ-INT-001, REQ-TST-040):
//! whatever it accepts, the writer writes back to the same bytes.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_input::key::{MAX_RECORD, read};

fuzz_target!(|data: &[u8]| {
    if let Ok(record) = read(data) {
        let mut out = [0_u8; MAX_RECORD];
        let length = record.write(&mut out).expect("an accepted record is written");
        assert_eq!(&out[..length], data);
    }
});
