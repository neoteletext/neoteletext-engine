//! Flash (decisions E.12 and E.48, REQ-RND-040): the phase of the flash
//! cycle, and whether a flashing cell is shown.

use crate::config::Config;
use crate::consts::{FLASH_CYCLE_US, FLASH_FAST_HALF_UNITS, FLASH_FAST_OFF_UNITS, FLASH_SLOW_ON_UNITS, FLASH_UNITS};
use crate::level1::{Cell, FlashMode, FlashRate};
use crate::types::Instant;

/// Which flash rates show their "on" phase at one moment (decision E.48,
/// REQ-RND-040). It is computed once per frame, from the flash unit, so the
/// cell raster only tests a bit (REQ-GEN-016).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct FlashPhase {
    /// Bit r set when [`FlashRate`] number r (slow, then fast phases 1 to
    /// 3) is on.
    on: u8,
}

/// The bit of a flash rate in [`FlashPhase`].
const fn rate_bit(rate: FlashRate) -> u8 {
    match rate {
        FlashRate::Slow => 0b0001,
        FlashRate::FastPhase1 => 0b0010,
        FlashRate::FastPhase2 => 0b0100,
        FlashRate::FastPhase3 => 0b1000,
    }
}

impl FlashPhase {
    /// Every rate on: flashing characters in their "on" phase, for still
    /// images.
    pub const STEADY: Self = Self {
        on: rate_bit(FlashRate::Slow)
            | rate_bit(FlashRate::FastPhase1)
            | rate_bit(FlashRate::FastPhase2)
            | rate_bit(FlashRate::FastPhase3),
    };

    /// The phase of flash unit `unit`, 0 to 11 (taken modulo 12).
    #[must_use]
    pub const fn at_unit(unit: u8) -> Self {
        let rates = [FlashRate::Slow, FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3];
        let mut on = 0;
        let mut index = 0;
        while index < rates.len() {
            // `index` is below the array's constant length.
            #[allow(clippy::indexing_slicing)]
            let rate = rates[index];
            if flash_on(rate, unit) {
                on |= rate_bit(rate);
            }
            index = index.wrapping_add(1);
        }
        Self { on }
    }

    /// The phase at a time on the caller's clock (REQ-GEN-005).
    #[must_use]
    pub fn at(now: Instant) -> Self {
        Self::at_unit(flash_unit(now))
    }

    /// True while a character of this rate shows its "on" phase.
    #[must_use]
    pub const fn is_on(self, rate: FlashRate) -> bool {
        self.on & rate_bit(rate) != 0
    }
}

/// The flash unit, 0 to 11, of a time on the caller's clock: the twelfth
/// of the one-second flash cycle it falls in (decision E.48, REQ-RND-040).
#[must_use]
pub fn flash_unit(now: Instant) -> u8 {
    let within = now.0.checked_rem(FLASH_CYCLE_US).unwrap_or(0);
    (within.saturating_mul(u64::from(FLASH_UNITS)) / FLASH_CYCLE_US) as u8
}

/// True when a character of `rate` is in its "on" phase in flash unit
/// `unit` (decision E.48, REQ-RND-040):
///
/// - slow flash (1 hertz) is on in units 0 to 8 and off in 9 to 11;
/// - fast flash (2 hertz) with phase p is off in units 2(p − 1) and 2p − 1
///   of each half-second of six units, and on otherwise.
///
/// Invert phase flash swaps the phases; that is the cell raster's business.
#[must_use]
pub const fn flash_on(rate: FlashRate, unit: u8) -> bool {
    let unit = unit % FLASH_UNITS;
    let off_pair = match rate {
        FlashRate::Slow => return unit < FLASH_SLOW_ON_UNITS,
        FlashRate::FastPhase1 => 0,
        FlashRate::FastPhase2 => 1,
        FlashRate::FastPhase3 => 2,
    };
    (unit % FLASH_FAST_HALF_UNITS) / FLASH_FAST_OFF_UNITS != off_pair
}

/// True when the cell is in its flash "on" phase: the rate's phase,
/// swapped for inverted flash (decision E.48).
pub(super) fn flash_shown(cell: &Cell, flash: FlashPhase) -> bool {
    let attributes = cell.attributes;
    flash.is_on(attributes.flash_rate()) != (attributes.flash_mode() == FlashMode::InvertPhase)
}

/// The flash phase at a time at Levels 1 and 1.5 (decision E.12): true
/// while shown. Levels 2.5 and 3.5 use [`FlashPhase::at`].
#[must_use]
pub fn flash_visible(now: Instant, config: &Config) -> bool {
    let period = config.flash_shown_us.saturating_add(config.flash_hidden_us);
    if period == 0 {
        return true;
    }
    now.0.checked_rem(period).unwrap_or(0) < config.flash_shown_us
}
