//! Enhancement data written by function (REQ-INP-025): every row and
//! column triplet of Tables 28 and 29 by name, with its fields.

use std::vec::Vec;

use neoteletext_core::consts::{ROW_ADDRESS_BASE, ROW_ADDRESS_LAST, ROW_ADDRESS_ROW_24};
use neoteletext_core::packet::{
    ColourData, DisplayAttributes, ObjectKind, ObjectNumber, ObjectSource, Triplet, TripletFunction,
};

use super::{AuthorError, Result};

/// The rows that a Full Row Colour or Address Display Row 0 triplet
/// colours: bits S1 S0 of its data (Table 28).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum RowScope {
    /// 00: this row only.
    ThisRow,
    /// 11: this row and every row below it.
    DownToBottom,
    /// 01 or 10, which set the position without a colour (decision E.45).
    Reserved(u8),
}

impl RowScope {
    const fn bits(self) -> u8 {
        match self {
            Self::ThisRow => 0b00,
            Self::DownToBottom => 0b11,
            Self::Reserved(bits) => bits & 0b11,
        }
    }
}

/// The flash mode of an Additional Flash Functions triplet, bits D1 D0
/// (Table 29, mode 00111).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FlashMode {
    /// 00: "Steady" (Table 29).
    Steady,
    /// 01: "Normal flash to background colour".
    Normal,
    /// 10: "Invert phase of flash to background colour".
    InvertPhase,
    /// 11: "Flash to the corresponding colour in an adjacent CLUT".
    AdjacentClut,
}

impl FlashMode {
    /// Every mode, in the order of their codes.
    pub const ALL: [Self; 4] = [Self::Steady, Self::Normal, Self::InvertPhase, Self::AdjacentClut];

    const fn bits(self) -> u8 {
        match self {
            Self::Steady => 0b00,
            Self::Normal => 0b01,
            Self::InvertPhase => 0b10,
            Self::AdjacentClut => 0b11,
        }
    }
}

/// The flash rate and phase of an Additional Flash Functions triplet, bits
/// D4 to D2 (Table 29, mode 00111).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FlashRate {
    /// 000: slow, 1 hertz.
    Slow,
    /// 001 to 011: fast, 2 hertz, with phase 1, 2 or 3.
    Fast(u8),
    /// 100: fast, phases incrementing left to right.
    Incremental,
    /// 101: fast, phases decrementing left to right.
    Decremental,
    /// 110 or 111: reserved.
    Reserved(u8),
}

impl FlashRate {
    /// Every defined rate: slow, the three fast phases, incremental and
    /// decremental.
    pub const ALL: [Self; 6] =
        [Self::Slow, Self::Fast(1), Self::Fast(2), Self::Fast(3), Self::Incremental, Self::Decremental];

    const fn bits(self) -> u8 {
        match self {
            Self::Slow => 0b000,
            Self::Fast(phase) => phase & 0b011,
            Self::Incremental => 0b100,
            Self::Decremental => 0b101,
            Self::Reserved(bits) => bits & 0b111,
        }
    }
}

/// An object as an invocation names it: source, type and number
/// (Table 28, modes 10001 to 10011).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct ObjectRef {
    /// Local, POP or GPOP.
    pub source: ObjectSource,
    /// Active, Adaptive or Passive.
    pub kind: ObjectKind,
    /// N8 to N0.
    pub number: ObjectNumber,
}

impl ObjectRef {
    /// The invocation triplet's function.
    #[must_use]
    pub const fn invocation(self) -> TripletFunction {
        TripletFunction::ObjectInvocation { source: self.source, kind: self.kind, number: self.number }
    }

    /// The same object named through another source, for instance a POP
    /// page that another display page links as its GPOP.
    #[must_use]
    pub const fn with_source(self, source: ObjectSource) -> Self {
        Self { source, ..self }
    }

    /// The same number invoked as another type, which a decoder skips
    /// (decision E.51).
    #[must_use]
    pub const fn as_kind(self, kind: ObjectKind) -> Self {
        Self { kind, ..self }
    }
}

/// A Local Object of a display page, before its place in the page's
/// packets X/26 is known ([`super::DisplayPage::local_object`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct LocalObject {
    pub(super) index: usize,
    pub(super) kind: ObjectKind,
}

impl LocalObject {
    /// The object's type.
    #[must_use]
    pub const fn kind(self) -> ObjectKind {
        self.kind
    }
}

/// One element of a triplet list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Element {
    /// A triplet as written.
    Triplet(Triplet),
    /// An invocation of a Local Object, resolved when the page's packets
    /// X/26 are laid out.
    Local {
        /// The object.
        object: LocalObject,
        /// The type it is invoked as.
        kind: ObjectKind,
    },
}

/// The row address of a page row for Set Active Position and Full Row
/// Colour: rows 1 to 23 are addresses 41 to 63 and row 24 is address 40
/// (clause 12.3.1). Inside an object the same addresses give the row
/// offset from the object's origin.
pub fn row_address(row: u8) -> Result<u8> {
    match row {
        1..=23 => Ok(ROW_ADDRESS_BASE.saturating_add(row)),
        24 => Ok(ROW_ADDRESS_ROW_24),
        _ => Err(AuthorError::BadPosition),
    }
}

/// A list of enhancement triplets: the local enhancement data of a page or
/// the body of an object. Every function of Tables 28 and 29 has a method;
/// [`Triplets::function`] and [`Triplets::raw`] write any other value,
/// reserved ones included.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Triplets {
    pub(super) elements: Vec<Element>,
}

impl Triplets {
    /// An empty list.
    #[must_use]
    pub const fn new() -> Self {
        Self { elements: Vec::new() }
    }

    /// The number of triplets.
    #[must_use]
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// True when there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Appends a triplet as written, reserved values included.
    pub fn raw(&mut self, triplet: Triplet) -> &mut Self {
        self.elements.push(Element::Triplet(triplet));
        self
    }

    /// Appends the triplet of a function (REQ-PKT-010's typed value).
    pub fn function(&mut self, function: TripletFunction) -> &mut Self {
        self.raw(function.to_triplet())
    }

    /// Appends `filler` until the list holds `length` triplets, so that the
    /// next triplet lands at a chosen place in the packets.
    pub fn pad_to(&mut self, length: usize, filler: TripletFunction) -> &mut Self {
        while self.elements.len() < length {
            self.function(filler);
        }
        self
    }

    // -- Row address group (Table 28) -------------------------------------

    /// Full Screen Colour (row mode 00000).
    pub fn full_screen_colour(&mut self, colour: u8) -> &mut Self {
        self.function(TripletFunction::FullScreenColour(ColourData { index: colour, high: 0 }))
    }

    /// Full Row Colour (row mode 00001) for page row `row` (1 to 24).
    pub fn full_row_colour(&mut self, row: u8, colour: u8, scope: RowScope) -> Result<&mut Self> {
        let address = row_address(row)?;
        Ok(self.function(TripletFunction::FullRowColour {
            address,
            colour: ColourData { index: colour, high: scope.bits() },
        }))
    }

    /// Set Active Position (row mode 00100) to page row `row` (1 to 24)
    /// and column `column`.
    pub fn position(&mut self, row: u8, column: u8) -> Result<&mut Self> {
        let address = row_address(row)?;
        Ok(self.function(TripletFunction::SetActivePosition { address, column }))
    }

    /// Set Active Position with a raw row address 40 to 63 and data, for
    /// the reserved forms (data 40 or more: decision E.46).
    pub fn position_address(&mut self, address: u8, data: u8) -> &mut Self {
        self.function(TripletFunction::SetActivePosition { address, column: data })
    }

    /// Address Display Row 0 (row mode 00111, address 63), with the colour
    /// of a Full Row Colour.
    pub fn row_0(&mut self, colour: u8, scope: RowScope) -> &mut Self {
        self.function(TripletFunction::AddressDisplayRow0 {
            address: ROW_ADDRESS_LAST,
            colour: ColourData { index: colour, high: scope.bits() },
        })
    }

    /// Origin Modifier (row mode 10000): the next invocation's origin is
    /// moved by these offsets.
    pub fn origin(&mut self, row_offset: u8, column_offset: u8) -> &mut Self {
        self.function(TripletFunction::OriginModifier { row_offset, column_offset })
    }

    /// Object Invocation (row modes 10001 to 10011) of a public, global or
    /// numbered local object.
    pub fn invoke(&mut self, object: ObjectRef) -> &mut Self {
        self.function(object.invocation())
    }

    /// Object Invocation of a Local Object of this page, as its own type.
    pub fn invoke_local(&mut self, object: LocalObject) -> &mut Self {
        self.invoke_local_as(object, object.kind)
    }

    /// Object Invocation of a Local Object as a chosen type, which a
    /// decoder skips when it differs (decision E.51).
    pub fn invoke_local_as(&mut self, object: LocalObject, kind: ObjectKind) -> &mut Self {
        self.elements.push(Element::Local { object, kind });
        self
    }

    /// Termination Marker (row mode 11111, address 63) of a type
    /// ([`crate::consts::TERMINATION_LOCAL_DATA_END`] and the others).
    pub fn terminate(&mut self, kind: u8) -> &mut Self {
        self.function(TripletFunction::Termination { kind })
    }

    /// A Programme Delivery Control row triplet (row modes 01000 to 01101),
    /// which presentation ignores (Part G).
    pub fn pdc_row(&mut self, mode: u8, address: u8, data: u8) -> &mut Self {
        self.function(TripletFunction::ProgrammeDeliveryRow { mode, address, data })
    }

    // -- Column address group (Table 29) ----------------------------------

    /// Foreground Colour (column mode 00000).
    pub fn foreground(&mut self, column: u8, colour: u8) -> &mut Self {
        self.function(TripletFunction::ForegroundColour { column, colour: ColourData { index: colour, high: 0 } })
    }

    /// Background Colour (column mode 00011).
    pub fn background(&mut self, column: u8, colour: u8) -> &mut Self {
        self.function(TripletFunction::BackgroundColour { column, colour: ColourData { index: colour, high: 0 } })
    }

    /// A G0 character without a diacritical mark (column mode 10000).
    pub fn character(&mut self, column: u8, code: u8) -> &mut Self {
        self.diacritic(column, code, 0)
    }

    /// G0 characters at consecutive columns from `column` (column mode
    /// 10000).
    pub fn text(&mut self, column: u8, text: &[u8]) -> &mut Self {
        for (offset, &code) in (0u8..).zip(text) {
            self.character(column.saturating_add(offset), code);
        }
        self
    }

    /// A G0 character with diacritical mark `mark` 0 to 15 (column modes
    /// 10000 to 11111).
    pub fn diacritic(&mut self, column: u8, code: u8, mark: u8) -> &mut Self {
        self.function(TripletFunction::G0Diacritic { column, code, diacritic: mark })
    }

    /// A G0 character of Levels 2.5 and 3.5 (column mode 01001).
    pub fn g0(&mut self, column: u8, code: u8) -> &mut Self {
        self.function(TripletFunction::G0Character { column, code })
    }

    /// A G1 block mosaic character (column mode 00001).
    pub fn g1(&mut self, column: u8, code: u8) -> &mut Self {
        self.function(TripletFunction::G1Character { column, code })
    }

    /// A G2 character (column mode 01111).
    pub fn g2(&mut self, column: u8, code: u8) -> &mut Self {
        self.function(TripletFunction::G2Character { column, code })
    }

    /// A G3 character in the form of Levels 2.5 and 3.5 (column mode
    /// 01011).
    pub fn g3(&mut self, column: u8, code: u8) -> &mut Self {
        self.function(TripletFunction::G3Character { column, code, level_15_form: false })
    }

    /// A G3 character in the Level 1.5 form (column mode 00010).
    pub fn g3_level_15(&mut self, column: u8, code: u8) -> &mut Self {
        self.function(TripletFunction::G3Character { column, code, level_15_form: true })
    }

    /// Additional Flash Functions (column mode 00111).
    pub fn flash(&mut self, column: u8, mode: FlashMode, rate: FlashRate) -> &mut Self {
        self.function(TripletFunction::AdditionalFlash { column, mode: mode.bits(), rate: rate.bits(), reserved: 0 })
    }

    /// Display Attributes (column mode 01100).
    pub fn attributes(&mut self, column: u8, attributes: DisplayAttributes) -> &mut Self {
        self.function(TripletFunction::DisplayAttributes { column, attributes })
    }

    /// Modified G0 and G2 Character Set Designation (column mode 01000)
    /// with a Table 34 value.
    pub fn designation(&mut self, column: u8, designation: u8) -> &mut Self {
        self.function(TripletFunction::ModifiedDesignation { column, designation })
    }

    /// A Programme Delivery Control column triplet (column mode 00110).
    pub fn pdc_column(&mut self, column: u8, data: u8) -> &mut Self {
        self.function(TripletFunction::ProgrammeDeliveryColumn { column, data })
    }

    /// Appends every element of another list.
    pub fn extend(&mut self, other: &Self) -> &mut Self {
        self.elements.extend_from_slice(&other.elements);
        self
    }

    /// The triplets, with Local Object invocations resolved by `resolve`.
    pub(super) fn resolved(
        &self,
        resolve: &dyn Fn(LocalObject, ObjectKind) -> Result<Triplet>,
    ) -> Result<Vec<Triplet>> {
        self.elements
            .iter()
            .map(|element| match *element {
                Element::Triplet(triplet) => Ok(triplet),
                Element::Local { object, kind } => resolve(object, kind),
            })
            .collect()
    }
}
