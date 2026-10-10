//! Builders for the Level 2.5 and 3.5 tests: pages, object pages, DRCS
//! pages and Magazine Organization Tables made with `packet::encode`, fed
//! through the decoder, and built into grids by the Level 1 pass and the
//! enhancement processor, as a receiver would.
// Each test file uses its own subset of these builders.
#![allow(dead_code, clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

use tellytxt_core::acq::{Decoder, EnhancementPacket, EnhancementStore, EnhancementSubPage, StoredPage};
use tellytxt_core::charset::{CharSet, to_unicode};
use tellytxt_core::config::{Config, PresentationLevel};
use tellytxt_core::consts::{
    COMPOSITIONAL_LINKS, DATA_BYTES, DESIGNATION_CODES, DRCS_PTU_BYTES, DRCS_PTUS, MAGAZINES, OBJECT_POINTER_UNUSED,
    PACKET_ENHANCEMENT, PAGE_FUNCTION_DRCS, PAGE_TIME_FILLING, ROW_ADDRESS_LAST, TRIPLETS_PER_PACKET,
};
use tellytxt_core::ecc::with_odd_parity;
use tellytxt_core::enhance::{self, EnhancementReport};
use tellytxt_core::level1::{Cell, DisplayOptions, Glyph, Grid, build_grid};
use tellytxt_core::packet::{
    CompositionalLink, DefaultObject, DrcsModes, MotDrcsLink, MotObjectLink, MotPageEntry, ObjectKind, ObjectNumber,
    ObjectSource, ObjectUsage, Packet, PacketMeta, PageFormat, Triplet, TripletFunction, encode, mot_position,
};
use tellytxt_core::types::{
    ControlBits, FallbackFields, Instant, Magazine, PageAddress, PageNumber, PresentationFields, SubCode,
};
use tellytxt_core::viewer::compose_level;

/// The displayed page of the tests: 100.
pub const DISPLAY: u16 = 0x100;

pub fn mag(number: u8) -> Magazine {
    Magazine::new(number).unwrap()
}

/// A header of page `mpp`.
pub fn header(mpp: u16, subcode: u16, control: u16) -> Packet {
    let address = PageAddress::from_mpp(mpp).unwrap();
    encode::header(
        address.magazine,
        address.page,
        SubCode::new(subcode),
        ControlBits(control),
        &[with_odd_parity(b' '); 32],
    )
}

/// A display row of magazine 1 from 7-bit codes, padded with spaces.
pub fn row(number: u8, codes: &[u8]) -> Packet {
    let mut bytes = [with_odd_parity(b' '); DATA_BYTES];
    for (slot, &code) in bytes.iter_mut().zip(codes) {
        *slot = with_odd_parity(code);
    }
    encode::row(mag(1), number, &bytes)
}

/// A triplet that does nothing: a reserved row mode (Table 28, 00010).
pub fn filler() -> Triplet {
    TripletFunction::ReservedRow { mode: 0b00010, address: 41, data: 0 }.to_triplet()
}

/// A packet X/26 of magazine `magazine` (`number` 26) or an object page
/// packet, with up to thirteen functions and fillers after them.
pub fn triplet_packet(magazine: u8, number: u8, byte_6: u8, functions: &[TripletFunction]) -> Packet {
    let mut triplets = [filler(); TRIPLETS_PER_PACKET];
    assert!(functions.len() <= TRIPLETS_PER_PACKET);
    for (slot, function) in triplets.iter_mut().zip(functions) {
        *slot = function.to_triplet();
    }
    encode::triplet_packet(mag(magazine), number, byte_6, &triplets)
}

/// A page's packet X/26 with designation code `designation`.
pub fn x26(designation: u8, functions: &[TripletFunction]) -> Packet {
    triplet_packet(1, PACKET_ENHANCEMENT, designation, functions)
}

/// The local enhancement data in as many packets X/26 as it needs, from
/// designation code 0, ended by a Termination Marker.
pub fn local_data(functions: &[TripletFunction]) -> Vec<Packet> {
    let mut all = functions.to_vec();
    all.push(TripletFunction::Termination { kind: 0b111 });
    all.chunks(TRIPLETS_PER_PACKET).enumerate().map(|(code, chunk)| x26(code as u8, chunk)).collect()
}

/// Corrupts triplet `index` of a packet beyond correction: two bits of
/// each of its bytes flipped.
pub fn damage(mut packet: Packet, index: usize) -> Packet {
    let start = 3 + 3 * index;
    for byte in &mut packet.bytes[start..start + 3] {
        *byte ^= 0b0000_0110;
    }
    packet
}

/// The display page 100: a header, its rows and its other packets.
pub fn display_page(control: u16, rows: &[(u8, &[u8])], others: Vec<Packet>) -> Vec<Packet> {
    let mut packets = vec![header(DISPLAY, 0, control)];
    packets.extend(others);
    for (number, codes) in rows {
        packets.push(row(*number, codes));
    }
    packets
}

/// Position shorthands for the triplet functions.
pub fn set_position(row: u8, column: u8) -> TripletFunction {
    let address = if row == 24 { 40 } else { 40 + row };
    TripletFunction::SetActivePosition { address, column }
}

pub fn g0(column: u8, code: u8) -> TripletFunction {
    TripletFunction::G0Character { column, code }
}

pub fn text_at(column: u8, text: &[u8]) -> Vec<TripletFunction> {
    text.iter().enumerate().map(|(index, &code)| g0(column + index as u8, code)).collect()
}

pub fn foreground(column: u8, index: u8) -> TripletFunction {
    TripletFunction::ForegroundColour { column, colour: tellytxt_core::packet::ColourData { index, high: 0 } }
}

pub fn background(column: u8, index: u8) -> TripletFunction {
    TripletFunction::BackgroundColour { column, colour: tellytxt_core::packet::ColourData { index, high: 0 } }
}

pub fn invoke(source: ObjectSource, kind: ObjectKind, number: ObjectNumber) -> TripletFunction {
    TripletFunction::ObjectInvocation { source, kind, number }
}

pub fn origin(row_offset: u8, column_offset: u8) -> TripletFunction {
    TripletFunction::OriginModifier { row_offset, column_offset }
}

pub fn define(kind: ObjectKind, usage: u8, number: ObjectNumber) -> TripletFunction {
    TripletFunction::ObjectDefinition { usage: ObjectUsage(usage), kind, number }
}

/// A local object's number: designation code and triplet (Table 28).
pub const fn local_number(designation: u8, triplet: u8) -> ObjectNumber {
    ObjectNumber((designation as u16) << 4 | triplet as u16)
}

/// A public or global object's number: object `index` (0 to 15) of its
/// type on sub-page `s1`: pointer packet N8 N7 = index / 8, group N6 N5 =
/// (index mod 8) / 2, half N4 = index mod 2 (Table 28).
pub const fn public_number(s1: u8, index: u8) -> ObjectNumber {
    let packet = (index / 8) as u16;
    let group = ((index % 8) / 2) as u16;
    let odd = (index % 2) as u16;
    ObjectNumber(packet << 7 | group << 5 | odd << 4 | s1 as u16)
}

/// One object of an object page.
#[derive(Clone)]
pub struct ObjectDef {
    pub kind: ObjectKind,
    /// The object's index within its type on the sub-page, 0 to 15.
    pub index: u8,
    /// Definition usage bits: 01 Level 2.5, 10 Level 3.5, 11 both.
    pub usage: u8,
    pub body: Vec<TripletFunction>,
}

impl ObjectDef {
    pub fn new(kind: ObjectKind, index: u8, body: Vec<TripletFunction>) -> Self {
        Self { kind, index, usage: 0b11, body }
    }
}

const fn type_offset(kind: ObjectKind) -> usize {
    match kind {
        ObjectKind::Active => 0,
        ObjectKind::Adaptive => 1,
        ObjectKind::Passive => 2,
    }
}

/// The 507 triplets and the pointer tables of an object page sub-page,
/// before they are put into packets. Each object is its definition, its
/// body and a Termination Marker.
pub struct ObjectPageData {
    pub triplets: Vec<Triplet>,
    pub pointers: [[(u16, u16); TRIPLETS_PER_PACKET]; 2],
    pub used: usize,
}

pub fn object_page_data(s1: u8, objects: &[ObjectDef]) -> ObjectPageData {
    let end = TripletFunction::Termination { kind: 0b011 }.to_triplet();
    let mut triplets = vec![end; 507];
    let mut pointers = [[(OBJECT_POINTER_UNUSED, OBJECT_POINTER_UNUSED); TRIPLETS_PER_PACKET]; 2];
    let mut position = 0usize;
    for object in objects {
        let number = public_number(s1, object.index);
        let packet = usize::from(object.index / 8);
        let triplet = 1 + 3 * usize::from((object.index % 8) / 2) + type_offset(object.kind);
        let pointer = &mut pointers[packet][triplet];
        if object.index % 2 == 0 {
            pointer.0 = position as u16;
        } else {
            pointer.1 = position as u16;
        }
        triplets[position] = define(object.kind, object.usage, number).to_triplet();
        position += 1;
        for function in &object.body {
            triplets[position] = function.to_triplet();
            position += 1;
        }
        triplets[position] = end;
        position += 1;
    }
    ObjectPageData { triplets, pointers, used: position }
}

/// The packets of an object page sub-page: header, pointer packets 1 and
/// 2, and the definition packets from 3 (and X/26 when needed).
pub fn object_page_packets(mpp: u16, s1: u8, data: &ObjectPageData) -> Vec<Packet> {
    let magazine = PageAddress::from_mpp(mpp).unwrap().magazine.number();
    let mut packets = vec![header(mpp, u16::from(s1), 0)];
    for (index, table) in data.pointers.iter().enumerate() {
        let mut triplets = [filler(); TRIPLETS_PER_PACKET];
        for (slot, (even, odd)) in triplets.iter_mut().zip(table).skip(1) {
            *slot = encode::pointer_triplet(*even, *odd);
        }
        packets.push(encode::triplet_packet(mag(magazine), 1 + index as u8, 1, &triplets));
    }
    let packets_used = data.used.div_ceil(TRIPLETS_PER_PACKET).max(1);
    for packet in 0..packets_used {
        let chunk: [Triplet; TRIPLETS_PER_PACKET] =
            data.triplets[packet * TRIPLETS_PER_PACKET..][..TRIPLETS_PER_PACKET].try_into().unwrap();
        let (number, byte_6) = if packet < 23 { (3 + packet as u8, 0) } else { (26, (packet - 23) as u8) };
        packets.push(encode::triplet_packet(mag(magazine), number, byte_6, &chunk));
    }
    packets
}

pub fn object_page(mpp: u16, s1: u8, objects: &[ObjectDef]) -> Vec<Packet> {
    object_page_packets(mpp, s1, &object_page_data(s1, objects))
}

/// An X/27/4 or X/27/5 link (Table 15) to `mpp`, all sub-pages.
pub fn link(function: u8, validity: u8, mpp: u16) -> CompositionalLink {
    let address = PageAddress::from_mpp(mpp).unwrap();
    CompositionalLink {
        function,
        validity,
        magazine: address.magazine,
        page: address.page,
        subpages: 0xFFFF,
        marker_first: true,
        marker_second: false,
    }
}

/// A null link (decision E.52: page FF).
pub fn null_link(function: u8) -> CompositionalLink {
    CompositionalLink { page: PageNumber(0xFF), validity: 0, subpages: 0, ..link(function, 0b11, 0x1FF) }
}

/// Packet X/27/4 (designation 4) or X/27/5 (designation 5) of page 100.
pub fn x27(designation: u8, links: [CompositionalLink; COMPOSITIONAL_LINKS]) -> Packet {
    encode::compositional_links(mag(1), designation, &links)
}

/// Packet X/28/0 format 1 of a basic page, or X/28/4.
pub fn x28(designation: u8, fields: &PresentationFields) -> Packet {
    encode::page_enhancement(mag(1), designation, PageFormat { function: 0, coding: 0 }, fields)
}

/// The Magazine Organization Table of magazine 1.
#[derive(Clone, Default)]
pub struct Mot {
    pub entries: Vec<(u8, MotPageEntry)>,
    pub gpop: Option<MotObjectLink>,
    pub pops: [Option<MotObjectLink>; 7],
    pub gdrcs: Option<MotDrcsLink>,
    pub drcs: [Option<MotDrcsLink>; 7],
    /// Send the links in X/19 to X/21 (Level 2.5).
    pub level_25: bool,
    /// Send them in X/22 to X/24 (Level 3.5), with these packets left out.
    pub level_35: bool,
    pub omit: Vec<u8>,
}

pub fn object_link(mpp: u16, fallback: FallbackFields, defaults: [Option<DefaultObject>; 2]) -> MotObjectLink {
    let address = PageAddress::from_mpp(mpp).unwrap();
    MotObjectLink {
        magazine: address.magazine,
        x27_flag: false,
        page: address.page,
        subpage_count: 0,
        fallback,
        default_objects: defaults,
    }
}

pub fn drcs_link(mpp: u16) -> MotDrcsLink {
    let address = PageAddress::from_mpp(mpp).unwrap();
    MotDrcsLink { magazine: address.magazine, x27_flag: false, page: address.page, subpage_count: 15 }
}

fn unused_object_link() -> MotObjectLink {
    MotObjectLink { page: PageNumber(0xFF), ..object_link(0x1FF, FallbackFields::default(), [None; 2]) }
}

pub fn mot_packets(mot: &Mot) -> Vec<Packet> {
    let mut packets = vec![header(0x1FE, 0, 0)];
    let mut pairs: Vec<(u8, [u8; DATA_BYTES])> = Vec::new();
    for (page, entry) in &mot.entries {
        let position = mot_position(PageNumber(*page));
        if !pairs.iter().any(|(packet, _)| *packet == position.packet) {
            pairs.push((position.packet, [0; DATA_BYTES]));
        }
        let (_, values) = pairs.iter_mut().find(|(packet, _)| *packet == position.packet).unwrap();
        values[position.index..position.index + 2].copy_from_slice(&encode::mot_page_entry(*entry));
    }
    for (packet, values) in &pairs {
        packets.push(encode::nibble_packet(mag(1), *packet, values));
    }
    let object_links = |first: Option<MotObjectLink>, rest: &[Option<MotObjectLink>]| {
        let mut values = [0u8; DATA_BYTES];
        for (index, link) in core::iter::once(first).chain(rest.iter().copied()).enumerate() {
            let link = link.unwrap_or_else(unused_object_link);
            values[index * 10..index * 10 + 10].copy_from_slice(&encode::mot_object_link(&link));
        }
        values
    };
    let first = object_links(mot.gpop, &mot.pops[..3]);
    let second = object_links(mot.pops[3], &mot.pops[4..]);
    let mut drcs = [0u8; DATA_BYTES];
    for (index, link) in core::iter::once(mot.gdrcs).chain(mot.drcs.iter().copied()).enumerate() {
        let link = link.unwrap_or(MotDrcsLink { page: PageNumber(0xFF), ..drcs_link(0x1FF) });
        drcs[index * 4..index * 4 + 4].copy_from_slice(&encode::mot_drcs_link(&link));
    }
    let mut add = |number: u8, values: &[u8; DATA_BYTES]| {
        if !mot.omit.contains(&number) {
            packets.push(encode::nibble_packet(mag(1), number, values));
        }
    };
    if mot.level_25 {
        add(19, &first);
        add(20, &second);
        add(21, &drcs);
    }
    if mot.level_35 {
        add(22, &first);
        add(23, &second);
        add(24, &drcs);
    }
    packets
}

/// One PTU of 12 by 10 dots from ten rows of 12-bit lines, bit 11 the
/// leftmost dot (clause 14.2.1).
pub fn ptu(lines: [u16; 10]) -> [u8; DRCS_PTU_BYTES] {
    let mut bytes = [0u8; DRCS_PTU_BYTES];
    for (row, line) in lines.iter().enumerate() {
        bytes[2 * row] = with_odd_parity(0x40 | ((line >> 6) & 0x3F) as u8);
        bytes[2 * row + 1] = with_odd_parity(0x40 | (line & 0x3F) as u8);
    }
    bytes
}

/// A DRCS page sub-page: PTUs by number, and the mode codes of X/28/3 when
/// given. PTUs not given are blank (D-bytes 4/0).
pub fn drcs_page(
    mpp: u16,
    s1: u8,
    ptus: &[(usize, [u8; DRCS_PTU_BYTES])],
    modes: Option<[u8; DRCS_PTUS]>,
) -> Vec<Packet> {
    let address = PageAddress::from_mpp(mpp).unwrap();
    let mut packets = vec![header(mpp, u16::from(s1), 0)];
    if let Some(modes) = modes {
        let format = PageFormat { function: PAGE_FUNCTION_DRCS, coding: 0 };
        packets.push(encode::drcs_modes(address.magazine, format, &DrcsModes::from_codes(Some(format), &modes)));
    }
    let highest = ptus.iter().map(|(number, _)| *number).max().unwrap_or(0);
    for packet in 0..=highest / 2 {
        let mut bytes = [with_odd_parity(0x40); DATA_BYTES];
        for (number, data) in ptus {
            if number / 2 == packet {
                let start = (number % 2) * DRCS_PTU_BYTES;
                bytes[start..start + DRCS_PTU_BYTES].copy_from_slice(data);
            }
        }
        packets.push(encode::row(address.magazine, 1 + packet as u8, &bytes));
    }
    packets
}

/// Owns a decoder's memory, so that tests can build grids from it.
pub struct Receiver {
    pages: Vec<StoredPage>,
    sub_pages: Vec<EnhancementSubPage>,
    packets: Vec<EnhancementPacket>,
}

impl Receiver {
    pub fn new() -> Self {
        Self {
            pages: vec![StoredPage::EMPTY; 32],
            sub_pages: vec![EnhancementSubPage::EMPTY; 64],
            packets: vec![EnhancementPacket::EMPTY; 1200],
        }
    }

    /// Feeds the packets, closes every page, and calls `f` with the decoder.
    pub fn run<R>(&mut self, packets: &[Packet], config: Config, f: impl FnOnce(&Decoder<'_>) -> R) -> R {
        let store = EnhancementStore::new(&mut self.sub_pages, &mut self.packets);
        let mut decoder = Decoder::with_enhancement_store(&mut self.pages, store, config);
        let mut all = packets.to_vec();
        for magazine in 1..=MAGAZINES as u16 {
            all.push(header(magazine << 8 | u16::from(PAGE_TIME_FILLING), 0, 0));
        }
        for (index, packet) in all.iter().enumerate() {
            decoder.push(packet, &PacketMeta::default(), Instant(index as u64 * 64), &mut |_| {});
        }
        decoder.tick(Instant(1_000_000_000), &mut |_| {});
        f(&decoder)
    }
}

/// The grid of page 100 as the viewer builds it, and at Levels 2.5 and 3.5
/// the enhancement report.
pub fn grid_of(decoder: &Decoder<'_>, config: &Config) -> (Box<Grid>, Option<EnhancementReport>) {
    page_grid(decoder, DISPLAY, config)
}

/// The grid of any stored page.
pub fn page_grid(decoder: &Decoder<'_>, mpp: u16, config: &Config) -> (Box<Grid>, Option<EnhancementReport>) {
    let address = PageAddress::from_mpp(mpp).unwrap();
    let page = decoder.store().get(address, SubCode::ANY).expect("the page is stored");
    let magazine = decoder.magazine_data(address.magazine);
    let mut grid = Box::new(Grid::EMPTY);
    let options = DisplayOptions::default();
    build_grid(&mut grid, Some(page), &magazine, config, &options);
    let lookup = decoder.enhancement_lookup();
    if config.level.is_enhanced() {
        let report = enhance::apply(&mut grid, page, &magazine, &lookup, config, &options);
        return (grid, Some(report));
    }
    let _ = compose_level(&mut grid, page, &magazine, &lookup, config, &options);
    (grid, None)
}

/// Builds page 100 from a stream at a level.
pub fn build(packets: &[Packet], level: PresentationLevel) -> (Box<Grid>, Option<EnhancementReport>) {
    let config = Config { level, ..Config::default() };
    Receiver::new().run(packets, config, |decoder| grid_of(decoder, &config))
}

/// Builds page 100 at Level 2.5.
pub fn build_25(packets: &[Packet]) -> Box<Grid> {
    build(packets, PresentationLevel::Level25).0
}

/// Builds page 100 at Level 3.5.
pub fn build_35(packets: &[Packet]) -> Box<Grid> {
    build(packets, PresentationLevel::Level35).0
}

/// The character a cell shows, as Unicode: a space for no glyph, `#` for a
/// mosaic, and the fallback for DRCS.
pub fn char_of(cell: &Cell) -> char {
    match cell.glyph {
        Glyph::Char(character) => to_unicode(character).base,
        Glyph::Mosaic { .. } => '#',
        Glyph::Space => ' ',
    }
}

/// Asserts that two grids have the same cells, naming the first that
/// differs.
pub fn assert_same_cells(left: &Grid, right: &Grid, label: &str) {
    for (row, (a, b)) in left.cells.iter().zip(right.cells.iter()).enumerate() {
        for (column, (x, y)) in a.iter().zip(b.iter()).enumerate() {
            assert_eq!(x, y, "{label}: row {row} column {column}");
        }
    }
}

/// A row's text.
pub fn text(grid: &Grid, row: usize) -> String {
    grid.cells[row].iter().map(char_of).collect()
}

/// The set of a cell's character, if it shows one.
pub fn set_of(cell: &Cell) -> Option<CharSet> {
    match cell.glyph {
        Glyph::Char(character) => Some(character.set),
        _ => None,
    }
}

/// The Termination Marker that ends local enhancement data.
pub fn end() -> TripletFunction {
    TripletFunction::Termination { kind: 0b111 }
}

/// Address Display Row 0 with a row colour.
pub fn row_0(colour: u8, high: u8) -> TripletFunction {
    TripletFunction::AddressDisplayRow0 {
        address: ROW_ADDRESS_LAST,
        colour: tellytxt_core::packet::ColourData { index: colour, high },
    }
}

pub const DESIGNATIONS: usize = DESIGNATION_CODES;

/// The worst-case page of REQ-TST-019: page 100 invoking, from all sixteen
/// packets X/26, an Active object of its GPOP 1A1 at 100 positions; each
/// object writes its cell, invokes the next type twice (Active, Adaptive,
/// Passive) and is padded so that the three fill the object page; both side
/// panels are on.
pub fn worst_case_page() -> Vec<Packet> {
    const GPOP: u16 = 0x1A1;
    let pad = |count: usize| {
        core::iter::repeat_n(TripletFunction::ReservedRow { mode: 0b00010, address: 41, data: 0 }, count)
    };
    let nested = |letter: u8, next: Option<ObjectKind>| {
        let mut body = vec![g0(0, letter)];
        if let Some(kind) = next {
            body.extend(core::iter::repeat_n(invoke(ObjectSource::Global, kind, public_number(0, 0)), 2));
        }
        body.extend(pad(162 - body.len()));
        body
    };
    let gpop = object_page(
        GPOP,
        0,
        &[
            ObjectDef::new(ObjectKind::Active, 0, nested(b'a', Some(ObjectKind::Adaptive))),
            ObjectDef::new(ObjectKind::Adaptive, 0, nested(b'd', Some(ObjectKind::Passive))),
            ObjectDef::new(ObjectKind::Passive, 0, nested(b'w', None)),
        ],
    );
    let mut local = vec![];
    for index in 0..100u8 {
        if index % 39 == 0 {
            local.push(set_position(1 + index / 39, 0));
        }
        local.push(g0(index % 39, 0x10));
        local.push(invoke(ObjectSource::Global, ObjectKind::Active, public_number(0, 0)));
    }
    let panels = PresentationFields {
        side_panels: Some(tellytxt_core::types::SidePanelFields {
            left: true,
            right: true,
            status_flag: true,
            left_columns: 8,
        }),
        ..PresentationFields::default()
    };
    let none = |function| null_link(function);
    let links = x27(4, [link(0b00, 0b11, GPOP), none(0b01), none(0b10), none(0b11), none(0), none(0)]);
    let mut others = vec![links, x28(0, &panels)];
    let data = local_data(&local);
    assert_eq!(data.len(), 16, "all sixteen packets X/26");
    others.extend(data);
    // The display page first, so that its X/27/4 names the GPOP before the
    // GPOP arrives.
    let mut packets = display_page(0, &[(1, b"WORST CASE")], others);
    packets.extend(gpop);
    packets
}
