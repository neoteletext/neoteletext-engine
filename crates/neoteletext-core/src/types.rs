//! Small shared types and fixed-capacity collections (Foundation).

mod address;
mod fixed_vec;
mod font_glyph;
mod presentation_fields;

pub use address::{ControlBits, Magazine, PageAddress, PageNumber, SubCode, hex_digit};
pub use fixed_vec::FixedVec;
pub use font_glyph::FontGlyph;
pub use presentation_fields::{
    ColourWords, DefaultColourFields, Designations, FallbackFields, PresentationFields, Rgb12, SidePanelFields,
};

/// A point in time in microseconds, from a monotonic clock supplied by the
/// caller (REQ-GEN-005).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default)]
pub struct Instant(pub u64);

impl Instant {
    /// Microseconds from `earlier` to `self`, zero if `earlier` is later.
    #[must_use]
    pub const fn since(self, earlier: Self) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

#[cfg(test)]
mod tests;
