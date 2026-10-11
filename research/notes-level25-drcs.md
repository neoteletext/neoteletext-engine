# ETSI EN 300 706 V1.2.1 (2003-04): DRCS, the G3 set and the character repertoire at Levels 2.5 and 3.5

Compiled 2026-10-10. Source: `specifications/EN_300_706_v1.2.1_Enhanced_Teletext.pdf` (relative to `~/devel/microcontrollers/teletext/`). The printed ETSI page number equals the PDF page number; "p.N" means both. Tables and level ticks were read from the page images, not from a text extraction. Where the 1997 edition (`specifications/ETS_300_706_Enhanced_Teletext_1997.pdf`, "ETS 1997") was checked, it is said so.

Markers:
- **[V]** read directly in the standard at the cited place.
- **[D]** derived by arithmetic or by combining cited statements; the derivation is shown.
- **[U]** unverified: an inference, a reading of an unclear passage, or something the standard does not say. Each one also appears in section 15.
- **[E]** empirical: measured from the raster of a standard figure, or observed by running zvbi 0.2.44 on `oldvga`. The method is given.

Other sources:
- [L2-19-025] Unicode L2/19-025, "Proposal to add characters from legacy computers and teletext to the UCS", Terminals Working Group, 2019-01-04, https://www.unicode.org/L2/L2019/19025-terminals-prop.pdf, and its attached mapping file `TELTXTG3.TXT` ("Map from Teletext G3 character set to Unicode", Rebecca Bettencourt, 2018-04-20). The characters were encoded in Unicode 13.0 (block U+1FB00, "Symbols for Legacy Computing").
- [MRG], [VBIT2], [ZXED]: as defined in `notes-formats.md`. Only public documentation was read; no GPL source.
- [ZVBI-H] `/usr/include/libzvbi.h` on `oldvga`, package `libzvbi-dev` 0.2.44-1ubuntu2. Read, not copied.

Scope boundary: MOT and X/27/4/X/27/5 linking are in `notes-level25-objects.md` (sections 7 and 9); triplet field encodings, the colour map and CLUT handling are in `notes-level25-enhancement.md`; Level 1.5 G0/G2 glyph tables, Tables 32/33 and Hamming coding are in `notes-level15.md`. They are cited here, not repeated. Quotations are short; the rest is paraphrase.

Conventions:
- A triplet's 18 data bits are numbered 1 to 18, bit 1 least significant and first transmitted (cl. 8.3 p.21; cl. 12.3.1 p.74). The fields are address = bits 1-6, mode = bits 7-11, data = bits 12-18 = D0-D6 (p.74). In code, with the corrected 18-bit value `t`: `address = t & 0x3F`, `mode = (t >> 6) & 0x1F`, `data = (t >> 11) & 0x7F`.
- **Bit stream of a packet's triplets.** Where a field crosses triplets (Tables 8, 9, 12), it is easiest to concatenate the data bits of triplets 2 to 13 into one stream: stream position `p = 18*(k-2) + (b-1)` for triplet k bit b. The stream has 216 positions, 0 to 215. [D]
- Character codes are written column/row, for example 2/0 = 0x20.
- Level ticks are written `[1 1.5 2.5 3.5]` with x = ticked and o = not ticked, as in `notes-level15.md`.

---------------------------------------------------------------------------------------------------

## 1. Where DRCS sits

### 1.1 Purpose (cl. 14, p.95) [V]
- Levels 2.5 and 3.5 can download new symbols as bit patterns on a matrix of 12 pixels across and 10 down, occupying one character position.
- Four display modes exist. They differ in the number of bits per pixel.
- DRCS characters can serve one page only, or be shared by any number of pages.

### 1.2 Level ticks [V]
| Item | Ticks | Source |
|---|---|---|
| X/28/3 (DRCS page function, coding, mode invocation) | [o o x x] | Table 1, p.24 |
| X/28/1 as DCLUT carrier (4 DCLUTs) | [o o o x] | Table 1, p.24 |
| M/29/1 | [o x o o], "Character set designation (according to earlier specifications)" | Table 1, p.24 |
| X/26/0-15 "redefinable characters" | [o o x x] | Table 1, p.23 |
| X/27/4 Format 1 | [o o x x]; X/27/5 Format 1 [o o o x] | Table 1, p.23 |
| "24 DRCS characters of mode type 12x10x1" | [o o x o] | Table 25, p.68 |
| Four DRCS modes, memory for 384 PTUs | [o o o x] | Table 25, p.68 |
| Mode 0 (12x10x1) | [o o x x] | Table 31, p.96 |
| Modes 1, 2, 3 | [o o o x] | Table 31, p.96 |
| DRCS Mode triplet (row 11000) | [o o x x] | Table 28, p.82 |
| DRCS Character Invocation (column 01101) | [o o x x] | Table 29, p.88 |

- Table 1 lists M/29/1 only with its old character-set function. Clause 9.5.2 (p.36) and Table 12, however, give M/29/1 the same DCLUT coding as X/28/1 for all Level 1 pages of the magazine. See A-1.

### 1.3 System components (cl. 10.2, Figure 18, p.47; cl. 10.5, p.48) [V]
- Public enhancement data are the object pages (POP, GPOP) and the DRCS downloading pages (DRCS, GDRCS). Each DRCS and GDRCS page in Figure 18 carries an X/28/3.
- Public enhancement data can be used by every Level 1 page in the service (cl. 10.5).

---------------------------------------------------------------------------------------------------

## 2. DRCS downloading pages

### 2.1 Identification
- **Page function** (Table 3, p.29, X/28/0 Format 1, X/28/3 and X/28/4 triplet 1 bits 1-4, bit 4 MSB) [V]:
  - 0100 (4) = Global DRCS downloading page (GDRCS);
  - 0101 (5) = Normal DRCS downloading page (DRCS).
- **Page coding** (Table 3 bits 5-7; Table 9, p.34) [V]: a DRCS page uses 000, "7 bits plus odd parity". Table 9 fixes this for the X/28/3 case.
- **How a decoder knows a page is a DRCS page** (Annex D.1, p.131) [V]: without X/28, a Level 2.5/3.5 decoder may decide from the MOT, from X/27/4 links of other pages, or from a Code of Practice that fixes page numbers.
- **Page numbers** (Annex E.1, p.134, informative) [V]: object and DRCS pages should use page numbers containing a hex digit, so that they are not captured as display pages.
- **How many** (cl. 10.5.2, p.51) [V]: per magazine, up to seven normal DRCS pages and one GDRCS page, each with up to 16 sub-pages.
- **Global versus normal** (cl. 14.1, p.95) [V]: there are one or two DRCS tables, each with up to 16 sub-tables. The MOT defines one as the global table (GDRCS) and the other as the normal table (DRCS) for a page. Each sub-table is a complete DRCS table of up to 48 characters.
- Finding the page for a given display page: the MOT links (X/21 at Level 2.5, X/24 at Level 3.5) and X/27/4 links 2 (GDRCS) and 3 (DRCS), with X/27/5 at Level 3.5. These are in `notes-level25-objects.md` sections 7.3, 7.7 and 9. X/27/4 and X/27/5 take precedence over the MOT (cl. 10.5.2.2, p.51) [V].

### 2.2 Page format (cl. 10.5.2.1, p.51; cl. 14.1, p.95; Figure 27, p.96) [V]
- A page header, then packets X/1 to X/24. Packet X/25 is not used (Figure 27 stops at row 24).
- An optional X/28/0 can state the page function and coding.
- An X/28/3 gives the mode of each PTU. It "may be omitted if the page defines 12x10x1 mode characters only" (p.96).
- Unused packets need not be transmitted, and unused addresses may be padded with SPACE 2/0 (p.95).
- Partial updates are allowed. The same paragraph also says "the full page has to be transmitted each magazine cycle" (p.51). See A-2.

### 2.3 Sub-pages and the sub-code
- Each sub-page is one sub-table. The sub-page is identified by the S1 nibble of its sub-code (cl. 10.5.2.1, p.51) [V].
- For data pages (MOT, POP, GPOP, DRCS, GDRCS, MIP), the sub-code is coded `nnXs` (Annex A.1, p.120) [V]:
  - nn = S4 (2 MSBs) and S3 (4 LSBs), a 6-bit number: the last packet that would be sent if the page went out uninterrupted. Values 1 to 25 mean X/1 to X/25, and 26 to 41 mean X/26/0 to X/26/15. For a DRCS page, nn is at most 24 [D].
  - X = S2, an update indicator incremented modulo 8 on every change.
  - s = S1, the sub-page identity. The first sub-page has S1 = 0, and S1 increases by one for each further sub-page.
- The DRCS Mode triplet's T3-T0 bits name the sub-table, and they "correspond to the S1 component of the sub-code" (Table 28, p.82) [V].
- The MOT DRCS link gives the number of sub-pages as the highest S1 transmitted (0 = 1 sub-page, F = 16) (Table 23, p.55) [V]. The X/27/4 link gives one flag per S1 value 0-15 (Table 15, p.40) [V].

### 2.4 Transmission rules (Annex B, normative) [V]
- Pages for further processing send X/1 to X/26 in ascending packet order, and X/26 in ascending designation code order (B.2 rule 3, p.125).
- Every enhancement page, DRCS and GDRCS included, is sent in full at least every 20 s (B.6 rule 1, p.126).
- No 20 ms erasure interval is required; pages may follow each other in a burst (B.6 rules 2-3).
- C4 set: erase all packets of the previous version (B.6 rule 5).
- Partial updates carry the header and the changed packets, with C13 = 1. Fragments use C12. Table B.1 (p.127) gives the C4/C8/C12/C13/S2 combinations (B.6 rules 6-7).
- When pages change, the order is: the updated MOT, then the updated DRCS pages, then the object pages, then the Level 1 page (B.7, p.127).
- At most 100 ms between successive packets of one page (B.9, p.127).

### 2.5 Missing packets and capacity [V]
- If a Level 2.5/3.5 decoder takes a page to be a DRCS page, missing packets "may be interpreted" as an error when a character whose PTUs are in those packets is selected for display, or when X/28/3 names a mode for a PTU in a missing packet. The decoder's response is not defined (D.1.1 rule 2, p.131).
- Without X/28/3, a Level 3.5 decoder that believes the page is a DRCS page should treat it as 12x10x1 only (D.1.7 rule 2, p.132).
- A Level 3.5 decoder checks for X/28/0 Format 1 and X/28/4 before assuming a page is a Level 1 text page when X/28/3 is absent (D.1.7 rule 1).
- Totals across all magazines (E.8, p.135, informative): Level 2.5 uses at most 500 packets of DRCS and object pages; Level 3.5 at most 1200.
- Decoders "should have sufficient memory" for the most packets of DRCS and object pages, sub-pages included, that each level may transmit (G.4, p.141, informative).

---------------------------------------------------------------------------------------------------

## 3. Pattern Transfer Units (PTUs)

### 3.1 Layout in the packets (cl. 14.1, p.95; Figure 27, p.96) [V]
- A PTU is 20 bytes. Each of X/1 to X/24 carries two PTUs, "20 plus 20 bytes, proceeding in time through the packet". So a sub-table has 48 PTUs, numbered 0 to 47 [D].
- PTU addresses "relate directly to positions in the code table used when invoking DRCS characters": the character number in a DRCS Character Invocation is the PTU address of the character's first PTU (cl. 14.1; cl. 14.2.1-14.2.4, p.97) [V].
- Every PTU byte is a 7-bit code plus odd parity. Only codes 4/0 to 7/F are used, so the 6 low bits carry pattern data and bit 7 is always 1. Such a byte is called a D-byte (p.95) [V].

| PTU address n | Packet | Packet bytes (1-45 numbering) | Index in a 42-byte T42 record (0 = first MRAG byte) |
|---|---|---|---|
| even, n = 2k | X/(k+1) | 6 to 25 | 2 to 21 |
| odd, n = 2k+1 | X/(k+1) | 26 to 45 | 22 to 41 |

The packet column follows from "two PTUs per packet"; the T42 index subtracts 4, because T42 starts at packet byte 4 (`notes-formats.md` section 1). [D]

- Figure 27 shows, as an example layout, character 0 = PTU 0 and character 1 = PTU 1 (12x10x1, both in X/1); character 2 = PTUs 2-3 (12x10x2); and character 4 = PTUs 4-7 (12x10x4, X/3 and X/4). There is no character 3 in that layout. A multi-PTU character may therefore span two packets (character 4 does) [V; the "may span" reading is D].

### 3.2 D-byte to pixel row (cl. 14.2.1, p.97) [V]
- Each D-byte supplies 6 dots, from its 6 least significant bits.
- Loading starts at the top-left corner and runs left to right, then row by row.
- The most significant of the 6 bits is the leftmost dot.
- So for a 12x10 plane, D-byte i (0 to 19) of the PTU covers row `r = i / 2`, columns `6*(i % 2)` to `6*(i % 2) + 5`. Dot `x` of that half-row is bit `5 - x` of the D-byte, where bit 0 is the least significant. [D]
- In code, with `c` = the received byte with parity stripped:
  - `pixels6 = c & 0x3F`;
  - `dot(row, col) = (D[2*row + col/6] >> (5 - col % 6)) & 1`. [D]

### 3.3 Modes (cl. 14.2, Table 31, p.96; cl. 14.2.1-14.2.4, pp.97-98) [V]
| Mode | Mode ID (Table 31) | Format (H dots × V dots × bits/pixel) | PTUs per character | Bytes per character | Levels |
|---|---|---|---|---|---|
| 0 | 0000 | 12×10×1 | 1 | 20 | 2.5, 3.5 |
| 1 | 0001 | 12×10×2 | 2 | 40 | 3.5 |
| 2 | 0010 | 12×10×4 | 4 | 80 | 3.5 |
| 3 | 0011 | 6×5×4 | 1 | 20 | 3.5 |

- **Mode 0 (12×10×1).** One PTU. Pixels with bit 1 take the foreground colour, and pixels with bit 0 the background colour, active at the character's position (p.97).
- **Mode 1 (12×10×2).** Two consecutive PTUs. The character address is the first PTU. Each PTU is one bit plane, loaded exactly as mode 0. The first plane is the least significant bit of the DCLUT address and the second plane the most significant (p.97).
  - Pixel value `v = plane0 | plane1 << 1`, in 0-3, indexes DCLUT4: the global DCLUT4 for a character from the global table, the normal DCLUT4 for one from the normal table. [D from p.97]
- **Mode 2 (12×10×4).** Four consecutive PTUs, one bit plane each, loaded as mode 0. The first plane is the DCLUT LSB and the fourth the MSB (p.97).
  - `v = p0 | p1<<1 | p2<<2 | p3<<3`, in 0-15, indexes DCLUT16, global or normal according to the table. [D]
- **Mode 3 (6×5×4).** One PTU, taken as five groups of four D-bytes. Within a group, the four D-bytes are the first, second, third and fourth bit planes of one row of pixels (p.97).
  - So D-byte `4*r + k` (r = 0-4, k = 0-3) is plane k of row r, and each D-byte holds the 6 pixels of that row. [D]
  - The bit order inside a mode 3 D-byte is not restated. The reading that most significant of the 6 bits = leftmost pixel, as in mode 0, is [U]; see A-5.
  - `v` indexes DCLUT16, global or normal (pp.97-98).
- **Mode IDs 1110 and 1111** (p.96; Table 9, p.34) [V]:
  - When a character uses more than one PTU, its first PTU carries the mode ID and each later PTU carries 1110 ("Subsequent PTU of a Mode 1 or 2 character").
  - 1111 means "no data is transmitted for the corresponding character".
  - All other values are reserved.

### 3.4 Characters per sub-table and per page [D from cl. 14.1, 14.4]
- One sub-table holds 48 PTUs. That is 48 characters in modes 0 and 3, 24 in mode 1, and 12 in mode 2. Mixtures are allowed, since modes are given per PTU (p.96). [D]
- Per-page limits are in section 7.

### 3.5 Worked PTU examples (built for tests; not from the standard) [D]
- A blank 12×10×1 character is 20 bytes of 0x40, written to the line with odd parity as 0x40. A full block is 20 bytes of 0x7F.
- Row 0 all on, then row 1 with only the leftmost dot: D-bytes `7F 7F 60 40`, then 16 × `40`.
  - Check: 0x60 & 0x3F = 0x20 = binary 100000, so its most significant bit gives dot 0.
- Rightmost dot of the left half (dot 5) is D-byte 0x41. The rightmost dot of the row (dot 11) is the second D-byte = 0x41.
- A 12×10×2 character at address 1, with row 0 left half = value 3 and right half = value 1:
  - plane 0 (PTU 1) = `7F 7F` and then `40`s;
  - plane 1 (PTU 2) = `7F 40` and then `40`s;
  - X/28/3 mode IDs: PTU 1 = 0001 and PTU 2 = 1110.
  - The same bytes were used in the zvbi probe of section 13.

---------------------------------------------------------------------------------------------------

## 4. Packet X/28/3 (cl. 9.4.6, Table 9, p.34) [V]

| Triplet | Bits | Content |
|---|---|---|
| 1 | 1-7 | page function 0100 or 0101 (bits 1-4) and page coding 000 (bits 5-7), as Table 3 |
| 1 | 8-18 | reserved |
| 2-11 | 1-18 | DRCS Downloading Mode Invocation: with triplet 12 bits 1-12, 192 bits holding 48 mode IDs of 4 bits, one per PTU |
| 12 | 1-12 | (continued) |
| 12 | 13-18 | reserved |
| 13 | 1-18 | reserved |

- In stream positions (see Conventions), the 48 mode IDs fill positions 0 to 191, and positions 192 to 215 are reserved. [D]
- Table 9 lists each mode ID as "Bits (MSB LSB)". It does not say which transmitted bit is the most significant, nor that PTU 0 comes first. A-4 records the reading proposed here [U]:
  - the ID for PTU n is at stream positions 4n to 4n+3;
  - position 4n is the least significant bit, the same "least significant bit first" rule that Table 8 states for DCLUT words.
- X/28/3 is ticked [o o x x] (Table 1). Clause 9.4.6 says it may be sent with a DRCS page "at presentation Levels 2.5 and 3.5". Clause 10.5.2.1 says "At Level 3.5 the page may also require" it, and Table 9's text speaks of modes "at Level 3.5". See A-3.
- Worked encoding [D], PTU 0 = mode 0, PTU 1 = mode 1, PTU 2 = 1110, and the rest 1111:
  - triplet 1 = 0x00005;
  - triplet 2 = 0x3FE10;
  - triplets 3-11 = 0x3FFFF;
  - triplet 12 = 0x00FFF;
  - triplet 13 = 0x00000.
  - These values use the LSB-first reading. zvbi did not confirm them (section 13.3).

---------------------------------------------------------------------------------------------------

## 5. Referencing DRCS characters from a page

### 5.1 DRCS Mode triplet: row address group, mode 11000 (Table 28, p.82) [V]
| Field | Bits | Meaning |
|---|---|---|
| Address | 1-6 | "reserved"; the Active Position is not modified |
| Mode | 7-11 | 11000 |
| D6 = S | 18 | 0 = Global DRCS, 1 = Normal DRCS (the source page) |
| D5, D4 = L1, L0 | 17, 16 | DRCS usage: 00 reserved, 01 Level 2.5, 10 Level 3.5, 11 Levels 2.5 and 3.5 |
| D3-D0 = T3-T0 | 15-12 | sub-table (sub-page) required, MSB to LSB = the S1 sub-code nibble |

- The page numbers of normal and global DRCS pages come from the MOT or from X/27/4/X/27/5.
- The designated sub-table "is assumed to be latched" for later DRCS Character Invocations.
- One normal and one global sub-table can be designated at the same time.
- If an object invokes a DRCS character, the object's own definition must contain the sub-table designation (also Annex C.4 rule 3, p.130).
- NOTE (p.82): the DRCS mode information applies only within the object, or the local enhancement data, where it occurs. It is not passed into an invoked object and not inherited back on return.
- The address field is reserved. A row address value must still be ≥ 40, because the address field decides which group the triplet is in (cl. 12.3.1, p.74) [D]. The encoder in this study used 41.
- Worked values [D]:
  - normal, L = 11, T = 0: data 0x70, triplet 0x38629 with address 41;
  - global, L = 01, T = 3: data 0x13, triplet 0x09E29.

### 5.2 DRCS Character Invocation: column address group, mode 01101 (Table 29, p.88) [V]
- The address (0-39) sets the column of the Active Position, as for all non-reserved column triplets (cl. 12.3.2, p.76; cl. 12.3.4, p.83).
- D5-D0 is the character, 0 to 47 decimal.
- D6 chooses the table: 0 = global, 1 = normal, "designated for current use".
- **Defaults** (p.88; C.4 rule 1, p.130): when no DRCS Mode triplet has occurred in the same object or local enhancement data, the global sub-table is 0 and the normal sub-table is 0, at Levels 2.5 and 3.5. This default applies at the start of local enhancement data and at each object invocation. An invoked object does not inherit the existing mode, and does not pass one back.
- Worked values [D]:
  - column 2, normal character 0: data 0x40, triplet 0x20342;
  - column 10, global character 47: data 0x2F, triplet 0x17B4A.
- Side panels: DRCS characters reach side-panel columns only through objects (cl. 12.5.3, p.90). Any character accessible via X/26 may be used there [V].

### 5.3 Encoder obligations (normative and informative) [V]
- The broadcaster shall make sure that DRCS characters used inside an object invoked by a page are reachable through that page's own MOT link, or are on DRCS pages the page references (Table 23 NOTE 2, p.55; E.6, p.135).

---------------------------------------------------------------------------------------------------

## 6. Colour of DRCS pixels

### 6.1 Mode 0 [V]
- Foreground and background colours "active at the positions where they are located" (cl. 14.2.1, p.97). So all attribute machinery applies: spacing colours, non-spacing colour triplets, invert and so on. Pixel 1 = foreground, pixel 0 = background.

### 6.2 Modes 1-3: DCLUTs (cl. 14.2.2-14.2.4, p.97; cl. 9.4.4, Table 8, p.33; cl. 9.5.2, Table 12, p.36; Figure 28, p.99) [V]
- Four DCLUTs exist: DCLUT4 global (12×10×2), DCLUT4 normal (12×10×2), DCLUT16 global (12×10×4 and 6×5×4), and DCLUT16 normal (12×10×4 and 6×5×4).
- Each DCLUT entry is a 5-bit word that selects one of the 32 Colour Map entries. Figure 28 draws 5-bit values pointing into CLUTs 0-3. INFERENCE from Figure 28: Colour Map index = word, so CLUT = word >> 3 and entry = word & 7 [U, but strongly supported by the figure].
- **Carried by X/28/1** for one page, or by M/29/1 for every Level 1 page of magazine M. X/28/1 overrides M/29/1 for its page (cl. 9.5.2, p.36). Clause 10.3.3 (p.48): DCLUT definition via X/28/1 is a Level 3.5 function.
- **Positions** (Table 8; "the transmission order is least significant bit first"; stream positions are [D]):

| DCLUT | Table 8 triplet/bits | Words | Stream positions | Word i at |
|---|---|---|---|---|
| DCLUT4 global | T2 1-18, T3 1-2 | 4 × 5 bits | 0-19 | 5i .. 5i+4 (LSB at 5i) |
| DCLUT4 normal | T3 3-18, T4 1-4 | 4 × 5 | 20-39 | 20+5i .. |
| DCLUT16 global | T4 5-18, T5-T7, T8 1-12 | 16 × 5 | 40-119 | 40+5i .. |
| DCLUT16 normal | T8 13-18, T9-T12, T13 1-2 | 16 × 5 | 120-199 | 120+5i .. |
| reserved | T13 3-18 | | 200-215 | |

- Triplet 1 of X/28/1 (G0/G1 character set codes) is for Level 1/1.5 compatibility only and is not meant for Level 2.5/3.5 decoders (Table 8 NOTE).
- The DCLUT16 Normal row of Table 8 says its words "define the DCLUT for use with global 12x10x4 and 6x5x4 DRCS". This is a copy error; the row heading says Normal. The 1997 edition has the same text (ETS 1997 p.36). See A-6.
- **Defaults** (D.2.2, p.133, a Level 3.5 decoder with no X/28/1 and no M/29/1; also D.1.6, p.132):
  - both DCLUT4s hold, in ascending address order, the first four full-intensity colours = Colour Map entries 0-3 of CLUT 0;
  - both DCLUT16s hold, in ascending order, CLUTs 0 and 1 = entries 0-15, "including transparent".
  - So the default DCLUT16 index 8 is CLUT 1 entry 0, transparent. [D]
- Worked X/28/1 carrying exactly the default DCLUTs [D], with triplet 1 = 0: triplets 2-13 =
  - 0x18820, 0x22080, 0x08201, 0x22906, 0x12839, 0x2B16A,
  - 0x207B9, 0x10620, 0x0398A, 0x16A4A, 0x3B9AC, 0x00001.

### 6.3 Transparency, flash and other attributes [V unless marked]
- A DCLUT entry pointing to CLUT 1 entry 0 makes that pixel transparent. The plane priorities are in Annex C.1 (p.128): transparency in a higher plane exposes the plane below. [D from Figure 28 and C.1]
- **Flash, mode 0:** as for G0-G3 characters; all modes of cl. 12.3.4 are valid (cl. 14.3.1, p.98).
- **Flash, modes 1-3:** every DRCS pixel is a foreground pixel (cl. 14.3.2, p.98; Additional Flash Functions, Table 29 mode 00111, p.85).
  - Normal and inverted flash: in the "on" phase the character is drawn as in steady mode. In the "off" phase "a space character is displayed in the background colour valid at that screen location".
  - Colour flash ("flash to the corresponding colour in an adjacent CLUT": CLUT 0↔1 and 2↔3) is handled as for the G sets. INFERENCE: each pixel's Colour Map index c is swapped with `c ^ 8` in the alternate phase [U]; see A-9.
- **Underline/Separated (Display Attributes D5):** "The response of a decoder is not defined when this bit is set for G3 or DRCS characters" (Table 29, p.87).
- **Invert (D4), Conceal (D2), Boxing/Window (D1)** with multi-colour DRCS: not addressed. Mode 0 behaves like any character. Modes 1-3 have no foreground/background pair to swap, and their concealed form is not stated. See A-10.

---------------------------------------------------------------------------------------------------

## 7. Level limits (cl. 14.4, p.98; Table 25, p.68) [V]

### 7.1 Level 2.5
- Only 12×10×1 is available.
- "A maximum of two DRCS sub-tables per display page can be designated."
- "Sub-tables from the normal and global DRCS tables can be invoked simultaneously."
- "A page may reference any 24 of the 96 DRCS characters defined by the two sub-tables."
- Table 25 summary: "24 DRCS characters of mode type 12x10x1".

### 7.2 Level 3.5
- All four modes.
- A page may reference any number of characters from the 32 accessible sub-tables (16 normal and 16 global), up to 384 PTUs.
- That is at most 384 characters if all are 12×10×1 or 6×5×4, and 96 if all are 12×10×4. Mixtures fall in between.
- Table 25: "A Level 3.5 decoder should have the memory capacity to display up to 384 PTUs."

### 7.3 What is not said
- What a decoder does with a 25th distinct character at Level 2.5, a third sub-table, or the 385th PTU.
- Whether "two sub-tables" means one normal plus one global, or any two.

See A-7 and A-8.

---------------------------------------------------------------------------------------------------

## 8. Geometry and rendering of DRCS

- The DRCS matrix is 12 pixels by 10 "to occupy one character position" (cl. 14, p.95) [V]. On a 12×20 cell (one per frame line, or both fields), each DRCS row covers two display lines [D].
- **Mode 3 (6×5).** The format column of Table 31 says 6 horizontal dots by 5 vertical dots. How the 6×5 grid fills the 12×10 cell is not stated. The natural reading is that each dot covers 2×2 pixels of the 12×10 matrix, so 2 × 4 lines on 12×20 [U]; see A-5.
- **Appearance is exact.** Annex G.2 (p.140, informative) makes "the precise appearance of all characters and mosaics (other than DRCS characters)" a decoder option, including enlarged and underlined forms. So DRCS appearance is *not* a decoder option.
  - INFERENCE: DRCS must not be smoothed or re-styled, and enlarging (double height, width or size) replicates pixels [U]; see A-11.
- **Size attributes** apply to DRCS like any character; there is no DRCS-specific size rule (C.3, pp.129-130; Table 29 mode 01100, pp.86-87) [V by absence].

---------------------------------------------------------------------------------------------------

## 9. The G3 smoothed mosaic and line drawing set

### 9.1 Definition [V]
- Table 25 (p.68), G3 [o x x x]: a fixed set of 95 positions = 91 symbols plus SPACE (5/F). Address range 2/0 to 7/F. The response to 6/E, 6/F, 7/E and 7/F is undefined; the text says "7/E and 7/E", a typo for 7/F, as `notes-level15.md` section 1.3 notes.
- G1 and G3 "are fixed and cannot be re-designated" (cl. 15.1, p.100; E.5 rule 1, p.135). So national options and ESC do not apply to G3 [D].
- Table 48 (p.119) notes:
  - 5/F "permits the display of background colour";
  - positions marked (2), which are 6/E, 6/F, 7/E and 7/F, have no character and should not be transmitted;
  - dark areas are foreground colour;
  - 2/F "represents the activation of alternate dots of the matrix";
  - bit allocation as Table 35.

### 9.2 Placing G3 characters [V]
| Mode (column group) | Name | Ticks | Notes |
|---|---|---|---|
| 00010 | G3 character "at Level 1.5" | [o x x x] | intended for Level 1.5. Level 2.5 and 3.5 decoders must also interpret it. Level 1.5 decoders may not respond to some or all codes (Table 29 NOTE 1, p.84) |
| 01011 | G3 character "at Levels 2.5 and 3.5" | [o o x x] | Level 1.5 decoders should ignore it (Table 29 NOTE 3, p.86) |

- Both use the 7 data bits as the G3 code. Data below 0x20 is reserved, but the column of the Active Position is still set (pp.84, 86).
- E.3 (p.134): Level 1.5 practice typically limits mode 00010 to 5/1, 5/B, 5/C and 5/D.
- A G3 cell has no G1-style contiguous or separated form. Underline/Separated is undefined for G3 (Table 29 D5, p.87).
- Hold Mosaics concerns G1 characters only (Table 26 code 1/E, p.74). Whether a G3 character changes the held-mosaic state is not stated [U]; see A-13.

### 9.3 How the shapes are drawn
- **Appearance is a decoder option** (G.2, p.140), so any faithful rendering conforms. The table gives each glyph's shape only as a small raster figure.
- **Vector geometry.** [L2-19-025] mapped every G3 position to a Unicode character, mostly in U+1FB3C-U+1FB6F, whose names define the shape exactly on the "sextant" grid:
  - x is LEFT 0, CENTRE ½ or RIGHT 1;
  - y is UPPER 0, UPPER MIDDLE ⅓, LOWER MIDDLE ⅔ or LOWER 1, with y increasing downwards.
- **Check against the standard's figure [E].**
  - The Table 48 figure is one embedded raster, 258×896 px (pdfimages, page 119). Each glyph box is about 31×39 px inside its frame.
  - Each of the 57 filled smooth-mosaic glyphs (columns 2, 3, 6, 7, except 2/E, 2/F and 3/E) was rasterised from its Unicode polygon and compared pixel by pixel.
  - The polygons agreed with the figure on 95.1 % to 100 % of pixels.
  - For 56 of the 57 glyphs, the glyph's own polygon matched best of all 57 candidates.
  - The exception is 7/4: 95.1 % for its own polygon against 95.3 % for 7/2's. Inspection shows 7/4's cut starting one dot below the top-right corner, a staircase artefact, while 7/2's cut starts at ⅓ height. The mapping stands.
- **Raster resolution of the figure [E].** Quantising the figure's cells to 12×10 gives clean shapes, so the figure was drawn on the 12×10 matrix that clause 15.6 (p.105) states for the alphanumeric sets. Grid points at ⅓ and ⅔ height do not fall on whole rows at 10 rows (or 20 lines). The figure rounds them; for example 2/0's fill begins at row 7 of 0-9, where ⅔ × 10 = 6.67. The staircase is the decoder's choice (G.2).

Filled polygons for columns 2, 3, 6 and 7. Vertices are (x, y) with y down; each polygon is filled in the foreground colour, and the rest of the cell is background:

| G3 | Code | Unicode | Shape (Unicode name, lower case) | Polygon |
|---|---|---|---|---|
| 2/0 | 0x20 | U+1FB3C | lower left block diagonal lower middle left to lower centre | (½,1) (0,1) (0,⅔) |
| 2/1 | 0x21 | U+1FB3D | lower left block diagonal lower middle left to lower right | (1,1) (0,1) (0,⅔) |
| 2/2 | 0x22 | U+1FB3E | lower left block diagonal upper middle left to lower centre | (½,1) (0,1) (0,⅓) |
| 2/3 | 0x23 | U+1FB3F | lower left block diagonal upper middle left to lower right | (1,1) (0,1) (0,⅓) |
| 2/4 | 0x24 | U+1FB40 | lower left block diagonal upper left to lower centre | (0,0) (½,1) (0,1) |
| 2/5 | 0x25 | U+25E3 | black lower left triangle | (0,0) (1,1) (0,1) |
| 2/6 | 0x26 | U+1FB41 | lower right block diagonal upper middle left to upper centre | (½,0) (1,0) (1,1) (0,1) (0,⅓) |
| 2/7 | 0x27 | U+1FB42 | lower right block diagonal upper middle left to upper right | (1,0) (1,1) (0,1) (0,⅓) |
| 2/8 | 0x28 | U+1FB43 | lower right block diagonal lower middle left to upper centre | (½,0) (1,0) (1,1) (0,1) (0,⅔) |
| 2/9 | 0x29 | U+1FB44 | lower right block diagonal lower middle left to upper right | (1,0) (1,1) (0,1) (0,⅔) |
| 2/A | 0x2A | U+1FB45 | lower right block diagonal lower left to upper centre | (½,0) (1,0) (1,1) (0,1) |
| 2/B | 0x2B | U+1FB46 | lower right block diagonal lower middle left to upper middle right | (1,⅓) (1,1) (0,1) (0,⅔) |
| 2/C | 0x2C | U+1FB68 | upper and right and lower triangular three quarters block | (0,0) (1,0) (1,1) (0,1) (½,½) |
| 2/D | 0x2D | U+1FB69 | left and lower and right triangular three quarters block | (0,0) (½,½) (1,0) (1,1) (0,1) |
| 3/0 | 0x30 | U+1FB47 | lower right block diagonal lower centre to lower middle right | (1,⅔) (1,1) (½,1) |
| 3/1 | 0x31 | U+1FB48 | lower right block diagonal lower left to lower middle right | (1,⅔) (1,1) (0,1) |
| 3/2 | 0x32 | U+1FB49 | lower right block diagonal lower centre to upper middle right | (1,⅓) (1,1) (½,1) |
| 3/3 | 0x33 | U+1FB4A | lower right block diagonal lower left to upper middle right | (1,⅓) (1,1) (0,1) |
| 3/4 | 0x34 | U+1FB4B | lower right block diagonal lower centre to upper right | (1,0) (1,1) (½,1) |
| 3/5 | 0x35 | U+25E2 | black lower right triangle | (1,0) (1,1) (0,1) |
| 3/6 | 0x36 | U+1FB4C | lower left block diagonal upper centre to upper middle right | (0,0) (½,0) (1,⅓) (1,1) (0,1) |
| 3/7 | 0x37 | U+1FB4D | lower left block diagonal upper left to upper middle right | (0,0) (1,⅓) (1,1) (0,1) |
| 3/8 | 0x38 | U+1FB4E | lower left block diagonal upper centre to lower middle right | (0,0) (½,0) (1,⅔) (1,1) (0,1) |
| 3/9 | 0x39 | U+1FB4F | lower left block diagonal upper left to lower middle right | (0,0) (1,⅔) (1,1) (0,1) |
| 3/A | 0x3A | U+1FB50 | lower left block diagonal upper centre to lower right | (0,0) (½,0) (1,1) (0,1) |
| 3/B | 0x3B | U+1FB51 | lower left block diagonal upper middle left to lower middle right | (1,⅔) (1,1) (0,1) (0,⅓) |
| 3/C | 0x3C | U+1FB6A | upper and left and lower triangular three quarters block | (0,0) (1,0) (½,½) (1,1) (0,1) |
| 3/D | 0x3D | U+1FB6B | left and upper and right triangular three quarters block | (0,0) (1,0) (1,1) (½,½) (0,1) |
| 3/F | 0x3F | U+2588 | full block | (0,0) (1,0) (1,1) (0,1) |
| 6/0 | 0x60 | U+1FB52 | upper right block diagonal lower middle left to lower centre | (0,0) (1,0) (1,1) (½,1) (0,⅔) |
| 6/1 | 0x61 | U+1FB53 | upper right block diagonal lower middle left to lower right | (0,0) (1,0) (1,1) (0,⅔) |
| 6/2 | 0x62 | U+1FB54 | upper right block diagonal upper middle left to lower centre | (0,0) (1,0) (1,1) (½,1) (0,⅓) |
| 6/3 | 0x63 | U+1FB55 | upper right block diagonal upper middle left to lower right | (0,0) (1,0) (1,1) (0,⅓) |
| 6/4 | 0x64 | U+1FB56 | upper right block diagonal upper left to lower centre | (0,0) (1,0) (1,1) (½,1) |
| 6/5 | 0x65 | U+25E5 | black upper right triangle | (0,0) (1,0) (1,1) |
| 6/6 | 0x66 | U+1FB57 | upper left block diagonal upper middle left to upper centre | (0,0) (½,0) (0,⅓) |
| 6/7 | 0x67 | U+1FB58 | upper left block diagonal upper middle left to upper right | (0,0) (1,0) (0,⅓) |
| 6/8 | 0x68 | U+1FB59 | upper left block diagonal lower middle left to upper centre | (0,0) (½,0) (0,⅔) |
| 6/9 | 0x69 | U+1FB5A | upper left block diagonal lower middle left to upper right | (0,0) (1,0) (0,⅔) |
| 6/A | 0x6A | U+1FB5B | upper left block diagonal lower left to upper centre | (0,0) (½,0) (0,1) |
| 6/B | 0x6B | U+1FB5C | upper left block diagonal lower middle left to upper middle right | (0,0) (1,0) (1,⅓) (0,⅔) |
| 6/C | 0x6C | U+1FB6C | left triangular one quarter block | (0,0) (0,1) (½,½) |
| 6/D | 0x6D | U+1FB6D | upper triangular one quarter block | (1,0) (0,0) (½,½) |
| 7/0 | 0x70 | U+1FB5D | upper left block diagonal lower centre to lower middle right | (0,0) (1,0) (1,⅔) (½,1) (0,1) |
| 7/1 | 0x71 | U+1FB5E | upper left block diagonal lower left to lower middle right | (0,0) (1,0) (1,⅔) (0,1) |
| 7/2 | 0x72 | U+1FB5F | upper left block diagonal lower centre to upper middle right | (0,0) (1,0) (1,⅓) (½,1) (0,1) |
| 7/3 | 0x73 | U+1FB60 | upper left block diagonal lower left to upper middle right | (0,0) (1,0) (1,⅓) (0,1) |
| 7/4 | 0x74 | U+1FB61 | upper left block diagonal lower centre to upper right | (0,0) (1,0) (½,1) (0,1) |
| 7/5 | 0x75 | U+25E4 | black upper left triangle | (0,0) (1,0) (0,1) |
| 7/6 | 0x76 | U+1FB62 | upper right block diagonal upper centre to upper middle right | (½,0) (1,0) (1,⅓) |
| 7/7 | 0x77 | U+1FB63 | upper right block diagonal upper left to upper middle right | (0,0) (1,0) (1,⅓) |
| 7/8 | 0x78 | U+1FB64 | upper right block diagonal upper centre to lower middle right | (½,0) (1,0) (1,⅔) |
| 7/9 | 0x79 | U+1FB65 | upper right block diagonal upper left to lower middle right | (0,0) (1,0) (1,⅔) |
| 7/A | 0x7A | U+1FB66 | upper right block diagonal upper centre to lower right | (½,0) (1,0) (1,1) |
| 7/B | 0x7B | U+1FB67 | upper right block diagonal upper middle left to lower middle right | (0,0) (1,0) (1,⅔) (0,⅓) |
| 7/C | 0x7C | U+1FB6E | right triangular one quarter block | (1,1) (1,0) (½,½) |
| 7/D | 0x7D | U+1FB6F | lower triangular one quarter block | (0,1) (1,1) (½,½) |

Symmetries useful for tests [D from the table]:
- column 3 is column 2 mirrored left to right;
- column 6 is the complement of column 2;
- column 7 is the complement of column 3.
- Exceptions: 2/C ↔ 3/C and 2/D ↔ 3/D are the left/right and top/bottom notches. 6/C, 6/D, 7/C and 7/D are the four quarter triangles. 6/E, 6/F, 7/E and 7/F are unassigned.

Glyphs that are not polygons. Stroke positions are measured from the figure quantised to 12×10, with dot columns 0-11 and rows 0-9 [E]. The Unicode equivalents are from [L2-19-025].

| G3 | Unicode | Description | Figure measurement [E] |
|---|---|---|---|
| 2/E | U+1FB70 vertical one eighth block-2 | thin vertical bar near the left | columns 2-3, full height |
| 3/E | U+1FB75 vertical one eighth block-7 | thin vertical bar near the right | columns 9-10, full height (not mirror-exact of 2/E) |
| 2/F | U+2592 medium shade | "alternate dots of the matrix" (NOTE 4) | checks 2 dots wide by 1 row high, alternating per row, not 1×1 (see A-14) |
| 4/0 | U+2537 up light, horizontal heavy | heavy horizontal plus light stub upwards | heavy band rows 3-6; stub columns 5-6, rows 0-2 |
| 4/1 | U+252F down light, horizontal heavy | heavy horizontal plus stub downwards | as 4/0, stub below |
| 4/2 | U+251D vertical light, right heavy | light vertical plus heavy bar to the right | vertical columns 5-6; heavy rows 3-6 from column 5 to 11 |
| 4/3 | U+2525 vertical light, left heavy | mirror of 4/2 | |
| 4/4-4/7 | U+1FBA4-U+1FBA7 | light chevrons: "<" upper centre-middle left-lower centre; ">" mirrored; "V" middle left-lower centre-middle right; "Λ" mirrored | strokes about 2 dots wide |
| 4/8-4/B | U+1FBA0-U+1FBA3 | light diagonal quarter arcs: upper centre-middle left, upper centre-middle right, middle left-lower centre, middle right-lower centre | |
| 4/C | U+253F vertical light, horizontal heavy | cross with heavy horizontal | heavy rows 3-6; vertical columns 5-6 |
| 4/D | U+2022 bullet | small filled disc | about columns 3-8, rows 3-6 |
| 4/E | U+25CF black circle | large filled disc | about the full width, rows 1-8 |
| 4/F | U+25CB white circle | ring | same outline as 4/E, about 2 dots thick |
| 5/0 | U+2502 light vertical | | columns 5-6, full height |
| 5/1 | U+2500 light horizontal | | rows 4-5, full width |
| 5/2-5/5 | U+250C, U+2510, U+2514, U+2518 | light corners: down-right, down-left, up-right, up-left | arms meet at columns 5-6 and rows 4-5 |
| 5/6-5/9 | U+251C, U+2524, U+252C, U+2534 | light tees: vertical+right, vertical+left, down+horizontal, up+horizontal | |
| 5/A | U+253C | light cross | |
| 5/B-5/E | U+2B62, U+2B60, U+2B61, U+2B63 | triangle-headed arrows: right, left, up, down | shaft 2 wide; head about 4 rows or 8 columns |
| 5/F | U+00A0 | SPACE in background colour (NOTE 1) | blank |

- Light strokes are 2 dots wide (columns 5-6) or 2 rows high (rows 4-5). Heavy horizontal strokes are 4 rows high (rows 3-6) [E]. Box-drawing glyphs join at the cell edges, so neighbouring cells connect [E].
- At 12×20, every row doubles; a vector renderer should keep the same proportions [D]. Because appearance is a decoder option (G.2), these measurements are guidance, not conformance.

---------------------------------------------------------------------------------------------------

## 10. Other repertoire changes at Levels 2.5 and 3.5

### 10.1 Character set designation [V]
- **Defaults** come from Table 32 (default G0 and G2 plus the national option), carried in triplet 1 bits 8-14 of X/28/0 Format 1 or X/28/4 for a page, and M/29/0 or M/29/4 for a magazine.
  - Priority, highest first: X/28/0 F1, then X/28/4 (Level 3.5 only), then M/29/0, then M/29/4 (Level 3.5 only) (C.4 rule 1, p.130; cl. 15.2, p.100).
  - Bits 8-10 should equal C12-C14. At Levels 2.5/3.5 the national option comes from these bits explicitly (cl. 15.2; E.9, pp.135-136). With no data, use Latin G0/G2 and the C12-C14 option (D.2.1 rule 1, p.133).
  - Table contents are in `notes-level15.md` section 9.
- **Second G0 set** (Table 33, X/28/0 F1 / X/28/4 / M/29/0 / M/29/4 triplet 1 bits 15-18 and triplet 2 bits 1-3) is toggled by ESC 1/B on the Level 1 page. The value 1111111 means no second set. It never designates a G2: the default G2 stays selected throughout (cl. 15.3, p.102).
- **Modified G0 and G2 Character Set Designation** (Table 29 column mode 01000, p.85; Table 34, p.105; cl. 15.4, p.104):
  - the 7 data bits pick a G0/G2 pair from Table 34;
  - it affects only characters written by X/26 and objects, never the Level 1 page;
  - Level 2.5 [o o x o]: at most two G0/G2 pairs per page, one of which is the default pair;
  - Level 3.5 [o o o x]: any number of pairs;
  - **duration** (C.4 rule 2, p.130): in local enhancement data and Active Objects, until another 01000 triplet or the end of the row; in Adaptive and Passive Objects, until the last cell of the object or another 01000 triplet. Table 29 adds that at the end of each display row, the defaults are re-selected.
- **Table 34** [V] (data bits 18-12):
  - 0000XXX to 0011XXX = Latin G0 + Latin G2 (four rows);
  - 0100000 Cyrillic-1 + Cyrillic G2;
  - 0100001, 0100010, 0100011 = Latin + Latin;
  - 0100100 Cyrillic-2 + Cyrillic;
  - 0100101 Cyrillic-3 + Cyrillic;
  - 0100110 Latin + Latin;
  - 0100111 blank, so reserved;
  - 0101XXX reserved;
  - 0110000-0110101 blank, so reserved;
  - 0110110 Latin + Latin;
  - 0110111 Greek + Greek;
  - 0111XXX reserved;
  - 1000000 Latin + Arabic;
  - 1000001-1000011 blank;
  - 1000100 Latin + Arabic;
  - 1000101-1000110 blank;
  - 1000111 Arabic + Arabic;
  - 1001XXX reserved;
  - 1010000-1010100 blank;
  - 1010101 Hebrew + Arabic;
  - 1010110 blank;
  - 1010111 Arabic + Arabic;
  - other values reserved.
  - Blank entries are reserved (Table 34 NOTE). Table 34 names no national option: when G0 is referenced by a triplet, positions 2/0-7/F are used unmodified by any national option (cl. 15.4 NOTE, p.104; Table 35 NOTE 2, p.106).
- **G2 per language:** Latin G2 (Table 37), Cyrillic G2 (Table 41), Greek G2 (Table 43) and Arabic G2 (Table 45). Hebrew G0 pairs with the Arabic G2. Contents are in `notes-level15.md` sections 7.1-7.2. Latin G2 codes 5/9, 5/A, 5/B and 6/5 should not be transmitted, and the response to them is undefined (Table 25, p.68; Table 37 NOTE 5).
- **G1 via X/26** (column mode 00001, p.84; Table 47 NOTES 2-3, p.118):
  - columns 2, 3, 6 and 7 are block mosaics;
  - columns 4-5 are the *currently selected G0 set's* characters, with no national option, when reached by a triplet.
- **Diacritics** (column modes 10000-11111, p.88; E.10, p.136): a G2 column 4 mark on a G0 character. Recommended for Latin sets only. Rendering details are in `notes-level15.md` section 6.
- **Accents and the `@` exception**: G0 2/A with mode 10000 shows "@" (Table 35 NOTE 3, p.106).

### 10.2 Level 3.5 font styles (Table 29 column mode 01110, p.88; Table 25, p.68; cl. 12, p.66) [V]
| D6 | D5 | D4 | D3 | D2 | D1 | D0 |
|---|---|---|---|---|---|---|
| R2 | R1 | R0 | reserved | Italics | Bold | Proportional spacing |

- The address field gives the column where the style starts.
- D0-D2: 1 enables, 0 cancels.
- The effect runs to the end of the display row, unless another Font Style triplet overrides it.
- R2-R0 (MSB-LSB) extends the style "to the following 0 to 7 (maximum) rows, at the same column positions".
- Ticks [o o o x].
- Scope: "The G0 and G2 character sets may be displayed in bold, in italics or proportionally spaced" (Table 25, p.68).
- **That is everything the standard says.** There are:
  - no glyph metrics;
  - no definition of what bold or italic look like (G.2 makes appearance a decoder option);
  - no rule for proportional advance widths;
  - no rule for how proportional text interacts with the 40-cell grid, with attributes held in cells, with double width or with background fill.

  See A-12.

---------------------------------------------------------------------------------------------------

## 11. Annex figures and worked examples useful for tests

- **Figure 27** (p.96): the PTU layout example: characters 0 and 1 in mode 0; character 2 in mode 1 (PTUs 2-3); character 4 in mode 2 (PTUs 4-7, crossing X/3 to X/4).
- **Figure 28** (p.99): DCLUT example values.
  - Normal DCLUT16 = 01111, 00101, 11001, 01010, 10101, 01000, 11010, 01011, 01100, 00100, 11010, 11100, 11110, 00011, 00111, 11000.
  - Global DCLUT4 = 10101, 00011, 01110, 11111 for pixel values 00-11.
  - Normal DCLUT4 = 10011, 10000, 10001, 00001.
  - Global DCLUT16 = 01100, 11010, 11001, 10101, 11100, 10011, 00011, 01111, 00001, 00000, 11111, 01010, 10111, 00010, 01110, 10001.
  - Each 5-bit value names a Colour Map entry. The figure draws the bit planes (LSB = first plane) pointing to a DCLUT row.
  - These are example data only. Read as written, MSB on the left, they make a ready test vector for X/28/1 packing [D].
  - The order of rows in the figure (top = address 0) is [U].
- **Table 31, Table 9 and Table 28/29 rows** give exact bit layouts for unit tests of the field decoders.
- **zvbi probe stream** (section 13): a synthetic T42 stream with a DRCS page, a MOT and a display page using X/26 G3, G2, diacritic and Font Style triplets. Its Hamming 24/18 encodings were accepted by zvbi, which validates the encoder used for the worked values above [E].

Worked triplet encodings [D, encoder validated as just described]:

| Triplet | 18-bit value | Hamming 24/18 bytes (on-air order) | TTI characters (hex) |
|---|---|---|---|
| DRCS Mode, normal, L=11, T=0 (address 41) | 0x38629 | 4C 62 70 | 69 58 78 |
| DRCS Mode, global, L=01, T=3 (address 41) | 0x09E29 | 4E 62 93 | 69 78 49 |
| DRCS Invocation, column 2, normal 0 | 0x20342 | 19 34 40 | 42 4D 60 |
| DRCS Invocation, column 10, global 47 | 0x17B4A | 5B 34 2F | 4A 6D 57 |
| G3 (01011), column 0, code 0x20 | 0x102C0 | 00 2C A0 | 40 4B 50 |
| G3 (00010), column 5, code 0x5B | 0x2D885 | 27 08 DB | 45 62 6D |
| Font Style, column 4, bold+italic+proportional, 2 more rows (data 0x27) | 0x13B84 | 20 B8 27 | 44 6E 53 |
| Termination Marker, local enhancement end, no objects (data 0x07) | 0x03FFF | 74 7F 87 | 7F 7F 43 |

---------------------------------------------------------------------------------------------------

## 12. TTI representation of enhancement and DRCS pages (public documentation only)

Details are in `notes-formats.md` sections 5.1-5.2; the points that matter here:
- **The MRG original** ([MRG] v1.1 §2.6) allows OL rows 1 to 27 only. It has no X/28, M/29 or enhancement encoding [V per `notes-formats.md` 5.1].
- **vbit2 extensions** ([VBIT2] wiki "Page-files", fetched 2026-10-10):
  - OL rows 26, 28 and 29 are "enhancement packets". The first character's low 4 bits are the designation code. The 39 characters that follow hold 13 triplets, three characters each with 6 bits per character, least significant group first, all characters in 0x40-0x7F. The data is "before hamming encoding".
  - So a triplet value t is written as the characters `0x40|(t&63)`, `0x40|((t>>6)&63)` and `0x40|((t>>12)&63)` [D]. For examples see the last column of the table in section 11.
  - Up to 16 OL lines per row number, each with its own designation code.
  - OL,28 lines with designation code 0, 2, 3 or 4 set the page function and coding, overriding an earlier PF line. So `OL,28,` followed by `C` (0x43, DC 3) and the X/28/3 triplets marks a DRCS page and carries its mode IDs [D].
  - OL,27 with DC 0-3 holds 39 nibbles; with DC 4-F it holds triplets as for X/26. X/27/4 and X/27/5 therefore use the triplet form [V per `notes-formats.md` 5.2].
  - `PF,f,c`: page function hex 0-B and coding 0-3. DRCS = `PF,5,0` and GDRCS = `PF,4,0` [D from Table 3].
  - With coding 0, vbit2 sends X/26/0-15 before X/1-X/25; otherwise it sends X/1-X/26 in order.
  - vbit2 "treats some pages as special pages" (GPOP, POP, GDRCS, DRCS, MOT) "based on the page function".
  - Row 29 on an mFF page is an M/29 packet and is not sent with the page.
- **DRCS PTU data in TTI** [D]:
  - With page coding 0, PTU bytes are ordinary OL,1-OL,24 characters in 0x40-0x7F.
  - These codes are at or above 0x20, so neither the 0x80 control-code translation nor the ESC form of [MRG] §3 applies.
  - A DRCS page TTI is therefore 24 lines of 40 printable characters, plus `OL,28,C…` for X/28/3.
  - SPACE padding (0x20) for unused PTUs is legal on air (p.95), and appears as spaces.
- **Not specified anywhere public** [U]: how the MRG/wxTED tools themselves stored enhancement packets. Only the vbit2 and zxnet conventions are documented. The project's `crates/neoteletext-formats/src/tti.rs` already decodes packed OL,26/27/28/29 lines this way (`packed_triplets`).

---------------------------------------------------------------------------------------------------

## 13. zvbi 0.2.44 as a test oracle (`oldvga`)

### 13.1 What the public header exposes ([ZVBI-H], read only) [V for declarations; semantics marked]
- `vbi_teletext_set_level(vbi, level)` and `vbi_fetch_vt_page(vbi, pg, pgno, subno, max_level, display_rows, navigation)`, with `vbi_wst_level` = `VBI_WST_LEVEL_1`, `_1p5`, `_2p5`, `_3p5`.
- `vbi_decode(vbi, vbi_sliced *, lines, timestamp)`. `vbi_sliced` is `{id, line, data[56]}`; a T42 record goes in `data` with `id = VBI_SLICED_TELETEXT_B`.
- `vbi_page` fields:
  - `rows`, `columns` (observed 25 × 41: the 40 columns plus one spare [E]);
  - `text[1056]` of `vbi_char`;
  - `screen_color`, `screen_opacity`;
  - `color_map[40]`;
  - `drcs_clut` ("64 entries");
  - `drcs[32]`;
  - `font[2]`, `page_opacity[2]`, `boxed_opacity[2]`, `nav_link[6]`, `nav_index`.
  - INFERENCE: `drcs[32]` is one pointer per accessible sub-table (16 normal + 16 global) [U].
- `vbi_char` bit-fields: `underline`, `bold`, `italic`, `flash`, `conceal`, `proportional`, `link`, `size` (`vbi_size`: normal, double width, double height, double size, over top, over bottom, double height 2, double size 2), `opacity` (`vbi_opacity`: transparent space, transparent full, semi-transparent, opaque), `foreground`, `background` (indices into `color_map`), `drcs_clut_offs`, `unicode` (16 bits).
- Inline helpers:
  - `vbi_is_print(u)` is u < 0xE600;
  - `vbi_is_gfx(u)` is 0xEE00 ≤ u ≤ 0xEFFF, the private range for G1 and G3;
  - `vbi_is_drcs(u)` is u ≥ 0xF000.
- Rendering: `vbi_draw_vt_page_region(pg, fmt, canvas, rowstride, column, row, width, height, reveal, flash_on)`, `vbi_get_vt_cell_size`.

### 13.2 Observed behaviour [E]
Method:
- A Python generator in the session scratchpad built a T42 stream: DRCS page 1A0 with X/28/3, a MOT 1FE, and display page 100 with X/27/4, X/28/0 and X/26. Hamming 8/4 was checked against the standard table (0x15, 0x02, … 0xEA). Hamming 24/18 passed all six parity tests of cl. 8.3.
- A C probe in `~/neoteletext-probe` on `oldvga`, outside the repository, fed each record to `vbi_decode` three times, then fetched page 100 at `VBI_WST_LEVEL_3p5` after `vbi_teletext_set_level(…, 3)`.

Results:
- **G3:** mode 01011 code 0x20 gave `unicode` U+EF20, and code 0x5B gave U+EF5B. zvbi stores G3 as `0xEF00 + code`, inside `vbi_is_gfx`.
- **G2 and diacritics:** mode 01111 code 0x24 gave U+0024. Mode 10010 (acute, G2 4/2) on G0 `a` gave U+00E1.
- **Font Style** at column 4 with D0-D2 set set `bold`, `italic` and `proportional` from column 4 to the end of the row. With R = 2, the same flags appeared at columns ≥ 4 on the next two rows and not on the third. This matches the reading "R further rows, from the same column".
- **Ordering:** when a triplet addressed a column earlier than the previous triplet (A.4 allows ignoring it), zvbi did not move back. It wrote that triplet's character at the current position instead. Test streams should keep A.4 order.
- **DRCS:** zvbi did not resolve the synthetic DRCS page in any of six variants:
  - with and without X/28/3;
  - with X/28/0 page function 0101;
  - with S4/S3 = last packet;
  - with a decimal page number;
  - with both MOT and X/27/4 links.

  While a DRCS triplet was present, zvbi dropped *every* X/26 enhancement on the page, not just the DRCS cells. With the DRCS triplets removed, the rest applied. The cause was not found.
- **DCLUT defaults:** `drcs_clut` held, with no X/28/1:
  - bytes 2-5 = 0 1 2 3;
  - bytes 6-9 = 0 1 2 3;
  - bytes 10-25 = 0-15;
  - bytes 26-41 = 0-15.

  This is consistent with D.2.2 and suggests the layout global DCLUT4, normal DCLUT4, global DCLUT16, normal DCLUT16 from offset 2 [U for the layout]. Bytes 42-63 held values that were not interpreted.
- **Colour map:** `color_map` entries 0-31 matched Table 30, as 0xAABBGGRR, for CLUT 0, CLUT 2 and CLUT 3, and for CLUT 1 entries 1-4 and 7. The exceptions:
  - entry 13 (CLUT 1 entry 5, half magenta) read ff007777 instead of the expected ff770077;
  - entry 14 (CLUT 1 entry 6, half cyan) read 00200000.

  Entries 32-39 are zvbi-private. Treat zvbi's CLUT 1 entries 5-6 with suspicion when using it as an oracle.
- An all-zero X/28/0 Format 1 was honoured: CLUTs 2 and 3 became black.

### 13.3 Using zvbi as an oracle
- Usable now for:
  - G3 placement, via the `unicode` value 0xEF00 + code;
  - G0/G2/diacritic composition;
  - font-style flag propagation;
  - attribute and size fields.
- Not usable yet for DRCS: the synthetic stream never got zvbi to load a DRCS page. A real broadcast capture with Level 2.5/3.5 DRCS, from the `$NEOTELETEXT_CORPUS` captures, should be tried before relying on it.
- zvbi is GPL, so it may only be run as an external program at test time, never linked into the product (CLAUDE.md).

---------------------------------------------------------------------------------------------------

## 14. Summary of what a decoder must hold [D]
- Per sub-table: 48 PTUs of 20 D-bytes (6 useful bits each) and 48 mode IDs of 4 bits.
  - Payload = 48 × 20 × 6 = 5760 bits = 720 bytes of pixel data per sub-table if packed, or 960 bytes stored as received.
- Level 2.5: two sub-tables, up to 24 distinct characters used per page.
- Level 3.5: up to 32 sub-tables accessible, of which at most 384 PTUs are referenced per page; four DCLUTs of 4 or 16 five-bit entries.
- Stored as received: 32 × 960 = 30 720 bytes plus 32 × 24 bytes of mode IDs [D]. The no-allocation core can size this statically.

---------------------------------------------------------------------------------------------------

## 15. Ambiguities, contradictions and gaps (for owner decisions)

- **A-1. M/29/1 role.** Table 1 (p.24) lists M/29/1 only as old character-set designation, [o x o o]. Clause 9.5.2 and Table 12 (p.36) give it the full X/28/1 DCLUT coding for all pages of the magazine, and D.2.2 (p.133) relies on it.
  - Decide: a Level 3.5 decoder takes DCLUTs from M/29/1 when X/28/1 is absent (proposed), or ignores M/29/1.
- **A-2. Full page every cycle versus partial updates.** Clause 10.5.2.1 (p.51) says partial updates are allowed but "the full page has to be transmitted each magazine cycle". B.6 (p.126) says in full at least every 20 s, with partial updates between.
  - Decide whether the acquisition logic may treat a DRCS page as complete from partial transmissions, and how stale data is replaced. Proposed: follow Annex B (normative).
- **A-3. Is X/28/3 a Level 2.5 packet?** Table 1 and cl. 9.4.6 say Levels 2.5 and 3.5. Clause 10.5.2.1 and Table 9 speak only of Level 3.5.
  - Decide whether a Level 2.5 decoder reads X/28/3. This matters for 1111 "no data" and for PTUs marked as modes 1-3.
  - Proposed: a 2.5 decoder reads X/28/3 and treats any non-0000 PTU as unusable.
- **A-4. Bit and PTU order of the X/28/3 mode IDs.** Table 9 gives "Bits (MSB LSB)" but not which transmitted bit is the MSB, nor that PTU 0 comes first.
  - Proposed: PTU n at stream positions 4n..4n+3, LSB first, consistent with Table 8's explicit LSB-first rule.
  - Not confirmed by any oracle; zvbi did not load the test page.
- **A-5. Mode 3 (6×5×4).**
  - The bit order within each D-byte is not restated; the proposal is MSB of 6 = left, as mode 0.
  - The mapping of 6×5 onto the 12×10 cell is not stated; the proposal is 2×2 replication.
- **A-6. Table 8 copy error.** The DCLUT16 Normal row says "global" (also in ETS 1997). Treat it as normal.
- **A-7. Level 2.5 "two sub-tables".** It is not clear whether this means one normal and one global, or any two, for example two normal sub-tables designated in turn.
  - Also undefined: the response to a third sub-table, or to a 25th distinct character.
  - Proposed: honour any designation and do not enforce the limit, since it binds the broadcaster. Or enforce it and ignore the excess; the owner decides.
- **A-8. Level 3.5 limit of 384 PTUs.** The decoder response when a page references more is undefined. Same choice as A-7.
- **A-9. Colour flash on multi-colour DRCS.** "Handled in the same way as for G0, G1, G2 and G3" (cl. 14.3.2). For multi-colour pixels, the reading proposed is that each pixel alternates between its Colour Map index and the corresponding entry of the adjacent CLUT (c ^ 8). Not stated.
- **A-10. Invert, Conceal and Boxing with modes 1-3.**
  - Invert swaps foreground and background, but multi-colour DRCS have no such pair.
  - What "displayed as SPACES" means for a multi-colour cell under Conceal: the proposal is a space in the background colour, as in the flash "off" phase.
  - Undefined.
- **A-11. Enlarged DRCS.** G.2 excludes DRCS from "decoder option" appearance but gives no enlargement rule. Proposed: exact pixel replication and no smoothing.
- **A-12. Font styles.** No appearance, metrics or proportional-spacing algorithm is given.
  - Decisions needed:
    - how bold and italic glyphs are produced (synthetic emboldening or slant, or separate glyph data);
    - whether proportional spacing changes the 40-cell grid at all, or only redistributes ink inside a run of cells;
    - how attributes in cells, background fill and double width interact with it;
    - how R2-R0 interacts with a later Font Style triplet on an extended row.
  - Also: whether styles apply to G1 columns 4-5, G3 or DRCS. Table 25 names only G0 and G2.
  - zvbi sets flags on every cell from the start column, whatever the character set.
- **A-13. G3 and Hold Mosaics.** Whether a G3 cell written by X/26 affects the held-mosaic character, or is replaced by it, is not stated. Table 26 NOTE 14 says hold applies to "characters defined at Levels 1 and 1.5", which arguably includes the Level 1.5 G3 triplet (00010).
- **A-14. G3 2/F "alternate dots".** The figure draws 2-dot by 1-row checks, not single-dot alternation.
  - Decide: single-dot checkerboard (matches the words) or 2×1 (matches the figure, and is about square on a 4:3 display) [E for the figure].
- **A-15. G3 geometry.** The standard gives only a small raster; appearance is a decoder option. Using the Unicode sextant geometry ([L2-19-025]) as the reference shape is a project choice. So are the rounding rule for ⅓ and ⅔ on 10- or 20-line cells, and the light/heavy stroke widths (2 dots, or 2 and 4 rows, measured).
- **A-16. Undefined codes.**
  - G3 6/E, 6/F, 7/E and 7/F; Latin G2 5/9, 5/A, 5/B and 6/5: the response is undefined.
  - DRCS characters whose first PTU is marked 1110 or 1111, or that run past PTU 47, for example mode 2 at PTU 46.
  - D-bytes outside 4/0-7/F, such as 2/0 padding, or bytes with parity errors.
  - Proposed: show a space, or the background, and log a diagnostic.
- **A-17. Missing DRCS packets or pages.** D.1.1 rule 2 allows treating these as an error, with the response undefined.
  - Decide the fallback: show the Level 1 character underneath, show a space, or show nothing for that cell. Not the zvbi behaviour, which drops all enhancement.
- **A-18. DRCS Mode triplet L1/L0.**
  - The meaning of "presentation Levels at which the DRCS data will be used" is not spelt out.
  - Proposed: a decoder running at level X ignores DRCS Mode triplets whose L bits exclude X. That lets a page designate different sub-tables for 2.5 and 3.5.
  - Also undefined: the reserved value 00.
- **A-19. DRCS Mode latch across rows.** Unlike mode 01000, nothing resets the DRCS sub-table at the end of a row. Proposed: it persists to the end of the local enhancement data or object [U].
- **A-20. Table 25 "7/E and 7/E"** for G3 undefined codes: a typo for 7/F (`notes-level15.md`).
- **A-21. Clause 9.4.7 text** says X/28/4 "redefines CLUTs 2 and 3 instead of CLUTs 0 and 1", the reverse of Table 10 and clause 10.3.4 (also in ETS 1997). This belongs to the colour topic and is listed only because DRCS DCLUT defaults depend on CLUTs 0 and 1. Follow Table 10: X/28/4 defines CLUTs 0 and 1.
- **A-22. zvbi CLUT 1 entries 5-6** differ from Table 30 (section 13.2) [E]. Do not use zvbi's `color_map` as an oracle for those entries.
