# Single-byte languages: design

How neoteletext can support about 55 more languages, with roughly 900 million speakers, without changing how a teletext page is sent: one byte per character, one character per cell, 96 characters per set. Nothing here is implemented yet. Its questions were all decided on 18 October 2026, by the owner or on the owner's instruction (section 12). This document is the one home of the design's detail. The others hold only their own part and link here:
- the choices and their reasons: decisions E.77 to E.82 in `decisions.md`;
- the behaviour: REQ-CHR-012 to REQ-CHR-016 in `requirements/chr.md`, REQ-FNT-012 and the font amendments in `requirements/fnt.md`, and REQ-CLI-010 in `requirements/cli.md`;
- where the code lives: `ARCHITECTURE.md` section 4.6;
- the evidence: `research/notes-languages.md`.

The design builds on `character-sets-and-fonts.md`, which explains character sets, designation and fonts as they work today. The prototype renderings behind it were made in October 2026 and are not in the repository.

## 1. Scope

### What stays as it is

- A row is 40 bytes, each a 7-bit code with odd parity. A code is either a control code or one character of the current G0 set.
- A set holds 96 characters, at codes 2/0 to 7/F (clause 15.6).
- A page chooses its sets by designation (clause 15.2, Tables 32 to 34) and may switch to a second G0 set with ESC (clause 15.3).
- X/26 enhancement data and objects work as today.

### What it adds

1. New character sets, designated with values the standard reserves (section 3).
2. Loaded glyphs of 12 × 20 pixels as well as 12 × 10 (section 4).
3. Fonts converted from Noto, kept in this repository with their converter (section 5).
4. One receiver rule, used only by the sets that need it: joining Arabic-script letters (section 6).
5. Writing pages in the new sets from Unicode text (section 9).

### Conformance with EN 300 706

Conformance comes first, as far as it can go. Every language uses the standard's own mechanisms. A departure is allowed only where no standard mechanism can show the language legibly, and then only in that language's sets. After the Thai sources were read (section 7), no structural departure remains: every row of every set is one byte per cell, read as the standard defines.

| Part | How it stands against the standard |
|---|---|
| New G0 and G2 sets, and their designation codes | Unavoidable: a language the standard has no set for needs new codes. They use only values the standard reserves (Tables 32 to 34), in the same structure as every existing set: a group, an option, a G0 set and its paired G2 set. Arabic and Hebrew entered Table 32 the same way, from the earlier register of the 1990 and 1992 specifications. The allocation is neoteletext's own, so other decoders show these pages wrongly. |
| National option sub-sets for African Latin | The standard's own model (Table 36). Vietnamese, Yoruba and Igbo use the same layout as whole sets (section 8.1). |
| Whole G0 sets for Cyrillic, Georgian, Armenian and Tifinagh | The standard's own model, as Cyrillic, Greek and Hebrew are. |
| Vietnamese tone marks, and Yoruba and Igbo tones | The standard's Level 1.5 method: a G0 letter with a diacritical mark placed by X/26 (clause 15.1). A Level 1 decoder shows the letters without their tones, as it shows Level 1.5 Czech without its accents. |
| 12 × 20 glyphs and Noto fonts | Within the standard: "the precise appearance of the characters ... is a decoder option" (clause 15.6). |
| Arabic script joined by the receiver | One code is one letter in one cell, and rows are in visual order, as in Table 44. Drawing a letter in its joined form is its appearance, which clause 15.6 leaves to the decoder. It differs from Table 44, where each form has its own code. |
| Thai and Lao | The scheme of a real decoder chip, the Philips SAA5246AP/S: a Thai G0 set selected by the national option bits, English as the second G0 set switched by ESC, rarer marks from a G2 set through X/26, and marks on rows of their own (section 7). Every row keeps one byte per cell. |

### What the Thai hardware taught, applied to every set

The Philips SAA5246AP/S (section 7) is the only decoder known to have carried, in hardware, a script beyond EN 300 706's sets; earlier standards defined more (next section). Its design shows how a manufacturer stayed within the standard, and four of its choices are adopted for every new set:

1. **Where hardware chose the codes, keep them.** Thai keeps the chip's table, order and option bits (010). No other in-scope language has a known teletext chip, so the other tables are designed here.
2. **Every new set has Latin English as its second G0 set.** The chip paired Thai with English through ESC, as Philips' Arabic and Hebrew variants (SAA5246AP/K and AP/L) paired those scripts with Latin. Each designation in groups 0101 to 1100 therefore names English as its second G0 set (section 3). Latin words, brand names and English lines can then appear on any page, switched by ESC, without a designation packet.
3. **Characters beyond 96 come from a G2 set through X/26,** never from a wider code. The chip reached its 32 rarer Thai glyphs through an 8-bit mode that teletext's 7 bits and parity do not allow; the standard's route is the G2 set (clause 15.1). The same route serves any set that is tight:
   - Armenian keeps its letters, digits and common punctuation in G0, and puts the ligature և and the rarer Armenian punctuation in an Armenian G2 set;
   - Sindhi, if its 52 letters and digits leave too little punctuation, moves the rarest to an Arabic-script G2 set.
4. **Marks that need more height go on rows of their own,** in the column of their letter, rather than changing how bytes map to cells. Thai and Lao do this. Vietnamese and Yoruba keep the X/26 diacritical marks (section 8.1), which also suit Level 1 receivers. Spain met the needs of Catalan, Galician, Basque and Portuguese the same way, with 128 characters through X/26 (Orden of 24 November 1986, p. 9). If a dense Yoruba page exceeds the X/26 budget, Yoruba's tone marks can move to a mark row as Thai's do.

One choice is kept despite the hardware. Philips' Arabic chips sent each letter's joined form, as Table 44 does, and Arabic keeps that. For Urdu, Pashto, Sindhi and the other Arabic-script languages no chip exists, and their forms do not fit 96 positions, so they keep receiver joining (section 6), which is what EBU Tech 3232 proposed in 1982.

### What the earlier standards defined

The research of 18 October 2026 (`research/notes-languages.md`) found that teletext's own standards went further than EN 300 706 before it narrowed them.

- **The 1990 World System Teletext specification** (WST90 below; `~/devel/teletext-reference/teletext-standards/Teletext1990-blunham.pdf`):
  - **A register of character-set codes, separate from today's Table 32** (p. 144, Figure 14). It had entries for Arabic, Devanagari, Hangul, Hebrew, Burmese, Lao, Malayalam, Sinhalese, Thai, Katakana, Hiragana, Chinese, Kanji, Tamil, Assamese and Bengali, Gujarati, Punjabi, Telugu and Oriya.
  - **Code tables for many of them:**
    - Thai, Burmese and Lao G0 sets (Figures 31 to 33, pp. 171–173), with their vowel and tone marks in column 5, "non-spacing when transmitted in a packet with Y=26";
    - Devanagari G0 and G2 sets (Figures 34 and 35, pp. 174–175), with half-width vowel signs and the left halves of conjuncts;
    - Tamil, Bengali, Gujarati, Punjabi, Telugu, Malayalam, Sinhalese and Oriya on the pages that follow.
  - **Two extra X/26 formats for these scripts** ("Character Set Extension – Sanscrit Derived Languages", §14.7, pp. 39–41):
    - each 18-bit group carries two pairs of a 5-bit address and a 4-bit mark, the mark taken "from the appropriate column of the in use G2 code table";
    - Format B puts marks above or below a character;
    - Format C also puts them left and right, and allows half-width characters.
  - **Two-byte ideographs downloaded through pseudo-pages** (§14.12, pp. 48–50).
- **EBU SPB 492 (1992)** kept the register but marked the Asian entries "Reserved", moving their coding to EBU supplements (`Teletext1992-blunham.pdf` p. 116).
- **ETS 300 706 (1997)** replaced the register with today's Tables 32 to 34 and dropped Formats B and C. Arabic and Hebrew, already in the 1990 and 1992 registers, were carried into Table 32. EN 300 706 (2003) changed no allocation.
- **EBU Tech 3232 (1982)** asks that new repertoires change as few codes as possible, so that foreign decoders still show readable text (p. 8, p. 11). It proposes sending one form of each Arabic letter and letting the decoder choose its joined form from its neighbours, in rows sent in visual order (p. 12). It treats Arabic-Indic digits as a way of drawing the codes 0 to 9 (p. 12).
- **EN 300 706 Annex E.10** (informative, p. 136) says the diacritical-mark technique "should be restricted to the Latin character sets".

What this design takes from them:
- **Arabic-script joining by the receiver** (section 6) is EBU Tech 3232's own proposal of 1982, and T.101 Data Syntax II's "contextual shape determination".
- **Arabic-script digits** are the codes 3/0 to 3/9, drawn in the language's own shapes (section 8.3), as EBU Tech 3232 describes.
- **Vietnamese** is a Latin-script set whose tones are X/26 marks on Latin letters, as Annex E.10 gives them (section 8.1).
- **Lao** keeps the code table of WST90 Figure 33 (section 7.7).
- **Thai** keeps the SAA5246AP/S scheme. WST90's Thai set and Format B are recorded as the standard-era alternative, and why the chip's scheme is preferred (section 7.2).
- **Indic and Burmese** stay out of scope by choice, not for want of a coding (section 1, "What it leaves out").
- **The Philips SAA5246AP/S** is the only decoder known to have carried, in hardware, a script outside EN 300 706's sets.


### What it leaves out

| Languages | Why |
|---|---|
| Chinese | Thousands of characters. China's CCST (GB/T 14219-1993) is a complete teletext standard on World System Teletext's transport: GB 2312 characters in byte pairs, each drawn 24 × 24 across 2 cells and 3 rows, switched with 0/E and 0/F. It is the way to add Chinese later, as a regional profile (section 3, "Codes kept for CCST"). |
| Japanese | Japan's teletext was a different system (CCIR System D), not World System Teletext. |
| Korean | Korea used the North American system (NABTS with NAPLPS), sending Hangul as letters that the receiver assembled into 16 × 20 syllables. It could be added later as two-byte KS X 1001 syllables drawn across two cells and built from a few hundred letter shapes, a departure from one byte per cell. |
| Amharic and Tigrinya (Ge'ez script) | About 350 syllables: more than two sets of 96. |
| Hindi, Bengali, Marathi, Telugu, Tamil, Gujarati, Kannada, Malayalam, Odia, Punjabi, Burmese, Khmer, Sinhala | Out of scope by choice. WST90 coded twelve Indic scripts and Burmese in single-byte cells, with half-width glyphs and marks overlaid by X/26 Formats B and C, but EN 300 706 dropped those formats. No broadcast used them: India's only teletext, INTEXT (Delhi, from 1985), was Antiope in English. Showing these scripts well needs either those withdrawn formats or a text shaper in the receiver. |

## 2. Languages in scope

| Group | Languages | New sets | Receiver rule |
|---|---|---|---|
| Already supported | About 430, including English, Spanish, French, Portuguese, German, Russian, Arabic, Indonesian, Swahili, Turkish, and Kazakh in its Latin alphabet | None | None |
| Latin extensions | Hausa, Fula, Akan (Twi, Fanti), Ewe, Fon, Bambara, Dyula, Mandinka, Lingala, Krio, Mende, Dinka, Nuer, Kanuri, Mossi, Kabyle, Tamazight (Latin), Azerbaijani, Yoruba and Igbo with tones, Vietnamese | 8 national options, 1 G2 set | None; tones by X/26 |
| Cyrillic extensions | Ukrainian, Belarusian, Macedonian, Serbian, Kyrgyz, Mongolian, Tatar, Tajik | 7 | None |
| Arabic script | Persian, Dari, Urdu, Pashto, Sindhi, Sorani Kurdish, Uyghur, Balochi | 7 | Joining |
| Other alphabets | Georgian, Armenian, Tifinagh (Berber) | 3 G0 sets, 1 G2 set | None |
| Thai and Lao | Thai, Lao | 2 G0 sets, 2 G2 sets | None; marks on rows of their own, rarer marks by X/26 |

The languages already supported need only better glyphs, which section 4 gives them.

The full list of languages already supported, measured against the engine's tables, can be regenerated from Google Fonts' language data and `crates/neoteletext-core/src/glyphs/required.txt`. That method was used for this design.

## 3. Designating the new sets

### Where they go

A designation is seven bits (clause 15.2): a four-bit group and a three-bit option. Table 32 assigns groups 0000, 0001, 0010, 0011, 0100, 0110, 1000 and 1010. Groups 0101, 0111, 1001 and 1011 to 1110 are reserved, as is every unlisted option inside an assigned group (Table 32 note 2). Tables 33 and 34 reserve the same values.

Table 32 itself dates from ETS 300 706 (1997), which replaced the earlier register of character-set codes of the 1990 and 1992 specifications (section 1, "What the earlier standards defined"). Arabic and Hebrew came into Table 32 from that register as G0 sets on their own designation values, the structure the new sets follow.

| Group | Name | Kind of set | Paired G2 set |
|---|---|---|---|
| 0101 | Latin extensions | National option sub-sets of Latin G0 (13 positions replaced); Yoruba and Igbo, and Vietnamese, as whole sets of the same layout | Latin; for Vietnamese, a new Vietnamese G2 set |
| 0111 | Cyrillic extensions | Whole G0 sets | Cyrillic |
| 1001 | Arabic script | Whole G0 sets of plain letters, joined by the receiver | Arabic |
| 1011 | Other alphabets | Whole G0 sets | Latin, or an Armenian G2 set |
| 1100 | Thai and Lao | Whole G0 sets, with Latin English as the second G0 set (section 7.2) | Thai or Lao |

Each group has eight options, so a group can hold eight sets.

### How a page chooses one

The new groups work exactly as the existing ones (`docs/spec/design/character-sets-and-fonts.md`, "How a page chooses its pair"):
- **At Level 1:** a page sends only the option bits C12 to C14. The group comes from the receiver's region setting (`Config::default_region`) or from the channel.
- **At Level 1.5 and above:** X/28/0 Format 1, X/28/4, M/29/0 or M/29/4 sends the whole value.
- **The second G0 set:** every new designation names Latin English as its second G0 set, switched by ESC, as the Thai chip did. At Level 1 this pairing is the region's "local Code of Practice" (clause 15.1); a page may still name another second set with Table 33. A new set can also be another page's second G0 set.
- **Within a page:** the X/26 Modified Designation triplet can switch to one (Table 34).
- **Send the whole designation.** ETSI's code of practice (TR 100 287, clause 10.3) notes that C12 to C14 alone cannot identify a language, and asks services to send X/28/0 or M/29/0; on services that mix alphabets, it suggests a magazine for each. Pages in these sets should always carry X/28/0 Format 1 or their magazine's M/29/0.

### On other receivers

A decoder that doesn't know these values falls back as REQ-CHR-003 describes: the same option bits in group 0000, which is a Western European Latin set. The page's bytes are then shown as Latin letters and punctuation, and are unreadable. This is the cost of a private allocation.

ETSI has not revised EN 300 706 since 2003, so a clash with a later official allocation is unlikely. The allocation is still neoteletext's own, and must be documented as such.

### Codes kept for CCST

China's CCST (GB/T 14219-1993) uses some codes and packets differently from EN 300 706. None of neoteletext's new sets may give these a meaning of its own, so that a CCST profile can be added later without conflict:
- **Header bits.** C12 to C14 = 111 means a Chinese page in a CCST region.
- **Shift codes.** 0/E and 0/F switch between Western and Chinese text, in place of double width and double size.
- **Packet X/25** carries vertical parity for error correction, not display data.
- **Packets X/27/4 to X/27/7** link DRCS pseudo-pages.
- **X/26:**
  - column mode 00100 invokes a character set;
  - bit D3 of mode 01100 sets half brightness;
  - b6 to b3 of row mode 00000 set the background and foreground of the whole page.
- **Two-byte codes** whose first byte is 7/F are downloaded characters, and 7F7E fills the rows under a Chinese character.

### Spain's even parity

Spain's 1986 order put even parity on the row bytes that X/26 overwrites, rather than the standard's stand-in character. The engine already shows an X/26 character over a byte that failed parity (decision E.5 rule (g)), so such pages decode correctly; the writer of section 9 writes stand-ins with odd parity, as the standard asks.

## 4. Glyphs of 12 × 20

Loaded fonts today are 12 × 10, each line drawn twice in the 12 × 20 cell (REQ-FNT-002, REQ-FNT-008, decision E.72 item 2). That is too coarse for:
- Arabic script;
- Thai and Lao, whose marks stack above and below;
- Vietnamese letters carrying a tone mark over a circumflex, breve or horn;
- Armenian and Georgian, whose letters have descenders and ascenders.

The prototype found every in-scope script legible at 12 × 20.

### The change

- **`LoadedGlyph`** holds 20 lines as well as 10. Today it is ten rows, at `crates/neoteletext-core/src/glyphs/mod.rs:140`. The G3 glyphs already have a 20-line form, `G3Glyph.large`.
- **A 12 × 20 glyph** is drawn line for line in a normal cell. Each line is drawn twice in double height and double size, as a 12 × 10 glyph's lines are today.
- **A 12 × 10 font** is drawn exactly as now, rounding included (REQ-FNT-008).
- **The font form (REQ-FNT-002)** also accepts `FONTBOUNDINGBOX 12 20 0 0`, with `BBX 12 20 0 0` and 20 bitmap rows in every character. One font has one height.
- **The glyph table** passed between processes (REQ-FNT-003, `docs/spec/design/font-security.md` section 3.1) carries 10 or 20 rows per entry, and its shape check covers both.
- **`neottx font convert`** keeps its rule of exact conversions only. It stops halving 12 × 20 fonts whose line pairs differ (decision E.72 item 7) and passes them through instead.

The owner decided on 18 October 2026 to allow 12 × 20 loaded fonts. This amends decision E.72 items 2 and 7, and REQ-FNT-002, REQ-FNT-003 and REQ-FNT-008.

### Memory

The core stores no font; the port supplies the glyph table (`LoadedFont<'a>`). A full 1,024-character font of 12 × 20 takes 40 KB as 16-bit rows, against 20 KB at 12 × 10. A font for one of the new sets has 96 to about 300 glyphs, 4 to 12 KB.

## 5. Fonts from Noto

### Where fonts come from

Bedstead has none of the new scripts and few of the African Latin letters, so every new set needs another font.

The owner decided on 18 October 2026 that **the Noto-derived fonts and their converter are kept in this repository.** This amends decision E.72 items 1 and 3, REQ-FNT-001, and the rule in `CLAUDE.md` that Bedstead is the only glyph data in the repository.

They are kept as Bedstead is, in a folder of their own, `third_party/noto/`:

| File | What it is |
|---|---|
| `convert.py` | The converter. A Python script with its dependencies pinned to exact versions in its inline metadata (PEP 723), run with `uv run`. It uses FreeType and HarfBuzz through `freetype-py` and `uharfbuzz`, as the prototype did. |
| `sources.txt` | Each Noto source font: its URL, its SHA-256, and the scripts taken from it. The converter downloads them into a cache outside the repository and refuses any file whose checksum differs, as the test corpus is pinned (REQ-TST-022). Sources are not committed. |
| `settings.txt` | The settings of each script (the table below), so the fonts can be regenerated exactly. |
| `*.bdf` | The converted fonts: one 12 × 20 BDF per script, in the profile of REQ-FNT-002, with a `fonts.txt` index so the folder is a font library (REQ-FNT-004). |
| `OFL.txt` | The SIL Open Font License 1.1, with each source's copyright line. Noto's licence declares no Reserved Font Name, so the converted fonts may keep "Noto" in their names. |
| `PROVENANCE.md` | Where each source came from, when, and how the fonts were made. |

`cargo xtask check-noto`, run by every gate as `check-bedstead` is, fails if a committed font differs from the checksum recorded in `PROVENANCE.md`.

**The fonts are not compiled into the engine.** They are loaded at run time as any loaded font is: chosen per character set (REQ-FNT-005), or named by the channel or the page (REQ-FNT-006, REQ-FNT-007). A small build therefore carries only Bedstead, and a port that wants them built in can embed the files itself.

`neottx` and the receivers built on the engine use `third_party/noto/` as their default font library. A page in a new set should still name its font, so that a receiver without the library draws it too. Without a font, a set's letters show as the placeholder of REQ-CHR-004.

### The converter

Drawing a font is not a decoder's job, so the converter stays outside the engine's crates and gates, beside its output. It produces BDF files in the profile of REQ-FNT-002.

The settings below are what looked best in the prototype's comparisons. All glyphs are strictly two-colour (FreeType monochrome), and glyph choice and mark placement come from HarfBuzz.

| Script | Noto font | Weight | Hinting | Size in the 12 × 20 cell |
|---|---|---|---|---|
| Latin, Cyrillic, Greek | Noto Sans Mono | 600 | FreeType's automatic hinter | Capitals 14 lines tall, H 10 pixels wide, baseline on line 14: Bedstead's metrics |
| Vietnamese | Noto Sans Mono | 600 | automatic | Capitals 13 lines, H 10 pixels wide, baseline on line 15, leaving room for stacked marks |
| Thai, Lao | Noto Sans Thai (loopless), Noto Sans Lao | 800 | automatic | Reference consonants (กคนมบ) 10 lines tall and 10 pixels wide, baseline on line 15 |
| Arabic script | Noto Naskh Arabic | 500 | the font's own | Fitted to the cell; joining strokes extended to the cell edges |
| Hebrew | Noto Sans Hebrew | 600 | automatic | As Latin |
| Georgian, Armenian, Tifinagh | the Noto Sans font for each script | 600 | to be compared | As Latin; not yet tried |

### What was learnt

- **Weight 700 is too heavy:** the three stems of m merge in 10 pixels. Below 500, strokes break.
- **Supersampling never helped:** neither 4× nor 8× antialiasing followed by a threshold beat hinting.
- **Size by reference letters, not by the font's em:** sizing by em lets the tallest stacked capital shrink every letter.
- **Widen to Bedstead's width:** the display pixel is 1.39 times wider than tall (480 × 500 on a 4:3 screen).
- **Shrink, don't clip:** a glyph that still overflows the cell, such as an accented capital or a stacked cluster, is redrawn smaller until it fits, as Bedstead's accented capitals are.

The converter must also produce the extra glyphs that sections 6 and 7 need:
- the positional forms of each joining letter;
- the precomposed Vietnamese letters;
- the Thai and Lao marks drawn at the bottom or top of the cell (section 7.6).

## 6. Arabic script: letters joined by the receiver

The owner left this choice to the designer, and on 18 October 2026 it was decided as below: letters are joined in the receiver. Sending the forms, as teletext Arabic does, cannot fit Urdu, Pashto or Sindhi in 96 positions. Receiver joining fits every Arabic-script language in one set, and its cost is a small table and one pass over each row.

### Why not the existing method

Teletext's Arabic G0 set (Table 44) holds the forms a fixed-pitch display needs: initial and medial forms, and one form serving as both isolated and final. The broadcaster chooses each cell's form. Rows are in visual order, right to left as displayed, so the receiver needs no bidirectional processing.

That method uses about 70 positions for Arabic's 28 letters. Urdu has 39 letters, Pashto 44 and Sindhi 52, so their forms cannot fit in 96 positions.

### The design

- **Each set holds plain letters, one position per letter.** Their joined forms are not in the set. Urdu needs about 39 letters, 10 digits, the Urdu punctuation ۔ ، ؟ ؛ and a few Latin marks, about 60 positions. The largest set, Sindhi, needs about 75.
- **Rows stay in visual order,** as Table 44's do. The receiver does no bidirectional reordering; the page's author does it (section 9).
- **The receiver chooses each letter's form** from its neighbours in the row:
  - Each letter has a joining type, from the Unicode file `ArabicShaping.txt`, held as a table in the core: dual-joining, right-joining, non-joining, or joining-causing (the tatweel, ـ).
  - Reading order runs right to left, so a letter's previous letter is the cell to its right and its next letter is the cell to its left.
  - A letter joins its previous letter when both can join that way. It joins its next letter when it is dual-joining and the next letter can join.
  - The two answers give its form: isolated, final, initial or medial.
- **Joining stops at any cell that isn't a letter of the set.** That includes spaces, control codes, mosaics, other sets and cells changed by X/26. A colour change inside a word therefore breaks the join, as it would on any teletext row.
- **The lam-alef ligature is one character of the set,** as it is at 7/E of Table 44. The author writes لا as that code, and the receiver treats it as right-joining.
- **X/26 is applied first.** The forms are chosen after the page's enhancement data, from the characters the cells finally hold.

### Glyphs for the forms

A font's glyphs are labelled by Unicode code point (REQ-FNT-002), so each form needs its own code point:
- **Arabic Presentation Forms** (U+FB50 to U+FDFF and U+FE70 to U+FEFF) are used where Unicode has the form. That covers all of Persian's and Urdu's letters and most of Sindhi's and Uyghur's.
- **A private-use code point** is used for any other form. These are in Supplementary Private Use Area-A, from U+F0000, allocated by a table in the core. Several Pashto letters, such as ټ ځ څ ډ ړ ږ ښ ګ ڼ, have no presentation forms.

A font conforms for such a set (REQ-FNT-011) when it has every position and every form of every joining letter.

### Text output

`to_unicode` returns the plain letter, not the form. Text taken from the page then reads naturally, though it is still in visual order, as text from Table 44 is today.

### Cost

About 150 lines in the core, with no allocation: the joining-type table, the form rule run over each row after X/26, and the code point table. The renderer only looks up a different code point.

## 7. Thai and Lao: the Philips SAA5246AP/S scheme

Thai is the one new language for which real teletext hardware exists, so Thai follows that hardware exactly, as decision E.75 asks ("character sets as the teletext hardware expected"). Lao has no such hardware and follows the same scheme.

### 7.1 Sources

| Source | What it gives |
|---|---|
| Philips, *Integrated VIP and Teletext (IVT1.0), SAA5246A*, product specification, January 1993: Table 13 (PDF p. 35), Tables 22 and 23 (pp. 45 and 46). In `~/devel/teletext-reference/teletext-fonts/manufacturers/philips/datasheets/saa5246a/`. | The SAA5246AP/S, the Thai variant of the decoder chip that is this project's receiver baseline: its Thai character set, drawn glyph by glyph, and how a page selects it. |
| Sayan Theerapanyawat, *Development of a teletext decoder with Thai/English fonts*, MEng thesis, Chulalongkorn University, 1993. In `~/devel/teletext-reference/captions/thailand/teletext/`. | ARMTEXT, Thailand's only teletext service, run by Tele-Information Co. with Army TV Channel 5: about 700 pages, Thai from page 200. A photograph of page 200 (chapter 6, Fig. 6.5). A decoder program that marks a Thai page with 0/A in row 1 and switches with ESC. |
| *World System Teletext and Data Broadcasting System*, December 1990 (`~/devel/teletext-reference/teletext-standards/Teletext1990-blunham.pdf`): Figure 31 (p. 171) and §14.7 (pp. 39–41). | The standard's own Thai G0 set of 1990, with Thai digits and the marks in column 5, and the X/26 Format B that overlays marks in a letter's cell. Withdrawn from EN 300 706. |
| Wanchaleam Pora, *A VHDL model of a teletext IC*, Chulalongkorn, 1995. Chanarong Angthong, *Development of a teletext inserter using VHDL*, Chulalongkorn, 1996. Same folder. | The same switching as Sayan's decoder, in a gate-level design. The 1996 inserter reproduces the SAA5246AP/S table and says Thai could not yet be encoded to match the chip. |

The two Chulalongkorn decoders signal Thai their own way: a 0/A byte at the start of row 1, and their own character ROMs. That signalling is outside the standard, and their ROM contents are not published. The SAA5246AP/S is fully documented and uses only the standard's mechanisms, so it is the scheme followed here.

**The 1990 standard's Thai, and why the chip's scheme is preferred.** WST90 Figure 31 is a different Thai G0 set: Thai digits in column 3, consonants and vowels in columns 4, 6 and 7, and the vowel signs and tone marks in column 5. Sent in a row, a column 5 mark takes a cell; sent in X/26 Format B, it is overlaid on a letter's own cell, so a Thai line takes one row, not two. The chip's scheme is preferred because:
- **Format B is not in EN 300 706.** ETS 300 706 (1997) dropped it, so using it would be a departure from the current standard, which the chip's scheme avoids.
- **No hardware is known to have used Figure 31 or Format B.** The SAA5246AP/S and ARMTEXT are real.
- **Two marks on one letter are uncertain in Format B.** A letter with both a vowel sign and a tone mark needs two marks in one cell, and Format B's addresses step forward by one to eight cells, so whether two marks can address the same cell is not clear from §14.7.

Reading pages coded to Figure 31 could be added later as a second Thai designation if any are found.

### 7.2 Selecting Thai

The chip uses the standard's own two mechanisms, unchanged:
- **National option bits.** In a Thai receiver, C12 to C14 = 010 means a Thai page, and 000 an English page (Tables 22 and 23; other values give English).
- **ESC, which Philips calls TWIST.** On a Thai page, ESC switches the rest of the row to English; on an English page, to Thai. Every row starts in the page's own set.

In the standard's terms this is a page with two G0 sets, Thai and Latin English, with ESC toggling between them (clause 15.3, REQ-CHR-009), exactly as Arabic and Hebrew pages mix with Latin. At Level 1 the two sets are "implied by a local Code of Practice" (clause 15.1), which the receiver's region setting supplies. neoteletext gives this a designation of its own in group 1100 (section 3), keeping the chip's option bits:

| Designation | Default G0 | Second G0 (ESC) | G2 |
|---|---|---|---|
| 1100 010 | Thai | Latin, English | Thai |
| 1100 000 | Latin, English | Thai | Thai |

A Level 1 page from a Thai service, with only C12 to C14 = 010 in its header and the receiver's region set to group 1100, then decodes as it did on the SAA5246AP/S.

### 7.3 The Thai G0 set

The Thai page's 96 positions (Table 23, columns 2 to 7). Columns 2 and 3 are the usual punctuation and digits; 7/F is the block, as in every G0 set.

| | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | A | B | C | D | E | F |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **2** | space | ! | " | # | $ | % | & | ' | ( | ) | * | + | , | - | . | / |
| **3** | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | : | ; | < | = | > | ? |
| **4** | ๅ? | ก | ข | ค | ฆ | ง | จ | ฉ | ช | ซ | ฌ | ญ | ฎ | ฏ | ฐ | ฑ |
| **5** | ฒ | ณ | ด | ต | ถ | ท | ธ | น | บ | ป | ผ | ฝ | พ | ฟ | ภ | ม |
| **6** | ย | ร | ฤ | ล | ว | ศ | ษ | ส | ห | ฬ | อ | ฮ | ฯ | ะ | ◌ั ↑ | า |
| **7** | ◌ิ ↑ | ◌ี ↑ | ◌ึ ↑ | ◌ื ↑ | ◌ุ ↓ | ◌ู ↓ | เ | แ | โ | ใ? | ไ? | ๆ | ◌็ ↑? | ◌่ ↑ | ◌้ ↑ | ■ |

- **↑ marks a mark drawn at the bottom of its cell,** for the row above a letter (section 7.5). **↓ marks one drawn at the top,** for the row below.
- **? marks a reading still to be confirmed** by a reader of Thai or by a dump of the chip's ROM. 4/0 is a tall stroke with a foot, probably ๅ. 7/9 and 7/A are small, low glyphs read as ใ and ไ from their place after เ แ โ. 7/C is a small mark read as ็.
- **The order is the chip's, not TIS-620's.** The consonants run alphabetically from 4/1 without the obsolete ฃ and ฅ, the vowels follow, and the marks are placed with the vowels.
- **Not in the set:** ฃ ฅ ฦ, the Thai digits ๐ to ๙, ฿ ๏ ๚ ๛, and ำ ์ ๊ ๋ ํ with the stacked marks of the G2 set below.

### 7.4 The Thai G2 set

The chip has 32 more glyphs: the less common marks, and every vowel sign combined with a tone mark, each in one cell. They are at 8/0 to 9/F of Table 13, codes the chip reaches only in its 8-bit mode, which teletext's 7 bits and parity do not allow. In the standard, characters beyond a page's 96 come from the G2 set through X/26 (clause 15.1, Level 1.5), so neoteletext puts them in a Thai G2 set: Latin G2 (Table 37) with columns 5 and 6 replaced by the chip's columns 8 and 9, position for position.

| Thai G2 | Chip | Glyph | Thai G2 | Chip | Glyph |
|---|---|---|---|---|---|
| 5/0 | 8/0 | ◌๊ | 6/0 | 9/0 | ◌ึ่ |
| 5/1 | 8/1 | ◌๋ | 6/1 | 9/1 | ◌ึ้ |
| 5/2 | 8/2 | ◌์ | 6/2 | 9/2 | ◌ึ๊ |
| 5/3 | 8/3 | ◌ั่ | 6/3 | 9/3 | ◌ึ๋ |
| 5/4 | 8/4 | ◌ั้ | 6/4 | 9/4 | ◌ื่ |
| 5/5 | 8/5 | ◌ั๊ | 6/5 | 9/5 | ◌ื้ |
| 5/6 | 8/6 | ◌ั๋ | 6/6 | 9/6 | ◌ื๊ |
| 5/7 | 8/7 | ◌ิ่ | 6/7 | 9/7 | ◌ื๋ |
| 5/8 | 8/8 | ◌ิ้ | 6/8 | 9/8 | ◌ํ่ |
| 5/9 | 8/9 | ◌ิ๊ | 6/9 | 9/9 | ◌ํ้ |
| 5/A | 8/A | ◌ิ๋ | 6/A | 9/A | ◌ํ๊ |
| 5/B | 8/B | ◌ิ์ | 6/B | 9/B | ◌ํ๋ |
| 5/C | 8/C | ◌ี่ | 6/C | 9/C | ◌ํ |
| 5/D | 8/D | ◌ี้ | 6/D | 9/D | not yet identified |
| 5/E | 8/E | ◌ี๊ | 6/E | 9/E | not yet identified |
| 5/F | 8/F | ◌ี๋ | 6/F | 9/F | not yet identified |

All of these are drawn at the bottom of the cell, for the row above a letter. 9/D to 9/F are full-height glyphs, perhaps ฦ, ฃ or ฅ; they need the same confirmation as section 7.3's.

A page places a G2 glyph in a cell with an X/26 "character from the G2 set" triplet (Table 29, column mode 01111). The row itself carries the nearest G0 character as its stand-in, as the standard expects: the vowel sign alone for a vowel-and-tone combination, a space otherwise. A Level 1 decoder therefore shows the vowel without its tone.

**The budget is small.** Measured on the Universal Declaration of Human Rights in Thai, about 13 in every 400 character clusters need a G2 glyph:
- a vowel sign with a tone mark: 2.7% of clusters;
- ำ: 0.8%;
- ์: 0.5%;
- ๊ and ๋: none in that text.

A full Thai page holds about 400 clusters (section 7.5), so about 13 triplets plus a row address triplet for each row, well within X/26's 208.

### 7.5 Layout: marks on rows of their own

The chip draws each character in its own cell, one byte per cell, with no overlaying. It draws the upper vowels and tone marks at the bottom of their cells, and the lower vowels ุ ู at the top. A mark placed in the row above a consonant, in the same column, therefore sits on it; a lower vowel placed in the row below hangs from it.

The ARMTEXT photograph shows this layout: Thai letters set close with no gaps, marks directly above their consonants (the tone over sara ii in นี้), and Thai lines about two rows apart where English rows are packed together.

Neither source states this layout in words. It is inferred from where the chip draws the marks and from the photograph, and it is the only layout in which those glyphs read correctly.

A Thai line therefore takes two rows:

| Row | Holds | Glyphs drawn |
|---|---|---|
| n | The marks above line 1: upper vowels, tone marks, ็ ์ ํ and the G2 combinations, each in the column of its consonant. Also the lower vowels of the line above, if any. | Upper marks at the bottom of the cell; lower vowels at the top |
| n + 1 | Line 1's consonants, the vowels written beside them (ะ า เ แ โ ใ ไ), ฯ ๆ, digits and punctuation | Full cell |
| n + 2 | Line 1's lower vowels and line 2's upper marks, sharing the row | As row n |
| n + 3 | Line 2's letters | Full cell |

**When a column needs both.** If a consonant in one line has a lower vowel and the consonant beneath it in the next line has an upper mark, the shared row cannot hold both. The page writer then gives that pair of lines a third row.

On the Thai sample this happens in about 0.4% of columns: a lower vowel in 2.2% of clusters times an upper mark in 19%. That is about two extra rows per page.

**ำ** (sara am) has no G0 glyph. It is written as its two parts, which is its Unicode compatibility decomposition: ํ in the mark row over the preceding consonant, and า in the letter row. With a tone mark, the mark row takes the tone-and-ํ combination from G2 (6/8 to 6/B).

**Capacity.** Rows 1 to 23 give about 11 Thai lines of 38 cells, roughly 400 character clusters: half the lines of a Latin page, as the ARMTEXT photograph shows. Double height doubles a mark row with its letter row, as for any row.

### 7.6 What the receiver does

Nothing new. Thai uses only what the engine has or this design adds for every set: two G0 tables, a G2 table, designation entries, ESC, and X/26 G2 characters. There is no joining, no overlaying and no change to how a row's bytes map to its cells. A character's column is always its byte position.

The appearance is the decoder's choice (clause 15.6), so the Noto font for Thai keeps the chip's convention:
- **upper marks and combinations** are drawn at the bottom of the cell, where HarfBuzz places them over a consonant in the cell below, moved up one row;
- **lower vowels** are drawn at the top, moved down one row;
- **everything else** is drawn in the full cell.

### 7.7 Lao

Lao has the same structure: consonants and vowels written beside them in a row, vowel signs above and below, tone marks above those. No Lao teletext hardware is known, but WST90 defines a Lao G0 set (Figure 33, p. 173), so Lao keeps that table's codes and follows the Thai scheme for layout:
- the Lao G0 set of WST90 Figure 33: consonants, side vowels, upper and lower vowel signs, tone marks, digits and punctuation, its marks drawn at the edges of their cells as Thai's are;
- a Lao G2 set of vowel-and-tone combinations, used through X/26;
- the same two-row lines.

Lao's designation is 1100 100 with Latin English as its second set. Its table is written in phase 4.

### 7.8 Not yet known

- **Seven Thai glyphs await confirmation:** 4/0, 7/9, 7/A and 7/C of the G0 set, and three of the G2 glyphs (the chip's 9/D to 9/F).
- **What ARMTEXT itself transmitted.** The 1995 thesis calls the Thai code a "TLI code", probably Tele-Information's own, which Sayan's decoder offsets by 0x83 into its own ROM. Whether ARMTEXT's codes match the SAA5246AP/S table is unknown. A Thai off-air capture, or the operator's "Teletext system introduction document" cited by Sayan, would settle it.
- **Whether lower vowels shared a row with the next line's upper marks,** as section 7.5 assumes, or always had a row of their own.

## 8. The sets

Each set below is provisional. Its 96-position table is written as reviewed data in the phase that adds it, checked against the language's letters in Unicode CLDR, and shown to a reader of the language. Every G0 set keeps the space at 2/0 and the block ■ at 7/F (clause 15.6).

### 8.1 Group 0101, Latin extensions

Options 000 to 101 are national option sub-sets: the 13 positions of Table 36 replaced, the rest of Latin G0 kept. Options 110 and 111 have the same layout but are whole G0 sets, as Cyrillic and Greek are, because their letters must carry X/26 marks: an X/26 diacritical mark is placed on the G0 set's base form at a code, never on a national option's character there (REQ-CHR-002), so a tone on ê at a national position would be drawn on the base form `|`. Letters that don't fit come through X/26, either from Latin G2 (which has Ŋ ŋ) or as a Latin letter with one of the 15 G2 diacritics, which covers the dot-below letters ẹ ọ ṣ ḍ ṭ.

| Option | Languages | Letters in the 13 positions |
|---|---|---|
| 000 | Hausa, Fula | Ɓ ɓ Ɗ ɗ Ƙ ƙ Ƴ ƴ Ŋ ŋ Ɲ ɲ ʼ |
| 001 | Akan, Lingala, Bambara, Dyula, Mandinka, Krio, Mende, Dinka, Nuer, Fon | Ɛ ɛ Ɔ ɔ Ŋ ŋ Ɲ ɲ Ɣ ɣ Ɖ ɖ, one spare |
| 010 | Ewe | Ɖ ɖ Ɛ ɛ Ƒ ƒ Ɣ ɣ Ɔ ɔ Ʋ ʋ, one spare (Ŋ ŋ from G2) |
| 011 | Kabyle, Tamazight (Latin) | Ɛ ɛ Ɣ ɣ Č č Ǧ ǧ ḍ ḥ ṛ ṣ ṭ (capitals with dot below, and ẓ, by X/26) |
| 100 | Azerbaijani | Ə ə Ğ ğ ı İ ç ö ş ü Ç Ö Ş (Ü by X/26) |
| 101 | Kanuri, Mossi | Ǝ ǝ Ɍ ɍ Ɛ ɛ Ɩ ɩ Ʋ ʋ Ŋ ŋ, one spare |
| 110 | Yoruba, Igbo | Ẹ ẹ Ọ ọ Ṣ ṣ Ị ị Ụ ụ Ṅ ṅ, one spare |
| 111 | Vietnamese | Ă ă Â â Ê ê Ô ô Ơ ơ Ư ư đ (Đ from Latin G2 6/2 through X/26); paired with the Vietnamese G2 set |

**Vietnamese** uses the standard's Level 1.5 method for its tones:
- **It is a Latin-script G0 set:** Latin G0 with the 13 national option positions holding the vowels with a circumflex, breve or horn and đ, so a cell never needs more than one mark. It is a whole set rather than a national option so that X/26 finds those vowels as bases (REQ-CHR-002). Its tones stay within the spirit of EN 300 706 Annex E.10, which gives diacritical marks to the Latin sets: every base is a Latin letter. A decoder that does not know the designation falls back to Latin G0 (REQ-CHR-003).
- **The five tone marks are added by X/26,** as "G0 character with diacritical mark" triplets (Table 29, modes 10000 to 11111), from column 4 of the paired G2 set.
- **The Vietnamese G2 set** is Latin G2 (Table 37) with one change: 4/C, the low line, becomes the hook above. Grave, acute, tilde and dot below are already at 4/1, 4/2, 4/4 and 4/9. The standard takes the mark "from column 4 of the G2 supplementary character set" currently designated (Table 29, modes 10000 to 11111), so a page paired with the Vietnamese G2 set gets the hook above. The engine takes the mark from the page's G2 set (`charset::page_mark`).
- **The budget fits.** Measured on the Universal Declaration of Human Rights, 15% of characters carry a tone, about 131 on a full page, and Đ adds a few more. With a row address triplet for each of up to 23 rows, that is about 160, within the 208 of 16 X/26 packets.
- **Older receivers degrade gracefully.** A decoder without Level 1.5 shows the letters without tones, readable as Vietnamese typed without tones often is.

Yoruba and Igbo work today without tone marks, with ẹ ọ ṣ ị ụ from X/26. Option 110, a whole set for the same reason as Vietnamese, puts those letters in the row, so a tone mark can be added to them by X/26 with the Latin G2 grave and acute: one mark per cell, as the standard allows. Yoruba marks most vowels; whether a dense page stays within X/26's 208 triplets has not been measured.

### 8.2 Group 0111, Cyrillic extensions

These are whole sets, each built from the EN 300 706 Cyrillic table nearest to it, so that letters keep their codes across sets:
- **Table 38 (option 1)** is the Cyrillic form of the teletext Serbian, Croatian and Slovenian Latin option (Table 36): Ч Ћ Ж Ђ Ш sit where Č Ć Ž Đ Š do, so a Latin and a Cyrillic page put corresponding letters at the same codes. It is not the Yugoslav national standard JUS I.B1.003 (ISO-IR 146), whose layout differs. It has Macedonian's Ѓ Ќ, but no lowercase џ (7/F is the block) and no Ѕ ѕ.
- **Table 40 (option 3)** is Ukrainian, lacking only Ґ ґ.
- **Table 39 (option 2)** is Russian and Bulgarian. Russia's GOST R 50861-96 (Fig. 8, p. 12) uses exactly this table, with no Ё: Russian teletext wrote Е for Ё.

The other sets make room for their own letters by giving up letters the language does not use, the capitals that cannot begin a word (Ъ Ы Ь), and punctuation positions.

| Option | Language | Built from | Changes |
|---|---|---|---|
| 000 | Ukrainian | Table 40 | Ґ ґ in two punctuation positions; everything else as Table 40, so a Table 40 page reads the same |
| 001 | Belarusian | Table 39 | І і Ў ў in place of И и Щ щ, which Belarusian does not use; Ъ ъ give way to the apostrophe it uses instead |
| 010 | Macedonian | Table 38 | Ѕ ѕ at 5/D and 7/D in place of Ђ ђ, and џ at 7/B in place of ћ, letters Macedonian does not use |
| 011 | Serbian | Table 38 | џ at 7/1 in place of ќ, which Serbian does not use: Table 38 completed |
| 100 | (spare) | | |
| 101 | Kyrgyz, Mongolian | Table 39 | Ң ң Ө ө Ү ү |
| 110 | Tatar | Table 39 | Ә ә Ө ө Ү ү Җ җ Ң ң Һ һ |
| 111 | Tajik | Table 39 | Ғ ғ Ӣ ӣ Қ қ Ӯ ӯ Ҳ ҳ Ҷ ҷ |

Kazakh has no Cyrillic set: the owner decided on 18 October 2026 to use the Latin alphabet Kazakhstan has adopted. It already works with the Turkish national option, which has Ğ ğ I ı İ Ö ö Ş ş Ü ü, and X/26 for Ä ä Ñ ñ Ū ū (a Latin letter with diaeresis, tilde or macron).

### 8.3 Group 1001, Arabic script

These are whole sets of plain letters, joined by the receiver (section 6). Each holds:
- the language's letters;
- the lam-alef ligature;
- digits at 3/0 to 3/9, drawn in the language's own shapes (Extended Arabic-Indic ۰ to ۹ for Persian, Urdu and the others that use them), as EBU Tech 3232 (p. 12) treats Arabic-Indic digits: a way of drawing the codes 0 to 9, not further characters. This frees ten positions for the larger alphabets such as Sindhi and Pashto;
- the Arabic punctuation ، ؛ ؟, plus ۔ for Urdu;
- the tatweel;
- as much Western punctuation as fits.

| Option | Languages | Letters |
|---|---|---|
| 000 | Persian, Dari | 32 |
| 001 | Urdu | 39 |
| 010 | Pashto | 44 |
| 011 | Sindhi | 52 |
| 100 | Sorani Kurdish | about 35 |
| 101 | Uyghur | 32 |
| 110 | Balochi | about 30 |
| 111 | Arabic, in plain letters | 28 |

Option 111 lets Arabic pages be written with receiver joining too. Table 44 remains for broadcast pages.

Urdu is drawn in Naskh style. Nastaliq, the style Urdu readers expect, slopes across letters and cannot be drawn one letter per cell.

### 8.4 Group 1011, other alphabets

| Option | Language | Layout |
|---|---|---|
| 000 | Georgian | As Hebrew G0 (Table 46): ASCII punctuation, digits and Latin capitals in columns 2 to 5; the 33 Mkhedruli letters in columns 6 and 7 and two of column 5's national positions. Georgian has no case in everyday use. |
| 001 | Armenian | 38 capitals and 38 small letters, digits, and the punctuation ։ ՝; the ligature և and the rarer punctuation ՛ ՞ ՜ in an Armenian G2 set through X/26; Latin through the second G0 set. Tight. |
| 010 | Tifinagh | The 33 letters of the Moroccan (IRCAM) alphabet and ⵯ, laid out as Georgian. |
| 011 | Reserved | Inuktitut syllabics was taken out of scope (decision E.78 item 7, `requirements/out-of-scope.md` item 16): the syllabary does not fit 96 codes. |

### 8.5 Group 1100, Thai and Lao

| Option | Language | Layout |
|---|---|---|
| 000 | Latin English, with Thai as the second set | As the SAA5246AP/S's English pages (section 7.2) |
| 010 | Thai | The SAA5246AP/S Thai set (section 7.3) |
| 100 | Lao | The same scheme (section 7.7) |

## 9. Writing pages

A set nobody can write pages in is not supported. `neoteletext-app` gains a writer, `write_text`, behind its feature `write-text`, and `neottx write-text` is its command (REQ-CLI-010). It lives in layer L9 because it needs the core's character tables, which the formats crate in layer L0 may not use (ARCHITECTURE.md section 9). It turns Unicode text into a page in a named set:
- **An error for a missing character:** it fails, naming the character, rather than dropping or substituting it silently.
- **Vietnamese, Yoruba and Igbo:** it writes each letter without its tone in the row, with odd parity as the standard's stand-in, and the tone as an X/26 diacritical mark triplet on that cell. It fails if a page needs more triplets than X/26 carries.
- **Arabic-script digits:** it writes the language's digits (۰ to ۹ and the others) as the codes 3/0 to 3/9.
- **Thai and Lao:** it lays each line out on two rows (section 7.5):
  - letters and side vowels on the letter row;
  - upper marks, and lower vowels, on the rows around it, each in its consonant's column;
  - a vowel-and-tone pair as one G2 glyph with its X/26 triplet;
  - ำ as ํ and า;
  - a third row only where a column needs both a lower vowel and the next line's upper mark.

  It maps Thai digits to Western digits and fails on the characters section 7.3 lists as missing.
- **Arabic script:**
  1. it writes لا as the lam-alef code;
  2. it breaks lines at the row width, counting cells;
  3. it reverses each line into visual order, keeping runs of digits and Latin text left to right.

  Step 3 is the Unicode Bidirectional Algorithm (UAX #9), applied to each line after it is broken: it orders runs of letters, digits and Latin text, and mirrors brackets. The writer uses the `unicode-bidi` crate for it (MIT or Apache-2.0); the core and the receiver never reorder. A rule of our own would cover letters and digits but would get neutral characters, brackets and mixed Latin wrong in cases that are easy to meet and hard to test.
- **Width:** it reports the cells the row will take after section 7's compaction, so authors can lay out a page.

It then hands the page to the page builders of `crates/neoteletext-formats/src/author.rs` and the TTI writer (REQ-INP-023), which:
- write the designation as an X/28/0 Format 1 packet;
- set C12 to C14 in the header;
- add a font reference for the set, where the page or channel names one.

A `neottx` command writes a page from a UTF-8 text file in a named set, so that pages can be made without code.

## 10. Security

- **The new tables are constants in the core,** like the existing ones. Nothing received can change them.
- **The joining rule is a bounded pass over one row** of at most 40 cells. They use no allocation, no recursion and no division, as the core requires (REQ-GEN-006, REQ-GEN-016).
- **The private-use code points are the core's own allocation.** A font can only supply glyphs for them, never define them.
- **12 × 20 fonts are read by the same confined reader** as 12 × 10 fonts, under the same size limits (REQ-FNT-002, REQ-FNT-003). The reader's fuzz target (REQ-TST-040) gains 20-row inputs.
- **Text output keeps stripping bidirectional controls** (REQ-SEC-010). Arabic-script rows are already in visual order, as Table 44's are.

## 11. Testing

- **Each set's table** is tested position by position against a reviewed data file listing each character's Unicode value. Every existing set's table is unchanged, and its existing tests must still pass.
- **Writing and reading round-trip:** each sample text, written for its set and decoded, gives back the same Unicode, apart from the substitutions the set documents.
- **Joining:**
  - the form chosen for every joining type, before and after every other type;
  - breaks at spaces, control codes and other sets;
  - the lam-alef ligature;
  - X/26 changing a letter inside a word.
- **Vietnamese:** every tone on every vowel of the set through X/26, the hook above from the Vietnamese G2 set, and the page shown without X/26.
- **Thai:**
  - every position of the Thai G0 and G2 sets against the reviewed table;
  - option 010 and 000 pages with ESC in both directions, resetting at each row;
  - G2 combinations through X/26, and the same page at Level 1 showing the stand-ins;
  - the page writer's two-row layout, ำ, and the third row where a column needs it;
  - a page rendered beside the datasheet's glyphs.
- **Fonts:** a 12 × 20 font accepted, each 20-row rule broken, and 12 × 10 fonts drawn exactly as before.
- **Renderings:** a sample page for each set, for the owner and a reader of the language to judge.
- **Regression tests** fail on the code before each change, as the project requires.

## 12. Decisions

Each becomes a Part E decision at integration.

All on 18 October 2026.

1. **12 × 20 loaded fonts are allowed** (section 4), amending decision E.72 items 2 and 7.
2. **The Noto-derived fonts and their converter are kept in this repository** (section 5), amending decision E.72 items 1 and 3, REQ-FNT-001 and `CLAUDE.md`.
3. **Kazakh is written in its Latin alphabet,** with no Cyrillic set (section 8.2).
4. **Arabic-script letters are joined by the receiver** (section 6). The owner left this to the designer.
5. **Thai and Lao follow the Philips SAA5246AP/S scheme** (section 7). The designer first chose marks that take no cell, the owner having left it to the designer. On the owner's instruction that conformance comes first as far as possible, Vietnamese moved to the standard's X/26 method, as a Latin national option (section 8.1). Then the Thai hardware sources showed that Thai itself needs no departure, so Thai and Lao use the chip's scheme.
6. **The reserved designation values of section 3 are allocated** to neoteletext's own sets, accepting that other decoders show such pages wrongly. Decided by the owner.
7. **The page writer reorders right-to-left text with the `unicode-bidi` crate** (section 9). The owner left this to the designer: a correct implementation of UAX #9 is a library's job, and it is used only when writing pages.
8. **The phases run in the order of section 13.** The owner left this to the designer. Phase 1 is the foundation every later phase draws with, and improves the 430 languages already supported. Phase 2 adds the most languages for the least new behaviour, and proves the way sets are added. Phase 3 brings the largest single population, about 400 million, with one new rule. Phase 4 changes the most: how a row's codes map to its cells.

## 13. Phases

| Phase | Work | Languages gained |
|---|---|---|
| 1 | 12 × 20 loaded glyphs; Noto fonts for the sets that exist today | Better glyphs for the 430 already supported, Arabic and Hebrew above all |
| 2 | Groups 0101, 0111 and 1011: tables, the Vietnamese and Armenian G2 sets and fonts | African Latin, Yoruba and Igbo with tones, Vietnamese, Cyrillic extensions, Georgian, Armenian, Tifinagh |
| 3 | Group 1001 and the joining rule; the page writer's reordering | Persian, Dari, Urdu, Pashto, Sindhi, Sorani, Uyghur, Balochi |
| 4 | Group 1100: the Thai and Lao tables, and the page writer's two-row layout | Thai, Lao |

The page writer of section 9 grows with each phase.

Each phase is one feature, worked as the project works: rapid iteration until the owner is satisfied, then integration (`CLAUDE.md`).

## 14. Not yet checked

- **No table has been laid out position by position.** The capacity figures in section 8 are counts of letters and punctuation. Armenian is the tightest.
- **Georgian, Armenian, Tifinagh and Lao** have not been rendered.
- **The Thai items of section 7.8:** seven glyphs to confirm, what ARMTEXT transmitted, and whether lower vowels shared rows.
- **The WST90 tables** (Thai, Lao, Burmese, Devanagari and the other Indic sets) have been located but not transcribed; Lao's is needed for phase 4.
- **The exact positions of Ґ ґ** in the Ukrainian set, and the punctuation each Cyrillic set gives up.
- **The letter lists in section 8.1** come from CLDR's main letters for each language. They need a reader's check, especially for the shared options.
- **Speaker figures** are Google Fonts' population data, which counts some second-language speakers and overlaps between languages.
