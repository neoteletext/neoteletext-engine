//! The bit allocation (RFC 6716 §4.3.3): each band's cap, the interpolation between the
//! allocation vectors, the skipped bands, and the split of each band's bits between its fine
//! energy and its shape; and the pulse caches that turn a shape's bits into pulses.

use crate::celt::energy::MAX_FINE_BITS;
use crate::celt::tables::{BAND_ALLOCATION, BAND_WIDTHS, CAPS, EBANDS, LOG_N, PULSE_CACHE_BITS, PULSE_CACHE_INDEX};
use crate::celt::{BITRES, FrameDuration, NB_BANDS};
use crate::range::EcDec;
use core::num::NonZeroU32;
use core::ops::Range;

/// `LOG_MAX_PSEUDO`: the bisection steps over a pulse cache.
const LOG_MAX_PSEUDO: i32 = 6;
/// `FINE_OFFSET`: the fine energy's offset from its fair share of a band's bits.
const FINE_OFFSET: i32 = 21;
/// `ALLOC_STEPS`: the bisection steps of the interpolation between two allocation vectors.
const ALLOC_STEPS: i32 = 6;
/// The last of the allocation vectors.
const LAST_VECTOR: usize = BAND_ALLOCATION.len() - 1;
/// The bits that hold one fine-energy bit, the least allocation a coded band has (`alloc_floor`
/// for one channel).
const ALLOC_FLOOR: i32 = 1 << BITRES;

/// The width of band `band` in a 2.5 ms block.
#[inline]
#[must_use]
pub fn band_width(band: usize) -> i32 {
    BAND_WIDTHS.get(band).map_or(0, |&width| i32::from(width))
}

/// The first bin of band `band` in a 2.5 ms block, or the end of the bands for 21.
#[inline]
#[must_use]
pub fn band_start(band: usize) -> i32 {
    EBANDS.get(band).map_or(0, |&edge| i32::from(edge))
}

/// `get_pulses`: the pulses of pseudo-pulse count `i`.
#[inline]
#[must_use]
pub const fn get_pulses(i: i32) -> i32 {
    // `8 + (i & 7)`, as a bit set on three bits.
    if i < 8 { i } else { (8 | (i & 7)) << (i >> 3).wrapping_sub(1) }
}

/// The pulse cache of band `band` at `lm` (from -1, a split of an LM 0 band, to 3); a band
/// whose index is negative has none.
#[inline]
fn cache_row(band: usize, lm: i32) -> &'static [u8] {
    let (rows, _) = PULSE_CACHE_INDEX.as_chunks::<NB_BANDS>();
    usize::try_from(lm.wrapping_add(1))
        .ok()
        .and_then(|row| rows.get(row)?.get(band))
        .and_then(|&index| usize::try_from(index).ok())
        .and_then(|index| PULSE_CACHE_BITS.get(index..))
        .unwrap_or(&[0])
}

/// `bits2pulses`: the pseudo-pulse count whose cost is nearest `bits`.
#[must_use]
pub fn bits2pulses(band: usize, lm: i32, bits: i32) -> u8 {
    let cache = cache_row(band, lm);
    let entry = |i: u8| cache.get(usize::from(i)).map_or(0, |&b| i32::from(b));
    let mut lo: u8 = 0;
    let mut hi: u8 = cache.first().copied().unwrap_or(0);
    let bits = bits.wrapping_sub(1);
    for _ in 0..LOG_MAX_PSEUDO {
        // Both are at most 255: their sum fits 16 bits, and its half 8 again.
        let mid = (u16::from(lo).wrapping_add(u16::from(hi)).wrapping_add(1) >> 1) as u8;
        if entry(mid) >= bits {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let low = if lo == 0 { -1 } else { entry(lo) };
    if bits.wrapping_sub(low) <= entry(hi).wrapping_sub(bits) { lo } else { hi }
}

/// `pulses2bits`: the bits `pulses` pseudo-pulses cost.
#[must_use]
pub fn pulses2bits(band: usize, lm: i32, pulses: u8) -> i32 {
    if pulses == 0 {
        0
    } else {
        cache_row(band, lm).get(usize::from(pulses)).map_or(0, |&b| i32::from(b)).wrapping_add(1)
    }
}

/// Whether a band at `lm` with `bits` would be split in two by the shape decoder: when it has
/// more than its largest codebook can use, with some to spare.
#[inline]
#[must_use]
pub fn splits(band: usize, lm: i32, bits: i32) -> bool {
    let cache = cache_row(band, lm);
    let largest = cache.first().and_then(|&max| cache.get(usize::from(max))).map_or(0, |&b| i32::from(b));
    bits > largest.wrapping_add(12)
}

/// `init_caps`: each band's largest useful allocation.
#[must_use]
pub fn init_caps(duration: FrameDuration) -> [i32; NB_BANDS] {
    let lm = duration.lm();
    let mut caps = [0i32; NB_BANDS];
    for ((out, &cap), &width) in caps.iter_mut().zip(duration.row(&CAPS)).zip(&BAND_WIDTHS) {
        *out = i32::from(cap).wrapping_add(64).wrapping_mul(i32::from(width) << lm) >> 2;
    }
    caps
}

/// A frame's allocation.
pub struct Allocation {
    /// The bands coded; those from here to the end were skipped.
    pub coded_bands: usize,
    /// The bits left over the caps, for the shape decoder to share out.
    pub balance: i32,
    /// Each band's shape bits, in 1/8 bits (the C's `pulses`).
    pub pulses: [i32; NB_BANDS],
    /// Each band's fine-energy bits (the C's `ebits`, `fine_quant`).
    pub fine_quant: [u8; NB_BANDS],
    /// Whether each band is among the last to have its final energy bit.
    pub fine_priority: [bool; NB_BANDS],
}

/// The two allocation vectors `interp_bits2pulses` interpolates between, with each band's
/// threshold and cap, all in 1/8 bits.
pub struct AllocationVectors<'a> {
    /// The lower vector.
    pub bits1: &'a [i32; NB_BANDS],
    /// The upper vector, less the lower.
    pub bits2: &'a [i32; NB_BANDS],
    /// Below this, a band gets no shape bits.
    pub thresh: &'a [i32; NB_BANDS],
    /// The most a band can use.
    pub cap: &'a [i32; NB_BANDS],
}

/// `celt_udiv` as the C calls it on two `opus_int32`s: both converted to unsigned, a negative
/// dividend as its two's complement, and the quotient converted back.
#[inline]
fn udiv(n: i32, d: i32) -> i32 {
    // The C never divides by zero here: every band has a bin at least.
    NonZeroU32::new(d.cast_unsigned()).map_or(0, |d| (n.cast_unsigned() / d).cast_signed())
}

/// The bits of the coded bands while the allocation is shared out: each band's, their sum, and
/// the frame's total.
struct Shares {
    bits: [i32; NB_BANDS],
    psum: i32,
    total: i32,
}

/// `interp_bits2pulses`, decoding mono (RFC 6716 §4.3.3): the interpolation between the
/// allocation vectors `bits1` and `bits1 + bits2`, the skipping of bands from the top (read from
/// `dec`), and each coded band's split into fine energy and shape.
pub fn interp_bits2pulses(
    bands: Range<usize>,
    skip_start: usize,
    vectors: &AllocationVectors,
    total: i32,
    skip_rsv: i32,
    lm: i32,
    dec: &mut EcDec,
) -> Allocation {
    // The mode's bands at most, so that each band's entries of the arrays are in reach.
    let Range { start, end } = bands.start..bands.end.min(NB_BANDS);
    if end <= start {
        // No bands: nothing to allocate (the C divides by zero here; a valid stream never has
        // its start band at or past its end band).
        return Allocation {
            coded_bands: start,
            balance: 0,
            pulses: [0; NB_BANDS],
            fine_quant: [0; NB_BANDS],
            fine_priority: [false; NB_BANDS],
        };
    }
    let mut shares = interpolate(start..end, vectors, total);
    let coded_bands = skip_bands(&mut shares, start..end, skip_start, vectors.thresh, skip_rsv, dec);

    // Allocate the remaining bits.
    let Shares { mut bits, psum, total } = shares;
    let mut left = total.wrapping_sub(psum);
    let width = band_start(coded_bands).wrapping_sub(band_start(start));
    let percoeff = udiv(left, width);
    left = left.wrapping_sub(width.wrapping_mul(percoeff));
    for (j, b) in bits.iter_mut().enumerate().take(coded_bands).skip(start) {
        *b = b.wrapping_add(percoeff.wrapping_mul(band_width(j)));
    }
    for (j, b) in bits.iter_mut().enumerate().take(coded_bands).skip(start) {
        let tmp = left.min(band_width(j));
        *b = b.wrapping_add(tmp);
        left = left.wrapping_sub(tmp);
    }
    split_fine_energy(bits, start..coded_bands, end, vectors.cap, lm)
}

/// The bisection between the allocation vectors for the most bits that fit `total`, and each
/// band's share there.
fn interpolate(bands: Range<usize>, vectors: &AllocationVectors, total: i32) -> Shares {
    let AllocationVectors { bits1, bits2, thresh, cap } = *vectors;
    let mut lo: i32 = 0;
    let mut hi: i32 = 1 << ALLOC_STEPS;
    // Each band's vectors, threshold and cap, from the top band down.
    let vectors = || bits1.iter().zip(bits2).zip(thresh.iter().zip(cap)).take(bands.end).skip(bands.start).rev();
    for _ in 0..ALLOC_STEPS {
        let mid = lo.wrapping_add(hi) >> 1;
        let mut psum: i32 = 0;
        let mut done = false;
        for ((&bits1, &bits2), (&thresh, &cap)) in vectors() {
            let tmp = bits1.wrapping_add(mid.wrapping_mul(bits2) >> ALLOC_STEPS);
            if tmp >= thresh || done {
                done = true;
                // Don't allocate more than we can actually use.
                psum = psum.wrapping_add(tmp.min(cap));
            } else if tmp >= ALLOC_FLOOR {
                psum = psum.wrapping_add(ALLOC_FLOOR);
            }
        }
        if psum > total {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let mut bits = [0i32; NB_BANDS];
    let mut psum: i32 = 0;
    let mut done = false;
    let shares = bits.iter_mut().take(bands.end).skip(bands.start).rev();
    for (share, ((&bits1, &bits2), (&thresh, &cap))) in shares.zip(vectors()) {
        let mut tmp = bits1.wrapping_add(lo.wrapping_mul(bits2) >> ALLOC_STEPS);
        if tmp < thresh && !done {
            tmp = if tmp >= ALLOC_FLOOR { ALLOC_FLOOR } else { 0 };
        } else {
            done = true;
        }
        // Don't allocate more than we can actually use.
        *share = tmp.min(cap);
        psum = psum.wrapping_add(*share);
    }
    Shares { bits, psum, total }
}

/// The skipping of bands from the top, each decision read from `dec` where the band could take
/// its shape; answers the bands coded. A skipped band keeps `ALLOC_FLOOR` bits for one bit of
/// fine energy, or nothing.
fn skip_bands(
    shares: &mut Shares,
    bands: Range<usize>,
    skip_start: usize,
    thresh: &[i32; NB_BANDS],
    skip_rsv: i32,
    dec: &mut EcDec,
) -> usize {
    let Shares { bits, psum, total } = shares;
    let start = bands.start;
    // From the top band down, while the band below this one would remain coded.
    let mut coded_bands = bands.end;
    for (j, (share, &thresh)) in bits.iter_mut().zip(thresh).enumerate().take(bands.end).rev() {
        // Never skip the first band, nor a band that has been boosted by dynalloc.
        if j <= skip_start {
            break;
        }
        // The left-over bits this band would get, with those stolen back from the skipped
        // bands above it.
        let mut left = total.wrapping_sub(*psum);
        let width = band_start(coded_bands).wrapping_sub(band_start(start));
        let percoeff = udiv(left, width);
        left = left.wrapping_sub(width.wrapping_mul(percoeff));
        let rem = left.wrapping_sub(band_start(j).wrapping_sub(band_start(start))).max(0);
        let mut band_bits = share.wrapping_add(percoeff.wrapping_mul(band_width(j))).wrapping_add(rem);
        // A skip decision is coded only above the band's threshold; below it the band is
        // skipped regardless, so there are always the bits to code the flag.
        if band_bits >= thresh.max(ALLOC_FLOOR + (1 << BITRES)) {
            if dec.dec_bit_logp(1) {
                return coded_bands;
            }
            // We used a bit to skip this band.
            *psum = psum.wrapping_add(1 << BITRES);
            band_bits = band_bits.wrapping_sub(1 << BITRES);
        }
        // Reclaim the bits originally allocated to this band.
        *psum = psum.wrapping_sub(*share);
        if band_bits >= ALLOC_FLOOR {
            // Enough for a fine energy bit: use it.
            *psum = psum.wrapping_add(ALLOC_FLOOR);
            *share = ALLOC_FLOOR;
        } else {
            // Otherwise this band gets nothing at all.
            *share = 0;
        }
        coded_bands = j;
    }
    // Give the bit we reserved to end skipping back.
    *total = total.wrapping_add(skip_rsv);
    coded_bands
}

/// Each coded band's bits `bits` split between its fine energy and its shape, the excess over
/// its cap passed up as the balance; and the skipped bands to `end` given their fine bit.
fn split_fine_energy(
    mut bits: [i32; NB_BANDS],
    coded: Range<usize>,
    end: usize,
    cap: &[i32; NB_BANDS],
    lm: i32,
) -> Allocation {
    // The mode's bands at most, so that each band's entries of the arrays are in reach.
    let end = end.min(NB_BANDS);
    let coded = coded.start.min(end)..coded.end.min(end);
    let mut ebits = [0u8; NB_BANDS];
    let mut fine_priority = [false; NB_BANDS];
    let log_m = lm << BITRES;
    let coded_bands = coded.end;
    let mut balance: i32 = 0;
    let coded_shares = bits
        .iter_mut()
        .zip(cap)
        .zip(LOG_N.iter().zip(&BAND_WIDTHS))
        .zip(ebits.iter_mut().zip(fine_priority.iter_mut()))
        .take(coded.end)
        .skip(coded.start);
    for (((share, &cap), (&log_n, &width)), (ebits, fine_priority)) in coded_shares {
        let n = i32::from(width) << lm;
        let bit = share.wrapping_add(balance);
        let mut excess: i32;
        if n > 1 {
            excess = bit.wrapping_sub(cap).max(0);
            *share = bit.wrapping_sub(excess);
            let nclogn = n.wrapping_mul(i32::from(log_n).wrapping_add(log_m));
            // The fine bits' offset from their fair share of the band's: log2(N)/2 + FINE_OFFSET.
            let mut offset = (nclogn >> 1).wrapping_sub(n.wrapping_mul(FINE_OFFSET));
            // N=2 is the only point that doesn't match the curve.
            if n == 2 {
                offset = offset.wrapping_add(n << BITRES >> 2);
            }
            // Changing the offset for allocating the second and third fine energy bit.
            if share.wrapping_add(offset) < n.wrapping_mul(2) << BITRES {
                offset = offset.wrapping_add(nclogn >> 2);
            } else if share.wrapping_add(offset) < n.wrapping_mul(3) << BITRES {
                offset = offset.wrapping_add(nclogn >> 3);
            }
            // Divide with rounding.
            let rounded = share.wrapping_add(offset).wrapping_add(n << (BITRES - 1)).max(0);
            let mut fine = udiv(rounded, n) >> BITRES;
            // Make sure not to bust, and take no more than PVQ can use.
            if fine > *share >> BITRES {
                fine = *share >> BITRES;
            }
            *ebits = u8::try_from(fine.clamp(0, i32::from(MAX_FINE_BITS))).unwrap_or(MAX_FINE_BITS);
            let fine = i32::from(*ebits);
            // A band rounded down or capped is a candidate for the final fine energy pass.
            *fine_priority = fine.wrapping_mul(n << BITRES) >= share.wrapping_add(offset);
            // The rest of the band's bits go to its shape.
            *share = share.wrapping_sub(fine << BITRES);
        } else {
            // For N=1, all bits go to fine energy except for a single sign bit.
            excess = bit.wrapping_sub(1 << BITRES).max(0);
            *share = bit.wrapping_sub(excess);
            *ebits = 0;
            *fine_priority = true;
        }
        // Fine energy can't take advantage of the shape decoder's re-balancing, so the excess
        // over the cap is re-balanced here.
        if excess > 0 {
            let room = MAX_FINE_BITS.saturating_sub(*ebits);
            // The excess is positive, so its whole bits convert unless there are more than
            // the room for them.
            let extra_fine = u8::try_from(excess >> BITRES).map_or(room, |whole| whole.min(room));
            *ebits = ebits.saturating_add(extra_fine);
            let extra_bits = i32::from(extra_fine) << BITRES;
            *fine_priority = extra_bits >= excess.wrapping_sub(balance);
            excess = excess.wrapping_sub(extra_bits);
        }
        balance = excess;
    }

    // The skipped bands use all their bits for fine energy: `skip_bands` left each `ALLOC_FLOOR`,
    // a bit, or nothing.
    let skipped = bits.iter_mut().zip(ebits.iter_mut().zip(fine_priority.iter_mut())).take(end).skip(coded_bands);
    for (share, (ebits, fine_priority)) in skipped {
        let has_fine_bit = *share >= ALLOC_FLOOR;
        *ebits = u8::from(has_fine_bit);
        *share = 0;
        *fine_priority = !has_fine_bit;
    }
    Allocation { coded_bands, balance, pulses: bits, fine_quant: ebits, fine_priority }
}

/// `clt_compute_allocation`, decoding mono (RFC 6716 §4.3.3): from the dynalloc boosts
/// `offsets`, the caps `cap`, the allocation trim and the `total` bits (in 1/8 bits), the
/// allocation of the bands `bands`.
pub fn compute_allocation(
    bands: Range<usize>,
    offsets: &[i32; NB_BANDS],
    cap: &[i32; NB_BANDS],
    alloc_trim: i32,
    total: i32,
    lm: i32,
    dec: &mut EcDec,
) -> Allocation {
    // The mode's bands at most, so that each band's entries of the arrays are in reach.
    let Range { start, end } = bands.start..bands.end.min(NB_BANDS);
    let mut bits1 = [0i32; NB_BANDS];
    let mut bits2 = [0i32; NB_BANDS];
    let mut thresh = [0i32; NB_BANDS];
    let mut trim_offset = [0i32; NB_BANDS];

    let mut total = total.max(0);
    // Reserve a bit to signal the end of manually skipped bands.
    let skip_rsv = if total >= 1 << BITRES { 1 << BITRES } else { 0 };
    total = total.wrapping_sub(skip_rsv);

    // From the top band down, each with the count of the bands above it.
    let tilts = thresh.iter_mut().zip(trim_offset.iter_mut()).zip(&BAND_WIDTHS).take(end).skip(start).rev();
    for (((thresh, trim_offset), &width), above) in tilts.zip(0i32..) {
        let n = i32::from(width);
        // Below this threshold, no PVQ bits are allocated.
        *thresh = ALLOC_FLOOR.max((n.wrapping_mul(3) << lm << BITRES) >> 4);
        // The tilt of the allocation curve.
        *trim_offset = n
            .wrapping_mul(alloc_trim.wrapping_sub(5).wrapping_sub(lm))
            .wrapping_mul(above)
            .wrapping_mul(1 << BITRES << lm)
            >> 6;
        // A band of one coefficient gets less resolution: it gains more from a coarse value of
        // its own.
        if n << lm == 1 {
            *trim_offset = trim_offset.wrapping_sub(ALLOC_FLOOR);
        }
    }
    let vector_bits = |row: usize, j: usize| {
        (band_width(j).wrapping_mul(BAND_ALLOCATION.get(row).and_then(|row| row.get(j)).map_or(0, |&b| i32::from(b)))
            << lm)
            >> 2
    };
    // Each band's tilt, boost, threshold and cap.
    let band_terms = || trim_offset.iter().zip(offsets).zip(thresh.iter().zip(cap)).enumerate().take(end).skip(start);
    let mut lo = 1;
    let mut hi = LAST_VECTOR;
    loop {
        let mut done = false;
        let mut psum: i32 = 0;
        let mid = usize::midpoint(lo, hi);
        for (j, ((&trim_offset, &offset), (&thresh, &cap))) in band_terms().rev() {
            let mut vector = vector_bits(mid, j);
            if vector > 0 {
                vector = vector.wrapping_add(trim_offset).max(0);
            }
            vector = vector.wrapping_add(offset);
            if vector >= thresh || done {
                done = true;
                // Don't allocate more than we can actually use.
                psum = psum.wrapping_add(vector.min(cap));
            } else if vector >= ALLOC_FLOOR {
                psum = psum.wrapping_add(ALLOC_FLOOR);
            }
        }
        // The bisection stays within 0 and `LAST_VECTOR + 1`: `mid` is at least `lo`, which
        // starts at 1.
        if psum > total {
            hi = mid.saturating_sub(1);
        } else {
            lo = mid.saturating_add(1);
        }
        if lo > hi {
            break;
        }
    }
    let hi = lo;
    let lo = lo.saturating_sub(1);
    let mut skip_start = start;
    for ((j, ((&trim_offset, &offset), (_, &cap))), (bits1, bits2)) in
        band_terms().zip(bits1.iter_mut().zip(bits2.iter_mut()).skip(start))
    {
        let mut bits1j = vector_bits(lo, j);
        let mut bits2j = if hi > LAST_VECTOR { cap } else { vector_bits(hi, j) };
        if bits1j > 0 {
            bits1j = bits1j.wrapping_add(trim_offset).max(0);
        }
        if bits2j > 0 {
            bits2j = bits2j.wrapping_add(trim_offset).max(0);
        }
        if lo > 0 {
            bits1j = bits1j.wrapping_add(offset);
        }
        bits2j = bits2j.wrapping_add(offset);
        if offset > 0 {
            skip_start = j;
        }
        *bits1 = bits1j;
        *bits2 = bits2j.wrapping_sub(bits1j).max(0);
    }
    interp_bits2pulses(
        start..end,
        skip_start,
        &AllocationVectors { bits1: &bits1, bits2: &bits2, thresh: &thresh, cap },
        total,
        skip_rsv,
        lm,
        dec,
    )
}
