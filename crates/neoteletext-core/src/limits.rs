//! The numbers of World System Teletext's page and sub-page addressing,
//! written once (area NUM, REQ-NUM-031, decisions E.122 and E.130).
//!
//! Every number of area NUM lives here, with the clause that gives it:
//! magazines, page numbers, the null page address, the reserved and
//! recommended page numbers, the counts, the three ways a sub-code is laid
//! out (a display page's, an enhancement data page's and a data
//! broadcasting page's), and the one function that numbers a page's
//! sub-pages (REQ-NUM-019) with its inverse. The engine's `consts`
//! re-exports the ones it used before this module existed; the server and
//! the browser import them from here.
//!
//! The module has no dependency, not even on the rest of this crate, and
//! no feature gate, so a part that needs only the numbers can depend on
//! `neoteletext-core` with its default features off and take nothing else.
//! Sub-codes are plain `u16` values here for the same reason; the typed
//! [`crate::types::SubCode`] is built from them.
//!
//! Sources are EN 300 706 V1.2.1 (2003-04) unless another document is
//! named: TR 100 287 V1.2.1 (2002-12), the code of practice, and
//! EN 300 708 V1.2.1 (2003-04), data broadcasting.
//!
//! A maximum here is the standard's: what may exist and be addressed. It is
//! never a budget: how much a part holds or sends at once is that part's
//! own named constant (REQ-NUM-030).

// ---------------------------------------------------------------------------
// Magazines (REQ-NUM-003)
// ---------------------------------------------------------------------------

/// Magazines in a service: the three magazine bits of packet address byte
/// 4 (clause 7.1.2), at most 8 (clause 7.4).
pub const MAGAZINES: usize = 8;
/// The magazine sent as 0 is magazine 8 (clause 7.1.2).
pub const MAGAZINE_ZERO_MEANS: u8 = 8;
/// The lowest magazine number, sent as 1 (clause 7.1.2).
pub const FIRST_MAGAZINE: u8 = 1;
/// Mask of the three magazine bits (clause 7.1.2).
pub const MAGAZINE_MASK: u8 = 0x07;

// ---------------------------------------------------------------------------
// Page numbers (REQ-NUM-004, REQ-NUM-008 to REQ-NUM-015)
// ---------------------------------------------------------------------------

/// The highest value of a page number digit, tens or units: each is a
/// Hamming 8/4 nibble, 0 to F (clause 9.3.1.1).
pub const PAGE_DIGIT_MAX: u8 = 0xF;
/// The highest decimal digit; page numbers with a digit above it are
/// hexadecimal and not for display (clause 9.1, Table 1, note).
pub const DECIMAL_DIGIT_MAX: u8 = 9;
/// Shift of the tens digit in a page number (clause 9.3.1.1).
pub const PAGE_TENS_SHIFT: u32 = 4;
/// Page numbers in a magazine, 00 to FF: two hexadecimal digits (clause
/// 9.3.1.1).
pub const PAGE_NUMBERS_PER_MAGAZINE: usize = (PAGE_DIGIT_MAX as usize + 1) * (PAGE_DIGIT_MAX as usize + 1);
/// Page numbers in a service, 2,048.
pub const PAGE_NUMBERS: usize = MAGAZINES * PAGE_NUMBERS_PER_MAGAZINE;

/// Page number FF: time filling and terminating headers only, never a page
/// (clause 7.3; Annex A.1, rule 2; Annex A.10; REQ-NUM-008).
pub const PAGE_TIME_FILLING: u8 = 0xFF;
/// The Magazine Inventory Page, mFD in every magazine (clause 11.3.2;
/// Annex A.10).
pub const PAGE_MIP: u8 = 0xFD;
/// The Magazine Organization Table, mFE in every magazine (clause 10.6.1;
/// Annex A.10).
pub const PAGE_MOT: u8 = 0xFE;
/// Page numbers reserved in every magazine (Annex A.10; REQ-NUM-010).
pub const RESERVED_IN_EVERY_MAGAZINE: [u8; 3] = [PAGE_MIP, PAGE_MOT, PAGE_TIME_FILLING];
/// Automatic Channel Installation, page 1BE alone (Annex A.10; Annex L),
/// as `0xMPP` with the magazine as 1 to 8.
pub const PAGE_ACI: u16 = 0x1BE;
/// The Basic TOP Table, page 1F0 alone (clause 11.2.2; Annex A.10).
pub const PAGE_BTT: u16 = 0x1F0;
/// Page numbers reserved in one magazine only (Annex A.10).
pub const RESERVED_PAGES: [u16; 2] = [PAGE_ACI, PAGE_BTT];
/// Page numbers in a service that may carry a page: 8 × 253 − 2 = 2,022,
/// display and data pages alike (REQ-NUM-010, REQ-NUM-011).
pub const USABLE_PAGE_NUMBERS: usize =
    MAGAZINES * (PAGE_NUMBERS_PER_MAGAZINE - RESERVED_IN_EVERY_MAGAZINE.len()) - RESERVED_PAGES.len();

/// The service's home page, unless packet 8/30 Format 1's Initial Teletext
/// Page names another (Annex E.15; Annex D.3; REQ-SVC-004). Recommended,
/// not reserved (REQ-NUM-013).
pub const PAGE_HOME: u16 = 0x100;
/// EPG data broadcasting (EN 300 707), unless a MIP names another (Annex
/// E.15). Recommended.
pub const PAGE_EPG: u16 = 0x1DF;
/// Trigger messages, unless a MIP names another (Annex E.15).
/// Recommended.
pub const PAGE_TRIGGER: u16 = 0x1E7;
/// The first and last recommended numbers of the TOP data pages (AIT, MPT
/// and MPT-EX), whose numbers the BTT assigns (Annex E.15; clause 11.2.2).
pub const PAGES_TOP_DATA: [u16; 2] = [0x1F1, 0x1F4];

/// Decimal page numbers in a magazine, 00 to 99.
pub const DECIMAL_PAGE_NUMBERS_PER_MAGAZINE: usize =
    (DECIMAL_DIGIT_MAX as usize + 1) * (DECIMAL_DIGIT_MAX as usize + 1);
/// The lowest page a viewer can key (TR 100 287 clause 10.1; REQ-NAV-001).
pub const FIRST_KEYABLE_PAGE: u16 = 0x100;
/// The highest page a viewer can key; 899 wraps to 100 (REQ-NAV-002).
pub const LAST_KEYABLE_PAGE: u16 = 0x899;
/// Pages a viewer can key, 100 to 899: 800 (TR 100 287 clause 10.1;
/// REQ-NUM-015). Pages with a hexadecimal digit are reached by links.
pub const KEYABLE_PAGES: usize = MAGAZINES * DECIMAL_PAGE_NUMBERS_PER_MAGAZINE;

/// Shift of the magazine digit in the three-digit form `0xMPP`: above the
/// page number's two digits.
pub const MPP_MAGAZINE_SHIFT: u32 = 2 * PAGE_TENS_SHIFT;
/// The lowest page address in the form `0xMPP`, the magazine as 1 to 8:
/// page 100.
pub const FIRST_PAGE_ADDRESS: u16 = (FIRST_MAGAZINE as u16) << MPP_MAGAZINE_SHIFT;
/// The highest page address in the form `0xMPP`: page 8FF.
pub const LAST_PAGE_ADDRESS: u16 =
    ((MAGAZINE_ZERO_MEANS as u16) << MPP_MAGAZINE_SHIFT) | (PAGE_NUMBERS_PER_MAGAZINE - 1) as u16;

/// True when both digits of a page number are decimal (Table 1, note).
#[must_use]
pub const fn is_decimal_page(page: u8) -> bool {
    (page >> PAGE_TENS_SHIFT) <= DECIMAL_DIGIT_MAX && (page & PAGE_DIGIT_MAX) <= DECIMAL_DIGIT_MAX
}

/// True when page `page` of magazine `magazine` (1 to 8) is reserved for a
/// function other than a display page (Annex A.10; REQ-NUM-010).
#[must_use]
pub fn is_reserved_page(magazine: u8, page: u8) -> bool {
    let mpp = (u16::from(magazine) << MPP_MAGAZINE_SHIFT) | u16::from(page);
    RESERVED_IN_EVERY_MAGAZINE.contains(&page) || RESERVED_PAGES.contains(&mpp)
}

/// True when page `page` of magazine `magazine` (1 to 8) can be keyed:
/// a decimal page number (TR 100 287 clause 10.1; REQ-NUM-015).
#[must_use]
pub const fn is_keyable_page(magazine: u8, page: u8) -> bool {
    magazine >= FIRST_MAGAZINE && magazine <= MAGAZINE_ZERO_MEANS && is_decimal_page(page)
}

// ---------------------------------------------------------------------------
// Sub-codes (REQ-NUM-005 to REQ-NUM-007, REQ-NUM-016 to REQ-NUM-020)
// ---------------------------------------------------------------------------

/// Shift and mask of sub-code element S1, four bits (clause 9.3.1.2).
pub const S1_SHIFT: u32 = 0;
/// Mask of S1 after its shift.
pub const S1_MASK: u16 = 0xF;
/// Shift of S2, three bits: the fourth bit of its byte is C4 (clause
/// 9.3.1.2).
pub const S2_SHIFT: u32 = 4;
/// Mask of S2 after its shift.
pub const S2_MASK: u16 = 0x7;
/// Shift of S3, four bits.
pub const S3_SHIFT: u32 = 8;
/// Mask of S3 after its shift.
pub const S3_MASK: u16 = 0xF;
/// Shift of S4, two bits: the other two bits of its byte are C5 and C6
/// (clause 9.3.1.2).
pub const S4_SHIFT: u32 = 12;
/// Mask of S4 after its shift.
pub const S4_MASK: u16 = 0x3;
/// Every bit a sub-code can carry, written S4 S3 S2 S1: 3F7F.
pub const SUBCODE_MASK: u16 =
    (S1_MASK << S1_SHIFT) | (S2_MASK << S2_SHIFT) | (S3_MASK << S3_SHIFT) | (S4_MASK << S4_SHIFT);
/// Bits of a sub-code: 4 + 3 + 4 + 2 (clause 9.3.1.2).
pub const SUBCODE_BITS: u32 = SUBCODE_MASK.count_ones();
/// Sub-code values, 0000 to 3F7F: 8,192.
pub const SUBCODE_VALUES: usize = 1 << SUBCODE_BITS;

/// The sub-code of a page with no sub-pages (Annex A.1, display rule 1).
pub const SUBCODE_NONE: u16 = 0x0000;
/// 3F7F: in a link, no particular sub-code; only the page number is used
/// (clause 9.6.1; TR 100 287 clause 10.6; REQ-NUM-018). The highest
/// sub-code, so every bit of [`SUBCODE_MASK`].
pub const SUBCODE_ANY: u16 = SUBCODE_MASK;
/// The page number of a link that names no particular page (clauses 9.6.1
/// and 9.6.3), the same FF as [`PAGE_TIME_FILLING`]: no page is ever
/// stored there, so a link to it can name none.
pub const LINK_NULL_PAGE: u8 = PAGE_TIME_FILLING;
/// The sub-code of the null page address.
pub const LINK_NULL_SUBCODE: u16 = SUBCODE_ANY;
/// The null page address XFF:3F7F (clause 7.2.2): never sent as a header
/// (Annex A.1, rule 3), and in a link "no page" (REQ-NUM-007).
pub const NULL_ADDRESS: (u8, u16) = (LINK_NULL_PAGE, LINK_NULL_SUBCODE);

/// The first sub-code of a sub-page: 0001 (Annex A.1, display rule 2).
pub const FIRST_SUBPAGE_CODE: u16 = 0x0001;
/// The last sub-code a link can name: 3F7E (TR 100 287 clause 10.1;
/// REQ-NUM-009).
pub const LAST_SUBPAGE_CODE: u16 = SUBCODE_ANY - 1;
/// Sub-pages a display page may have: 0001 to 3F7E, 8,190; the MIP's code
/// 51 counts to 2^13 − 2 as well (clause 11.3.3, Table 24; REQ-NUM-020).
pub const MAX_SUBPAGES: usize = SUBCODE_VALUES - 2;

/// Sub-pages of a rotating set numbered 0001 to 0079 (Annex A.1, display
/// rule 2; TR 100 287 clause 10.1).
pub const SET_SUBPAGES: u16 = 79;
/// The last sub-code of such a set, 0079: decimal digits in S2 and S1.
pub const SET_LAST_SUBCODE: u16 = 0x0079;
/// Sub-pages of a set of more than 79, each above 0079 and none 3F7F
/// (Annex A.1, display rule 2; REQ-NUM-016 rule 3): 8,069.
pub const SUBCODES_ABOVE_SET: usize = MAX_SUBPAGES - rank(SET_LAST_SUBCODE) as usize;

/// Decimal values of each element: S1 and S3 0 to 9, S2 0 to 7 (all its
/// values), S4 0 to 3 (all its values).
const DECIMAL_S1_VALUES: u16 = DECIMAL_DIGIT_MAX as u16 + 1;
const DECIMAL_S2_VALUES: u16 = S2_MASK + 1;
const DECIMAL_S3_VALUES: u16 = DECIMAL_DIGIT_MAX as u16 + 1;
const DECIMAL_S4_VALUES: u16 = S4_MASK + 1;
/// Decimal sub-codes with a given S4: S3, S2 and S1 decimal.
const DECIMAL_PER_S4: u16 = DECIMAL_S3_VALUES * DECIMAL_S2_VALUES * DECIMAL_S1_VALUES;
/// Decimal sub-codes with a given S4 and S3.
const DECIMAL_PER_S3: u16 = DECIMAL_S2_VALUES * DECIMAL_S1_VALUES;
/// Decimal sub-code values, 0000 included.
const DECIMAL_VALUES: u16 = DECIMAL_S4_VALUES * DECIMAL_PER_S4;
/// The decimal sub-codes a viewer can key, 0001 to 3979: 3,199
/// (REQ-NUM-017).
pub const DECIMAL_SUBCODES: usize = DECIMAL_VALUES as usize - 1;
/// Of those, the ones above 0079: 3,120 (REQ-NUM-017).
pub const DECIMAL_SUBCODES_ABOVE_SET: usize = DECIMAL_SUBCODES - SET_SUBPAGES as usize;
/// Digits of a sub-code entered by a viewer (REQ-NAV-005).
pub const SUBCODE_DIGITS: usize = 4;
/// Sub-codes at or below 0079 with a hexadecimal digit in S1 (000A to
/// 006F): skipped when a set of more than 79 is numbered.
const HEXADECIMAL_TO_SET: u16 = rank(SET_LAST_SUBCODE) + 1 - (SET_SUBPAGES + 1);
/// The low bits of a rank: S1 and S2, which sit together in a sub-code.
const RANK_LOW_BITS: u32 = S2_SHIFT + S2_MASK.count_ones();
/// Mask of the low bits of a rank.
const RANK_LOW_MASK: u16 = (1 << RANK_LOW_BITS) - 1;
/// Mask of S4 and S3 together, after S3's shift.
const RANK_HIGH_MASK: u16 = (S4_MASK << (S4_SHIFT - S3_SHIFT)) | S3_MASK;

/// Element S1 of a sub-code.
#[must_use]
pub const fn s1(sub_code: u16) -> u8 {
    ((sub_code >> S1_SHIFT) & S1_MASK) as u8
}

/// Element S2 of a sub-code.
#[must_use]
pub const fn s2(sub_code: u16) -> u8 {
    ((sub_code >> S2_SHIFT) & S2_MASK) as u8
}

/// Element S3 of a sub-code.
#[must_use]
pub const fn s3(sub_code: u16) -> u8 {
    ((sub_code >> S3_SHIFT) & S3_MASK) as u8
}

/// Element S4 of a sub-code.
#[must_use]
pub const fn s4(sub_code: u16) -> u8 {
    ((sub_code >> S4_SHIFT) & S4_MASK) as u8
}

/// A sub-code from its four elements, each masked to its width (clause
/// 9.3.1.2).
#[must_use]
pub const fn sub_code(s4: u8, s3: u8, s2: u8, s1: u8) -> u16 {
    ((s4 as u16 & S4_MASK) << S4_SHIFT)
        | ((s3 as u16 & S3_MASK) << S3_SHIFT)
        | ((s2 as u16 & S2_MASK) << S2_SHIFT)
        | ((s1 as u16 & S1_MASK) << S1_SHIFT)
}

/// True when a value is a sub-code: no bit outside the thirteen.
#[must_use]
pub const fn is_sub_code(value: u16) -> bool {
    value & !SUBCODE_MASK == 0
}

/// The position of a sub-code among all 8,192 in ascending order, 0 to
/// 8,191: the thirteen bits packed together. Ascending sub-codes have
/// ascending ranks.
#[must_use]
pub const fn rank(sub_code: u16) -> u16 {
    (sub_code & RANK_LOW_MASK) | (((sub_code >> S3_SHIFT) & RANK_HIGH_MASK) << RANK_LOW_BITS)
}

/// The sub-code of a rank: the inverse of [`rank`].
#[must_use]
pub const fn from_rank(rank: u16) -> u16 {
    (rank & RANK_LOW_MASK) | (((rank >> RANK_LOW_BITS) & RANK_HIGH_MASK) << S3_SHIFT)
}

/// True for a decimal sub-code: every element a decimal digit (S2 and S4
/// always are). A viewer keys these (Annex A.1; REQ-NUM-017).
#[must_use]
pub const fn is_decimal_sub_code(sub_code: u16) -> bool {
    is_sub_code(sub_code) && s1(sub_code) <= DECIMAL_DIGIT_MAX && s3(sub_code) <= DECIMAL_DIGIT_MAX
}

/// How many decimal sub-codes, 0000 included, are below `sub_code`.
const fn decimal_below(sub_code: u16) -> u16 {
    let (s4, s3, s2, s1) = (s4(sub_code) as u16, s3(sub_code) as u16, s2(sub_code) as u16, s1(sub_code) as u16);
    let whole_s4 = s4.saturating_mul(DECIMAL_PER_S4);
    let s3_below = if s3 < DECIMAL_S3_VALUES { s3 } else { DECIMAL_S3_VALUES };
    let whole_s3 = s3_below.saturating_mul(DECIMAL_PER_S3);
    let rest = if s3 < DECIMAL_S3_VALUES {
        let s1_below = if s1 < DECIMAL_S1_VALUES { s1 } else { DECIMAL_S1_VALUES };
        s2.saturating_mul(DECIMAL_S1_VALUES).saturating_add(s1_below)
    } else {
        0
    };
    whole_s4.saturating_add(whole_s3).saturating_add(rest)
}

/// The decimal sub-code at `index` among them in ascending order, 0 being
/// 0000; `None` past 3979.
const fn decimal_at(index: u16) -> Option<u16> {
    if index >= DECIMAL_VALUES {
        return None;
    }
    let s1 = index.wrapping_rem(DECIMAL_S1_VALUES);
    let s2 = index.wrapping_div(DECIMAL_S1_VALUES).wrapping_rem(DECIMAL_S2_VALUES);
    let s3 = index.wrapping_div(DECIMAL_PER_S3).wrapping_rem(DECIMAL_S3_VALUES);
    let s4 = index.wrapping_div(DECIMAL_PER_S4);
    Some(sub_code(s4 as u8, s3 as u8, s2 as u8, s1 as u8))
}

/// How many sub-codes with a hexadecimal digit are at or below `sub_code`.
const fn hexadecimal_to(sub_code: u16) -> u16 {
    let decimal = decimal_below(sub_code).saturating_add(is_decimal_sub_code(sub_code) as u16);
    rank(sub_code).saturating_add(1).saturating_sub(decimal)
}

/// The rank of 3F7F, the highest sub-code.
const LAST_RANK: u16 = (SUBCODE_VALUES - 1) as u16;

/// The `count`th sub-code with a hexadecimal digit, from 1, in ascending
/// order: found by halving the ranks, thirteen steps.
const fn hexadecimal_at(count: u16) -> Option<u16> {
    let (mut low, mut high) = (0u16, LAST_RANK);
    while low < high {
        // Halfway between, rounded down.
        let middle = low.wrapping_add(high.wrapping_sub(low) >> 1);
        if hexadecimal_to(from_rank(middle)) < count {
            low = middle.wrapping_add(1);
        } else {
            high = middle;
        }
    }
    let found = from_rank(low);
    if !is_decimal_sub_code(found) && hexadecimal_to(found) == count { Some(found) } else { None }
}

/// The sub-code a channel gives the sub-page at `place` (1 to `count`) of
/// a display page of `count` sub-pages (REQ-NUM-016, REQ-NUM-019):
/// - a page of one sub-page: 0000;
/// - a page of 2 to 79: places 1 to 79 take 0001 to 0079;
/// - a page of 80 to 8,069: only sub-codes above 0079, the 3,120 decimal
///   ones first in ascending order (0100 to 3979), then the 4,949 with a
///   hexadecimal digit in ascending order, 3F7F left out.
///
/// `None` for a place outside 1 to `count`, or a count of 0 or above
/// 8,069, which the code of practice cannot number.
#[must_use]
pub const fn sub_code_of(place: u16, count: u16) -> Option<u16> {
    if place == 0 || place > count {
        return None;
    }
    if count == 1 {
        return Some(SUBCODE_NONE);
    }
    if count <= SET_SUBPAGES {
        return decimal_at(place);
    }
    if count as usize > SUBCODES_ABOVE_SET {
        return None;
    }
    if place as usize <= DECIMAL_SUBCODES_ABOVE_SET {
        return decimal_at(place.saturating_add(SET_SUBPAGES));
    }
    hexadecimal_at(place.saturating_sub(DECIMAL_SUBCODES_ABOVE_SET as u16).saturating_add(HEXADECIMAL_TO_SET))
}

/// The place, 1 to `count`, that [`sub_code_of`] gives `sub_code` on a
/// display page of `count` sub-pages: its inverse. `None` when the
/// numbering gives no place that sub-code.
#[must_use]
pub const fn place_of(sub_code: u16, count: u16) -> Option<u16> {
    if !is_sub_code(sub_code) || count == 0 || count as usize > SUBCODES_ABOVE_SET {
        return None;
    }
    let place = if count == 1 {
        if sub_code == SUBCODE_NONE { 1 } else { return None }
    } else if count <= SET_SUBPAGES {
        if !is_decimal_sub_code(sub_code) || sub_code == SUBCODE_NONE || sub_code > SET_LAST_SUBCODE {
            return None;
        }
        decimal_below(sub_code)
    } else if sub_code <= SET_LAST_SUBCODE || sub_code == SUBCODE_ANY {
        return None;
    } else if is_decimal_sub_code(sub_code) {
        decimal_below(sub_code).saturating_sub(SET_SUBPAGES)
    } else {
        (DECIMAL_SUBCODES_ABOVE_SET as u16).saturating_add(hexadecimal_to(sub_code)).saturating_sub(HEXADECIMAL_TO_SET)
    };
    if place <= count { Some(place) } else { None }
}

/// The sub-code a viewer keys as four decimal digits, most significant
/// first (REQ-NAV-005, REQ-NUM-017): one of the 3,199 decimal sub-codes,
/// 0001 to 3979, or `None` for any other entry (a digit above 9, S4 above
/// 3, S2 above 7, or 0000).
#[must_use]
pub const fn decimal_sub_code(digits: [u8; SUBCODE_DIGITS]) -> Option<u16> {
    let [d4, d3, d2, d1] = digits;
    if d4 > S4_MASK as u8
        || d3 > DECIMAL_DIGIT_MAX
        || d2 > S2_MASK as u8
        || d1 > DECIMAL_DIGIT_MAX
        || (d4 | d3 | d2 | d1) == 0
    {
        return None;
    }
    Some(sub_code(d4, d3, d2, d1))
}

// ---------------------------------------------------------------------------
// The sub-code of an enhancement data page: nnXs (Annex A.1; REQ-NUM-022)
// ---------------------------------------------------------------------------

/// The sub-pages a POP, GPOP, DRCS or GDRCS page may have: S1 alone names
/// one, 0 to F (Annex A.1; clauses 10.5.1.1 and 10.5.2.1).
pub const ENHANCEMENT_SUBPAGES: usize = S1_MASK as usize + 1;
/// The sub-pages of a MOT or MIP page: one, S1 always 0 (clauses 10.6.1
/// and 11.3.2).
pub const TABLE_SUBPAGES: usize = 1;
/// nn, the last packet of an uninterrupted transmission: S4 the two most
/// significant bits and S3 the four least (Annex A.1; Annex B.2), so six
/// bits from S3's shift.
pub const ENHANCEMENT_LAST_PACKET_SHIFT: u32 = S3_SHIFT;
/// Mask of nn after its shift.
pub const ENHANCEMENT_LAST_PACKET_MASK: u16 = RANK_HIGH_MASK;
/// The lowest nn: packet X/1.
pub const ENHANCEMENT_LAST_PACKET_FIRST: u8 = 1;
/// The highest nn: 1 to 25 are X/1 to X/25 and 26 to 41 are X/26/0 to
/// X/26/15 (Annex A.1).
pub const ENHANCEMENT_LAST_PACKET_LAST: u8 = 41;

/// nn of an enhancement data page's sub-code.
#[must_use]
pub const fn enhancement_last_packet(sub_code: u16) -> u8 {
    ((sub_code >> ENHANCEMENT_LAST_PACKET_SHIFT) & ENHANCEMENT_LAST_PACKET_MASK) as u8
}

/// X, the update indicator: S2, incremented modulo 8 at each change of the
/// sub-page (Annex B.6, Table B.1).
#[must_use]
pub const fn enhancement_update(sub_code: u16) -> u8 {
    s2(sub_code)
}

/// s, the sub-page: S1, 0 for the first (Annex A.1).
#[must_use]
pub const fn enhancement_sub_page(sub_code: u16) -> u8 {
    s1(sub_code)
}

/// The sub-code nnXs of an enhancement data page's sub-page, each field
/// masked to its width (Annex A.1).
#[must_use]
pub const fn enhancement_sub_code(last_packet: u8, update: u8, sub_page: u8) -> u16 {
    ((last_packet as u16 & ENHANCEMENT_LAST_PACKET_MASK) << ENHANCEMENT_LAST_PACKET_SHIFT)
        | ((update as u16 & S2_MASK) << S2_SHIFT)
        | ((sub_page as u16 & S1_MASK) << S1_SHIFT)
}

// ---------------------------------------------------------------------------
// The sub-code of a data broadcasting page (EN 300 708 clause 4.3.1.2;
// REQ-NUM-024): no sub-pages
// ---------------------------------------------------------------------------

/// S3 value reserved for future use, with no 20 ms rule (EN 300 708
/// clause 4.3.1.2, Table 2).
pub const DATA_STREAM_RESERVED: u8 = 0xF;
/// Data streams that share one page number: S3 values 0 to E (EN 300 708
/// clause 4.3.1.2).
pub const DATA_STREAMS: usize = DATA_STREAM_RESERVED as usize;
/// The data stream identifier's least significant bit: clear when the
/// 20 ms rule applies before further packets of the page (EN 300 708
/// clause 4.3.1.2, Table 2).
pub const DATA_STREAM_NO_20MS_RULE: u8 = 0x1;
/// The continuity index counts modulo this, S1 (EN 300 708 clause
/// 4.3.1.2).
pub const DATA_CONTINUITY_MODULUS: u8 = S1_MASK as u8 + 1;
/// The last transmitted packet: S4 and S2 together, S2 the three least
/// significant bits (EN 300 708 clause 4.3.1.2).
pub const DATA_LAST_PACKET_S4_SHIFT: u32 = S2_MASK.count_ones();
/// The lowest and highest last transmitted packet (EN 300 708 clause
/// 4.3.1.2): 1 to 25, so 3F7F never occurs.
pub const DATA_LAST_PACKET_FIRST: u8 = 1;
/// The highest last transmitted packet.
pub const DATA_LAST_PACKET_LAST: u8 = 25;

/// The data stream identifier, S3.
#[must_use]
pub const fn data_stream(sub_code: u16) -> u8 {
    s3(sub_code)
}

/// True when the 20 ms rule applies to the stream (its identifier's least
/// significant bit is 0; EN 300 708 Table 2).
#[must_use]
pub const fn data_stream_has_20ms_rule(sub_code: u16) -> bool {
    data_stream(sub_code) & DATA_STREAM_NO_20MS_RULE == 0
}

/// The continuity index, S1.
#[must_use]
pub const fn data_continuity(sub_code: u16) -> u8 {
    s1(sub_code)
}

/// The last transmitted packet: S4 as the high bits, S2 as the low three.
#[must_use]
pub const fn data_last_packet(sub_code: u16) -> u8 {
    (s4(sub_code) << DATA_LAST_PACKET_S4_SHIFT) | s2(sub_code)
}

/// The sub-code of a data broadcasting page from its stream, continuity
/// index and last transmitted packet, each masked to its width.
#[must_use]
pub const fn data_sub_code(stream: u8, continuity: u8, last_packet: u8) -> u16 {
    sub_code(last_packet >> DATA_LAST_PACKET_S4_SHIFT, stream, last_packet, continuity)
}

#[cfg(test)]
mod tests;
