//! Every teletext constant, defined once (REQ-GEN-014).
//!
//! No other module may restate, round, bit-reverse or re-derive a teletext
//! value. Where a value exists in two forms, both are defined here side by
//! side with the conversion stated, and a test asserts that they agree.
//!
//! Sources are cited by clause or table of ETSI EN 300 706 V1.2.1 unless
//! another standard is named.

mod audio;
mod character_sets;
mod coding;
mod colour;
mod enhancement_packets;
mod geometry;
mod inventory;
mod object_pages;
mod packets;
mod pages;
mod signal;
mod spacing_attributes;
mod timing;
mod triplets;

pub use audio::*;
pub use character_sets::*;
pub use coding::*;
pub use colour::*;
pub use enhancement_packets::*;
pub use geometry::*;
pub use inventory::*;
pub use object_pages::*;
pub use packets::*;
pub use pages::*;
pub use signal::*;
pub use spacing_attributes::*;
pub use timing::*;
pub use triplets::*;

#[cfg(test)]
mod tests;
