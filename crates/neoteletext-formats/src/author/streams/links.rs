//! Magazines 2 and 3: X/27/4 links take precedence over the table.
//! Magazine 5: the Level 3.5 links.

use neoteletext_core::packet::{MotPageEntry, ObjectKind};
use neoteletext_core::types::PageNumber;

use crate::author::{
    AuthorError, DisplayPage, LinkFunction, Mot, MotLevel, ObjectPage, Result, Stream, VALID_BOTH, VALID_LEVEL_25,
    VALID_LEVEL_35, address, link, object_link,
};

use super::{BOTH, SUBPAGE_0, magazine, page, text_body};

pub(super) fn links_over_the_table(stream: &mut Stream) -> Result<()> {
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

pub(super) fn level_35_links(stream: &mut Stream) -> Result<()> {
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
