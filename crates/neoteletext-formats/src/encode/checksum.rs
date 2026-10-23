//! The page checksum of clause 9.6.1 (REQ-ECC-007) and the word packet
//! X/27/0 carries.

use neoteletext_core::consts::{DATA_BYTES, HEADER_DISPLAY_BYTES, PACKET_ROW_25, SPACE};
use neoteletext_core::ecc::{PageChecksum, with_odd_parity};

use super::CHECKSUM_HEADER_BYTES;

/// Computes the page checksum of clause 9.6.1 (REQ-ECC-007): header bytes
/// 14 to 37, then bytes 6 to 45 of packets X/1 to X/25, absent packets as
/// spaces. Every byte is applied with its parity bit, as transmitted.
#[must_use]
pub fn page_checksum(header_display: &[u8; HEADER_DISPLAY_BYTES], rows: &[Option<[u8; DATA_BYTES]>]) -> u16 {
    let mut checksum = PageChecksum::new();
    // Bytes 14 to 37 are the first 24 display bytes; the clock bytes 38 to
    // 45 change every second and are left out.
    checksum.update_all(header_display.get(..CHECKSUM_HEADER_BYTES).unwrap_or(&[]));
    let space = with_odd_parity(SPACE);
    for row in rows.iter().take(usize::from(PACKET_ROW_25)) {
        match row {
            Some(bytes) => checksum.update_all(bytes),
            None => {
                for _ in 0..DATA_BYTES {
                    checksum.update(space);
                }
            }
        }
    }
    checksum.value()
}

/// The 16-bit word handed to the X/27/0 builder, which writes it big-endian
/// into bytes 44 and 45. Every decision about the transmitted order of the
/// checksum lives here.
#[must_use]
pub const fn checksum_word(checksum: u16) -> u16 {
    checksum
}
