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

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use p256::elliptic_curve::ops::Reduce;
use p256::elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
use p256::{AffinePoint, EncodedPoint, FieldBytes, ProjectivePoint, Scalar, U256};
use sha2::{Digest, Sha256};

use crate::Error;

/// The pairing secret's bytes (`SECRET_BYTES`).
pub const SECRET_BYTES: usize = 32;
/// Each endpoint's salt (`SALT_BYTES`).
pub const SALT_BYTES: usize = 16;
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
/// An uncompressed P-256 point: 0x04, then x and y.
pub const SHARE_BYTES: usize = 65;
/// A confirmation: HMAC-SHA-256.
pub const CONFIRM_BYTES: usize = 32;
/// A P-256 scalar's bytes.
const SCALAR_BYTES: usize = 32;

/// RFC 9382 section 6: P-256's M and N, compressed.
const SPAKE2_M: &str = "02886e2f97ace46e55ba9dd7242579f2993b64e16ef3dcab95afd497333d8fa12f";
const SPAKE2_N: &str = "03d8bbd6c639c62937b04d997f38c3770719c629d7014d49a24b4f98baa1292b49";

/// Which end of the pairing this is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// The screen: RFC 9382's A.
    Device,
    /// The phone: RFC 9382's B.
    Phone,
}

fn hkdf(salt: &[u8], secret: &[u8], info: &str) -> [u8; SECRET_BYTES] {
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
    fn new(key: &[u8; SECRET_BYTES]) -> Self {
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
    fn new(key: &[u8; SECRET_BYTES]) -> Self {
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

/// Whether two byte strings are equal, in time that does not depend on
/// where they differ.
#[must_use]
pub fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && left.iter().zip(right).fold(0_u8, |difference, (a, b)| difference | (a ^ b)) == 0
}

fn point(hex: &str) -> Option<ProjectivePoint> {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|at| hex.get(at..at.saturating_add(2)).and_then(|pair| u8::from_str_radix(pair, 16).ok()))
        .collect::<Option<_>>()?;
    decode_point(&bytes)
}

fn decode_point(bytes: &[u8]) -> Option<ProjectivePoint> {
    let encoded = EncodedPoint::from_bytes(bytes).ok()?;
    let affine: Option<AffinePoint> = AffinePoint::from_encoded_point(&encoded).into();
    let projective = ProjectivePoint::from(affine?);
    // The identity is no share.
    (projective != ProjectivePoint::IDENTITY).then_some(projective)
}

fn encode(point: &ProjectivePoint) -> Vec<u8> {
    point.to_affine().to_encoded_point(false).as_bytes().to_vec()
}

fn reduce(bytes: &[u8; SCALAR_BYTES]) -> Scalar {
    <Scalar as Reduce<U256>>::reduce_bytes(FieldBytes::from_slice(bytes))
}

/// The password scalar of a code and a PIN: SHA-256 of
/// `qrkb/spake2/v1:<code>:<pin>`, reduced, and 1 in place of 0.
#[must_use]
pub fn password_scalar(code: &str, pin: &str) -> Scalar {
    let digest: [u8; SCALAR_BYTES] = Sha256::digest(format!("qrkb/spake2/v1:{code}:{pin}").as_bytes()).into();
    let scalar = reduce(&digest);
    if scalar == Scalar::ZERO { Scalar::ONE } else { scalar }
}

/// A random scalar: 256 random bits reduced, never 0. The bias is below
/// 2⁻³² for P-256, whose order is within 2⁻³² of 2²⁵⁶.
///
/// # Errors
/// When the operating system has no random numbers.
pub fn random_scalar() -> Result<Scalar, Error> {
    loop {
        let mut bytes = [0_u8; SCALAR_BYTES];
        crate::random(&mut bytes)?;
        let scalar = reduce(&bytes);
        if scalar != Scalar::ZERO {
            return Ok(scalar);
        }
    }
}

/// RFC 9382 section 3.2: each part of the transcript after its length as
/// eight bytes, little-endian.
fn length_prefixed(transcript: &mut Vec<u8>, part: &[u8]) {
    transcript.extend_from_slice(&(part.len() as u64).to_le_bytes());
    transcript.extend_from_slice(part);
}

/// One side of a SPAKE2 run.
pub struct Spake2 {
    role: Role,
    code: String,
    config_hash: Vec<u8>,
    password: Scalar,
    ephemeral: Scalar,
    share: Vec<u8>,
}

/// What a finished run gives.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Spake2Result {
    /// The pairing secret.
    pub secret: [u8; SECRET_BYTES],
    /// The confirmation this side sends.
    pub confirm_out: [u8; CONFIRM_BYTES],
    /// The confirmation the peer must send.
    pub confirm_expected: [u8; CONFIRM_BYTES],
}

impl Spake2 {
    /// Starts a run with `ephemeral` as this side's secret scalar
    /// (`spake2Start`, which draws it at random).
    #[must_use]
    #[allow(clippy::arithmetic_side_effects, reason = "operations of the P-256 group, which are modular")]
    pub fn start(role: Role, code: &str, pin: &str, config_hash: &[u8], ephemeral: Scalar) -> Option<Self> {
        let password = password_scalar(code, pin);
        let base = point(if role == Role::Device { SPAKE2_M } else { SPAKE2_N })?;
        let share = encode(&(ProjectivePoint::GENERATOR * ephemeral + base * password));
        Some(Self { role, code: code.to_owned(), config_hash: config_hash.to_vec(), password, ephemeral, share })
    }

    /// This side's share, to send.
    #[must_use]
    pub fn share(&self) -> &[u8] {
        &self.share
    }

    /// Finishes with the peer's share (`spake2Finish`), or `None` when it
    /// is not a point of the curve.
    #[must_use]
    #[allow(clippy::arithmetic_side_effects, reason = "operations of the P-256 group, which are modular")]
    pub fn finish(&self, peer_share: &[u8]) -> Option<Spake2Result> {
        let peer = decode_point(peer_share)?;
        let peer_base = point(if self.role == Role::Device { SPAKE2_N } else { SPAKE2_M })?;
        let shared = encode(&((peer - peer_base * self.password) * self.ephemeral));
        let peer_encoded = encode(&peer);
        let (device_share, phone_share) =
            if self.role == Role::Device { (&self.share, &peer_encoded) } else { (&peer_encoded, &self.share) };
        let mut transcript = Vec::new();
        length_prefixed(&mut transcript, b"qrkb/spake2/v2");
        length_prefixed(&mut transcript, self.code.as_bytes());
        length_prefixed(&mut transcript, &self.config_hash);
        length_prefixed(&mut transcript, device_share);
        length_prefixed(&mut transcript, phone_share);
        length_prefixed(&mut transcript, &shared);
        length_prefixed(&mut transcript, &self.password.to_bytes());
        let hash = Sha256::digest(&transcript);
        let secret = hkdf(&[], &hash, "qrkb/ke");
        let confirmation = hkdf(&[], &hash, "qrkb/kc");
        let confirm = |label: &[u8]| -> Option<[u8; CONFIRM_BYTES]> {
            let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&confirmation).ok()?;
            mac.update(label);
            Some(mac.finalize().into_bytes().into())
        };
        let (device, phone) = (confirm(b"CA")?, confirm(b"CB")?);
        let (confirm_out, confirm_expected) = if self.role == Role::Device { (device, phone) } else { (phone, device) };
        Some(Spake2Result { secret, confirm_out, confirm_expected })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.len()).step_by(2).map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap()).collect()
    }

    fn scalar(byte: u8) -> Scalar {
        reduce(&[byte; SCALAR_BYTES])
    }

    /// Computed by qrkeyboard's own `crypto.ts` under Node on 20 October
    /// 2026, with the same fixed ephemeral scalars.
    #[test]
    fn spake2_agrees_with_qrkeyboard() {
        let hash: Vec<u8> = (0..16).collect();
        assert_eq!(
            hex(&password_scalar("ABC123", "4321").to_bytes()),
            "d8a945ce46575888468a289c521e86393939cd4cec47495ee70aed471bf7ffba"
        );
        let device = Spake2::start(Role::Device, "ABC123", "4321", &hash, scalar(0x11)).unwrap();
        let phone = Spake2::start(Role::Phone, "ABC123", "4321", &hash, scalar(0x22)).unwrap();
        assert_eq!(
            hex(device.share()),
            "045fb74d61083faaf1b94218c71f81c6a57b921f76858f94fac5248eeb604550f27a24bea8c177571f0284d8eacc4d145c9095d7d29abf7bfa6cd61c4dd18d8426"
        );
        assert_eq!(
            hex(phone.share()),
            "049362396448e4ee333ecc215b58fb06594f13255fdd91236c19d3ecfc2c11fd633866aa5aac5b5c3ed248c2b301ef2f21be0fd08b17202563bff91ef3baad20b5"
        );
        let a = device.finish(phone.share()).unwrap();
        let b = phone.finish(device.share()).unwrap();
        assert_eq!(hex(&a.secret), "67ea5d825640e4336f82c4db1ec4aca2265e3e200cdae34c3255dee9632869a2");
        assert_eq!(a.secret, b.secret);
        assert_eq!(hex(&a.confirm_out), "b3809b21b20f504ce514468cb33c2c7fb33fba1b993716207dda25fcb9c908c7");
        assert_eq!(hex(&b.confirm_out), "b5e3a81384d1d0b0b66401e2e413b8f13d3fca44bf32d3d299278d527f578298");
        assert_eq!((a.confirm_expected, b.confirm_expected), (b.confirm_out, a.confirm_out));
    }

    #[test]
    fn a_wrong_pin_fails_confirmation_and_a_bad_share_is_refused() {
        let hash = [0_u8; 16];
        let device = Spake2::start(Role::Device, "C", "1111", &hash, scalar(3)).unwrap();
        let phone = Spake2::start(Role::Phone, "C", "2222", &hash, scalar(4)).unwrap();
        let (a, b) = (device.finish(phone.share()).unwrap(), phone.finish(device.share()).unwrap());
        assert_ne!(a.secret, b.secret);
        assert!(!constant_time_equal(&a.confirm_expected, &b.confirm_out));
        assert!(device.finish(&[4; SHARE_BYTES]).is_none(), "not on the curve");
        assert!(device.finish(&[0]).is_none(), "the identity");
    }

    /// Sealed by qrkeyboard's `FrameSealer` with keys from
    /// `deriveDirectionalKeys` over the same secret and salts.
    #[test]
    fn frames_agree_with_qrkeyboard_and_refuse_replay() {
        let secret: [u8; 32] = core::array::from_fn(|index| 0x40 + index as u8);
        let salt: [u8; 32] = core::array::from_fn(|index| 0x80 + index as u8);
        let (mut device_send, mut device_open) = directional_keys(&secret, Role::Device, &salt);
        let (mut phone_send, mut phone_open) = directional_keys(&secret, Role::Phone, &salt);
        let plaintext = [0x10, b'{', b'}'];
        let frame = device_send.seal(&plaintext).unwrap();
        assert_eq!(hex(&frame), "e1000000000000000000000000a07f153c5fcbfdedc62359dd2b767ba04f9a31");
        let back = phone_send.seal(&plaintext).unwrap();
        assert_eq!(hex(&back), "e10000000000000000000000002bb1b63a9753afd166378a5fc99dd62467c1bb");
        assert_eq!(phone_open.open(&frame).unwrap(), plaintext);
        assert_eq!(phone_open.open(&frame), None, "a counter is taken once");
        let mut forged = back.clone();
        *forged.last_mut().unwrap() ^= 1;
        assert_eq!(device_open.open(&forged), None, "a forged frame spends no counter");
        assert_eq!(device_open.open(&back).unwrap(), plaintext);
        let second = phone_send.seal(&plaintext).unwrap();
        let third = phone_send.seal(&plaintext).unwrap();
        assert!(device_open.open(&third).is_some());
        assert_eq!(device_open.open(&second), None, "never backwards");
        assert_eq!(bytes("e1"), [TAG_SEALED]);
    }

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write;
        bytes.iter().fold(String::new(), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
    }
}
