//! SPAKE2 (RFC 9382) on P-256, for pairing with a PIN.

use super::{CONFIRM_BYTES, Role, SECRET_BYTES, hkdf};
use crate::Error;
use hmac::{Hmac, Mac};
use p256::elliptic_curve::ops::Reduce;
use p256::elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
use p256::{AffinePoint, EncodedPoint, FieldBytes, ProjectivePoint, Scalar, U256};
use sha2::{Digest, Sha256};

/// A P-256 scalar's bytes.
pub(super) const SCALAR_BYTES: usize = 32;

/// RFC 9382 section 6: P-256's M and N, compressed.
const SPAKE2_M: &str = "02886e2f97ace46e55ba9dd7242579f2993b64e16ef3dcab95afd497333d8fa12f";
const SPAKE2_N: &str = "03d8bbd6c639c62937b04d997f38c3770719c629d7014d49a24b4f98baa1292b49";

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

pub(super) fn reduce(bytes: &[u8; SCALAR_BYTES]) -> Scalar {
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
