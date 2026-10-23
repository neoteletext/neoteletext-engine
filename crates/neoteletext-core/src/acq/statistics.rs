//! Statistics (REQ-ARCH-007) and the faults met at Levels 2.5 and 3.5
//! (REQ-L25-051), with the build they are counted for.

use super::StoredPage;
use crate::config::PresentationLevel;
use crate::consts::MAGAZINES;
use crate::fsm::PageKey;

/// Counters the caller can read and reset (REQ-ARCH-007).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Statistics {
    /// Packets received per magazine, index 0 for magazine 8.
    pub per_magazine: [u32; MAGAZINES],
    /// Packets received per packet number.
    pub per_packet_number: [u32; 32],
    /// Packets discarded for an uncorrectable address.
    pub address_failures: u32,
    /// Uncorrectable designation codes.
    pub designation_failures: u32,
    /// Triplets that failed Hamming 24/18.
    pub triplet_failures: u32,
    /// Bytes that failed parity.
    pub parity_failures: u32,
    /// Rows discarded with no page in progress, or out of sequence.
    pub discarded: u32,
    /// Packets counted and ignored.
    pub ignored: u32,
    /// Pages started.
    pub pages_started: u32,
    /// Pages completed.
    pub pages_completed: u32,
    /// Packets of enhancement pages dropped because every slot of the
    /// enhancement store belonged to a sub-page being received.
    pub enhancement_dropped: u32,
    /// Faults met while building pages at Levels 2.5 and 3.5 (REQ-L25-051),
    /// added by [`Decoder::record_enhancement`].
    pub enhancement: EnhancementFaults,
}

/// The faults one build of a page met at Levels 2.5 and 3.5, one count for
/// each kind (REQ-L25-051). The enhancement processor fills it; the decoder
/// keeps the sum in its [`Statistics`]. It is defined here, with the
/// statistics, because acquisition may not use the presentation layer
/// (ARCHITECTURE.md section 9).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EnhancementFaults {
    /// Triplets that failed their Hamming 24/18 check and were skipped.
    pub damaged_triplets: u32,
    /// Object invocations skipped by the nesting rules of REQ-OBJ-008 or
    /// with an illegal source.
    pub forbidden_invocations: u32,
    /// Invocations whose object was absent: no link, no page, no pointer,
    /// no matching definition, or a missing packet (REQ-OBJ-003 to
    /// REQ-OBJ-006).
    pub absent_objects: u32,
    /// Invocations ignored because a limit of Annex A.8 was reached.
    pub limited_invocations: u32,
    /// Characters and attributes discarded by REQ-OBJ-014 and REQ-L25-050.
    pub discarded_writes: u32,
    /// Modified designations ignored: reserved values, or a third pair at
    /// Level 2.5 (REQ-L25-012).
    pub ignored_designations: u32,
    /// Cells showing a space for an undefined G2 or G3 position (decision
    /// E.50).
    pub undefined_characters: u32,
}

impl EnhancementFaults {
    /// No faults.
    pub const NONE: Self = Self {
        damaged_triplets: 0,
        forbidden_invocations: 0,
        absent_objects: 0,
        limited_invocations: 0,
        discarded_writes: 0,
        ignored_designations: 0,
        undefined_characters: 0,
    };

    /// Adds `other`'s counts to these, saturating.
    pub const fn add(&mut self, other: &Self) {
        self.damaged_triplets = self.damaged_triplets.saturating_add(other.damaged_triplets);
        self.forbidden_invocations = self.forbidden_invocations.saturating_add(other.forbidden_invocations);
        self.absent_objects = self.absent_objects.saturating_add(other.absent_objects);
        self.limited_invocations = self.limited_invocations.saturating_add(other.limited_invocations);
        self.discarded_writes = self.discarded_writes.saturating_add(other.discarded_writes);
        self.ignored_designations = self.ignored_designations.saturating_add(other.ignored_designations);
        self.undefined_characters = self.undefined_characters.saturating_add(other.undefined_characters);
    }
}

/// What one build of a grid found, for the decoder's statistics and the
/// trace (REQ-L25-051, REQ-ARCH-007, REQ-TST-019). Fault counts are of the
/// page-level run of the walk and of the row resolution; `steps` counts
/// every triplet read in every run. Pass `build` and `faults` to
/// `Decoder::record_enhancement` once the grid is built.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnhancementReport {
    /// The page, reception and level the grid was built from.
    pub build: PageBuild,
    /// The level the page was processed at: Level 2.5 when a Level 3.5
    /// decoder falls back to the table's Level 2.5 links (clause 10.6.1).
    pub level: PresentationLevel,
    /// The faults met, one count for each kind.
    pub faults: EnhancementFaults,
    /// Objects run.
    pub objects: u32,
    /// Triplets read, over every run of the walk.
    pub steps: u32,
    /// Modified G0 and G2 Character Set Designation triplets applied
    /// (Table 34, REQ-L25-012).
    pub designations: u32,
}

#[cfg(feature = "level25")]
impl EnhancementReport {
    pub(crate) const fn new(build: PageBuild, level: PresentationLevel) -> Self {
        Self { build, level, faults: EnhancementFaults::NONE, objects: 0, steps: 0, designations: 0 }
    }
}

/// Which build of a page an enhancement report belongs to: the page, the
/// reception it was built from, and the presentation level asked for. The
/// decoder uses it so that redrawing a page does not count its faults
/// again ([`Decoder::record_enhancement`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PageBuild {
    /// The page and sub-page.
    pub key: PageKey,
    /// The page's reception count when it was built ([`StoredPage::receptions`]).
    pub receptions: u32,
    /// The presentation level of the configuration it was built with.
    pub level: PresentationLevel,
}

impl PageBuild {
    /// The build of `page` at `level`.
    #[must_use]
    pub const fn of(page: &StoredPage, level: PresentationLevel) -> Self {
        Self { key: page.key, receptions: page.receptions, level }
    }
}
