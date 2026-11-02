//! The code tables of NeoTeletext's own character sets, as examples of the
//! generated protocol document (neoteletext-common's REQ-1732 item 4;
//! REQ-CHR-012, REQ-CHR-013): a developer outside the project needs them to
//! draw a page in one of these sets, and no standard holds them. Each table
//! is read through `to_unicode`, the conversion every receiver on the engine
//! uses, so that a table in the document cannot drift from the engine's: a
//! changed cell changes the example's digest, which `rdm spec -check` then
//! reports.

use neoteletext_core::charset::{
    CharRef, CharSet, G0, G0Set, G2Set, NationalOption, Unicode, is_national_position, to_unicode,
};

use super::{Example, Field};

/// The columns of a set's table, as EN 300 706 Tables 35 to 48 draw them:
/// the first code of each, and the cells it names.
const COLUMNS: [(u8, &str); 6] = [
    (0x20, "2/0 to 2/F"),
    (0x30, "3/0 to 3/F"),
    (0x40, "4/0 to 4/F"),
    (0x50, "5/0 to 5/F"),
    (0x60, "6/0 to 6/F"),
    (0x70, "7/0 to 7/F"),
];
/// The cells of a column.
const CELLS_IN_COLUMN: u8 = 16;
/// The field that holds a national option's positions.
const NATIONAL_FIELD: &str = "the 13 national option positions";
/// The Unicode blocks of right-to-left letters the tables hold: Hebrew to
/// Arabic Supplement, and the Arabic presentation forms a lam-alef is. A
/// mark after each keeps a row in code order, since a browser would
/// otherwise reverse the run of letters and numbers.
const RIGHT_TO_LEFT: [core::ops::RangeInclusive<char>; 2] = ['\u{0590}'..='\u{077F}', '\u{FB50}'..='\u{FEFF}'];
/// U+200E LEFT-TO-RIGHT MARK.
const LEFT_TO_RIGHT_MARK: char = '\u{200E}';

/// NeoTeletext's whole G0 sets, each with its example's name and caption.
const G0_SETS: [(G0Set, &str, &str); 22] = [
    (G0Set::YorubaIgbo, "charset-g0-yoruba-igbo", "Yoruba and Igbo G0, designation 0101 110"),
    (G0Set::Vietnamese, "charset-g0-vietnamese", "Vietnamese G0, designation 0101 111"),
    (G0Set::Ukrainian, "charset-g0-ukrainian", "Ukrainian G0, designation 0111 000"),
    (G0Set::Belarusian, "charset-g0-belarusian", "Belarusian G0, designation 0111 001"),
    (G0Set::Macedonian, "charset-g0-macedonian", "Macedonian G0, designation 0111 010"),
    (G0Set::Serbian, "charset-g0-serbian", "Serbian G0, designation 0111 011"),
    (G0Set::KyrgyzMongolian, "charset-g0-kyrgyz-mongolian", "Kyrgyz and Mongolian G0, designation 0111 101"),
    (G0Set::Tatar, "charset-g0-tatar", "Tatar G0, designation 0111 110"),
    (G0Set::Tajik, "charset-g0-tajik", "Tajik G0, designation 0111 111"),
    (G0Set::Persian, "charset-g0-persian", "Persian and Dari G0, designation 1001 000"),
    (G0Set::Urdu, "charset-g0-urdu", "Urdu G0, designation 1001 001"),
    (G0Set::Pashto, "charset-g0-pashto", "Pashto G0, designation 1001 010"),
    (G0Set::Sindhi, "charset-g0-sindhi", "Sindhi G0, designation 1001 011"),
    (G0Set::Sorani, "charset-g0-sorani", "Sorani Kurdish G0, designation 1001 100"),
    (G0Set::Uyghur, "charset-g0-uyghur", "Uyghur G0, designation 1001 101"),
    (G0Set::Balochi, "charset-g0-balochi", "Balochi G0, designation 1001 110"),
    (G0Set::ArabicPlain, "charset-g0-arabic-plain", "Arabic in plain letters G0, designation 1001 111"),
    (G0Set::Georgian, "charset-g0-georgian", "Georgian G0, designation 1011 000"),
    (G0Set::Armenian, "charset-g0-armenian", "Armenian G0, designation 1011 001"),
    (G0Set::Tifinagh, "charset-g0-tifinagh", "Tifinagh G0, designation 1011 010"),
    (G0Set::Thai, "charset-g0-thai", "Thai G0, designation 1100 010"),
    (G0Set::Lao, "charset-g0-lao", "Lao G0, designation 1100 100"),
];
/// NeoTeletext's own G2 sets.
const G2_SETS: [(G2Set, &str, &str); 4] = [
    (G2Set::Vietnamese, "charset-g2-vietnamese", "Vietnamese G2"),
    (G2Set::Armenian, "charset-g2-armenian", "Armenian G2"),
    (G2Set::Thai, "charset-g2-thai", "Thai G2"),
    (G2Set::Lao, "charset-g2-lao", "Lao G2"),
];
/// NeoTeletext's own Latin national options, in `NationalOption::EXTENDED`'s
/// order.
const OPTIONS: [(&str, &str); NationalOption::EXTENDED.len()] = [
    ("charset-latin-hausa-fula", "Latin, the Hausa and Fula option, designation 0101 000"),
    ("charset-latin-west-african", "Latin, the West African option, designation 0101 001"),
    ("charset-latin-ewe", "Latin, the Ewe option, designation 0101 010"),
    ("charset-latin-kabyle-tamazight", "Latin, the Kabyle and Tamazight option, designation 0101 011"),
    ("charset-latin-azerbaijani", "Latin, the Azerbaijani option, designation 0101 100"),
    ("charset-latin-kanuri-mossi", "Latin, the Kanuri and Mossi option, designation 0101 101"),
];

/// One cell: its code points, then the character itself.
fn cell(unicode: Unicode) -> String {
    let Unicode { base, combining } = unicode;
    if base == ' ' {
        return "0020 space".to_owned();
    }
    let right_to_left = RIGHT_TO_LEFT.iter().any(|block| block.contains(&base));
    let mark = if right_to_left { String::from(LEFT_TO_RIGHT_MARK) } else { String::new() };
    match combining {
        Some(second) => format!("{:04X} {:04X} {base}{second}{mark}", u32::from(base), u32::from(second)),
        None => format!("{:04X} {base}{mark}", u32::from(base)),
    }
}

/// `unicode`'s UTF-8 added to `bytes`.
fn push_utf8(bytes: &mut Vec<u8>, unicode: Unicode) {
    let mut buffer = [0_u8; 4];
    bytes.extend_from_slice(unicode.base.encode_utf8(&mut buffer).as_bytes());
    if let Some(second) = unicode.combining {
        bytes.extend_from_slice(second.encode_utf8(&mut buffer).as_bytes());
    }
}

/// The whole of `set`: its characters in UTF-8, code 2/0 first, and a field
/// for each column of its table.
fn whole_table(set: CharSet, name: &'static str, caption: &'static str) -> Example {
    let mut bytes = Vec::new();
    let mut fields = Vec::new();
    for (first, column) in COLUMNS {
        let offset = bytes.len();
        let mut cells = Vec::new();
        for code in first..first + CELLS_IN_COLUMN {
            let unicode = to_unicode(CharRef::new(set, code));
            push_utf8(&mut bytes, unicode);
            cells.push(cell(unicode));
        }
        fields.push(Field { offset, length: bytes.len() - offset, bits: None, name: column, value: cells.join(", ") });
    }
    Example { name, caption, bytes, fields }
}

/// A Latin option: only its national positions differ from Table 35.
fn option_table(option: NationalOption, name: &'static str, caption: &'static str) -> Example {
    let set = CharSet::G0(G0 { set: G0Set::Latin, national: Some(option) });
    let mut bytes = Vec::new();
    let mut cells = Vec::new();
    for code in COLUMNS.iter().flat_map(|&(first, _)| first..first + CELLS_IN_COLUMN) {
        if is_national_position(code) {
            let unicode = to_unicode(CharRef::new(set, code));
            push_utf8(&mut bytes, unicode);
            cells.push(format!("{}/{:X}: {}", code >> 4, code & 0x0F, cell(unicode)));
        }
    }
    let fields =
        vec![Field { offset: 0, length: bytes.len(), bits: None, name: NATIONAL_FIELD, value: cells.join(", ") }];
    Example { name, caption, bytes, fields }
}

/// Every table the document embeds.
pub fn examples() -> Vec<Example> {
    let options = NationalOption::EXTENDED.into_iter().zip(OPTIONS);
    let mut tables: Vec<Example> =
        options.map(|(option, (name, caption))| option_table(option, name, caption)).collect();
    for (set, name, caption) in G0_SETS {
        tables.push(whole_table(CharSet::G0(G0 { set, national: None }), name, caption));
    }
    for (set, name, caption) in G2_SETS {
        tables.push(whole_table(CharSet::G2(set), name, caption));
    }
    tables
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoteletext_core::charset::designation;

    /// The groups Table 32 reserves and REQ-CHR-012 fills.
    const OWN_GROUPS: [u8; 5] = [0b0101, 0b0111, 0b1001, 0b1011, 0b1100];
    /// Bits of a designation's national option.
    const OPTION_BITS: u32 = 3;

    /// A set added to the designations without a table here fails, so the
    /// document cannot fall behind the engine.
    #[test]
    fn every_set_a_designation_names_has_a_table() {
        for group in OWN_GROUPS {
            for option in 0..1_u8 << OPTION_BITS {
                let Some(found) = designation((group << OPTION_BITS) | option) else { continue };
                let own_option = found.g0.national.is_some_and(|national| NationalOption::EXTENDED.contains(&national));
                let own_set = G0_SETS.iter().any(|&(set, _, _)| set == found.g0.set);
                assert!(
                    own_option || own_set || found.g0.national == Some(NationalOption::English),
                    "no table of {found:?}"
                );
                let standard_g2 = matches!(found.g2, G2Set::Latin | G2Set::Cyrillic | G2Set::Arabic | G2Set::Greek);
                assert!(standard_g2 || G2_SETS.iter().any(|&(set, _, _)| set == found.g2), "no table of {found:?}");
            }
        }
    }

    #[test]
    fn a_whole_table_holds_every_code_of_its_set() {
        let table = whole_table(CharSet::G2(G2Set::Vietnamese), "t", "t");
        assert_eq!(table.fields.len(), COLUMNS.len());
        let cells: usize = table.fields.iter().map(|field| field.value.split(", ").count()).sum();
        assert_eq!(cells, COLUMNS.len() * usize::from(CELLS_IN_COLUMN));
    }

    #[test]
    fn a_table_says_what_the_engines_table_holds() {
        let set = CharSet::G0(G0 { set: G0Set::Vietnamese, national: None });
        let table = whole_table(set, "t", "t");
        assert!(table.fields.first().expect("a column").value.contains("01B0 ư"), "2/3 is ư");
    }

    #[test]
    fn a_cell_of_two_marks_gives_both_code_points() {
        let cell = cell(Unicode { base: '\u{0E34}', combining: Some('\u{0E48}') });
        assert!(cell.starts_with("0E34 0E48 "), "{cell}");
    }

    #[test]
    fn right_to_left_letters_keep_their_rows_in_code_order() {
        assert!(cell(Unicode { base: 'ب', combining: None }).ends_with(LEFT_TO_RIGHT_MARK));
        assert!(cell(Unicode { base: '\u{FEFB}', combining: None }).ends_with(LEFT_TO_RIGHT_MARK));
        assert_eq!(cell(Unicode { base: 'ư', combining: None }), "01B0 ư");
    }

    #[test]
    fn a_latin_option_lists_its_thirteen_positions() {
        let table = option_table(NationalOption::Ewe, "t", "t");
        assert_eq!(table.fields.first().expect("a field").value.split(", ").count(), 13);
    }
}
