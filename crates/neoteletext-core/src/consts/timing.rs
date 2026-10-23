//! Timing (decisions E.12 and E.19, REQ-NAV-007) and the flash units of
//! Levels 2.5 and 3.5 (decision E.48, Table 29 mode 00111).

/// Microseconds per field at 50 fields per second.
pub const FIELD_PERIOD_US: u64 = 20_000;
/// Fields a flashing character is shown (decision E.12).
pub const FLASH_FIELDS_SHOWN: u64 = 48;
/// Fields a flashing character is hidden (decision E.12).
pub const FLASH_FIELDS_HIDDEN: u64 = 16;
/// Page completion timeout (decision E.19, Annex B.9).
pub const PAGE_COMPLETION_TIMEOUT_US: u64 = 100_000;
/// Momentary reveal timeout (REQ-NAV-007).
pub const MOMENTARY_REVEAL_US: u64 = 160_000;

// ---------------------------------------------------------------------------
// Flash at Levels 2.5 and 3.5 (decision E.48, Table 29 mode 00111)
// ---------------------------------------------------------------------------

/// One flash cycle at Levels 2.5 and 3.5 lasts one second (decision E.48).
pub const FLASH_CYCLE_US: u64 = 1_000_000;
/// The cycle is divided into twelve units of one twelfth of a second,
/// numbered 0 to 11: twelve divides both rates and the three fast phases
/// evenly (decision E.48).
pub const FLASH_UNITS: u8 = 12;
/// Slow flash (1 hertz) is on during units 0 to 8 and off during units 9
/// to 11, the 3 to 1 ratio of decision E.12.
pub const FLASH_SLOW_ON_UNITS: u8 = 9;
/// Fast flash (2 hertz) treats each half-second of six units alike.
pub const FLASH_FAST_HALF_UNITS: u8 = FLASH_UNITS / 2;
/// The fast flash phases, 1 to 3.
pub const FLASH_PHASES: u8 = 3;
/// A fast flash character with phase p is off during the two units
/// 2(p − 1) and 2p − 1 of each half-second.
pub const FLASH_FAST_OFF_UNITS: u8 = 2;
