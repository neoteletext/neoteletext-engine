//! Output adapters for tellytxt (layer L5): colour palettes, pixel-format
//! conversion (REQ-RND-018), scaling (REQ-RND-024), and the PNG and PPM
//! writers (REQ-RND-007).
//!
//! Everything here is `no_std` and allocation-free. Writers emit bytes to a
//! [`Sink`]; with the `std` feature any `std::io::Write` is a sink.
#![no_std]
#![forbid(unsafe_code)]
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

#[cfg(any(test, feature = "std"))]
extern crate std;

pub mod convert;
pub mod palette;
pub mod png;
pub mod ppm;
pub mod scale;

/// A destination for encoded bytes.
pub trait Sink {
    /// The sink's error type.
    type Error;

    /// Writes all of `bytes`.
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;
}

#[cfg(feature = "std")]
impl<W: std::io::Write> Sink for W {
    type Error = std::io::Error;

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        std::io::Write::write_all(self, bytes)
    }
}

/// A sink that writes into a fixed slice.
#[derive(Debug)]
pub struct SliceSink<'a> {
    buffer: &'a mut [u8],
    length: usize,
}

/// The slice was too small.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Overflow;

impl<'a> SliceSink<'a> {
    /// A sink over `buffer`.
    pub fn new(buffer: &'a mut [u8]) -> Self {
        Self { buffer, length: 0 }
    }

    /// The bytes written so far.
    #[must_use]
    pub fn written(&self) -> &[u8] {
        self.buffer.get(..self.length).unwrap_or(&[])
    }
}

impl Sink for SliceSink<'_> {
    type Error = Overflow;

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Overflow> {
        let end = self.length.checked_add(bytes.len()).ok_or(Overflow)?;
        let target = self.buffer.get_mut(self.length..end).ok_or(Overflow)?;
        target.copy_from_slice(bytes);
        self.length = end;
        Ok(())
    }
}
