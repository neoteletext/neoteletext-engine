//! What the decoder reports: the magazine data it keeps, why it discarded
//! a packet, and the events it raises (REQ-ARCH-005).

use super::EnhancementKey;
use crate::fsm::PageKey;
use crate::packet::PacketError;
use crate::trace::TraceEntry;
use crate::types::{Magazine, PageNumber, PresentationFields};

/// The magazine data of one magazine (REQ-ACQ-009, REQ-ACQ-018): the most
/// recent packet of each designation code, each replacing the last.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MagazineData {
    /// The latest M/29/0 with packet function 0 (REQ-PKT-016).
    pub data: Option<PresentationFields>,
    /// The latest M/29/4 with packet function 0: the Level 3.5 data.
    pub level35: Option<PresentationFields>,
}

/// Why a packet was discarded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Discard {
    /// An uncorrectable address or designation byte.
    Packet(PacketError),
    /// A row with no page being received in its magazine.
    RowWithoutHeader,
    /// A row of another magazine in serial mode (Annex B.3).
    OtherMagazine,
}

/// An event raised by the decoder (REQ-ARCH-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    /// A header was received, for the rolling header.
    HeaderReceived {
        /// Its magazine.
        magazine: Magazine,
        /// Its page number, if decodable.
        page: Option<PageNumber>,
    },
    /// A page reception started.
    PageStarted(PageKey),
    /// A row changed.
    PageUpdated {
        /// The page.
        key: PageKey,
        /// The packet number.
        row: u8,
    },
    /// A page reception completed.
    PageCompleted(PageKey),
    /// A page was erased by C4.
    PageErased(PageKey),
    /// C8 was seen, or the content changed (REQ-ACQ-010).
    UpdateIndicator(PageKey),
    /// The page has not been received for the configured period.
    PageStale(PageKey),
    /// Packet 8/30 was received.
    ServiceData,
    /// M/29 was received for a magazine, or the magazine's M/29 packets
    /// were discarded because the service changed (REQ-ACQ-018).
    MagazineData(Magazine),
    /// An enhancement sub-page was stored or changed during the reception
    /// that just ended, so pages that use it are to be built again
    /// (REQ-ACQ-022).
    EnhancementPageChanged(EnhancementKey),
    /// A packet was discarded.
    Discarded(Discard),
    /// A state machine transition (REQ-FSM-010).
    Trace(TraceEntry),
}
