//! Raster output: palette-index framebuffers, PNG and PPM files
//! (REQ-RND-007), and the iTerm2 inline image protocol (REQ-RND-029).

use tellytxt_core::consts::{COLOURS, PAGE_WIDTH, ROWS};
use tellytxt_core::glyphs::GlyphSet;
use tellytxt_core::level1::Grid;
use tellytxt_core::render::{ALL_ROWS, IndexBuffer, IndexFormat, RenderOptions, render_screen};
use tellytxt_image::palette::{Rgba, palette};
use tellytxt_image::scale::{scale_indices, scaled_size};

/// A rendered page: palette indices plus the palette for one flash phase.
pub struct Raster {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Palette indices, row by row.
    pub indices: Vec<u8>,
    /// The palette.
    pub palette: [Rgba; tellytxt_core::consts::PALETTE_SIZE],
}

/// Scaling of the output image (REQ-RND-004).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Scale {
    /// Horizontal factor.
    pub x: usize,
    /// Vertical factor.
    pub y: usize,
    /// Stretch height by 1.2 for 4:3 proportions.
    pub aspect: bool,
}

/// Renders a grid to a scaled raster.
#[must_use]
pub fn raster(
    grid: &Grid,
    options: RenderOptions,
    scale: Scale,
    colours: &[Rgba; COLOURS],
    picture: Rgba,
    flash_visible: bool,
) -> Raster {
    let height = ROWS * options.row_height();
    let mut indices = vec![0u8; PAGE_WIDTH * height];
    let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut indices, stride: PAGE_WIDTH };
    // The buffer is sized for the whole page, so rendering cannot fail.
    let _ = render_screen(grid, ALL_ROWS, &GlyphSet::bedstead(), options, &mut buffer);
    let (width, scaled_height) = scaled_size(PAGE_WIDTH, height, scale.x, scale.y, scale.aspect);
    let mut scaled = vec![0u8; width * scaled_height];
    let _ = scale_indices(&indices, PAGE_WIDTH, height, scale.x.max(1), scale.y.max(1), scale.aspect, &mut scaled);
    Raster {
        width,
        height: scaled_height,
        indices: scaled,
        palette: palette(colours, picture, flash_visible, options.reveal),
    }
}

/// Encodes a raster as PNG bytes.
pub fn png(raster: &Raster) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut scratch = vec![0u8; 2 * (raster.width + 1)];
    let width = raster.width;
    tellytxt_image::png::write(
        &mut out,
        raster.width,
        raster.height,
        &raster.palette,
        &mut |y, row: &mut [u8]| {
            if let Some(source) = raster.indices.get(y * width..(y + 1) * width) {
                row.copy_from_slice(source);
            }
        },
        &mut scratch,
    )
    .map_err(|error| match error {
        tellytxt_image::png::PngError::Sink(error) => error,
        other => std::io::Error::other(format!("{other:?}")),
    })?;
    Ok(out)
}

/// Encodes a raster as PPM bytes, transparent areas in `fill`.
pub fn ppm(raster: &Raster, fill: Rgba) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    tellytxt_image::ppm::write(&mut out, &raster.indices, raster.width, raster.height, &raster.palette, fill)?;
    Ok(out)
}

/// Standard base64 (RFC 4648 section 4), for the iTerm2 protocol.
#[must_use]
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk.first().copied().unwrap_or(0), chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let value = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for index in 0..4 {
            if index <= chunk.len() {
                let sextet = (value >> (18 - 6 * index)) & 0x3F;
                out.push(char::from(ALPHABET[sextet as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The iTerm2 inline image escape sequence for a PNG (the "File=" form of
/// iTerm2's proprietary OSC 1337). Only this fixed sequence is emitted
/// around data we encoded ourselves (REQ-SEC-010).
#[must_use]
pub fn iterm2_inline(png: &[u8], width: usize, height: usize) -> String {
    format!(
        "\x1b]1337;File=inline=1;size={};width={}px;height={}px;preserveAspectRatio=0:{}\x07",
        png.len(),
        width,
        height,
        base64(png)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_vectors() {
        // RFC 4648 section 10.
        for (input, expected) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foobar", "Zm9vYmFy")] {
            assert_eq!(base64(input.as_bytes()), expected);
        }
    }

    #[test]
    fn inline_sequence_shape() {
        let sequence = iterm2_inline(b"xyz", 480, 500);
        assert!(sequence.starts_with("\x1b]1337;File=inline=1;size=3;width=480px;height=500px"));
        assert!(sequence.ends_with(":eHl6\x07"));
    }
}
