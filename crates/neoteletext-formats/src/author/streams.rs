//! The authored Level 2.5 and 3.5 test stream (REQ-TST-024), written with
//! the builder of REQ-INP-025. Between them its pages use every triplet
//! mode of Tables 28 and 29, the Colour Map, side panels of each width,
//! every size, underline, invert, conceal, boxing, flash at each rate and
//! phase, the G1, G2 and G3 sets, Local, Public and Global
//! objects of each type and priority reached through the Magazine
//! Organization Table and through X/27/4 and X/27/5, default objects, the
//! variants of X/28 and M/29 that are read, and the worked examples of
//! clause 13.9 and Annex C.2. Pages 114 to 116 and 119 held DRCS
//! characters, which are no longer decoded (decision E.95), and page 111
//! the Level 3.5 font styles, which are no longer drawn (decision E.96);
//! their numbers are left unused.
//!
//! The stream is generated, never stored: `cargo xtask level25-stream`
//! writes it as T42 and TTI, and the tests build it.
//!
//! | Magazine | Pages | What they test |
//! |---|---|---|
//! | 1 | 100 to 113, 117, 118, 120 to 124, 1A0, 1E0, 1FE | the features one by one |
//! | 2, 3 | 200 to 202, 2A9, 2F0, 2FE, 3A0 | X/27/4 links over the table, across magazines |
//! | 4 | 400 to 402, 4A0, 4F0, 4FE | default objects and the fallback of a POP link |
//! | 5 | 500, 501, 5A0 to 5A2, 5FE | the Level 3.5 links of X/27/5 and X/22 to X/24 |
//! | 7 | 700 to 702, 7FF | magazine packets M/29 and their precedence |

use std::vec::Vec;

use neoteletext_core::consts::{ALPHA_BLACK, ALPHA_YELLOW, COLOUR_BLUE_BIT, COLOUR_GREEN_BIT};
use neoteletext_core::packet::{ObjectUsage, TripletFunction};
use neoteletext_core::types::{ColourWords, Magazine, PageAddress, Rgb12, SubCode};

use super::{AuthorError, DisplayPage, ObjectPage, ObjectRef, Result, Stream, Triplets, address, colour, row_address};

mod default_objects;
mod links;
mod magazine_1;
mod magazine_packets;
mod object_examples;
mod presentation;

use default_objects::default_objects;
use links::{level_35_links, links_over_the_table};
use magazine_1::magazine_1;
use magazine_packets::magazine_packets;

/// Objects required at both levels.
const BOTH: ObjectUsage = ObjectUsage(0b11);
/// Objects required at Level 3.5 only.
const LEVEL_35: ObjectUsage = ObjectUsage(0b10);
/// Row mode 11000, DRCS Mode (Table 28), with data choosing the normal
/// table's sub-table 0 at both levels, as page 113 sends it to show that
/// it is not acted on (decision E.95).
const DRCS_MODE: u8 = 0b11000;
const DRCS_MODE_DATA: u8 = 0x70;
/// Data bit D6 of DRCS Character Invocation: the normal table (Table 29).
const DRCS_NORMAL: u8 = 0x40;
/// A DRCS character number above 47, which Table 29 makes invalid.
const DRCS_INVALID_CHARACTER: u8 = 50;
/// Data bit D1 of Font Style: bold (Table 29), as page 113 sends it.
const FONT_STYLE_BOLD: u8 = 0x02;
/// Level 1 colours as Colour Map indices (decision E.43).
const RED: u8 = colour(0, 1);
const GREEN: u8 = colour(0, 2);
const YELLOW: u8 = colour(0, 3);
const BLUE: u8 = colour(0, 4);
const MAGENTA: u8 = colour(0, 5);
const CYAN: u8 = colour(0, 6);
/// The "pink" of the clause 13.9 example: CLUT 2 entry 0, whose default
/// is red 15, green 0, blue 5 (REQ-TST-016).
const PINK: u8 = colour(2, 0);
/// Level 1 spacing attributes not named in the core's constants.
const ALPHA_GREEN: u8 = ALPHA_BLACK | COLOUR_GREEN_BIT;
const ALPHA_BLUE: u8 = ALPHA_BLACK | COLOUR_BLUE_BIT;
/// Sub-page flags of a link to sub-page 0.
const SUBPAGE_0: u16 = 0b01;
/// A Table 34 designation: the Latin G0 set with the German national
/// option (Table 32 group 0, option 1).
const GERMAN: u8 = 0b000_0001;
/// The columns a swatch of one Colour Map entry covers.
const SWATCH_COLUMNS: u8 = 4;
/// The first column of the swatches, after a label.
const SWATCH_COLUMN: u8 = 8;
/// The Origin Modifier column offsets of the right and left side-panel
/// areas (clause 12.5.3: columns 40 to 55 and 56 to 71).
const RIGHT_PANEL_COLUMN: u8 = 40;
const LEFT_PANEL_COLUMN: u8 = 56;

fn magazine(number: u8) -> Result<Magazine> {
    Magazine::new(number).ok_or(AuthorError::BadPosition)
}

/// A display page with a header naming it and a yellow title in row 1.
fn page(mpp: u16, title: &[u8]) -> Result<DisplayPage> {
    let target = address(mpp)?;
    let mut page = DisplayPage::new(target, SubCode::new(0));
    let mut header = *b"NEOTELETEXT L2.5 mpp AUTHORED   ";
    if let Some(slot) = header.get_mut(14..17) {
        slot.copy_from_slice(&target.ascii());
    }
    page.header(&header);
    page.text(1, 0, &[&[ALPHA_YELLOW][..], title].concat())?;
    Ok(page)
}

/// A body that writes text from relative column `column` of the origin
/// row.
fn text_body(column: u8, text: &[u8]) -> Triplets {
    let mut body = Triplets::new();
    body.text(column, text);
    body
}

/// Set Active Position to a cell, which repeated changes nothing: used to
/// place a later triplet at a chosen place in the packets.
fn filler_position(row: u8, column: u8) -> Result<TripletFunction> {
    Ok(TripletFunction::SetActivePosition { address: row_address(row)?, column })
}

/// The colour words of a CLUT pair from sixteen `0xRGB` values.
fn words(values: [u16; 16]) -> ColourWords {
    values.map(Rgb12)
}

/// Swatches of every Colour Map entry, one CLUT per row from `first_row`,
/// as background colours over the spaces of the Level 1 rows.
fn swatches(page: &mut DisplayPage, first_row: u8) -> Result<()> {
    for clut in 0..4u8 {
        let row = first_row.saturating_add(clut.saturating_mul(2));
        page.text(row, 0, &[b'C', b'L', b'U', b'T', b' ', b'0'.saturating_add(clut)])?;
        page.local().position(row, 0)?;
        for entry in 0..8u8 {
            let column = SWATCH_COLUMN.saturating_add(entry.saturating_mul(SWATCH_COLUMNS));
            page.local().background(column, colour(clut, entry));
        }
    }
    Ok(())
}

/// The stream.
pub fn stream() -> Result<Stream> {
    let mut stream = Stream::new();
    magazine_1(&mut stream)?;
    links_over_the_table(&mut stream)?;
    default_objects(&mut stream)?;
    level_35_links(&mut stream)?;
    magazine_packets(&mut stream)?;
    Ok(stream)
}

/// The addresses of the display pages of the stream, in page order.
pub fn display_pages() -> Result<Vec<PageAddress>> {
    let stream = stream()?;
    let mut pages: Vec<PageAddress> = stream
        .pages()
        .into_iter()
        .filter(|page| crate::encode::transmission_class(page) == crate::encode::TransmissionClass::Display)
        .map(|page| page.address)
        .collect();
    pages.sort();
    pages.dedup();
    Ok(pages)
}

/// The objects of magazine 1 and the references that invoke them.
struct Objects {
    pop: ObjectPage,
    gpop: ObjectPage,
    pop_active: ObjectRef,
    pop_adaptive: ObjectRef,
    pop_passive: ObjectRef,
    pop_level_35: ObjectRef,
    gpop_active: ObjectRef,
    gpop_adaptive: ObjectRef,
}
