//! `neottx service-info` (REQ-SVC-005): the broadcast service data of
//! packet 8/30.

use super::sandbox;
use neoteletext_app::input::{self, Options};
use neoteletext_app::{Failure, exit};

/// `neottx service-info`.
pub fn service_info(options: &Options) -> Result<(), Failure> {
    let session = input::load(options)?;
    sandbox(options, None)?;
    let Some(info) = session.service() else {
        return Err((exit::NOT_FOUND, "no packet 8/30 in the input".into()));
    };
    println!("format: {}", if info.format2 { 2 } else { 1 });
    if let Some((address, subcode)) = info.initial_page {
        println!("initial page: {} sub-code {:04X}", String::from_utf8_lossy(&address.ascii()), subcode.value());
    }
    if let Some(network) = info.network_id {
        println!("network identification: {network:04X}");
    }
    if let Some(time) = info.utc {
        println!(
            "date and time (UTC): {:04}-{:02}-{:02} {:02}:{:02}:{:02} (refers to the next second boundary)",
            time.year, time.month, time.day, time.hour, time.minute, time.second
        );
    }
    if let Some(offset) = info.offset_half_hours {
        println!(
            "local offset: {}{}:{:02}",
            if offset < 0 { '-' } else { '+' },
            offset.unsigned_abs() / 2,
            (offset.unsigned_abs() % 2) * 30
        );
    }
    // REQ-SVC-005: the core decodes the message with the Latin G0 set.
    let status: String = neoteletext_core::charset::latin_text(&info.status)
        .iter()
        .map(|&c| if neoteletext_core::policy::is_safe_output_char(c) { c } else { ' ' })
        .collect();
    println!("status: {}", status.trim_end());
    Ok(())
}
