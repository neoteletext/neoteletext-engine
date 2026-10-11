//! Output adapters for neoteletext (layer L5): colour palettes, pixel-format
//! conversion (REQ-RND-018), scaling (REQ-RND-024), and the PNG and PPM
//! writers (REQ-RND-007).
//!
//! The writers are reference callers of the row renderer (REQ-RND-021):
//! they take the palette indices it drew from the cell grid and never see
//! packets (REQ-RND-001), and they write files, never a screen.
//!
//! Everything here is `no_std` and allocation-free. Writers emit bytes to a
//! [`Sink`]; with the `std` feature any `std::io::Write` is a sink.
#![no_std]
// REQ-SEC-015: only the C interface crate may contain unsafe code.
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

/// REQ-GEN-011: the error types implement `core::error::Error`. Their
/// `Display` text is their `Debug` text, and both need `core::fmt`, which
/// REQ-GEN-016 keeps behind this optional feature so that a small build
/// leaves it out.
#[cfg(feature = "error-trait")]
mod error_trait {
    macro_rules! error_trait {
        ($($error:ty),+ $(,)?) => {$(
            impl core::fmt::Display for $error {
                fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    core::fmt::Debug::fmt(self, formatter)
                }
            }
            impl core::error::Error for $error {}
        )+};
    }
    error_trait!(crate::convert::ConvertError, crate::Overflow, crate::scale::ScaleError);

    impl<E: core::fmt::Debug> core::fmt::Display for crate::png::PngError<E> {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            core::fmt::Debug::fmt(self, formatter)
        }
    }
    impl<E: core::fmt::Debug> core::error::Error for crate::png::PngError<E> {}

    #[cfg(test)]
    mod tests {
        const fn is_error<T: core::error::Error>() {}

        #[test]
        fn every_error_type_is_an_error() {
            is_error::<crate::convert::ConvertError>();
            is_error::<crate::Overflow>();
            is_error::<crate::scale::ScaleError>();
            is_error::<crate::png::PngError<crate::Overflow>>();
        }
    }
}
