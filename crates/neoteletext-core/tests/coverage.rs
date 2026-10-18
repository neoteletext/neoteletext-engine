//! REQ-FNT-001: every character the character set tables can show that
//! Bedstead, the bundled font, draws as the placeholder of REQ-CHR-004.
//! A loaded font may supply them (REQ-FNT-005).

#![allow(clippy::unwrap_used)]

use neoteletext_core::charset::{CharRef, CharSet, G0, G0Set, G2Set, NationalOption, to_unicode};
use neoteletext_core::glyphs::{GlyphSet, PLACEHOLDER, SPACING_MARKS};
use std::collections::BTreeSet;

/// Characters 2/0 to 7/F of a set (clause 15).
const CODES: core::ops::RangeInclusive<u8> = 0x20..=0x7F;

fn sets() -> Vec<CharSet> {
    let mut sets = vec![CharSet::G0(G0 { set: G0Set::Latin, national: None })];
    sets.extend(NationalOption::ALL.map(|national| CharSet::G0(G0 { set: G0Set::Latin, national: Some(national) })));
    for set in [G0Set::Cyrillic1, G0Set::Cyrillic2, G0Set::Cyrillic3, G0Set::Greek, G0Set::Arabic, G0Set::Hebrew] {
        sets.push(CharSet::G0(G0 { set, national: None }));
    }
    sets.extend([G2Set::Latin, G2Set::Cyrillic, G2Set::Greek, G2Set::Arabic].map(CharSet::G2));
    sets
}

/// The characters drawn as the placeholder, by code point.
fn missing() -> BTreeSet<char> {
    let bedstead = GlyphSet::bedstead();
    let mut missing = BTreeSet::new();
    for set in sets() {
        for code in CODES {
            let character = CharRef::new(set, code);
            let unicode = to_unicode(character).base;
            if !unicode.is_control() && bedstead.glyph(character) == PLACEHOLDER {
                missing.insert(unicode);
            }
        }
    }
    for (_, spacing) in SPACING_MARKS {
        if bedstead.lookup(spacing).is_none() {
            missing.insert(spacing);
        }
    }
    missing
}

#[test]
fn every_required_character_bedstead_lacks_is_listed() {
    let missing = missing();
    // The Arabic sets map to the Arabic block and its presentation forms
    // (Tables 43 and 48).
    let is_arabic = |c: &char| {
        ('\u{0600}'..='\u{06FF}').contains(c)
            || ('\u{FB50}'..='\u{FDFF}').contains(c)
            || ('\u{FE70}'..='\u{FEFF}').contains(c)
    };
    let arabic: BTreeSet<char> = missing.iter().copied().filter(is_arabic).collect();
    let others: String = missing.iter().filter(|c| !arabic.contains(c)).collect();
    // Decision E.72 item 1: no Arabic letter, six Latin national
    // characters (Ă ă Ğ ğ Ƶ ₺) and two G2 characters (― ™). The
    // Vietnamese hook above's spacing form ˀ (REQ-CHR-014) comes from a
    // loaded font, as the rest of neoteletext's own sets do (REQ-FNT-012).
    assert_eq!(others, "\u{0102}\u{0103}\u{011E}\u{011F}\u{01B5}\u{02C0}\u{2015}\u{20BA}\u{2122}", "{others:?}");
    assert_eq!(arabic.len(), 99, "every Arabic character of Tables 43 and 48");
    println!("Bedstead lacks {} required characters: {others} and {} Arabic", missing.len(), arabic.len());
}
