# SAA5050 / teletext rendering: answers to the open questions

Clean-room note: everything below describes behaviour in my own words. No code or glyph data is copied. Line numbers refer to the copies downloaded on 2026-10-01 (master/main branches).

Sources:
- **MAME** `src/devices/video/saa5050.cpp` and `saa5050.h`. Licence line: `// license:BSD-3-Clause`; copyright holders Curt Coder and Nigel Barnes (cpp line 1–2). The ROM is described as "verified both from datasheet listing and decap" (cpp:45).
- **Bedstead** `/Users/andrewstuart/devel/neoteletext-engine/third_party/bedstead/bedstead.c`. Released under CC0; the SAA5050 typeface is also discussed under CDPA s.55 (lines 1–26).
- **jsbeeb** `src/teletext.js` (mattgodbolt/jsbeeb).
- **beebjit** `teletext.c` (scarybeasts/beebjit). Note: its glyph header comes from b-em and is GPL.
- **b-em** `src/video.c` (stardot/b-em; the Mode 7 code is in `mode7_render`, around lines 612–770).
- **vhs-teletext** `teletext/elements.py`, `parser.py`, `charset.py`.
- **Hardware reverse engineering:** stardot thread "SAA5050 reverse engineering" (https://stardot.org.uk/forums/viewtopic.php?t=21608, pages 1–3; posters dreamseal, scarybeasts, Richard Russell). This is decap-based analysis of the real chip.

---

## 1. Character rounding rule

**Answer: the candidate rule is exactly MAME's rule.** jsbeeb uses the same rule, and beebjit and Bedstead use an equivalent one. Confidence: high.

- **MAME** `character_rounding(a, b)` returns `a | ((a>>1) & b & ~(b>>1)) | ((a<<1) & b & ~(b<<1))` (saa5050.cpp:433–436). This is the candidate expression, symbol for symbol.
  - `get_rom_data` (cpp:413–426) takes one 5-dot ROM row and widens every dot to 2 half-dots. The result sits in a 12-bit word, so the cell is 6 dots or 12 half-dots, and the leftmost dot column is always blank. Bit 11 is shifted out first, so the high bits are on the left (cpp:599–607).
  - A row index outside 0..19 returns 0, so no rounding happens across the top or bottom edge of the cell.
- **Which neighbour row** (cpp:494): the rounding call passes the current display line `ra` (0..19) and a second line `ra + (odd ? +1 : -1)`. The ROM row used is `line >> 1`. So:
  - On the **upper** sub-line of a ROM row (even `ra`), B is the ROM row **above**.
  - On the **lower** sub-line (odd `ra`), B is the ROM row **below**.
  - jsbeeb (teletext.js:59–69) does the same. Its sub-line parity comes from the 6845 RA0 signal fed to the CRS pin (teletext.js:388–393, 433–437). So "row above in the first/even field, row below in the second/odd field" matches your candidate, provided your first field is the one showing the upper sub-lines.
- **In words.** Treat the doubled row as half-dots. For each dot boundary on the current line A:
  - If dot d is lit in A, dot d+1 is lit in the neighbour row B, and dot d is **not** lit in B, then light the **left** half of dot d+1 on this line.
  - Mirror image: if dot d is lit in A, dot d−1 is lit in B, and dot d is not lit in B, then light the **right** half of dot d−1 on this line.
  - In other words, a diagonal between two rows gets one half-dot added on each of the two sub-lines that meet at the join, on the empty side of the corner. This fills the notch of each 2×2 diagonal.
  - The rule never removes half-dots.
- **Real hardware.** Richard Russell's summary of the decap logic in the stardot thread (page 2, post #308069) compares a 2×2 group of dots, A B / C D. Half-dot "1" is added when `!A & B & C & !D`, and half-dot "2" when `A & !B & !C & D`; the other two positions are mirror images. This is the same diagonal test. He notes that "already lit" positions are "don't care", which matches the rule having no `!A[d+1]` term.
- **Bedstead** (bedstead.c:36–49 comment, and `dochar` at 4938–4995) describes the same 2×2 diagonal detection. Its unlit-pixel corner test is: the top-left quarter is filled if the left neighbour and the up neighbour are lit and the up-left one is not, and likewise for the other corners. That is identical to MAME's rule expressed per quarter-pixel. Bedstead's comment also says that at 20 px high its outlines rasterise to the SAA5050 rounded output (lines 81–85).
- **beebjit** `teletext_smooth_diagonals` (teletext.c:128–164) works on the 12×20 doubled bitmap. It lights a pixel if (down & left & !down-left), (up & right & !up-right), (down & right & !down-right) or (up & left & !up-left). This is equivalent, except that it skips the outer ring of pixels.
- **What is rounded:** alphanumerics, and blast-through characters 0x40–0x5F while in graphics mode. Mosaic blocks are **never** rounded (MAME cpp:493–496; jsbeeb teletext.js:68–71; beebjit teletext.c:189–244).
- **Implementation note for a 10-half-dot representation:** mask the result to 10 bits, because `a<<1` can carry out of the top.

**Double-height rounding in MAME** (cpp:481–486, 494):
- On a double-height cell the line index is halved (`ra >>= 1`), and 10 is added on the bottom row. The rounding call is then made with that halved index and its parity.
- So the chip effectively takes the normal 12×20 rounded glyph and repeats each rounded line twice. The top row shows rounded lines 0–9 and the bottom row shows lines 10–19. No extra rounding is done at the 40-line resolution.
- jsbeeb (glyphScanline, teletext.js:433–446), beebjit (teletext.c:592–598) and b-em (video.c `mode7_render`, around lines 733–737 and 748) all do the same.
- Whether the real chip does exactly this in double height has not been independently verified: medium confidence. It is consistent across all four emulators, and MAME's ROM and logic are decap-checked.

## 2. Separated mosaic geometry

**Confirmed:** the block heights are **3 / 4 / 3 lines** and the widths **3 / 3 dots**. In separated mode, each lit block loses its **leftmost dot column** and its **bottom line**. Confidence: high (four independent implementations agree).

6×10 cell, with row 0 at the top and column 0 at the left:

| Block | Rows (contiguous) | Rows kept (separated) | Columns kept (separated) |
|---|---|---|---|
| Top left / right (bits b1 / b2) | 0–2 | 0–1 | 1–2 / 4–5 |
| Middle left / right (b3 / b4) | 3–6 | 3–5 | 1–2 / 4–5 |
| Bottom left / right (b5 / b7) | 7–9 | 7–8 | 1–2 / 4–5 |

- Blanked **columns: 0 and 3**. Blanked **lines: 2, 6 and 9**.
- **12×20 cell:** blanked half-dot columns 0, 1, 6, 7; blanked lines 4, 5, 12, 13, 18, 19. The kept areas are therefore 4 half-dots wide and 4/6/4 lines tall.
- Evidence:
  - MAME `get_gfx_data` (cpp:369–406): groups of doubled lines 0–2 / 3–6 / 7–9. Separated mode drops ROM line 2, 6 and 9, and masks 0x3CF, which clears the left two half-dots of each 6-half-dot block.
  - jsbeeb `setGraphicsBlock` (teletext.js:92–124): separated blanks the block's first column and last line.
  - beebjit (teletext.c:207–230): separated blocks drawn at x=1 and x=4, width 2, heights 2/3/2.
  - Bedstead `domosaic` (bedstead.c:5133–5149): the Y axis points up, and separated mode shifts each tile's left and bottom edge by one pixel.
- Bit mapping in all of them: b1 = top-left, b2 = top-right, b3 = middle-left, b4 = middle-right, b5 = bottom-left, b7 (0x40) = bottom-right. b6 (0x20) is the mosaic/blast-through selector.

## 3. Hold mosaics

- **Stored form, not current form.** Confidence: high.
  - MAME saves the chartype (contiguous or separated) together with the held character (cpp:468–475) and restores it when the held character is substituted (cpp:456–461).
  - jsbeeb (`heldGlyphs`, teletext.js:411–415, 302–304), beebjit (a pointer into the glyph table active at the time, teletext.c:472–475) and b-em (`mode7_heldp`, video.c around lines 719–722) all do the same.
  - dreamseal in the stardot thread (page 2) quotes the rule that the held mosaic "is always displayed in its original contiguous or separated form regardless of the mode prevailing at the time of substitution". This is consistent with the hardware re-circulating the rendered pattern in its "graphics shift register" (page 1).
  - vhs-teletext (parser.py:69–75, `_heldsolid`) also uses the stored form.
- **What gets held:** only mosaic codes with b6 = 1 (0x20–0x3F and 0x60–0x7F) that are received while in graphics mode. Blast-through capitals do not update the held character (MAME cpp:470; beebjit teletext.c:472–478).
- **When the held character is shown at a control-code cell** (MAME cpp:456; jsbeeb teletext.js:302; beebjit teletext.c:486–489). All three conditions must hold:
  1. Graphics mode was active **before** this control code, and
  2. hold was on either before or after this code, and
  3. the height did not change at this code.
  Otherwise the cell shows a space **and the held character is reset to space**.
- **Reset events** (MAME, jsbeeb, beebjit; b-em is similar):
  - Start of every display line/row.
  - Any control code shown while the conditions above fail.
  - An actual change of size. It must be a real toggle: the stardot thread found an XOR gate that detects a height change, so repeating the current size code does not reset.
  - In alphanumeric mode, any displayable character also resets it.
  - The net effect of alpha mode is reset on a change to alphanumerics, with the code cell itself still showing the held character (set-after).
  - Contiguous/separated changes do **not** reset it.
  - vhs-teletext resets on alphanumeric colour codes and on size changes (parser.py:39–41, 90).

| Code | MAME | jsbeeb | beebjit | b-em | vhs-teletext |
|---|---|---|---|---|---|
| Hold (0x1E) | at | at | at | at | at |
| Release (0x1F) | after | after | after | after | after |

- dreamseal (stardot page 2): "Hold is 'set at' and Release is 'set after'… It takes 2 cycles of LOADCHAR to clear the Hold state… but only 1 cycle… to set it."
- Page 1 has an earlier, contradictory remark that "the Hold doesn't take effect until the next character". It was superseded by the page-2 conclusion. I'd rate "hold set-at" as medium-high confidence.

## 4. Flash timing

| Source | Counter | Hidden | Shown | Period (50 Hz fields) | Rate / ratio |
|---|---|---|---|---|---|
| Real SAA5050 (stardot RE, dreamseal) | 6-bit, +1 per DEW (per field) | first 16 counts | 48 counts | 64 fields = 1.28 s | **0.78125 Hz, 3:1 on:off** |
| MAME | `m_frame_count & 0x3f` per `dew_w` (cpp:514–523); off when `!(count & 0x30)` (cpp:488) | 16 | 48 | 64 | same |
| jsbeeb | `flashTime` 0..63 per DEW falling edge (teletext.js:337–347) | 16 | 48 | 64 | same |
| beebjit | 6-bit per vsync, visible when ≥16 (teletext.c:296–302) | 16 | 48 | 64 | same |
| b-em | per vsync: on 32, off 16 (video.c ~1292–1296) | 16 | 32 | 48 | ≈1.04 Hz, 2:1 (deviates) |

- The datasheet's "0.75 Hz, 3:1" is approximate. The silicon is 64 fields at 3:1 = 0.78125 Hz (PAL). Confidence: high.
- Flash only hides the character. The background colour is still drawn. MAME replaces the character with a space.

## 5. Double height

- **Which row becomes a bottom row.** Any 0x0D code on a row (MAME `m_dbl_height_prev_row`) makes the next row a "bottom row". The row after a bottom row is always a normal/top row again, even if the bottom row also contains 0x0D (MAME cpp:537–543; jsbeeb teletext.js:375–383; beebjit teletext.c:276–289; b-em video.c ~1228–1231).
- **What is drawn on the bottom row.** The chip draws the lower half (rounded lines 10–19, each repeated twice) of the character in each cell **where double height is in effect on that row as the chip is currently being fed**. Cells that are in normal height on the bottom row are drawn as **spaces**, but the **background colour is still drawn** (MAME cpp:490–491; jsbeeb teletext.js:455; beebjit teletext.c:580–585; b-em draws the foreground in the background colour, video.c ~742–743).
- **SAA5050-specific quirk vs EN 300 706.** The SAA5050 itself does **not** re-fetch the upper row: it renders whatever byte stream it is given on the second row. Confidence: medium-high.
  - On the BBC Micro you must repeat the text in row n+1. All BBC emulators render the second row's own data.
  - The chip outputs TLC ("transmitted large character", MAME `tlc_r`, cpp:566–569) so that the external timing/memory controller in a TV decoder can re-present row n. MAME's generic `screen_update` does exactly this: it uses TLC to read row sy−1 again (cpp:638).
  - EN 300 706 behaviour (row n+1 is ignored and row n's lower halves are shown) therefore depends on that controller. A decoder written from EN 300 706 should copy row n and ignore row n+1's content. Double-height cells are copied into the next row as the lower half, and every other cell becomes a space that keeps its attributes, including the background.
- **Timing.** 0x0D and 0x0C both take effect set-after for glyph selection in MAME (`dbl_height_prev`, cpp:445, 482) and in jsbeeb (`oldDbl`). Bottom-row blanking uses the state after processing the code.
  - vhs-teletext: double sizes are set-after and normal size is set-at (parser.py:103–107).
  - The EN 300 706 convention is double height set-after and normal size set-at, which vhs implements.
  - Since a control cell shows only a space or the held character, and a size change kills hold, the difference is mostly invisible.
- **Real-chip rounding in double height:** see §1.

## 6. Box, conceal, background

- **Start Box / End Box:**
  - MAME: TODO/no-op (cpp:293–296, header TODO line 17).
  - BBC emulators: ignored, because a Micro has no boxing.
  - vhs-teletext (parser.py:93–102) requires **two consecutive** 0x0B (or 0x0A) codes and acts on the second of the pair, set-at, so the box starts at the second code's cell.
  - On a real SAA5050, boxing only drives the BLAN/picture-insertion output used for mixing with TV video, not the RGB itself (per datasheet, not verified here: medium confidence).
- **Conceal (0x18): set-at** in every implementation.
  - MAME implements it crudely: it sets the foreground to the background colour for the current cell and after (cpp:316–318). It has no reveal control. A later colour code restores a visible foreground from the following cell onward.
  - jsbeeb (teletext.js:278–279, 424–425) and beebjit (teletext.c:383–387, 414–418): colour codes clear conceal, but "the current character remains concealed", so the reveal is set-after. Both mark conceal set-at.
  - vhs-teletext: conceal set-at, cleared by colour codes set-after.
- **Black background (0x1C) and New background (0x1D): set-at** everywhere (MAME cpp:330–336 with `m_bg` used immediately; jsbeeb, beebjit, b-em, vhs).
  - New background takes the **current** foreground, meaning after any preceding colour code has been processed. For example, "red, new-bg" gives red background from the 0x1D cell onward.
- **Colour codes (alpha 0x01–0x07, mosaic 0x11–0x17): set-after** everywhere. MAME latches `m_prev_col` before processing (cpp:450). b-em and beebjit use a copy saved before processing.
  - Codes 0x00 and 0x10 do nothing on the SAA5050 (beebjit comment at teletext.c:368–373 and 402–403: pre-2.5 presentation, black foreground not supported).
- **Flash (0x08): set-after. Steady (0x09): set-at** in jsbeeb (teletext.js:420–422) and vhs.
  - MAME uses the pre-code flash state for both codes, which makes Steady set-after as well: a minor deviation.
  - beebjit uses the post-code state for both, which makes Flash set-at.
- **Contiguous / separated (0x19 / 0x1A):** set-at in vhs. In MAME, jsbeeb and b-em they affect the *next* character's type (`m_next_chartype`). This only matters for the held character, which is in stored form anyway.

## 7. Glyph identification

The national option columns are, in order, 2/3, 2/4, 4/0, 5/B, 5/C, 5/D, 5/E, 5/F, 6/0, 7/B, 7/C, 7/D, 7/E. Row 0 is the unmodified set, then Czech, English, Estonian, French, German, Italian, Lettish/Lithuanian, Polish, Portuguese/Spanish, Rumanian, Serbian/Croatian/Slovene, Swedish/Finnish/Hungarian, Turkish.

| Position | Code point | Notes |
|---|---|---|
| Polish 5/B | **U+01B5 Ƶ** LATIN CAPITAL LETTER Z WITH STROKE | Not Ź; Ź U+0179 is not in the Polish row. Polish 7/E is ź U+017A. Confidence: high. |
| Polish 7/D | **U+0142 ł** | Polish 5/D is Ł U+0141 |
| English 6/0 | **U+2014 EM DASH** | Bedstead also uses U+2014 "emdash" for the SAA5050 long dash (bedstead.c:367). vhs-teletext uses U+2014, with U+2500 commented out as an alternative (charset.py:70–71). U+2015 is not used by any of them. |
| English 7/C | **U+2016 ‖ DOUBLE VERTICAL LINE** | Bedstead: 0x2016 "dblverticalbar" (bedstead.c:369); vhs agrees |
| Turkish 2/3 | **U+E800** (Private Use Area) | The official U+20BA TURKISH LIRA SIGN has existed since Unicode 6.2, but the teletext glyph is the older "TL" currency symbol. Mapping it to U+20BA is a reasonable modern choice but my own judgement (medium confidence). Bedstead has no Turkish TL glyph (its `lira` 0x20A4 is ₤). |
| Serbian/Croatian/Slovene 2/4 | **U+00CB Ë** | Bedstead also has Ë 0x00CB (bedstead.c:667) |
| Latin G2 4/9 | **U+002E** '.' | This position seems to be a dot below, which Unicode has only as a combining form. That form would be U+0323. vhs-teletext instead uses U+02CC MODIFIER LETTER LOW VERTICAL LINE, so the sources disagree: low confidence on what the glyph really is. |
| Latin G2 4/C | **U+02CD** MODIFIER LETTER LOW MACRON | i.e. an underline/low-line accent. vhs uses U+005F LOW LINE; Bedstead has a U+02CD glyph (bedstead.c:1193). |
| Latin G2 7/F | **U+25A0 ■ BLACK SQUARE** | vhs agrees |

Related G2 mappings in vhs-teletext, for reference:
- 4/B is U+00B8 CEDILLA, the spec glyph.
- 5/0 is U+2500.
- 5/6 is U+20A0 EURO-CURRENCY SIGN.
- 5/8 is U+03B1.
- 4/0 is U+00A0.

## 8. National option index from C12–C14

**Index = C12·4 + C13·2 + C14**, meaning C12 is the most significant bit: EN 300 706 Table 32 read as a binary number written "C12 C13 C14".
- Group 0 in order is English, German, Swedish/Finnish/Hungarian, Italian, French, Portuguese/Spanish, Czech/Slovak, (none).
  - Example: German is C12=0, C13=0, C14=1. French is C12=1, C13=0, C14=0.

**vhs-teletext is not directly comparable.**
- elements.py:177–188: `control` packs C4 as bit 0, C5–C6 as bits 1–2, and C7–C14 as bits 3–10. `codepage = (control >> 8) & 7` = C12 + 2·C13 + 4·C14, so C12 is the **least** significant bit, the reverse of Table 32's reading.
- However vhs never uses this number as a Table 32 index. parser.py:47–60 only checks that it is non-zero, and the actual subset comes from a user-chosen `localcodepage`.
- So vhs-teletext is no evidence for the opposite ordering. If you follow its bit packing literally you get the reversed (wrong) index.
- Recommendation: **index = (C12<<2) | (C13<<1) | C14** into Table 32 order.

## 9. Unicode sextants

- **Contiguous: U+1FB00–U+1FB3B** "BLOCK SEXTANT-…".
  - Let n be the 6-bit sextant value, with bit k−1 set for cell k. The cells are numbered 1 = top-left, 2 = top-right, 3 = middle-left, 4 = middle-right, 5 = bottom-left, 6 = bottom-right.
  - The code points cover n = 1..62 in ascending order, **skipping n = 21 (left half) and n = 42 (right half)**. Those use U+258C ▌ and U+2590 ▐. n = 0 is a space (Bedstead uses U+2003 for the em-width blank mosaic) and n = 63 is U+2588 █.
  - Formula: n ≤ 20 → U+1FB00 + n − 1; 22 ≤ n ≤ 41 → U+1FB00 + n − 2; 43 ≤ n ≤ 62 → U+1FB00 + n − 3.
  - Bedstead encodes the inverse as `n = i*21/20 + 1` for i = 0..59 (bedstead.c:523–530, with 0x15, 0x2A and 0x3F special-cased at 519–522).
- **Separated: U+1CE51–U+1CE8F** "SEPARATED BLOCK SEXTANT-1" … "SEPARATED BLOCK SEXTANT-123456", Unicode 16.0 Symbols for Legacy Computing Supplement.
  - This is a straight mapping, **code point = U+1CE50 + n** for n = 1..63. There are no gaps: the half blocks and the full block have their own separated forms. Separated n = 0 is just a space.
  - Bedstead SEP6 table: `M(u)` gives U(1CExx) with value xx − 0x50, for xx = 51..8F (bedstead.c:533–541).
  - Verified against UnicodeData 16.0: 1CE51 = SEPARATED BLOCK SEXTANT-1 and 1CE8F = SEPARATED BLOCK SEXTANT-123456. 1CE50 is a Large Type piece and 1CE90 starts the one-sixteenth blocks.
- Separated quadrants (for reference) are U+1CC21–U+1CC2F.
- **Teletext byte to n:** n = (c & 0x1F) | ((c & 0x40) >> 1), for mosaic codes c where bit 0x20 is set.
