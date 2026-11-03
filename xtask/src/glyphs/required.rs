//! The lists of required characters and joined forms, generated from the
//! core's character set tables: every character a glyph set must draw.

use neoteletext_core::charset::{
    CharRef, CharSet, G0, G0Set, G2Set, JoinForm, NationalOption, compositions, extended_compositions, glyph_key,
    joined_forms, mark_sequences,
};
use neoteletext_core::glyphs::SPACING_MARKS;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

pub(super) const REQUIRED: &str = "crates/neoteletext-core/src/glyphs/required.txt";
pub(super) const FORMS: &str = "crates/neoteletext-core/src/glyphs/forms.txt";
/// Characters 2/0 to 7/F of a set (clause 15).
const FIRST_CODE: u8 = 0x20;
const LAST_CODE: u8 = 0x7F;
/// The standard's whole G0 sets, which [`sets`] lists first (Table 32).
const STANDARD_WHOLE_G0: usize = 6;
/// The standard's G2 sets, which [`sets`] lists first (Table 33).
const STANDARD_G2: usize = 4;

/// How `required.txt` names the Latin compositions of decision E.26, which
/// `third_party/noto/settings.txt` selects with `uses:`.
const COMPOSED: &str = "Composed";
/// How it names those neoteletext's own sets add (REQ-CHR-014).
const EXTENDED_COMPOSED: &str = "ComposedExtended";

/// One character set the renderer can show, with its characters.
struct Set {
    name: String,
    /// The set is the standard's, not one of neoteletext's own
    /// (REQ-CHR-012), so a build without `extended-sets` draws it.
    standard: bool,
    characters: Vec<(u8, char)>,
}

/// Every G0 set, with each national option of the Latin set, and every G2
/// set.
fn sets() -> Vec<Set> {
    let mut g0: Vec<(String, bool, CharSet)> =
        vec![("G0 Latin".to_string(), true, CharSet::G0(G0 { set: G0Set::Latin, national: None }))];
    let standard_options = NationalOption::ALL.into_iter().map(|national| (true, national));
    for (standard, national) in standard_options.chain(NationalOption::EXTENDED.map(|national| (false, national))) {
        let set = CharSet::G0(G0 { set: G0Set::Latin, national: Some(national) });
        g0.push((format!("G0 Latin {national:?}"), standard, set));
    }
    // The standard's sets, then neoteletext's own (REQ-CHR-012).
    let whole = [
        G0Set::Cyrillic1,
        G0Set::Cyrillic2,
        G0Set::Cyrillic3,
        G0Set::Greek,
        G0Set::Arabic,
        G0Set::Hebrew,
        G0Set::Vietnamese,
        G0Set::YorubaIgbo,
        G0Set::Ukrainian,
        G0Set::Belarusian,
        G0Set::Macedonian,
        G0Set::Serbian,
        G0Set::KyrgyzMongolian,
        G0Set::Tatar,
        G0Set::Tajik,
        G0Set::Georgian,
        G0Set::Armenian,
        G0Set::Tifinagh,
        G0Set::Persian,
        G0Set::Urdu,
        G0Set::Pashto,
        G0Set::Sindhi,
        G0Set::Sorani,
        G0Set::Uyghur,
        G0Set::Balochi,
        G0Set::ArabicPlain,
        G0Set::Thai,
        G0Set::Lao,
        G0Set::Hiragana,
        G0Set::Katakana,
    ];
    for (index, set) in whole.into_iter().enumerate() {
        g0.push((format!("G0 {set:?}"), index < STANDARD_WHOLE_G0, CharSet::G0(G0 { set, national: None })));
    }
    let g2_sets = [
        G2Set::Latin,
        G2Set::Cyrillic,
        G2Set::Greek,
        G2Set::Arabic,
        G2Set::Vietnamese,
        G2Set::Armenian,
        G2Set::Thai,
        G2Set::Lao,
    ];
    for (index, set) in g2_sets.into_iter().enumerate() {
        g0.push((format!("G2 {set:?}"), index < STANDARD_G2, CharSet::G2(set)));
    }
    g0.into_iter()
        .map(|(name, standard, set)| Set {
            name,
            standard,
            characters: (FIRST_CODE..=LAST_CODE)
                .map(|code| (code, glyph_key(CharRef::new(set, code))))
                .filter(|(_, c)| !c.is_control())
                .collect(),
        })
        .collect()
}

/// The required characters and where each is used.
fn required() -> BTreeMap<char, Vec<String>> {
    let mut out: BTreeMap<char, Vec<String>> = BTreeMap::new();
    for set in sets() {
        for (code, character) in &set.characters {
            out.entry(*character).or_default().push(format!("{} {}/{:X}", set.name, code >> 4, code & 0xF));
        }
    }
    for (_, spacing) in SPACING_MARKS {
        out.entry(spacing).or_default().push("spacing mark (decision E.26)".to_string());
    }
    for &(base, mark, composed) in compositions() {
        out.entry(composed).or_default().push(format!(
            "{COMPOSED} U+{:04X}+U+{:04X}",
            u32::from(base),
            u32::from(mark)
        ));
    }
    for &(base, mark, composed) in extended_compositions() {
        out.entry(composed).or_default().push(format!(
            "{EXTENDED_COMPOSED} U+{:04X}+U+{:04X}",
            u32::from(base),
            u32::from(mark)
        ));
    }
    out
}

/// The characters a build without `extended-sets` draws: those of the
/// standard's sets, the spacing marks and the Latin compositions. The rest
/// of the font is compiled only with that feature (REQ-FNT-001).
pub(super) fn standard_characters() -> BTreeSet<char> {
    let used = sets().into_iter().filter(|set| set.standard).flat_map(|set| set.characters).map(|(_, c)| c);
    let spacing = SPACING_MARKS.iter().map(|&(_, spacing)| spacing);
    let composed = compositions().iter().map(|&(_, _, composed)| composed);
    used.chain(spacing).chain(composed).collect()
}

pub(super) fn required_text() -> String {
    let mut text = String::from(
        "; Generated by `cargo xtask glyphs` from the character set tables. Do not edit.\n\
         ; Every character a glyph set must draw: code point, character, where it is used.\n",
    );
    for (character, uses) in required() {
        let shown = if character == ' ' { "SPACE".to_string() } else { character.to_string() };
        let _ = writeln!(text, "U+{:04X} {shown} {}", u32::from(character), uses.join("; "));
    }
    text
}

pub(super) fn forms_text() -> String {
    let mut text = String::from(
        "; Generated by `cargo xtask glyphs` from the core's joined forms. Do not edit.\n\
         ; Each joined form a font draws (REQ-CHR-015): code point, letter, form;\n\
         ; and each Thai and Lao glyph of two marks (REQ-CHR-016): key, marks, sequence.\n",
    );
    for &(letter, form, point) in joined_forms() {
        let name = match form {
            JoinForm::Isolated => "isolated",
            JoinForm::Final => "final",
            JoinForm::Initial => "initial",
            JoinForm::Medial => "medial",
        };
        let _ = writeln!(text, "U+{:04X} U+{:04X} {name}", u32::from(point), u32::from(letter));
    }
    // The Thai and Lao G2 glyphs of two marks (REQ-CHR-016).
    for &(key, first, second) in mark_sequences() {
        let _ =
            writeln!(text, "U+{:04X} U+{:04X}+U+{:04X} sequence", u32::from(key), u32::from(first), u32::from(second));
    }
    text
}
