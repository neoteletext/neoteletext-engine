//! Black level (REQ-SLC-003): from the back porch when the synchronising
//! edge is known, otherwise from a histogram of the line's levels.

use neoteletext_core::consts::BIT_RATE_HZ;

use crate::consts::{
    BACK_PORCH_GUARD_NS, BLACK_HIGH_PERCENTILE, BLACK_HISTOGRAM_BINS, BLACK_HISTOGRAM_SMOOTH, BLACK_LOW_PERCENTILE,
    BLACK_REFINE_BINS, BURST_CYCLES_MAX, BURST_START_NS, PAL_SUBCARRIER_HZ, TIMING_EARLY_NS, TIMING_REFERENCE_BIT,
    TIMING_REFERENCE_NS,
};
use crate::math::{ceil_index, f32_from_i32, f32_from_u32, f32_from_usize, floor_index};

use super::line::{Line, SampleFormat};
use super::{NS_PER_SECOND, RateParameters, Slicer};

impl Slicer {
    /// Converts the line's samples, estimates its black level and stores the
    /// samples relative to it.
    pub(super) fn load(&mut self, line: &Line<'_>) {
        if self.rate.is_none_or(|rate| rate.sample_rate_hz != line.sample_rate_hz) {
            self.rate = RateParameters::new(line.sample_rate_hz);
        }
        let mut histogram = Histogram::new();
        let mut len = 0_usize;
        match line.format {
            SampleFormat::U8 => {
                for (slot, &sample) in self.samples.iter_mut().zip(line.samples) {
                    histogram.add(usize::from(sample), f32::from(sample));
                    *slot = f32::from(sample);
                    len = len.saturating_add(1);
                }
            }
            SampleFormat::U16Le => {
                for (slot, pair) in self.samples.iter_mut().zip(line.samples.as_chunks::<2>().0) {
                    // REQ-GEN-007: assembled as the format names it,
                    // whatever the host's byte order.
                    let sample = u16::from_le_bytes(*pair);
                    histogram.add(usize::from(sample >> u8::BITS), f32::from(sample));
                    *slot = f32::from(sample);
                    len = len.saturating_add(1);
                }
            }
        }
        self.len = len;
        self.format = line.format;
        let back_porch = self.back_porch_black(line);
        let black = back_porch.unwrap_or_else(|| histogram.black());
        self.black = black;
        for sample in self.samples.iter_mut().take(len) {
            *sample -= black;
        }
    }

    /// The mean of the back porch, between the colour burst and the earliest
    /// data, when the synchronising edge is known and the window is inside
    /// the line.
    fn back_porch_black(&self, line: &Line<'_>) -> Option<f32> {
        let offset = f32_from_i32(line.sync_offset_ns?);
        let rate = self.rate?;
        let bit_ns = NS_PER_SECOND / f32_from_u32(BIT_RATE_HZ);
        let burst_end = f32_from_u32(BURST_START_NS)
            + f32_from_u32(BURST_CYCLES_MAX) * NS_PER_SECOND / PAL_SUBCARRIER_HZ
            + f32_from_u32(BACK_PORCH_GUARD_NS);
        // The earliest reference bit centre, back to the start of bit 1 and
        // one more bit for the pulse's precursor.
        let earliest_data =
            f32_from_u32(TIMING_REFERENCE_NS - TIMING_EARLY_NS) - (f32_from_usize(TIMING_REFERENCE_BIT) + 0.5) * bit_ns;
        let to_sample = |ns: f32| (ns - offset) * f32_from_u32(rate.sample_rate_hz) / NS_PER_SECOND;
        let first = to_sample(burst_end);
        let last = to_sample(earliest_data);
        if first < 0.0 || last <= first {
            return None;
        }
        let (first, last) = (ceil_index(first), floor_index(last));
        let window = self.samples.get(first..=last.min(self.len.checked_sub(1)?))?;
        if window.is_empty() {
            return None;
        }
        Some(window.iter().sum::<f32>() / f32_from_usize(window.len()))
    }
}

/// A histogram of the line's levels, for the black level when the
/// synchronising edge is unknown.
struct Histogram {
    counts: [u16; BLACK_HISTOGRAM_BINS],
    sums: [f32; BLACK_HISTOGRAM_BINS],
}

impl Histogram {
    const fn new() -> Self {
        Self { counts: [0; BLACK_HISTOGRAM_BINS], sums: [0.0; BLACK_HISTOGRAM_BINS] }
    }

    fn add(&mut self, bin: usize, value: f32) {
        if let Some(count) = self.counts.get_mut(bin) {
            *count = count.saturating_add(1);
        }
        if let Some(sum) = self.sums.get_mut(bin) {
            *sum += value;
        }
    }

    /// The black level when the synchronising edge is unknown.
    ///
    /// Blanking and the "0" bits are at black (EN 300 706 clause 5.2), so
    /// black is the most common level once two things are set aside: the
    /// synchronising pulse, the lowest [`BLACK_LOW_PERCENTILE`] percent of
    /// the samples, and the "1" bits, which can outnumber the "0" bits on a
    /// row of solid mosaics. Black is therefore looked for between that
    /// percentile and the midpoint of the line's range, on a smoothed
    /// histogram, and refined to the mean of the samples around the peak.
    fn black(&self) -> f32 {
        let total: u32 = self.counts.iter().map(|&count| u32::from(count)).sum();
        let low_bin = self.percentile_bin(total, BLACK_LOW_PERCENTILE);
        let high_bin = self.percentile_bin(total, BLACK_HIGH_PERCENTILE);
        let middle = low_bin.midpoint(high_bin);
        let mut best_bin = low_bin;
        let mut best_count = 0u32;
        for bin in low_bin..=middle {
            let (count, _) = self.around(bin, BLACK_HISTOGRAM_SMOOTH, low_bin);
            if count > best_count {
                best_count = count;
                best_bin = bin;
            }
        }
        let (count, sum) = self.around(best_bin, BLACK_REFINE_BINS, low_bin);
        if count == 0 { 0.0 } else { sum / f32_from_u32(count) }
    }

    /// The count and sum of the samples in the bins within `reach` of
    /// `bin`, not below `floor`, so the synchronising pulse never counts.
    fn around(&self, bin: usize, reach: usize, floor: usize) -> (u32, f32) {
        let low = bin.saturating_sub(reach).max(floor);
        let high = bin.saturating_add(reach).min(BLACK_HISTOGRAM_BINS - 1);
        let count = self.counts.get(low..=high).unwrap_or(&[]).iter().map(|&count| u32::from(count)).sum();
        let sum = self.sums.get(low..=high).unwrap_or(&[]).iter().sum();
        (count, sum)
    }

    /// The first bin at which the cumulative count reaches `percent` of
    /// `total`.
    fn percentile_bin(&self, total: u32, percent: u32) -> usize {
        let target = total.saturating_mul(percent) / 100;
        let mut cumulative = 0u32;
        for (bin, &count) in self.counts.iter().enumerate() {
            cumulative = cumulative.saturating_add(u32::from(count));
            if cumulative > target {
                return bin;
            }
        }
        BLACK_HISTOGRAM_BINS - 1
    }
}
