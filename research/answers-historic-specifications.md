# Findings from historic teletext specifications

Citation convention: "PDF pN" is the page index in the PDF file. Where the scanned page carries its own printed page number it is given as "printed N". Text was OCR'd with `pdftotext -layout`. Key tables (1976 Tables 2/3, 1976 Fig. 6, 1974 Table 5, 1990 Fig. 5, 1992 Fig. 17) were checked visually against the page images.

## 0. Document identification

| File | Title | Issuing body | Date / edition |
|---|---|---|---|
| TeletextSpec1974.pdf (38 pp) | "Specification of standards for information transmission by digitally coded signals in the field-blanking interval of 625-line television systems" | Joint BBC / IBA / BREMA (Joint Executive Committee, Combined Working Group) | October 1974, first edition. Graphics are in an Addendum. |
| TeletextSpec1976.pdf (22 pp) | "Broadcast Teletext Specification" | Joint BBC / IBA / BREMA, ISBN 0 563 17261 4 | September 1976, reprinted with typographical corrections October 1976. Supersedes the 1974 spec and an interim spec of 13 January 1976 (Foreword, PDF p3). |
| Teletext1990.pdf (243 pp) | "World System Teletext and Data Broadcasting System (CCIR Teletext System B) – Technical Specification" | Industry/UK WST document. The cover gives the contact as B J Rogers, Woodford Halse (possibly DTI/BREMA-circulated; issuing body uncertain). Scan by Colin Hinson. | February 1990. Two-column tabular format: 625-line and 525-line. |
| Teletext1992.pdf (148 pp) | "Teletext Specification (625 line television systems)", Interim Technical Document **SPB 492** | European Broadcasting Union, Geneva | December 1992. This is the document that EN 300 706 Annex O says it replaces (together with EN 300 708). |
| TV625lineUKspec.pdf (44 pp) | "Specification of Television Standards for 625-Line System I Transmissions in the United Kingdom" | DTI Radio Regulatory Division / Radiocommunications Agency (prepared by UK CCIR SG11 Sub-Group 11A) | 1984, reprinted and amended 1990 |

The 1992 SPB 492 is essentially a re-typeset and updated 1990 WST text. The section numbering is largely shared, with 625-line material only.

---

## 1. Character Set Code values (X/28/1, M/29/1)

**Source of the "earlier specification":** SPB 492 §14.8 and §14.9.4 (1992 PDF p38–39, printed 28–29), and WST 1990 §14.9 (PDF p44).

**Packet layout (SPB 492 §14.8.3.1; 1990 §14.9.3.1):**
- Designation code is 0001. Only the first **two** triplets are used; the rest are reserved.
- Triplet 1:
  - bits 1–2 = 0
  - bits 3–9 = **Character Set Code of G0**
  - bit 10 = 0
  - bits 11–17 = **Character Set Code of G1**
  - bit 18 = 0
- **Triplet 2 has the same layout, but bits 3–9 designate G2 and bits 11–17 designate G3.**
- Bits are transmitted LSB first.
- "Up to 8 options may be designated with the G0 set and invoked by C12, C13, C14."
- Some codes allow more than one G0 set to be designated at once, provided the total number of options does not exceed 8.
- "Equipment intended for operation with only a single group of character sets may ignore this data."
- M/29/1 (§14.9.4, 1992 PDF p39) uses the same coding (designation code 0001) for all pages in the magazine. A page-level packet takes precedence (§14.9, PDF p39).

**Conflict with EN 300 706:** EN 300 706 Table 8 (X/28/1) and Table 12 (M/29/1) keep only triplet 1 (G0/G1) for backward compatibility and reuse **triplet 2 bits 1–18 for DCLUT4 (global 12x10x2 DRCS)**. ETS 300 706 (1997) Table 8 is identical. A legacy SPB 492 decoder would therefore read DCLUT4 data in triplet 2 as G2/G3 designation. A new Level 1/1.5 decoder should read only triplet 1 bits 3–9 and 11–17.

**Figure 14 "Designation of Character Sets" (1992 PDF p116, printed 106).** The 7-bit code is the set number in binary.

| Set | Code | Name (SPB 492, 1992) | Code-table figure |
|---|---|---|---|
| 0–5 | 0000000–0000101 | Reserved for other applications (may not designate a set) | – |
| 6 | 0000110 | 1st Latin Primary Set | 15 & 17 |
| 7 | 0000111 | 1st Latin Supplementary Set | 16 |
| 8 | 0001000 | Block Mosaic Set | 18 |
| 9 | 0001001 | Smoothed Mosaic Set | 19 |
| 10 | 0001010 | Arabic Primary | 21 |
| 11 | 0001011 | Arabic Supplementary | 21 |
| 12 | 0001100 | Cyrillic Primary | 23, 25 & 26 |
| 13 | 0001101 | Cyrillic Supplementary | 24 |
| 14–15 | | Reserved (1990: Devanagari primary/supplementary) | |
| 16 | 0010000 | Greek Primary | 27 |
| 17 | 0010001 | Greek Supplementary | 28 |
| 18–19 | | Reserved (1990: Hangul single-byte / two-byte) | |
| 20 | 0010100 | Hebrew Primary | 22 |
| 21 | 0010101 | Hebrew Supplementary | |
| 22–37 | | Reserved (1990: Burmese, Lao, Malayalam, Singhalese, Thai, Katakana, Hiragana, Chinese 36, Japanese Kanji 37) | |
| 38 | 0100110 | 2nd Latin Primary | 15 & 17 |
| 39 | 0100111 | 2nd Latin Supplementary | |
| 40 | 0101000 | 3rd Latin Primary | 15 & 17 |
| 41 | 0101001 | 3rd Latin Supplementary | |
| 42 | 0101010 | Cyrillic and Latin (associated) Primary sets | 26 |
| 43–54 | | Reserved (1990: Tamil, Assamese, Gujerati, Punjabi, Telegu, Oriya) | |
| 55 | 0110111 | 4th Latin Primary | 15 & 17 |
| 56 | 0111000 | 4th Latin Supplementary | |
| 57–69 | | Reserved | |
| 70 | 1000110 | "Yugoslav (& other former states)" Latin Primary | |
| 71 | 1000111 | Same, Latin Supplementary | |
| 72–75 | | Reserved for other applications | |
| 76 | 1001100 | Yugoslav Cyrillic Primary | |
| 77 | 1001101 | Yugoslav Cyrillic Supplementary | |
| 78–127 | | Reserved for other applications | |

Notes on Figure 14:
- The figure itself says: "Set 70 is identical to Set 6 and Set 71 is identical to Set 7." This contradicts Figure 17 part 2, where set 70 has different options (see below). **Treat set 70 as uncertain.**
- The 1990 Figure 14 (1990 PDF p144) has the same numbering. It fills most reserved slots with Asian scripts, and codes 70–77 are absent.

**National options per primary set, selected by C12 C13 C14.** Source: 1992 Figure 17 part 2 (PDF p120, printed 110). Option n corresponds to bits C12 C13 C14 with **C12 as MSB**.

| C12 C13 C14 | Set 6 | Set 38 | Set 55 | Set 70 |
|---|---|---|---|---|
| 000 | English | Polish | English | – |
| 001 | German | German | German | Albanian |
| 010 | Swedish/Finnish | Swedish/Finnish | Swedish/Finnish | Hungarian |
| 011 | Italian | Italian | Italian | – |
| 100 | French | French | French | Slovakian |
| 101 | Portuguese/Spanish | – | Portuguese/Spanish | – |
| 110 | Czech/Slovak | Czech/Slovak | Turkish | Serbian/Croatian/Slovenian |
| 111 | Reserved | Reserved | Reserved | Rumanian |

The 1990 equivalent is Figure 3(b) (1990 PDF p129), with columns for sets 6, 38, 40 and 55:
- **Set 40** (3rd Latin) = English(US), German, reserved, Italian, French, Portuguese/Spanish, reserved, reserved.
- **Set 38, option 6 (101)** = "Serbo-Croat" in 1990 (blank in 1992).

**Mapping to EN 300 706 Table 32.** EN 300 706 Table 32 group bits 14–11:
- 0000 matches set 6 (EN adds Hungarian to option 010).
- 0001 matches set 38 (Polish variant).
- 0010 matches set 55 (Turkish variant).
- 0011 roughly matches set 70 (Serbian/Croatian/Slovenian, Rumanian).

This mapping is my inference and is **not stated** in any of these documents.

Practical recommendation for X/28/1 and M/29/1 G0 codes:
- **Map 6, 38, 55 and 70** to the corresponding EN 300 706 G0 groups.
- **Map 12, 16, 10 and 20** to Cyrillic, Greek, Arabic and Hebrew.
- **Ignore codes 0–5 and 72–127.**
- For G1, expect code 8 (block mosaic).

---

## 2. Hold Graphics in the 1976 spec vs EN 300 706 Table 26

**1976 §3.1.7 "Hold Graphics"** (PDF p10, printed 8), plus the definitions on PDF p11 (printed 9):
- **Which character:** "the most recent character with b6=1 in its character code". It is "only defined during the Graphics Mode". Blast-through characters (columns 4/5, where b6=0) are therefore never held. A graphics-mode SPACE (2/0) has b6=1 and so does become the (blank) held character. That last point is implied, not stated.
- **Form:** "This character is to be displayed in the Contiguous or Separated Mode as when it was first displayed." Contiguous/Separated "forms part of the structure of the Held Graphics Character". All other modes (colour, flash, conceal, height, box) are those of the rectangle where the held character is displayed.
- **Reset:** the held character is valid only "providing that there has been no intervening change in either the Alphanumerics/Graphics or the Normal/Double Height modes". In the absence of such a character, the held character is a Space. It is implicitly reset at row start, because all modes reset at row start (§3.1, PDF p9).
- **Where substituted:** in "the rectangle corresponding to any Control Character occurring during the Graphics Mode" while in Hold mode.
- **Set-at/after:** Hold 1/14 is **Set-At** and Release 1/15 is **Set-After** (Table 2, PDF p21, printed 19).

**WST 1990 §11.5.6** (PDF p16) has similar wording, but says "most recently transmitted before the 'hold mosaic' control". Read literally, this means the held character is the one before the 1/E code, which is wrong. The reset condition is "no intervening change of display mode, normal/double height or alphanumeric/mosaic". The "display mode" phrase is ambiguous.

**SPB 492 §11.5.6** (1992 PDF p18, printed 8) corrects this to "most recently transmitted before the control character in the 'held-mosaic' position". The reset wording is the same as 1990. Both say "displayed in the original contiguous or separated form".

**Differences from EN 300 706 Table 26 (1/E):**
- EN adds explicitly:
  - "not reset by reinforcement of the existing size setting"
  - "not reset by a change in Hold Mosaics mode"
  - at Level 2.5/3.5, enhancement size/charset changes do not cancel hold
- The 1976 spec says nothing about hold toggling. Its phrase "change in ... Normal/Double Height modes" is consistent with EN's reinforcement rule, because in 1976 terms a repeated 0/12 or 0/13 is not a *change* of mode (§3.1 "a change to a mode already obtaining ... continues uninterrupted").
- Double width and double size did not exist in 1976. EN's "change of size" generalises the rule.
- Otherwise the three specs agree:
  - the held character is the most recent b6=1 mosaic on the row
  - it keeps its original contiguous/separated form
  - it is reset by an alpha↔mosaic change or a size change
  - substitution happens only in mosaic mode

Edge case: alpha colour codes are Set-After. A 0/x code met in graphics+hold mode is therefore itself displayed as the held character, and the mode change happens after it. This is consistent across all versions.

---

## 3. Set-at / set-after and related rules

**1976 general rule** (§3.1, PDF p9, printed 7):
- "Some Control Characters have immediate effect ('set at') ... others have subsequent effect ('set after')."
- "A later Control Character takes precedence over an earlier one."
- "When a Control Character signifies a change to a mode already obtaining, that mode generally continues uninterrupted throughout the corresponding Character Rectangle (but see 3.1.3)." The exception is New Background: 1/13 always adopts the current display colour.
- Control characters are normally displayed as spaces.

**1976 Table 2** (PDF p21, printed 19), checked on the image:

| Mode | Set-At | Set-After |
|---|---|---|
| Alphanumerics + colour | Row start (0/7 white) | 0/1–0/7 |
| Graphics + colour | – | 1/1–1/7 |
| Contiguous | Row start, **1/9\*** | 1/9\* |
| Separated | **1/10\*** | 1/10\* |
| Black background | Row start, 1/12 | – |
| New background | 1/13\*\* | – |
| Reveal | Row start / user control\*\*\* | 0/1–0/7, 1/1–1/7 (any colour code ends conceal) |
| Conceal | 1/8 | – |
| Steady | Row start, 0/9 | – |
| Flash | – | 0/8 |
| Unboxed / End box | Row start, 0/10\*\*\*\* | 0/10\*\*\*\* |
| Boxed / Start box | 0/11\*\*\*\* | 0/11\*\*\*\* |
| Normal height | Row start, 0/12 | – |
| Double height | – | 0/13 |
| Release | Row start | 1/15 |
| Hold | 1/14 | – |

Table 2 footnotes:
- \* "these codes may take effect 'at' or 'after' their occurrence". **Contiguous/Separated timing was deliberately left to the decoder in 1976.** EN 300 706, WST 1990 and SPB 492 all fix them as Set-At.
- \*\*\*\* "two consecutive codes are transmitted, the mode changes between them".
- 1976 has **no alpha black or graphics black**. 0/0 NUL, 1/0 DLE, 0/14 SO, 0/15 SI and 1/11 ESC are "reserved for compatibility with other data codes" (Table 3, PDF p22).
- Codes presumed at row start (Table 3 note 2) are 0/7, 0/9, 0/10, 0/12, 1/9, 1/12 and 1/15.

**Later tables:**
- **WST 1990 Fig. 5** (PDF p131, checked on the image):
  - Set-At codes: Conceal, Steady, **End Box**, Contiguous, Separated, Normal Height, Black Background, New Background, Hold.
  - Only Start Box carries the "two consecutive codes, action between them" note.
  - 1/11 is "Reserved". 0/0 and 1/0 (black) and 0/14 and 0/15 (double width and size) have "no action at level 1".
- **SPB 492 Fig. 20** (1992 PDF p123, printed 113):
  - Same as 1990, except **End Box is "(1,3)"**: presumed at row start, plus the double-transmission note. It is no longer Set-At.
  - 1/11 is used for data for processing and "in some countries for language switching" (the ESC/switch origin).
- **EN 300 706:** End Box and Start Box are both Set-After, with the note that action takes place between the double codes. **1990 WST is the outlier in marking End Box as Set-At.**

**Start Box double transmission:**
- 1974 §2.16 (PDF p7, printed 9) and App. VI (PDF p30, printed 32):
  - Box and end-box codes are each transmitted twice, and the box opens between the pair of Start Box codes.
  - It closes "either after the first of a pair of 'End Box' control characters or on reaching the end of a row".
  - Maximum box width is 39 characters ("advisably a maximum of 37 displayed").
- 1976 §3.1.5 (PDF p9): "two consecutive Start Box ... to start ... two consecutive End Box ... to terminate ... The mode changes occur between the corresponding consecutive Character Rectangles."
- A **single** isolated Start Box is not required to act, by implication.

**Double height:**
- **1976 §3.1.6** (PDF p9–10):
  - The row with double height defines both row R and row R+1. A responding receiver "must ignore any information received for the Row of next higher address".
  - In R+1, the lower halves of the double-height characters are shown. "The remaining Character Rectangles of Row R+1 are to be displayed as unboxed Spaces in the same Background Colour as the corresponding Character Rectangles of Row R."
  - Note: **unboxed**. On a newsflash or subtitle page, the non-double-height parts of R+1 are therefore not inset. EN says "same local background colour and no foreground data" and does not mention boxing.
- The 1976 spec has no row 23/24 restriction; the page is only rows 0–23.
- **SPB 492 §14.5.2** (1992 PDF p29–30, Level 2):
  - Double height and double size are inactive on the bottom row of the display area and on the bottom row of a scrolling region.
  - Double width and double size are inactive in the last column.
  - Attributes at obscured positions do not take effect if they would break these rules.
  - One size attribute terminates any other.
  - At Level 2, data addressed to row R+1 *is* processed. In the absence of such data, the attributes of the previous row are adopted.

**Conceal:**
- 1976 §3.1.4 (PDF p9): concealed characters are shown as spaces "until the Reveal mode is restored after a time delay in the receiver or by user control". A **timed auto-reveal** is explicitly contemplated.
- Table 2 footnote \*\*\*: "the Reveal mode may be maintained throughout a page by a user control".
- Reveal (end of conceal) is triggered by any alpha or graphics colour code, Set-After.

**Flash:**
- 1976 gives no rate: "alternately ... and as Spaces, under the control of a timing device in the receiver" (§3.1.4, PDF p9). WST 1990 §11.5.8 and SPB 492 §11.5.8 say the same.
- The only rate figures are in SPB 492 §14.6.8, the X/26 "Additional Flash Functions" (1992 PDF p35, printed 25):
  - 000 = slow rate 1 Hz
  - 001, 010, 011 = fast rate 2 Hz, phases 1–3
  - 100 and 101 = incremental and decremental 2 Hz
  - Flash modes: steady, normal, invert phase, or flash to the next colour table.
- In 1976, flash alternates the whole character with a Space. EN changes this to "foreground pixels alternate between foreground and background colours", which looks the same.

---

## 4. National option characters and C12–C14

**1974** (Table 5 "Character codes for data broadcasting", PDF p23, printed 25; §2.12, PDF p7):
- The character set is ISO-7 (BS 4730:1974), described as "ASCII with selected 'National usage' characters".
- UK national-use positions (note ②): 2/3 £, 4/0 @, 5/B [, 5/C \, 5/D ], 5/E ↑, 6/0 \` (grave), 7/B {, 7/C | (vertical bar), 7/D }, 7/E ‾ (overline). 7/F is DEL.
- The control code allocation was completely different from 1976 (Table 1, PDF p9, printed 11):
  - graphics colours 1/1–1/7
  - **alpha colours 1/9–1/15**
  - Flash 0/12, Steady 0/13, End Box 0/14, Start Box 0/15
  - ETX 0/3 reserved for broadcasters
- There was no hold, conceal, double height or background colour.
- C7–C14 were "Unallocated" (Table 2, PDF p10).
- Page 99 was reserved for engineering tests (§2.10 footnote, PDF p6, printed 8).

**1976 Table 3** (PDF p22, printed 20, read from the image). This is the fixed English set, with no options:

| Pos | Glyph | Pos | Glyph |
|---|---|---|---|
| 2/3 | £ | 5/F | # |
| 2/4 | $ | 6/0 | ― (long dash, full-width bar) |
| 4/0 | @ | 7/B | ¼ |
| 5/B | ← | 7/C | ‖ (double vertical bar) |
| 5/C | ½ | 7/D | ¾ |
| 5/D | → | 7/E | ÷ |
| 5/E | ↑ | 7/F | ■ (solid block) |

Other notes on 1976 Table 3:
- 2/7 is drawn as a right single quote/apostrophe.
- 7/F appears as a filled rectangle inset in the cell. SPB 492 Fig. 15 note (1992 PDF p117): "7/15 occupies an area equivalent to that of any character which does not include a descender. It is thus a rectangle surrounded by the background colour."
- SPB 492 Fig. 17 part 1 (1992 PDF p119, printed 109) English column: identical to 1976 at 2/3 £, 2/4 $, 4/0 @, 5/11 ←, 5/12 ½, 5/13 →, 5/14 ↑, 5/15 #, 6/0 ―, 7/11 ¼, 7/12 ‖, 7/13 ¾, 7/14 ÷. EN 300 706 matches as well.
- National option positions in SPB 492 are exactly 13: 2/3, 2/4, 4/0, 5/B–5/F, 6/0, 7/B–7/E. 7/F is not included.
- SPB 492 Fig. 18 note (1992 PDF p121): G1 columns 4 and 5 "contain the corresponding characters from the G0 set". **Blast-through characters therefore follow the national option.**

**C12–C14 numbering and order:**
- **1976** Fig. 6 (PDF p16, printed 14, image checked):
  - Control Group B carries C11, C12, C13, C14 in transmission order, in message bits b2, b4, b6, b8 of the last header Hamming byte.
  - No binary weights are assigned.
  - §2.3.1 (PDF p8): "C12, C13, C14 Unallocated. It is desirable that these unallocated Control Bits be accessible in decoders for future use as display control functions."
- **WST 1990 §14.1.2** (PDF p27) and **SPB 492 §14.1.2** (1992 PDF p28, printed 18): the option number table is listed as "C12 C13 C14" with option 1 = 000 … option 8 = 111. **C12 is the MSB and C14 the LSB**, e.g. option 2 = German = C14 set.
- EN 300 706 Table 32 note 1 agrees: X/28/0 triplet 1 bit 10 = C12, bit 9 = C13, bit 8 = C14.
- **Implementation pitfall:** in header byte 13, the Hamming-decoded nibble holds (C11, C12, C13, C14) as data bits D1–D4, with LSB first. So nibble bit 1 = C12, bit 2 = C13, bit 3 = C14. **The option index must be bit-reversed:** `opt = (C12<<2)|(C13<<1)|C14`, not `nibble>>1`.

---

## 5. Decoder behaviour: errors, headers, magazine modes, time filling

**Hamming 8/4:**
- 1976 §1.2.3 and Tables 1a–1c (PDF p7 and p20, printed 5 and 18):
  - All four checks OK: accept.
  - A, B and C OK but D fails: the error is in b7 (a protection bit), so accept.
  - D OK but not all of A, B, C OK: multiple errors, so **reject message bits**.
  - Otherwise, correct the single error.
  - 3, 5, 7 or 8 errors decode to a false message.
- 1974 §2.17 (PDF p8) is the same: a double error is recognised as D̄·(A+B+C).
- SPB 492 App. 3 (1992 PDF p80–81) gives the parity equations for 8/4 and 24/18.
- None of the old specs say what to *display* for a rejected Hamming byte or a character parity error.

**Repeated rows and parity:**
- 1976 §2.2.2 (PDF p8): "Rows, including the Page-Header, may be repeated in which case the latest apparently error-free information should take precedence."
- This supports a policy of not overwriting a stored character with a parity-failed one, or at least preferring later good data.

**Framing code:** 1976 §1.2.2 and Fig. 9 (PDF p7): it is detected with any 7 of the 8 bits correct, which tolerates one bit error. EN 300 706 Annex P warns about this.

**Addresses:**
- Rows 24–31 "must be ignored" (1976 §2.1.1, PDF p7). In 1976 the page has 24 rows only.
- Page selection must not respond to units or tens values 10–15 (hex A–F), and time-code selection must not respond to hex minute or hour units (§2.1.4, PDF p7).
- The time code ranges are hours-tens 0–3 and minutes-tens 0–7, giving 3200 versions.

**Page termination (serial vs parallel):**
- **1976 §2.2.1** (PDF p7–8): a page begins with its header and "ends with, and excludes, the next Page-Header of the selected Magazine number". This is magazine-parallel termination, always. Rows from other magazines may be interleaved.
- C11 is defined only as a *display* hint: when set, "it is preferable to display all Page-Headers as Rolling Headers rather than only those of the selected Magazine" (§2.3.1, PDF p8).
- **WST 1990 §10.4** (PDF p13) and **SPB 492 §10.4** (1992 PDF p15, printed 5): the page is "terminated by and excludes the next Page Header Packet having the same magazine address but, including the sub-code, a different page address".
  - This is **different from EN 300 706**. In SPB 492 a header with the same page number but a different sub-code also terminates.
  - A header with an identical page number and sub-code does *not* terminate.
  - C11 (§11.1.8) means "Magazines are transmitted one at a time in sequence". No serial termination rule is given.
- EN 300 706 C11 defines serial mode as termination by *any* header with a different page number.

**Rolling headers and time:**
- 1976 definitions (PDF p11–12):
  - Rolling Headers are the top row showing all page headers of the selected magazine, or of all magazines when C11 is set.
  - C9 (Interrupted Sequence) "allows the Page-Header to be suppressed when Rolling Headers are displayed, to avoid discontinuities" (PDF p8).
  - The last 8 header characters are reserved for clock time, and the receiver may display them from rolling headers (Time Display, PDF p12).
- 1974 App. V (PDF p30, printed 32):
  - Taking the page number from every header makes it change every quarter-second. It is the designer's choice whether to take it only from the selected page.
  - To display seconds usefully, the time should be taken from every header.
- 1974 §2.5 (PDF p6): the page header may be repeated at the clock-seconds changeover to improve accuracy.

**Erase timing:**
- 1976 §2.2.3 (PDF p8): an active field period is allowed between an erase-page header and further rows. 1974 says the same: a row of the page will not follow its header in the same field-blanking interval.
- SPB 492 App. 2 (1992 PDF p80, printed 70): "Up to 20ms may be required to erase the decoder memory after the receipt of a page header". The same appendix notes attributes may be limited to 40 per row and DRCS memory to 16 kbit.

**C4 and C8 behaviour:** SPB 492 App. 6 (1992 PDF p94, printed 84).
- A retransmission overwrites the stored page. Packets that are not overwritten remain.
- C4 set means erase completely.
- C8 update may re-prompt or redisplay a cancelled newsflash.
- Both apply to packets 1–28.

**C5/C6 "mixed" mode:** SPB 492 §11.1.2–11.1.3 (1992 PDF p16) notes that "Decoders operating in 'mixed' mode also display non-boxed characters".

**Time-filling headers:** SPB 492 App. 5 (1992 PDF p93, printed 83).
- Page FF with sub-codes 0000–3F7E may be used "for time filling and as a terminator" and carries no data for acquisition.
- FF/3F7F is never transmitted in a header (the null link).
- FE/3F7E is the default Magazine Inventory Page address (§10.3, PDF p14).
- WST 1990 C4–C6 and page addressing are otherwise as above.

**Page 100 default:**
- SPB 492 App. 4 (FLOF code of practice), §E.1 (1992 PDF p84, printed 74): "No Packet 8/30: Multiplexed operation is assumed, with an initial page address 100, page sub-code being interpreted as 'don't care'."
- §D.1.3 (PDF p82): FLOF decoders "shall acquire the initial teletext page on switch-on or on changing channel".
- §E.2: with no X/27 and no X/24, the index key shows the initial page.
- 8/30 Format 1 is sent about once per second and the initial page is in bytes 7–12 (§13, 13.2.2, PDF p23–24).

**VBI lines:**
- The 1976 spec (PDF p6) uses lines 17/18 (330/331) initially, with other lines permitted.
- The 1984/1990 UK System I spec, App. III (PDF p37, p39), says teletext may use lines 7–22 and 320–335. It also gives BBC ITS-3 teletext test data on lines 20/333.
- WST 1990 item 1.1 (PDF p12) gives 625-line data lines as 6–22 and 318–335.

---

## 6. Level 1.5 / X/26 in the 1990 and 1992 documents

- **Neither document uses the term "Level 1.5".** EN 300 706 says Level 1.5 "has evolved in the field ... documented here for the first time" (EN §intro).
- In the old documents X/26 belongs to "Presentation Level Two": WST 1990 §14.6 (PDF p30–37) and SPB 492 §14.6 (1992 PDF p30–37, printed 20–27).

**Addressing (SPB 492 §14.6.4, 1992 PDF p31, printed 21; identical in 1990 PDF p31):**
- Address 0–39 is a column. Addresses 41–63 are rows 1–23, and 40 is row 24.
- **"A character-space is thus defined explicitly by a data group including a row address, followed by one or more character position data groups."**
- No restriction by mode is stated. This is the origin of the EN 300 706 Annex A.3 remark that Level 1.5 decoders take the row from *any* row-address triplet regardless of mode.
- Triplets must be sent in display order.
- **Row 0** (§14.6.4.1): use row-address mode 00111 with address 63. The following column triplets address row 0.
  - The usable column range is **"8 to 39" in SPB 492 but "0 to 39" in WST 1990**.
  - Addresses 40–62 with this mode are ignored.
  - Data 0 means no full-row colour. Data bits 6–7 = "01" means bits 1–5 give the row-0 full-row colour.
- The **Termination marker** (§14.6.11) has row address and mode all 1s, in bytes 40–42 of the last X/26. Unused groups are filled with copies of it.
- §14.6.12: X/26/15 carries a CRC over X/28 and X/26. EN A.3 notes some Level 1.5 decoders ignore X/26/15.
- Designation codes 0000–1111 are sequence labels for up to 16 X/26 packets (§14.6.2).

**Row-address modes** (SPB 492 Fig. 9, 1992 PDF p108, printed 98; 1990 Fig. 11, PDF p138):

| Mode | SPB 492 | EN 300 706 |
|---|---|---|
| 00000 | Full screen colour | same |
| 00001 | Full row colour. Data bits 6–7 = 00 for a single row, 11 for this row to the bottom | same |
| 00010, 00011 | 1992: Sanskrit-related languages. 1990: "redefines rest of packet" | reserved |
| 00100 | **Set Cursor Position.** Column is given by data codes **4/0–6/7** (0x40–0x67); other codes mean "no cursor" | **Set Active Position**, column 0–39 in data. **Different data coding** |
| 00101 | Start scroll region (data = region colour) | reserved |
| 00110 | End scroll region | reserved |
| 00111 | Address display row 0 | same |
| 01000–01101 | PDC data (called "VPT" in 1990) | same PDC set |
| 01110–11110 | not assigned | EN: object modes, origin modifier, DRCS mode |
| 11111 | Terminator | same |

**Column-address modes** (SPB 492 Fig. 10 part 1, PDF p109, printed 99; 1990 Fig. 12, PDF p139):

| Mode | SPB 492 | EN 300 706 |
|---|---|---|
| 00000 | Foreground colour | same |
| 00001 | G1 character (block mosaic default) | same |
| 00010 | G3 character (smoothed mosaic default) | same (Level 1.5) |
| 00011 | Background colour | same |
| 00100 | **Latching shift**: 0–3 = G0–G3, 4/5 = DRCS 1/2, 6–126 = invoke and latch set from Fig. 14, 127 = cancel | reserved |
| 00101 | **Single shift**, same data coding | reserved |
| 00110 | PDC | same |
| 00111 | Additional flash | same |
| 01000–01011 | **Modified designation of G0, G1, G2 and G3** respectively (data = Fig. 14 set code). Lasts to end of row, then reverts to X/28 or X/29 designation or the default | 01000 = modified G0/G2 designation, 01001 = G0 character without diacritic (Levels 2.5/3.5), 01011 = G3 character (Levels 2.5/3.5) |
| 01100 | Non-spacing display attributes: bit1 double height, bit2 box/window, bit3 conceal, bit4 marked area, bit5 invert, bit6 underline/separated, bit7 double width | similar Display Attributes triplet |
| 01101 | 1st DRCS | DRCS invocation |
| 01110 | 2nd DRCS | Font style |
| 01111 | G2 character | same |
| 10000–11111 | **G0 character plus diacritic from G2 column 4, rows 0–15 in ascending order**, so 10000 = G2 4/0 | 10000 = G0 without diacritic, 10001–11111 = diacritics 1–15 |

The 10000 difference is small in practice: Latin G2 4/0 is not a combining mark. **Mark uncertain.**

For a Level 1.5 decoder, the older documents support these conclusions:
- The row is taken from the most recent row-address triplet of any mode.
- Characters come from modes 00001, 00010, 01111 and 10000–11111.
- A legacy decoder might act on modes 00100/00101 (shifts) and 01000–01011 (modified designation).

---

## 7. Character rounding and display geometry

**1974 Addendum §2** (PDF p22, printed 24):
- A character rectangle is 10 or 11 lines per field (20 or 22 raster lines).
- Because this is not divisible by 3, the mosaic cell heights may be asymmetric. The example (Fig. 11) uses **10 lines split 3:4:3**.
- Separated or contiguous cells are both allowed, and "a preferred method will evolve".
- The mosaic bits are b1 = top left, b2 = top right, b3 = middle left, b4 = middle right, b5 = bottom left, b7 = bottom right (Fig. 10, PDF p24).

**1976:**
- "Each Row occupies about 20 television display lines" (definition of Row, PDF p11, printed 9).
- The shapes of alphanumeric characters "are not defined but they should all be different and recognisable" (PDF p10).
- The Separated Mode has "a Background Colour boundary around and between the six cells" (PDF p12). Fig. 8 (PDF p18) shows examples.
- The graphics sets hold 127 distinct shapes (64 contiguous and 63 separated, with the blank shared). The total repertoire is 222 display characters.

**SPB 492:**
- Separated mosaics: "each mosaic block is surrounded by a border of the background colour" (§11.5.5).
- 7/F is a descender-free-height rectangle surrounded by background (Fig. 15 note).
- G3 2/F means "activation of alternate dots of the matrix" (Fig. 19 note, 1992 PDF p122).

**No document here specifies character rounding (diagonal smoothing)**, a dot-matrix size (for example 5x9 or 12x10), or interlace handling for text. These are left to the decoder. The only interlace statement is the System I spec §2.4: 2:1 interlace, 575 active lines, 4:3 aspect (PDF p10).
