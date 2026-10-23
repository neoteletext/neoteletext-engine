//! G3 smooth mosaics and line drawing (Table 48).

use super::code_table::CodeTable;

/// Table 48, converted as Unicode document L2/19-025 and its mapping file
/// `TELTXTG3.TXT` assign the positions (REQ-CHR-011, listed in
/// `research/notes-level25-drcs.md` section 9.3). The smooth mosaics map to
/// Symbols for Legacy Computing (U+1FB3C to U+1FB6F, U+1FB70, U+1FB75) and
/// Geometric Shapes; 2/F, the alternate dots, to U+2592 MEDIUM SHADE; the
/// diagonal line pieces of column 4 to U+1FBA0 to U+1FBA7. Line pieces
/// that join a thick bar to a thin line (4/0 to 4/3 and 4/C) use the box
/// drawing characters with a heavy horizontal or right/left arm and light
/// other arms; all other lines are light. 4/D, 4/E and 4/F are a bullet, a
/// black circle and a white circle, and 5/B to 5/E the triangle-headed
/// arrows. 5/F is the background-colour space of note 1, and 6/E, 6/F, 7/E
/// and 7/F are unassigned (note 2); all four display as a space
/// (REQ-CHR-007).
pub(super) const G3: CodeTable = [
    '\u{1FB3C}', '\u{1FB3D}', '\u{1FB3E}', '\u{1FB3F}', '\u{1FB40}', '\u{25E3}', '\u{1FB41}',
    '\u{1FB42}', '\u{1FB43}', '\u{1FB44}', '\u{1FB45}', '\u{1FB46}', '\u{1FB68}', '\u{1FB69}',
    '\u{1FB70}', '\u{2592}', //
    '\u{1FB47}', '\u{1FB48}', '\u{1FB49}', '\u{1FB4A}', '\u{1FB4B}', '\u{25E2}', '\u{1FB4C}',
    '\u{1FB4D}', '\u{1FB4E}', '\u{1FB4F}', '\u{1FB50}', '\u{1FB51}', '\u{1FB6A}', '\u{1FB6B}',
    '\u{1FB75}', '\u{2588}', //
    '\u{2537}', '\u{252F}', '\u{251D}', '\u{2525}', '\u{1FBA4}', '\u{1FBA5}', '\u{1FBA6}',
    '\u{1FBA7}', '\u{1FBA0}', '\u{1FBA1}', '\u{1FBA2}', '\u{1FBA3}', '\u{253F}', '\u{2022}',
    '\u{25CF}', '\u{25CB}', //
    '\u{2502}', '\u{2500}', '\u{250C}', '\u{2510}', '\u{2514}', '\u{2518}', '\u{251C}',
    '\u{2524}', '\u{252C}', '\u{2534}', '\u{253C}', '\u{2B62}', '\u{2B60}', '\u{2B61}',
    '\u{2B63}', ' ', //
    '\u{1FB52}', '\u{1FB53}', '\u{1FB54}', '\u{1FB55}', '\u{1FB56}', '\u{25E5}', '\u{1FB57}',
    '\u{1FB58}', '\u{1FB59}', '\u{1FB5A}', '\u{1FB5B}', '\u{1FB5C}', '\u{1FB6C}', '\u{1FB6D}',
    ' ', ' ', //
    '\u{1FB5D}', '\u{1FB5E}', '\u{1FB5F}', '\u{1FB60}', '\u{1FB61}', '\u{25E4}', '\u{1FB62}',
    '\u{1FB63}', '\u{1FB64}', '\u{1FB65}', '\u{1FB66}', '\u{1FB67}', '\u{1FB6E}', '\u{1FB6F}',
    ' ', ' ', //
];
