//! A channel's log lines (REQ-TST-040, REQ-INP-026): the `LG` reader alone,
//! and inside TTI files, where every line it passes on must hold what the
//! reader promises.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_formats::log::{MAX_LOG_LINES_PER_PAGE, MAX_LOG_TEXT_BYTES, read};
use neoteletext_formats::tti::TtiReader;
use neoteletext_formats::{Item, Reader};

/// What a passed-on line's text may never hold.
fn clean(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_LOG_TEXT_BYTES
        && text.chars().all(|character| {
            !character.is_control()
                && !matches!(character, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
        })
}

fuzz_target!(|data: &[u8]| {
    if let Ok(line) = read(data) {
        assert!(clean(line.text));
    }
    // The same bytes as a TTI file, and as the text of one LG line.
    // A page is passed on at the next `PN`, after its log lines: between
    // two pages come at most a page's lines and those before the first
    // `PN`.
    let mut reader = TtiReader::new();
    let mut since_page = 0_usize;
    let mut check = |item: Item<'_>| match item {
        Item::Log { line, .. } => {
            since_page += 1;
            assert!(clean(line.text));
            assert!(since_page <= 2 * MAX_LOG_LINES_PER_PAGE);
        }
        Item::Page(_) => since_page = 0,
        _ => {}
    };
    reader.feed(data, &mut check);
    reader.finish(&mut check);
    let mut wrapped = b"LG,".to_vec();
    wrapped.extend_from_slice(data);
    let mut reader = TtiReader::new();
    let mut ignore = |item: Item<'_>| {
        if let Item::Log { line, .. } = item {
            assert!(clean(line.text));
        }
    };
    reader.feed(&wrapped, &mut ignore);
    reader.finish(&mut ignore);
});
