//! The page-level presentation data of Levels 2.5 and 3.5 (decision E.44):
//! its sources, and their resolution into the Colour Map, the default
//! colours, re-mapping and substitution.

use super::{ColourIndex, ColourMap, Remapping};
use crate::config::PresentationLevel;
use crate::consts::{
    CLUT_ENTRIES, CLUT_LEVEL_25_FIRST, CLUT_LEVEL_35_FIRST, COLOUR_BLACK, COLOUR_MAP_ENTRIES, COLOUR_WORDS,
    TRIPLET_COLOUR_INDEX_MASK,
};
use crate::types::{ColourWords, FallbackFields, PresentationFields};

/// The sources of page-level presentation data, in the order of decision
/// E.44. Each is the decoded packet's field groups, or `None` when the
/// packet is absent. The caller passes the page's X/28/0 only when it is
/// read as format 1 on a displayed page.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PresentationSources<'a> {
    /// The page's X/28/0 format 1.
    pub page: Option<&'a PresentationFields>,
    /// The page's X/28/4, used at Level 3.5 only.
    pub page_level_35: Option<&'a PresentationFields>,
    /// The magazine's M/29/0.
    pub magazine: Option<&'a PresentationFields>,
    /// The magazine's M/29/4, used at Level 3.5 only.
    pub magazine_level_35: Option<&'a PresentationFields>,
    /// The fallback bits of the page's POP link in the Magazine
    /// Organization Table (decision E.53).
    pub fallback: Option<FallbackFields>,
}

/// The page-level presentation data of a page at Level 2.5 or 3.5, as data
/// that the enhancement processor applies (REQ-L25-030, REQ-L25-031).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct PagePresentation {
    /// The page's Colour Map.
    pub colour_map: ColourMap,
    /// The default screen colour; a Full Screen Colour triplet replaces it
    /// (decision E.45).
    pub default_screen: ColourIndex,
    /// The default row colour of every row; Full Row Colour triplets
    /// replace it (decision E.45).
    pub default_row: ColourIndex,
    /// Black Background Colour Substitution.
    pub black_background_substitution: bool,
    /// Colour Table Re-mapping.
    pub remapping: Remapping,
}

impl PagePresentation {
    /// The defaults of Annex D.2.1 over a base Colour Map: black screen and
    /// rows, no re-mapping and no substitution.
    #[must_use]
    pub const fn defaults(colour_map: ColourMap) -> Self {
        Self {
            colour_map,
            default_screen: COLOUR_BLACK,
            default_row: COLOUR_BLACK,
            black_background_substitution: false,
            remapping: Remapping::NONE,
        }
    }
}

/// Builds the page-level presentation data (decisions E.43 and E.44).
/// `base` is the Colour Map before page data: Table 30, or the
/// user's replacement colours (REQ-RND-030).
///
/// - CLUTs 2 and 3 come from the first of X/28/0 and M/29/0 whose colour
///   words are valid; CLUTs 0 and 1, at Level 3.5 only, from the first of
///   X/28/4 and M/29/4. CLUT 1 entry 0 always stays transparent.
/// - The default colours, substitution and re-mapping come from the first
///   source, in the order X/28/0, X/28/4 (Level 3.5), M/29/0, M/29/4
///   (Level 3.5), whose field group is valid; substitution then falls back
///   to the table's POP link.
/// - The sources' side-panel fields have no effect (REQ-1644).
/// - At Levels 1 and 1.5 the result is the defaults over `base`.
#[must_use]
pub fn resolve(sources: &PresentationSources<'_>, level: PresentationLevel, base: &ColourMap) -> PagePresentation {
    let mut out = PagePresentation::defaults(*base);
    if !cfg!(feature = "level25-32colors") {
        // Decision E.105: without the Colour Map of Level 2.5, CLUTs 2 and
        // 3 are copies of CLUTs 0 and 1, and nothing redefines a CLUT.
        let (low, high) = out.colour_map.entries.split_at_mut(COLOUR_MAP_ENTRIES / 2);
        high.copy_from_slice(low);
    }
    if !level.is_enhanced() {
        return out;
    }
    let level_35 = level.is_level35();
    let (page_35, magazine_35) =
        if level_35 { (sources.page_level_35, sources.magazine_level_35) } else { (None, None) };
    let ordered = [sources.page, page_35, sources.magazine, magazine_35];
    let first = |group: fn(&PresentationFields) -> bool| ordered.into_iter().flatten().find(|fields| group(fields));

    // CLUTs 2 and 3 are the Level 2.5 Colour Map's, CLUTs 0 and 1 the
    // Level 3.5 one's (decision E.105).
    if cfg!(feature = "level25-32colors")
        && let Some(words) =
            [sources.page, sources.magazine].into_iter().flatten().find_map(|fields| fields.colour_words)
    {
        out.colour_map.set_cluts(CLUT_LEVEL_25_FIRST, &words);
    }
    if cfg!(feature = "level35-32colors")
        && let Some(words) = [page_35, magazine_35].into_iter().flatten().find_map(|fields| fields.colour_words)
    {
        out.colour_map.set_cluts(CLUT_LEVEL_35_FIRST, &words);
    }

    let colours = first(|fields| fields.colours.is_some()).and_then(|fields| fields.colours);
    if let Some(colours) = colours {
        out.default_screen = colours.screen & TRIPLET_COLOUR_INDEX_MASK;
        out.default_row = colours.row & TRIPLET_COLOUR_INDEX_MASK;
        out.black_background_substitution = colours.black_background_substitution;
        out.remapping = Remapping::from_value(colours.remapping);
    } else if let Some(fallback) = sources.fallback {
        out.black_background_substitution = fallback.black_background_substitution;
    }

    out
}

/// The colour words of a [`ColourMap`]'s two CLUTs from `first_clut`, in
/// transmission order: the inverse of [`ColourMap::set_cluts`], for the
/// encoder and tests.
#[must_use]
pub fn colour_words(map: &ColourMap, first_clut: u8) -> ColourWords {
    let start = usize::from(first_clut).saturating_mul(CLUT_ENTRIES);
    core::array::from_fn(|offset| map.entries.get(start.saturating_add(offset)).copied().unwrap_or_default())
}

/// The words of two CLUTs are sixteen, as the packets carry them.
const _: () = assert!(COLOUR_WORDS == 2 * CLUT_ENTRIES);
