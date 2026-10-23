//! Magazine 4: default objects.

use neoteletext_core::packet::{DefaultObject, MotPageEntry, ObjectKind};
use neoteletext_core::types::{FallbackFields, PageNumber, SidePanelFields};

use crate::author::{
    Mot, MotLevel, ObjectPage, ObjectRef, Result, Stream, Triplets, address, object_link, row_address,
};

use super::{BOTH, magazine, page};

/// The 8-bit number a table's default object carries (clause 10.6.4.1).
fn default_object(object: ObjectRef) -> DefaultObject {
    // N8 is 0: only pointer packets 1 and 2 can be reached this way.
    DefaultObject { kind: object.kind, number: object.number.0 as u8 }
}

pub(super) fn default_objects(stream: &mut Stream) -> Result<()> {
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
