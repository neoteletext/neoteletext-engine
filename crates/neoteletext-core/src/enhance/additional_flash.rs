//! The Additional Flash Functions (Table 29 mode 00111, decision E.48),
//! compiled in with the feature `additional-flash-functions`.

use super::attributes::{Flash, Rate};
use crate::consts::{
    FLASH_MODE_ADJACENT_CLUT, FLASH_MODE_INVERT_PHASE, FLASH_MODE_NORMAL, FLASH_RATE_DECREMENTAL,
    FLASH_RATE_INCREMENTAL, FLASH_RATE_PHASE_1, FLASH_RATE_PHASE_2, FLASH_RATE_PHASE_3, FLASH_RATE_SLOW,
};
use crate::level1::{FlashMode, FlashRate};

/// The flash an Additional Flash Functions triplet sets (Table 29 mode
/// 00111, decision E.48); `None` for its reserved values, which only set the
/// column (REQ-L25-027).
pub(super) fn flash_of(mode: u8, rate: u8, reserved: u8, column: u8) -> Option<Flash> {
    if reserved != 0 {
        return None;
    }
    let rate = match rate {
        FLASH_RATE_SLOW => Rate::Fixed(FlashRate::Slow),
        FLASH_RATE_PHASE_1 => Rate::Fixed(FlashRate::FastPhase1),
        FLASH_RATE_PHASE_2 => Rate::Fixed(FlashRate::FastPhase2),
        FLASH_RATE_PHASE_3 => Rate::Fixed(FlashRate::FastPhase3),
        FLASH_RATE_INCREMENTAL => Rate::Incremental,
        FLASH_RATE_DECREMENTAL => Rate::Decremental,
        _ => return None,
    };
    let mode = match mode {
        FLASH_MODE_NORMAL => FlashMode::Normal,
        FLASH_MODE_INVERT_PHASE => FlashMode::InvertPhase,
        FLASH_MODE_ADJACENT_CLUT => FlashMode::AdjacentClut,
        _ => return Some(Flash::Steady),
    };
    Some(Flash::On { mode, rate, start: column })
}
