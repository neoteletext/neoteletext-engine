//! RFC 1982 serial arithmetic over 16 bits, for the channels' sequence
//! numbers and the live sounds' block indices.

/// A sequence number newer than another by RFC 1982 serial arithmetic over
/// 16 bits: the difference is positive and less than half the range. Two
/// numbers exactly half the range apart are neither (REQ-AUD-010).
#[must_use]
pub const fn serial_newer(candidate: u16, than: u16) -> bool {
    let difference = candidate.wrapping_sub(than);
    difference != 0 && difference < 0x8000
}
