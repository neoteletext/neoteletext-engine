//! `neottx qr` (REQ-CLI-012): text, such as a qrkeyboard join address, as
//! a QR code in a TTI page's mosaics.

use super::io_failure;
use super::page_number;
use neoteletext_app::{Failure, exit};
use neoteletext_qrkeyboard::qr::{Correction, Matrix};
use std::fmt::Write as _;
use std::path::Path;

/// TTI writes a code below 0x20 as Escape and the code plus 0x40.
const ESCAPE: char = '\x1b';
const ESCAPE_OFFSET: u8 = 0x40;
/// The last row a page's text may use; row 24 is the Fastext row.
const LAST_ROW: usize = 23;

/// A page whose rows from `row` hold the QR of `text` in mosaics, from
/// `column`, as TTI.
fn qr_page(text: &str, page: u16, row: usize, column: usize) -> Result<String, Failure> {
    let matrix = Matrix::new(text, Correction::Low).map_err(|error| (exit::USAGE, error.to_string()))?;
    let rows = matrix.mosaic_rows();
    let width = rows.first().map_or(0, Vec::len);
    if row.saturating_add(rows.len()).saturating_sub(1) > LAST_ROW
        || column.saturating_add(width) > neoteletext_core::consts::COLUMNS
    {
        return Err((
            exit::USAGE,
            format!("the code is {width} columns by {} rows: it does not fit there", rows.len()),
        ));
    }
    let mut tti = format!("PN,{page:03X}00\r\nSC,0000\r\nPS,8000\r\n");
    for (index, line) in rows.iter().enumerate() {
        let _ = write!(tti, "OL,{},", row.saturating_add(index));
        tti.push_str(&" ".repeat(column));
        for &code in line {
            if code < 0x20 {
                tti.push(ESCAPE);
                tti.push(char::from(code.saturating_add(ESCAPE_OFFSET)));
            } else {
                tti.push(char::from(code));
            }
        }
        tti.push_str("\r\n");
    }
    Ok(tti)
}

/// `neottx qr`: writes the QR of `text` as a TTI page.
pub fn qr(text: &str, page: &str, row: usize, column: usize, output: &Path) -> Result<(), Failure> {
    let tti = qr_page(text, page_number(page)?, row, column)?;
    std::fs::write(output, tti).map_err(|error| io_failure(output, error))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a test")]
mod tests {
    use super::*;

    #[test]
    fn a_join_address_is_written_as_a_page() {
        let join = "https://qrkeyboard.com/app#c=ABCDEFGHIJKLMNOP&k=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&h=AAAAAAAAAAAAAAAAAAAAAA";
        let tti = qr_page(join, 0x600, 2, 0).unwrap();
        assert!(tti.starts_with("PN,60000\r\n"));
        // The engine's own reader takes the page back.
        let mut pages = Vec::new();
        let mut reader = neoteletext_formats::tti::TtiReader::new();
        neoteletext_formats::Reader::feed(&mut reader, tti.as_bytes(), &mut |item| {
            if let neoteletext_formats::Item::Page(page) = item {
                pages.push(page.address.mpp());
            }
        });
        neoteletext_formats::Reader::finish(&mut reader, &mut |item| {
            if let neoteletext_formats::Item::Page(page) = item {
                pages.push(page.address.mpp());
            }
        });
        assert_eq!(pages, [0x600]);
        assert!(qr_page(join, 0x600, 20, 0).is_err(), "too low on the page");
    }
}
