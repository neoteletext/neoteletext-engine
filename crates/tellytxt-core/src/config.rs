//! Configuration: every behaviour choice of REQUIREMENTS.md Part E as a
//! field, with documented defaults and named profiles (REQ-ARCH-008, area
//! PRF). Changing the configuration never requires re-acquiring pages.

use crate::consts::{
    FIELD_PERIOD_US, FLASH_FIELDS_HIDDEN, FLASH_FIELDS_SHOWN, MOMENTARY_REVEAL_US,
    PAGE_COMPLETION_TIMEOUT_US,
};

/// How characters that fail their parity check are shown (decision E.8).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParityPolicy {
    /// Keep the last character that passed; never-good cells show a space.
    KeepLastGood,
    /// Show a space for every character that failed.
    AlwaysSpace,
    /// Show a visible error marker, for diagnostics.
    ErrorMarker,
}

/// When row 24 is displayed (decision E.23).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Row24Policy {
    /// Only when X/27/0 carries the "display row 24" flag (Annex H).
    AnnexH,
    /// Whenever packet X/24 is present (the SAA5246A behaviour).
    WhenPresent,
}

/// Reveal style (REQ-NAV-007).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RevealStyle {
    /// Toggled on and off.
    Latched,
    /// On for 160 milliseconds after each request, like the SAA5050.
    Momentary,
}

/// Next and previous page order (REQ-NAV-002).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StepOrder {
    /// Numerical order.
    Numerical,
    /// Only pages present in the store.
    Stored,
}

/// The raster cell size (REQ-RND-002 and REQ-RND-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellMode {
    /// 12 by 20, the SAA5050 style.
    Saa5050,
    /// 12 by 10, the SAA5246A style.
    Saa5246a,
}

/// The decoder configuration.
// Each flag is a separate Part E decision that REQ-ARCH-008 requires to be
// its own setting.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Apply packets X/26 (REQ-L15-001).
    pub level15: bool,
    /// Process X/26/15 like the other designation codes (decision E.2).
    pub process_x26_15: bool,
    /// Use the clause 12.3.2 row-setting rule instead of Annex A.3 (E.3).
    pub strict_row_setting: bool,
    /// Ignore triplets addressing positions earlier than the previous one
    /// (REQ-L15-009).
    pub ignore_out_of_order_triplets: bool,
    /// Default character set group when neither X/28/0 nor M/29/0 gives one
    /// (decision E.4), 0 to 15.
    pub default_region: u8,
    /// Use a fixed national option and ignore C12 to C14 (the SAA5050
    /// profile); `None` follows the header.
    pub fixed_national_index: Option<u8>,
    /// Parity failure display policy (decision E.8).
    pub parity_policy: ParityPolicy,
    /// Roll incoming headers while searching (decision E.11).
    pub rolling_header: bool,
    /// Microseconds a flashing character is shown (decision E.12).
    pub flash_shown_us: u64,
    /// Microseconds a flashing character is hidden (decision E.12).
    pub flash_hidden_us: u64,
    /// Round alphanumeric glyphs in the 12-by-20 mode (REQ-RND-012).
    pub rounding: bool,
    /// Require two consecutive Start Box codes (decision E.17).
    pub double_start_box: bool,
    /// Treat 0/0 and 1/0 as black colour attributes (decision E.18).
    pub black_colour_codes: bool,
    /// Page completion timeout in microseconds (decision E.19).
    pub page_completion_timeout_us: u64,
    /// Row 24 display rule (decision E.23).
    pub row24: Row24Policy,
    /// Accept a framing code with one wrong bit (decision E.24).
    pub framing_tolerance: bool,
    /// Store pages with hexadecimal numbers, for diagnostics (REQ-ACQ-013).
    pub store_hexadecimal_pages: bool,
    /// Microseconds after which a page not received again is stale
    /// (REQ-ACQ-012, REQ-FSM-004).
    pub stale_after_us: u64,
    /// Reveal style (REQ-NAV-007).
    pub reveal: RevealStyle,
    /// Momentary reveal timeout in microseconds.
    pub momentary_reveal_us: u64,
    /// Next and previous page order (REQ-NAV-002).
    pub step_order: StepOrder,
    /// Raster cell size.
    pub cell_mode: CellMode,
    /// Report state machine transitions (REQ-FSM-010).
    pub trace: bool,
    /// Show row 0's status area even on pages with C7 set (REQ-L1-014).
    pub status_on_suppressed_header: bool,
}

/// Flash shown and hidden durations derived from decision E.12's field
/// counts at 50 fields per second.
const FLASH_SHOWN_US: u64 = FLASH_FIELDS_SHOWN * FIELD_PERIOD_US;
const FLASH_HIDDEN_US: u64 = FLASH_FIELDS_HIDDEN * FIELD_PERIOD_US;

/// Microseconds a page may go unreceived before it is stale: 60 seconds, a
/// generous multiple of the longest normal magazine cycle.
const DEFAULT_STALE_AFTER_US: u64 = 60_000_000;

impl Config {
    /// The default profile, "EN 300 706" (REQ-PRF-001).
    #[must_use]
    pub const fn en_300_706() -> Self {
        Self {
            level15: true,
            process_x26_15: true,
            strict_row_setting: false,
            ignore_out_of_order_triplets: false,
            default_region: 0,
            fixed_national_index: None,
            parity_policy: ParityPolicy::KeepLastGood,
            rolling_header: true,
            flash_shown_us: FLASH_SHOWN_US,
            flash_hidden_us: FLASH_HIDDEN_US,
            rounding: true,
            double_start_box: true,
            black_colour_codes: false,
            page_completion_timeout_us: PAGE_COMPLETION_TIMEOUT_US,
            row24: Row24Policy::AnnexH,
            framing_tolerance: true,
            store_hexadecimal_pages: false,
            stale_after_us: DEFAULT_STALE_AFTER_US,
            reveal: RevealStyle::Latched,
            momentary_reveal_us: MOMENTARY_REVEAL_US,
            step_order: StepOrder::Numerical,
            cell_mode: CellMode::Saa5050,
            trace: false,
            status_on_suppressed_header: false,
        }
    }

    /// The "SAA5050" profile (REQ-PRF-002): Level 1.0, a fixed English
    /// character set, row 24 never displayed, momentary reveal.
    #[must_use]
    pub const fn saa5050() -> Self {
        let mut config = Self::en_300_706();
        config.level15 = false;
        // English is national option index 0 in Table 32 group 0000.
        config.fixed_national_index = Some(0);
        config.reveal = RevealStyle::Momentary;
        config
    }

    /// The "SAA5246A" profile (REQ-PRF-003).
    #[must_use]
    pub const fn saa5246a() -> Self {
        let mut config = Self::en_300_706();
        config.process_x26_15 = false;
        config.black_colour_codes = true;
        config.row24 = Row24Policy::WhenPresent;
        config.cell_mode = CellMode::Saa5246a;
        config.rounding = false;
        config
    }

    /// The profile with the given name: `en300706`, `saa5050` or
    /// `saa5246a`.
    #[must_use]
    pub fn profile(name: &str) -> Option<Self> {
        match name {
            "en300706" | "default" => Some(Self::en_300_706()),
            "saa5050" => Some(Self::saa5050()),
            "saa5246a" => Some(Self::saa5246a()),
            _ => None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::en_300_706()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_matches_req_prf_001() {
        let config = Config::default();
        assert!(config.level15 && config.process_x26_15 && !config.strict_row_setting);
        assert!(config.double_start_box && !config.black_colour_codes && config.rounding);
        assert_eq!(config.row24, Row24Policy::AnnexH);
        assert_eq!(config.flash_shown_us, 960_000);
        assert_eq!(config.flash_hidden_us, 320_000);
    }

    #[test]
    fn profiles_can_be_overridden() {
        let mut config = Config::saa5246a();
        config.rounding = true;
        assert!(config.rounding && !config.process_x26_15);
        assert!(Config::profile("nonsense").is_none());
    }
}
