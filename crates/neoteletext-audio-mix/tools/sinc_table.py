"""Writes src/sinc.rs: the resampler's windowed-sinc filter, as integers.

TAPS taps for each of PHASES fractional positions between two source
samples, each a Kaiser-windowed sinc with its cutoff at CUTOFF of the
source's Nyquist frequency, scaled so every phase sums to ONE: the filter
passes a constant signal unchanged. Run it from the crate's folder:

    python3 tools/sinc_table.py > src/sinc.rs
"""
import math

TAPS = 16
PHASES = 64
CUTOFF = 0.9
BETA = 6.0
ONE = 1 << 14


def bessel_i0(x):
    total, term, k = 1.0, 1.0, 1
    while term > 1e-12 * total:
        term *= (x / (2 * k)) ** 2
        total += term
        k += 1
    return total


def kaiser(offset):
    """The Kaiser window at `offset` samples from the interpolation point,
    reaching zero at TAPS / 2 either side."""
    ratio = offset / (TAPS / 2)
    if abs(ratio) >= 1:
        return 0.0
    return bessel_i0(BETA * math.sqrt(1 - ratio * ratio)) / bessel_i0(BETA)


rows = []
for phase in range(PHASES):
    fraction = phase / PHASES
    weights = []
    for tap in range(TAPS):
        # Tap t multiplies source sample (position - TAPS/2 + 1 + t).
        offset = tap - (TAPS // 2 - 1) - fraction
        x = CUTOFF * offset
        sinc = 1.0 if x == 0 else math.sin(math.pi * x) / (math.pi * x)
        window = kaiser(offset)
        weights.append(CUTOFF * sinc * window)
    total = sum(weights)
    scaled = [round(w / total * ONE) for w in weights]
    # Rounding error goes to the largest tap, so each phase sums to ONE.
    scaled[scaled.index(max(scaled))] += ONE - sum(scaled)
    rows.append(scaled)

print("//! The resampler's filter, written by `tools/sinc_table.py`: do not edit.")
print(f"//! {TAPS} taps for each of {PHASES} phases; Kaiser window, beta {BETA},")
print(f"//! cutoff {CUTOFF} of the source's Nyquist frequency; each phase sums to")
print(f"//! {ONE}, unity in Q14.")
print()
print("/// Taps of the filter.")
print(f"pub const TAPS: usize = {TAPS};")
print("/// Taps before the interpolation point.")
print(f"pub const TAPS_BEFORE: i64 = {TAPS // 2 - 1};")
print("/// Fractional positions between two source samples.")
print(f"pub const PHASES: usize = {PHASES};")
print("/// Unity gain in the table's fixed point.")
print(f"pub const ONE: i32 = {ONE};")
print("/// The coefficients: `FILTER[phase][tap]` weighs source sample")
print("/// `position - TAPS / 2 + 1 + tap`.")
print("#[rustfmt::skip]")
print(f"pub const FILTER: [[i16; TAPS]; PHASES] = [")
for row in rows:
    print("    [" + ", ".join(str(v) for v in row) + "],")
print("];")
