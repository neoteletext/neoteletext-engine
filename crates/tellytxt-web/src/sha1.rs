//! SHA-1 (RFC 3174), used only to check the WebSocket handshake's
//! `Sec-WebSocket-Accept` value (RFC 6455 section 4.2.2). It provides no
//! security here: RFC 6455 uses it as a checksum that proves the server
//! understood the handshake. Written in the project (decision E.38).

use crate::consts::{SHA1_BLOCK_BYTES, SHA1_DIGEST_BYTES};

/// Initial hash value H0 to H4 (RFC 3174 section 6.1).
const INITIAL: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
/// Round constants K for rounds 0-19, 20-39, 40-59 and 60-79 (RFC 3174
/// section 5).
const ROUND_CONSTANTS: [u32; 4] = [0x5A82_7999, 0x6ED9_EBA1, 0x8F1B_BCDC, 0xCA62_C1D6];
/// Words in the message schedule (RFC 3174 section 6.1 step b).
const SCHEDULE_WORDS: usize = 80;
/// Rounds per round constant.
const ROUNDS_PER_CONSTANT: usize = 20;
/// Bytes of the message length appended in the padding (RFC 3174 section 4).
const LENGTH_BYTES: usize = 8;
/// The padding's first byte: a single 1 bit (RFC 3174 section 4 item a).
const PAD_FIRST: u8 = 0x80;

/// An incremental SHA-1 computation.
#[derive(Clone, Debug)]
pub struct Sha1 {
    state: [u32; 5],
    block: [u8; SHA1_BLOCK_BYTES],
    block_len: usize,
    total_bytes: u64,
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha1 {
    /// A new computation.
    #[must_use]
    pub const fn new() -> Self {
        Self { state: INITIAL, block: [0; SHA1_BLOCK_BYTES], block_len: 0, total_bytes: 0 }
    }

    /// Adds message bytes.
    pub fn update(&mut self, mut data: &[u8]) {
        self.total_bytes = self.total_bytes.wrapping_add(data.len() as u64);
        while !data.is_empty() {
            let room = SHA1_BLOCK_BYTES.saturating_sub(self.block_len);
            let take = room.min(data.len());
            let (head, tail) = data.split_at(take);
            if let Some(target) = self.block.get_mut(self.block_len..self.block_len.saturating_add(take)) {
                target.copy_from_slice(head);
            }
            self.block_len = self.block_len.saturating_add(take);
            data = tail;
            if self.block_len == SHA1_BLOCK_BYTES {
                let block = self.block;
                self.compress(&block);
                self.block_len = 0;
            }
        }
    }

    /// Finishes and returns the 20-byte digest.
    #[must_use]
    pub fn finish(mut self) -> [u8; SHA1_DIGEST_BYTES] {
        let bit_length = self.total_bytes.wrapping_mul(8);
        self.update_padding(&[PAD_FIRST]);
        while self.block_len != SHA1_BLOCK_BYTES.saturating_sub(LENGTH_BYTES) {
            self.update_padding(&[0]);
        }
        self.update_padding(&bit_length.to_be_bytes());
        let mut digest = [0u8; SHA1_DIGEST_BYTES];
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

    // The working variables keep the single-letter names of RFC 3174
    // section 6.1, so the code can be checked against it line by line.
    #[allow(clippy::many_single_char_names)]
    fn compress(&mut self, block: &[u8; SHA1_BLOCK_BYTES]) {
        let mut schedule = [0u32; SCHEDULE_WORDS];
        for (word, bytes) in schedule.iter_mut().zip(block.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*bytes);
        }
        for t in 16..SCHEDULE_WORDS {
            let w = |offset: usize| schedule.get(t.wrapping_sub(offset)).copied().unwrap_or(0);
            let value = (w(3) ^ w(8) ^ w(14) ^ w(16)).rotate_left(1);
            if let Some(slot) = schedule.get_mut(t) {
                *slot = value;
            }
        }
        let [mut a, mut b, mut c, mut d, mut e] = self.state;
        for (t, &word) in schedule.iter().enumerate() {
            let group = t / ROUNDS_PER_CONSTANT;
            let f = match group {
                0 => (b & c) | (!b & d),
                2 => (b & c) | (b & d) | (c & d),
                _ => b ^ c ^ d,
            };
            let k = ROUND_CONSTANTS.get(group).copied().unwrap_or(0);
            let temp = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(word).wrapping_add(k);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        for (state, value) in self.state.iter_mut().zip([a, b, c, d, e]) {
            *state = state.wrapping_add(value);
        }
    }
}

/// The SHA-1 digest of the concatenation of `parts`.
#[must_use]
pub fn digest(parts: &[&[u8]]) -> [u8; SHA1_DIGEST_BYTES] {
    let mut sha = Sha1::new();
    for part in parts {
        sha.update(part);
    }
    sha.finish()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;

    fn hex(digest: [u8; 20]) -> [u8; 40] {
        let mut out = [0u8; 40];
        for (i, byte) in digest.iter().enumerate() {
            out[2 * i] = tellytxt_core::types::hex_digit(byte >> 4);
            out[2 * i + 1] = tellytxt_core::types::hex_digit(*byte);
        }
        out
    }

    /// The four tests of RFC 3174 section 7.3.
    #[test]
    fn rfc3174_vectors() {
        assert_eq!(&hex(digest(&[b"abc"])), b"A9993E364706816ABA3E25717850C26C9CD0D89D");
        assert_eq!(
            &hex(digest(&[b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"])),
            b"84983E441C3BD26EBAAE4AA1F95129E5E54670F1"
        );
        let mut sha = Sha1::new();
        for _ in 0..1_000_000 / 64 {
            sha.update(&[b'a'; 64]);
        }
        assert_eq!(&hex(sha.finish()), b"34AA973CD4C4DAA4F61EEB2BDBAD27316534016F");
        let mut sha = Sha1::new();
        for _ in 0..10 {
            sha.update(b"0123456701234567012345670123456701234567012345670123456701234567");
        }
        assert_eq!(&hex(sha.finish()), b"DEA356A2CDDD90C7A7ECEDC5EBB563934F460452");
    }

    #[test]
    fn empty_and_split_input() {
        assert_eq!(&hex(digest(&[])), b"DA39A3EE5E6B4B0D3255BFEF95601890AFD80709");
        assert_eq!(digest(&[b"a", b"bc"]), digest(&[b"abc"]));
    }
}
