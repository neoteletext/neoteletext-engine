//! The TTI writer (REQ-INP-014), compiled with the `tti` feature.

use std::io::{self, Write};

use neoteletext_core::consts::{
    ATTRIBUTE_LAST, DATA_START, ESC, NIBBLE_MASK, PACKET_ENHANCEMENT, PACKET_LINKS, SPACE, STORED_ROWS,
    TRIPLETS_PER_PACKET, TTI_ESCAPE_OFFSET,
};
use neoteletext_core::ecc::hamming84;
use neoteletext_core::packet::{Triplet, encode};
use neoteletext_core::types::{Magazine, PageNumber, SubCode};

use crate::consts::{TTI_FL_NULL_LINK, TTI_PACKED_CHARACTER_BASE, TTI_PACKED_CHARACTERS};
use crate::encode::pack_triplets;
use crate::page::{FieldKind, FormField, FormMethod, LinkDestination, PageFile, PageFont, RawBody, Warning};

/// Writes row text in the escape form: control codes as ESC followed by the
/// code plus 0x40, so that the file is plain text (REQ-INP-014). Trailing
/// spaces are left out; the reader restores them.
fn write_row_text(codes: &[u8], out: &mut dyn Write) -> io::Result<()> {
    let end = codes.iter().rposition(|&code| code != SPACE).map_or(0, |index| index.saturating_add(1));
    for &code in codes.get(..end).unwrap_or(&[]) {
        if code <= ATTRIBUTE_LAST {
            out.write_all(&[ESC, code.wrapping_add(TTI_ESCAPE_OFFSET)])?;
        } else {
            out.write_all(&[code])?;
        }
    }
    Ok(())
}

fn write_triplets(triplets: &[Triplet; TRIPLETS_PER_PACKET], out: &mut dyn Write) -> io::Result<()> {
    out.write_all(&pack_triplets(triplets))
}

fn write_nibbles(values: &[u8; TTI_PACKED_CHARACTERS], out: &mut dyn Write) -> io::Result<()> {
    for &value in values {
        out.write_all(&[TTI_PACKED_CHARACTER_BASE | (value & NIBBLE_MASK)])?;
    }
    Ok(())
}

fn flag(value: bool) -> &'static [u8] {
    if value { b"Y" } else { b"" }
}

fn write_field(field: &FormField, page: &PageFile, index: usize, out: &mut dyn Write) -> io::Result<()> {
    out.write_all(b"FD,")?;
    out.write_all(field.kind.keyword())?;
    write!(out, ",")?;
    out.write_all(field.name.as_bytes())?;
    write!(out, ",{},{}", field.column, field.width)?;
    let max_length = |out: &mut dyn Write| match field.max_length {
        Some(length) => write!(out, ",{length}"),
        None => write!(out, ","),
    };
    let text = |out: &mut dyn Write, bytes: &[u8]| -> io::Result<()> {
        out.write_all(b",")?;
        out.write_all(bytes)
    };
    match field.kind {
        FieldKind::Text | FieldKind::Telephone | FieldKind::Email => {
            max_length(out)?;
            text(out, flag(field.required))?;
            if field.kind != FieldKind::Email {
                text(out, field.pattern.as_bytes())?;
            }
            text(out, field.default.as_bytes())?;
        }
        FieldKind::Number | FieldKind::Date | FieldKind::Time => {
            text(out, field.minimum.as_bytes())?;
            text(out, field.maximum.as_bytes())?;
            if field.kind == FieldKind::Number {
                text(out, field.step.as_bytes())?;
            }
            text(out, flag(field.required))?;
            text(out, field.default.as_bytes())?;
        }
        FieldKind::Select | FieldKind::Radio => text(out, flag(field.required))?,
        FieldKind::Checkbox => {
            text(out, field.value.as_bytes())?;
            text(out, flag(field.required))?;
            text(out, flag(field.checked))?;
        }
        FieldKind::TextArea => {
            write!(out, ",{}", field.rows)?;
            max_length(out)?;
            text(out, flag(field.required))?;
            text(out, field.default.as_bytes())?;
        }
        FieldKind::Submit => {
            text(out, field.label.as_bytes())?;
            text(out, field.action.as_bytes())?;
        }
        FieldKind::Reset => text(out, field.label.as_bytes())?,
    }
    writeln!(out)?;
    for option in page.form.options.as_slice().iter().filter(|option| usize::from(option.field) == index) {
        out.write_all(b"OPTION,")?;
        out.write_all(option.value.as_bytes())?;
        text(out, option.display.as_bytes())?;
        text(out, flag(option.selected))?;
        if let Some(column) = option.column {
            write!(out, ",{column}")?;
        }
        writeln!(out)?;
    }
    Ok(())
}

/// A link's target as the Link Extension writes it: an address, a named
/// service's page, or a page of this service.
fn write_destination(destination: &LinkDestination, out: &mut dyn Write) -> io::Result<()> {
    if !destination.url.is_empty() {
        return out.write_all(destination.url.as_bytes());
    }
    if !destination.service.is_empty() {
        out.write_all(destination.service.as_bytes())?;
        out.write_all(b":")?;
    }
    out.write_all(&destination.address.ascii())?;
    if let Some(subcode) = destination.subcode {
        write!(out, ":{:04X}", subcode.value())?;
    }
    Ok(())
}

/// Writes the X/27/0 links as `FL` when every link is to any sub-page and
/// row 24 is shown, otherwise as a packed `OL,27` line.
fn write_fastext(page: &PageFile, out: &mut dyn Write) -> io::Result<()> {
    if page.fastext.iter().all(Option::is_none) {
        return Ok(());
    }
    let simple = page.fastext_row_24 && page.fastext.iter().flatten().all(|link| link.subcode.is_any());
    if simple {
        out.write_all(b"FL")?;
        for link in page.fastext {
            match link {
                Some(link) => {
                    out.write_all(b",")?;
                    out.write_all(&link.address.ascii())?;
                }
                None => write!(out, ",{TTI_FL_NULL_LINK:X}")?,
            }
        }
        return writeln!(out);
    }
    let magazine = page.address.magazine;
    let mut links: [Option<(Magazine, PageNumber, SubCode)>; 6] = [None; 6];
    for (slot, link) in links.iter_mut().zip(page.fastext) {
        *slot = link.map(|link| (link.address.magazine, link.address.page, link.subcode));
    }
    let packet = encode::links(magazine, &links, page.fastext_row_24, 0);
    let mut nibbles = [0; TTI_PACKED_CHARACTERS];
    let body = packet.bytes.get(DATA_START.saturating_add(1)..).unwrap_or(&[]);
    for (slot, &byte) in nibbles.iter_mut().zip(body) {
        // The checksum bytes are not Hamming-coded; the encoder computes them.
        *slot = hamming84(byte).unwrap_or(0);
    }
    write!(out, "OL,{PACKET_LINKS},")?;
    out.write_all(&[TTI_PACKED_CHARACTER_BASE])?;
    write_nibbles(&nibbles, out)?;
    writeln!(out)
}

fn write_page(page: &PageFile, first: &PageFile, is_first: bool, out: &mut dyn Write) -> io::Result<()> {
    out.write_all(b"PN,")?;
    out.write_all(&page.address.ascii())?;
    writeln!(out, "{:02}", page.subpage)?;
    if !is_first && page.description != first.description {
        out.write_all(b"DE,")?;
        out.write_all(page.description.as_bytes())?;
        writeln!(out)?;
    }
    if !is_first && page.source != first.source {
        out.write_all(b"DS,")?;
        out.write_all(page.source.as_bytes())?;
        writeln!(out)?;
    }
    writeln!(out, "SC,{:04X}", page.subcode.value())?;
    writeln!(out, "PS,{:04X}", page.status)?;
    if let Some(region) = page.region {
        writeln!(out, "RE,{region:X}")?;
    }
    if let Some(function) = page.function {
        writeln!(out, "PF,{:X},{:X}", function.function, function.coding)?;
    }
    for row in 0..STORED_ROWS {
        if !page.has_row(row) {
            continue;
        }
        write!(out, "OL,{row},")?;
        write_row_text(page.row(row).map_or(&[][..], |codes| &codes[..]), out)?;
        writeln!(out)?;
        for (index, field) in page.form.fields.as_slice().iter().enumerate() {
            if usize::from(field.row) == row {
                write_field(field, page, index, out)?;
            }
        }
    }
    let mut enhancements = page.enhancements;
    enhancements.as_mut_slice().sort_unstable_by_key(|packet| packet.designation);
    for packet in enhancements.as_slice() {
        write!(out, "OL,{PACKET_ENHANCEMENT},")?;
        out.write_all(&[TTI_PACKED_CHARACTER_BASE | packet.designation])?;
        write_triplets(&packet.triplets, out)?;
        writeln!(out)?;
    }
    for raw in page.raw_packets.as_slice() {
        write!(out, "OL,{},", raw.number)?;
        out.write_all(&[TTI_PACKED_CHARACTER_BASE | raw.designation])?;
        match &raw.body {
            RawBody::Triplets(triplets) => write_triplets(triplets, out)?,
            RawBody::Nibbles(values) => write_nibbles(values, out)?,
        }
        writeln!(out)?;
    }
    write_fastext(page, out)?;
    for link in page.links.as_slice() {
        out.write_all(&[b'L', b'N', b',', link.key.letter(), b','])?;
        let destination =
            LinkDestination { service: link.service, address: link.address, subcode: link.subcode, url: link.url };
        write_destination(&destination, out)?;
        out.write_all(b",")?;
        out.write_all(link.label.as_bytes())?;
        writeln!(out)?;
    }
    for link in page.number_links.as_slice() {
        out.write_all(b"LK,")?;
        out.write_all(&link.number.ascii())?;
        out.write_all(b",")?;
        write_destination(&link.destination, out)?;
        writeln!(out)?;
    }
    for link in page.text_links.as_slice() {
        write!(out, "LT,{},{},{},", link.row, link.column, link.length)?;
        write_destination(&link.destination, out)?;
        writeln!(out)?;
    }
    write_form_lines(page, out)?;
    // REQ-FNT-007: the writer keeps the page's font; a null one came from a
    // malformed line, which is not written back.
    if let PageFont::Named { address, sha256 } = &page.font {
        out.write_all(b"TF,")?;
        out.write_all(address.as_bytes())?;
        out.write_all(b",")?;
        out.write_all(sha256)?;
        writeln!(out)?;
    }
    for warning in Warning::ALL {
        if page.considered & warning.bit() != 0 {
            out.write_all(b"CK,considered,")?;
            out.write_all(warning.name())?;
            writeln!(out)?;
        }
    }
    Ok(())
}

fn write_form_lines(page: &PageFile, out: &mut dyn Write) -> io::Result<()> {
    if let Some((method, address)) = &page.form.submission {
        out.write_all(match method {
            FormMethod::Post => b"FS,POST,",
            FormMethod::Get => b"FS,GET,",
        })?;
        out.write_all(address.as_bytes())?;
        writeln!(out)?;
    }
    if let Some((success, error)) = page.form.result_pages {
        out.write_all(b"FP,")?;
        out.write_all(&success.ascii())?;
        out.write_all(b",")?;
        out.write_all(&error.ascii())?;
        writeln!(out)?;
    }
    if !page.form.token.is_empty() {
        out.write_all(b"FT,")?;
        out.write_all(page.form.token.as_bytes())?;
        writeln!(out)?;
    }
    Ok(())
}

/// Writes sub-pages as one TTI file (REQ-INP-014): the first page's
/// description and source at the top, then each sub-page from its `PN`
/// line. Reading the file reproduces the pages.
pub fn write_tti(pages: &[&PageFile], out: &mut dyn Write) -> io::Result<()> {
    let Some(first) = pages.first() else { return Ok(()) };
    if !first.description.is_empty() {
        out.write_all(b"DE,")?;
        out.write_all(first.description.as_bytes())?;
        writeln!(out)?;
    }
    if !first.source.is_empty() {
        out.write_all(b"DS,")?;
        out.write_all(first.source.as_bytes())?;
        writeln!(out)?;
    }
    for (index, page) in pages.iter().enumerate() {
        write_page(page, first, index == 0, out)?;
    }
    Ok(())
}
