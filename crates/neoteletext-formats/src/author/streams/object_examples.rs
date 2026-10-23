//! Magazine 1's object pages: the examples of clause 13.9 and Annex C.2,
//! and Local, Public and Global objects.

use neoteletext_core::consts::ALPHA_RED;
use neoteletext_core::packet::{ObjectKind, ObjectSource};

use crate::author::{DisplayPage, ObjectRef, Result, Triplets, public_number};

use super::{ALPHA_BLUE, ALPHA_GREEN, BLUE, BOTH, LEVEL_35, Objects, PINK, YELLOW, filler_position, page, text_body};

/// 117: the object example of clause 13.9, once for each type (Table in
/// clause 13.9 and its notes 1 to 8).
pub(super) fn clause_13_9() -> Result<DisplayPage> {
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
pub(super) fn annex_c2() -> Result<DisplayPage> {
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
pub(super) fn local_objects() -> Result<DisplayPage> {
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
pub(super) fn public_objects(objects: &Objects) -> Result<DisplayPage> {
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
pub(super) fn global_objects(objects: &Objects) -> Result<DisplayPage> {
    let mut page = page(0x123, b"GPOP OBJECTS VIA THE MOT")?;
    page.text(3, 0, b"ACTIVE")?;
    page.text(6, 0, b"NESTED")?;
    let local = page.local();
    local.position(3, 0)?.origin(0, 10).invoke(objects.gpop_active);
    local.position(6, 0)?.origin(0, 10).invoke(objects.gpop_adaptive);
    Ok(page)
}
