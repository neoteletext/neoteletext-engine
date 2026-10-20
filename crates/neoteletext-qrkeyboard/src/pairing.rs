//! One pairing run between the screen and the phone, per connection, as
//! qrkeyboard's `pairing.ts` runs it: each end sends a fresh salt, and in
//! PIN mode a SPAKE2 share and then a confirmation. The relay delivers
//! frames in order, so the peer's salt comes before its PAKE frames. No
//! input or output: frames in, frames to send out.

use crate::Error;
use crate::crypto::{
    CONFIRM_BYTES, Opener, Role, SALT_BYTES, SECRET_BYTES, SHARE_BYTES, Sealer, Spake2, Spake2Result,
    constant_time_equal, directional_keys, random_scalar,
};

/// A salt frame's tag (`TAG_SALT`).
pub const TAG_SALT: u8 = 0xE3;
/// A SPAKE2 share's tag (`TAG_PAKE_SHARE`).
pub const TAG_PAKE_SHARE: u8 = 0xE0;
/// A SPAKE2 confirmation's tag (`TAG_PAKE_CONFIRM`).
pub const TAG_PAKE_CONFIRM: u8 = 0xE2;

/// How the two ends share a secret.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Secret {
    /// No-PIN mode: the random key the QR carries.
    Key([u8; SECRET_BYTES]),
    /// PIN mode: the PIN the screen shows.
    Pin(String),
}

/// How a run ended.
pub enum Outcome {
    /// Paired: this side's sealer and opener.
    Paired(Box<(Sealer, Opener)>),
    /// The confirmations disagreed: a wrong PIN, or a relay that altered
    /// the configuration.
    WrongPin,
}

/// One pairing run.
pub struct Pairing {
    role: Role,
    key: Option<[u8; SECRET_BYTES]>,
    spake: Option<Spake2>,
    result: Option<Spake2Result>,
    own_salt: [u8; SALT_BYTES],
    peer_salt: Option<[u8; SALT_BYTES]>,
    finished: bool,
}

fn tagged(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(body.len().saturating_add(1));
    frame.push(tag);
    frame.extend_from_slice(body);
    frame
}

impl Pairing {
    /// A run for join code `code`, with the configuration's hash bound into
    /// a PIN run's transcript.
    ///
    /// # Errors
    /// When the operating system has no random numbers.
    pub fn new(role: Role, code: &str, secret: &Secret, config_hash: &[u8]) -> Result<Self, Error> {
        let mut own_salt = [0_u8; SALT_BYTES];
        crate::random(&mut own_salt)?;
        let (key, spake) = match secret {
            Secret::Key(key) => (Some(*key), None),
            Secret::Pin(pin) => {
                let spake =
                    Spake2::start(role, code, pin, config_hash, random_scalar()?).ok_or(Error::Protocol("SPAKE2"))?;
                (None, Some(spake))
            }
        };
        Ok(Self { role, key, spake, result: None, own_salt, peer_salt: None, finished: false })
    }

    /// Whether this run tests a PIN.
    #[must_use]
    pub const fn uses_pin(&self) -> bool {
        self.spake.is_some()
    }

    /// The frames that start the run: the salt, and the share in PIN mode.
    #[must_use]
    pub fn start(&self) -> Vec<Vec<u8>> {
        let mut frames = vec![tagged(TAG_SALT, &self.own_salt)];
        if let Some(spake) = &self.spake {
            frames.push(tagged(TAG_PAKE_SHARE, spake.share()));
        }
        frames
    }

    /// Feeds one binary frame: frames to send, and the outcome once the run
    /// ends. Frames that are not part of pairing change nothing.
    pub fn receive(&mut self, frame: &[u8]) -> (Vec<Vec<u8>>, Option<Outcome>) {
        let Some((&tag, body)) = frame.split_first() else { return (Vec::new(), None) };
        if self.finished {
            return (Vec::new(), None);
        }
        match tag {
            TAG_SALT if body.len() == SALT_BYTES && self.peer_salt.is_none() => {
                let mut salt = [0_u8; SALT_BYTES];
                salt.copy_from_slice(body);
                self.peer_salt = Some(salt);
                let key = self.key;
                (Vec::new(), key.and_then(|key| self.paired(&key)))
            }
            TAG_PAKE_SHARE if body.len() == SHARE_BYTES && self.result.is_none() => {
                let Some(result) = self.spake.as_ref().and_then(|spake| spake.finish(body)) else {
                    // Not a point: as a wrong PIN, the run is over.
                    self.finished = true;
                    return (Vec::new(), Some(Outcome::WrongPin));
                };
                let confirm = tagged(TAG_PAKE_CONFIRM, &result.confirm_out);
                self.result = Some(result);
                (vec![confirm], None)
            }
            TAG_PAKE_CONFIRM if body.len() == CONFIRM_BYTES => {
                let Some(result) = self.result.clone() else { return (Vec::new(), None) };
                if !constant_time_equal(body, &result.confirm_expected) {
                    self.finished = true;
                    return (Vec::new(), Some(Outcome::WrongPin));
                }
                (Vec::new(), self.paired(&result.secret))
            }
            _ => (Vec::new(), None),
        }
    }

    fn paired(&mut self, secret: &[u8; SECRET_BYTES]) -> Option<Outcome> {
        let peer = self.peer_salt?;
        self.finished = true;
        let (device, phone) = if self.role == Role::Device { (self.own_salt, peer) } else { (peer, self.own_salt) };
        let mut salt = [0_u8; 2 * SALT_BYTES];
        let (first, second) = salt.split_at_mut(SALT_BYTES);
        first.copy_from_slice(&device);
        second.copy_from_slice(&phone);
        let (sealer, opener) = directional_keys(secret, self.role, &salt);
        Some(Outcome::Paired(Box::new((sealer, opener))))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Runs two ends against each other, in order, as the relay delivers.
    fn run(device_secret: &Secret, phone_secret: &Secret) -> (Option<Outcome>, Option<Outcome>) {
        let hash = [7_u8; 16];
        let mut device = Pairing::new(Role::Device, "CODE", device_secret, &hash).unwrap();
        let mut phone = Pairing::new(Role::Phone, "CODE", phone_secret, &hash).unwrap();
        let (mut to_phone, mut to_device) = (device.start(), phone.start());
        let (mut device_outcome, mut phone_outcome) = (None, None);
        while !to_phone.is_empty() || !to_device.is_empty() {
            for frame in core::mem::take(&mut to_phone) {
                let (out, outcome) = phone.receive(&frame);
                to_device.extend(out);
                phone_outcome = phone_outcome.or(outcome);
            }
            for frame in core::mem::take(&mut to_device) {
                let (out, outcome) = device.receive(&frame);
                to_phone.extend(out);
                device_outcome = device_outcome.or(outcome);
            }
        }
        (device_outcome, phone_outcome)
    }

    fn talk(device: Option<Outcome>, phone: Option<Outcome>) {
        let (Some(Outcome::Paired(device)), Some(Outcome::Paired(phone))) = (device, phone) else {
            panic!("not paired");
        };
        let ((mut device_seal, mut device_open), (mut phone_seal, mut phone_open)) = (*device, *phone);
        assert_eq!(phone_open.open(&device_seal.seal(b"down").unwrap()).unwrap(), b"down");
        assert_eq!(device_open.open(&phone_seal.seal(b"up").unwrap()).unwrap(), b"up");
    }

    #[test]
    fn a_key_pairs() {
        let (device, phone) = run(&Secret::Key([9; 32]), &Secret::Key([9; 32]));
        talk(device, phone);
    }

    #[test]
    fn a_pin_pairs_and_a_wrong_one_does_not() {
        let (device, phone) = run(&Secret::Pin("1234".into()), &Secret::Pin("1234".into()));
        talk(device, phone);
        let (device, phone) = run(&Secret::Pin("1234".into()), &Secret::Pin("9999".into()));
        assert!(matches!(device, Some(Outcome::WrongPin)) && matches!(phone, Some(Outcome::WrongPin)));
    }

    #[test]
    fn different_keys_cannot_talk() {
        let (device, phone) = run(&Secret::Key([1; 32]), &Secret::Key([2; 32]));
        let (Some(Outcome::Paired(device)), Some(Outcome::Paired(phone))) = (device, phone) else { panic!() };
        let ((mut seal, _), (_, mut open)) = (*device, *phone);
        assert!(open.open(&seal.seal(b"x").unwrap()).is_none());
    }
}
