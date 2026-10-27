//! The Magazine Inventory Page (clause 11.3, Figures 25 and 26, Table 24;
//! REQ-NUM-026). Its page number, mFD, is area NUM's and lives in
//! `limits`.

/// The last packet of page codes: X/1 to X/14 hold a pair of Hamming 8/4
/// bytes for each page number, in the order of Figure 26, which is the
/// MOT's order of Figure 20 (clause 11.3.2).
pub const MIP_CODE_LAST_PACKET: u8 = 14;
/// The first packet of sub-page counts (clause 11.3.4).
pub const MIP_COUNT_FIRST_PACKET: u8 = 15;
/// The last packet of sub-page counts (clause 11.3.4).
pub const MIP_COUNT_LAST_PACKET: u8 = 24;
/// Groups of three bytes in each packet X/15 to X/24 (Figure 26).
pub const MIP_COUNT_GROUPS_PER_PACKET: usize = 13;
/// Hamming 8/4 bytes in a group: D0 to D3, D4 to D7, D8 to D11, the
/// least significant first (clause 11.3.4).
pub const MIP_COUNT_GROUP_BYTES: usize = 3;
/// The data byte index of the first group: byte 7, since byte 6 is not
/// used (clause 11.3.4).
pub const MIP_COUNT_FIRST_INDEX: usize = 1;
/// Bits of a count the three bytes carry; D12, where it applies, is the
/// page code's least significant bit (clause 11.3.4).
pub const MIP_COUNT_GROUP_BITS: u32 = 12;
/// Bits of a Hamming 8/4 nibble.
pub const MIP_NIBBLE_BITS: u32 = 4;

// Table 24: the page codes. Ranges are inclusive.

/// 00: page not in transmission.
pub const MIP_NOT_IN_TRANSMISSION: u8 = 0x00;
/// 01: a single normal page.
pub const MIP_SINGLE_PAGE: u8 = 0x01;
/// 02 to 4F: a normal page of 2 to 79 sub-pages, the code being the count.
pub const MIP_MULTI_PAGE: [u8; 2] = [0x02, 0x4F];
/// 50: a normal page of 80 to 2^12 − 1 sub-pages, counted in X/15 to X/24.
pub const MIP_MULTI_PAGE_LARGE: u8 = 0x50;
/// 51: a normal page of 2^12 to 2^13 − 2 sub-pages, counted with D12 set.
pub const MIP_MULTI_PAGE_LARGEST: u8 = 0x51;
/// 70 to 77: subtitle pages; the three low bits are C12 to C14.
pub const MIP_SUBTITLE: [u8; 2] = [0x70, 0x77];
/// 78: the subtitle menu page.
pub const MIP_SUBTITLE_MENU: u8 = 0x78;
/// 79: a page not following the normal sub-code rules, such as an alarm
/// clock page; one page of storage (REQ-NUM-021).
pub const MIP_NOT_FOLLOWING_RULES: u8 = 0x79;
/// 7A: a TV programme related warning page.
pub const MIP_TV_WARNING: u8 = 0x7A;
/// 7B: current TV programme information, a multi-page set counted in X/15
/// to X/24 (2 to 2^12 − 1).
pub const MIP_PROGRAMME_MULTI: u8 = 0x7B;
/// 7C: current TV programme information, a single page.
pub const MIP_PROGRAMME_SINGLE: u8 = 0x7C;
/// 7D: "Now and Next" TV programmes.
pub const MIP_NOW_AND_NEXT: u8 = 0x7D;
/// 7E: an index page to TV-related pages, a multi-page set counted in X/15
/// to X/24.
pub const MIP_TV_INDEX_MULTI: u8 = 0x7E;
/// 7F: an index page to TV-related pages, a single page.
pub const MIP_TV_INDEX_SINGLE: u8 = 0x7F;
/// 80: a page transmitted but not part of the public service.
pub const MIP_NOT_PUBLIC: u8 = 0x80;
/// 81: a single page of TV schedule information.
pub const MIP_SCHEDULE_SINGLE: u8 = 0x81;
/// 82 to CF: TV schedule pages of 2 to 79 sub-pages, the code less 80
/// being the count.
pub const MIP_SCHEDULE_MULTI: [u8; 2] = [0x82, 0xCF];
/// The offset of the schedule codes from their counts.
pub const MIP_SCHEDULE_OFFSET: u8 = 0x80;
/// D0: TV schedule pages, 80 to 2^12 − 1 sub-pages counted.
pub const MIP_SCHEDULE_LARGE: u8 = 0xD0;
/// D1: TV schedule pages, 2^12 to 2^13 − 2 sub-pages counted.
pub const MIP_SCHEDULE_LARGEST: u8 = 0xD1;
/// E0 to E4: data broadcasting pages of EN 300 708 (REQ-NUM-024).
pub const MIP_DATA_BROADCASTING: [u8; 2] = [0xE0, 0xE4];
/// E0: a CA data broadcasting page, 1 to 2^12 − 1 sub-pages counted.
pub const MIP_DATA_CA_LARGE: u8 = 0xE0;
/// E1: a CA data broadcasting page, 2^12 to 2^13 − 2 sub-pages counted.
pub const MIP_DATA_CA_LARGEST: u8 = 0xE1;
/// E5: a DRCS page, use not defined.
pub const MIP_DRCS_UNDEFINED: u8 = 0xE5;
/// E6: an object page, use not defined.
pub const MIP_OBJECT_UNDEFINED: u8 = 0xE6;
/// E7: a systems page without displayable element, such as the MOT and the
/// MIP.
pub const MIP_SYSTEM_NO_DISPLAY: u8 = 0xE7;
/// E8 to EB: DRCS pages, by how the MOT references them.
pub const MIP_DRCS: [u8; 2] = [0xE8, 0xEB];
/// EC to EF: object pages, by how the MOT references them.
pub const MIP_OBJECT: [u8; 2] = [0xEC, 0xEF];
/// F0 to F3: systems pages for broadcasters' use.
pub const MIP_BROADCASTER_SYSTEM: [u8; 2] = [0xF0, 0xF3];
/// F4 to F6: engineering test pages.
pub const MIP_ENGINEERING: [u8; 2] = [0xF4, 0xF6];
/// F7: a systems page with a displayable element.
pub const MIP_SYSTEM_DISPLAY: u8 = 0xF7;
/// F8: a keyword search list page, a multi-page set counted in X/15 to
/// X/24.
pub const MIP_KEYWORD_MULTI: u8 = 0xF8;
/// F9: a keyword search list page, a single page.
pub const MIP_KEYWORD_SINGLE: u8 = 0xF9;
/// FC: a trigger message page. Table 24 lists FA to FC as reserved and
/// then FC as this; the specific row is taken (REQ-NUM-026).
pub const MIP_TRIGGER: u8 = 0xFC;
/// FD: Automatic Channel Installation (Annex L).
pub const MIP_ACI: u8 = 0xFD;
/// FE: a TOP page: BTT, AIT, MPT or MPT-EX (REQ-NUM-028).
pub const MIP_TOP: u8 = 0xFE;
/// Codes whose number of sub-pages is in packets X/15 to X/24 (clause
/// 11.3.4; Table 24).
pub const MIP_COUNTED: [u8; 9] = [
    MIP_MULTI_PAGE_LARGE,
    MIP_MULTI_PAGE_LARGEST,
    MIP_PROGRAMME_MULTI,
    MIP_TV_INDEX_MULTI,
    MIP_SCHEDULE_LARGE,
    MIP_SCHEDULE_LARGEST,
    MIP_DATA_CA_LARGE,
    MIP_DATA_CA_LARGEST,
    MIP_KEYWORD_MULTI,
];
/// Codes of the counted ones whose least significant bit is the count's
/// D12, the pairs that reach 2^13 − 2 (clause 11.3.4).
pub const MIP_COUNT_D12_FROM_CODE: [u8; 6] = [
    MIP_MULTI_PAGE_LARGE,
    MIP_MULTI_PAGE_LARGEST,
    MIP_SCHEDULE_LARGE,
    MIP_SCHEDULE_LARGEST,
    MIP_DATA_CA_LARGE,
    MIP_DATA_CA_LARGEST,
];
