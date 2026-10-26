//! The range decoder (RFC 6716 §4.1): every symbol of a frame, SILK's and CELT's, comes
//! through it, so it is written for speed and for hostile input: no index can go out of
//! bounds, no division by zero can occur, and a malformed stream only sets `error`.

use crate::fixed::{bit_count, ec_ilog};
use core::num::NonZeroU32;

/// Bits per input symbol (RFC 6716 §4.1: `EC_SYM_BITS`).
const SYM_BITS: u32 = 8;
const SYM_MAX: u32 = (1 << SYM_BITS) - 1;
/// The state's precision (`EC_CODE_BITS`), its top (`EC_CODE_TOP`) and the normalisation
/// threshold (`EC_CODE_BOT`).
const CODE_BITS: u32 = 32;
const CODE_TOP: u32 = 1 << (CODE_BITS - 1);
const CODE_BOT: u32 = CODE_TOP >> SYM_BITS;
/// Bits carried over from the first symbol (`EC_CODE_EXTRA`).
const CODE_EXTRA: u32 = (CODE_BITS - 2) % SYM_BITS + 1;
/// The most bits `dec_uint` reads through the range coder before raw bits (`EC_UINT_BITS`).
const UINT_BITS: u32 = 8;
/// Fractional bits of `tell_frac` (`BITRES`).
const BITRES: u32 = 3;
/// The raw-bit window's width in bits.
const WINDOW_BITS: u32 = 32;

/// A range decoder over one frame's bytes; raw bits are read from the frame's end.
#[derive(Clone)]
pub struct EcDec<'a> {
    buf: &'a [u8],
    /// The bytes in use: the frame less any trailing redundancy (`storage`).
    storage: usize,
    /// Bytes read from the front (`offs`) and from the back (`end_offs`).
    offs: usize,
    end_offs: usize,
    /// Raw bits read from the back and not yet used, and how many (`end_window`, `nend_bits`).
    end_window: u32,
    nend_bits: u32,
    /// Whole bits used so far by both coders (`nbits_total`).
    nbits_total: i32,
    rng: u32,
    val: u32,
    /// The scale of the last `decode` (`ext`).
    ext: u32,
    /// The last symbol read, of which `CODE_EXTRA` bits are not yet used (`rem`).
    rem: u32,
    error: bool,
}

impl<'a> EcDec<'a> {
    /// A decoder at the start of `buf` (`ec_dec_init`).
    #[must_use]
    pub fn new(buf: &'a [u8]) -> Self {
        let mut dec = Self {
            buf,
            storage: buf.len(),
            offs: 0,
            end_offs: 0,
            end_window: 0,
            nend_bits: 0,
            nbits_total: (CODE_BITS + 1 - ((CODE_BITS - CODE_EXTRA) / SYM_BITS) * SYM_BITS).cast_signed(),
            rng: 1 << CODE_EXTRA,
            val: 0,
            ext: 0,
            rem: 0,
            error: false,
        };
        dec.rem = u32::from(dec.read_byte());
        dec.val = dec.rng.wrapping_sub(1).wrapping_sub(dec.rem >> (SYM_BITS - CODE_EXTRA));
        dec.normalize();
        dec
    }

    /// Whole bits used so far (`ec_tell`).
    #[inline]
    #[must_use]
    pub fn tell(&self) -> i32 {
        self.nbits_total.wrapping_sub(ec_ilog(self.rng))
    }

    /// Bits used so far in eighths (`ec_tell_frac`).
    #[inline]
    #[must_use]
    pub fn tell_frac(&self) -> i32 {
        // The thresholds of the fractional part, 2^(k/8) in Q16 (RFC 6716 §4.1.4).
        const CORRECTION: [u32; 8] = [35733, 38967, 42495, 46340, 50535, 55109, 60097, 65535];
        let nbits = self.nbits_total.cast_unsigned() << BITRES;
        let l = bit_count(self.rng);
        // After normalisation the range has at least 24 bits, so its top 16 are r.
        let r = self.rng >> l.saturating_sub(16);
        let mut b = (r >> 12).wrapping_sub(8);
        let threshold = CORRECTION.get((b & 7) as usize).copied().unwrap_or(u32::MAX);
        b = b.wrapping_add(u32::from(r > threshold));
        // A frame's bits in eighths are far below 2^31.
        nbits.wrapping_sub((l << BITRES).wrapping_add(b)).cast_signed()
    }

    /// The range: after the last symbol, the final state, which an encoder and a decoder
    /// agree on (RFC 6716 §4.1.6.1).
    #[inline]
    #[must_use]
    pub fn rng(&self) -> u32 {
        self.rng
    }

    /// The bytes in use.
    #[inline]
    #[must_use]
    pub fn storage(&self) -> usize {
        self.storage
    }

    /// Count `bits` more as used: a silent CELT frame claims the rest of its bytes.
    pub fn add_nbits_total(&mut self, bits: i32) {
        self.nbits_total = self.nbits_total.wrapping_add(bits);
    }

    /// Hide `bytes` at the end of the frame from the decoder: a redundant frame that follows.
    pub fn shrink_storage(&mut self, bytes: usize) {
        self.storage = self.storage.saturating_sub(bytes);
        self.end_offs = self.end_offs.min(self.storage);
        self.offs = self.offs.min(self.storage);
    }

    /// The next byte from the front, or 0 past the end.
    #[inline]
    fn read_byte(&mut self) -> u8 {
        if self.offs < self.storage {
            let byte = self.buf.get(self.offs).copied().unwrap_or(0);
            self.offs = self.offs.saturating_add(1);
            byte
        } else {
            0
        }
    }

    /// The next byte from the back, or 0 once the front is reached.
    #[inline]
    fn read_byte_from_end(&mut self) -> u8 {
        if self.end_offs < self.storage {
            self.end_offs = self.end_offs.saturating_add(1);
            self.storage.checked_sub(self.end_offs).and_then(|at| self.buf.get(at)).copied().unwrap_or(0)
        } else {
            0
        }
    }

    /// Bring the range back above `CODE_BOT`, reading bytes as it goes (RFC 6716 §4.1.2.1).
    #[inline]
    fn normalize(&mut self) {
        while self.rng <= CODE_BOT {
            self.nbits_total = self.nbits_total.wrapping_add(SYM_BITS.cast_signed());
            self.rng <<= SYM_BITS;
            let mut sym = self.rem;
            self.rem = u32::from(self.read_byte());
            sym = ((sym << SYM_BITS) | self.rem) >> (SYM_BITS - CODE_EXTRA);
            self.val = ((self.val << SYM_BITS).wrapping_add(SYM_MAX & !sym)) & (CODE_TOP - 1);
        }
    }

    /// The cumulative frequency of the next symbol out of `ft` (`ec_decode`, RFC 6716 §4.1.2).
    /// The range is at least 2^23 after normalisation, so `ext` is never 0 for `ft` < 2^23.
    #[inline]
    pub fn decode(&mut self, ft: u32) -> u32 {
        self.ext = self.rng / NonZeroU32::new(ft).unwrap_or(NonZeroU32::MIN);
        let s = self.val.checked_div(self.ext).unwrap_or(u32::MAX);
        ft.saturating_sub(s.saturating_add(1))
    }

    /// `decode` with `ft` = 2^`bits` (`ec_decode_bin`).
    #[inline]
    pub fn decode_bin(&mut self, bits: u32) -> u32 {
        self.ext = self.rng >> bits;
        let s = self.val.checked_div(self.ext).unwrap_or(u32::MAX);
        let ft = 1u32 << bits;
        ft.saturating_sub(s.saturating_add(1))
    }

    /// Consume the symbol `decode` found, with cumulative frequencies `fl`..`fh` of `ft`
    /// (`ec_dec_update`, RFC 6716 §4.1.2).
    #[inline]
    pub fn update(&mut self, fl: u32, fh: u32, ft: u32) {
        let s = self.ext.wrapping_mul(ft.wrapping_sub(fh));
        self.val = self.val.wrapping_sub(s);
        self.rng = if fl > 0 { self.ext.wrapping_mul(fh.wrapping_sub(fl)) } else { self.rng.wrapping_sub(s) };
        self.normalize();
    }

    /// A bit that is 1 with probability 2^-`logp` (`ec_dec_bit_logp`, RFC 6716 §4.1.3.2).
    #[inline]
    pub fn dec_bit_logp(&mut self, logp: u32) -> bool {
        let r = self.rng;
        let d = self.val;
        let s = r >> logp;
        let one = d < s;
        if !one {
            self.val = d.wrapping_sub(s);
        }
        self.rng = if one { s } else { r.wrapping_sub(s) };
        self.normalize();
        one
    }

    /// A symbol from an inverse cumulative table of 8-bit entries with total 2^`ftb`
    /// (`ec_dec_icdf`, RFC 6716 §4.1.3.3). A table always ends in 0, which every value
    /// reaches; a table that did not would end the search as malformed.
    #[inline]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        reason = "every table has fewer than 256 entries"
    )]
    pub fn dec_icdf(&mut self, icdf: &[u8], ftb: u32) -> i32 {
        let d = self.val;
        let r = self.rng >> ftb;
        let mut t = self.rng;
        for (symbol, &p) in icdf.iter().enumerate() {
            let s = r.wrapping_mul(u32::from(p));
            if d >= s {
                self.val = d.wrapping_sub(s);
                self.rng = t.wrapping_sub(s);
                self.normalize();
                return symbol as i32;
            }
            t = s;
        }
        self.error = true;
        (icdf.len() as i32).wrapping_sub(1)
    }

    /// A value in 0..`ft` (`ec_dec_uint`, RFC 6716 §4.1.5): the top 8 bits through the range
    /// coder, the rest raw. `ft` is at least 2; less is malformed.
    pub fn dec_uint(&mut self, ft: u32) -> u32 {
        if ft <= 1 {
            self.error = true;
            return 0;
        }
        let top = ft.wrapping_sub(1);
        let bits = bit_count(top);
        if bits > UINT_BITS {
            let shift = bits.wrapping_sub(UINT_BITS);
            let fts = (top >> shift).wrapping_add(1);
            let s = self.decode(fts);
            self.update(s, s.wrapping_add(1), fts);
            let t = (s << shift) | self.dec_bits(shift);
            if t <= top {
                t
            } else {
                self.error = true;
                top
            }
        } else {
            let s = self.decode(ft);
            self.update(s, s.wrapping_add(1), ft);
            s
        }
    }

    /// `bits` raw bits from the end of the frame (`ec_dec_bits`, RFC 6716 §4.1.4), at most 25.
    #[inline]
    pub fn dec_bits(&mut self, bits: u32) -> u32 {
        let mut window = self.end_window;
        let mut available = self.nend_bits;
        if available < bits {
            while available <= WINDOW_BITS - SYM_BITS {
                window |= u32::from(self.read_byte_from_end()) << available;
                available = available.wrapping_add(SYM_BITS);
            }
        }
        let ret = window & 1u32.checked_shl(bits).unwrap_or(0).wrapping_sub(1);
        window = window.checked_shr(bits).unwrap_or(0);
        available = available.wrapping_sub(bits);
        self.end_window = window;
        self.nend_bits = available;
        self.nbits_total = self.nbits_total.wrapping_add(bits.cast_signed());
        ret
    }
}
