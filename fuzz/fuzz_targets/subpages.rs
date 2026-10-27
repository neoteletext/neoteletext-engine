//! A page's catalogue of sub-pages (REQ-WEB-034's `NNN.sub`, decision
//! E.132, REQ-TST-040): its size check, then every entry the input's lines
//! hold, each written back and read again unchanged.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_web::bytes::Writer;
use neoteletext_web::subpages::{self, MAX_LINE};

fuzz_target!(|data: &[u8]| {
    let Ok(entries) = subpages::read(data) else { return };
    let mut kept = 0usize;
    for entry in entries {
        let Ok(entry) = entry else { continue };
        kept += 1;
        assert!(kept <= subpages::MAX_ENTRIES);
        let mut line = [0u8; MAX_LINE];
        let mut writer = Writer::new(&mut line);
        subpages::write_entry(&mut writer, entry.subcode, &entry.validator, entry.title.unwrap_or("")).unwrap();
        let again = subpages::read(writer.written()).unwrap().next().unwrap().unwrap();
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
