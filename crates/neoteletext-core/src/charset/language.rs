//! The one table from a language to the character set that writes it
//! (REQ-2181): every language NeoTeletext writes, by its BCP 47 language
//! and script subtags, with the Table 32 value of its set and, where
//! EN 300 706 has one, the standard's own set for it.

use crate::consts::DESIGNATION_GROUP_SHIFT;

/// One language written in one script.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LanguageSet {
    /// The BCP 47 language subtag.
    pub language: &'static str,
    /// The ISO 15924 script subtag of the script this entry writes.
    pub script: &'static str,
    /// True for the script the language is written in when a tag names
    /// none.
    pub default: bool,
    /// The Table 32 value of the set that writes it.
    pub value: u8,
    /// The Table 32 value of EN 300 706's own set for the language, if the
    /// standard has one: the group a receiver assumes for its pages, and
    /// rōmaji's set on pages for Level 3.5 and below.
    pub standard: Option<u8>,
}

/// A Table 32 value from its group and option.
const fn value(group: u8, option: u8) -> u8 {
    (group << DESIGNATION_GROUP_SHIFT) | option
}

const fn entry(language: &'static str, script: &'static str, value: u8, standard: Option<u8>) -> LanguageSet {
    LanguageSet { language, script, default: true, value, standard }
}

const fn other_script(language: &'static str, script: &'static str, value: u8, standard: Option<u8>) -> LanguageSet {
    LanguageSet { language, script, default: false, value, standard }
}

/// The standard's sets (Table 32).
const ENGLISH: u8 = value(0b0000, 0b000);
const GERMAN: u8 = value(0b0000, 0b001);
const SWEDISH_FINNISH_HUNGARIAN: u8 = value(0b0000, 0b010);
const ITALIAN: u8 = value(0b0000, 0b011);
const FRENCH: u8 = value(0b0000, 0b100);
const PORTUGUESE_SPANISH: u8 = value(0b0000, 0b101);
const CZECH_SLOVAK: u8 = value(0b0000, 0b110);
const POLISH: u8 = value(0b0001, 0b000);
const TURKISH: u8 = value(0b0010, 0b110);
const SERBIAN_CROATIAN_SLOVENIAN: u8 = value(0b0011, 0b101);
const ROMANIAN: u8 = value(0b0011, 0b111);
const CYRILLIC_SERBIAN_CROATIAN: u8 = value(0b0100, 0b000);
const ESTONIAN: u8 = value(0b0100, 0b010);
const LETTISH_LITHUANIAN: u8 = value(0b0100, 0b011);
const RUSSIAN_BULGARIAN: u8 = value(0b0100, 0b100);
const CYRILLIC_UKRAINIAN: u8 = value(0b0100, 0b101);
const GREEK: u8 = value(0b0110, 0b111);
const ARABIC_TABLE_44: u8 = value(0b1000, 0b111);
const HEBREW: u8 = value(0b1010, 0b101);

/// Every language NeoTeletext writes (DEC-602 item 1, DEC-2184,
/// DEC-2187): the standard's sets, then NeoTeletext's own.
pub const LANGUAGES: [LanguageSet; 76] = [
    entry("en", "Latn", ENGLISH, Some(ENGLISH)),
    entry("id", "Latn", ENGLISH, Some(ENGLISH)),
    entry("sw", "Latn", ENGLISH, Some(ENGLISH)),
    entry("de", "Latn", GERMAN, Some(GERMAN)),
    entry("sv", "Latn", SWEDISH_FINNISH_HUNGARIAN, Some(SWEDISH_FINNISH_HUNGARIAN)),
    entry("fi", "Latn", SWEDISH_FINNISH_HUNGARIAN, Some(SWEDISH_FINNISH_HUNGARIAN)),
    entry("hu", "Latn", SWEDISH_FINNISH_HUNGARIAN, Some(SWEDISH_FINNISH_HUNGARIAN)),
    entry("it", "Latn", ITALIAN, Some(ITALIAN)),
    entry("fr", "Latn", FRENCH, Some(FRENCH)),
    entry("pt", "Latn", PORTUGUESE_SPANISH, Some(PORTUGUESE_SPANISH)),
    entry("es", "Latn", PORTUGUESE_SPANISH, Some(PORTUGUESE_SPANISH)),
    entry("cs", "Latn", CZECH_SLOVAK, Some(CZECH_SLOVAK)),
    entry("sk", "Latn", CZECH_SLOVAK, Some(CZECH_SLOVAK)),
    entry("pl", "Latn", POLISH, Some(POLISH)),
    entry("tr", "Latn", TURKISH, Some(TURKISH)),
    // DEC-602 item 1: Kazakh in the Latin alphabet Kazakhstan adopted.
    entry("kk", "Latn", TURKISH, Some(TURKISH)),
    entry("hr", "Latn", SERBIAN_CROATIAN_SLOVENIAN, Some(SERBIAN_CROATIAN_SLOVENIAN)),
    entry("sl", "Latn", SERBIAN_CROATIAN_SLOVENIAN, Some(SERBIAN_CROATIAN_SLOVENIAN)),
    other_script("sr", "Latn", SERBIAN_CROATIAN_SLOVENIAN, Some(SERBIAN_CROATIAN_SLOVENIAN)),
    entry("ro", "Latn", ROMANIAN, Some(ROMANIAN)),
    entry("et", "Latn", ESTONIAN, Some(ESTONIAN)),
    entry("lv", "Latn", LETTISH_LITHUANIAN, Some(LETTISH_LITHUANIAN)),
    entry("lt", "Latn", LETTISH_LITHUANIAN, Some(LETTISH_LITHUANIAN)),
    entry("ru", "Cyrl", RUSSIAN_BULGARIAN, Some(RUSSIAN_BULGARIAN)),
    entry("bg", "Cyrl", RUSSIAN_BULGARIAN, Some(RUSSIAN_BULGARIAN)),
    entry("el", "Grek", GREEK, Some(GREEK)),
    entry("he", "Hebr", HEBREW, Some(HEBREW)),
    // NeoTeletext's own sets (DEC-602): Ukrainian, Serbian and Arabic
    // have a set of the standard too, for the receiver's region.
    entry("uk", "Cyrl", value(0b0111, 0b000), Some(CYRILLIC_UKRAINIAN)),
    entry("sr", "Cyrl", value(0b0111, 0b011), Some(CYRILLIC_SERBIAN_CROATIAN)),
    entry("ar", "Arab", value(0b1001, 0b111), Some(ARABIC_TABLE_44)),
    entry("ha", "Latn", value(0b0101, 0b000), None),
    entry("ff", "Latn", value(0b0101, 0b000), None),
    entry("ak", "Latn", value(0b0101, 0b001), None),
    entry("bm", "Latn", value(0b0101, 0b001), None),
    entry("dyu", "Latn", value(0b0101, 0b001), None),
    entry("din", "Latn", value(0b0101, 0b001), None),
    entry("fon", "Latn", value(0b0101, 0b001), None),
    entry("kri", "Latn", value(0b0101, 0b001), None),
    entry("ln", "Latn", value(0b0101, 0b001), None),
    entry("mnk", "Latn", value(0b0101, 0b001), None),
    entry("men", "Latn", value(0b0101, 0b001), None),
    entry("nus", "Latn", value(0b0101, 0b001), None),
    entry("ee", "Latn", value(0b0101, 0b010), None),
    entry("kab", "Latn", value(0b0101, 0b011), None),
    entry("tzm", "Latn", value(0b0101, 0b011), None),
    entry("az", "Latn", value(0b0101, 0b100), None),
    entry("kr", "Latn", value(0b0101, 0b101), None),
    entry("mos", "Latn", value(0b0101, 0b101), None),
    entry("yo", "Latn", value(0b0101, 0b110), None),
    entry("ig", "Latn", value(0b0101, 0b110), None),
    entry("vi", "Latn", value(0b0101, 0b111), None),
    entry("be", "Cyrl", value(0b0111, 0b001), None),
    entry("mk", "Cyrl", value(0b0111, 0b010), None),
    entry("ky", "Cyrl", value(0b0111, 0b101), None),
    entry("mn", "Cyrl", value(0b0111, 0b101), None),
    entry("tt", "Cyrl", value(0b0111, 0b110), None),
    entry("tg", "Cyrl", value(0b0111, 0b111), None),
    entry("fa", "Arab", value(0b1001, 0b000), None),
    entry("prs", "Arab", value(0b1001, 0b000), None),
    entry("ur", "Arab", value(0b1001, 0b001), None),
    entry("ps", "Arab", value(0b1001, 0b010), None),
    entry("sd", "Arab", value(0b1001, 0b011), None),
    entry("ckb", "Arab", value(0b1001, 0b100), None),
    entry("ug", "Arab", value(0b1001, 0b101), None),
    entry("bal", "Arab", value(0b1001, 0b110), None),
    entry("ka", "Geor", value(0b1011, 0b000), None),
    entry("hy", "Armn", value(0b1011, 0b001), None),
    entry("zgh", "Tfng", value(0b1011, 0b010), None),
    other_script("tzm", "Tfng", value(0b1011, 0b010), None),
    entry("th", "Thai", value(0b1100, 0b010), None),
    entry("lo", "Laoo", value(0b1100, 0b100), None),
    // Japanese (DEC-2184, DEC-2187): kana by default; rōmaji is the
    // standard's Latin English with marks on pages for Level 3.5 and below.
    entry("ja", "Jpan", value(0b1101, 0b001), None),
    other_script("ja", "Hira", value(0b1101, 0b001), None),
    other_script("ja", "Hrkt", value(0b1101, 0b001), None),
    other_script("ja", "Kana", value(0b1101, 0b010), None),
    other_script("ja", "Latn", value(0b1101, 0b000), Some(ENGLISH)),
];

/// The group of NeoTeletext's Japanese sets, which need its own receiver
/// (DEC-2184, DEC-2187).
const JAPANESE_GROUP: u8 = 0b1101;

/// The entry for a BCP 47 tag: its language subtag and, where the tag
/// names one, its script subtag, matched without regard to case; region
/// and other subtags are ignored. A tag naming no script takes the
/// language's default script. `None` for a language the table lacks.
#[must_use]
pub fn language(tag: &str) -> Option<&'static LanguageSet> {
    let mut parts = tag.split(['-', '_']);
    let language = parts.next()?;
    // BCP 47: the script subtag, if any, is the one of four letters after
    // the language.
    let script =
        parts.next().filter(|part| part.len() == SCRIPT_LENGTH && part.chars().all(|c| c.is_ascii_alphabetic()));
    LANGUAGES.iter().find(|entry| {
        entry.language.eq_ignore_ascii_case(language)
            && script.map_or(entry.default, |script| entry.script.eq_ignore_ascii_case(script))
    })
}

/// ISO 15924 script subtags are four letters.
const SCRIPT_LENGTH: usize = 4;

/// The Table 32 value a page in `tag` is written in: on a page for
/// NeoTeletext's own receivers (`own_level`), the language's set; for
/// Level 3.5 and below, the same, except that the Japanese sets give way
/// to the standard's set where it has one and to nothing where it has
/// none (DEC-2184, DEC-2187 item 9).
#[must_use]
pub fn language_value(tag: &str, own_level: bool) -> Option<u8> {
    let found = language(tag)?;
    if own_level || found.value >> DESIGNATION_GROUP_SHIFT != JAPANESE_GROUP {
        return Some(found.value);
    }
    found.standard
}

/// ISO 639-2 codes, as subtitle streams carry them, and the BCP 47 tags
/// they stand for in [`LANGUAGES`]. Serbian subtitles are Latin, as the
/// receiver region of the standard's Serbian option is.
const ISO_639_2: [(&str, &str); 22] = [
    ("pol", "pl"),
    ("tur", "tr"),
    ("srp", "sr-Latn"),
    ("scc", "sr-Latn"),
    ("hrv", "hr"),
    ("scr", "hr"),
    ("slv", "sl"),
    ("ron", "ro"),
    ("rum", "ro"),
    ("rus", "ru"),
    ("bul", "bg"),
    ("ukr", "uk"),
    ("est", "et"),
    ("lav", "lv"),
    ("lit", "lt"),
    ("gre", "el"),
    ("ell", "el"),
    ("ara", "ar"),
    ("heb", "he"),
    ("eng", "en"),
    ("ger", "de"),
    ("deu", "de"),
];

/// The Table 32 group a receiver assumes for pages in the language of an
/// ISO 639-2 code (REQ-2181 item 5): the group of the standard's set for
/// it, else of its own set; group 0000 for a code the table lacks.
#[must_use]
pub fn region_for_iso_639_2(code: [u8; 3]) -> u8 {
    let mut lower = code;
    lower.make_ascii_lowercase();
    ISO_639_2
        .iter()
        .find(|(iso, _)| iso.as_bytes() == lower)
        .and_then(|(_, tag)| language(tag))
        .map_or(0, |found| found.standard.unwrap_or(found.value) >> DESIGNATION_GROUP_SHIFT)
}
