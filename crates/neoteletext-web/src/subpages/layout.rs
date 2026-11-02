//! A channel's pages written as its files (REQ-WEB-019, decision E.132;
//! feature `std`): a page of one sub-page as `NNN.t42`; a page of several
//! as `NNN.t42` holding its first sub-page, and in its folder `NNN/SSSS.t42`
//! for each sub-page and the parts `NNN/P.sub` listing them; and `index.t42` holding first
//! sub-pages only, so that a channel's opening costs the same at any
//! number of sub-pages (`STANDARD-LIMITS.md` section 4 items 7 and 8).
//!
//! The pages come as T42 already encoded: each sub-page's packets, ended
//! so that a reader completes it (a header that ends the page, as a
//! time-filling header does, EN 300 706 clause 7.2.1). The writer that has
//! the pages (the author API, a server) encodes them; this module only
//! lays them out.

use std::collections::BTreeMap;
use std::string::String;
use std::vec::Vec;

use neoteletext_core::types::{PageAddress, SubCode};

use super::{
    MAX_BYTES, MAX_ENTRIES, MAX_SUBPAGE_FILE, PartNumber, ROWS_PER_PART, SUBCODE_DIGITS, Validator, parse_subcode,
    write_entry, write_header, write_subcode,
};
use crate::bytes::Writer;
use crate::channel::{self, BUNDLE_FILE, ChannelFile, MAX_FILE_NAME};
use crate::consts::DEFAULT_PAGE_LIMIT;

/// One sub-page of a page, encoded.
#[derive(Clone, Copy, Debug)]
pub struct SubPageFile<'a> {
    /// Its sub-code, which names its file.
    pub subcode: SubCode,
    /// Its packets, as T42.
    pub t42: &'a [u8],
    /// Its title, made valid as [`write_entry`] says.
    pub title: &'a str,
}

/// Why a page could not be laid out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayoutError {
    /// A page with no sub-page.
    Empty,
    /// More than [`MAX_ENTRIES`] sub-pages.
    TooMany,
    /// Two sub-pages with one sub-code.
    Duplicate(SubCode),
    /// A sub-page's T42 is larger than its file may be (REQ-1904), or is not
    /// whole packets with valid addresses (REQ-WEB-027): the sub-page's
    /// sub-code, or `None` for a page laid out whole.
    Content(Option<SubCode>),
    /// A sub-code that names no sub-page's file, such as 3F7F, on a page whose
    /// sub-pages are files of their own: a reader would refuse its name.
    SubCode(SubCode),
    /// The same page laid out twice.
    PageTwice(PageAddress),
}

/// A file's name.
fn name(file: ChannelFile) -> String {
    let mut buffer = [0u8; MAX_FILE_NAME];
    let mut writer = Writer::new(&mut buffer);
    // Every name fits: MAX_FILE_NAME is the longest.
    let _ = channel::write_file_name(&mut writer, file);
    String::from_utf8_lossy(writer.written()).into_owned()
}

/// True when a sub-page's file may have this sub-code in its name, by the
/// reader that will read that name.
fn names_a_file(subcode: SubCode) -> bool {
    let mut digits = [0u8; SUBCODE_DIGITS];
    let mut writer = Writer::new(&mut digits);
    write_subcode(&mut writer, subcode).is_ok() && parse_subcode(writer.written()).is_some()
}

/// A page's sub-pages in the page's order, ascending by sub-code (REQ-1890),
/// whatever order they were given in.
fn in_page_order<'a, 'b>(sub_pages: &'a [SubPageFile<'b>]) -> Vec<&'a SubPageFile<'b>> {
    let mut ordered: Vec<_> = sub_pages.iter().collect();
    ordered.sort_by_key(|sub_page| sub_page.subcode);
    ordered
}

/// The files of one page: `NNN.t42` for a page of one sub-page; otherwise
/// `NNN.t42` holding the first, then each sub-page's `NNN/SSSS.t42` and the
/// parts of its list `NNN/P.sub`, in the page's order (REQ-1887, REQ-1888).
pub fn page_files(page: PageAddress, sub_pages: &[SubPageFile<'_>]) -> Result<Vec<(String, Vec<u8>)>, LayoutError> {
    let sub_pages = in_page_order(sub_pages);
    let Some(first) = sub_pages.first() else { return Err(LayoutError::Empty) };
    if sub_pages.len() > MAX_ENTRIES {
        return Err(LayoutError::TooMany);
    }
    // A page of one sub-page is one page file, with a page file's bound.
    let in_folder = sub_pages.len() > 1;
    let limit = if in_folder { MAX_SUBPAGE_FILE } else { DEFAULT_PAGE_LIMIT };
    let mut seen = std::collections::BTreeSet::new();
    for sub_page in &sub_pages {
        if !seen.insert(sub_page.subcode) {
            return Err(LayoutError::Duplicate(sub_page.subcode));
        }
        if in_folder && !names_a_file(sub_page.subcode) {
            return Err(LayoutError::SubCode(sub_page.subcode));
        }
        if sub_page.t42.len() > limit || !channel::is_t42(sub_page.t42) {
            return Err(LayoutError::Content(Some(sub_page.subcode)));
        }
    }
    let mut files = std::vec![(name(ChannelFile::Page { page, t42: true }), first.t42.to_vec())];
    if !in_folder {
        return Ok(files);
    }
    for sub_page in &sub_pages {
        let file = ChannelFile::SubPage { page, subcode: sub_page.subcode, t42: true };
        files.push((name(file), sub_page.t42.to_vec()));
    }
    let version = Validator::of_page(sub_pages.iter().map(|sub_page| sub_page.t42));
    // MAX_ENTRIES was checked above, so the count fits and every part has a number.
    let total = u16::try_from(sub_pages.len()).unwrap_or(u16::MAX);
    for (number, rows) in sub_pages.chunks(ROWS_PER_PART).enumerate() {
        let Some(part) = PartNumber::new(number) else { return Err(LayoutError::TooMany) };
        let mut bytes = std::vec![0u8; MAX_BYTES];
        let mut writer = Writer::new(&mut bytes);
        // Room for the header and every row was made above.
        let _ = write_header(&mut writer, total, &version);
        for sub_page in rows {
            let _ = write_entry(&mut writer, sub_page.subcode, &Validator::of(sub_page.t42), sub_page.title);
        }
        let length = writer.len();
        bytes.truncate(length);
        files.push((name(ChannelFile::SubPages { page, part }), bytes));
    }
    Ok(files)
}

/// One page's files, and what it would put in the bundle.
#[derive(Clone, Debug)]
struct LaidOut {
    files: Vec<(String, Vec<u8>)>,
    bundled: Vec<u8>,
}

/// A channel's files, page by page, in page order whatever order they were
/// added in, so that the bundle does not depend on it (REQ-1893).
#[derive(Clone, Debug, Default)]
pub struct ChannelFiles {
    pages: BTreeMap<PageAddress, LaidOut>,
}

impl ChannelFiles {
    /// No pages yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a page's files ([`page_files`]), with its first sub-page for the
    /// bundle.
    pub fn add_page(&mut self, page: PageAddress, sub_pages: &[SubPageFile<'_>]) -> Result<(), LayoutError> {
        if self.pages.contains_key(&page) {
            return Err(LayoutError::PageTwice(page));
        }
        let files = page_files(page, sub_pages)?;
        let bundled = in_page_order(sub_pages).first().map_or(&[][..], |first| first.t42).to_vec();
        self.pages.insert(page, LaidOut { files, bundled });
        Ok(())
    }

    /// Adds a page whose sub-pages are not files of their own, as one
    /// `NNN.t42` holding all of them, to be bundled whole: an enhancement
    /// data page, whose sub-pages a display page asks for together
    /// (REQ-WEB-022) and which are never listed as sub-pages (REQ-NUM-022).
    pub fn add_whole_page(&mut self, page: PageAddress, t42: &[u8]) -> Result<(), LayoutError> {
        if self.pages.contains_key(&page) {
            return Err(LayoutError::PageTwice(page));
        }
        if t42.len() > DEFAULT_PAGE_LIMIT || !channel::is_t42(t42) {
            return Err(LayoutError::Content(None));
        }
        let files = std::vec![(name(ChannelFile::Page { page, t42: true }), t42.to_vec())];
        self.pages.insert(page, LaidOut { files, bundled: t42.to_vec() });
        Ok(())
    }

    /// How many pages, from the lowest, the bundle holds, and their bytes
    /// (REQ-1893): it stops before the first page that would pass the bound,
    /// so what it holds never depends on which later pages happen to be small.
    fn bundled(&self) -> (usize, usize) {
        let mut pages = 0usize;
        let mut bytes = 0usize;
        for laid_out in self.pages.values() {
            let after = bytes.saturating_add(laid_out.bundled.len());
            if after > DEFAULT_PAGE_LIMIT {
                break;
            }
            bytes = after;
            pages = pages.saturating_add(1);
        }
        (pages, bytes)
    }

    /// Pages left out of the bundle; each still has its own files
    /// (REQ-WEB-019: a channel larger than the bundle publishes page files).
    #[must_use]
    pub fn unbundled(&self) -> usize {
        self.pages.len().saturating_sub(self.bundled().0)
    }

    /// Every file in page order, `index.t42` last when any page was bundled.
    #[must_use]
    pub fn finish(self) -> Vec<(String, Vec<u8>)> {
        let (kept, bytes) = self.bundled();
        let mut bundle = Vec::with_capacity(bytes);
        let mut files = Vec::new();
        for (index, laid_out) in self.pages.into_values().enumerate() {
            if index < kept {
                bundle.extend_from_slice(&laid_out.bundled);
            }
            files.extend(laid_out.files);
        }
        if !bundle.is_empty() {
            files.push((String::from(BUNDLE_FILE), bundle));
        }
        files
    }
}

#[cfg(test)]
mod tests;
