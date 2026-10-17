//! The platform interface: the whole of a port (REQ-ARCH-009, layer L8).
//!
//! A port supplies only these: a data source, a time source, key input and,
//! optionally, a sleep function. Pixels are pulled from the row renderer
//! into the port's own buffers (REQ-RND-017); the network transport and
//! cookie storage belong to the web crate's half of this interface.
//!
//! This is the boundary of REQ-ARCH-010: the library's pixel output ends
//! when the row renderer has filled the port's buffer. Display drivers,
//! scan-out, buffering strategy and timing stay on the port's side, so
//! nothing here names a device, a display or a video standard.
//! The porting guide is `docs/spec/design/porting.md`.

use crate::types::Instant;

/// Input bytes in any format of area INP (REQ-INP-001). Modelled on the
/// `embedded-io` `Read` trait so existing implementations adapt directly.
pub trait DataSource {
    /// The source's error type.
    type Error;

    /// Reads up to `buffer.len()` bytes. `Ok(0)` means the end of input.
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error>;
}

/// A monotonic clock in microseconds (REQ-GEN-005).
pub trait Clock {
    /// The current time.
    fn now(&self) -> Instant;
}

/// Viewer key input: the numbered functions of area NAV.
pub trait Keys {
    /// The key type, normally `neoteletext_core::viewer::Key`.
    type Key;

    /// The next key press, if any.
    fn poll(&mut self) -> Option<Self::Key>;
}

/// An optional function for platforms that must yield.
pub trait Sleep {
    /// Sleeps for about `microseconds`.
    fn sleep(&mut self, microseconds: u64);
}
