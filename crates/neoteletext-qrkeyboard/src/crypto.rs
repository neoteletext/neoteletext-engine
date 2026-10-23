//! The endpoints' cryptography, as qrkeyboard's `crypto.ts` defines it.
//!
//! - **Directional keys.** HKDF-SHA-256 over the 32-byte pairing secret,
//!   salted with the device's and then the phone's fresh 16-byte salts,
//!   gives one AES-256-GCM key each way (`qrkb/p2d/v2`, `qrkb/d2p/v2`).
//! - **Sealed frames.** Tag `0xE1`, a 12-byte nonce of four zero bytes and
//!   a 64-bit counter (NIST SP 800-38D section 8.2.1), the ciphertext and
//!   its 16-byte tag. A receiver takes each counter once and in order.
//! - **SPAKE2** (RFC 9382) on P-256 for pairing with a PIN: the device is
//!   A and uses M, the phone is B and uses N; the transcript binds the
//!   join code and the configuration hash, and HKDF derives the secret and
//!   the confirmation key.

use hkdf::Hkdf;
use sha2::Sha256;

use crate::Error;

mod sealed;
mod spake2;

pub use sealed::{NONCE_BYTES, Opener, SEALED_OVERHEAD, Sealer, TAG_BYTES, TAG_SEALED};
pub use spake2::{Spake2, Spake2Result, password_scalar, random_scalar};

/// The pairing secret's bytes (`SECRET_BYTES`).
pub const SECRET_BYTES: usize = 32;
/// Each endpoint's salt (`SALT_BYTES`).
pub const SALT_BYTES: usize = 16;
/// An uncompressed P-256 point: 0x04, then x and y.
pub const SHARE_BYTES: usize = 65;
/// A confirmation: HMAC-SHA-256.
pub const CONFIRM_BYTES: usize = 32;

/// Which end of the pairing this is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// The screen: RFC 9382's A.
    Device,
    /// The phone: RFC 9382's B.
    Phone,
}

pub(super) fn hkdf(salt: &[u8], secret: &[u8], info: &str) -> [u8; SECRET_BYTES] {
    let mut out = [0_u8; SECRET_BYTES];
    // 32 bytes is far below HKDF-SHA-256's limit of 255 blocks.
    let _ = Hkdf::<Sha256>::new(Some(salt), secret).expand(info.as_bytes(), &mut out);
    out
}

/// The two keys of a connection: `salt` is the device's salt then the
/// phone's (`deriveDirectionalKeys`).
#[must_use]
pub fn directional_keys(secret: &[u8; SECRET_BYTES], role: Role, salt: &[u8; 2 * SALT_BYTES]) -> (Sealer, Opener) {
    let phone_to_device = hkdf(salt, secret, "qrkb/p2d/v2");
    let device_to_phone = hkdf(salt, secret, "qrkb/d2p/v2");
    let (send, receive) = match role {
        Role::Device => (device_to_phone, phone_to_device),
        Role::Phone => (phone_to_device, device_to_phone),
    };
    (Sealer::new(&send), Opener::new(&receive))
}

/// Whether two byte strings are equal, in time that does not depend on
/// where they differ.
#[must_use]
pub fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && left.iter().zip(right).fold(0_u8, |difference, (a, b)| difference | (a ^ b)) == 0
}

/// Random bytes from the operating system.
///
/// # Errors
/// When the operating system has none to give.
pub fn random(bytes: &mut [u8]) -> Result<(), Error> {
    getrandom::getrandom(bytes).map_err(|error| Error::Network(format!("no random numbers: {error}")))
}

#[cfg(test)]
mod tests;
