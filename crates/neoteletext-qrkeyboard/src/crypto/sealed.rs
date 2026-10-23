//! Sealed frames: AES-256-GCM with a counter nonce, each counter taken once
//! and in order (NIST SP 800-38D section 8.2.1).

use super::SECRET_BYTES;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};

/// AES-GCM's nonce: 96 bits (NIST SP 800-38D section 8.2).
pub const NONCE_BYTES: usize = 12;
/// AES-GCM's full-length tag (NIST SP 800-38D section 5.2.1.2).
pub const TAG_BYTES: usize = 16;
/// Where the counter starts in the nonce: after four zero bytes.
const COUNTER_OFFSET: usize = NONCE_BYTES - 8;
/// The sealed frame's tag (`TAG_SEALED`).
pub const TAG_SEALED: u8 = 0xE1;
/// A sealed frame's bytes beyond its plaintext.
pub const SEALED_OVERHEAD: usize = 1 + NONCE_BYTES + TAG_BYTES;

fn nonce(counter: u64) -> [u8; NONCE_BYTES] {
    let mut nonce = [0_u8; NONCE_BYTES];
    for (target, source) in nonce.iter_mut().skip(COUNTER_OFFSET).zip(counter.to_be_bytes()) {
        *target = source;
    }
    nonce
}

/// Seals this side's frames with counter nonces (`FrameSealer`).
pub struct Sealer {
    cipher: Aes256Gcm,
    counter: Option<u64>,
}

impl Sealer {
    pub(super) fn new(key: &[u8; SECRET_BYTES]) -> Self {
        Self { cipher: Aes256Gcm::new(key.into()), counter: Some(0) }
    }

    /// One sealed frame, or `None` once the 2⁶⁴ counters are spent.
    pub fn seal(&mut self, plaintext: &[u8]) -> Option<Vec<u8>> {
        let counter = self.counter?;
        self.counter = counter.checked_add(1);
        let nonce = nonce(counter);
        let sealed = self.cipher.encrypt(Nonce::from_slice(&nonce), plaintext).ok()?;
        let mut frame = Vec::with_capacity(sealed.len().saturating_add(1 + NONCE_BYTES));
        frame.push(TAG_SEALED);
        frame.extend_from_slice(&nonce);
        frame.extend_from_slice(&sealed);
        Some(frame)
    }
}

/// Opens the peer's frames, each counter once and only in increasing
/// order (`FrameOpener`).
pub struct Opener {
    cipher: Aes256Gcm,
    last: Option<u64>,
}

impl Opener {
    pub(super) fn new(key: &[u8; SECRET_BYTES]) -> Self {
        Self { cipher: Aes256Gcm::new(key.into()), last: None }
    }

    /// The plaintext, or `None` for a frame that is not sealed, is forged,
    /// or repeats or goes back on a counter.
    pub fn open(&mut self, frame: &[u8]) -> Option<Vec<u8>> {
        let (&tag, rest) = frame.split_first()?;
        if tag != TAG_SEALED || rest.len() < NONCE_BYTES {
            return None;
        }
        let (nonce, sealed) = rest.split_at(NONCE_BYTES);
        let (zeros, counter) = nonce.split_at(COUNTER_OFFSET);
        if zeros.iter().any(|&byte| byte != 0) {
            return None;
        }
        let counter = u64::from_be_bytes(counter.try_into().ok()?);
        if self.last.is_some_and(|last| counter <= last) {
            return None;
        }
        let plaintext = self.cipher.decrypt(Nonce::from_slice(nonce), sealed).ok()?;
        // Advanced only after authentication, so a forged frame cannot
        // spend a counter.
        self.last = Some(counter);
        Some(plaintext)
    }
}
