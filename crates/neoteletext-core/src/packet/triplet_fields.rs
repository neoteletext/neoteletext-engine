//! The fields of Tables 28 and 29 that triplet functions carry
//! (REQ-PKT-010): colours, object sources, kinds, numbers and usage, and
//! display attributes. Reserved bits are dropped on decoding and written
//! as 0 on encoding (REQ-PKT-009).

use crate::consts::{
    ATTRIBUTE_BOXING, ATTRIBUTE_CONCEAL, ATTRIBUTE_DOUBLE_HEIGHT, ATTRIBUTE_DOUBLE_WIDTH, ATTRIBUTE_INVERT,
    ATTRIBUTE_UNDERLINE, LEVEL_BIT_25, LEVEL_BIT_35, MODE_OBJECT_TYPE_MASK, OBJECT_ADDRESS_N8_N7_MASK,
    OBJECT_NUMBER_HIGH_SHIFT, OBJECT_SOURCE_MASK, TRIPLET_COLOUR_HIGH_SHIFT, TRIPLET_COLOUR_INDEX_MASK,
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
    pub(super) const fn from_data(data: u8) -> Self {
        Self { index: data & TRIPLET_COLOUR_INDEX_MASK, high: data >> TRIPLET_COLOUR_HIGH_SHIFT }
    }

    pub(super) const fn data(self) -> u8 {
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
    pub(super) const fn from_bits(bits: u8) -> Self {
        match bits & OBJECT_SOURCE_MASK {
            0b00 => Self::Illegal,
            0b01 => Self::Local,
            0b10 => Self::Public,
            _ => Self::Global,
        }
    }

    pub(super) const fn bits(self) -> u8 {
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
    pub(super) const fn from_fields(address: u8, data: u8) -> Self {
        Self(((address & OBJECT_ADDRESS_N8_N7_MASK) as u16) << OBJECT_NUMBER_HIGH_SHIFT | data as u16)
    }

    pub(super) const fn data(self) -> u8 {
        (self.0 as u8) & TRIPLET_DATA_FIELD_MASK as u8
    }

    pub(super) const fn high(self) -> u8 {
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
    pub(super) const fn from_data(data: u8) -> Self {
        Self {
            double_height: data & ATTRIBUTE_DOUBLE_HEIGHT != 0,
            boxing: data & ATTRIBUTE_BOXING != 0,
            conceal: data & ATTRIBUTE_CONCEAL != 0,
            invert: data & ATTRIBUTE_INVERT != 0,
            underline: data & ATTRIBUTE_UNDERLINE != 0,
            double_width: data & ATTRIBUTE_DOUBLE_WIDTH != 0,
        }
    }

    pub(super) const fn data(self) -> u8 {
        flag(self.double_height, ATTRIBUTE_DOUBLE_HEIGHT)
            | flag(self.boxing, ATTRIBUTE_BOXING)
            | flag(self.conceal, ATTRIBUTE_CONCEAL)
            | flag(self.invert, ATTRIBUTE_INVERT)
            | flag(self.underline, ATTRIBUTE_UNDERLINE)
            | flag(self.double_width, ATTRIBUTE_DOUBLE_WIDTH)
    }
}

const fn flag(on: bool, bit: u8) -> u8 {
    if on { bit } else { 0 }
}
