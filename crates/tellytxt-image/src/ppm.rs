//! The binary Portable Pixmap writer ("P6", REQ-RND-007). Transparent
//! pixels take the fill colour (REQ-RND-006).

use crate::Sink;
use crate::palette::Rgba;
use tellytxt_core::consts::PALETTE_SIZE;

/// Writes a decimal number into `buffer`, returning the digits.
fn decimal(value: usize, buffer: &mut [u8; 20]) -> &[u8] {
    let mut position = buffer.len();
    let mut rest = value;
    loop {
        position = position.saturating_sub(1);
        if let Some(slot) = buffer.get_mut(position) {
            *slot = b'0'.saturating_add((rest % 10) as u8);
        }
        rest /= 10;
        if rest == 0 || position == 0 {
            break;
        }
    }
    buffer.get(position..).unwrap_or(&[])
}

/// Maximum sample value of an 8-bit PPM.
const MAX_SAMPLE: &[u8] = b"255";

/// Writes a `width` by `height` image of palette indices as P6.
pub fn write<S: Sink>(
    sink: &mut S,
    indices: &[u8],
    width: usize,
    height: usize,
    palette: &[Rgba; PALETTE_SIZE],
    fill: Rgba,
) -> Result<(), S::Error> {
    let mut digits = [0u8; 20];
    sink.write_all(b"P6\n")?;
    sink.write_all(decimal(width, &mut digits))?;
    sink.write_all(b" ")?;
    sink.write_all(decimal(height, &mut digits))?;
    sink.write_all(b"\n")?;
    sink.write_all(MAX_SAMPLE)?;
    sink.write_all(b"\n")?;
    let mut row = [0u8; 3 * 64];
    for y in 0..height {
        let start = y.saturating_mul(width);
        let line = indices.get(start..start.saturating_add(width)).unwrap_or(&[]);
        for chunk in line.chunks(64) {
            let mut length = 0usize;
            for (&index, pixel) in chunk.iter().zip(row.chunks_mut(3)) {
                let colour = palette.get(usize::from(index)).copied().unwrap_or(fill);
                let colour = if colour.a == 0 { fill } else { colour };
                if let [r, g, b] = pixel {
                    (*r, *g, *b) = (colour.r, colour.g, colour.b);
                }
                length = length.saturating_add(3);
            }
            sink.write_all(row.get(..length).unwrap_or(&[]))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::SliceSink;
    use crate::palette::{CLUT0, palette};

    #[test]
    fn header_and_fill() {
        let colours = palette(&CLUT0, Rgba::TRANSPARENT, true, false);
        let mut buffer = [0u8; 64];
        let mut sink = SliceSink::new(&mut buffer);
        write(&mut sink, &[1, 8], 2, 1, &colours, Rgba::opaque(0, 0, 255)).unwrap();
        assert_eq!(sink.written(), b"P6\n2 1\n255\n\xFF\x00\x00\x00\x00\xFF");
    }
}
