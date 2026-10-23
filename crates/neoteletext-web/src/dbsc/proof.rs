//! The proof a request carries (section 9.10): a JWS in compact form,
//! written without allocation and signed by the port's key.

use super::{ALGORITHM, DeviceKeys, KeyHandle, PROOF_TYPE, PublicKey};
use crate::bytes::{Overflow, Writer};

/// Why a proof could not be written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProofError {
    /// It did not fit.
    Overflow,
    /// The port could not sign.
    Signing,
}

impl From<Overflow> for ProofError {
    fn from(Overflow: Overflow) -> Self {
        Self::Overflow
    }
}

/// The base64url alphabet without padding (RFC 4648 section 5), as JWS
/// uses it (RFC 7515 section 2).
const BASE64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
/// Bytes in a group of four base64 characters.
const BASE64_GROUP: usize = 3;
/// Where each byte of a group sits in its 24 bits, and where each of the
/// four characters' six bits sit (RFC 4648 section 4).
const BYTE_SHIFTS: [u32; BASE64_GROUP] = [16, 8, 0];
const SEXTET_SHIFTS: [u32; BASE64_GROUP + 1] = [18, 12, 6, 0];
const SEXTET_MASK: u32 = 0x3F;

/// Writes `bytes` in base64url without padding: a group of n bytes, read as
/// the high bits of 24, gives n + 1 characters.
fn base64url(bytes: &[u8], out: &mut Writer<'_>) -> Result<(), Overflow> {
    for group in bytes.chunks(BASE64_GROUP) {
        let value =
            group.iter().zip(BYTE_SHIFTS).fold(0_u32, |value, (&byte, shift)| value | (u32::from(byte) << shift));
        for shift in SEXTET_SHIFTS.iter().take(group.len().saturating_add(1)) {
            let sextet = (value >> shift) & SEXTET_MASK;
            out.byte(BASE64URL.get(sextet as usize).copied().unwrap_or(b'A'))?;
        }
    }
    Ok(())
}

/// Writes `text` as the inside of a JSON string: `"` and `\` escaped, as
/// the sf-strings it comes from hold only printable ASCII.
fn json_text(text: &[u8], out: &mut Writer<'_>) -> Result<(), Overflow> {
    for &byte in text {
        if byte == b'"' || byte == b'\\' {
            out.byte(b'\\')?;
        }
        out.byte(byte)?;
    }
    Ok(())
}

/// Room for a proof's header and payload before they are encoded.
const PROOF_JSON_BYTES: usize = 1024;

/// Writes a proof (section 9.10) to `out`: a JWS in compact form, its
/// header `{"alg":"ES256","typ":"dbsc+jwt"}` with the public key as `jwk`
/// at registration, its payload `{"jti": challenge}` (`{}` when the
/// server gave no challenge) with `authorization` when given, signed by
/// `key` through `keys`.
///
/// # Errors
/// [`ProofError`].
pub fn write_proof(
    keys: &mut dyn DeviceKeys,
    key: KeyHandle,
    public: Option<&PublicKey>,
    challenge: Option<&[u8]>,
    authorization: Option<&[u8]>,
    out: &mut Writer<'_>,
) -> Result<(), ProofError> {
    let start = out.len();
    let mut json = [0_u8; PROOF_JSON_BYTES];
    let mut header = Writer::new(&mut json);
    header.put(b"{\"alg\":\"")?;
    header.put(ALGORITHM)?;
    header.put(b"\",\"typ\":\"")?;
    header.put(PROOF_TYPE)?;
    header.byte(b'"')?;
    if let Some(public) = public {
        // RFC 7518 section 6.2.1: an elliptic-curve public key.
        header.put(b",\"jwk\":{\"kty\":\"EC\",\"crv\":\"P-256\",\"x\":\"")?;
        base64url(&public.x, &mut header)?;
        header.put(b"\",\"y\":\"")?;
        base64url(&public.y, &mut header)?;
        header.put(b"\"}")?;
    }
    header.byte(b'}')?;
    base64url(header.written(), out)?;
    out.byte(b'.')?;
    let mut json = [0_u8; PROOF_JSON_BYTES];
    let mut payload = Writer::new(&mut json);
    payload.byte(b'{')?;
    if let Some(challenge) = challenge {
        payload.put(b"\"jti\":\"")?;
        json_text(challenge, &mut payload)?;
        payload.byte(b'"')?;
    }
    if let Some(authorization) = authorization {
        if challenge.is_some() {
            payload.byte(b',')?;
        }
        payload.put(b"\"authorization\":\"")?;
        json_text(authorization, &mut payload)?;
        payload.byte(b'"')?;
    }
    payload.byte(b'}')?;
    base64url(payload.written(), out)?;
    let signing_input = out.written().get(start..).unwrap_or(&[]);
    let signature = keys.sign(key, signing_input).ok_or(ProofError::Signing)?;
    out.byte(b'.')?;
    base64url(&signature, out)?;
    Ok(())
}
