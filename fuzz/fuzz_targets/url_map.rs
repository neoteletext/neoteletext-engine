//! The channel URL map: reading and normalising an address (REQ-TST-040,
//! neoteletext-common's REQ-1693).
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_web::bytes::Writer;
use neoteletext_web::consts::MAX_URL_LENGTH;
use neoteletext_web::map::{MapAddress, Place};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = core::str::from_utf8(data) else { return };
    if let Some(place) = Place::read(text) {
        let mut buffer = [0u8; MAX_URL_LENGTH];
        let mut writer = Writer::new(&mut buffer);
        assert!(place.write(&mut writer).is_ok());
        // A place's canonical fragment reads as the same place.
        let canonical = core::str::from_utf8(writer.written()).expect("a fragment is ASCII");
        assert_eq!(Place::read(canonical), Some(place));
    }
    let Ok(address) = MapAddress::read(text) else { return };
    let mut buffer = [0u8; MAX_URL_LENGTH];
    let mut writer = Writer::new(&mut buffer);
    assert!(address.write(&mut writer).is_ok());
    // A canonical address is canonical: reading it changes nothing.
    let canonical = core::str::from_utf8(writer.written()).expect("an address is ASCII");
    let again = MapAddress::read(canonical).expect("a canonical address reads");
    assert_eq!(again.place, address.place);
    assert_eq!(again.query, address.query);
    assert_eq!(again.folder.as_str(), address.folder.as_str());
});
