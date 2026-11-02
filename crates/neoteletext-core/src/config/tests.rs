//! Tests of the configuration and its profiles.

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

/// REQ-367: every choice of the default profile.
#[test]
fn default_profile_matches_req_prf_001() {
    let config = Config::default();
    assert!(config.process_x26_15 && !config.strict_row_setting);
    assert_eq!(config.level, PresentationLevel::Level35, "decision E.40");
    assert!(config.double_start_box && config.black_colour_codes && config.rounding);
    assert_eq!(config.row24, Row24Policy::AnnexH);
    assert_eq!(config.cell_mode, CellMode::Saa5050, "12 by 20 cells");
    assert_eq!(config.flash_shown_us, 960_000);
    assert_eq!(config.flash_hidden_us, 320_000);
}

/// REQ-PRF-006: each profile selects its level, the levels are ordered
/// as clause 12 defines them, and the level can be overridden.
#[test]
fn profiles_select_their_presentation_level() {
    assert_eq!(Config::saa5246a().level, PresentationLevel::Level15);
    assert!(PresentationLevel::Level1 < PresentationLevel::Level15);
    assert!(PresentationLevel::Level15 < PresentationLevel::Level25);
    assert!(PresentationLevel::Level25 < PresentationLevel::Level35);
    assert!(!PresentationLevel::Level15.is_enhanced() && PresentationLevel::Level25.is_enhanced());
    assert!(!PresentationLevel::Level25.is_level35() && PresentationLevel::Level35.is_level35());
    let config = Config { level: PresentationLevel::Level25, ..Config::saa5050() };
    assert_eq!(config.level, PresentationLevel::Level25);
}

/// REQ-371: a setting changed after a profile is applied keeps the change.
#[test]
fn profiles_can_be_overridden() {
    let mut config = Config::saa5246a();
    config.rounding = true;
    assert!(config.rounding && !config.process_x26_15);
    assert!(Config::profile("nonsense").is_none());
}

#[test]
fn saa5050_profile_matches_req_prf_002() {
    let config = Config::saa5050();
    assert!(config.rounding);
    assert_eq!(config.level, PresentationLevel::Level1);
    assert_eq!(config.row24, Row24Policy::Never);
    assert_eq!(config.fixed_national_index, Some(0));
    assert_eq!(config.reveal, RevealStyle::Momentary);
}

/// REQ-369: the profile's settings. Its variant setting, which would choose
/// the character set group of Part D.5, is not built.
#[test]
fn saa5246a_profile_matches_req_prf_003() {
    let config = Config::saa5246a();
    assert_eq!(config.level, PresentationLevel::Level15);
    assert!(!config.process_x26_15, "X/26/15 ignored");
    assert!(config.black_colour_codes, "alpha black and mosaic black decoded");
    assert_eq!(config.row24, Row24Policy::WhenPresent);
    assert_eq!(config.cell_mode, CellMode::Saa5246a, "12 by 10 cells");
    assert!(!config.rounding, "without rounding");
    assert_eq!(Config::profile("saa5246a"), Some(config));
}
