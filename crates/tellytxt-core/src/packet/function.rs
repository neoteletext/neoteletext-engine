//! The function of an enhancement triplet as a typed value (REQ-PKT-010):
//! every row mode of Table 28 and every column mode of Table 29, with the
//! fields those tables define. The Level 1.5 pass, the enhancement processor
//! and the object resolver all read triplets through this one decoding.
//!
//! [`TripletFunction::to_triplet`] is the inverse, for the encoder and for
//! tests. Reserved bits are dropped on decoding and written as 0 on encoding
//! (REQ-PKT-009), so decoding an encoded function gives the same function.

use super::Triplet;
use crate::consts::{
    ATTRIBUTE_BOXING, ATTRIBUTE_CONCEAL, ATTRIBUTE_DOUBLE_HEIGHT, ATTRIBUTE_DOUBLE_WIDTH, ATTRIBUTE_INVERT,
    ATTRIBUTE_UNDERLINE, DRCS_CHARACTER_MASK, DRCS_LEVEL_MASK, DRCS_LEVEL_SHIFT, DRCS_NORMAL_BIT, DRCS_SUBTABLE_MASK,
    FLASH_MODE_MASK, FLASH_RATE_MASK, FLASH_RATE_SHIFT, FONT_BOLD, FONT_ITALIC, FONT_PROPORTIONAL, FONT_ROWS_MASK,
    FONT_ROWS_SHIFT, LEVEL_BIT_25, LEVEL_BIT_35, MODE_ADDITIONAL_FLASH, MODE_ADDRESS_ROW_0, MODE_BACKGROUND_COLOUR,
    MODE_COLUMN_PDC, MODE_DIACRITIC_MASK, MODE_DISPLAY_ATTRIBUTES, MODE_DRCS_CHARACTER, MODE_DRCS_MODE,
    MODE_FONT_STYLE, MODE_FOREGROUND_COLOUR, MODE_FULL_ROW_COLOUR, MODE_FULL_SCREEN_COLOUR, MODE_G0_LEVEL_25,
    MODE_G0_NO_DIACRITIC, MODE_G1, MODE_G2, MODE_G3_LEVEL_15, MODE_G3_LEVEL_25, MODE_MODIFIED_DESIGNATION,
    MODE_OBJECT_DEFINITION_FIRST, MODE_OBJECT_DEFINITION_LAST, MODE_OBJECT_INVOCATION_FIRST,
    MODE_OBJECT_INVOCATION_LAST, MODE_OBJECT_TYPE_MASK, MODE_ORIGIN_MODIFIER, MODE_ROW_PDC_FIRST, MODE_ROW_PDC_LAST,
    MODE_SET_ACTIVE_POSITION, MODE_TERMINATION, OBJECT_ADDRESS_BIT6, OBJECT_ADDRESS_N8_N7_MASK,
    OBJECT_NUMBER_HIGH_SHIFT, OBJECT_SOURCE_MASK, OBJECT_SOURCE_SHIFT, ROW_ADDRESS_BASE, ROW_ADDRESS_LAST,
    ROW_ADDRESS_RESERVED_DEFAULT, TERMINATION_TYPE_MASK, TRIPLET_COLOUR_HIGH_SHIFT, TRIPLET_COLOUR_INDEX_MASK,
    TRIPLET_DATA_FIELD_MASK,
};

/// A colour field of Tables 28 and 29 (decision E.43).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct ColourData {
    /// D4 to D0: the Colour Map index, 8 × CLUT + entry.
    pub index: u8,
    /// D6 and D5. They are 00 in the defined Foreground Colour, Background
    /// Colour and Full Screen Colour values; any other value is reserved.
    /// In Full Row Colour and Address Display Row 0 they are S1 S0, which
    /// choose the rows that are coloured.
    pub high: u8,
}

impl ColourData {
    const fn from_data(data: u8) -> Self {
        Self { index: data & TRIPLET_COLOUR_INDEX_MASK, high: data >> TRIPLET_COLOUR_HIGH_SHIFT }
    }

    const fn data(self) -> u8 {
        (self.index & TRIPLET_COLOUR_INDEX_MASK) | (self.high << TRIPLET_COLOUR_HIGH_SHIFT)
    }

    /// True when D6 and D5 are 00, the only defined value for Foreground
    /// Colour, Background Colour and Full Screen Colour.
    #[must_use]
    pub const fn is_defined(self) -> bool {
        self.high == 0
    }
}

/// The source of an invoked object: bits S1 S0 of the address (Table 28).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ObjectSource {
    /// 00, illegal. Address bit 6 is 1, so this value would put the triplet
    /// in the column address group; it is decoded for completeness.
    Illegal,
    /// 01, a Local Object defined in the page's own packets X/26.
    Local,
    /// 10, an object of the page's Public Object Page (POP).
    Public,
    /// 11, an object of the page's Global Public Object Page (GPOP).
    Global,
}

impl ObjectSource {
    const fn from_bits(bits: u8) -> Self {
        match bits & OBJECT_SOURCE_MASK {
            0b00 => Self::Illegal,
            0b01 => Self::Local,
            0b10 => Self::Public,
            _ => Self::Global,
        }
    }

    const fn bits(self) -> u8 {
        match self {
            Self::Illegal => 0b00,
            Self::Local => 0b01,
            Self::Public => 0b10,
            Self::Global => 0b11,
        }
    }
}

/// The type of an object: mode bits M1 M0 (Table 28).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ObjectKind {
    /// 01, an Active Object.
    Active,
    /// 10, an Adaptive Object.
    Adaptive,
    /// 11, a Passive Object.
    Passive,
}

impl ObjectKind {
    /// The type from bits M1 M0; 00 is not an object type.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits & MODE_OBJECT_TYPE_MASK {
            0b01 => Some(Self::Active),
            0b10 => Some(Self::Adaptive),
            0b11 => Some(Self::Passive),
            _ => None,
        }
    }

    /// Bits M1 M0.
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::Active => 0b01,
            Self::Adaptive => 0b10,
            Self::Passive => 0b11,
        }
    }
}

/// An object number, N8 to N0 (Table 28). Its bits mean different things
/// for local and for public or global objects, so both readings are given.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct ObjectNumber(pub u16);

/// Masks and shifts of the object number fields (Table 28, p.80).
const NUMBER_NIBBLE: u16 = 0x0F;
const NUMBER_N4_SHIFT: u32 = 4;
const NUMBER_N6_N5_SHIFT: u32 = 5;
const NUMBER_TWO_BITS: u16 = 0x03;
const NUMBER_N8_SHIFT: u32 = 8;
const NUMBER_N8_N7_SHIFT: u32 = 7;

impl ObjectNumber {
    /// N3 to N0 of a public or global object: the S1 sub-code nibble of the
    /// object page's sub-page that holds the definition.
    #[must_use]
    pub const fn subpage(self) -> u8 {
        (self.0 & NUMBER_NIBBLE) as u8
    }

    /// N8 N7 of a public or global object: which pointer packet, 0 to 3 for
    /// packets 1 to 4.
    #[must_use]
    pub const fn pointer_packet_index(self) -> u8 {
        ((self.0 >> NUMBER_N8_N7_SHIFT) & NUMBER_TWO_BITS) as u8
    }

    /// N6 N5 of a public or global object: the group of three pointer
    /// triplets, 0 to 3, so that the pointer triplet is 1 + 3 × group + k
    /// with k = 0 Active, 1 Adaptive, 2 Passive (REQ-OBJ-004).
    #[must_use]
    pub const fn pointer_group(self) -> u8 {
        ((self.0 >> NUMBER_N6_N5_SHIFT) & NUMBER_TWO_BITS) as u8
    }

    /// N4 of a public or global object: the pointer is in bits 10 to 18 of
    /// the pointer triplet when true, and in bits 1 to 9 when false.
    #[must_use]
    pub const fn pointer_odd(self) -> bool {
        (self.0 >> NUMBER_N4_SHIFT) & 1 == 1
    }

    /// N8 of a local object, which shall be 0 (Table 28).
    #[must_use]
    pub const fn n8(self) -> bool {
        (self.0 >> NUMBER_N8_SHIFT) & 1 == 1
    }

    /// N7 to N4 of a local object: the designation code of the X/26 packet
    /// that holds the definition.
    #[must_use]
    pub const fn designation(self) -> u8 {
        ((self.0 >> NUMBER_N4_SHIFT) & NUMBER_NIBBLE) as u8
    }

    /// N3 to N0 of a local object: the triplet within that packet, 0 to 12
    /// (13 to 15 address no triplet, decision E.51).
    #[must_use]
    pub const fn triplet(self) -> u8 {
        (self.0 & NUMBER_NIBBLE) as u8
    }

    /// The number from an address and data field: N8 N7 are address bits 2
    /// and 1, N6 to N0 the data.
    const fn from_fields(address: u8, data: u8) -> Self {
        Self(((address & OBJECT_ADDRESS_N8_N7_MASK) as u16) << OBJECT_NUMBER_HIGH_SHIFT | data as u16)
    }

    const fn data(self) -> u8 {
        (self.0 as u8) & TRIPLET_DATA_FIELD_MASK as u8
    }

    const fn high(self) -> u8 {
        (self.0 >> OBJECT_NUMBER_HIGH_SHIFT) as u8 & OBJECT_ADDRESS_N8_N7_MASK
    }
}

/// The usage of an object definition: bits S1 S0 of its address, the
/// presentation levels at which the object is required (Table 28).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct ObjectUsage(pub u8);

impl ObjectUsage {
    /// Bit S0: required at Level 2.5.
    #[must_use]
    pub const fn level_25(self) -> bool {
        self.0 & LEVEL_BIT_25 != 0
    }

    /// Bit S1: required at Level 3.5.
    #[must_use]
    pub const fn level_35(self) -> bool {
        self.0 & LEVEL_BIT_35 != 0
    }
}

/// The six functions of a Display Attributes triplet (Table 29, mode
/// 01100). D3 is reserved.
// Each flag is one bit of the data field, which Table 29 defines one by one.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct DisplayAttributes {
    /// D0: double height; with D6, double size.
    pub double_height: bool,
    /// D1: boxing, or a window on a page with neither C5 nor C6 set.
    pub boxing: bool,
    /// D2: conceal.
    pub conceal: bool,
    /// D4: invert colour.
    pub invert: bool,
    /// D5: underline, or separated mosaics.
    pub underline: bool,
    /// D6: double width; with D0, double size.
    pub double_width: bool,
}

impl DisplayAttributes {
    const fn from_data(data: u8) -> Self {
        Self {
            double_height: data & ATTRIBUTE_DOUBLE_HEIGHT != 0,
            boxing: data & ATTRIBUTE_BOXING != 0,
            conceal: data & ATTRIBUTE_CONCEAL != 0,
            invert: data & ATTRIBUTE_INVERT != 0,
            underline: data & ATTRIBUTE_UNDERLINE != 0,
            double_width: data & ATTRIBUTE_DOUBLE_WIDTH != 0,
        }
    }

    const fn data(self) -> u8 {
        flag(self.double_height, ATTRIBUTE_DOUBLE_HEIGHT)
            | flag(self.boxing, ATTRIBUTE_BOXING)
            | flag(self.conceal, ATTRIBUTE_CONCEAL)
            | flag(self.invert, ATTRIBUTE_INVERT)
            | flag(self.underline, ATTRIBUTE_UNDERLINE)
            | flag(self.double_width, ATTRIBUTE_DOUBLE_WIDTH)
    }
}

/// The Font Style triplet's data (Table 29, mode 01110). D3 is reserved.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct FontStyleData {
    /// D0: proportional spacing.
    pub proportional: bool,
    /// D1: bold.
    pub bold: bool,
    /// D2: italics.
    pub italic: bool,
    /// D6 to D4: the number of following rows the style extends over, 0
    /// to 7.
    pub rows: u8,
}

impl FontStyleData {
    const fn from_data(data: u8) -> Self {
        Self {
            proportional: data & FONT_PROPORTIONAL != 0,
            bold: data & FONT_BOLD != 0,
            italic: data & FONT_ITALIC != 0,
            rows: (data >> FONT_ROWS_SHIFT) & FONT_ROWS_MASK,
        }
    }

    const fn data(self) -> u8 {
        flag(self.proportional, FONT_PROPORTIONAL)
            | flag(self.bold, FONT_BOLD)
            | flag(self.italic, FONT_ITALIC)
            | (self.rows & FONT_ROWS_MASK) << FONT_ROWS_SHIFT
    }
}

const fn flag(on: bool, bit: u8) -> u8 {
    if on { bit } else { 0 }
}

/// What a triplet does: one variant for each row mode of Table 28 and each
/// column mode of Table 29. Row variants carry the address where it is a
/// row address; column variants carry the column address.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TripletFunction {
    /// Row mode 00000, Full Screen Colour. The address is reserved and does
    /// not move the active position.
    FullScreenColour(ColourData),
    /// Row mode 00001, Full Row Colour. `colour.high` is S1 S0: 00 this row
    /// only, 11 down to the bottom, 01 and 10 reserved.
    FullRowColour {
        /// The row address, 40 to 63.
        address: u8,
        /// The colour and row selection.
        colour: ColourData,
    },
    /// Row mode 00100, Set Active Position.
    SetActivePosition {
        /// The row address, 40 to 63.
        address: u8,
        /// The column, when below 40; larger values are reserved.
        column: u8,
    },
    /// Row mode 00111, Address Display Row 0. Only address 63 is defined.
    AddressDisplayRow0 {
        /// The address, 63 when defined.
        address: u8,
        /// The colour and row selection, as for Full Row Colour.
        colour: ColourData,
    },
    /// Row modes 01000 to 01101, Programme Delivery Control (Part G).
    ProgrammeDeliveryRow {
        /// The mode.
        mode: u8,
        /// The address.
        address: u8,
        /// The data.
        data: u8,
    },
    /// Row mode 10000, Origin Modifier.
    OriginModifier {
        /// The row offset, the address minus 40: 0 to 23.
        row_offset: u8,
        /// The column offset, the data: 0 to 71 are valid.
        column_offset: u8,
    },
    /// Row modes 10001 to 10011, Object Invocation.
    ObjectInvocation {
        /// Address bits S1 S0.
        source: ObjectSource,
        /// Mode bits M1 M0.
        kind: ObjectKind,
        /// N8 to N0.
        number: ObjectNumber,
    },
    /// Row modes 10101 to 10111, Object Definition.
    ObjectDefinition {
        /// Address bits S1 S0: the levels the object is for.
        usage: ObjectUsage,
        /// Mode bits M1 M0.
        kind: ObjectKind,
        /// N8 to N0, equal to those of the invoking triplet.
        number: ObjectNumber,
    },
    /// Row mode 11000, DRCS Mode. The address is reserved.
    DrcsMode {
        /// D6: normal DRCS when true, global DRCS when false.
        normal: bool,
        /// D5 D4, L1 L0: 01 Level 2.5, 10 Level 3.5, 11 both, 00 reserved.
        levels: u8,
        /// D3 to D0, T3 to T0: the sub-table, the S1 of the DRCS sub-page.
        subtable: u8,
    },
    /// Row mode 11111 with address 63, the Termination Marker.
    Termination {
        /// Data bits D2 to D0: the type of data ended (Table 28).
        kind: u8,
    },
    /// Any other row mode, or mode 11111 with an address other than 63.
    ReservedRow {
        /// The mode.
        mode: u8,
        /// The address.
        address: u8,
        /// The data.
        data: u8,
    },
    /// Column mode 00000, Foreground Colour.
    ForegroundColour {
        /// The column.
        column: u8,
        /// The colour.
        colour: ColourData,
    },
    /// Column mode 00001, a G1 block mosaic character.
    G1Character {
        /// The column.
        column: u8,
        /// The character code.
        code: u8,
    },
    /// Column modes 00010 and 01011, a G3 character.
    G3Character {
        /// The column.
        column: u8,
        /// The character code.
        code: u8,
        /// True for the Level 1.5 form, mode 00010; false for mode 01011,
        /// which Level 1.5 decoders ignore.
        level_15_form: bool,
    },
    /// Column mode 00011, Background Colour.
    BackgroundColour {
        /// The column.
        column: u8,
        /// The colour.
        colour: ColourData,
    },
    /// Column mode 00110, Programme Delivery Control; it does not move the
    /// active position.
    ProgrammeDeliveryColumn {
        /// The address.
        column: u8,
        /// The data.
        data: u8,
    },
    /// Column mode 00111, Additional Flash Functions.
    AdditionalFlash {
        /// The column.
        column: u8,
        /// D1 D0: 00 steady, 01 normal, 10 inverted phase, 11 adjacent CLUT.
        mode: u8,
        /// D4 to D2: 000 slow; 001 to 011 fast with phase 1 to 3; 100 fast
        /// incremental; 101 fast decremental; 110 and 111 reserved.
        rate: u8,
        /// D6 D5, which shall be 00; other values are reserved.
        reserved: u8,
    },
    /// Column mode 01000, Modified G0 and G2 Character Set Designation.
    ModifiedDesignation {
        /// The column.
        column: u8,
        /// D6 to D0, a Table 34 value.
        designation: u8,
    },
    /// Column mode 01001, a G0 character at Levels 2.5 and 3.5.
    G0Character {
        /// The column.
        column: u8,
        /// The character code.
        code: u8,
    },
    /// Column mode 01100, Display Attributes.
    DisplayAttributes {
        /// The column.
        column: u8,
        /// The six functions.
        attributes: DisplayAttributes,
    },
    /// Column mode 01101, DRCS Character Invocation.
    DrcsCharacter {
        /// The column.
        column: u8,
        /// D6: the normal table when true, the global table when false.
        normal: bool,
        /// D5 to D0: the character, 0 to 47; 48 to 63 are invalid.
        character: u8,
    },
    /// Column mode 01110, Font Style (Level 3.5).
    FontStyle {
        /// The column.
        column: u8,
        /// The styles and row count.
        style: FontStyleData,
    },
    /// Column mode 01111, a G2 character.
    G2Character {
        /// The column.
        column: u8,
        /// The character code.
        code: u8,
    },
    /// Column modes 10000 to 11111, a G0 character with the diacritical
    /// mark numbered by the low four bits of the mode (0 for none).
    G0Diacritic {
        /// The column.
        column: u8,
        /// The character code.
        code: u8,
        /// The mark, 0 to 15.
        diacritic: u8,
    },
    /// Column modes 00100, 00101 and 01010, reserved; they do not move the
    /// active position.
    ReservedColumn {
        /// The address.
        column: u8,
        /// The mode.
        mode: u8,
        /// The data.
        data: u8,
    },
}

impl Triplet {
    /// The triplet's function (REQ-PKT-010).
    #[must_use]
    pub const fn function(self) -> TripletFunction {
        if self.is_row_address() { row_function(self) } else { column_function(self) }
    }
}

/// Table 28.
const fn row_function(triplet: Triplet) -> TripletFunction {
    use TripletFunction as F;
    let Triplet { address, mode, data } = triplet;
    match mode {
        MODE_FULL_SCREEN_COLOUR => F::FullScreenColour(ColourData::from_data(data)),
        MODE_FULL_ROW_COLOUR => F::FullRowColour { address, colour: ColourData::from_data(data) },
        MODE_SET_ACTIVE_POSITION => F::SetActivePosition { address, column: data },
        MODE_ADDRESS_ROW_0 => F::AddressDisplayRow0 { address, colour: ColourData::from_data(data) },
        MODE_ROW_PDC_FIRST..=MODE_ROW_PDC_LAST => F::ProgrammeDeliveryRow { mode, address, data },
        MODE_ORIGIN_MODIFIER => {
            F::OriginModifier { row_offset: address.wrapping_sub(ROW_ADDRESS_BASE), column_offset: data }
        }
        MODE_OBJECT_INVOCATION_FIRST..=MODE_OBJECT_INVOCATION_LAST => match ObjectKind::from_bits(mode) {
            Some(kind) => F::ObjectInvocation {
                source: ObjectSource::from_bits(address >> OBJECT_SOURCE_SHIFT),
                kind,
                number: ObjectNumber::from_fields(address, data),
            },
            None => F::ReservedRow { mode, address, data },
        },
        MODE_OBJECT_DEFINITION_FIRST..=MODE_OBJECT_DEFINITION_LAST => match ObjectKind::from_bits(mode) {
            Some(kind) => F::ObjectDefinition {
                usage: ObjectUsage((address >> OBJECT_SOURCE_SHIFT) & OBJECT_SOURCE_MASK),
                kind,
                number: ObjectNumber::from_fields(address, data),
            },
            None => F::ReservedRow { mode, address, data },
        },
        MODE_DRCS_MODE => F::DrcsMode {
            normal: data & DRCS_NORMAL_BIT != 0,
            levels: (data >> DRCS_LEVEL_SHIFT) & DRCS_LEVEL_MASK,
            subtable: data & DRCS_SUBTABLE_MASK,
        },
        MODE_TERMINATION if address == ROW_ADDRESS_LAST => F::Termination { kind: data & TERMINATION_TYPE_MASK },
        _ => F::ReservedRow { mode, address, data },
    }
}

/// Table 29.
const fn column_function(triplet: Triplet) -> TripletFunction {
    use TripletFunction as F;
    let Triplet { address: column, mode, data } = triplet;
    match mode {
        MODE_FOREGROUND_COLOUR => F::ForegroundColour { column, colour: ColourData::from_data(data) },
        MODE_G1 => F::G1Character { column, code: data },
        MODE_G3_LEVEL_15 => F::G3Character { column, code: data, level_15_form: true },
        MODE_BACKGROUND_COLOUR => F::BackgroundColour { column, colour: ColourData::from_data(data) },
        MODE_COLUMN_PDC => F::ProgrammeDeliveryColumn { column, data },
        MODE_ADDITIONAL_FLASH => F::AdditionalFlash {
            column,
            mode: data & FLASH_MODE_MASK,
            rate: (data >> FLASH_RATE_SHIFT) & FLASH_RATE_MASK,
            reserved: data >> TRIPLET_COLOUR_HIGH_SHIFT,
        },
        MODE_MODIFIED_DESIGNATION => F::ModifiedDesignation { column, designation: data },
        MODE_G0_LEVEL_25 => F::G0Character { column, code: data },
        MODE_G3_LEVEL_25 => F::G3Character { column, code: data, level_15_form: false },
        MODE_DISPLAY_ATTRIBUTES => F::DisplayAttributes { column, attributes: DisplayAttributes::from_data(data) },
        MODE_DRCS_CHARACTER => {
            F::DrcsCharacter { column, normal: data & DRCS_NORMAL_BIT != 0, character: data & DRCS_CHARACTER_MASK }
        }
        MODE_FONT_STYLE => F::FontStyle { column, style: FontStyleData::from_data(data) },
        MODE_G2 => F::G2Character { column, code: data },
        MODE_G0_NO_DIACRITIC..=MODE_TERMINATION => {
            F::G0Diacritic { column, code: data, diacritic: mode & MODE_DIACRITIC_MASK }
        }
        _ => F::ReservedColumn { column, mode, data },
    }
}

impl TripletFunction {
    /// The triplet that carries this function: the encoder's inverse of
    /// [`Triplet::function`]. Reserved bits are written as 0, and reserved
    /// address fields as row address 41 (Annex A.5 note 1).
    #[must_use]
    pub const fn to_triplet(self) -> Triplet {
        let (address, mode, data) = match self {
            Self::FullScreenColour(colour) => (ROW_ADDRESS_RESERVED_DEFAULT, MODE_FULL_SCREEN_COLOUR, colour.data()),
            Self::FullRowColour { address, colour } => (address, MODE_FULL_ROW_COLOUR, colour.data()),
            Self::SetActivePosition { address, column } => (address, MODE_SET_ACTIVE_POSITION, column),
            Self::AddressDisplayRow0 { address, colour } => (address, MODE_ADDRESS_ROW_0, colour.data()),
            Self::ProgrammeDeliveryRow { mode, address, data }
            | Self::ReservedRow { mode, address, data }
            | Self::ReservedColumn { column: address, mode, data } => (address, mode, data),
            Self::OriginModifier { row_offset, column_offset } => {
                (ROW_ADDRESS_BASE.wrapping_add(row_offset), MODE_ORIGIN_MODIFIER, column_offset)
            }
            Self::ObjectInvocation { source, kind, number } => (
                object_address(source.bits(), number),
                (MODE_OBJECT_INVOCATION_FIRST & !MODE_OBJECT_TYPE_MASK) | kind.bits(),
                number.data(),
            ),
            Self::ObjectDefinition { usage, kind, number } => (
                object_address(usage.0, number),
                (MODE_OBJECT_DEFINITION_FIRST & !MODE_OBJECT_TYPE_MASK) | kind.bits(),
                number.data(),
            ),
            Self::DrcsMode { normal, levels, subtable } => (
                ROW_ADDRESS_RESERVED_DEFAULT,
                MODE_DRCS_MODE,
                flag(normal, DRCS_NORMAL_BIT)
                    | (levels & DRCS_LEVEL_MASK) << DRCS_LEVEL_SHIFT
                    | (subtable & DRCS_SUBTABLE_MASK),
            ),
            Self::Termination { kind } => (ROW_ADDRESS_LAST, MODE_TERMINATION, kind & TERMINATION_TYPE_MASK),
            Self::ForegroundColour { column, colour } => (column, MODE_FOREGROUND_COLOUR, colour.data()),
            Self::G1Character { column, code } => (column, MODE_G1, code),
            Self::G3Character { column, code, level_15_form } => {
                (column, if level_15_form { MODE_G3_LEVEL_15 } else { MODE_G3_LEVEL_25 }, code)
            }
            Self::BackgroundColour { column, colour } => (column, MODE_BACKGROUND_COLOUR, colour.data()),
            Self::ProgrammeDeliveryColumn { column, data } => (column, MODE_COLUMN_PDC, data),
            Self::AdditionalFlash { column, mode, rate, reserved } => (
                column,
                MODE_ADDITIONAL_FLASH,
                (mode & FLASH_MODE_MASK)
                    | (rate & FLASH_RATE_MASK) << FLASH_RATE_SHIFT
                    | reserved << TRIPLET_COLOUR_HIGH_SHIFT,
            ),
            Self::ModifiedDesignation { column, designation } => (column, MODE_MODIFIED_DESIGNATION, designation),
            Self::G0Character { column, code } => (column, MODE_G0_LEVEL_25, code),
            Self::DisplayAttributes { column, attributes } => (column, MODE_DISPLAY_ATTRIBUTES, attributes.data()),
            Self::DrcsCharacter { column, normal, character } => {
                (column, MODE_DRCS_CHARACTER, flag(normal, DRCS_NORMAL_BIT) | (character & DRCS_CHARACTER_MASK))
            }
            Self::FontStyle { column, style } => (column, MODE_FONT_STYLE, style.data()),
            Self::G2Character { column, code } => (column, MODE_G2, code),
            Self::G0Diacritic { column, code, diacritic } => {
                (column, MODE_G0_NO_DIACRITIC | (diacritic & MODE_DIACRITIC_MASK), code)
            }
        };
        Triplet { address, mode, data }
    }
}

impl TripletFunction {
    /// The column a column triplet moves the active position to: every
    /// column mode except the reserved modes and Programme Delivery Control
    /// (clause 12.3.4). `None` for row triplets and for those column modes.
    #[must_use]
    pub const fn active_column(self) -> Option<u8> {
        match self {
            Self::ForegroundColour { column, .. }
            | Self::G1Character { column, .. }
            | Self::G3Character { column, .. }
            | Self::BackgroundColour { column, .. }
            | Self::AdditionalFlash { column, .. }
            | Self::ModifiedDesignation { column, .. }
            | Self::G0Character { column, .. }
            | Self::DisplayAttributes { column, .. }
            | Self::DrcsCharacter { column, .. }
            | Self::FontStyle { column, .. }
            | Self::G2Character { column, .. }
            | Self::G0Diacritic { column, .. } => Some(column),
            Self::ProgrammeDeliveryColumn { .. }
            | Self::ReservedColumn { .. }
            | Self::FullScreenColour(_)
            | Self::FullRowColour { .. }
            | Self::SetActivePosition { .. }
            | Self::AddressDisplayRow0 { .. }
            | Self::ProgrammeDeliveryRow { .. }
            | Self::OriginModifier { .. }
            | Self::ObjectInvocation { .. }
            | Self::ObjectDefinition { .. }
            | Self::DrcsMode { .. }
            | Self::Termination { .. }
            | Self::ReservedRow { .. } => None,
        }
    }
}

/// The address of an Object Invocation or Definition: bits 6 to 1 are 1,
/// S1, S0, X (reserved, written 0), N8, N7 (Table 28).
const fn object_address(source_bits: u8, number: ObjectNumber) -> u8 {
    OBJECT_ADDRESS_BIT6 | (source_bits & OBJECT_SOURCE_MASK) << OBJECT_SOURCE_SHIFT | number.high()
}
