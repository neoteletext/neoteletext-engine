//! Independent data lines and low bit-rate audio (ETSI EN 300 708 clauses
//! 6.4 and 6.7, Tables 16 and 21). The audio profile of area AUD reads them;
//! the decoder itself still ignores packets 30 outside magazine 8 and 31.

use super::signal::{DATA_START, PACKET_BYTES};

/// The designation code of an independent data line, byte 5 (EN 300 708
/// clause 6.4.1): with it, the packet address is 30 or 31.
pub const IDL_DESIGNATION: u8 = 0b1111;
/// Data channel 4: programme-related audio, such as Audetel (EN 300 708
/// Table 16 and clause 6.7.2).
pub const IDL_CHANNEL_PROGRAMME_AUDIO: u8 = 4;
/// Data channel 12: programme-independent audio (EN 300 708 Table 16 and
/// clause 6.7.3), the channel the audio profile uses.
pub const IDL_CHANNEL_INDEPENDENT_AUDIO: u8 = 12;
/// The Service Byte, packet byte 6, in the internal form (EN 300 708
/// clause 6.7.3.1).
pub const IDL_AUDIO_SERVICE_BYTE: usize = DATA_START;
/// The Control Byte, packet byte 7 (EN 300 708 clause 6.7.3.2).
pub const IDL_AUDIO_CONTROL_BYTE: usize = DATA_START + 1;
/// The first audio data byte, packet byte 8 (EN 300 708 clause 6.7.3.3).
pub const IDL_AUDIO_DATA_START: usize = DATA_START + 2;
/// Audio data bytes in one packet, bytes 8 to 45 (EN 300 708 clause
/// 6.7.3.1: 38 bytes in single-channel mode).
pub const IDL_AUDIO_DATA_BYTES: usize = PACKET_BYTES - IDL_AUDIO_DATA_START;
/// Service Byte message bit 4: a user-defined service, whose other bits and
/// whose Control Byte are the application's (EN 300 708 Table 21).
pub const IDL_AUDIO_USER_DEFINED: u8 = 0b1000;
/// Service Byte message bits 1 to 3 of a user-defined service.
pub const IDL_AUDIO_SERVICE_MASK: u8 = 0b0111;
