//! `neottx list` (REQ-CLI-001): the pages and sub-pages found, with their
//! counts and error statistics.

use super::sandbox_closed;
use neoteletext_app::Failure;
use neoteletext_app::input::{self, Options};
use neoteletext_core::acq::StoredPage;
use neoteletext_core::fsm::PageKey;
use std::io::Write;

/// `neottx list`.
pub fn list(options: &Options) -> Result<(), Failure> {
    let mut session = input::load(options)?;
    sandbox_closed(options.no_sandbox)?;
    let mut out = std::io::stdout().lock();
    let pages = session.pages();
    let page_count = pages.len();
    let keys: Vec<PageKey> = pages.iter().map(|page| page.key).collect();
    let mut index = 0;
    while let Some(first) = pages.get(index) {
        let address = first.key.address;
        let subpages: Vec<&&StoredPage> =
            pages.get(index..).unwrap_or_default().iter().take_while(|page| page.key.address == address).collect();
        let receptions: u32 = subpages.iter().map(|page| page.receptions).sum();
        let errors: u32 = subpages.iter().map(|page| page.errors).sum();
        let codes: Vec<String> = subpages.iter().map(|page| format!("{:04X}", page.key.subcode.value())).collect();
        let _ = writeln!(
            out,
            "{}  sub-pages {:>3}  receptions {:>5}  errors {:>5}  [{}]",
            String::from_utf8_lossy(&address.ascii()),
            subpages.len(),
            receptions,
            errors,
            codes.join(" ")
        );
        index = index.saturating_add(subpages.len());
    }
    // REQ-L25-051: enhancement faults are met only when a page is built, so
    // every page is built once to count them.
    let enhanced = session.decoder().config().level.is_enhanced();
    if enhanced {
        for key in keys {
            let _ = session.grid(key, None);
        }
    }
    let stats = session.decoder().statistics();
    let _ = writeln!(
        out,
        "\n{} page(s); packets per magazine 1-8: {:?}; address failures {}; parity failures {}; triplet failures {}; discarded {}",
        page_count,
        [1, 2, 3, 4, 5, 6, 7, 0].map(|m| stats.per_magazine.get(m).copied().unwrap_or(0)),
        stats.address_failures,
        stats.parity_failures,
        stats.triplet_failures,
        stats.discarded
    );
    if enhanced {
        let faults = &stats.enhancement;
        let _ = writeln!(
            out,
            "enhancement faults: damaged triplets {}; forbidden invocations {}; absent objects {}; limited invocations {}; discarded writes {}; ignored designations {}; undefined characters {}",
            faults.damaged_triplets,
            faults.forbidden_invocations,
            faults.absent_objects,
            faults.limited_invocations,
            faults.discarded_writes,
            faults.ignored_designations,
            faults.undefined_characters
        );
    }
    Ok(())
}
