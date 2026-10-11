//! Security policy: every security decision as a pure function, asked by
//! the layers that need it (ARCHITECTURE.md section 8 rule 4, REQ-SEC-001).
//! No other module reimplements these decisions.

use crate::consts::{RECEIVER_COLUMNS, STATUS_COLUMNS};

/// True when a row-0 column belongs to the receiver alone (REQ-SEC-005).
#[must_use]
pub const fn is_receiver_column(row: usize, column: usize) -> bool {
    row == 0 && column < RECEIVER_COLUMNS
}

/// Columns of the security indicator: the receiver status area.
pub const INDICATOR_COLUMNS: usize = STATUS_COLUMNS;

/// True when an IPv4 address may not be contacted (REQ-SEC-006):
/// loopback, private, link-local, shared address space, unspecified,
/// broadcast and multicast.
#[must_use]
pub const fn ipv4_blocked(address: [u8; 4]) -> bool {
    let [a, b, _, _] = address;
    matches!(a, 0 | 10 | 127)
        || (a == 100 && b >= 64 && b <= 127)
        || (a == 169 && b == 254)
        || (a == 172 && b >= 16 && b <= 31)
        || (a == 192 && b == 168)
        || a >= 224
}

/// True when an IPv6 address may not be contacted (REQ-SEC-006): loopback,
/// unspecified, unique local `fc00::/7`, link-local `fe80::/10`, multicast
/// `ff00::/8`, and the IPv4-mapped forms of blocked IPv4 addresses.
#[must_use]
pub const fn ipv6_blocked(segments: [u16; 8]) -> bool {
    let [s0, s1, s2, s3, s4, s5, s6, s7] = segments;
    let zero_prefix = s0 == 0 && s1 == 0 && s2 == 0 && s3 == 0 && s4 == 0;
    if zero_prefix && s5 == 0 && s6 == 0 && (s7 == 0 || s7 == 1) {
        return true;
    }
    if zero_prefix && s5 == 0xFFFF {
        let v4 = [(s6 >> 8) as u8, s6 as u8, (s7 >> 8) as u8, s7 as u8];
        return ipv4_blocked(v4);
    }
    (s0 & 0xFE00) == 0xFC00 || (s0 & 0xFFC0) == 0xFE80 || (s0 & 0xFF00) == 0xFF00
}

/// The parts of an origin the policy compares (REQ-WEB-015). Hosts are
/// already normalised to lower case by the address parser.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OriginParts<'a> {
    /// The connection is encrypted (https or wss).
    pub secure: bool,
    /// The host.
    pub host: &'a str,
    /// The port, with the scheme's default filled in.
    pub port: u16,
}

/// True when two origins are the same (REQ-WEB-015): scheme security, host
/// and port all equal; ws and wss count as http and https.
#[must_use]
pub fn same_origin(a: OriginParts<'_>, b: OriginParts<'_>) -> bool {
    a.secure == b.secure && a.port == b.port && a.host.eq_ignore_ascii_case(b.host)
}

/// True when moving from `from` to `to` would downgrade a service first
/// reached over encryption (REQ-SEC-007). The rule applies to the host
/// whatever the scheme.
#[must_use]
pub fn is_downgrade(from_secure: bool, to: OriginParts<'_>, host: &str) -> bool {
    from_secure && !to.secure && to.host.eq_ignore_ascii_case(host)
}

/// True when a form on a page from `page` may submit to `target`
/// (REQ-WEB-008, REQ-SEC-009): the same origin, or one the descriptor lists.
#[must_use]
pub fn may_submit(page: OriginParts<'_>, target: OriginParts<'_>, permitted: &[OriginParts<'_>]) -> bool {
    same_origin(page, target) || permitted.iter().any(|&allowed| same_origin(allowed, target))
}

/// True when a submission needs the plain-HTTP warning and confirmation
/// (REQ-SEC-008).
#[must_use]
pub const fn needs_plain_http_warning(target_secure: bool) -> bool {
    !target_secure
}

/// True when a cookie may be sent over this connection (REQ-SEC-008,
/// REQ-WEB-006): `Secure` cookies only over encryption.
#[must_use]
pub const fn may_send_cookie(cookie_secure: bool, connection_secure: bool) -> bool {
    !cookie_secure || connection_secure
}

/// True when a character may be written to a terminal or text file
/// (REQ-SEC-010): no C0 or C1 control, no DEL, no bidirectional control.
#[must_use]
pub const fn is_safe_output_char(c: char) -> bool {
    let value = c as u32;
    !(value < 0x20
        || value == 0x7F
        || (value >= 0x80 && value <= 0x9F)
        || value == 0x061C
        || value == 0x200E
        || value == 0x200F
        || (value >= 0x202A && value <= 0x202E)
        || (value >= 0x2066 && value <= 0x2069))
}

/// True when a byte may appear in a file name derived from received
/// content (REQ-SEC-010): letters, digits, hyphen, underscore and full stop
/// only, so no path separator can appear.
#[must_use]
pub const fn is_safe_file_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b'.'
}

#[cfg(test)]
mod tests {
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
}
