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
    /// The language's English name, as a list of languages shows it
    /// (REQ-2215), with the script where the language has an entry for
    /// another.
    pub name: &'static str,
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

const fn entry(
    language: &'static str,
    script: &'static str,
    name: &'static str,
    value: u8,
    standard: Option<u8>,
) -> LanguageSet {
    LanguageSet { language, script, name, default: true, value, standard }
}

const fn other_script(
    language: &'static str,
    script: &'static str,
    name: &'static str,
    value: u8,
    standard: Option<u8>,
) -> LanguageSet {
    LanguageSet { language, script, name, default: false, value, standard }
}

/// The ISO 15924 scripts written right to left among [`LANGUAGES`]: the
/// Arabic-script sets and Hebrew, which the page writer puts in visual
/// order (REQ-CHR-015, DEC-606).
const RIGHT_TO_LEFT_SCRIPTS: [&str; 2] = ["Arab", "Hebr"];
/// The scripts whose marks take the row above and the row below their
/// letters: Thai and Lao (DEC-604).
const MARK_ROW_SCRIPTS: [&str; 2] = ["Thai", "Laoo"];

impl LanguageSet {
    /// True when the language is written right to left.
    #[must_use]
    pub fn right_to_left(&self) -> bool {
        RIGHT_TO_LEFT_SCRIPTS.contains(&self.script)
    }

    /// The rows above and below its letters the language's marks take: 1
    /// for Thai and Lao (DEC-604), 0 for every other.
    #[must_use]
    pub fn mark_rows(&self) -> u8 {
        u8::from(MARK_ROW_SCRIPTS.contains(&self.script))
    }

    /// The BCP 47 tag that names this entry exactly: its language subtag,
    /// and its script subtag where the script is not the language's
    /// default, such as `sr-Latn`.
    #[must_use]
    pub const fn tag_parts(&self) -> (&'static str, Option<&'static str>) {
        (self.language, if self.default { None } else { Some(self.script) })
    }
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
    entry("en", "Latn", "English", ENGLISH, Some(ENGLISH)),
    entry("id", "Latn", "Indonesian", ENGLISH, Some(ENGLISH)),
    entry("sw", "Latn", "Swahili", ENGLISH, Some(ENGLISH)),
    entry("de", "Latn", "German", GERMAN, Some(GERMAN)),
    entry("sv", "Latn", "Swedish", SWEDISH_FINNISH_HUNGARIAN, Some(SWEDISH_FINNISH_HUNGARIAN)),
    entry("fi", "Latn", "Finnish", SWEDISH_FINNISH_HUNGARIAN, Some(SWEDISH_FINNISH_HUNGARIAN)),
    entry("hu", "Latn", "Hungarian", SWEDISH_FINNISH_HUNGARIAN, Some(SWEDISH_FINNISH_HUNGARIAN)),
    entry("it", "Latn", "Italian", ITALIAN, Some(ITALIAN)),
    entry("fr", "Latn", "French", FRENCH, Some(FRENCH)),
    entry("pt", "Latn", "Portuguese", PORTUGUESE_SPANISH, Some(PORTUGUESE_SPANISH)),
    entry("es", "Latn", "Spanish", PORTUGUESE_SPANISH, Some(PORTUGUESE_SPANISH)),
    entry("cs", "Latn", "Czech", CZECH_SLOVAK, Some(CZECH_SLOVAK)),
    entry("sk", "Latn", "Slovak", CZECH_SLOVAK, Some(CZECH_SLOVAK)),
    entry("pl", "Latn", "Polish", POLISH, Some(POLISH)),
    entry("tr", "Latn", "Turkish", TURKISH, Some(TURKISH)),
    // DEC-602 item 1: Kazakh in the Latin alphabet Kazakhstan adopted.
    entry("kk", "Latn", "Kazakh", TURKISH, Some(TURKISH)),
    entry("hr", "Latn", "Croatian", SERBIAN_CROATIAN_SLOVENIAN, Some(SERBIAN_CROATIAN_SLOVENIAN)),
    entry("sl", "Latn", "Slovenian", SERBIAN_CROATIAN_SLOVENIAN, Some(SERBIAN_CROATIAN_SLOVENIAN)),
    other_script("sr", "Latn", "Serbian (Latin script)", SERBIAN_CROATIAN_SLOVENIAN, Some(SERBIAN_CROATIAN_SLOVENIAN)),
    entry("ro", "Latn", "Romanian", ROMANIAN, Some(ROMANIAN)),
    entry("et", "Latn", "Estonian", ESTONIAN, Some(ESTONIAN)),
    entry("lv", "Latn", "Latvian", LETTISH_LITHUANIAN, Some(LETTISH_LITHUANIAN)),
    entry("lt", "Latn", "Lithuanian", LETTISH_LITHUANIAN, Some(LETTISH_LITHUANIAN)),
    entry("ru", "Cyrl", "Russian", RUSSIAN_BULGARIAN, Some(RUSSIAN_BULGARIAN)),
    entry("bg", "Cyrl", "Bulgarian", RUSSIAN_BULGARIAN, Some(RUSSIAN_BULGARIAN)),
    entry("el", "Grek", "Greek", GREEK, Some(GREEK)),
    entry("he", "Hebr", "Hebrew", HEBREW, Some(HEBREW)),
    // NeoTeletext's own sets (DEC-602): Ukrainian, Serbian and Arabic
    // have a set of the standard too, for the receiver's region.
    entry("uk", "Cyrl", "Ukrainian", value(0b0111, 0b000), Some(CYRILLIC_UKRAINIAN)),
    entry("sr", "Cyrl", "Serbian", value(0b0111, 0b011), Some(CYRILLIC_SERBIAN_CROATIAN)),
    entry("ar", "Arab", "Arabic", value(0b1001, 0b111), Some(ARABIC_TABLE_44)),
    entry("ha", "Latn", "Hausa", value(0b0101, 0b000), None),
    entry("ff", "Latn", "Fula", value(0b0101, 0b000), None),
    entry("ak", "Latn", "Akan", value(0b0101, 0b001), None),
    entry("bm", "Latn", "Bambara", value(0b0101, 0b001), None),
    entry("dyu", "Latn", "Dyula", value(0b0101, 0b001), None),
    entry("din", "Latn", "Dinka", value(0b0101, 0b001), None),
    entry("fon", "Latn", "Fon", value(0b0101, 0b001), None),
    entry("kri", "Latn", "Krio", value(0b0101, 0b001), None),
    entry("ln", "Latn", "Lingala", value(0b0101, 0b001), None),
    entry("mnk", "Latn", "Mandinka", value(0b0101, 0b001), None),
    entry("men", "Latn", "Mende", value(0b0101, 0b001), None),
    entry("nus", "Latn", "Nuer", value(0b0101, 0b001), None),
    entry("ee", "Latn", "Ewe", value(0b0101, 0b010), None),
    entry("kab", "Latn", "Kabyle", value(0b0101, 0b011), None),
    entry("tzm", "Latn", "Central Atlas Tamazight", value(0b0101, 0b011), None),
    entry("az", "Latn", "Azerbaijani", value(0b0101, 0b100), None),
    entry("kr", "Latn", "Kanuri", value(0b0101, 0b101), None),
    entry("mos", "Latn", "Mossi", value(0b0101, 0b101), None),
    entry("yo", "Latn", "Yoruba", value(0b0101, 0b110), None),
    entry("ig", "Latn", "Igbo", value(0b0101, 0b110), None),
    entry("vi", "Latn", "Vietnamese", value(0b0101, 0b111), None),
    entry("be", "Cyrl", "Belarusian", value(0b0111, 0b001), None),
    entry("mk", "Cyrl", "Macedonian", value(0b0111, 0b010), None),
    entry("ky", "Cyrl", "Kyrgyz", value(0b0111, 0b101), None),
    entry("mn", "Cyrl", "Mongolian", value(0b0111, 0b101), None),
    entry("tt", "Cyrl", "Tatar", value(0b0111, 0b110), None),
    entry("tg", "Cyrl", "Tajik", value(0b0111, 0b111), None),
    entry("fa", "Arab", "Persian", value(0b1001, 0b000), None),
    entry("prs", "Arab", "Dari", value(0b1001, 0b000), None),
    entry("ur", "Arab", "Urdu", value(0b1001, 0b001), None),
    entry("ps", "Arab", "Pashto", value(0b1001, 0b010), None),
    entry("sd", "Arab", "Sindhi", value(0b1001, 0b011), None),
    entry("ckb", "Arab", "Central Kurdish", value(0b1001, 0b100), None),
    entry("ug", "Arab", "Uyghur", value(0b1001, 0b101), None),
    entry("bal", "Arab", "Balochi", value(0b1001, 0b110), None),
    entry("ka", "Geor", "Georgian", value(0b1011, 0b000), None),
    entry("hy", "Armn", "Armenian", value(0b1011, 0b001), None),
    entry("zgh", "Tfng", "Standard Moroccan Tamazight", value(0b1011, 0b010), None),
    other_script("tzm", "Tfng", "Central Atlas Tamazight (Tifinagh script)", value(0b1011, 0b010), None),
    entry("th", "Thai", "Thai", value(0b1100, 0b010), None),
    entry("lo", "Laoo", "Lao", value(0b1100, 0b100), None),
    // Japanese (DEC-2184, DEC-2187): kana by default; rōmaji is the
    // standard's Latin English with marks on pages for Level 3.5 and below.
    entry("ja", "Jpan", "Japanese", value(0b1101, 0b001), None),
    other_script("ja", "Hira", "Japanese (hiragana)", value(0b1101, 0b001), None),
    other_script("ja", "Hrkt", "Japanese (kana)", value(0b1101, 0b001), None),
    other_script("ja", "Kana", "Japanese (katakana)", value(0b1101, 0b010), None),
    other_script("ja", "Latn", "Japanese (rōmaji)", value(0b1101, 0b000), Some(ENGLISH)),
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
