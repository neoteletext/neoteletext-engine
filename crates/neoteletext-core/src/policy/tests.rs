//! Tests of the security policy decisions (area SEC).

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

#[test]
fn blocked_addresses() {
    for blocked in [
        [127, 0, 0, 1],
        [10, 1, 2, 3],
        [172, 16, 0, 1],
        [172, 31, 9, 9],
        [192, 168, 1, 1],
        [169, 254, 1, 1],
        [100, 64, 0, 1],
        [0, 0, 0, 0],
        [224, 0, 0, 1],
        [255, 255, 255, 255],
    ] {
        assert!(ipv4_blocked(blocked), "{blocked:?}");
    }
    for allowed in [[8, 8, 8, 8], [172, 32, 0, 1], [100, 128, 0, 1], [192, 169, 0, 1]] {
        assert!(!ipv4_blocked(allowed), "{allowed:?}");
    }
    assert!(ipv6_blocked([0, 0, 0, 0, 0, 0, 0, 1]));
    assert!(ipv6_blocked([0xFE80, 0, 0, 0, 0, 0, 0, 1]));
    assert!(ipv6_blocked([0xFD00, 0, 0, 0, 0, 0, 0, 1]));
    assert!(ipv6_blocked([0, 0, 0, 0, 0, 0xFFFF, 0x7F00, 0x0001]), "mapped loopback");
    assert!(!ipv6_blocked([0x2001, 0x4860, 0, 0, 0, 0, 0, 0x8888]));
    assert!(!ipv6_blocked([0, 0, 0, 0, 0, 0xFFFF, 0x0808, 0x0808]));
}

#[test]
fn origins_and_submission() {
    let page = OriginParts { secure: true, host: "example.org", port: 443 };
    let same = OriginParts { secure: true, host: "EXAMPLE.org", port: 443 };
    let plain = OriginParts { secure: false, host: "example.org", port: 80 };
    let other = OriginParts { secure: true, host: "forms.example", port: 443 };
    assert!(same_origin(page, same));
    assert!(!same_origin(page, plain));
    assert!(may_submit(page, same, &[]));
    assert!(!may_submit(page, other, &[]));
    assert!(may_submit(page, other, &[other]));
    assert!(is_downgrade(true, plain, "example.org"));
    assert!(!may_send_cookie(true, false));
}

#[test]
fn output_sanitising() {
    assert!(is_safe_output_char('A') && is_safe_output_char('\u{1FB00}'));
    for unsafe_char in ['\u{1B}', '\u{7F}', '\u{9B}', '\u{202E}', '\u{2066}', '\n'] {
        assert!(!is_safe_output_char(unsafe_char));
    }
    assert!(!is_safe_file_name_byte(b'/') && !is_safe_file_name_byte(b'\\'));
}
