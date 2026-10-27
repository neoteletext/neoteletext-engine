//! The Magazine Inventory Page (clause 11.3, Figures 25 and 26, Table 24;
//! REQ-NUM-026), read from the 40 stored data bytes of its packets, which
//! the enhancement store keeps as received (decision E.56).
//!
//! Packets X/1 to X/14 give a page code for each page number 00 to FF, a
//! pair of Hamming 8/4 bytes, the lower nibble first (clause 11.3.3), in
//! the order of Figure 26, which is the order of the MOT's Figure 20: the
//! decimal page numbers first. Packets X/15 to X/24 give the number of
//! sub-pages for the codes that need it, thirteen bits in groups of three
//! bytes, in the order those codes occur (clause 11.3.4).

use super::magazine_table::{MotPosition, mot_position};
use crate::consts::{
    DATA_BYTES, MIP_ACI, MIP_BROADCASTER_SYSTEM, MIP_CODE_LAST_PACKET, MIP_COUNT_D12_FROM_CODE, MIP_COUNT_FIRST_INDEX,
    MIP_COUNT_FIRST_PACKET, MIP_COUNT_GROUP_BITS, MIP_COUNT_GROUP_BYTES, MIP_COUNT_GROUPS_PER_PACKET,
    MIP_COUNT_LAST_PACKET, MIP_COUNTED, MIP_DATA_BROADCASTING, MIP_DRCS, MIP_DRCS_UNDEFINED, MIP_ENGINEERING,
    MIP_KEYWORD_MULTI, MIP_KEYWORD_SINGLE, MIP_MULTI_PAGE, MIP_MULTI_PAGE_LARGE, MIP_MULTI_PAGE_LARGEST,
    MIP_NIBBLE_BITS, MIP_NOT_FOLLOWING_RULES, MIP_NOT_IN_TRANSMISSION, MIP_NOT_PUBLIC, MIP_NOW_AND_NEXT, MIP_OBJECT,
    MIP_OBJECT_UNDEFINED, MIP_PROGRAMME_MULTI, MIP_PROGRAMME_SINGLE, MIP_SCHEDULE_LARGE, MIP_SCHEDULE_LARGEST,
    MIP_SCHEDULE_MULTI, MIP_SCHEDULE_OFFSET, MIP_SCHEDULE_SINGLE, MIP_SINGLE_PAGE, MIP_SUBTITLE, MIP_SUBTITLE_MENU,
    MIP_SYSTEM_DISPLAY, MIP_SYSTEM_NO_DISPLAY, MIP_TOP, MIP_TRIGGER, MIP_TV_INDEX_MULTI, MIP_TV_INDEX_SINGLE,
    MIP_TV_WARNING,
};
use crate::ecc::hamming84;
use crate::types::PageNumber;

/// What a page code says a page is (Table 24).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum InventoryFunction {
    /// 00: not in transmission.
    NotInTransmission,
    /// 01 to 51: a normal page.
    Normal,
    /// 70 to 77: a subtitle page; the value is the three low bits, C12 to
    /// C14 of its header.
    Subtitle(u8),
    /// 78: the subtitle menu page.
    SubtitleMenu,
    /// 79: a page not following the normal sub-code rules (REQ-NUM-021).
    NotFollowingRules,
    /// 7A: a TV programme related warning page.
    TvWarning,
    /// 7B, 7C: current TV programme information.
    Programme,
    /// 7D: "Now and Next" TV programmes.
    NowAndNext,
    /// 7E, 7F: an index page to TV-related pages.
    TvIndex,
    /// 80: transmitted but not part of the public service.
    NotPublic,
    /// 81 to D1: TV schedule pages.
    Schedule,
    /// E0 to E4: data broadcasting (EN 300 708; REQ-NUM-024).
    DataBroadcasting,
    /// E5, E8 to EB: a DRCS page.
    Drcs,
    /// E6, EC to EF: an object page.
    Object,
    /// E7: a systems page without displayable element, such as the MOT and
    /// the MIP.
    SystemWithoutDisplay,
    /// F0 to F3: systems pages for broadcasters' use.
    BroadcasterSystem,
    /// F4 to F6: engineering test pages.
    Engineering,
    /// F7: a systems page with a displayable element.
    SystemWithDisplay,
    /// F8, F9: a keyword search list page.
    KeywordSearch,
    /// FC: trigger messages.
    Trigger,
    /// FD: Automatic Channel Installation.
    ChannelInstallation,
    /// FE: a TOP page (REQ-NUM-028).
    Top,
    /// 52 to 6F, D2 to DF, FA, FB and FF.
    Reserved,
}

/// A page code of the Magazine Inventory Page (Table 24).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct PageCode(pub u8);

/// True when `code` is within the inclusive range `[first, last]`.
const fn within(code: u8, [first, last]: [u8; 2]) -> bool {
    code >= first && code <= last
}

/// The first code of an inclusive range.
const fn first_of([first, _]: [u8; 2]) -> u8 {
    first
}

impl PageCode {
    /// What the code says the page is (Table 24).
    #[must_use]
    pub const fn function(self) -> InventoryFunction {
        let code = self.0;
        match code {
            MIP_NOT_IN_TRANSMISSION => InventoryFunction::NotInTransmission,
            MIP_SINGLE_PAGE | MIP_MULTI_PAGE_LARGE | MIP_MULTI_PAGE_LARGEST => InventoryFunction::Normal,
            _ if within(code, MIP_MULTI_PAGE) => InventoryFunction::Normal,
            _ if within(code, MIP_SUBTITLE) => InventoryFunction::Subtitle(code.wrapping_sub(first_of(MIP_SUBTITLE))),
            MIP_SUBTITLE_MENU => InventoryFunction::SubtitleMenu,
            MIP_NOT_FOLLOWING_RULES => InventoryFunction::NotFollowingRules,
            MIP_TV_WARNING => InventoryFunction::TvWarning,
            MIP_PROGRAMME_MULTI | MIP_PROGRAMME_SINGLE => InventoryFunction::Programme,
            MIP_NOW_AND_NEXT => InventoryFunction::NowAndNext,
            MIP_TV_INDEX_MULTI | MIP_TV_INDEX_SINGLE => InventoryFunction::TvIndex,
            MIP_NOT_PUBLIC => InventoryFunction::NotPublic,
            MIP_SCHEDULE_SINGLE | MIP_SCHEDULE_LARGE | MIP_SCHEDULE_LARGEST => InventoryFunction::Schedule,
            _ if within(code, MIP_SCHEDULE_MULTI) => InventoryFunction::Schedule,
            _ if within(code, MIP_DATA_BROADCASTING) => InventoryFunction::DataBroadcasting,
            MIP_DRCS_UNDEFINED => InventoryFunction::Drcs,
            _ if within(code, MIP_DRCS) => InventoryFunction::Drcs,
            MIP_OBJECT_UNDEFINED => InventoryFunction::Object,
            _ if within(code, MIP_OBJECT) => InventoryFunction::Object,
            MIP_SYSTEM_NO_DISPLAY => InventoryFunction::SystemWithoutDisplay,
            _ if within(code, MIP_BROADCASTER_SYSTEM) => InventoryFunction::BroadcasterSystem,
            _ if within(code, MIP_ENGINEERING) => InventoryFunction::Engineering,
            MIP_SYSTEM_DISPLAY => InventoryFunction::SystemWithDisplay,
            MIP_KEYWORD_MULTI | MIP_KEYWORD_SINGLE => InventoryFunction::KeywordSearch,
            MIP_TRIGGER => InventoryFunction::Trigger,
            MIP_ACI => InventoryFunction::ChannelInstallation,
            MIP_TOP => InventoryFunction::Top,
            _ => InventoryFunction::Reserved,
        }
    }

    /// The number of sub-pages the code itself gives: 1 for a single page
    /// (01, 81), the code for 02 to 4F, the code less 80 for 82 to CF.
    #[must_use]
    pub const fn sub_pages_in_code(self) -> Option<u16> {
        let code = self.0;
        match code {
            MIP_SINGLE_PAGE | MIP_SCHEDULE_SINGLE => Some(1),
            _ if within(code, MIP_MULTI_PAGE) => Some(code as u16),
            _ if within(code, MIP_SCHEDULE_MULTI) => Some(code.wrapping_sub(MIP_SCHEDULE_OFFSET) as u16),
            _ => None,
        }
    }

    /// True when the code's number of sub-pages is in packets X/15 to X/24
    /// (clause 11.3.4).
    #[must_use]
    pub fn is_counted(self) -> bool {
        MIP_COUNTED.contains(&self.0)
    }

    /// The count's D12 for codes that carry it in their least significant
    /// bit (50 and 51, D0 and D1, E0 and E1), `None` for the others.
    #[must_use]
    pub fn count_d12(self) -> Option<u16> {
        MIP_COUNT_D12_FROM_CODE.contains(&self.0).then_some(u16::from(self.0 & 1))
    }
}

/// Where the byte pair of page `page` lies in its Magazine Inventory Page:
/// Figure 26 lays the pairs out as the MOT's Figure 20 does.
#[must_use]
pub fn mip_position(page: PageNumber) -> MotPosition {
    mot_position(page)
}

/// The page code of page `page`, from `row`, which gives the stored data
/// bytes of packet X/n. A packet of X/1 to X/14 not sent describes no page
/// in transmission (clause 11.3.2), so its pages read 00; `None` when a
/// byte of the pair is uncorrectable (decision E.7).
pub fn page_code<'r>(row: &impl Fn(u8) -> Option<&'r [u8; DATA_BYTES]>, page: PageNumber) -> Option<PageCode> {
    let position = mip_position(page);
    if position.packet > MIP_CODE_LAST_PACKET {
        return None;
    }
    let Some(bytes) = row(position.packet) else { return Some(PageCode(MIP_NOT_IN_TRANSMISSION)) };
    let low = hamming84(*bytes.get(position.index)?)?;
    let high = hamming84(*bytes.get(position.index.saturating_add(1))?)?;
    Some(PageCode((high << MIP_NIBBLE_BITS) | low))
}

/// The number of sub-pages of page `page` (clauses 11.3.3 and 11.3.4): the
/// code's own count, or the thirteen-bit count of its group in X/15 to
/// X/24. `None` when the code gives none, when a byte needed is
/// uncorrectable, or when the group's packet has not been received.
pub fn sub_page_count<'r>(row: &impl Fn(u8) -> Option<&'r [u8; DATA_BYTES]>, page: PageNumber) -> Option<u16> {
    let code = page_code(row, page)?;
    if let Some(count) = code.sub_pages_in_code() {
        return Some(count);
    }
    if !code.is_counted() {
        return None;
    }
    // The group is the page's place among the counted codes, in the order
    // of Figure 26.
    let here = position_order(mip_position(page));
    let mut group = 0usize;
    for other in 0..=u8::MAX {
        let other = PageNumber(other);
        if position_order(mip_position(other)) < here && page_code(row, other)?.is_counted() {
            group = group.saturating_add(1);
        }
    }
    let packet =
        usize::from(MIP_COUNT_FIRST_PACKET).saturating_add(group.checked_div(MIP_COUNT_GROUPS_PER_PACKET).unwrap_or(0));
    if packet > usize::from(MIP_COUNT_LAST_PACKET) {
        return None;
    }
    let bytes = row(packet as u8)?;
    let in_packet = group.checked_rem(MIP_COUNT_GROUPS_PER_PACKET).unwrap_or(0);
    let first = MIP_COUNT_FIRST_INDEX.saturating_add(in_packet.saturating_mul(MIP_COUNT_GROUP_BYTES));
    let mut count = 0u16;
    for nibble in 0..MIP_COUNT_GROUP_BYTES {
        let value = hamming84(*bytes.get(first.saturating_add(nibble))?)?;
        count |= u16::from(value) << (MIP_NIBBLE_BITS.saturating_mul(nibble as u32));
    }
    Some(count | (code.count_d12().unwrap_or(0) << MIP_COUNT_GROUP_BITS))
}

/// A position's order in Figure 26: by packet, then by byte.
const fn position_order(position: MotPosition) -> (u8, usize) {
    (position.packet, position.index)
}
