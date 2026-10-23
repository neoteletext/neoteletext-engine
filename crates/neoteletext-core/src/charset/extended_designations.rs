//! neoteletext's own designations at values Table 32 reserves (REQ-CHR-012,
//! decision E.78), compiled in with the feature `extended-sets`.

use super::designation::{Table32Entry, latin, whole};
use super::{G0, G0Set, G2Set, NationalOption};
use crate::consts::DESIGNATION_GROUP_SHIFT as GROUP_SHIFT;

/// neoteletext's own sets at values Table 32 reserves (REQ-CHR-012,
/// decision E.78). Tables 33 and 34 read the same entries.
#[cfg(feature = "extended-sets")]
pub(super) const EXTENDED_TABLE_32: [Table32Entry; 29] = {
    use G0Set as S;
    use G2Set::{Arabic, Armenian, Cyrillic, Lao, Latin, Thai, Vietnamese};
    use NationalOption as N;
    [
        // Group 0101: African Latin and Vietnamese.
        (0b0101, 0b000, latin(N::HausaFula), Latin),
        (0b0101, 0b001, latin(N::WestAfrican), Latin),
        (0b0101, 0b010, latin(N::Ewe), Latin),
        (0b0101, 0b011, latin(N::KabyleTamazight), Latin),
        (0b0101, 0b100, latin(N::Azerbaijani), Latin),
        (0b0101, 0b101, latin(N::KanuriMossi), Latin),
        // Whole sets, so that X/26 marks find their bases (REQ-CHR-002).
        (0b0101, 0b110, whole(S::YorubaIgbo), Latin),
        (0b0101, 0b111, whole(S::Vietnamese), Vietnamese),
        // Group 0111: Cyrillic languages.
        (0b0111, 0b000, whole(S::Ukrainian), Cyrillic),
        (0b0111, 0b001, whole(S::Belarusian), Cyrillic),
        (0b0111, 0b010, whole(S::Macedonian), Cyrillic),
        (0b0111, 0b011, whole(S::Serbian), Cyrillic),
        (0b0111, 0b101, whole(S::KyrgyzMongolian), Cyrillic),
        (0b0111, 0b110, whole(S::Tatar), Cyrillic),
        (0b0111, 0b111, whole(S::Tajik), Cyrillic),
        // Group 1001: Arabic script in plain letters, joined by the
        // receiver (REQ-CHR-015).
        (0b1001, 0b000, whole(S::Persian), Arabic),
        (0b1001, 0b001, whole(S::Urdu), Arabic),
        (0b1001, 0b010, whole(S::Pashto), Arabic),
        (0b1001, 0b011, whole(S::Sindhi), Arabic),
        (0b1001, 0b100, whole(S::Sorani), Arabic),
        (0b1001, 0b101, whole(S::Uyghur), Arabic),
        (0b1001, 0b110, whole(S::Balochi), Arabic),
        (0b1001, 0b111, whole(S::ArabicPlain), Arabic),
        // Group 1011: other alphabets.
        (0b1011, 0b000, whole(S::Georgian), Latin),
        (0b1011, 0b001, whole(S::Armenian), Armenian),
        (0b1011, 0b010, whole(S::Tifinagh), Latin),
        // Group 1100: Thai and Lao, the SAA5246AP/S scheme (REQ-CHR-016).
        (0b1100, 0b000, latin(N::English), Thai),
        (0b1100, 0b010, whole(S::Thai), Thai),
        (0b1100, 0b100, whole(S::Lao), Lao),
    ]
};

/// The second G0 sets of neoteletext's own designations that are not
/// Latin English: an English page with Thai second (REQ-CHR-016).
#[cfg(feature = "extended-sets")]
// Group 1100, option 000.
pub(super) const EXTENDED_SECOND: [(u8, G0); 1] = [(0b1100 << GROUP_SHIFT, whole(G0Set::Thai))];
