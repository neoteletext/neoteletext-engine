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
/// broadcast and multicast, the IETF protocol assignments `192.0.0.0/24`
/// (RFC 6890 section 2.2.2) and benchmarking `198.18.0.0/15` (RFC 2544
/// appendix C.2.2), which reach this computer, the local network or a
/// translator to them (decision E.115).
#[must_use]
pub const fn ipv4_blocked(address: [u8; 4]) -> bool {
    let [a, b, c, _] = address;
    matches!(a, 0 | 10 | 127)
        || (a == 100 && b >= 64 && b <= 127)
        || (a == 169 && b == 254)
        || (a == 172 && b >= 16 && b <= 31)
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && c == 0)
        || (a == 198 && (b == 18 || b == 19))
        || a >= 224
}

/// NAT64's Well-Known Prefix `64:ff9b::/96` (RFC 6052 section 2.1), its
/// first six groups. The IPv4 address is the last 32 bits.
const NAT64_WELL_KNOWN: [u16; 6] = [0x0064, 0xFF9B, 0, 0, 0, 0];

/// NAT64's local-use prefix `64:ff9b:1::/48` (RFC 8215 section 4), its
/// first three groups. The operator places a translation prefix of 48 to 96
/// bits inside it, so the IPv4 address may be at any of RFC 6052 section
/// 2.2's positions for those lengths.
const NAT64_LOCAL_USE: [u16; 3] = [0x0064, 0xFF9B, 0x0001];

/// 6to4's prefix `2002::/16` (RFC 3056 section 2); the IPv4 address is the
/// next 32 bits.
const SIX_TO_FOUR: u16 = 0x2002;

/// Masks and values of the IPv6 ranges blocked by their first group: unique
/// local `fc00::/7` (RFC 4193), link-local `fe80::/10` (RFC 4291 section
/// 2.5.6), site-local `fec0::/10` (deprecated by RFC 3879, which says it
/// stays reserved) and multicast `ff00::/8` (RFC 4291 section 2.7).
const FIRST_GROUP_RANGES: [(u16, u16); 4] = [(0xFE00, 0xFC00), (0xFFC0, 0xFE80), (0xFFC0, 0xFEC0), (0xFF00, 0xFF00)];

/// True when an IPv6 address may not be contacted (REQ-SEC-006): the
/// IPv4-compatible addresses `::a.b.c.d` (deprecated, RFC 4291 section
/// 2.5.5.1), which include loopback and unspecified; unique local,
/// link-local, site-local and multicast; and every address that embeds a
/// blocked IPv4 address, IPv4-mapped (RFC 4291 section 2.5.5.2), NAT64
/// (RFC 6052, RFC 8215) or 6to4 (RFC 3056), judged by the IPv4 address it
/// embeds so that a private one cannot be reached through it (decision
/// E.115).
#[must_use]
pub const fn ipv6_blocked(segments: [u16; 8]) -> bool {
    let [s0, s1, s2, s3, s4, s5, s6, s7] = segments;
    let last32 = split(s6, s7);
    let zero_prefix = s0 == 0 && s1 == 0 && s2 == 0 && s3 == 0 && s4 == 0;
    if zero_prefix && s5 == 0 {
        return true;
    }
    if zero_prefix && s5 == 0xFFFF {
        return ipv4_blocked(last32);
    }
    let [w0, w1, w2, w3, w4, w5] = NAT64_WELL_KNOWN;
    if s0 == w0 && s1 == w1 && s2 == w2 && s3 == w3 && s4 == w4 && s5 == w5 {
        return ipv4_blocked(last32);
    }
    let [l0, l1, l2] = NAT64_LOCAL_USE;
    if s0 == l0 && s1 == l1 && s2 == l2 {
        return nat64_local_use_blocked([s3, s4, s5, s6, s7]);
    }
    if s0 == SIX_TO_FOUR {
        return ipv4_blocked(split(s1, s2));
    }
    let [unique_local, link_local, site_local, multicast] = FIRST_GROUP_RANGES;
    in_range(s0, unique_local) || in_range(s0, link_local) || in_range(s0, site_local) || in_range(s0, multicast)
}

/// True when a first group is in a range given as (mask, value).
const fn in_range(group: u16, (mask, value): (u16, u16)) -> bool {
    group & mask == value
}

/// The IPv4 address embedded in two groups.
const fn split(high: u16, low: u16) -> [u8; 4] {
    let [a, b] = high.to_be_bytes();
    let [c, d] = low.to_be_bytes();
    [a, b, c, d]
}

/// True when an address under `64:ff9b:1::/48`, given by its groups after
/// the prefix (bytes 6 to 15), may not be contacted. The prefix length the
/// operator chose is not in the address, so it is judged by every reading
/// RFC 6052 section 2.2 allows: the /96 one always, and the /48, /56 and
/// /64 ones when they are well formed, with bits 64 to 71 (the "u" octet)
/// and the suffix after the IPv4 address zero, as that section requires. A
/// public address read as /96 is not misjudged by the shorter readings,
/// whose suffix it fills.
const fn nat64_local_use_blocked(groups: [u16; 5]) -> bool {
    let [g3, g4, g5, g6, g7] = groups;
    let [b6, b7] = g3.to_be_bytes();
    let [u, b9] = g4.to_be_bytes();
    let [b10, b11] = g5.to_be_bytes();
    let [b12, b13] = g6.to_be_bytes();
    let [b14, b15] = g7.to_be_bytes();
    if ipv4_blocked([b12, b13, b14, b15]) {
        return true;
    }
    if u != 0 {
        return false;
    }
    let tail_from_13 = b13 == 0 && b14 == 0 && b15 == 0;
    let prefix_64 = tail_from_13 && ipv4_blocked([b9, b10, b11, b12]);
    let prefix_56 = b12 == 0 && tail_from_13 && ipv4_blocked([b7, b9, b10, b11]);
    let prefix_48 = b11 == 0 && b12 == 0 && tail_from_13 && ipv4_blocked([b6, b7, b9, b10]);
    prefix_64 || prefix_56 || prefix_48
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
mod tests;
