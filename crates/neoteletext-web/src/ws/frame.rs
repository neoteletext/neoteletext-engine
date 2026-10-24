//! Client frames (RFC 6455 section 5.2): masked with keys from the
//! caller's random source, never fragmented, and the close frame.

use crate::bytes::{Overflow, Writer};
use crate::consts::{
    OPCODE_CLOSE, WS_FIN, WS_LENGTH_7_MAX, WS_LENGTH_16, WS_LENGTH_64, WS_MASK_BIT, WS_MASK_BYTES, WS_OPCODE_MASK,
};
use crate::port::RandomSource;

/// Writes one complete, masked client frame (RFC 6455 section 5.2). The
/// client never fragments what it sends.
pub fn write_frame(
    writer: &mut Writer<'_>,
    opcode: u8,
    payload: &[u8],
    random: &mut dyn RandomSource,
) -> Result<(), Overflow> {
    let mut mask = [0u8; WS_MASK_BYTES];
    random.fill(&mut mask);
    write_frame_with_mask(writer, opcode, payload, mask)
}

/// Writes a frame with a given masking key.
pub fn write_frame_with_mask(
    writer: &mut Writer<'_>,
    opcode: u8,
    payload: &[u8],
    mask: [u8; WS_MASK_BYTES],
) -> Result<(), Overflow> {
    let start = writer.len();
    let result = (|| {
        writer.byte(WS_FIN | (opcode & WS_OPCODE_MASK))?;
        let len = payload.len();
        if len <= WS_LENGTH_7_MAX {
            writer.byte(WS_MASK_BIT | len as u8)?;
        } else if let Ok(short) = u16::try_from(len) {
            writer.byte(WS_MASK_BIT | WS_LENGTH_16)?;
            writer.put(&short.to_be_bytes())?;
        } else {
            writer.byte(WS_MASK_BIT | WS_LENGTH_64)?;
            writer.put(&(len as u64).to_be_bytes())?;
        }
        writer.put(&mask)?;
        for (index, &byte) in payload.iter().enumerate() {
            writer.byte(byte ^ mask.get(index % WS_MASK_BYTES).copied().unwrap_or(0))?;
        }
        Ok(())
    })();
    if result.is_err() {
        writer.truncate(start);
    }
    result
}

/// Writes a close frame with a status code (RFC 6455 section 5.5.1).
pub fn write_close(writer: &mut Writer<'_>, code: u16, random: &mut dyn RandomSource) -> Result<(), Overflow> {
    write_frame(writer, OPCODE_CLOSE, &code.to_be_bytes(), random)
}
