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

/// Finding F-46, decision E.115: the ranges the review found open, and
/// private IPv4 addresses written inside NAT64 and 6to4 addresses.
#[test]
fn ranges_from_the_security_review_are_blocked() {
    for blocked in [[192, 0, 0, 1], [192, 0, 0, 170], [198, 18, 0, 1], [198, 19, 255, 255]] {
        assert!(ipv4_blocked(blocked), "{blocked:?}");
    }
    for allowed in [[192, 0, 1, 1], [198, 17, 255, 255], [198, 20, 0, 1]] {
        assert!(!ipv4_blocked(allowed), "{allowed:?}");
    }
    for blocked in [
        // ::10.0.0.1 and ::8.8.8.8, IPv4-compatible (RFC 4291 section 2.5.5.1).
        [0, 0, 0, 0, 0, 0, 0x0A00, 0x0001],
        [0, 0, 0, 0, 0, 0, 0x0808, 0x0808],
        // fec0::1, site-local (RFC 3879).
        [0xFEC0, 0, 0, 0, 0, 0, 0, 1],
        [0xFEFF, 0, 0, 0, 0, 0, 0, 1],
        // 64:ff9b::127.0.0.1 and 64:ff9b::192.168.1.1 (RFC 6052).
        [0x0064, 0xFF9B, 0, 0, 0, 0, 0x7F00, 0x0001],
        [0x0064, 0xFF9B, 0, 0, 0, 0, 0xC0A8, 0x0101],
        // 64:ff9b:1::10.0.0.1 with a /96 prefix, and 10.0.0.1 with the
        // /48 prefix itself (RFC 8215, RFC 6052 section 2.2).
        [0x0064, 0xFF9B, 0x0001, 0, 0, 0, 0x0A00, 0x0001],
        [0x0064, 0xFF9B, 0x0001, 0x0A00, 0x0001, 0, 0, 0],
        // 2002:c0a8:0101::1, 6to4 of 192.168.1.1 (RFC 3056).
        [0x2002, 0xC0A8, 0x0101, 0, 0, 0, 0, 1],
        [0x2002, 0x7F00, 0x0001, 0, 0, 0, 0, 1],
    ] {
        assert!(ipv6_blocked(blocked), "{blocked:x?}");
    }
    for allowed in [
        // 64:ff9b::8.8.8.8 and 2002:0808:0808::1 embed a public address.
        [0x0064, 0xFF9B, 0, 0, 0, 0, 0x0808, 0x0808],
        [0x0064, 0xFF9B, 0x0001, 0, 0, 0, 0x0808, 0x0808],
        [0x2002, 0x0808, 0x0808, 0, 0, 0, 0, 1],
        // Global unicast next to the site-local range.
        [0xFE00, 0, 0, 0, 0, 0, 0, 1],
        [0x2001, 0x0DB9, 0, 0, 0, 0, 0, 1],
    ] {
        assert!(!ipv6_blocked(allowed), "{allowed:x?}");
    }
}

/// REQ-SEC-005: row 0 columns 0 to 7 belong to the receiver alone; no
/// other row's columns do.
#[test]
fn receiver_columns_are_row_0_columns_0_to_7() {
    for column in 0..8 {
        assert!(is_receiver_column(0, column), "column {column}");
        assert!(!is_receiver_column(1, column), "row 1 column {column}");
    }
    assert!(!is_receiver_column(0, 8));
    assert!(!is_receiver_column(24, 0));
}

/// RFC 6052 section 2.2: an address under `64:ff9b:1::/48` whose bits 64
/// to 71 (the "u" octet) are not zero is not a well-formed /48, /56 or
/// /64 translation, so only its /96 reading counts: a public IPv4
/// address in its last 32 bits is allowed, whatever a shorter reading
/// would have embedded.
#[test]
fn a_nat64_local_use_address_with_a_u_octet_is_read_as_96_only() {
    // 64:ff9b:1:0:a::800:0 read as /64 embeds 10.0.0.8, as /96 8.0.0.0:
    // blocked while u is zero, allowed once u is not.
    assert!(ipv6_blocked([0x0064, 0xFF9B, 0x0001, 0, 0x000A, 0, 0x0800, 0]));
    assert!(!ipv6_blocked([0x0064, 0xFF9B, 0x0001, 0, 0x010A, 0, 0x0800, 0]));
    // 64:ff9b:1:a00:101::808:808: /48 would read 10.0.1.1, /96 8.8.8.8.
    assert!(!ipv6_blocked([0x0064, 0xFF9B, 0x0001, 0x0A00, 0x0101, 0, 0x0808, 0x0808]));
}

/// REQ-SEC-008: every submission over plain HTTP needs the warning, and
/// none over encryption does.
#[test]
fn plain_http_submission_needs_the_warning() {
    assert!(needs_plain_http_warning(false));
    assert!(!needs_plain_http_warning(true));
}
