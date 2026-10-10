//! Scaling at the edge (REQ-RND-024): whole-number factors, with optional
//! 4:3 aspect correction, by nearest pixel. The core never scales.

use tellytxt_core::consts::{ASPECT_DENOMINATOR, ASPECT_NUMERATOR};

/// Errors from scaling.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScaleError {
    /// The target buffer is too small.
    BufferTooSmall,
    /// A factor of zero was given.
    ZeroFactor,
}

/// The output size for an input size, factors and aspect correction.
#[must_use]
pub fn scaled_size(width: usize, height: usize, x_factor: usize, y_factor: usize, aspect: bool) -> (usize, usize) {
    let out_height = height.saturating_mul(y_factor);
    let out_height = if aspect {
        // REQ-RND-004: pixels about 1.2 times taller than wide.
        out_height.saturating_mul(ASPECT_NUMERATOR) / ASPECT_DENOMINATOR
    } else {
        out_height
    };
    (width.saturating_mul(x_factor), out_height)
}

/// Scales an image of single-byte pixels (palette indices) by nearest
/// pixel. `out` is `out_width` by `out_height` as given by
/// [`scaled_size`], tightly packed.
pub fn scale_indices(
    input: &[u8],
    width: usize,
    height: usize,
    x_factor: usize,
    y_factor: usize,
    aspect: bool,
    out: &mut [u8],
) -> Result<(usize, usize), ScaleError> {
    if x_factor == 0 || y_factor == 0 {
        return Err(ScaleError::ZeroFactor);
    }
    let (out_width, out_height) = scaled_size(width, height, x_factor, y_factor, aspect);
    let needed = out_width.checked_mul(out_height).ok_or(ScaleError::BufferTooSmall)?;
    if out.len() < needed {
        return Err(ScaleError::BufferTooSmall);
    }
    for oy in 0..out_height {
        // Map back to the source row; integer arithmetic only.
        let sy = oy.saturating_mul(height).checked_div(out_height).unwrap_or(0).min(height.saturating_sub(1));
        let source = input.get(sy.saturating_mul(width)..sy.saturating_add(1).saturating_mul(width)).unwrap_or(&[]);
        let target = out.get_mut(oy.saturating_mul(out_width)..oy.saturating_add(1).saturating_mul(out_width));
        let Some(target) = target else { return Err(ScaleError::BufferTooSmall) };
        for (ox, pixel) in target.iter_mut().enumerate() {
            *pixel = source.get(ox.checked_div(x_factor).unwrap_or(0)).copied().unwrap_or(0);
        }
    }
    Ok((out_width, out_height))
}

/// Scales an RGB888 image by whole numbers with a 2 by 2 box filter
/// applied after nearest scaling, the "smoothed" option of REQ-RND-024.
#[allow(clippy::too_many_arguments)] // Geometry of input and output; grouping them adds nothing.
pub fn scale_rgb_smoothed(
    input: &[u8],
    width: usize,
    height: usize,
    x_factor: usize,
    y_factor: usize,
    aspect: bool,
    out: &mut [u8],
) -> Result<(usize, usize), ScaleError> {
    if x_factor == 0 || y_factor == 0 {
        return Err(ScaleError::ZeroFactor);
    }
    let (out_width, out_height) = scaled_size(width, height, x_factor, y_factor, aspect);
    let needed = out_width.saturating_mul(out_height).saturating_mul(3);
    if out.len() < needed {
        return Err(ScaleError::BufferTooSmall);
    }
    let sample = |x: usize, y: usize, channel: usize| -> u16 {
        let sx = x.checked_div(x_factor).unwrap_or(0).min(width.saturating_sub(1));
        let sy = y.saturating_mul(height).checked_div(out_height).unwrap_or(0).min(height.saturating_sub(1));
        let index = sy.saturating_mul(width).saturating_add(sx).saturating_mul(3).saturating_add(channel);
        u16::from(input.get(index).copied().unwrap_or(0))
    };
    for oy in 0..out_height {
        for ox in 0..out_width {
            for channel in 0..3 {
                let x1 = ox.saturating_add(1).min(out_width.saturating_sub(1));
                let y1 = oy.saturating_add(1).min(out_height.saturating_sub(1));
                let sum = sample(ox, oy, channel)
                    .saturating_add(sample(x1, oy, channel))
                    .saturating_add(sample(ox, y1, channel))
                    .saturating_add(sample(x1, y1, channel));
                let index = oy.saturating_mul(out_width).saturating_add(ox).saturating_mul(3).saturating_add(channel);
                if let Some(slot) = out.get_mut(index) {
                    *slot = (sum >> 2) as u8;
                }
            }
        }
    }
    Ok((out_width, out_height))
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn nearest_scaling_and_aspect() {
        let input = [1u8, 2, 3, 4];
        let mut out = [0u8; 16];
        assert_eq!(scale_indices(&input, 2, 2, 2, 2, false, &mut out).unwrap(), (4, 4));
        assert_eq!(&out[..8], &[1, 1, 2, 2, 1, 1, 2, 2]);
        assert_eq!(scaled_size(480, 500, 1, 1, true), (480, 600));
        assert_eq!(scale_indices(&input, 2, 2, 0, 1, false, &mut out), Err(ScaleError::ZeroFactor));
    }
}
