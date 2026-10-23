//! Signal and packet framing (clauses 5 to 7).

/// Bit rate of System B teletext, 444 × line frequency (clause 5.3).
pub const BIT_RATE_HZ: u32 = 6_937_500;
/// Clock run-in byte, 10101010 in transmission order (clause 6.1).
pub const CLOCK_RUN_IN: u8 = 0x55;
/// Framing code 11100100 in transmission order, first bit as bit 0
/// (clause 6.2). This is the T42 form.
pub const FRAMING_CODE: u8 = 0x27;
/// The framing code in the bit order of EN 300 472 clause 4.4, which is the
/// bit-reversal of [`FRAMING_CODE`].
pub const FRAMING_CODE_DVB: u8 = 0xE4;
/// The framing code of inverted teletext data units (EN 300 472 Table 3).
pub const FRAMING_CODE_DVB_INVERTED: u8 = 0x1B;
/// Bits in a packet after the framing code (clause 7.1): 42 bytes.
pub const PACKET_BITS: usize = 336;
/// Bit periods in one television line: the bit rate is 444 × the line
/// frequency (clause 5.3).
pub const BITS_PER_LINE: u32 = 444;
/// The "1" level as a percentage of black-to-peak-white (clause 5.2:
/// 66 ± 6 percent).
pub const ONE_LEVEL_PERCENT_OF_WHITE: u32 = 66;
/// Leading run-in bits that may be "absent or reduced in amplitude"
/// (clause 6.1, note).
pub const RUN_IN_UNRELIABLE_BITS: usize = 2;
/// Nominal time from the half-amplitude point of the line synchronising
/// pulse's leading edge to the centre of the timing reference bit, in
/// nanoseconds (clause 6.3, Figure 4: 12.0 µs).
pub const TIMING_REFERENCE_NS: u32 = 12_000;
/// How much earlier than nominal the timing reference may fall (Figure 4:
/// −1.0 µs).
pub const TIMING_EARLY_NS: u32 = 1_000;
/// How much later than nominal the timing reference may fall (Figure 4:
/// +0.4 µs).
pub const TIMING_LATE_NS: u32 = 400;
/// Bytes of a packet after the framing code: bytes 4 to 45 (clause 7.1).
pub const PACKET_BYTES: usize = 42;
/// Bytes per packet with the framing code in front (REQ-INP-003).
pub const PACKET_BYTES_WITH_FRAMING: usize = 43;
/// Bytes per packet with clock run-in and framing code (REQ-INP-003).
pub const PACKET_BYTES_WITH_RUN_IN: usize = 45;
/// The transmitted byte number of the first byte of the internal packet
/// form (REQ-ARCH-002): internal index = packet byte number − 4.
pub const PACKET_FIRST_BYTE_NUMBER: usize = 4;
/// Index of the first packet address byte (packet byte 4).
pub const ADDRESS_BYTE_0: usize = 0;
/// Index of the second packet address byte (packet byte 5).
pub const ADDRESS_BYTE_1: usize = 1;
/// Index of the first data byte, packet byte 6.
pub const DATA_START: usize = 2;
/// Data bytes in a packet: bytes 6 to 45 (clause 7.1.3).
pub const DATA_BYTES: usize = 40;
