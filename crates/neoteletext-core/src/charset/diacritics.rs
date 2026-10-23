//! The diacritical marks of G2 column 4 (REQ-CHR-008).


/// The combining character of each column 4 mark, indexed by mark number.
/// 4/0 is "no mark". The last is the Vietnamese hook above, numbered
/// [`DIACRITIC_HOOK_ABOVE`](crate::consts::DIACRITIC_HOOK_ABOVE).
pub(super) const COMBINING_MARKS: [Option<char>; 17] = [
    None,
    Some('\u{0300}'), // 4/1 grave
    Some('\u{0301}'), // 4/2 acute
    Some('\u{0302}'), // 4/3 circumflex
    Some('\u{0303}'), // 4/4 tilde
    Some('\u{0304}'), // 4/5 macron
    Some('\u{0306}'), // 4/6 breve
    Some('\u{0307}'), // 4/7 dot above
    Some('\u{0308}'), // 4/8 diaeresis
    Some('\u{0323}'), // 4/9 dot below
    Some('\u{030A}'), // 4/A ring above
    Some('\u{0327}'), // 4/B cedilla
    Some('\u{0332}'), // 4/C low line
    Some('\u{030B}'), // 4/D double acute
    Some('\u{0328}'), // 4/E ogonek
    Some('\u{030C}'), // 4/F caron
    Some('\u{0309}'), // Vietnamese 4/C hook above
];

pub(super) fn diacritic_combining(mark: u8) -> Option<char> {
    COMBINING_MARKS.get(usize::from(mark)).copied().flatten()
}
