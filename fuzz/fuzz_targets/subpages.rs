//! A part of a page's list of sub-pages (REQ-1888's `NNN/P.sub`, REQ-1904,
//! REQ-TST-040): its size and header checks, then every row the input's
//! lines hold, each written back and read again unchanged.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_web::bytes::Writer;
use neoteletext_web::subpages::{self, MAX_HEADER, MAX_LINE, ROWS_PER_PART};

fuzz_target!(|data: &[u8]| {
    let Ok(part) = subpages::read(data) else { return };
    let (total, version) = (part.total, part.version);
    assert!((1..=subpages::MAX_ENTRIES).contains(&usize::from(total)));
    let mut kept = 0usize;
    let mut last = None;
    for entry in part.entries {
        let Ok(entry) = entry else { continue };
        kept += 1;
        assert!(kept <= ROWS_PER_PART);
        assert!(last.is_none_or(|last| entry.subcode > last), "rows ascend");
        last = Some(entry.subcode);
        let mut line = [0u8; MAX_HEADER + MAX_LINE];
        let mut writer = Writer::new(&mut line);
        subpages::write_header(&mut writer, total, &version).unwrap();
        subpages::write_entry(&mut writer, entry.subcode, &entry.validator, entry.title.unwrap_or("")).unwrap();
        let mut again = subpages::read(writer.written()).unwrap();
        assert_eq!((again.total, again.version), (total, version));
        let again = again.entries.next().unwrap().unwrap();
        assert_eq!(again.subcode, entry.subcode);
        assert_eq!(again.validator, entry.validator);
        // The writer trims and folds spaces, so only a title already in
        // that form comes back the same.
        if let Some(title) = entry.title
            && !title.starts_with(' ')
            && !title.ends_with(' ')
            && !title.contains("  ")
        {
            assert_eq!(again.title, Some(title));
        }
    }
});
