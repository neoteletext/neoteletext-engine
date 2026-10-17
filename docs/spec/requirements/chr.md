# F.7 Character sets (CHR)

**REQ-CHR-001 — Internal character representation.**
Inside the decoder, each displayable character shall be represented by a character set identifier and a 7-bit code, and not by a Unicode value. The character set identifiers are:
- G0 Latin with a given national option;
- G0 Latin without national option;
- G0 Cyrillic option 1, 2 or 3;
- G0 Greek, Arabic or Hebrew;
- G1 mosaic;
- G2 Latin, Cyrillic, Greek or Arabic;
- G3.

A G0 character combined with a diacritical mark shall carry the mark's number as well. Conversion to Unicode shall happen only at the output stage.
*Amended 18 October 2026* (decision E.78): in Phase 2 the identifiers also include neoteletext's own G0 and G2 sets of REQ-CHR-012, and a Latin G0 set may carry one of the African Latin options.
*Reason:* Some teletext characters, such as mosaics and some G3 shapes, have no exact Unicode equivalent. Keeping the teletext identity lets each renderer choose its own representation.
*Verification:* Inspection.

**REQ-CHR-002 — Latin G0 with national options.**
The Latin G0 primary set (Table 35) shall be implemented with all 13 national option subsets of Table 36, for every one of the 13 national option positions:
- Czech and Slovak
- English
- Estonian
- French
- German
- Italian
- Lettish and Lithuanian
- Polish
- Portuguese and Spanish
- Rumanian
- Serbian, Croatian and Slovenian
- Swedish, Finnish and Hungarian
- Turkish

The base forms shown in Table 35 (#, ¤, @, [, \, ], ^, _, `, {, |, } and ~) shall be used whenever a Latin G0 character is referred to by packet X/26.

Four national option glyphs are drawn too small in the standard to identify with certainty. They were identified by the reference mapping in zvbi and by the Bedstead font:

| Position | Character | Unicode |
|---|---|---|
| Polish 5/B | Z with stroke | U+01B5 |
| Polish 7/D | l with stroke | U+0142 |
| English 6/0 | The SAA5050 long dash | U+2014, the em dash, which is also what Bedstead uses |
| English 7/C | Double vertical bar | U+2016 |

The Turkish currency sign at 2/3 is mapped to the Turkish lira sign, U+20BA. zvbi uses a private-use code, U+E800, from before Unicode had the sign.
*Verification:* Test against the transcribed tables and against the reference mapping (REQ-TST-011).

**REQ-CHR-003 — Selecting the character set of a page.**
The G0 and G2 sets and the national option in force on a page shall be found from Table 32 using the 7-bit value built as decision E.4 describes. The national option index shall be 4×C12 + 2×C13 + C14 (decision E.21).

If the resulting Table 32 entry is reserved:
1. the same three national option bits shall first be tried in group 0000;
2. if that entry is also reserved, English shall be used.

The fallback shall be counted as a diagnostic.
*Verification:* Test covering every defined entry of Table 32.

**REQ-CHR-004 — Other G0 sets.**
*Phase:* Phase 1 for the code tables and Unicode mappings; Phase 2 for the glyphs of these sets.
The following sets shall be implemented as code tables with their Unicode mappings (Tables 38 to 46):
- Cyrillic G0 options 1 (Serbian and Croatian), 2 (Russian and Bulgarian) and 3 (Ukrainian);
- Greek G0;
- Arabic G0;
- Hebrew G0.

Glyphs for these sets should be provided for the raster renderer; where a glyph is missing, the renderer shall draw a visible placeholder rather than a wrong character.
*Verification:* Test of the mappings; inspection of glyph coverage.

**REQ-CHR-005 — The G1 mosaic set.**
In mosaic mode, codes 2/0 to 3/F and 6/0 to 7/F shall be treated as block mosaics using the bit-to-block mapping of Part A.8, and codes 4/0 to 5/F as blast-through G0 characters with national option substitution applied (Table 47 note 2). For text output, mosaics shall map to Unicode as follows. Take n = (code AND 0x1F) OR ((code AND 0x40) shifted right by 1), the 6-bit block pattern, from 0 to 63.

**Contiguous mosaics:**

| n | Unicode |
|---|---|
| 0 | Space |
| 21 | U+258C (left half block) |
| 42 | U+2590 (right half block) |
| 63 | U+2588 (full block) |
| Any other value | The sextant characters U+1FB00 to U+1FB3B, in increasing order of n, skipping 21 and 42 |

**Separated mosaics** map to U+1CE50 + n (U+1CE51 to U+1CE8F), from the Symbols for Legacy Computing Supplement added in Unicode 16. Output that must work with older fonts may use the contiguous forms instead.

These ranges were checked against the Bedstead font's tables.
*Verification:* Test of all 64 mosaic codes.

**REQ-CHR-006 — The G2 sets.**
The following sets shall be implemented with their Unicode mappings:
- the Latin G2 supplementary set (Table 37), with all 95 characters;
- the Cyrillic G2 set (Table 41);
- the Greek G2 set (Table 43);
- the Arabic G2 set (Table 45).

Positions that the standard says are undefined or should not be transmitted (in the Latin set, 5/9, 5/A, 5/B and 6/5) shall display as a space.
*Verification:* Test.

**REQ-CHR-007 — The G3 set.**
The G3 set (Table 48) shall be implemented. Every defined position shall have a glyph, drawn as REQ-RND-037 describes, including positions 5/1 (horizontal line), 5/B (right arrow), 5/C (left arrow) and 5/D (up arrow), the ones Level 1.5 transmissions use in practice (Annex E.3). The unassigned positions 6/E, 6/F, 7/E and 7/F shall display as a space.
*Amended 10 October 2026:* glyphs for the G3 positions not used at Level 1.5 were Phase 2. They are Phase 1 now that Levels 2.5 and 3.5 are (decision E.40), and are drawn from the project's own geometry (decision E.50).
*Verification:* Test; inspection of glyph coverage.

**REQ-CHR-008 — Diacritical marks.**
The sixteen entries of column 4 of the G2 set shall be implemented as diacritical marks for the "G0 character with diacritical mark" triplet modes:

| Position | Mark |
|---|---|
| 4/0 | None |
| 4/1 | Grave |
| 4/2 | Acute |
| 4/3 | Circumflex |
| 4/4 | Tilde |
| 4/5 | Macron |
| 4/6 | Breve |
| 4/7 | Dot above |
| 4/8 | Diaeresis |
| 4/9 | Dot below (uncertain; see note) |
| 4/A | Ring above |
| 4/B | Cedilla |
| 4/C | Low line (uncertain; see note) |
| 4/D | Double acute |
| 4/E | Ogonek |
| 4/F | Caron |

*Note:* The glyphs at 4/9 and 4/C are drawn in Table 37 as a low dot and a low bar, but the standard does not name them. zvbi maps them to U+002E and U+02CD, and vhs-teletext to U+02CC and U+005F. This project treats them as the combining dot below (U+0323) and the combining low line (U+0332). They are rarely transmitted. The mapping shall be revisited if a real transmission is found that uses them.

Each shall have a Unicode combining character mapping. Composition shall follow decision E.26. Composition shall be attempted only for Latin G0 base characters (Annex E.10). For other G0 sets, the base character shall be shown without the mark, and the omission shall be counted as a diagnostic.
*Verification:* Test that every mark on every Latin letter produces the expected Unicode normalised form.

**REQ-CHR-009 — The second G0 set and the ESC attribute.**
When a second G0 set has been designated through Table 33 (decision E.4), the ESC attribute shall toggle the current row between the default and second G0 sets. When no second set has been designated, or the designation is 1111111 ("no second G0 set required"), ESC shall have no effect other than occupying its cell. The national option bits do not apply to the second set (clause 15.3).
*Verification:* Test.

**REQ-CHR-010 — Unicode conversion.**
A function shall convert any internal character to a Unicode scalar value or to a short sequence of scalar values. For characters with no Unicode equivalent, it shall return a documented private-use or fallback value.
*Verification:* Test (REQ-TST-011).

**REQ-CHR-011 — Unicode for the G3 set and for DRCS characters.**
Each defined G3 position shall convert to the Unicode character that Unicode document L2/19-025 and its mapping file `TELTXTG3.TXT` assign to it, as `research/notes-level25-drcs.md` section 9.3 lists them. Most are in the block "Symbols for Legacy Computing" from U+1FB3C to U+1FB6F, and the rest are box-drawing, geometric-shape and arrow characters. Position 5/F, which shows the background colour, shall convert to a space. The unassigned positions shall convert to a space. A DRCS character has no Unicode equivalent and shall convert to the documented fallback value of REQ-CHR-010.
*Verification:* Test of every G3 position against the mapping file.

**REQ-CHR-012 — Designations of neoteletext's own sets.**
*Phase:* Phase 2 (whole requirement).
The reserved Table 32 values of this table shall designate the sets of decision E.78, and the same values shall be accepted in Tables 33 and 34. Every one names Latin with the English option as its second G0 set, used when no Table 33 designation is received (decision E.78 item 4).

| Value | Default G0 set | G2 set |
|---|---|---|
| 0101 000 | Latin, option Hausa and Fula | Latin |
| 0101 001 | Latin, option West African (Akan, Lingala, Bambara, Krio, Mende, Dinka, Nuer, Fon) | Latin |
| 0101 010 | Latin, option Ewe | Latin |
| 0101 011 | Latin, option Kabyle and Tamazight | Latin |
| 0101 100 | Latin, option Azerbaijani | Latin |
| 0101 101 | Latin, option Kanuri and Mossi | Latin |
| 0101 110 | Latin, option Yoruba and Igbo | Latin |
| 0101 111 | Latin, option Vietnamese | Vietnamese |
| 0111 000 | Cyrillic, Ukrainian | Cyrillic |
| 0111 001 | Cyrillic, Belarusian | Cyrillic |
| 0111 010 | Cyrillic, Macedonian | Cyrillic |
| 0111 011 | Cyrillic, Serbian | Cyrillic |
| 0111 101 | Cyrillic, Kyrgyz and Mongolian | Cyrillic |
| 0111 110 | Cyrillic, Tatar | Cyrillic |
| 0111 111 | Cyrillic, Tajik | Cyrillic |
| 1001 000 | Arabic script, Persian and Dari | Arabic |
| 1001 001 | Arabic script, Urdu | Arabic |
| 1001 010 | Arabic script, Pashto | Arabic |
| 1001 011 | Arabic script, Sindhi | Arabic |
| 1001 100 | Arabic script, Sorani Kurdish | Arabic |
| 1001 101 | Arabic script, Uyghur | Arabic |
| 1001 110 | Arabic script, Balochi | Arabic |
| 1001 111 | Arabic script, Arabic in plain letters | Arabic |
| 1011 000 | Georgian | Latin |
| 1011 001 | Armenian | Armenian |
| 1011 010 | Tifinagh | Latin |
| 1011 011 | Inuktitut syllabics | Latin |
| 1100 000 | Latin, option English, with Thai as the second G0 set | Thai |
| 1100 010 | Thai | Thai |
| 1100 100 | Lao | Lao |

At Level 1 the group comes from the receiver's region setting, as for every group (REQ-CHR-003). A build without this requirement's feature treats these values as reserved, with the fallback of REQ-CHR-003.
*Verification:* Test of every value in Tables 32, 33 and 34, of the default second set, and of the fallback in a build without the feature.

**REQ-CHR-013 — The code tables of neoteletext's own sets.**
*Phase:* Phase 2 (whole requirement).
Each set of REQ-CHR-012 shall be a 96-position code table with its Unicode mapping (REQ-CHR-010), in the core with the standard's tables. Each table is written from a reviewed data file that lists every position's character and is checked against Unicode CLDR's letters for its language. The layouts are those of `docs/spec/design/languages.md` section 8:
- the African Latin and Vietnamese options replace the 13 national option positions of Table 35;
- the Serbian and Macedonian sets start from Table 38, the Ukrainian set from Table 40 and the other Cyrillic sets from Table 39, so that letters keep their codes;
- the Lao set takes the codes of the 1990 World System Teletext specification's Figure 33;
- the Arabic-script sets keep the digits at 3/0 to 3/9, drawn in each language's own shapes;
- Georgian and Tifinagh follow the layout of Table 46;
- every G0 set has the space at 2/0 and the block at 7/F (clause 15.6).

The Armenian, Vietnamese, Thai and Lao G2 sets are Latin G2 (Table 37) with the positions that decisions E.78, E.80 and E.81 name replaced.
*Verification:* Test of every position of every table against its data file.

**REQ-CHR-014 — Diacritical marks from the page's G2 set.**
*Phase:* Phase 2 (whole requirement).
A "G0 character with diacritical mark" triplet shall take its mark from column 4 of the page's current G2 set when that set's column 4 holds diacritical marks (the Latin, Armenian and Vietnamese G2 sets), and from Latin G2 otherwise (decision E.81, Table 29 modes 10000 to 11111). The Vietnamese G2 set's 4/C is the combining hook above, U+0309. Composition follows decision E.26 for the Latin bases and for the letters of the African Latin and Vietnamese national options.
*Verification:* Test that every tone on every Vietnamese vowel composes to its precomposed letter, and that pages with the standard's sets compose exactly as before.

**REQ-CHR-015 — Joining Arabic-script letters.**
*Phase:* Phase 2 (whole requirement).
After a page's grid is composed at its presentation level, each row shall be passed once, from right to left, and every cell holding a letter of a group 1001 set shall be given its form (decision E.79):
- **Its neighbours.** A letter's previous letter is the cell to its right, and its next letter the cell to its left, when each holds a letter of the same set.
- **Joining.** It joins its previous letter when that letter is dual-joining or join-causing. It joins its next letter when it is itself dual-joining and the next letter is dual-joining, right-joining or join-causing.
- **Its form.** Joining neither is isolated, joining both medial, only the previous final, and only the next initial.

The joining types are those of Unicode's `ArabicShaping.txt`; the lam-alef ligature is right-joining. The form is drawn from the font's glyph at the letter's Arabic Presentation Form, or at the code point of the core's table in Supplementary Private Use Area-A where Unicode has no presentation form. `to_unicode` shall return the plain letter. The pass shall use no allocation and run in time proportional to the row's length.
*Verification:* Test of every joining type before and after every other, of breaks at spaces, control codes, mosaics and other sets, of the lam-alef ligature, of a letter changed by X/26, and of the private-use forms.

**REQ-CHR-016 — Thai and Lao.**
*Phase:* Phase 2 (whole requirement).
- **The Thai sets** shall be those of the Philips SAA5246AP/S (decision E.80):
  - the G0 set from Table 23, columns 2 to 7;
  - the G2 set from Table 13, columns 8 and 9, at Thai G2 columns 5 and 6;

  both as `docs/spec/design/languages.md` sections 7.3 and 7.4 transcribe them.
- **Designation 1100 010** gives a Thai page with English as the second G0 set, and **1100 000** an English page with Thai second. ESC shall toggle them as REQ-CHR-009 describes, every row starting in the page's default set.
- **The Lao sets** follow the same scheme at 1100 100, with the G0 codes of the 1990 World System Teletext specification's Figure 33.
- **No receiver rule.** A mark is an ordinary character in its own cell; the receiver neither overlays nor reorders. The glyph conventions are part of the sets: upper vowel signs, tone marks and their combinations are drawn at the bottom of the cell, and lower vowel signs at the top.
- **Unicode.** `to_unicode` of a G2 combination shall return its marks as a combining sequence, vowel sign first: ◌ี่ gives U+0E35 U+0E48.
*Verification:* Test of every position against the transcription, of both designations with ESC in each direction, of G2 characters through X/26 and their stand-ins at Level 1, and a rendering beside the datasheet's glyphs.
