# Character sets and fonts

How teletext chooses characters, and how neoteletext turns them into glyphs. The normative rules are in `requirements/chr.md` and `requirements/fnt.md`, and the choices in decisions E.26, E.72 and E.75 (`decisions.md`); this document explains them from the beginning.

## The history of G2, and the problem it solves

Teletext began as a British system for English, and every later character set, G2 among them, is an answer to the same question: how to show more than 96 characters while staying compatible with every receiver already sold. The specifications this section draws on are summarised, with page references, in `research/answers-historic-specifications.md` and `research/notes-level15.md`.

### One set for English (1974 to 1976)

The first specification, by the BBC, the IBA and the television manufacturers' association BREMA (October 1974), used ISO 7-bit ASCII with British "national usage" characters, such as £ at 2/3. The 1976 *Broadcast Teletext Specification*, the system Ceefax and ORACLE broadcast, fixed a single English set of 96 characters with no options at all. The header bits that would later choose a language, C12 to C14, were marked "unallocated ... for future use as display control functions".

### One language per chip: national options

When teletext spread across Europe, other languages needed letters English did not: Ä Ö Ü ß, é è à ç, Å, and so on. The answer had to fit the existing 7-bit page format, so the set was not enlarged. Instead, **13 positions** (2/3, 2/4, 4/0, 5/B to 5/F, 6/0, 7/B to 7/E: the ones holding £ $ @ arrows # ½ ¼ ‖ ¾ ÷ in English) were given over to each language's most needed letters. Each such variant is a **national option sub-set**. The Mullard SAA5050 family made one chip per option (SAA5050 English, SAA5051 German, SAA5052 Swedish and so on), and a manufacturer chose a language by fitting the chip. Later decoders held several options and used the header's C12 to C14, finally allocated for this, to pick one of 8 within a region (World System Teletext 1990, EBU SPB 492 of 1992).

Thirteen positions were not enough. A language with many accented letters, Czech, Polish, Turkish, Romanian or the Baltic languages, could not fit even its own alphabet into 13 swaps, and a page could still use only one option at a time.

### Supplementary sets and "Level 2"

The fuller answer came with the enhanced levels of the late 1980s and early 1990s, written down in the *World System Teletext* specification (CCIR Teletext System B, 1990) and the EBU's SPB 492 (1992). They kept the Level 1 page untouched and added **packets X/26**: lists of instructions, each aimed at one cell, that a newer decoder applies on top of the page. They also defined, for each script, a **primary set** (now called G0) and a **supplementary set** (now G2): Latin, Arabic, Cyrillic, Greek and Hebrew, with slots reserved for Devanagari, Thai, Korean, Japanese and others that were never filled. The Latin supplementary set took its accented capitals, symbols and the 15 non-spacing accent marks of its column 4 from ISO 6937, the international standard for Latin text communication; EN 300 706 notes that three of its symbols are additions to ISO 6937. These documents call this "Presentation Level Two".

With supplementary sets, a page can place any accented letter: either a ready-made character from G2, or a G0 letter with one of G2's accents drawn over it. Twenty-six letters and 15 accents cover far more than any 96-character table could.

### Level 1.5: what broadcasters actually did

Most of what Level 2 offered (more colours, side panels, objects, downloadable characters) is not needed to write a language correctly. Broadcasters in countries whose languages needed more than 96 characters began sending only the part that mattered: X/26 packets carrying **a few G2 characters and a few accented letters** to fix up the Level 1 page, and decoders were built that understood just that. Neither the 1990 nor the 1992 document names this practice. The first standard to describe it is ETS 300 706 (May 1997), now EN 300 706, which calls it **Level 1.5** and says it "has evolved in the field due to the language requirements in certain countries and is documented here for the first time". Its clause 15.1 states the purpose: "Where the local language requirements require more than 96 alphanumeric characters to provide a basic service, additional packets X/26 may be introduced to form Level 1.5 transmissions. Typically these incorporate a few characters from the G2 supplementary set of characters, plus a few G0 characters with diacritical marks."

EN 300 706 keeps four G2 sets: Latin, Cyrillic, Greek and Arabic. The Hebrew supplementary set of the 1992 list has no table in it, and Hebrew pages are paired with the Arabic G2 set (Table 32). The repertoire a Level 1.5 decoder offers "is not fixed and is determined by the language requirements in the country of use".

### Why it was done this way

Every step kept old receivers working. A Level 1 television ignores X/26 and shows the stand-in the broadcaster put in the row, usually the unaccented letter, so a page sent with G2 enhancements reads correctly, if plainly, on a 1980 set and exactly on a later one. G2 solved the problem of languages that need more than 96 characters without changing the 7-bit page or breaking a single receiver.

## Pairing: how a page's G0 and G2 sets are chosen together

### The pairs are fixed by the standard

A page never names G0 and G2 separately. EN 300 706 **Table 32** lists the allowed combinations, each row a **pair**: a G0 set with its national option, and the G2 set that goes with it. The table is built into every receiver (in neoteletext, the core's `charset::designation`); no broadcast can change it. A page only **chooses a row**.

| Table 32 row (group, option) | G0 set | Paired G2 set |
|---|---|---|
| 0000, 001 | Latin, German | Latin |
| 0100, 100 | Cyrillic 2 (Russian/Bulgarian) | Cyrillic |
| 0110, 111 | Greek | Greek |
| 1000, 111 | Arabic | Arabic |
| 1010, 101 | Hebrew | **Arabic** |
| 1000, 000 | Latin, English | **Arabic** |

The table has 36 rows in 8 groups; a page can say "I am row 0000 001", meaning German with G2 Latin, but cannot ask for a combination the table does not list, such as German G0 with Greek G2.

### Why these pairs

A G2 set is designed to complete a particular G0 set: it holds what that G0 set lacks.

- **G2 Latin** adds accents, accented capitals and symbols to the Latin G0 sets.
- **G2 Cyrillic** holds the Latin letters missing from Cyrillic G0 (D E F G I J K L N Q R S U V W Z), so a Russian page can still write a brand name.
- **G2 Greek** likewise holds Latin letters missing from Greek G0, plus accented Greek capitals.
- **G2 Arabic** holds Arabic letter forms and a Latin alphabet with French accented letters, which is why it is paired with Arabic G0, and also with Latin English and Latin French in the Arabic group: pages there mix Arabic and Latin.
- **Hebrew has no G2 set of its own** in EN 300 706, so Table 32 pairs Hebrew G0 with G2 Arabic.

### How a page chooses its pair

- **At Level 1**, by the three national option bits C12 to C14 in its header. The four-bit group is not sent: the receiver supplies it from its setting (a "local Code of Practice"), which the standard itself calls "an ambiguous reference to an entry in table 32".
- **At Level 1.5 and above**, by a full seven-bit value, group and option, in packet **X/28/0 Format 1** or **X/28/4** for one page, or **M/29/0** or **M/29/4** for every page of a magazine; a page's own packet takes priority over its magazine's.
- **A second G0 set** (Table 33), for pages in two languages, comes **without a G2 of its own**: when ESC switches a row to the second G0 set, G2 characters still come from the default pair's G2.
- **Within a page**, at Levels 2.5 and 3.5, an X/26 **Modified G0 and G2 Character Set Designation** triplet picks a row of **Table 34** and switches G0 and G2 together for the rest of the page; its G0 set has no national option. Level 2.5 allows one such extra pair, Level 3.5 any number.

A page's G2 characters, placed cell by cell through X/26, always come from the G2 set paired with its current G0 set. That is why the 96-position grid shows a set beside its partner: the left grid is what the page's text can be, the right grid the extra characters it can call on.

### Where it is specified

| What | EN 300 706 V1.2.1 |
|---|---|
| The pairs | Table 32, pp. 101 to 102 |
| One value designates both sets, and its priority order | Clause 15.2, pp. 100 to 101 |
| The ambiguity of Level 1 | Clause 15.2, p. 101 |
| A second G0 set has no G2 | Clause 15.3 and Table 33, pp. 102 to 104 |
| Switching the pair within a page | Clause 15.4 and Table 34, pp. 104 to 105; Table 29 mode 01000 |
| The packets carrying the values | Clause 9.4.2.2 (X/28/0 Format 1) and the M/29 clauses |

In neoteletext, Table 32 is `charset::designation`, Table 33 `second_g0`, Table 34 `modified_designation`, and the priority order of clause 15.2 is `level1::designation_report`. The developer tools panel (F12) shows a page's choice step by step.

## How G2 works

### 1. Why G2 exists

Teletext sends text as 7-bit codes, so a page row can only refer to 96 printable characters: that table is the page's **G0 set**. It is enough for basic English but not for French, German, Polish and so on, which need many accented letters and symbols. Level 1's workaround was **national options**: swap 13 of the 96 positions for a language's most needed letters, so German puts `Ä Ö Ü ä ö ü ß` where English has `← ↑ # ½`. But one page gets one option, and most accented letters still do not fit.

### 2. What G2 is

The **G2 supplementary set** is a second table of 96 characters holding what G0 could not. In G2 Latin:

- accented and special capitals and letters: `Æ Ð Ħ Ł Ø Œ Þ æ đ ħ ł ø œ ß þ`;
- symbols: `¡ ¿ ¢ £ $ ¥ § ¤ ° ± × ÷ µ ¶`, quotation marks, `™ ® © ♪ €`, fractions `¼ ½ ¾ ⅛ ⅜`;
- **column 4: 15 accent marks on their own**, `` ` ´ ˆ ˜ ¯ ˘ ˙ ¨ ˚ ¸ ˝ ˛ ˇ``, which are combined with letters.

There are four G2 sets: Latin, Cyrillic, Greek and Arabic. The page's designation picks which one it pairs with its G0 set (EN 300 706 Table 32).

### 3. A page's rows never contain G2 codes

Every byte in a row is a G0 character or a control code, so the rows alone always form a complete Level 1 page. G2 characters arrive separately, in **packet X/26**, the page's enhancement data: a list of instructions, each aimed at one cell.

```
row 3, column 7:  put G2 character 3/D here           → ½ replaces what was there
row 5, column 2:  add accent ¨ (G2 4/8) to this cell   → the G0 "u" becomes "ü"
```

The receiver first builds the page from its rows, then applies X/26 on top, cell by cell. A cell keeps its colours and other attributes; only its character changes.

### 4. The two ways a cell gets G2

| X/26 instruction | Effect on the cell |
|---|---|
| Character from the G2 set | **Replaces** the cell's G0 character with a G2 character |
| G0 character with diacritical mark | **Overlays**: keeps the cell's G0 letter and draws a G2 accent on it |

The second is how pages reach any accented letter: 26 letters times 15 accents, far more than any 96-character table holds. G2 therefore overrides individual cells; the G0 set remains the page's set everywhere else.

### 5. Every television can still show the page

The broadcaster puts a stand-in in the row itself, usually the plain letter or a close symbol: `Muller` in the row, plus an X/26 accent instruction for the `u`.

- A **Level 1 television** (an SAA5050) ignores X/26 and shows `Muller`.
- A **Level 1.5 television** applies it and shows `Müller`.

Nothing breaks; older sets show a simpler version.

### 6. In neoteletext

- G2 is a character set like any other, so the user chooses a font for it in its row of the fonts panel (REQ-FNT-005). Cells that X/26 replaces with a G2 character are drawn in that font.
- An accent overlay uses the precomposed letter (`ü`) when the G0 font has it; otherwise neoteletext draws the letter with the accent's own glyph on top (decision E.26).
- G2 only appears through X/26, so a page with no enhancement data never uses its G2 font. That is why the 96-position grid shows the G2 set beside its G0 set: those are the extra characters the page can call on.

## How many glyphs a set needs

Every set has the same **96 positions**, 2/0 to 7/F, but a G2 set leaves a few empty:

| Set | Positions with a character | Glyphs a font needs |
|---|---|---|
| Any G0 set | all 96 (2/0 is the space, 7/F the block ■) | 96 |
| G2 Latin | 90, plus the space; 2/0, 4/0, 5/9, 5/A, 5/B and 6/5 are empty | 91 |
| G2 Cyrillic | | 93 |
| G2 Greek, G2 Arabic | | 95 |

Two more differences:

- **15 of G2 Latin's glyphs are accent marks** (column 4), not standalone characters: they are mostly used as overlays on a G0 letter, though they can also be drawn alone.
- **A few G2 characters repeat G0 ones**, such as `$`, `#` and the block ■ at 7/F; one glyph in a font serves both sets.

In the 96-position grid, the empty positions are drawn as a faint dot on dark paper, so that the standard leaving them blank is not mistaken for a hole in the font.

## How neoteletext knows the position of each glyph

Through Unicode. A font says nothing about teletext positions: each glyph in a BDF file is labelled with a **Unicode code point** (its `ENCODING` line), for example `U+00C4` for Ä. neoteletext connects positions to those code points with the standard's own tables.

1. **The character set tables, in the core.** The core holds EN 300 706 Tables 35 to 46 as data: for every character set and every position 2/0 to 7/F, the Unicode character (REQ-CHR-010). G0 Latin German 5/B is `Ä` (U+00C4), G0 Latin English 5/B is `←` (U+2190), G2 Latin 3/D is `½` (U+00BD).
2. **Drawing a cell.** Each cell of a decoded page records which set and which position it holds, plus an accent if X/26 added one. The renderer looks the set and position up in the tables to get the Unicode character; with an accent it asks for the precomposed character first (`u` plus `¨` gives `ü`); then it finds that code point in the font chosen for the cell's set, falling back to Bedstead and then a hollow box (REQ-FNT-005, `GlyphSet::loaded`).
3. **The grid and the conformance check** (REQ-FNT-011) walk the same tables: for each of a set's 96 positions, what character belongs there, and does the font have a glyph for it.
4. **Chip dumps** are stored by position, not by Unicode. When the font collection converts them, it uses the same tables to give each position its code point.

Two consequences of going through Unicode:

- **One glyph serves every set with that character.** `A` is the same code point in English, German and Cyrillic 1's Latin positions, so one glyph covers them all; that is how Bedstead conforms for 16 sets.
- **A single font cannot draw the same character two ways in two sets.** If the German chip's `#` looked different from the English chip's, one font could not hold both. Choosing a font per character set solves this: the German set uses the German chip's font, the English set the English one.

## Beyond the standard: neoteletext's own character sets

EN 300 706 has sets for the Latin, Cyrillic, Greek, Arabic and Hebrew scripts. neoteletext adds sets of its own for about 55 more languages, without changing how a page is sent: one byte for each cell, as always. In outline:
- **New designation codes.** The new sets are selected by designation values Table 32 leaves reserved, exactly as the standard's own sets are selected.
- **Pairing with English.** Each new set has Latin English as its second set, switched by ESC.
- **Characters beyond 96** come from a G2 set through X/26.
- **Accents** come from the page's own G2 set, as Table 29 says.
- **Arabic-script letters** are joined by the receiver.
- **Thai** follows Philips' Thai decoder chip, the SAA5246AP/S, with marks on rows of their own.
- **Fonts** are 12 × 20, converted from Noto.

`design/languages.md` is the design, with the reasons and the sources. Decisions E.77 to E.82 record the choices, and REQ-CHR-012 to REQ-CHR-016 the behaviour.
