//! The page encoder (REQ-ARCH-004, REQ-INP-019 and REQ-INP-024): turns a
//! [`PageFile`] into correctly coded packets in the order of Annex B.2,
//! so that page files take the same decoding path as broadcasts.
//!
//! The rows of a page are coded as its page coding says (Table 3,
//! REQ-INP-023): display pages as seven bits with odd parity,
//! object pages as a Hamming 8/4 byte and thirteen Hamming 24/18 triplets,
//! and the Magazine Organization Table as Hamming 8/4 nibbles. A page file
//! holds every row as forty 7-bit codes, which for the last two codings are
//! the packed characters of the TTI form ([`pack_triplets`], [`nibble_row`]).

use neoteletext_core::consts::{
    CLOCK_COLUMN, COLUMNS, DESIGNATION_CODES, HEADER_DISPLAY_COLUMN, MODE_TERMINATION, ROW_ADDRESS_LAST,
    TRIPLETS_PER_PACKET,
};
use neoteletext_core::packet::Triplet;
use neoteletext_core::types::{Magazine, PageNumber, SubCode};

use crate::page::FASTEXT_LINKS;

mod checksum;
mod link_row;
mod page;
mod page_data;
mod rows;
mod transmission;
mod triplets;

pub use checksum::{checksum_word, page_checksum};
pub use link_row::{LinkRow, link_row};
pub(crate) use link_row::{Prompt, draw_link_row, key_colour};
pub use page::encode_page;
pub use page_data::default_clut_2_and_3;
pub use rows::{RowCoding, is_enhancement_page, nibble_row, packed_row, row_bytes, row_codes, row_coding};
pub use transmission::{TransmissionClass, transmission_class};
pub use triplets::{pack_triplets, packed_line_of, raw_packet, unpack_triplets};

pub use neoteletext_core::consts::{DESIGNATION_GROUP_SHIFT, G2_RIGHT_GUILLEMET};
/// The Level 1 character written where the guillemet goes, for receivers
/// without X/26: ">" is the same in every Latin national option.
pub const GUILLEMET_FALLBACK: u8 = b'>';
/// Columns per link prompt: four equal prompts (REQ-NAV-011).
pub const PROMPT_COLUMNS: usize = COLUMNS / 4;
/// Label characters of a same-service prompt: the prompt less its colour
/// attribute (Appendix E.1).
pub const LABEL_CHARACTERS: usize = PROMPT_COLUMNS - 1;
/// Label characters of an other-service prompt, which also holds the mark.
pub const OTHER_SERVICE_LABEL_CHARACTERS: usize = LABEL_CHARACTERS - 1;
/// Header bytes the page checksum covers: bytes 14 to 37, the display
/// columns before the clock (clause 9.6.1, decision E.11).
pub const CHECKSUM_HEADER_BYTES: usize = CLOCK_COLUMN - HEADER_DISPLAY_COLUMN;
/// Triplets a page can carry in its sixteen X/26 packets.
const PAGE_TRIPLETS: usize = DESIGNATION_CODES * TRIPLETS_PER_PACKET;

/// The six X/27/0 link targets handed to the packet builder.
type LinkTargets = [Option<(Magazine, PageNumber, SubCode)>; FASTEXT_LINKS];

/// The Termination Marker (clause 12.3.1, Table 27).
const TERMINATION: Triplet = Triplet { address: ROW_ADDRESS_LAST, mode: MODE_TERMINATION, data: 0 };

#[cfg(test)]
mod tests;
