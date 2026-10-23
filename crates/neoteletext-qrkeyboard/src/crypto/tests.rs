#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use super::spake2::{SCALAR_BYTES, reduce};
use super::*;
use p256::Scalar;

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
