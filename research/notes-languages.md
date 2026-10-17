# Teletext beyond the standard's character sets: sources and findings

Research of 18 October 2026, behind decisions E.77 to E.82 and `docs/spec/design/languages.md`. Every source is in `~/devel/teletext-reference/`, whose folders' `SOURCES.json` give each file's address, size, checksum and licence. Page numbers are PDF pages unless marked "printed". "Inferred" marks a reading the source does not state in words.

---------------------------------------------------------------------------------------------------

## 1. The standards

### 1.1 The 1990 World System Teletext specification

File: `teletext-standards/Teletext1990-blunham.pdf` (WST90 below).

- **A register of character-set codes (p. 144, Figure 14).** It is separate from today's Table 32. X/28 assigns these 7-bit codes to the G0 to G3 tables, with up to 8 header-bit options per G0 set (p. 44). The entries include:
  - Arabic 10/11, Devanagari 14/15, Hangul 18/19, Hebrew 20/21, Burmese 22/23, Lao 24/25;
  - Malayalam 26/27, Sinhalese 28/29, Thai 30/31, Katakana 32/33, Hiragana 34/35;
  - Chinese 36, Kanji 37;
  - Tamil 43/44, Assamese and Bengali 45/46, Gujarati 47/48, Punjabi 49/50, Telugu 51/52, Oriya 53/54.
- **Code tables for the Asian scripts:**
  - Thai G0 (Figure 31, p. 171): Thai digits in column 3, consonants and vowels in columns 4, 6 and 7, vowel signs and tone marks in column 5, and the block at 7/F. "Column 5 characters are non-spacing when transmitted in a packet with Y=26."
  - Burmese and Lao G0 sets (Figures 32 and 33, pp. 172–173).
  - Devanagari G0 and G2 sets (Figures 34 and 35, pp. 174–175). G0 column 5 holds half-width left and right vowel signs and half-width spaces, with the left halves of conjuncts at 5/8 to 5/F. G2 column 2 holds the vowel signs, the halant and a modifier.
  - Tamil, Bengali, Gujarati, Punjabi, Telugu, Malayalam, Sinhalese and Oriya follow (pp. 177–195).
- **"Character Set Extension – Sanscrit Derived Languages"** (§14.7, pp. 39–41) adds two X/26 formats:
  - Each 18-bit group is two 9-bit words, each a 5-bit display address and a 4-bit diacritical mark "from the appropriate column of the in use G2 code table". Address values 1 to 23 name a row, 0 names row 24, and 24 to 31 step forward by 1 to 8 characters. A NUL mark extends the step.
  - **Format B:** marks above or below a character only.
  - **Format C:** marks left, right, above and below, and half-width G0 characters.
  - Inferred: since a step is at least one character, Format B gives no plain way to put two marks on one letter.
- **Two-byte ideographs downloaded through pseudo-pages** (§14.12, pp. 48–50).

### 1.2 EBU SPB 492 (1992) and ETS/EN 300 706

- **SPB 492** (`teletext-standards/Teletext1992-blunham.pdf`) keeps the register but marks the Asian entries "Reserved" (p. 116): "Entries identified as 'Reserved' have been allocated to other character sets or writing systems." Their coding moves to EBU Supplements 1 to 3 (p. 7, references [3] to [5]; p. 11). Hebrew appears in this register (pp. 116, 125).
- **ETS 300 706 (1997)** replaces the register with Tables 32 to 34 (pp. 109–113):
  - groups 0101, 0111 and 1001 are explicit "Reserved" rows, every other unlisted group is reserved under "Other values", and Note 2 reserves any blank entry;
  - Formats B and C are gone.
- **EN 300 706 (2003)** changes no allocation from ETS 300 706. It adds the "@" rule for mode 10000 with data 2/A (p. 88), and deletes a copied note from Arabic G2 that said its column 4 held marks (ETS p. 124).
- **Diacritical marks come from the current G2 set.** EN 300 706 Table 29, modes 10000 to 11111 (p. 88): the mark comes "from column 4 of the G2 supplementary character set", meaning the default set, X/28 or M/29, or the latest Modified Designation triplet.
  - Latin, Cyrillic and Greek G2 have the same marks in column 4 (pp. 108, 112, 114).
  - Arabic G2 column 4 holds Latin letters (p. 116), so Hebrew pages, which use Arabic G2, have no marks.
- **Annex E.10 (informative, p. 136):** the diacritic technique "should be restricted to the Latin character sets … Decoders are unlikely to be able to handle other G0 sets in this way."
- **Level 1.5's G2 repertoire** "is not fixed and is determined by the language requirements in the country of use" (p. 88, Note 5).

### 1.3 EBU Tech 3232 (1982) and other texts

- **EBU Tech 3232**, *Displayable Character Sets for Broadcast Teletext* (`teletext-standards/ebu-tech-3232/`):
  - New repertoires should give as few characters as possible different codes, so that foreign decoders still show readable text (pp. 8, 11).
  - For Arabic it proposes sending one form of each letter and letting decoder logic choose the joined form from the preceding character, in rows sent in visual order; composition equipment handles the right-to-left order (p. 12).
  - Arabic-Indic digits are a rendering of the codes 0 to 9 (p. 12; appendices p. 6).
  - Hebrew's direction "has no influence on the display requirements" (p. 13).
- **CCITT T.101 Data Syntax II** has an Arabic set of isolated forms with "contextual shape determination", with or without the lam-alef ligature (SAPV 13 or 14), and a choice of digit shapes (SAPV 1 or 2) (`teletext-standards/itu_t_t101.pdf` pp. 54, 59).
- **ITU-R BT.653-3** note 13 (p. 22): System B covers "12 syllabic writing systems in use in the Indian sub-continent and adjacent areas", and two-byte Kanji, Kana and Hangul. ISO 2375 registration applies to System C and T.101, not to System B (p. 15, note 15).
- **ETSI TR 100 287**, the Enhanced Teletext code of practice (`captions/teletext-subtitles/etsi/`), clause 10.3 (p. 17):
  - C12 to C14 alone cannot identify a language (000 is both English and Polish), so services should send X/28/0 or M/29/0;
  - a service that mixes alphabets should give each language its own magazines.
- **EBU Tech 3264** (STL subtitle files, `captions/exchange-formats/ebu/`, p. 7): each non-Latin code table pairs its script with Latin (ISO 8859-5 to 8859-8). It has no Thai or Indic table.
- **ITU-R Report BT.2342-3** (2019, `captions/japan/itu/`) says nothing about Thai, Indic or Arabic in teletext. It recommends Unicode and UTF-8 for new caption formats.

## 2. Thailand

### 2.1 The Philips SAA5246AP/S

File: Philips, *Integrated VIP and Teletext (IVT1.0), SAA5246A*, product specification, January 1993 (`teletext-fonts/manufacturers/philips/datasheets/saa5246a/PHILIPS-SAA5246A-INTEGRATED-VIP-TELETEXT-(IVT1.0)-HANDBOOK.PDF`).

- **Table 13 (p. 35), "SAA5246AP/S character data input decoding, English and Thai languages":**
  - the Thai glyphs are in columns 8, 9 and 12 to 15;
  - control code 1/B is labelled TWIST;
  - 0/E and 0/F are SO and SI, marked "reserved for compatibility with other data codes".
- **Tables 22 and 23 (pp. 45–46):** the national option sets.
  - Table 22 is an English page (C12 to C14 = 000) without TWIST, or a Thai page (010) with TWIST.
  - Table 23 is a Thai page without TWIST, or an English page with TWIST. On it, columns 2 and 3 are punctuation and digits, columns 4 to 7 are the Thai letters, and 7/F is the block.
  - "Other combinations default to English."
- **Glyph placement**, read from the tables at 600 dpi:
  - upper vowel signs, tone marks and their combinations are drawn at the bottom of the cell;
  - the lower vowel signs ุ ู are drawn at the top.
- **The 32 combination glyphs** are in columns 8 and 9: ์ ๊ ๋ ํ, each upper vowel sign combined with each tone mark, and the tone marks combined with ํ. Codes 8/x and 9/x come from the chip's "7 bits with parity checking or 8-bit mode" setting (register text, p. 23), so they need 8-bit data. Inferred: they cannot be reached in 7-bit transmission.
- **The transcription** of the Thai G0 and G2 sets is in `docs/spec/design/languages.md` sections 7.3 and 7.4. Seven glyphs are uncertain.
- **Other variants** in this specification: AP/E (West European), AP/H (East European), AP/T (West European and Turkish), AP/J (Yugoslav), AP/K (French and Arabic), AP/L (Arabic and Hebrew), AP/I (Greek and Turkish). AP/S is the only one with a script outside EN 300 706.

### 2.2 ARMTEXT and the Chulalongkorn theses

Folder: `captions/thailand/teletext/`. All three theses are image-only scans.

- **Sayan Theerapanyawat, 1993,** *Development of a teletext decoder with Thai/English fonts* (MEng):
  - **ARMTEXT** was run by Tele-Information Co. with Army TV Channel 5: about 700 pages in 7 magazines, at first in English only, with the Thai index at page 200 (ch1 pp. 2–5; ch6 p. 2).
  - **The decoder** uses an SAA5231 and SAA5243 for acquisition only. An 80C652 reads the page over I²C and adds 0x80 (in the listing, 0x83) to each Thai byte. A 6845 drives a 2764 EPROM font of 8 × 10 cells, with no overlay logic (ch4 pp. 23–24; ch5 pp. 4–5; appendix pp. 68, 76–79).
  - **Thai signalling:** 0/A in row 1, column 0 marks a Thai page, and ESC toggles between Thai and English, each row starting in Thai. The national option bits are not used (ch4 p. 10).
  - **Fig. 6.5 (ch6 p. 4)** photographs ARMTEXT page 200 on 15 February 1993. Thai letters are set close together, marks sit directly above their consonants (ี้ in นี้), and lines are about two rows apart. Inferred: the marks travelled on rows of their own.
- **Wanchaleam Pora, 1995,** *A VHDL model of a teletext IC*: the same signalling (THAIPAGE 0x8A, TWISTCODE 0x9B); a "TLI code" for Thai at 0xA0 to 0xFF; 12-dot cells of 10 lines; and ARMTEXT described as "the only teletext service in Thailand" (ch3 p. 1; ch4 pp. 9–12, 15, 41).
- **Chanarong Angthong, 1996,** *Development of a teletext inserter using VHDL*: it reproduces the SAA5246AP/S English and Thai table (ch2 p. 9) and says Thai "cannot be encoded to match the decoder chip's codes as easily as English" (ch7 p. 2).
- **Raynu Rattanaprasert, 1996** (`captions/thailand/other-mentions/`): in a 1995 survey, 8 of 200 stock investors took a teletext data service carrying real-time prices and news (abstract; ch4 p. 25).

## 3. China: CCST

Files: `captions/china/teletext/`.

- **GB/T 14219-1993** (CCST) uses CCIR Rec. 653 System B transport: 6.9375 Mbit/s, framing code 27H, 42-byte packets (pp. 2, 4–6). Within it:
  - **Header bits.** C12 to C14 = 000 means the Western set G0s; 111 means the two-byte Chinese set G0d, "currently GB 2312"; 001 to 110 are reserved (p. 8).
  - **Shift codes.** 0/E shifts to Western text and 0/F to Chinese, both set-at (p. 10; Fig. 12 p. 15).
  - **Layout.** A character is anchored at any column of rows 1 to 22 and covers two columns of three rows; 7F7E fills its place in the two rows below (p. 10). That gives 20 characters per row and 8 Chinese rows per page, each character a 24 × 24 glyph in a 24 × 30 area, from the GB 5007 font (pp. 2, 9).
  - **Packets.** X/25 is vertical odd parity for correction (p. 17). X/26 column mode 04H invokes a character set, and bit b3 of mode 0CH sets half brightness (pp. 16–17). X/27/4 to X/27/7 link DRCS pseudo-pages. Two-byte DRCS codes begin 7F (pp. 18–19, 24).
- **Philips US 5,221,968 (1993)** describes ideograms of 2 columns × 3 rows with spacing "Shift To Latin" and "Shift To Ideograms" attributes and C12 to C14 as the page default. It cites a two-byte set in "FIG. 55" of the December 1987 WST specification (pp. 1–2, 5–6).
- **GB 2 051 527 A (1981)** proposes 16 × 16 glyphs from a 1,014-character ROM on a 14-row page, with downloaded bitmaps in rows 14 to 23 (pp. 13–15).
- **NXP SAA7154** slices "European teletext (WST 625) and Chinese teletext (CCST 625)" as one data type (p. 29, Table 6).

## 4. Korea, India and other countries

- **Korea** (`teletext-standards/south-korea/`) used NABTS (System C) with NAPLPS, never WST (1988 Kim p. 2, p. 6; 1984 Kang p. 7).
  - Hangul was a set of letters designated by ISO 2022 final byte 4B and sent three codes per syllable (1987 Lee p. 2).
  - The receiver built each syllable from 51 letters stored as 204 position-dependent patterns (1988 Kim p. 6), drawn as 16 × 20 dots (1987 Lee p. 2).
  - Sending the letters as DRCS was rejected for its 25.6 s delay (1988 Kim p. 6).
- **India** (`captions/india/`): INTEXT, run by NIC and Doordarshan in Delhi from 14 November 1985, was Antiope (System A) from Sofratev, and carried English-language data.
  - About 300 of 10,500 imported decoders were sold (Rajya Sabha questions of 1985 to 1993; BT.653-3 p. 28).
  - The only claim about Indian scripts is that Antiope's mosaics could draw them (Rajya Sabha SQ 103, 2 May 1989, p. 1).
  - A plan for "Teletext Service in all regional languages" (Estimates Committee, 82nd report, p. 158) was never carried out.
- **Russia:** GOST R 50861-96 (`teletext-standards/russia/`) uses Levels 1 and 1.5, System B (p. 3).
  - Its only character table (Fig. 8, p. 12) is EN 300 706 Table 39, with no Ё; neither the test page alphabet (p. 22) nor the table has it.
  - Russian and English subtitles are on separate pages, 888 and 889 (p. 13).
- **Spain:** the Orden of 24 November 1986 (`teletext-standards/spain/`), re-enacted in Real Decreto 1160/1989:
  - 128 characters so that all official languages can be written (p. 2), with C12 to C14 = 101 (p. 3);
  - 32 extra characters through X/26 diacritical marks, G2 and G3 (Fig. 10, p. 9; pp. 4–5);
  - row bytes under X/26 overwrites sent with even parity, unlike EN 300 706's stand-ins (p. 4, §14).
- **Cyrillic layouts** (`teletext-standards/iso-ir/`):
  - JUS I.B1.003 (ISO-IR 146) and JUS I.B1.004 (ISO-IR 147) lay Serbian and Macedonian Cyrillic out differently from EN 300 706 Table 38. JUS has lowercase џ at 7/8 and Ѕ ѕ at 5/9 and 7/9.
  - Table 38 is the Cyrillic form of the teletext Serbian, Croatian and Slovenian Latin option (Table 36): Ч Ћ Ж Ђ Ш at the codes of Č Ć Ž Đ Š.
- **Malaysia, Singapore, South Africa, Brazil, Colombia, Ecuador and Belgium:** no character-set material. Singapore's 1983 pilot was English on both channels, and no Jawi, Tamil or Chinese teletext scheme was found.

## 5. What the design takes from these findings

Recorded in `docs/spec/design/languages.md` and decisions E.78 to E.81:
- **Thai** follows the SAA5246AP/S. WST90's Thai set and Format B are the recorded alternative.
- **Lao** keeps WST90 Figure 33's codes with the chip's layout.
- **Arabic-script letters are joined by the receiver,** as EBU Tech 3232 proposed, and digits keep the codes 3/0 to 3/9.
- **Vietnamese is a Latin national option** (Annex E.10).
- **The Serbian and Macedonian sets build on Table 38,** and the Ukrainian set on Table 40.
- **The codes CCST uses are kept free** for a later CCST profile.
- **Indic, Burmese, Korean and Japanese stay out of scope,** with the reasons above.
