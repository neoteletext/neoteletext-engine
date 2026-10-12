//! SHA-256 (FIPS 180-4), which names fonts: a font is fetched, checked and
//! cached by its digest (REQ-FNT-003, REQ-FNT-009). Written in the project,
//! as SHA-1 is (decision E.38).

use crate::bytes::copy_prefix;
use crate::consts::{SHA256_BLOCK_BYTES, SHA256_DIGEST_BYTES, SHA256_HEX_DIGITS};

/// Initial hash value H(0) (FIPS 180-4 section 5.3.3).
const INITIAL: [u32; 8] =
    [0x6A09_E667, 0xBB67_AE85, 0x3C6E_F372, 0xA54F_F53A, 0x510E_527F, 0x9B05_688C, 0x1F83_D9AB, 0x5BE0_CD19];
/// The round constants K (FIPS 180-4 section 4.2.2), eight to a line as
/// the standard prints them.
#[rustfmt::skip]
const ROUND_CONSTANTS: [u32; 64] = [
    0x428A_2F98, 0x7137_4491, 0xB5C0_FBCF, 0xE9B5_DBA5, 0x3956_C25B, 0x59F1_11F1, 0x923F_82A4, 0xAB1C_5ED5,
    0xD807_AA98, 0x1283_5B01, 0x2431_85BE, 0x550C_7DC3, 0x72BE_5D74, 0x80DE_B1FE, 0x9BDC_06A7, 0xC19B_F174,
    0xE49B_69C1, 0xEFBE_4786, 0x0FC1_9DC6, 0x240C_A1CC, 0x2DE9_2C6F, 0x4A74_84AA, 0x5CB0_A9DC, 0x76F9_88DA,
    0x983E_5152, 0xA831_C66D, 0xB003_27C8, 0xBF59_7FC7, 0xC6E0_0BF3, 0xD5A7_9147, 0x06CA_6351, 0x1429_2967,
    0x27B7_0A85, 0x2E1B_2138, 0x4D2C_6DFC, 0x5338_0D13, 0x650A_7354, 0x766A_0ABB, 0x81C2_C92E, 0x9272_2C85,
    0xA2BF_E8A1, 0xA81A_664B, 0xC24B_8B70, 0xC76C_51A3, 0xD192_E819, 0xD699_0624, 0xF40E_3585, 0x106A_A070,
    0x19A4_C116, 0x1E37_6C08, 0x2748_774C, 0x34B0_BCB5, 0x391C_0CB3, 0x4ED8_AA4A, 0x5B9C_CA4F, 0x682E_6FF3,
    0x748F_82EE, 0x78A5_636F, 0x84C8_7814, 0x8CC7_0208, 0x90BE_FFFA, 0xA450_6CEB, 0xBEF9_A3F7, 0xC671_78F2,
];
/// Words in the message schedule (FIPS 180-4 section 6.2.2 step 1).
const SCHEDULE_WORDS: usize = 64;
/// Message words in one block.
const BLOCK_WORDS: usize = 16;
/// Bytes of the message length appended in the padding (FIPS 180-4
/// section 5.1.1).
const LENGTH_BYTES: usize = 8;
/// The padding's first byte: a single 1 bit (FIPS 180-4 section 5.1.1).
const PAD_FIRST: u8 = 0x80;

/// A SHA-256 digest.
pub type Digest = [u8; SHA256_DIGEST_BYTES];

/// An incremental SHA-256 computation.
#[derive(Clone, Debug)]
pub struct Sha256 {
    state: [u32; 8],
    block: [u8; SHA256_BLOCK_BYTES],
    block_len: usize,
    total_bytes: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    /// A new computation.
    #[must_use]
    pub const fn new() -> Self {
        Self { state: INITIAL, block: [0; SHA256_BLOCK_BYTES], block_len: 0, total_bytes: 0 }
    }

    /// Adds message bytes.
    pub fn update(&mut self, mut data: &[u8]) {
        self.total_bytes = self.total_bytes.wrapping_add(data.len() as u64);
        while !data.is_empty() {
            let take = copy_prefix(self.block.get_mut(self.block_len..).unwrap_or(&mut []), data);
            self.block_len = self.block_len.saturating_add(take);
            data = data.get(take..).unwrap_or(&[]);
            if self.block_len == SHA256_BLOCK_BYTES {
                let block = self.block;
                self.compress(&block);
                self.block_len = 0;
            }
        }
    }

    /// Finishes and returns the 32-byte digest.
    #[must_use]
    pub fn finish(mut self) -> Digest {
        let bit_length = self.total_bytes.wrapping_mul(8);
        self.update_padding(&[PAD_FIRST]);
        while self.block_len != SHA256_BLOCK_BYTES.saturating_sub(LENGTH_BYTES) {
            self.update_padding(&[0]);
        }
        self.update_padding(&bit_length.to_be_bytes());
        let mut digest = [0u8; SHA256_DIGEST_BYTES];
        for (chunk, word) in digest.as_chunks_mut::<4>().0.iter_mut().zip(self.state) {
            *chunk = word.to_be_bytes();
        }
        digest
    }

    /// Like `update`, without counting the bytes into the message length.
    fn update_padding(&mut self, data: &[u8]) {
        let total = self.total_bytes;
        self.update(data);
        self.total_bytes = total;
    }

    // The working variables keep the single-letter names of FIPS 180-4
    // section 6.2.2, so the code can be checked against it line by line.
    #[allow(clippy::many_single_char_names)]
    fn compress(&mut self, block: &[u8; SHA256_BLOCK_BYTES]) {
        let mut schedule = [0u32; SCHEDULE_WORDS];
        for (word, bytes) in schedule.iter_mut().zip(block.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*bytes);
        }
        for t in BLOCK_WORDS..SCHEDULE_WORDS {
            let w = |offset: usize| schedule.get(t.wrapping_sub(offset)).copied().unwrap_or(0);
            let sigma0 = w(15).rotate_right(7) ^ w(15).rotate_right(18) ^ (w(15) >> 3);
            let sigma1 = w(2).rotate_right(17) ^ w(2).rotate_right(19) ^ (w(2) >> 10);
            let value = sigma1.wrapping_add(w(7)).wrapping_add(sigma0).wrapping_add(w(16));
            if let Some(slot) = schedule.get_mut(t) {
                *slot = value;
            }
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for (&word, &k) in schedule.iter().zip(&ROUND_CONSTANTS) {
            let big_sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ (!e & g);
            let t1 = h.wrapping_add(big_sigma1).wrapping_add(choose).wrapping_add(k).wrapping_add(word);
            let big_sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = big_sigma0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (state, value) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *state = state.wrapping_add(value);
        }
    }
}

/// The SHA-256 digest of `data`.
#[must_use]
pub fn digest(data: &[u8]) -> Digest {
    let mut sha = Sha256::new();
    sha.update(data);
    sha.finish()
}

/// A digest written as 64 hexadecimal digits, upper or lower case
/// (REQ-FNT-004, REQ-FNT-006, REQ-FNT-007); `None` for anything else.
#[must_use]
pub fn parse_hex(text: &[u8]) -> Option<Digest> {
    if text.len() != SHA256_HEX_DIGITS {
        return None;
    }
    let mut digest = [0u8; SHA256_DIGEST_BYTES];
    for (byte, pair) in digest.iter_mut().zip(text.as_chunks::<2>().0) {
        let high = hex_value(pair[0])?;
        let low = hex_value(pair[1])?;
        *byte = (high << 4) | low;
    }
    Some(digest)
}

/// A digest as 64 lower-case hexadecimal digits, as `sha256sum` writes it.
#[must_use]
pub fn to_hex(digest: &Digest) -> [u8; SHA256_HEX_DIGITS] {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; SHA256_HEX_DIGITS];
    for (pair, byte) in out.as_chunks_mut::<2>().0.iter_mut().zip(digest) {
        pair[0] = DIGITS.get(usize::from(byte >> 4)).copied().unwrap_or(b'0');
        pair[1] = DIGITS.get(usize::from(byte & 0x0F)).copied().unwrap_or(b'0');
    }
    out
}

/// The value of the letter digits a to f.
const HEX_LETTER_BASE: u8 = 10;

// Each arm's range bounds the subtraction, so wrapping never happens.
const fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit.wrapping_sub(b'0')),
        b'a'..=b'f' => Some(digit.wrapping_sub(b'a').wrapping_add(HEX_LETTER_BASE)),
        b'A'..=b'F' => Some(digit.wrapping_sub(b'A').wrapping_add(HEX_LETTER_BASE)),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn hex(digest: Digest) -> [u8; SHA256_HEX_DIGITS] {
        to_hex(&digest)
    }

    /// The examples of FIPS 180-4's companion document (NIST, "SHA-256
    /// examples"): one block, two blocks, and a million times "a".
    #[test]
    fn nist_vectors() {
        assert_eq!(&hex(digest(b"abc")), b"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(
            &hex(digest(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            b"248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let mut sha = Sha256::new();
        for _ in 0..1_000_000 / 64 {
            sha.update(&[b'a'; 64]);
        }
        assert_eq!(&hex(sha.finish()), b"cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    }

    #[test]
    fn empty_split_and_padding_boundaries() {
        assert_eq!(&hex(digest(b"")), b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        let mut sha = Sha256::new();
        sha.update(b"a");
        sha.update(b"bc");
        assert_eq!(sha.finish(), digest(b"abc"));
        // 55 and 56 bytes: the length fits in the first block, then not.
        assert_eq!(&hex(digest(&[b'a'; 55])), b"9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318");
        assert_eq!(&hex(digest(&[b'a'; 56])), b"b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a");
    }

    #[test]
    fn hex_round_trip_and_refusals() {
        let value = digest(b"abc");
        assert_eq!(parse_hex(&to_hex(&value)), Some(value));
        let upper = to_hex(&value).map(|digit| digit.to_ascii_uppercase());
        assert_eq!(parse_hex(&upper), Some(value), "either case");
        assert_eq!(parse_hex(&to_hex(&value)[..63]), None, "too short");
        let mut bad = to_hex(&value);
        bad[10] = b'g';
        assert_eq!(parse_hex(&bad), None, "not hexadecimal");
        assert_eq!(parse_hex(b""), None);
    }
}
