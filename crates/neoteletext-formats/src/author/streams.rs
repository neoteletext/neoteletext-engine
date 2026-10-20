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

use neoteletext_core::consts::{
    ALPHA_BLACK, ALPHA_RED, ALPHA_WHITE, ALPHA_YELLOW, BLACK_BACKGROUND, COLOUR_BLUE_BIT, COLOUR_GREEN_BIT, CONCEAL,
    DOUBLE_HEIGHT, END_BOX, FLASH, MODE_DRCS_CHARACTER, MODE_FONT_STYLE, MODE_TERMINATION, MOSAIC_WHITE,
    NEW_BACKGROUND, ROW_ADDRESS_LAST, ROW_ADDRESS_RESERVED_DEFAULT, START_BOX,
};
use neoteletext_core::packet::{
    ColourData, DefaultObject, DisplayAttributes, MotPageEntry, MotTotals, ObjectKind, ObjectSource, ObjectUsage,
    Triplet, TripletFunction,
};
use neoteletext_core::types::{
    ColourWords, FallbackFields, Magazine, PageAddress, PageNumber, Rgb12, SidePanelFields, SubCode,
};

use super::{
    AuthorError, DisplayPage, FlashMode, FlashRate, LinkFunction, MagazinePackets, Mot, MotLevel, ObjectPage,
    ObjectRef, Result, RowScope, Stream, Triplets, VALID_BOTH, VALID_LEVEL_25, VALID_LEVEL_35, address, colour, link,
    neutral_fields, neutral_fields_level_35, object_link, public_number, row_address,
};

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

// ---------------------------------------------------------------------------
// Magazine 1: the features one by one
// ---------------------------------------------------------------------------

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

fn magazine_1(stream: &mut Stream) -> Result<()> {
    let objects = magazine_1_objects()?;
    stream.extend(objects.pop.build()?).extend(objects.gpop.build()?);
    stream.push(magazine_1_table(&objects)?.build()?);
    stream.push(index()?.build()?);
    stream.push(colour_map()?.build()?);
    stream.push(colour_map_level_35()?.build()?);
    stream.push(screen_and_row_colours()?.build()?);
    let panels = [
        (0x104, true, false, 0, true),
        (0x105, false, true, 4, true),
        (0x106, true, true, 6, true),
        (0x107, true, true, 0, true),
        (0x108, true, false, 0, false),
    ];
    for (mpp, left, right, columns, status) in panels {
        stream.push(side_panels(mpp, left, right, columns, status)?.build()?);
    }
    stream.push(sizes_and_attributes()?.build()?);
    stream.push(flash()?.build()?);
    stream.push(g3_and_g1()?.build()?);
    stream.push(g2_and_diacritics()?.build()?);
    stream.push(every_mode()?.build()?);
    stream.push(sizes_written_by_enhancement_data()?.build()?);
    stream.push(clause_13_9()?.build()?);
    stream.push(annex_c2()?.build()?);
    stream.push(local_objects()?.build()?);
    stream.push(public_objects(&objects)?.build()?);
    stream.push(global_objects(&objects)?.build()?);
    Ok(())
}

/// POP 1A0 (two sub-pages) and GPOP 1E0. Not 1F0: TOP receivers take
/// page 1F0 to be the Basic TOP Table whatever it carries.
fn magazine_1_objects() -> Result<Objects> {
    let mut pop = ObjectPage::new(address(0x1A0)?, false);
    let mut active = Triplets::new();
    active.foreground(0, CYAN).text(0, b"ACTIVE POP").background(12, colour(1, 4));
    let pop_active = pop.define(0, ObjectKind::Active, BOTH, active)?;
    let mut adaptive = Triplets::new();
    adaptive.foreground(0, YELLOW).text(0, b"ADAPTIVE");
    let pop_adaptive = pop.define(0, ObjectKind::Adaptive, BOTH, adaptive)?;
    let mut passive = Triplets::new();
    passive.background(0, colour(2, 4)).text(0, b"PASSIVE");
    passive.position(1, 0)?.g3(0, 0x2C).g3(1, 0x2D);
    let pop_passive = pop.define(0, ObjectKind::Passive, BOTH, passive)?;
    // Sub-page 1: an object for Level 3.5 only.
    let mut level_35 = Triplets::new();
    level_35.text(0, b"L3.5 ONLY");
    let pop_level_35 = pop.define(1, ObjectKind::Active, LEVEL_35, level_35)?;

    let mut gpop = ObjectPage::new(address(0x1E0)?, true);
    let mut global = Triplets::new();
    global.foreground(0, MAGENTA).text(0, b"GLOBAL ACTIVE");
    let gpop_active = gpop.define(0, ObjectKind::Active, BOTH, global)?;
    // An Adaptive object that invokes a Passive one (clause 13.4 item 4).
    let inner = gpop.define(0, ObjectKind::Passive, BOTH, text_body(0, b"INNER PASSIVE"))?;
    let mut outer = Triplets::new();
    outer.foreground(0, GREEN).text(0, b"OUTER ADAPTIVE");
    outer.position(1, 0)?.invoke(inner);
    let gpop_adaptive = gpop.define(0, ObjectKind::Adaptive, BOTH, outer)?;
    Ok(Objects { pop, gpop, pop_active, pop_adaptive, pop_passive, pop_level_35, gpop_active, gpop_adaptive })
}

/// MOT 1FE, with the same links at both levels.
fn magazine_1_table(objects: &Objects) -> Result<Mot> {
    let mut mot = Mot::new(magazine(1)?);
    mot.object_link(MotLevel::Both, 0, object_link(objects.gpop.address(), 0))?;
    mot.object_link(MotLevel::Both, 1, object_link(objects.pop.address(), 1))?;
    mot.page(PageNumber(0x22), MotPageEntry { global_objects: false, pop_link: 1 });
    mot.page(PageNumber(0x23), MotPageEntry { global_objects: true, pop_link: 0 });
    let totals = MotTotals { magazines: 1 << magazine(1)?.index(), object_pages: 3 };
    mot.totals(MotLevel::Both, totals);
    Ok(mot)
}

/// 100: the index.
fn index() -> Result<DisplayPage> {
    let mut page = page(0x100, b"LEVEL 2.5 AND 3.5 AUTHORED PAGES")?;
    let lines: [&[u8]; 19] = [
        b"101 COLOUR MAP     102 CLUTS 0 AND 1",
        b"103 SCREEN, ROWS   104-108 SIDE PANELS",
        b"109 SIZES, ATTRS   110 FLASH",
        b"112 G3 AND G1 SETS",
        b"113 EVERY MODE     117 CLAUSE 13.9",
        b"118 G2, DIACRITICS 120 ANNEX C.2",
        b"121 LOCAL OBJECTS  122 POP OBJECTS",
        b"123 GPOP OBJECTS   124 SIZES BY X/26",
        b"",
        b"200-202 X/27/4 LINKS OVER THE MOT",
        b"400-402 DEFAULT OBJECTS",
        b"500-501 LEVEL 3.5 LINKS",
        b"700-702 MAGAZINE PACKETS M/29",
        b"",
        b"EVERY PAGE IS BUILT BY FUNCTION",
        b"WITH THE AUTHORING BUILDER OF",
        b"REQ-INP-025. NO PAGE OF THIS",
        b"STREAM IS STORED: IT IS MADE",
        b"BY CARGO XTASK LEVEL25-STREAM.",
    ];
    for (row, line) in (3u8..).zip(lines) {
        page.text(row, 1, line)?;
    }
    Ok(page)
}

/// 101: CLUTs 2 and 3 redefined by X/28/0 (Table 4).
fn colour_map() -> Result<DisplayPage> {
    let mut page = page(0x101, b"COLOUR MAP: X/28/0 CLUTS 2 AND 3")?;
    let mut fields = neutral_fields(0);
    fields.colour_words = Some(words([
        0xF00, 0xE10, 0xD20, 0xC30, 0xB40, 0xA50, 0x960, 0x870, //
        0x111, 0x333, 0x555, 0x777, 0x999, 0xBBB, 0xDDD, 0xFFF,
    ]));
    page.page_data(fields);
    swatches(&mut page, 4)?;
    page.text(14, 1, b"CLUT 1 ENTRY 0 IS TRANSPARENT")?;
    Ok(page)
}

/// 102: CLUTs 0 and 1 redefined by X/28/4, which beats X/28/0 for those
/// CLUTs only (decision E.44); the word for CLUT 1 entry 0 is ignored.
fn colour_map_level_35() -> Result<DisplayPage> {
    let mut page = page(0x102, b"COLOUR MAP: X/28/4 CLUTS 0 AND 1")?;
    let mut level_35 = neutral_fields_level_35(0);
    level_35.colour_words = Some(words([
        0x000, 0xF80, 0x8F0, 0xFF8, 0x08F, 0xF8F, 0x8FF, 0xEEE, //
        0xFFF, 0x800, 0x080, 0x880, 0x008, 0x808, 0x088, 0x444,
    ]));
    if let Some(colours) = level_35.colours.as_mut() {
        colours.screen = colour(1, 1);
    }
    page.level_35_data(level_35);
    let mut fields = neutral_fields(0);
    if let Some(colours) = fields.colours.as_mut() {
        colours.screen = colour(0, 0);
    }
    page.page_data(fields);
    swatches(&mut page, 4)?;
    page.text(14, 1, b"SCREEN: X/28/0 WINS OVER X/28/4")?;
    Ok(page)
}

/// 103: Full Screen Colour, Address Display Row 0, Full Row Colour for one
/// row and down to the bottom, the default row colour, Black Background
/// Colour Substitution and Colour Table Re-mapping (Table 4, decision E.45).
fn screen_and_row_colours() -> Result<DisplayPage> {
    let mut page = page(0x103, b"SCREEN AND ROW COLOURS, RE-MAPPING")?;
    let mut fields = neutral_fields(0);
    fields.colours = Some(neoteletext_core::types::DefaultColourFields {
        screen: colour(3, 0),
        row: colour(1, 4),
        black_background_substitution: true,
        remapping: 5,
    });
    page.page_data(fields);
    // Annex A.4: Full Screen Colour, then Address Display Row 0, then the
    // rows in order.
    page.local().full_screen_colour(colour(1, 1)).row_0(colour(2, 2), RowScope::ThisRow);
    page.local().character(30, b'R');
    page.local().full_row_colour(5, colour(2, 4), RowScope::ThisRow)?;
    page.local().full_row_colour(10, colour(2, 6), RowScope::DownToBottom)?;
    page.local().full_row_colour(15, colour(3, 4), RowScope::ThisRow)?;
    page.local().full_row_colour(20, colour(1, 2), RowScope::Reserved(0b01))?;
    page.text(3, 1, &[&[ALPHA_RED][..], b"RED", &[ALPHA_GREEN], b"GREEN", &[ALPHA_BLUE], b"BLUE"].concat())?;
    page.text(5, 1, b"ROW 5: FULL ROW COLOUR, THIS ROW")?;
    page.text(7, 1, &[&[BLACK_BACKGROUND][..], b"BLACK BACKGROUND IS SUBSTITUTED"].concat())?;
    page.text(8, 1, &[&[ALPHA_BLACK, NEW_BACKGROUND, ALPHA_WHITE][..], b"ALPHA BLACK, NEW: NOT SUBSTITUTED"].concat())?;
    page.text(10, 1, b"ROW 10: DOWN TO THE BOTTOM")?;
    page.text(15, 1, b"ROW 15: THIS ROW ONLY")?;
    page.text(20, 1, b"ROW 20: S1 S0 = 01 SETS NO COLOUR")?;
    Ok(page)
}

/// 104 to 108: side panels (decision E.49), with text placed in both
/// panel areas by a Local Passive Object and Origin Modifiers; writes to
/// a panel that is not enabled are discarded.
fn side_panels(mpp: u16, left: bool, right: bool, columns: u8, status: bool) -> Result<DisplayPage> {
    let mut page = page(mpp, b"SIDE PANELS")?;
    let mut fields = neutral_fields(0);
    fields.side_panels = Some(SidePanelFields { left, right, status_flag: status, left_columns: columns });
    page.page_data(fields);
    let title = [
        b'L',
        b'E',
        b'F',
        b'T',
        b' ',
        if left { b'Y' } else { b'N' },
        b' ',
        b'R',
        b'I',
        b'G',
        b'H',
        b'T',
        b' ',
        if right { b'Y' } else { b'N' },
        b' ',
        b'N',
        b'=',
        b'0'.saturating_add(columns / 10),
        b'0'.saturating_add(columns % 10),
        b' ',
        b'S',
        b'T',
        b'A',
        b'T',
        b'U',
        b'S',
        b' ',
        b'0'.saturating_add(u8::from(status)),
    ];
    page.text(2, 1, &title)?;
    let mut body = Triplets::new();
    body.background(0, colour(2, 3)).text(0, b"PANEL 0123456789");
    let panel = page.local_object(ObjectKind::Passive, BOTH, body);
    for row in [3u8, 12, 23] {
        page.text(row, 0, b"MAIN PAGE COLUMNS 0 TO 39 ..............")?;
        page.local().position(row, 0)?;
        page.local().origin(0, RIGHT_PANEL_COLUMN).invoke_local(panel);
        page.local().origin(0, LEFT_PANEL_COLUMN).invoke_local(panel);
    }
    // An object element that would cross from the page into a panel is
    // discarded (decision E.49).
    page.local().position(20, 0)?.origin(0, 36).invoke_local(panel);
    Ok(page)
}

/// 109: double height, width and size, underline, invert, conceal and
/// boxing as Display Attributes (Table 29, mode 01100; Annex C.3).
fn sizes_and_attributes() -> Result<DisplayPage> {
    let mut page = page(0x109, b"SIZES AND DISPLAY ATTRIBUTES")?;
    let none = DisplayAttributes::default();
    let cases: [(u8, &[u8], DisplayAttributes); 8] = [
        (3, b"DOUBLE HEIGHT", DisplayAttributes { double_height: true, ..none }),
        (6, b"DOUBLE WIDTH", DisplayAttributes { double_width: true, ..none }),
        (9, b"DOUBLE SIZE", DisplayAttributes { double_height: true, double_width: true, ..none }),
        (12, b"UNDERLINE", DisplayAttributes { underline: true, ..none }),
        (14, b"INVERT", DisplayAttributes { invert: true, ..none }),
        (16, b"CONCEAL", DisplayAttributes { conceal: true, ..none }),
        (18, b"BOXING IS A WINDOW", DisplayAttributes { boxing: true, ..none }),
        (20, b"UNDERLINE AND INVERT", DisplayAttributes { underline: true, invert: true, ..none }),
    ];
    for (row, text, attributes) in cases {
        page.text(row, 4, text)?;
        page.text(row, 26, b"PLAIN")?;
        page.local().position(row, 0)?.attributes(4, attributes);
        // Decision E.47: a zero bit cancels the attribute from its cell.
        page.local().attributes(26, none);
    }
    page.text(
        22,
        0,
        &[&[DOUBLE_HEIGHT][..], b"LEVEL 1 DOUBLE HEIGHT, ", &[START_BOX, END_BOX, CONCEAL], b"HIDDEN"].concat(),
    )?;
    Ok(page)
}

/// 110: Additional Flash Functions, every mode at every rate and phase
/// (Table 29, mode 00111; decision E.48), beside Level 1 Flash.
fn flash() -> Result<DisplayPage> {
    let mut page = page(0x110, b"FLASH: MODES, RATES AND PHASES")?;
    page.text(2, 0, b"MODE SLOW  F1    F2    F3    INC   DEC")?;
    for (row, mode) in (4u8..).step_by(3).zip(FlashMode::ALL) {
        let label: &[u8] = match mode {
            FlashMode::Steady => b"STD",
            FlashMode::Normal => b"NOR",
            FlashMode::Inverted => b"INV",
            FlashMode::AdjacentClut => b"ADJ",
        };
        page.text(row, 0, label)?;
        page.local().position(row, 0)?;
        // An adjacent-CLUT flash shows index XOR 8 in its off phase.
        page.local().foreground(4, colour(2, 1));
        for (block, rate) in (0u8..).zip(FlashRate::ALL) {
            let column = 5u8.saturating_add(block.saturating_mul(6));
            page.text(row, column, b"FLASH")?;
            page.local().flash(column, mode, rate);
        }
    }
    page.text(17, 0, b"RESERVED RATE 110:  FLASH")?;
    page.local().position(17, 0)?.flash(20, FlashMode::Normal, FlashRate::Reserved(0b110));
    page.text(19, 0, &[&[FLASH][..], b"LEVEL 1 FLASH (0/8), SLOW AT LEVEL 2.5"].concat())?;
    page.text(21, 0, b"FAST INCREMENTAL OVER A LONG RUN OF TEXT")?;
    page.local().position(21, 0)?.flash(5, FlashMode::Normal, FlashRate::Incremental);
    page.text(23, 0, b"FAST DECREMENTAL OVER A LONG RUN OF TEXT")?;
    page.local().position(23, 0)?.flash(5, FlashMode::Normal, FlashRate::Decremental);
    Ok(page)
}

/// 112: the G3 set by both modes, and G1 mosaics written by X/26
/// (Table 29, modes 01011, 00010 and 00001).
fn g3_and_g1() -> Result<DisplayPage> {
    let mut page = page(0x112, b"G3 SET, BOTH MODES; G1 MOSAICS")?;
    for (row, codes) in (2u8..).zip((0x20u8..0x80).collect::<Vec<u8>>().chunks(16)) {
        page.text(row, 0, b"G3")?;
        page.local().position(row, 0)?;
        for (column, &code) in (4u8..).zip(codes) {
            page.local().g3(column, code);
        }
    }
    page.text(9, 0, b"G3 L1.5")?;
    page.local().position(9, 0)?;
    for (column, code) in (8u8..).zip(0x20u8..0x30) {
        page.local().g3_level_15(column, code);
    }
    page.text(12, 0, &[MOSAIC_WHITE, b'G', b'1'])?;
    page.local().position(12, 0)?;
    for (column, code) in (4u8..).zip((0x20u8..0x40).chain(0x60..0x68)) {
        page.local().g1(column, code);
    }
    Ok(page)
}

/// 118: the G2 set, every diacritical mark, and a Modified G0 and G2
/// Designation (Table 29, modes 01111, 10000 to 11111, 01000 and 01001).
fn g2_and_diacritics() -> Result<DisplayPage> {
    let mut page = page(0x118, b"G2 SET, DIACRITICS, DESIGNATION")?;
    for (row, codes) in (2u8..).zip((0x20u8..0x80).collect::<Vec<u8>>().chunks(16)) {
        page.text(row, 0, b"G2")?;
        page.local().position(row, 0)?;
        for (column, &code) in (4u8..).zip(codes) {
            page.local().g2(column, code);
        }
    }
    for (row, base) in [(10u8, b'a'), (11, b'O')] {
        page.text(row, 0, b"MARK")?;
        page.local().position(row, 0)?;
        for (column, mark) in (5u8..).zip(0u8..16) {
            page.local().diacritic(column, base, mark);
        }
    }
    page.text(14, 0, b"DESIG")?;
    page.local().position(14, 0)?.designation(6, GERMAN);
    for (column, &code) in (6u8..).zip(b"#$@[\\]^_`{|}~") {
        page.local().g0(column, code);
    }
    Ok(page)
}

/// 113: every row mode of Table 28 and every column mode of Table 29,
/// with reserved values of their data (REQ-TST-015).
fn every_mode() -> Result<DisplayPage> {
    let mut page = page(0x113, b"EVERY TRIPLET MODE")?;
    let active = page.local_object(ObjectKind::Active, BOTH, text_body(0, b"ACT"));
    let adaptive = page.local_object(ObjectKind::Adaptive, BOTH, text_body(0, b"ADP"));
    let passive = page.local_object(ObjectKind::Passive, BOTH, text_body(0, b"PAS"));
    let local = page.local();
    local.full_screen_colour(colour(1, 0)).row_0(colour(3, 0), RowScope::ThisRow);
    local.character(30, b'0');
    local.full_row_colour(2, colour(1, 7), RowScope::ThisRow)?;
    // Row 2: column modes 00000 to 01111.
    local.foreground(0, RED).g1(1, 0x7F).g3_level_15(2, 0x30).background(3, BLUE);
    for (column, mode) in [(4u8, 0b00100u8), (5, 0b00101)] {
        local.raw(Triplet { address: column, mode, data: 0 });
    }
    local.pdc_column(6, 0x15);
    local.flash(7, FlashMode::Normal, FlashRate::Slow).designation(8, GERMAN).g0(9, b'@');
    local.raw(Triplet { address: 10, mode: 0b01010, data: 0 });
    local.g3(11, 0x41).attributes(12, DisplayAttributes { underline: true, ..DisplayAttributes::default() });
    // DRCS Mode and DRCS Character Invocation, sent as a service would
    // and not acted on (decision E.95): the second moves the column only.
    local.raw(Triplet { address: ROW_ADDRESS_RESERVED_DEFAULT, mode: DRCS_MODE, data: DRCS_MODE_DATA });
    local.raw(Triplet { address: 13, mode: MODE_DRCS_CHARACTER, data: DRCS_NORMAL });
    // Font Style, bold, sent as a service would and not acted on (decision
    // E.96): it moves the column only.
    local.raw(Triplet { address: 14, mode: MODE_FONT_STYLE, data: FONT_STYLE_BOLD });
    local.g2(15, 0x41);
    // Column modes 10000 to 11111: G0 with each diacritical mark.
    for (column, mark) in (16u8..).zip(0u8..16) {
        local.diacritic(column, b'e', mark);
    }
    // Row 3: reserved data values.
    local.position(3, 0)?;
    local.function(TripletFunction::ForegroundColour { column: 1, colour: ColourData { index: GREEN, high: 0b01 } });
    local.function(TripletFunction::AdditionalFlash { column: 2, mode: 1, rate: 0b111, reserved: 0b11 });
    local.raw(Triplet { address: 3, mode: MODE_DRCS_CHARACTER, data: DRCS_NORMAL | DRCS_INVALID_CHARACTER });
    local.position_address(row_address(4)?, 45);
    // Reserved row modes and Programme Delivery Control, which do not move
    // the Active Position (decision E.46).
    for mode in [0b00010u8, 0b00011, 0b00101, 0b00110, 0b01110, 0b01111, 0b10100] {
        local.raw(Triplet { address: row_address(4)?, mode, data: 0 });
    }
    for mode in 0b01000u8..=0b01101 {
        local.pdc_row(mode, row_address(4)?, 0x11);
    }
    for mode in 0b11001u8..=0b11110 {
        local.raw(Triplet { address: row_address(4)?, mode, data: 0 });
    }
    // Rows 5 to 7: an Origin Modifier and the three invocations, whose
    // definitions use row modes 10101 to 10111.
    local.position(5, 0)?.origin(0, 4).invoke_local(active);
    local.position(6, 0)?.origin(0, 8).invoke_local(adaptive);
    local.position(7, 0)?.origin(0, 12).invoke_local(passive);
    // Mode 11111 with an address other than 63 is reserved (Table 28). It
    // comes last, so that a decoder that ends the data on it loses nothing.
    local.raw(Triplet { address: ROW_ADDRESS_LAST - 1, mode: MODE_TERMINATION, data: 0 });
    page.text(9, 0, b"ROW 0 COLUMN 30: ADDRESS DISPLAY ROW 0")?;
    Ok(page)
}

/// 124: double height, width and size on characters that the enhancement
/// data writes itself, each size ended after its text, with the Level 1
/// rows left empty: the sizes that a decoder which sizes only enhancement
/// characters (zvbi) shows too (REQ-TST-023, Annex C.3).
fn sizes_written_by_enhancement_data() -> Result<DisplayPage> {
    let mut page = page(0x124, b"SIZES OF CHARACTERS WRITTEN BY X/26")?;
    let none = DisplayAttributes::default();
    let sizes = [
        (3u8, DisplayAttributes { double_height: true, ..none }, &b"HEIGHT"[..]),
        (6, DisplayAttributes { double_width: true, ..none }, b"WIDTH"),
        (9, DisplayAttributes { double_height: true, double_width: true, ..none }, b"SIZE"),
    ];
    for (row, attributes, text) in sizes {
        // A double-width character covers the next column too, so the
        // characters leave no space between them that the size runs over.
        let pitch = if attributes.double_width { 2 } else { 1 };
        page.local().position(row, 0)?.attributes(4, attributes);
        for (column, &code) in (4u8..).step_by(pitch).zip(text) {
            page.local().character(column, code);
        }
        let end = 4u8.saturating_add(u8::try_from(text.len().saturating_mul(pitch)).unwrap_or(u8::MAX));
        page.local().attributes(end, none);
    }
    Ok(page)
}

/// 117: the object example of clause 13.9, once for each type (Table in
/// clause 13.9 and its notes 1 to 8).
fn clause_13_9() -> Result<DisplayPage> {
    let mut page = page(0x117, b"CLAUSE 13.9 OBJECT EXAMPLE")?;
    let mut body = Triplets::new();
    body.text(5, b"LAZY")
        .foreground(10, PINK)
        .text(10, b"COW")
        .text(18, b"FOX")
        .foreground(22, YELLOW)
        .text(22, b"SAT");
    let mut row = [b' '; 28];
    for (column, &code) in row.iter_mut().zip(b" THE FAST DOG AND CAT RAN IN") {
        *column = code;
    }
    let attributes = [(0usize, ALPHA_BLUE), (17, ALPHA_RED), (21, ALPHA_GREEN)];
    for (column, code) in attributes {
        if let Some(slot) = row.get_mut(column) {
            *slot = code;
        }
    }
    for (line, kind, label) in [
        (4u8, ObjectKind::Active, &b"ACTIVE"[..]),
        (8, ObjectKind::Adaptive, b"ADAPTIVE"),
        (12, ObjectKind::Passive, b"PASSIVE"),
    ] {
        let object = page.local_object(kind, BOTH, body.clone());
        page.text(line.saturating_sub(1), 0, label)?;
        page.text(line, 0, &row)?;
        page.local().position(line, 0)?.invoke_local(object);
    }
    Ok(page)
}

/// 120: the three contention examples of Annex C.2: red by rule 1, blue by
/// rule 2, yellow by rule 3. The third straddles X/26/0 and X/26/1.
fn annex_c2() -> Result<DisplayPage> {
    let mut page = page(0x120, b"ANNEX C.2 ATTRIBUTE CONTENTION")?;
    for (row, expected) in [(4u8, &b"RULE 1: RED"[..]), (8, b"RULE 2: BLUE"), (12, b"RULE 3: YELLOW")] {
        page.text(row, 0, b"CASE")?;
        page.text(row, 9, &[ALPHA_RED, b'A'])?;
        page.text(row, 14, expected)?;
    }
    let local = page.local();
    local.position(4, 0)?.foreground(9, BLUE);
    local.position(8, 0)?.foreground(10, BLUE);
    local.position(12, 0)?;
    local.pad_to(12, filler_position(12, 10)?);
    local.foreground(10, BLUE).foreground(10, YELLOW);
    Ok(page)
}

/// 121: Local Objects: the character priority of clause 13.2, nesting of
/// the three types, and invocations that a decoder skips (decision E.51).
fn local_objects() -> Result<DisplayPage> {
    let mut page = page(0x121, b"LOCAL OBJECTS: PRIORITY, NESTING")?;
    let passive = page.local_object(ObjectKind::Passive, BOTH, text_body(10, b"P"));
    let adaptive = page.local_object(ObjectKind::Adaptive, BOTH, text_body(10, b"D"));
    let active = page.local_object(ObjectKind::Active, BOTH, text_body(10, b"A"));
    let inner = page.local_object(ObjectKind::Passive, BOTH, text_body(0, b"PASSIVE IN ADAPTIVE"));
    let mut middle_body = text_body(0, b"ADAPTIVE IN ACTIVE");
    middle_body.position(1, 0)?.invoke_local(inner);
    let middle = page.local_object(ObjectKind::Adaptive, BOTH, middle_body);
    let mut outer_body = text_body(0, b"ACTIVE");
    outer_body.position(1, 0)?.invoke_local(middle);
    let outer = page.local_object(ObjectKind::Active, BOTH, outer_body);
    let mut forbidden_body = text_body(0, b"PASSIVE");
    forbidden_body.invoke_local(adaptive);
    let forbidden = page.local_object(ObjectKind::Passive, BOTH, forbidden_body);
    let level_35 = page.local_object(ObjectKind::Active, LEVEL_35, text_body(0, b"LEVEL 3.5 ONLY"));
    for (row, label) in [
        (3u8, &b"L WINS"[..]),
        (5, b"A WINS"),
        (7, b"D WINS"),
        (11, b"NESTED"),
        (16, b"P CANNOT INVOKE"),
        (18, b"WRONG TYPE"),
        (20, b"L3.5 ONLY"),
    ] {
        page.text(row, 0, label)?;
    }
    let local = page.local();
    local.position(3, 0)?.invoke_local(passive).invoke_local(adaptive).invoke_local(active).character(10, b'L');
    local.position(5, 0)?.invoke_local(passive).invoke_local(adaptive).invoke_local(active);
    local.position(7, 0)?.invoke_local(passive).invoke_local(adaptive);
    local.position(12, 0)?.invoke_local(outer);
    local.position(16, 0)?.origin(0, 20).invoke_local(forbidden);
    local.position(18, 0)?.origin(0, 20).invoke_local_as(passive, ObjectKind::Active);
    local.position(20, 0)?.origin(0, 20).invoke_local(level_35);
    Ok(page)
}

/// 122: POP objects of each type through the table's POP link, with
/// Origin Modifiers, a second sub-page, a Level 3.5 object, an unused
/// pointer and an object that runs off the page.
fn public_objects(objects: &Objects) -> Result<DisplayPage> {
    let mut page = page(0x122, b"POP OBJECTS VIA THE MOT")?;
    for (row, label) in
        [(3u8, &b"ACTIVE"[..]), (5, b"ADAPTIVE"), (7, b"PASSIVE"), (12, b"L3.5"), (14, b"UNUSED"), (23, b"EDGE")]
    {
        page.text(row, 0, label)?;
    }
    let unused = ObjectRef { source: ObjectSource::Public, kind: ObjectKind::Active, number: public_number(0, 5) };
    let local = page.local();
    local.position(3, 0)?.origin(0, 10).invoke(objects.pop_active);
    local.position(5, 0)?.origin(0, 10).invoke(objects.pop_adaptive);
    local.position(7, 0)?.origin(0, 10).invoke(objects.pop_passive);
    local.position(12, 0)?.origin(0, 10).invoke(objects.pop_level_35);
    local.position(14, 0)?.origin(0, 10).invoke(unused);
    local.position(23, 0)?.origin(0, 35).invoke(objects.pop_active);
    Ok(page)
}

/// 123: GPOP objects through the table's GPOP link, an Adaptive object
/// that invokes a Passive one.
fn global_objects(objects: &Objects) -> Result<DisplayPage> {
    let mut page = page(0x123, b"GPOP OBJECTS VIA THE MOT")?;
    page.text(3, 0, b"ACTIVE")?;
    page.text(6, 0, b"NESTED")?;
    let local = page.local();
    local.position(3, 0)?.origin(0, 10).invoke(objects.gpop_active);
    local.position(6, 0)?.origin(0, 10).invoke(objects.gpop_adaptive);
    Ok(page)
}

// ---------------------------------------------------------------------------
// Magazines 2 and 3: X/27/4 links take precedence over the table
// ---------------------------------------------------------------------------

fn links_over_the_table(stream: &mut Stream) -> Result<()> {
    let mut wrong = ObjectPage::new(address(0x2A9)?, false);
    let wrong_ref = wrong.define(0, ObjectKind::Active, BOTH, text_body(0, b"POP OF THE MOT"))?;
    let mut right = ObjectPage::new(address(0x3A0)?, false);
    let right_ref = right.define(0, ObjectKind::Active, BOTH, text_body(0, b"POP OF X/27/4, MAGAZINE 3"))?;
    let mut gpop = ObjectPage::new(address(0x2F0)?, true);
    let global_ref = gpop.define(0, ObjectKind::Passive, BOTH, text_body(0, b"GPOP OF X/27/4"))?;
    // The same number names an object on whichever POP the page links.
    if wrong_ref.number != right_ref.number {
        return Err(AuthorError::BadPosition);
    }
    let mut mot = Mot::new(magazine(2)?);
    mot.object_link(MotLevel::Both, 1, object_link(wrong.address(), 0))?;
    for page in [0x00u8, 0x01, 0x02] {
        mot.page(PageNumber(page), MotPageEntry { global_objects: false, pop_link: 1 });
    }
    stream.extend(wrong.build()?).extend(right.build()?).extend(gpop.build()?).push(mot.build()?);

    let invoke = |page: &mut DisplayPage| -> Result<()> {
        page.text(3, 0, b"POP")?;
        page.text(5, 0, b"GPOP")?;
        page.local().position(3, 0)?.origin(0, 10).invoke(right_ref);
        page.local().position(5, 0)?.origin(0, 10).invoke(global_ref);
        Ok(())
    };
    let mut over = page(0x200, b"X/27/4 LINKS BEAT THE MOT")?;
    over.link_level_25(LinkFunction::Gpop, gpop.address(), SUBPAGE_0)?;
    over.link_level_25(LinkFunction::Pop, right.address(), SUBPAGE_0)?;
    invoke(&mut over)?;
    stream.push(over.build()?);
    // Decision E.52: a link whose function bits do not match its position,
    // or whose Format 1 marker is wrong, is ignored.
    let mut mismatched = page(0x201, b"X/27/4 POP LINK, GPOP FUNCTION")?;
    mismatched.compositional_link(4, 1, link(LinkFunction::Gpop, VALID_BOTH, right.address(), SUBPAGE_0))?;
    invoke(&mut mismatched)?;
    stream.push(mismatched.build()?);
    let mut unmarked = page(0x202, b"X/27/4 LINK NOT FORMAT 1")?;
    let mut bad = link(LinkFunction::Pop, VALID_BOTH, right.address(), SUBPAGE_0);
    bad.marker_first = false;
    unmarked.compositional_link(4, 1, bad)?;
    invoke(&mut unmarked)?;
    stream.push(unmarked.build()?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Magazine 4: default objects
// ---------------------------------------------------------------------------

/// The 8-bit number a table's default object carries (clause 10.6.4.1).
fn default_object(object: ObjectRef) -> DefaultObject {
    // N8 is 0: only pointer packets 1 and 2 can be reached this way.
    DefaultObject { kind: object.kind, number: object.number.0 as u8 }
}

fn default_objects(stream: &mut Stream) -> Result<()> {
    // Default objects have their origin at row 0, column 0 (clause 13.8).
    let at = |row: u8, text: &[u8]| -> Result<Triplets> {
        let mut body = Triplets::new();
        body.position(row, 0)?.text(20, text);
        Ok(body)
    };
    let mut gpop = ObjectPage::new(address(0x4F0)?, true);
    let gpop_1 = gpop.define(0, ObjectKind::Passive, BOTH, at(3, b"GPOP DEFAULT 1")?)?;
    let gpop_2 = gpop.define(0, ObjectKind::Active, BOTH, at(4, b"GPOP DEFAULT 2")?)?;
    let mut pop = ObjectPage::new(address(0x4A0)?, false);
    let pop_1 = pop.define(0, ObjectKind::Adaptive, BOTH, at(5, b"POP DEFAULT 1")?)?;
    let pop_2 = pop.define(0, ObjectKind::Passive, BOTH, at(6, b"POP DEFAULT 2")?)?;
    let mut mot = Mot::new(magazine(4)?);
    let mut global_link = object_link(gpop.address(), 0);
    global_link.default_objects = [Some(default_object(gpop_1)), Some(default_object(gpop_2))];
    let mut pop_link = object_link(pop.address(), 0);
    pop_link.default_objects = [Some(default_object(pop_1)), Some(default_object(pop_2))];
    // The fallback of a POP link applies to pages with no X/28 or M/29
    // data: both side panels, eight columns each, and substitution.
    pop_link.fallback = FallbackFields {
        side_panels: SidePanelFields { left: true, right: true, status_flag: true, left_columns: 8 },
        black_background_substitution: true,
    };
    mot.object_link(MotLevel::Both, 0, global_link)?;
    mot.object_link(MotLevel::Both, 1, pop_link)?;
    for page in [0x00u8, 0x01, 0x02] {
        mot.page(PageNumber(page), MotPageEntry { global_objects: true, pop_link: 1 });
    }
    stream.extend(gpop.build()?).extend(pop.build()?).push(mot.build()?);

    stream.push(page(0x400, b"DEFAULT OBJECTS: NO X/26")?.build()?);
    let mut suppressed = page(0x401, b"X/26 DISPLAY DATA: NO DEFAULTS")?;
    suppressed.text(10, 0, b"THE LOCAL DATA WRITES ONE CHARACTER:")?;
    suppressed.local().position(10, 0)?.character(37, b'*');
    stream.push(suppressed.build()?);
    // Decision E.53: X/26 with Programme Delivery Control only keeps them.
    let mut pdc = page(0x402, b"X/26 PDC ONLY: DEFAULTS KEPT")?;
    pdc.local().pdc_row(0b01000, row_address(1)?, 0x01).pdc_column(5, 0x02);
    stream.push(pdc.build()?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Magazine 5: Level 3.5 links
// ---------------------------------------------------------------------------

fn level_35_links(stream: &mut Stream) -> Result<()> {
    let mut level_25 = ObjectPage::new(address(0x5A0)?, false);
    let object = level_25.define(0, ObjectKind::Active, BOTH, text_body(0, b"POP OF THE LEVEL 2.5 LINK"))?;
    let mut level_35 = ObjectPage::new(address(0x5A1)?, false);
    level_35.define(0, ObjectKind::Active, BOTH, text_body(0, b"POP OF THE LEVEL 3.5 LINK"))?;
    let mut x27_5 = ObjectPage::new(address(0x5A2)?, false);
    x27_5.define(0, ObjectKind::Active, BOTH, text_body(0, b"POP OF X/27/5"))?;
    let mut mot = Mot::new(magazine(5)?);
    mot.object_link(MotLevel::Level25, 1, object_link(level_25.address(), 0))?;
    mot.object_link(MotLevel::Level35, 1, object_link(level_35.address(), 0))?;
    let entry = MotPageEntry { global_objects: false, pop_link: 1 };
    mot.page(PageNumber(0x00), entry);
    stream.extend(level_25.build()?).extend(level_35.build()?).extend(x27_5.build()?).push(mot.build()?);

    let mut by_table = page(0x500, b"MOT X/19 OR X/22 BY LEVEL")?;
    by_table.local().position(3, 0)?.invoke(object);
    stream.push(by_table.build()?);
    // Table 16: X/27/4 link 1 for Level 2.5 only; X/27/5 link 0, a POP
    // link by its function bits, for Level 3.5.
    let mut by_links = page(0x501, b"X/27/4 AT 2.5, X/27/5 AT 3.5")?;
    by_links.compositional_link(4, 1, link(LinkFunction::Pop, VALID_LEVEL_25, level_25.address(), SUBPAGE_0))?;
    by_links.compositional_link(5, 0, link(LinkFunction::Pop, VALID_LEVEL_35, x27_5.address(), SUBPAGE_0))?;
    by_links.local().position(3, 0)?.invoke(object);
    stream.push(by_links.build()?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Magazine 7: magazine packets M/29
// ---------------------------------------------------------------------------

fn magazine_packets(stream: &mut Stream) -> Result<()> {
    let mut data = neutral_fields(0);
    data.side_panels = Some(SidePanelFields { left: true, right: true, status_flag: true, left_columns: 8 });
    data.colour_words = Some(words([
        0x0F0, 0x0E1, 0x0D2, 0x0C3, 0x0B4, 0x0A5, 0x096, 0x087, //
        0x00F, 0x11E, 0x22D, 0x33C, 0x44B, 0x55A, 0x669, 0x778,
    ]));
    data.colours = Some(neoteletext_core::types::DefaultColourFields {
        screen: colour(2, 1),
        row: colour(2, 2),
        black_background_substitution: false,
        remapping: 0,
    });
    let mut level_35 = neutral_fields_level_35(0);
    level_35.colour_words = Some(words([
        0x000, 0xA00, 0x0A0, 0xAA0, 0x00A, 0xA0A, 0x0AA, 0xAAA, //
        0x000, 0x500, 0x050, 0x550, 0x005, 0x505, 0x055, 0x555,
    ]));
    let mut packets = MagazinePackets::new(magazine(7)?);
    packets.data(data).level_35(level_35);
    stream.push(packets.build()?);

    let mut plain = page(0x700, b"M/29/0 AND M/29/4 APPLY")?;
    swatches(&mut plain, 4)?;
    stream.push(plain.build()?);
    let mut page_data = page(0x701, b"X/28/0 BEATS M/29/0")?;
    let mut fields = neutral_fields(0);
    if let Some(colours) = fields.colours.as_mut() {
        colours.screen = colour(0, 4);
    }
    page_data.page_data(fields);
    swatches(&mut page_data, 4)?;
    stream.push(page_data.build()?);
    let mut page_35 = page(0x702, b"X/28/4 BEATS M/29/4 FOR CLUTS 0, 1")?;
    let mut fields = neutral_fields_level_35(0);
    fields.colour_words = Some(words([
        0x000, 0xF44, 0x4F4, 0xFF4, 0x44F, 0xF4F, 0x4FF, 0xCCC, //
        0x000, 0x422, 0x242, 0x442, 0x224, 0x424, 0x244, 0x666,
    ]));
    page_35.level_35_data(fields);
    swatches(&mut page_35, 4)?;
    stream.push(page_35.build()?);
    Ok(())
}
