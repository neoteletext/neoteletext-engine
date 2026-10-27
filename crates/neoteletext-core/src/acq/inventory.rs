//! The Magazine Inventory Page as the decoder uses it (clause 11.3;
//! REQ-NUM-021, REQ-NUM-024, REQ-NUM-026, REQ-NUM-028): each page's code
//! and number of sub-pages, and how its sub-code is read.

use super::{Decoder, Numbering};
use crate::packet::{InventoryFunction, PageCode, page_code, sub_page_count};
use crate::types::PageAddress;

impl Decoder<'_> {
    /// The page code the stored Magazine Inventory Page of the page's
    /// magazine gives it (clause 11.3.3, Table 24), or `None` when no MIP
    /// is stored or its pair is uncorrectable. A MIP is kept only by a
    /// decoder with a store for enhancement pages.
    #[must_use]
    pub fn inventory_code(&self, address: PageAddress) -> Option<PageCode> {
        let lookup = self.enhancement_lookup();
        let mip = lookup.magazine_inventory_page(address.magazine)?;
        page_code(&|packet| mip.row(packet), address.page)
    }

    /// The number of sub-pages the stored MIP gives a page, from its code
    /// or from packets X/15 to X/24 (clauses 11.3.3 and 11.3.4).
    #[must_use]
    pub fn inventory_sub_pages(&self, address: PageAddress) -> Option<u16> {
        let lookup = self.enhancement_lookup();
        let mip = lookup.magazine_inventory_page(address.magazine)?;
        sub_page_count(&|packet| mip.row(packet), address.page)
    }

    /// How a page's sub-code is read, by its MIP code: a page not
    /// following the sub-code rules keeps its latest version only
    /// (REQ-NUM-021), and a data page by its code is not a display page
    /// (REQ-NUM-022, REQ-NUM-024, REQ-NUM-028). With no MIP, every page is
    /// a display page until its page function says otherwise (REQ-NUM-002,
    /// REQ-NUM-027).
    pub(super) fn numbering(&self, address: PageAddress) -> Numbering {
        self.inventory_code(address).map_or(Numbering::SubPages, |code| match code.function() {
            InventoryFunction::NotFollowingRules => Numbering::LatestOnly,
            InventoryFunction::DataBroadcasting
            | InventoryFunction::Drcs
            | InventoryFunction::Object
            | InventoryFunction::SystemWithoutDisplay
            | InventoryFunction::BroadcasterSystem
            | InventoryFunction::Trigger
            | InventoryFunction::Top => Numbering::Data,
            _ => Numbering::SubPages,
        })
    }
}
