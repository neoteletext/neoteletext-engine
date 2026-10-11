//! Service descriptors and the JSON reader under them (REQ-TST-040).
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_core::types::PageAddress;
use neoteletext_web::descriptor::Descriptor;
use neoteletext_web::url::Url;

fuzz_target!(|data: &[u8]| {
    let Ok(base) = Url::parse("https://example.org/teletext/service.json") else { return };
    let Ok(descriptor) = Descriptor::parse(data, &base) else { return };
    if let Some(page) = PageAddress::from_mpp(0x100) {
        let _ = descriptor.page_url(page, None);
    }
    for service in descriptor.services() {
        core::hint::black_box(descriptor.service_address(service));
    }
});
