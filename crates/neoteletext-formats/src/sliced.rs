//! Linux sliced VBI records (REQ-INP-010): 64-byte records of four 32-bit
//! fields (service identifier, field, line, reserved) and 48 data bytes, as
//! in V4L2's `struct v4l2_sliced_vbi_data`.
//!
//! [`ByteOrder`] is always compiled; the reader needs the `sliced` feature
//! (decision E.61).

#[cfg(feature = "sliced")]
mod reader;

#[cfg(feature = "sliced")]
pub use reader::{SlicedReader, record_packet};

/// Byte order of the 32-bit fields, which is that of the computer that
/// wrote the file.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum ByteOrder {
    /// Little-endian, the default (REQ-INP-010).
    #[default]
    Little,
    /// Big-endian.
    Big,
}

impl ByteOrder {
    /// Reads a 32-bit field at `offset` of a record, in the byte order the
    /// format names rather than the host's (REQ-GEN-007).
    #[must_use]
    pub fn word(self, record: &[u8], offset: usize) -> Option<u32> {
        let bytes: [u8; 4] = record.get(offset..offset.saturating_add(4))?.try_into().ok()?;
        Some(match self {
            Self::Little => u32::from_le_bytes(bytes),
            Self::Big => u32::from_be_bytes(bytes),
        })
    }
}
