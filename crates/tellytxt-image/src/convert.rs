//! Pixel-format helpers (REQ-RND-018): a row of palette indices to RGB565,
//! RGB888, RGBA8888, 8-bit grey or 1-bit, in either byte order, at any
//! stride, rotated by 0, 90, 180 or 270 degrees.

use crate::palette::Rgba;

/// Output pixel formats.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PixelFormat {
    /// 16-bit 5-6-5.
    Rgb565,
    /// 24-bit.
    Rgb888,
    /// 32-bit with alpha.
    Rgba8888,
    /// 8-bit grey.
    Grey8,
    /// 1-bit black and white, most significant bit leftmost.
    Mono1,
}

impl PixelFormat {
    /// Bits per pixel.
    #[must_use]
    pub const fn bits(self) -> usize {
        match self {
            Self::Rgb565 => 16,
            Self::Rgb888 => 24,
            Self::Rgba8888 => 32,
            Self::Grey8 => 8,
            Self::Mono1 => 1,
        }
    }
}

/// Byte order of multi-byte pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ByteOrder {
    /// Least significant byte first; for RGB888 and RGBA8888, red first.
    Little,
    /// Most significant byte first; for RGB888 and RGBA8888, the reverse.
    Big,
}

/// Rotation of the output image.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rotation {
    /// None.
    None,
    /// 90 degrees clockwise.
    Clockwise90,
    /// 180 degrees.
    Half,
    /// 270 degrees clockwise.
    Clockwise270,
}

/// Errors from conversion.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConvertError {
    /// The target buffer is too small.
    BufferTooSmall,
}

/// Luma weights of ITU-R BT.601 in parts of 256, for grey output.
const LUMA_R: u32 = 77;
const LUMA_G: u32 = 150;
const LUMA_B: u32 = 29;
/// Grey level at or above which a 1-bit pixel is white.
const MONO_THRESHOLD: u8 = 0x80;

fn grey(colour: Rgba) -> u8 {
    let weighted = LUMA_R
        .saturating_mul(u32::from(colour.r))
        .saturating_add(LUMA_G.saturating_mul(u32::from(colour.g)))
        .saturating_add(LUMA_B.saturating_mul(u32::from(colour.b)));
    (weighted >> 8).min(0xFF) as u8
}

/// Writes one pixel's bytes into `out` at bit offset `bit` (bytes are
/// byte-aligned except for 1-bit).
fn put(out: &mut [u8], bit: usize, colour: Rgba, format: PixelFormat, order: ByteOrder) -> Result<(), ConvertError> {
    let byte = bit / 8;
    let mut write = |offset: usize, value: u8| -> Result<(), ConvertError> {
        let slot = out.get_mut(byte.saturating_add(offset)).ok_or(ConvertError::BufferTooSmall)?;
        *slot = value;
        Ok(())
    };
    match format {
        PixelFormat::Rgb565 => {
            let value = (u16::from(colour.r >> 3) << 11) | (u16::from(colour.g >> 2) << 5) | u16::from(colour.b >> 3);
            let bytes = match order {
                ByteOrder::Little => value.to_le_bytes(),
                ByteOrder::Big => value.to_be_bytes(),
            };
            for (offset, value) in bytes.into_iter().enumerate() {
                write(offset, value)?;
            }
        }
        PixelFormat::Rgb888 => {
            let bytes = match order {
                ByteOrder::Little => [colour.r, colour.g, colour.b],
                ByteOrder::Big => [colour.b, colour.g, colour.r],
            };
            for (offset, value) in bytes.into_iter().enumerate() {
                write(offset, value)?;
            }
        }
        PixelFormat::Rgba8888 => {
            let bytes = match order {
                ByteOrder::Little => [colour.r, colour.g, colour.b, colour.a],
                ByteOrder::Big => [colour.a, colour.b, colour.g, colour.r],
            };
            for (offset, value) in bytes.into_iter().enumerate() {
                write(offset, value)?;
            }
        }
        PixelFormat::Grey8 => write(0, grey(colour))?,
        PixelFormat::Mono1 => {
            let slot = out.get_mut(byte).ok_or(ConvertError::BufferTooSmall)?;
            let mask = 0x80u8 >> (bit % 8);
            if grey(colour) >= MONO_THRESHOLD { *slot |= mask } else { *slot &= !mask }
        }
    }
    Ok(())
}

/// Converts a `width` by `height` image of palette indices (`stride` bytes
/// per row) into `out` (`out_stride` bytes per output row), rotated.
/// `palette` is the 144 colours of the palette mode or the 32 of a Colour
/// Map (REQ-RND-030); an index beyond it is transparent.
#[allow(clippy::too_many_arguments)] // Each argument is a separate geometry or format parameter of REQ-RND-018.
pub fn convert(
    indices: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    palette: &[Rgba],
    format: PixelFormat,
    order: ByteOrder,
    rotation: Rotation,
    out: &mut [u8],
    out_stride: usize,
) -> Result<(), ConvertError> {
    for y in 0..height {
        for x in 0..width {
            let index = indices.get(y.saturating_mul(stride).saturating_add(x)).copied().unwrap_or(0);
            let colour = palette.get(usize::from(index)).copied().unwrap_or(Rgba::TRANSPARENT);
            let (ox, oy) = match rotation {
                Rotation::None => (x, y),
                Rotation::Clockwise90 => (height.saturating_sub(1).saturating_sub(y), x),
                Rotation::Half => {
                    (width.saturating_sub(1).saturating_sub(x), height.saturating_sub(1).saturating_sub(y))
                }
                Rotation::Clockwise270 => (y, width.saturating_sub(1).saturating_sub(x)),
            };
            let bit = oy.saturating_mul(out_stride).saturating_mul(8).saturating_add(ox.saturating_mul(format.bits()));
            put(out, bit, colour, format, order)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::palette::{CLUT0, palette};

    #[test]
    fn formats_and_rotation() {
        let colours = palette(&CLUT0, Rgba::TRANSPARENT, true, false);
        // Two pixels: red then white.
        let indices = [1u8, 7];
        let mut out = [0u8; 8];
        convert(&indices, 2, 1, 2, &colours, PixelFormat::Rgb565, ByteOrder::Big, Rotation::None, &mut out, 4).unwrap();
        assert_eq!(&out[..4], &[0xF8, 0x00, 0xFF, 0xFF]);
        let mut rotated = [0u8; 6];
        convert(
            &indices,
            2,
            1,
            2,
            &colours,
            PixelFormat::Rgb888,
            ByteOrder::Little,
            Rotation::Clockwise90,
            &mut rotated,
            3,
        )
        .unwrap();
        assert_eq!(&rotated, &[255, 0, 0, 255, 255, 255]);
        let mut mono = [0u8; 1];
        convert(&indices, 2, 1, 2, &colours, PixelFormat::Mono1, ByteOrder::Little, Rotation::None, &mut mono, 1)
            .unwrap();
        assert_eq!(mono[0], 0b0100_0000);
        let mut small = [0u8; 2];
        assert!(
            convert(
                &indices,
                2,
                1,
                2,
                &colours,
                PixelFormat::Rgba8888,
                ByteOrder::Little,
                Rotation::None,
                &mut small,
                8
            )
            .is_err()
        );
    }
}
