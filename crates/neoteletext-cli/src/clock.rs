//! The milliseconds clock the input session counts in (`INPUT.md` section 6):
//! time since a start, wrapping at 2^32 as the session's u32 times do.

use std::time::Instant;

/// Milliseconds since `start`, modulo 2^32.
pub(crate) fn now_ms(start: Instant) -> u32 {
    u32::try_from(start.elapsed().as_millis() & u128::from(u32::MAX)).unwrap_or(0)
}
