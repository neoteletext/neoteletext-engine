//! Device Bound Session Credentials (REQ-WEB-017, decision E.110), the
//! W3C Web Application Security Working Group's Editor's Draft of 8
//! September 2026 (<https://w3c.github.io/webappsec-dbsc/>), sans-IO and
//! without allocation: the headers and the session instructions read, the
//! proofs written, and the sessions kept. The agent (`agent`) makes the
//! requests; the port keeps the keys ([`DeviceKeys`]), which never leave
//! the device: this crate sees only their public halves and signatures.
//!
//! A server asks for a session with `Secure-Session-Registration` on a
//! response; the agent creates a key and posts a proof to the path named.
//! The server's answer, the session instructions, names the cookies the
//! session keeps and the address that refreshes them. When a request in
//! the session's scope would go without one of those cookies, the agent
//! first asks the refresh address for new ones, with a proof signed by
//! the same key, and then sends the request.

mod headers;
mod instructions;
mod proof;
mod session;
mod structured;

pub use headers::{Challenge, Registration, read_challenge, read_registration};
pub use instructions::{Answer, Instructions, InstructionsError, ScopeRule, read_instructions};
pub use proof::{ProofError, write_proof};
pub use session::{Session, Sessions};

/// The registration header (section 9.2).
pub const REGISTRATION_HEADER: &[u8] = b"secure-session-registration";
/// The challenge header (section 9.4).
pub const CHALLENGE_HEADER: &[u8] = b"secure-session-challenge";
/// The proof a request carries (section 9.10).
pub const RESPONSE_HEADER: &[u8] = b"Secure-Session-Response";
/// The session a refresh names (section 9.9).
pub const SESSION_ID_HEADER: &[u8] = b"Sec-Secure-Session-Id";
/// The JWT type of every proof (section 9.10).
pub const PROOF_TYPE: &[u8] = b"dbsc+jwt";
/// The one algorithm offered: ECDSA on P-256 with SHA-256 (RFC 7518
/// section 3.4), which every platform's protected key storage has.
pub const ALGORITHM: &[u8] = b"ES256";

/// Sessions kept at once; a registration with every slot taken is not
/// made.
pub const MAX_SESSIONS: usize = 4;
/// The longest session identifier kept.
pub const MAX_SESSION_ID: usize = 64;
/// The longest challenge kept.
pub const MAX_CHALLENGE: usize = 128;
/// The longest `authorization` value kept.
pub const MAX_AUTHORIZATION: usize = 256;
/// The longest registration path or refresh address kept.
pub const MAX_SESSION_URL: usize = 512;
/// The cookies one session may name.
pub const MAX_CREDENTIALS: usize = 4;
/// The longest cookie name a session may name.
pub const MAX_CREDENTIAL_NAME: usize = 64;
/// The scope rules one session may give.
pub const MAX_SCOPE_RULES: usize = 8;
/// The longest domain or path of a scope rule.
pub const MAX_RULE_TEXT: usize = 128;
/// The longest session instructions read.
pub const MAX_INSTRUCTIONS_BYTES: usize = 8 * 1024;
/// How long a session lasts from its registration or its last refresh:
/// the longest a cookie may last (RFC 6265bis section 5.6.1, 400 days), as
/// section 8.9 recommends.
pub const SESSION_LIFETIME_SECONDS: u64 = 400 * 24 * 60 * 60;

/// Bytes of one coordinate of a P-256 public key, and of each half of an
/// ES256 signature (RFC 7518 section 3.4).
pub const COORDINATE_BYTES: usize = 32;
/// An ES256 signature: R then S, each 32 bytes, big-endian.
pub const SIGNATURE_BYTES: usize = 2 * COORDINATE_BYTES;

/// The port's name for one of its keys.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct KeyHandle(pub u32);

/// The public half of a P-256 key: its affine coordinates, big-endian.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PublicKey {
    /// x.
    pub x: [u8; COORDINATE_BYTES],
    /// y.
    pub y: [u8; COORDINATE_BYTES],
}

/// Keys that never leave the device (REQ-WEB-017): the optional part of
/// the platform interface a port with protected key storage supplies
/// (REQ-ARCH-009). Each key is a P-256 key pair whose private half the
/// port keeps where it cannot be read: a secure element, a trusted
/// platform module, the operating system's key store. It has no `Debug`
/// supertrait: a trait object's table would carry `core::fmt` into every
/// build (REQ-GEN-016).
pub trait DeviceKeys {
    /// Makes a new key pair and gives its handle and public half, or `None`
    /// when no key can be made.
    fn create(&mut self) -> Option<(KeyHandle, PublicKey)>;
    /// Signs `message` with ECDSA on P-256 and SHA-256, giving R then S,
    /// or `None` when the key is gone or cannot sign.
    fn sign(&mut self, key: KeyHandle, message: &[u8]) -> Option<[u8; SIGNATURE_BYTES]>;
    /// Destroys a key no session uses any more.
    fn delete(&mut self, key: KeyHandle);
}

#[cfg(test)]
mod tests;
