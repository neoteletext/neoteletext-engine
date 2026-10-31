//! The display planes of clause 13.2, in which the walk places its events.

use crate::consts::ADDRESS_COLUMNS;
use crate::level1::CharSource;
use crate::packet::ObjectKind;

/// The display planes of clause 13.2, which are also the sources of the
/// walk.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Plane {
    /// The page's local enhancement data.
    Local,
    /// An Active object.
    Active,
    /// An Adaptive object.
    Adaptive,
    /// A Passive object.
    Passive,
}

impl Plane {
    pub(super) const fn of(kind: ObjectKind) -> Self {
        match kind {
            ObjectKind::Active => Self::Active,
            ObjectKind::Adaptive => Self::Adaptive,
            ObjectKind::Passive => Self::Passive,
        }
    }

    /// REQ-OBJ-008: local data invokes anything; Active objects Adaptive
    /// and Passive ones; Adaptive objects Passive ones; Passive nothing.
    pub(super) const fn may_invoke(self, kind: ObjectKind) -> bool {
        match self {
            Self::Local => true,
            Self::Active => matches!(kind, ObjectKind::Adaptive | ObjectKind::Passive),
            Self::Adaptive => matches!(kind, ObjectKind::Passive),
            Self::Passive => false,
        }
    }

    /// REQ-OBJ-013: Full Screen Colour, Full Row Colour and Address Display
    /// Row 0 act only in local data and Active objects.
    pub(super) const fn sets_colours(self) -> bool {
        matches!(self, Self::Local | Self::Active)
    }

    pub(super) const fn source(self) -> CharSource {
        match self {
            Self::Local => CharSource::Local,
            Self::Active => CharSource::Active,
            Self::Adaptive => CharSource::Adaptive,
            Self::Passive => CharSource::Passive,
        }
    }
}

/// The type's index for the per-type limit.
pub(super) const fn type_index(kind: ObjectKind) -> usize {
    match kind {
        ObjectKind::Active => 0,
        ObjectKind::Adaptive => 1,
        ObjectKind::Passive => 2,
    }
}

/// A position in scan order, for the open area of Annex A.8: row-major over
/// the 72 columns of Annex A.4.
pub(super) fn scan_index(row: usize, column: usize) -> usize {
    row.saturating_mul(ADDRESS_COLUMNS).saturating_add(column.min(ADDRESS_COLUMNS))
}
