//! Magazine 1: the features one by one. This file holds the magazine's
//! objects and table, the index, the Colour Map pages, screen and row
//! colours and the page that shows side-panel data read past, and pushes
//! every page of the magazine in its order.

use neoteletext_core::consts::{ALPHA_BLACK, ALPHA_RED, ALPHA_WHITE, BLACK_BACKGROUND, NEW_BACKGROUND};
use neoteletext_core::packet::{MotPageEntry, MotTotals, ObjectKind};
use neoteletext_core::types::{PageNumber, SidePanelFields};

use crate::author::{
    DisplayPage, Mot, MotLevel, ObjectPage, Result, RowScope, Stream, Triplets, address, colour, neutral_fields,
    neutral_fields_level_35, object_link,
};

use super::object_examples::{annex_c2, clause_13_9, global_objects, local_objects, public_objects};
use super::presentation::{
    every_mode, flash, g2_and_diacritics, g3_and_g1, sizes_and_attributes, sizes_written_by_enhancement_data,
};
use super::{
    ALPHA_BLUE, ALPHA_GREEN, BOTH, CYAN, GREEN, LEFT_PANEL_COLUMN, LEVEL_35, MAGENTA, Objects, RIGHT_PANEL_COLUMN,
    YELLOW, magazine, page, swatches, text_body, words,
};

pub(super) fn magazine_1(stream: &mut Stream) -> Result<()> {
    let objects = magazine_1_objects()?;
    stream.extend(objects.pop.build()?).extend(objects.gpop.build()?);
    stream.push(magazine_1_table(&objects)?.build()?);
    stream.push(index()?.build()?);
    stream.push(colour_map()?.build()?);
    stream.push(colour_map_level_35()?.build()?);
    stream.push(screen_and_row_colours()?.build()?);
    stream.push(side_panels_read_past()?.build()?);
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
        b"103 SCREEN, ROWS   104 SIDE PANELS",
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

/// 104: the packet fields of both side panels, and objects placed in the
/// columns they would have used; the fields are read past and those
/// elements discarded, so the page shows its 40 columns (REQ-1644).
fn side_panels_read_past() -> Result<DisplayPage> {
    let mut page = page(0x104, b"SIDE PANELS READ PAST")?;
    let mut fields = neutral_fields(0);
    fields.side_panels = Some(SidePanelFields { left: true, right: true, status_flag: true, left_columns: 8 });
    page.page_data(fields);
    let mut body = Triplets::new();
    body.background(0, colour(2, 3)).text(0, b"PANEL 0123456789");
    let panel = page.local_object(ObjectKind::Passive, BOTH, body);
    for row in [3u8, 12, 23] {
        page.text(row, 0, b"MAIN PAGE COLUMNS 0 TO 39 ..............")?;
        page.local().position(row, 0)?;
        page.local().origin(0, RIGHT_PANEL_COLUMN).invoke_local(panel);
        page.local().origin(0, LEFT_PANEL_COLUMN).invoke_local(panel);
    }
    // An object element past column 39 is discarded, and the object
    // continues (REQ-OBJ-014).
    page.local().position(20, 0)?.origin(0, 36).invoke_local(panel);
    Ok(page)
}
