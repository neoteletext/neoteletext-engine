//! Run-in search (REQ-SLC-004), measurement and the amplitude and match
//! gate (REQ-SLC-005).

use crate::consts::{
    DETECT_AMPLITUDE_FRACTION, DETECT_CONCENTRATION, PEAK_FOLLOW_BITS, PERCENT, PHASOR_RENORMALISE_STEPS,
    RUN_IN_MATCH_BITS,
};
use crate::fsm::SlicerEvent;
use crate::math::{
    atan2, ceil_index, f32_from_i32, f32_from_usize, round_i32, sin_cos, sqrt, to_u8_saturating, to_u16_saturating,
};

use super::{Candidate, RateParameters, Slicer, Work, complex_multiply, renormalise};

impl Slicer {
    pub(super) fn search(&self, work: &mut Work) -> SlicerEvent {
        let Some(rate) = self.rate else { return SlicerEvent::WindowExhausted };
        let first = self.config.search_start;
        let mut last = self.len.saturating_sub(rate.needed);
        if let Some(end) = self.config.search_end {
            last = last.min(end);
        }
        if first > last || self.len < rate.needed {
            return SlicerEvent::WindowExhausted;
        }
        match self.find_run_in(&rate, first, last, self.config.nominal_amplitude(self.format)) {
            Some(candidate) => {
                work.candidate = candidate;
                SlicerEvent::RunInFound
            }
            None => SlicerEvent::WindowExhausted,
        }
    }

    /// Slides the 14-bit match window from `first` to `last` and returns the
    /// strongest window near the first one that passes the detector.
    fn find_run_in(&self, rate: &RateParameters, first: usize, last: usize, nominal: f32) -> Option<Candidate> {
        let samples = self.samples.get(..self.len)?;
        let window = rate.window;
        let window_f = f32_from_usize(window);
        // A run-in of "1" level `a` above "0" is a tone of amplitude a/2,
        // whose correlation magnitude over the window is (a/2)(L/2).
        let floor_amplitude =
            nominal * f32::from(self.config.min_amplitude_percent) / PERCENT * DETECT_AMPLITUDE_FRACTION;
        let floor_magnitude = floor_amplitude / 2.0 * window_f / 2.0;
        let energy_floor = floor_magnitude * floor_magnitude;
        let follow = ceil_index(f32_from_usize(PEAK_FOLLOW_BITS) * rate.samples_per_bit);

        // The window's sums at `first`.
        let (sine, cosine) = sin_cos(rate.omega * f32_from_usize(first));
        let mut phasor = (cosine, -sine);
        let mut tone = (0.0_f32, 0.0_f32);
        let mut sum = 0.0_f32;
        let mut sum_squares = 0.0_f32;
        let mut running = phasor;
        for &value in samples.get(first..first.checked_add(window)?)? {
            tone = (tone.0 + value * running.0, tone.1 + value * running.1);
            sum += value;
            sum_squares += value * value;
            running = complex_multiply(running, rate.step);
        }
        let mut ahead = complex_multiply(phasor, rate.window_step);

        let mut best: Option<(f32, Candidate)> = None;
        let mut stop_after = last;
        let outgoing = samples.get(first..)?.iter();
        let incoming = samples.get(first.checked_add(window)?..)?.iter();
        for (step, (&old, &new)) in
            outgoing.zip(incoming).enumerate().take(last.saturating_sub(first).saturating_add(1))
        {
            let position = first.saturating_add(step);
            // The tone relative to the window start, with the window's mean
            // removed so the black level and the data's average cannot leak
            // into it.
            let relative = complex_multiply(tone, (phasor.0, -phasor.1));
            let mean = sum / window_f;
            let centred = (relative.0 - mean * rate.window_sum.0, relative.1 - mean * rate.window_sum.1);
            let energy = centred.0 * centred.0 + centred.1 * centred.1;
            let varying = (sum_squares - sum * mean).max(0.0);
            match best {
                None => {
                    if energy >= energy_floor && energy >= DETECT_CONCENTRATION * varying * window_f / 2.0 {
                        best = Some((energy, Candidate { start: position, tone: centred }));
                        stop_after = position.saturating_add(follow).min(last);
                    }
                }
                Some((best_energy, _)) => {
                    if energy > best_energy {
                        best = Some((energy, Candidate { start: position, tone: centred }));
                    }
                }
            }
            if position >= stop_after {
                break;
            }
            // Slide by one sample.
            tone = (tone.0 - old * phasor.0 + new * ahead.0, tone.1 - old * phasor.1 + new * ahead.1);
            sum += new - old;
            sum_squares += new * new - old * old;
            phasor = complex_multiply(phasor, rate.step);
            ahead = complex_multiply(ahead, rate.step);
            if step % PHASOR_RENORMALISE_STEPS == 0 {
                phasor = renormalise(phasor);
                ahead = renormalise(ahead);
            }
        }
        best.map(|(_, candidate)| candidate)
    }

    /// The centre of run-in bit 3 from the candidate's tone phase.
    pub(super) fn bit3_centre(&self, candidate: Candidate) -> f32 {
        let Some(rate) = self.rate else { return 0.0 };
        let start = f32_from_usize(candidate.start);
        // The run-in is a cosine peaking at the centres of its "1" bits:
        // its tone relative to the window start has phase −ω(c − start).
        let phase = atan2(candidate.tone.1, candidate.tone.0);
        let one_centre = start - phase / rate.omega;
        // Bit 3 is a "1"; the window starts near bit 3, so its centre is
        // the "1" centre (period two bits) nearest start + half a bit.
        let period = 2.0 * rate.samples_per_bit;
        let expected = start + rate.samples_per_bit / 2.0;
        one_centre + period * f32_from_i32(round_i32((expected - one_centre) / period))
    }

    // -----------------------------------------------------------------------
    // Levels, threshold and match (REQ-SLC-004, REQ-SLC-005)
    // -----------------------------------------------------------------------

    /// Estimates the levels, threshold and match from the run-in.
    ///
    /// The threshold is the mean of the 14 bit-centre values: half are "1"
    /// and half "0", so it lies midway, and interpolation errors at peaks
    /// and troughs cancel in it. The data amplitude ("1" above "0") is
    /// taken from the run-in tone instead of the bit centres, because
    /// linear interpolation of a tone at a few samples per bit cuts its
    /// peaks: a tone of peak-to-peak `a` has correlation magnitude a·L/4.
    pub(super) fn measure(&self, work: &mut Work) -> SlicerEvent {
        let Some(rate) = self.rate else { return SlicerEvent::Estimated };
        let mut values = [0.0_f32; RUN_IN_MATCH_BITS];
        let mut position = work.bit3_centre;
        for value in &mut values {
            *value = self.at(position);
            position += rate.samples_per_bit;
        }
        work.threshold = values.iter().sum::<f32>() / f32_from_usize(RUN_IN_MATCH_BITS);
        let tone = work.candidate.tone;
        let amplitude = 4.0 * sqrt(tone.0 * tone.0 + tone.1 * tone.1) / f32_from_usize(rate.window);
        work.one_level = work.threshold + amplitude / 2.0;
        work.zero_level = work.threshold - amplitude / 2.0;
        // The match: how far the bit-centre values agree in sign with the
        // alternating pattern, weighted by their distance from the
        // threshold; 100 for a clean run-in, about 0 for a random signal.
        let mut agreement = 0.0;
        let mut total = 0.0;
        for (index, value) in values.iter().enumerate() {
            let deviation = *value - work.threshold;
            agreement += if index % 2 == 0 { deviation } else { -deviation };
            total += deviation.abs();
        }
        let matched = if total > 0.0 { (agreement / total).max(0.0) * PERCENT } else { 0.0 };
        work.match_percent = to_u8_saturating(matched);
        let nominal = self.config.nominal_amplitude(self.format);
        work.amplitude_percent = to_u16_saturating(amplitude / nominal * PERCENT);
        SlicerEvent::Estimated
    }

    pub(super) fn gate(&self, work: &Work) -> SlicerEvent {
        if work.amplitude_percent >= self.config.min_amplitude_percent
            && work.match_percent >= self.config.min_match_percent
        {
            SlicerEvent::GatePassed
        } else {
            SlicerEvent::BelowGate
        }
    }
}
