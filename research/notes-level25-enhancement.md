# ETSI EN 300 706 V1.2.1 (2003-04): Levels 2.5 and 3.5 — enhancement triplets, colour model, side panels and page-level packets

Source: `teletext-standards/EN_300_706_v1.2.1_Enhanced_Teletext.pdf` (relative to `~/devel/teletext-reference/`). Printed ETSI page number = PDF page number throughout. "p.N" means both.

Conventions:
- **[V]** = read directly in the standard at the cited place (tables and tick marks were read from the page images, not from a text extraction).
- **INFERENCE** = follows from the cited text but is not stated in it. **GAP** = the standard does not say.
- Level ticks are written `[1 1.5 2.5 3.5] = [x x x x]`, where x = ticked (filled circle) and o = not ticked (open circle), as in `notes-level15.md`.
- Bit numbering: a triplet's 18 Hamming 24/18 data bits are numbered 1 to 18, bit 1 the least significant and first transmitted (cl. 8.3 p.21; cl. 12.3.1 p.74). In code, with the corrected 18-bit value `t`, spec bit n is `(t >> (n-1)) & 1`. Byte and bit layouts of Hamming 24/18 are in `notes-level15.md` section 2.
- Quotations are kept short; everything else is paraphrase.

Scope boundary with other research topics: objects (invocation semantics, POP/GPOP pointer tables, Adaptive/Passive rules in depth) and DRCS (PTU formats, downloading pages, DCLUT use) are covered elsewhere. Here they appear only as triplet encodings and where they touch attributes or colours. Level 1.5 behaviour, G2/G3 glyph tables, Table 32/33 contents and Hamming are in `notes-level15.md` and are not repeated.

---------------------------------------------------------------------------------------------------

## 1. What Levels 2.5 and 3.5 add, and how they differ

### 1.1 Level definitions [V]
- Clause 1 (p.10): Level 2.5 extends the language repertoire, adds re-definable colours, introduces non-spacing attributes, a number of simple re-definable characters, and side panels. Level 3.5 extends the number and complexity of re-definable characters and introduces font styles and proportional spacing. Levels 2.5 and 3.5 replace the earlier Levels 2 and 3.
- Clause 12 (p.66): Level 2.5 increases display colours; "Sixteen of the 32 colours are re-definable"; row width can grow to 56 with side panels of 16 characters; non-spacing attributes and a limited number of DRCS. Level 3.5: more DRCS and modes, bold, italic, proportional spacing, "All 32 colours are redefinable" (but see ambiguity A-17: CLUT 1 entry 0 is fixed transparent).
- Clause 10.2, Figure 18 (p.47): a Level 2.5/3.5 page is the LOP plus local enhancement data X/26/0-15, X/28/0, X/28/1, X/28/4, plus public enhancement data (POP, GPOP, DRCS, GDRCS, the last two with X/28/3).

### 1.2 Packet usage by level — Table 1 (pp.23-24) [V]
| Packet | Function | [1 1.5 2.5 3.5] |
|---|---|---|
| X/26/0-15 | address locations incl. side panels; G0/G1/G2/G3 characters, DRCS, non-spacing attributes, objects | [o o x x] |
| X/26/0-15 | object definition pages | [o o x x] |
| X/27/4 F1 | compositional links to object and DRCS pages | [o o x x] |
| X/27/5 F1 | compositional links | [o o o x] |
| X/28/0 F1 | page function, page coding (note 3: not mandatory) | [x x x x] |
| X/28/0 F1 | character set designation, side panels, CLUTs 2 and 3, default screen and row colour, black background substitution, colour table re-mapping | [o o x x] |
| X/28/1 | character set designation per earlier specifications (note 4) | [o x o o] |
| X/28/1 | DCLUT4 global/normal, DCLUT16 global/normal | [o o o x] |
| X/28/3 | DRCS downloading page function, coding, mode invocation | [o o x x] |
| X/28/4 | as X/28/0 F1 but CLUTs 0 and 1 | [o o o x] |
| M/29/0 | as X/28/0 F1 (minus page function/coding), whole magazine unless the page has X/28/0 F1 | [o o x x] |
| M/29/1 | earlier-spec designation | [o x o o] |
| M/29/4 | as X/28/4 for the magazine unless the page has X/28/4 | [o o o x] |

Clause 9.2 (p.24): "Decoders should ignore bits and bytes which are indicated as being reserved for future use."

### 1.3 Display components by level — Table 25 (pp.67-69) [V]
| Component | [1 1.5 2.5 3.5] | Notes |
|---|---|---|
| 16 extra side-panel columns in rows 1-24 | [o o x x] | "display data is defined via packets X/26 and Objects"; nothing transmitted for row 0, whose format is the manufacturer's choice |
| Fallback character under X/26-overwritten cells | [o x x x] | even-parity bytes allowed where no Level 1 decoders exist |
| Two G0 sets plus national option via X/28/0 F1 and M/29/0 | [o x x x] | X/28/0 F1 has precedence over M/29/0 |
| G0 re-designation by a column triplet | [o o x x] | |
| Two G0 sets via X/28/4 and M/29/4 | [o o o x] | X/28/4 has precedence over M/29/4 |
| G2 designation via X/28/0 F1 / M/29/0 | [o o x x] | |
| G2 re-designation by a column triplet | [o o x x] | |
| G2 designation via X/28/4 / M/29/4 | [o o o x] | |
| Bold, italic, proportional spacing of G0 and G2 | [o o o x] | |
| DRCS: 24 characters of mode 12x10x1 | [o o x o] | |
| DRCS: four modes, up to 384 PTUs | [o o o x] | |
| Objects (may be shown in page and side-panel areas; may not straddle the boundary) | [o o x x] | |
| 8 background / 7 foreground full-intensity colours via spacing attributes | [x x x x] | |
| Black foreground as a spacing attribute | [o o x x] | |
| 32 colours, four CLUTs, CLUTs 2 and 3 redefinable by X/28/0 F1 or M/29/0, re-mapping of Level 1 colours | [o o x x] | |
| CLUTs 0 and 1 (except CLUT 1 entry 0) redefinable by X/28/4 or M/29/4 | [o o o x] | |
| Row and screen colours | [o o x x] | default is CLUT 0 entry 0: black at Level 2.5, redefinable at 3.5; "The colour selected also applies to the side-panel areas" |
| Side panels, 56 characters per row total | [o o x x] | |

### 1.4 Level 2.5 versus Level 3.5 — consolidated differences
| Feature | Level 2.5 | Level 3.5 | Source |
|---|---|---|---|
| CLUT 0 | fixed (Table 30 defaults) | redefinable via X/28/4, M/29/4 | Table 30 p.89; Table 25 p.69 |
| CLUT 1 entries 1-7 | fixed half-intensity | redefinable via X/28/4, M/29/4 | Table 30 |
| CLUT 1 entry 0 | transparent, fixed | transparent, fixed; its 12 bits in X/28/4 are ignored | Table 10 p.35 |
| CLUTs 2, 3 | redefinable via X/28/0 F1, M/29/0 | same | Table 30 |
| X/28/4, M/29/4 | ignored | used | Table 1; C.4 rule 1 "(Level 3.5 only)" p.130 |
| X/28/1 DCLUTs | ignored | used | Table 1; cl. 10.3.3 p.48 |
| X/27/5 | ignored | used | Table 1; Annex E.4 p.134 |
| Modified G0/G2 designation triplet | at most two G0/G2 pairs per page, one of them the default pair | any number of pairs | Table 29 mode 01000 p.85 |
| Font style triplet (bold, italic, proportional, row extension) | ignored | used | Table 29 mode 01110 p.88 |
| DRCS | 12x10x1 only; at most two sub-tables designated per page; any 24 of the 96 characters of those two sub-tables | all four modes; 32 sub-tables accessible (16 normal, 16 global); limit 384 PTUs | cl. 14.4 p.98; Table 31 p.96 |
| Object definitions | Object Usage bits: 01 = Level 2.5, 10 = Level 3.5, 11 = both | | Table 28 modes 10101-10111 p.81 |
| DRCS Mode triplet | L1 L0: 01 = Level 2.5, 10 = Level 3.5, 11 = both | | Table 28 mode 11000 p.82 |
| Side panels | shown only if X/28 Side Panel Status Flag = 1 | shown if enabled, flag 0 or 1 | Table 4 T2 bit 6 p.30 |
| MOT links | packets X/19-X/21 | X/22-X/24, falling back to X/19-X/21 if X/22 or X/24 absent | cl. 10.6.1 p.51 |
| Global enhancement data | total ≤ 500 packets of DRCS and object pages across all magazines | ≤ 1200 | Annex E.8 p.135 |
| Default screen/row colour "black" | always black | CLUT 0 entry 0, which may be redefined | Table 25 p.69 |

Limits common to both levels (objects topic owns the detail): up to 16 X/26 packets per page (cl. 10.3.1 p.47); one object up to 507 triplets (cl. 13.1 p.91); at most 127 invocations of each object type per page and at most 15 objects "open" at once (A.8 p.123-124).

INFERENCE: everything in Table 28 and Table 29 ticked [o o x x] is mandatory for both levels; the only 3.5-only triplet functions are Font Style and the "any number of pairs" sub-row of mode 01000.

---------------------------------------------------------------------------------------------------

## 2. Processing model for local enhancement data

### 2.1 Packet X/26 at Levels 2.5/3.5 [V]
- Up to 16 packets X/26 (designation codes 0-15) per basic page; each carries 13 Hamming 24/18 triplets (cl. 9.4 Figure 11 p.27; cl. 10.3.1 p.47).
- Enhancements carried (cl. 10.3.1 pp.47-48): full screen colour, full row colours, non-spacing attributes, G0/G2 designation, G0/G1/G2/G3 characters, diacritical marks, DRCS designation and characters, object invocation, Local Object definitions.
- The same triplet coding applies to packets X/3-X/25 of object definition pages (cl. 12.3.1 p.74).
- Transmission: X/26 before X/1-X/25 for display pages, ascending designation code (B.2 p.125); designation codes start at 0, contiguous (A.3 p.121); a missing designation code before the Termination Marker "may be interpreted as an error condition", response undefined (D.1.3 p.131).
- Absence of X/26: Level 2.5/3.5 decoders assume no enhancements other than default objects and defaults from X/28/0 F1, X/28/4, M/29/0, M/29/4 (D.1.2 p.131).

### 2.2 Order and structure [V]
- Local enhancement data comes first, then a Termination Marker, then any Local Object definitions (cl. 13.1 p.91; A.3 p.121). The terminator's data field identifies what ends (section 4.11).
- Triplets of local enhancement data and of each object "shall be encoded in order of the screen address", left to right, top to bottom, where "screen address" order is columns 0-39, then right panel 40-55, then left panel 56-71 (A.4 p.121). A decoder "may ignore" any triplet addressing a location earlier than the previous triplet from the same source (A.4).
- Row-address triplets that move the Active Position (00001, 00100, 00111) and all column triplets must be in screen address order; row/screen colour order is 00000, then 00111, then 00001 (A.4 pp.121-122).
- A.5 notes (p.122): only one Full Screen Colour per page, as the first triplet that defines any attribute or character; only one Full Row Colour per row, as the first triplet for that row.

### 2.3 Active Position at Levels 2.5/3.5 [V] (cl. 12.3.2 pp.75-76)
- Row 0-24, column 0-39; extended to the side panels only through the Origin Modifier.
- Reset to row 0, column 0 at the start of local enhancement data and when placing default objects.
- Row 0 is set only by mode 00111 with address 63 (also sets column 0).
- Rows 1-24 are set only by the address field of Full Row Colour (00001, also sets column 0) or Set Active Position (00100, column from data). Row = 24 if address = 40, else address − 40.
- "Level 2.5 and 3.5 decoders should not use the address field of other row address triplets for modifying the Active Position."
- Column: set by the address field of every column triplet except reserved modes and the PDC mode 00110; the row is not changed.
- Object invocation (13.7 p.93): the object's origin is the Active Position plus any Origin Modifier offsets; inside the object the Active Position starts at 0,0 relative to that origin; after the object the Active Position returns to its value at invocation and the Origin Modifier is cancelled (p.94). Default objects: Active Position deemed 0,0 before and after (13.8 p.94; G.5 p.141).

Consequence for this project (INFERENCE): decision E.3's default (Annex A.3, any address 40-63 sets the row) is incompatible with Level 2.5/3.5. Object Invocation and Object Definition triplets carry S1 S0 and N8 N7 in their address field (section 4.7), and a decoder applying A.3 would move the row on every invocation. At Levels 2.5/3.5 the strict clause 12.3.2 rule must be used.

---------------------------------------------------------------------------------------------------

## 3. Triplet fields and the colour index

### 3.1 Field layout [V] (cl. 12.3.1 p.74; cl. 12.3.4 p.83)
| Field | Spec bits | Code (`t` = 18-bit value) |
|---|---|---|
| Address A5..A0 | 6..1 | `t & 0x3F` |
| Mode M4..M0 | 11..7 | `(t >> 6) & 0x1F` |
| Data D6..D0 | 18..12 | `(t >> 11) & 0x7F` |
Address 0-39 = Column Address Group; 40-63 = Row Address Group.

### 3.2 The 5-bit colour index [V]
All colour-selecting fields (Full Screen Colour, Full Row Colour, Address Display Row 0, Foreground Colour, Background Colour, X/28 default screen and row colour) use 5 bits: two CLUT bits C1 C0 = D4 D3 and three entry bits E2 E1 E0 = D2 D1 D0 (Table 28 p.76; Table 29 p.84). So index = `D & 0x1F` = CLUT × 8 + entry, matching the 0-31 numbering of Figure 28 (p.99). The two bits above (D6 D5, called S1 S0 in row triplets) must be 0 for Foreground/Background/Full Screen Colour; other values are reserved.

---------------------------------------------------------------------------------------------------

## 4. Row Address Group triplets (address 40-63) — Table 28 (pp.76-83)

Summary (Table 27 p.75, Table 28):
| Mode | Function | [1 1.5 2.5 3.5] | Active Position effect |
|---|---|---|---|
| 00000 | Full Screen Colour | [o o x x] | none ("address value does not alter the Active Position") |
| 00001 | Full Row Colour | [o o x x] | row from address; column = 0 |
| 00010, 00011 | Reserved | — | none |
| 00100 | Set Active Position | row sub-row [o x x x]; column sub-row [o o x x] | row from address; column = data if < 40 |
| 00101, 00110 | Reserved | — | none |
| 00111 | Address Display Row 0 | [o x x x] | address 63: row 0, column 0; 40-62 reserved, no change |
| 01000-01101 | PDC (EN 300 231) | [o x x x] | none at 2.5/3.5 (cl. 12.3.2) |
| 01110, 01111 | Reserved | — | none |
| 10000 | Origin Modifier | [o o x x] | none; temporary offsets for the next invocation |
| 10001-10011 | Object Invocation: Active, Adaptive, Passive | [o o x x] | none (object origin = Active Position + offsets) |
| 10100 | Reserved | — | none |
| 10101-10111 | Object Definition: Active, Adaptive, Passive | [o o x x] | starts a definition |
| 11000 | DRCS Mode | [o o x x] | none ("address field is reserved") |
| 11001-11110 | Reserved | — | none |
| 11111 | Termination Marker | [o x o o] data reserved; [o o x x] data meaningful | ends the data |

### 4.1 Mode 00000 — Full Screen Colour [V] (p.76)
- Address: any 40-63; does not alter the Active Position. A.5 NOTE 1 recommends 41 for Level 1.5 compatibility.
- Data: S1 S0 C1 C0 E2 E1 E0. Only S1 S0 = 00 is defined: CLUT = C1 C0, entry = E2..E0. All other data values reserved.
- Applies to the screen area above row 0 and below row 23 (or below row 24 if row 24 is used). Default when absent: see A.5 (section 7.6).
- Restricted to local enhancement data and Active Objects; not allowed in side-panel objects (cl. 12.5.3 p.90).
- Duration: whole page (INFERENCE: it is a page-level property; A.5 NOTE 1 says one per page).

### 4.2 Mode 00001 — Full Row Colour [V] (p.77)
- Address: row per cl. 12.3.2; column set to 0.
- Data S1 S0 C1 C0 E2 E1 E0. When S1 = S0 the colour is CLUT C1C0 entry E2-E0; S1 S0 selects the rows:
  - 00: the addressed row only;
  - 01, 10: reserved;
  - 11: from the addressed row down to row 23 (or 24 if used), "unless subsequently overridden by a further Full Row Colour command on a subsequent row".
- Restricted to local enhancement data and Active Objects. Default when absent: A.5.
- Duration: a property of the row(s), not an attribute that runs along the row.

### 4.3 Mode 00100 — Set Active Position [V] (p.77)
- Address sets the row (cl. 12.3.2) for every data value.
- Data C6..C0: if < 40, the column. Data ≥ 40 reserved (GAP: whether the column is kept or reset; see A-9).

### 4.4 Mode 00111 — Address Display Row 0 [V] (p.78)
- Only address 111111 (63) is defined: Active Position becomes row 0, column 0; later column triplets "should address columns 8 to 39 only". Addresses 40-62 are reserved and do not move the Active Position.
- Data S1 S0 C1 C0 E2 E1 E0, as Full Row Colour but for row 0: S1 S0 = 00 colours row 0 only; 11 colours row 0 down to row 23 (or 24); 01 and 10 reserved.
- NOTE: double height and double size are not permitted in row 0.
- Restricted to local enhancement data and Active Objects. Default row colour: A.5.

### 4.5 Modes 01000-01101 — PDC [V] (p.78)
Country of Origin and Programme Source; Month and Day; Cursor Row and Announced Starting Time Hours; Cursor Row and Announced Finishing Time Hours; Cursor Row and Local Time Offset; Series Identifier and Series Code. Coding in EN 300 231. No display effect; no Active Position change at Level 2.5/3.5.

### 4.6 Mode 10000 — Origin Modifier [V] (p.79)
- Address rR5..rR0 = row offset + 40 (row offset = address − 40, so 0-23).
- Data rC6..rC0 = column offset, valid 0-71 (gives access to the side panels).
- Applies only to the object whose invocation triplet immediately follows; after one invocation both offsets are reset to 0 and the Active Position is restored.

### 4.7 Modes 10001-10011 — Object Invocation (encoding only) [V] (pp.79-80)
- Address bits 6..1 = 1, S1, S0, X(reserved), N8, N7. Mode bits 11..7 = 1, 0, 0, M1, M0. Data bits 18..12 = N6..N0.
- Object source S1 S0: 00 illegal (it would make the address < 40), 01 Local, 10 POP, 11 GPOP.
- Object type M1 M0: 01 Active, 10 Adaptive, 11 Passive (00 is the Origin Modifier mode).
- POP/GPOP: N3-N0 = S1 sub-code nibble of the object page; N8 N7 = pointer packet 1-4; N6 N5 with type = pointer triplet number (Active 1, 4, 7, 10; Adaptive 2, 5, 8, 11; Passive 3, 6, 9, 12); N4 = pointer in bits 1-9 (0) or bits 10-18 (1).
- Local: N8 = 0; N7-N4 = X/26 designation code (0-15); N3-N0 = triplet number (0-12). A Local Object can be invoked only from local enhancement data or another Local Object.

### 4.8 Modes 10101-10111 — Object Definition (encoding only) [V] (pp.81-82)
- Address bits 6..1 = 1, S1, S0, X, N8, N7; mode = 1, 0, 1, M1, M0; data = N6..N0.
- S1 S0 here is Object Usage: 00 illegal, 01 Level 2.5, 10 Level 3.5, 11 Levels 2.5 and 3.5. M1 M0: 00 reserved (mode 10100), 01 Active, 10 Adaptive, 11 Passive.
- N0-N8 equal the corresponding bits of the invoking triplet (sub-page, pointer location; for Local Objects N8 = 0, N7-N4 = designation code, N3-N0 = triplet number).
- An object ends at the next Object Definition or Termination Marker.

### 4.9 Mode 11000 — DRCS Mode (encoding only) [V] (p.82)
- Address reserved; Active Position not modified.
- Data: bit 18 (D6) = S: 0 global DRCS, 1 normal DRCS. Bits 17, 16 (D5, D4) = L1 L0: 00 reserved, 01 Level 2.5, 10 Level 3.5, 11 Levels 2.5 and 3.5. Bits 15-12 (D3-D0) = T3-T0, the sub-table (= S1 sub-code nibble).
- The designated sub-table is latched for later DRCS Character Invocations. A normal and a global sub-table can be designated at the same time. Scope: the object or the local enhancement data in which it occurs; not passed to an invoked object nor inherited back.

### 4.10 Reserved row modes
00010, 00011, 00101, 00110, 01110, 01111, 10100, 11001-11110: ignore (cl. 9.2); Active Position unchanged at Level 2.5/3.5 (cl. 12.3.2).

### 4.11 Mode 11111 — Termination Marker [V] (p.83)
- Address 111111 and mode 11111; placed in the last triplet (bytes 43-45) of the last packet in use; unused triplets between the data and the terminator repeat the terminator.
- Levels 2.5/3.5: data bits 12-14 (D2 D1 D0) give the type; bits 15-18 reserved:
  - 000 intermediate (G)POP sub-page, end of object, more objects follow on this page;
  - 001 intermediate (G)POP sub-page, end of last object on this page;
  - 010 last (G)POP sub-page, end of object, more follow;
  - 011 last (G)POP sub-page, end of last object;
  - 100 Local Object definitions, end of object, more follow;
  - 101 Local Object definitions, end of last object;
  - 110 local enhancement data ends, Local Object definitions follow;
  - 111 local enhancement data ends, no Local Object definitions follow.
- May be used as a space filler between object definitions and between the end of local enhancement data and the first Local Object definition.

---------------------------------------------------------------------------------------------------

## 5. Column Address Group triplets (address 0-39) — Table 29 (pp.84-88)

Summary (Table 27 p.75, Table 29):
| Mode | Function | [1 1.5 2.5 3.5] | Sets column | Duration |
|---|---|---|---|---|
| 00000 | Foreground Colour | [o o x x] | yes | to end of row unless overridden |
| 00001 | G1 block mosaic character | [o o x x] | yes (also for data < 0x20) | one cell |
| 00010 | G3 character (Level 1.5 form) | [o x x x] | yes (also for data < 0x20) | one cell |
| 00011 | Background Colour | [o o x x] | yes | to end of row unless overridden |
| 00100, 00101 | Reserved | — | no | — |
| 00110 | PDC cursor column, start/finish minutes | [x x x x] | no ("Active Position is not modified") | — |
| 00111 | Additional Flash Functions | [o o x x] | yes | GAP (INFERENCE: to end of row, see 6.2) |
| 01000 | Modified G0 and G2 designation | 2 pairs [o o x o]; any pairs [o o o x] | yes | to end of row (local/Active) or end of object (Adaptive/Passive) |
| 01001 | G0 character (Levels 2.5/3.5) | [o o x x] | yes (also for data < 0x20) | one cell |
| 01010 | Reserved | — | no | — |
| 01011 | G3 character (Levels 2.5/3.5) | [o o x x] | yes (also for data < 0x20) | one cell |
| 01100 | Display Attributes | [o o x x] | yes | to end of row unless cancelled |
| 01101 | DRCS Character Invocation | [o o x x] | yes | one cell |
| 01110 | Font Style | [o o o x] | yes | to end of row, optionally extended to 0-7 more rows |
| 01111 | G2 character | [o x x x] | yes (also for data < 0x20) | one cell |
| 10000-11111 | G0 character with diacritical mark (10000 = none) | [o x x x] | yes (also for data < 0x20) | one cell |

Cl. 12.3.4 (p.83): for every mode except reserved ones and the PDC mode, the address sets the column.

### 5.1 Mode 00000 — Foreground Colour [V] (p.84)
- Data: D6 D5 must be 00; D4 D3 = CLUT, D2-D0 = entry. Other data values reserved.
- Effect "persists to the end of a display row unless overridden by either a spacing or a non-spacing attribute defining the foreground colour".
- Does not change alphanumeric/mosaic mode (INFERENCE: only spacing colour codes 0/0-1/7 select G0 or G1, Table 26 pp.70, 73).

### 5.2 Mode 00011 — Background Colour [V] (p.84)
Same data format and persistence as Foreground Colour, for the background.

### 5.3 Mode 00001 — G1 block mosaic character [V] (p.84; Table 47 p.118)
- Data selects a G1 character (Table 47). Data < 0x20 reserved, column still set.
- Table 47 NOTE 3: when selected by X/26, positions in columns 4 and 5 (0x40-0x5F) are the corresponding characters of the G0 set currently selected (cl. 15.4 NOTE p.104: triplet-referenced G0 is not modified by national options).
- Separated form is controlled by the non-spacing Underline/Separated attribute (section 5.8), not by the Level 1 spacing codes 1/9 and 1/A (Table 26 NOTE 11, 12 p.73: those act "only on characters defined at Levels 1 and 1.5").

### 5.4 Modes 00010 and 01011 — G3 character [V] (pp.84, 86)
- Both select from G3 (Table 48); data < 0x20 reserved but column set.
- 00010 is the Level 1.5 form; Level 2.5/3.5 decoders must also interpret it (NOTE 1). 01011 is for Levels 2.5/3.5; Level 1.5 decoders ignore it (NOTE 3).
- The underline/separated attribute's effect on G3 is "not defined" (p.87).

### 5.5 Mode 01001 — G0 character, and modes 10000-11111 — G0 with diacritic [V] (pp.85, 88)
- 01001: character from the current G0 set (default, X/28/0 F1, X/28/4, M/29/0, M/29/4, or the latest Modified G0/G2 Designation). Data < 0x20 reserved, column set.
- 10000-11111: mode low 4 bits select a diacritic from G2 column 4 (4/0 = none); data = G0 character; mode 10000 with data 0x2A shows "@". Full detail in `notes-level15.md` section 6.
- National option substitution never applies to triplet-referenced G0 (Table 25 p.67; cl. 15.4 NOTE p.104). ESC (1/B) does not affect them (Table 26 p.73).

### 5.6 Mode 01111 — G2 character [V] (p.88)
From the current G2 set (default, X/28/0 F1, X/28/4, M/29/0, M/29/4, or latest Modified designation). Data < 0x20 reserved, column set.

### 5.7 Mode 00111 — Additional Flash Functions [V] (p.85)
- Data D6 D5 must be 00; other values reserved.
- D1 D0 = mode: 00 steady; 01 normal flash to background colour; 10 flash to background in inverted phase; 11 flash to the corresponding colour of the adjacent CLUT, only between CLUT 0 and 1 or CLUT 2 and 3, same entry (example in the table: CLUT 0 entry 2 with CLUT 1 entry 2).
- D4 D3 D2 = rate and phase: 000 slow, 1 Hz; 001 fast, 2 Hz, phase 1; 010 fast, phase 2; 011 fast, phase 3; 100 fast, incremental, apparent movement to the right, starting at phase 1; 101 fast, decremental, apparent movement to the left, starting at phase 1; 110 and 111 reserved.
- Flash affects the foreground colour only; for 12x10x2, 12x10x4 and 6x5x4 DRCS characters all pixels count as foreground. DRCS flash detail: cl. 14.3 p.98 (in the "off" phase of normal/inverted flash, multi-colour DRCS show a space in the background colour).
- Duration is not stated in the mode text (GAP, A-4). Flashing is one of the four attribute types of A.7.1 (p.123).

### 5.8 Mode 01100 — Display Attributes [V] (pp.86-87)
Bit assignment (data field):
| D6 | D5 | D4 | D3 | D2 | D1 | D0 |
|---|---|---|---|---|---|---|
| Double Width | Underline / Separated Mosaics | Invert Colour | Reserved | Conceal | Boxing / Window | Double Height |

General rule: each bit enables its function when 1; the functions take effect together at the Active Position ("Set-At", C.3 p.129); the action lasts to the end of the row but can be cancelled by a later triplet of this type with the bit at 0, or "in most cases" by an appropriate Level 1 spacing attribute. D0 = D6 = 0 means normal size. D0 and D6 both 1 = double size.

Per bit:
- **Double Height (D0).** If only non-spacing double height/size is used on a row, the row below is not suppressed: everything there keeps its normal effect outside the cells covered by the lower halves, including Level 1 characters and attributes. If a row mixes spacing and non-spacing double height/size, the row below is handled as Table 26 0/D (Level 1 content of the lower row ignored; see section 6.5).
- **Boxing/Window (D1).** With header bit C5 or C6 set: boxing (start of an area inset into video). With neither set: "Window" — the start of an area where the full row colour becomes transparent, so video shows where the foreground or background is also transparent (Table C.1, section 7.8).
- **Conceal (D2).** Characters shown as spaces until revealed by the decoder or user.
- **Reserved (D3).**
- **Invert Colour (D4).** Swaps foreground and background. "It has the priority of the display plane or the object in which it is invoked." No Level 1 equivalent, so only this bit (or end of row) sets or clears it. While in force, a Level 1 foreground change alters the displayed background and vice versa. NOTE 4: may spoil "Mix" display.
- **Underline/Separated (D5).** Alphanumerics of G0, G1 columns 4-5 and G2 are underlined; G1 mosaics (columns 2, 3, 6, 7) are drawn separated; undefined for G3 and DRCS. Lasts until a Display Attributes triplet with D5 = 0 or end of row. When set by a triplet it "cannot be cancelled" by a Level 1 Contiguous Mosaics (1/9). Clearing D5 by a triplet also clears separated mode set by a Level 1 1/A.
- **Double Width (D6).** Characters stretched over the next cell. **Double Size (D0 + D6).** Same row-below rule as double height.

### 5.9 Mode 01101 — DRCS Character Invocation (encoding only) [V] (p.88)
- D5-D0 = character 0-47; D6 = 0 global sub-table, 1 normal sub-table (the one most recently designated by DRCS Mode within the same object or local enhancement data).
- Defaults when no DRCS Mode triplet: global sub-table 0 and normal sub-table 0, at Levels 2.5 and 3.5; assumed at the start of local enhancement data and at each object invocation (also C.4 rule 1 p.130).
- Mode 0 (12x10x1) pixels use the foreground/background colours at the position (cl. 14.2.1 p.97); other modes use DCLUTs (DRCS topic).

### 5.10 Mode 01110 — Font Style (Level 3.5 only) [V] (p.88)
| D6 | D5 | D4 | D3 | D2 | D1 | D0 |
|---|---|---|---|---|---|---|
| R2 | R1 | R0 | Reserved | Italics | Bold | Proportional spacing |
- D0, D1, D2 enable when 1, cancel when 0. The address is the starting column. Lasts to end of row unless another Font Style triplet overrides.
- R2-R0 (D6 MSB) extend the style to the following 0-7 rows "at the same column positions".
- Applies to G0 and G2 characters (Table 25 p.68). No metrics or proportional layout rules are given (A-12).

### 5.11 Mode 01000 — Modified G0 and G2 Character Set Designation [V] (p.85; Table 34 p.105; cl. 15.4 p.104; C.4 p.130)
- Data D6-D0 (bits 18-12) index Table 34, which has the same structure as Table 32 (D6..D0 in the position of Table 32's bits 14..8). Defined entries:
  - 0000xxx, 0001xxx, 0010xxx, 0011xxx: Latin G0 / Latin G2;
  - 0100000 Cyrillic-1/Cyrillic; 0100001-0100011 Latin/Latin; 0100100 Cyrillic-2/Cyrillic; 0100101 Cyrillic-3/Cyrillic; 0100110 Latin/Latin; 0100111 reserved;
  - 0101xxx reserved; 0110000-0110101 reserved; 0110110 Latin/Latin; 0110111 Greek/Greek; 0111xxx reserved;
  - 1000000 Latin/Arabic; 1000001-1000011 reserved; 1000100 Latin/Arabic; 1000101-1000110 reserved; 1000111 Arabic/Arabic; 1001xxx reserved;
  - 1010000-1010100 reserved; 1010101 Hebrew/Arabic; 1010110 reserved; 1010111 Arabic/Arabic; all other values reserved.
- Affects only characters written by X/26 and objects, never the Level 1 page.
- Level 2.5: only two G0/G2 pairs per page, one of them the default pair (from X/28/0 F1, X/28/4, M/29/0, M/29/4 or default). Level 3.5: any number.
- In local enhancement data or Active Objects: applies to later characters of those sets on the same row from that source; lasts until another 01000 triplet or end of row; at row end the default sets are re-selected. In Adaptive or Passive objects: lasts until the last cell of the object or another 01000 triplet in the object (C.4 rule 2; 13.4 property 3 p.92).

### 5.12 Mode 00110 — PDC [V] (p.84)
Coding in EN 300 231; Active Position not modified.

---------------------------------------------------------------------------------------------------

## 6. How non-spacing attributes interact with the Level 1 page

### 6.1 Where an attribute takes effect, and for how long [V]
- Cl. 12.3.1 (p.74): a triplet or group defines a location and the enhancements there; some enhancements also apply to later positions on the same row until explicitly terminated or end of row.
- Non-spacing attributes are "Set-At": they act on the cell where they are placed (C.3 p.129 for size; C.2 example 2 p.129 for colour).
- Local enhancement data behaves like a one-time Active Object (cl. 13.3 p.92). Active Object properties:
  1. Default attributes at the Active Position are those of the Level 1 page as modified by Active Objects and local enhancement data at that location.
  2. Attributes in it act on all characters to the end of the row, whether the character comes from the object or the Level 1 page; later attributes from the object or the Level 1 page may supersede them.
- Adaptive Object (13.4 p.92): starts with the Level 1 attributes (after Active Objects and local data) at its position; changes it makes affect object and Level 1 characters between its first and last addressed cells on the row; right of its last cell, the Level 1 attributes resume. Exception: character set designation persists to the end of the object.
- Passive Object (13.5 p.93): starts at white foreground (CLUT 0 entry 7), black background (CLUT 0 entry 0), normal size, steady, boxing off, not concealed, contiguous; its attributes persist until changed or object end, are not reset by row changes, and affect only cells where it defines a character; underlying attributes do not affect it.
- Display priority (13.2 p.92): Passive > Adaptive > local enhancement data > Active > Level 1 page.

### 6.2 Attribute contention (Annex C.2, pp.128-129) [V]
When two attributes of the same type contend at one location, the latest takes precedence, judged in order:
1. the location where each first takes effect (set-after spacing attributes take effect one cell later than they are placed);
2. if tied, the location where each is placed;
3. if still tied, the relative position in the transmission.

Worked examples in C.2 (all [V]):
- Level 1 alpha-red at n−1 (effect at n), X/26 foreground blue at n−1 (effect at n−1): character at n is red (rule 1).
- Alpha-red at n−1 (effect at n), X/26 foreground blue at n (effect at n): character at n is blue (rule 2: placed later).
- Alpha-red at n−1, X/26/0 foreground blue at n, X/26/1 foreground yellow at n: yellow (rule 3: later in transmission).

Attribute types for contention and overlap (A.7.1 p.123): Foreground Colour; Background Colour; Flashing; Display Attributes (double height, double width, boxing/window, conceal, invert, separated/underline).

INFERENCE for implementation: process each row left to right; at each cell apply set-after spacing attributes from the previous cell, then set-at spacing attributes at this cell, then non-spacing attributes placed at this cell in transmission order. That ordering reproduces all three C.2 examples, except for the unresolved tie in A-7.

### 6.3 Mapping of Level 1 spacing attributes at Levels 2.5/3.5 [V]
- 0/0 Alpha Black and 1/0 Mosaics Black: active at 2.5/3.5 [o o x x] (Table 26 pp.70, 73); black foreground = entry 0 of the foreground CLUT (INFERENCE from the re-mapping rule, section 7.4).
- 0/1-0/7, 1/1-1/7: foreground = entry n of the foreground CLUT selected by re-mapping (Table 4 T13 bits 16-18 p.30-31).
- 0/E Double Width, 0/F Double Size: active at 2.5/3.5 [o o x x] (p.72).
- 1/C Black Background: background = entry 0 of the background CLUT, or the current full row colour if black background substitution is on (p.73).
- 1/D New Background at 2.5/3.5: background CLUT from the re-mapping bits (default CLUT 0); entry = the entry number of the foreground colour "currently selected by the Level 1 page" (p.74). The older rule "foreground colour adopted as background" is [x x o o].
- Any one size code (spacing or non-spacing) ends any other Level 1 spacing or Level 2.5/3.5 non-spacing size attribute (Table 26 NOTE 2, 5, 8 pp.71-72).
- 1/9, 1/A, 1/E, 1/F act only on characters defined at Levels 1 and 1.5 (NOTE 11, 12, 14, 15). Size and character set changes made by enhancement triplets do not cancel Hold Mosaics (NOTE 14 p.74).
- 1/B ESC does not affect X/26 or object characters (p.73).

### 6.4 Size rules (Annex C.3, pp.129-130) [V]
- Size attributes: Normal, Double Height, Double Width, Double Size; spacing or non-spacing.
- Spacing size codes except Normal Size are set-after; non-spacing size and spacing Normal Size are set-at. All act on following characters until superseded or end of row.
- Rules:
  1. Transmissions should not imply partial characters: no double height/size in rows 0, 23, 24; no double width/size taking effect in column 39 or the last column of either side panel. On receiving such a combination a decoder may show a partial character, reduce the size, or ignore the character.
  2. Priority is in scan order (left to right, top to bottom); earlier size attributes beat later ones.
  3. On conflict, drop the doubling of width and/or height of the conflicting character to satisfy rules 1 and 2; reduce a double-size character to normal only if neither double width nor double height alone would work.
  4. All attributes at the origin of an enlarged character apply to all its parts.
  5. Attributes at positions hidden by an enlarged character take effect at the first non-hidden position after it, unless superseded.
- Side panels: double width and double size may not spread from the page into a side panel or back (cl. 12.5.4 rule 3 p.91).

### 6.5 The row below double height [V] (Table 26 0/D p.71, 0/F p.72; Table 29 p.86-87)
- Spacing double height/size on a row (alone or mixed with non-spacing): the next row shows the "local background colour" (the background at the cell above after Level 1 data, Active Objects and local enhancement data) with no foreground; its Level 1 characters and attributes are ignored.
- At 2.5/3.5 [o o x x], enhancement data (local X/26 or objects) may write to cells of that lower row not covered by lower halves, and they are displayed. For Active and Adaptive Objects, and for local X/26 characters, the attributes there are the start-of-row defaults modified only by earlier enhancement data on the row. Cells where enhancement data defines no character are unchanged.
- Non-spacing double height/size only: the row below is not suppressed (section 5.8).

Consequence for this project (INFERENCE): decision E.5(f) (an overwrite on the lower row of a double-height row is not shown) is right for Level 1.5 compatibility but wrong at Levels 2.5/3.5, and E.15 ("transmitted content of that row is ignored entirely") must exclude enhancement data at 2.5/3.5.

### 6.6 Boxing, windows and display planes
See section 7.8 (Table C.1). Annex E.13 (p.136): Start Box/End Box on Level 1/1.5 pages without C5/C6 can make a Level 2.5 decoder open a spurious window. INFERENCE: at 2.5/3.5 the spacing Start Box on a page without C5/C6 acts as the Window function.

### 6.7 Side panels
Default attributes at the start of each side-panel row are the Level 1 start-of-row defaults except that the background is the full row colour (section 8.4).

---------------------------------------------------------------------------------------------------

## 7. Colour model

### 7.1 The Colour Map — Table 30 (p.89) [V]
32 entries in four CLUTs of 8. Each entry is a 4-bit R, G, B (16 equal steps, 0 = zero intensity, 15 = full, "not gamma corrected").

| CLUT | Entry | Default | R | G | B | Status |
|---|---|---|---|---|---|---|
| 0 | 0 | Black | 0 | 0 | 0 | fixed at Levels 1, 1.5, 2.5; redefinable by X/28/4 or M/29/4 at 3.5 |
| 0 | 1 | Red | 15 | 0 | 0 | |
| 0 | 2 | Green | 0 | 15 | 0 | |
| 0 | 3 | Yellow | 15 | 15 | 0 | |
| 0 | 4 | Blue | 0 | 0 | 15 | |
| 0 | 5 | Magenta | 15 | 0 | 15 | |
| 0 | 6 | Cyan | 0 | 15 | 15 | |
| 0 | 7 | White | 15 | 15 | 15 | |
| 1 | 0 | Transparent | – | – | – | valid at 2.5 and 3.5, fixed |
| 1 | 1 | Half red | 7 | 0 | 0 | valid at 2.5 and 3.5; fixed at 2.5; redefinable by X/28/4 or M/29/4 at 3.5 |
| 1 | 2 | Half green | 0 | 7 | 0 | |
| 1 | 3 | Half yellow | 7 | 7 | 0 | |
| 1 | 4 | Half blue | 0 | 0 | 7 | |
| 1 | 5 | Half magenta | 7 | 0 | 7 | |
| 1 | 6 | Half cyan | 0 | 7 | 7 | |
| 1 | 7 | Grey | 7 | 7 | 7 | |
| 2 | 0 | | 15 | 0 | 5 | valid at 2.5 and 3.5; redefinable by X/28/0 F1 or M/29/0 |
| 2 | 1 | | 15 | 7 | 0 | |
| 2 | 2 | | 0 | 15 | 7 | |
| 2 | 3 | | 15 | 15 | 11 | |
| 2 | 4 | | 0 | 12 | 10 | |
| 2 | 5 | | 5 | 0 | 0 | |
| 2 | 6 | | 6 | 5 | 2 | |
| 2 | 7 | | 12 | 7 | 7 | |
| 3 | 0 | | 3 | 3 | 3 | valid at 2.5 and 3.5; redefinable by X/28/0 F1 or M/29/0 |
| 3 | 1 | | 15 | 7 | 7 | |
| 3 | 2 | | 7 | 15 | 7 | |
| 3 | 3 | | 15 | 15 | 7 | |
| 3 | 4 | | 7 | 7 | 15 | |
| 3 | 5 | | 15 | 7 | 15 | |
| 3 | 6 | | 7 | 15 | 15 | |
| 3 | 7 | | 13 | 13 | 13 | |

The CLUT 2 and 3 values were read from the page image; they agree with the default palette used by widely deployed decoders (INFERENCE: cross-check with an oracle at test time). Converting to 8 bits: `v * 17` (INFERENCE from "16 equally spaced steps").

### 7.2 Who may change which CLUT [V]
- CLUTs 2 and 3: X/28/0 F1 (page) or M/29/0 (magazine), Levels 2.5 and 3.5 (Table 25 p.69; cl. 10.3.2 p.48).
- CLUTs 0 and 1 (not CLUT 1 entry 0): X/28/4 (page) or M/29/4 (magazine), Level 3.5 only (Table 25 p.69; cl. 10.3.4 p.48; Table 10 p.35).
- X/28/0 F1 has precedence over X/28/4 "for all but the colour map entry coding" (cl. 9.4.2.2 p.29; cl. 9.4.7 p.34): the two packets define different CLUTs, so both apply.
- Page packets override magazine packets of the same kind: M/29/0 is overridden by the page's X/28/0 F1; M/29/4 by the page's X/28/4 (cl. 9.5.1 p.35; cl. 9.5.3 p.37). Where M/29/0 and M/29/4 both exist, M/29/0 has precedence (for everything except the CLUTs, INFERENCE as above).

### 7.3 Colour Map entry coding in X/28/0 F1, X/28/4, M/29/0, M/29/4 [V] (Table 4 p.30; Table 10 p.35)
- 16 words × 12 bits = 192 bits in T2 bits 11-18 (8 bits), T3-T12 bits 1-18 (180 bits), T13 bits 1-4 (4 bits).
- Words in transmission order: X/28/0 F1 and M/29/0 = CLUT 2 entries 0-7, then CLUT 3 entries 0-7; X/28/4 and M/29/4 = CLUT 0 entries 0-7, then CLUT 1 entries 0-7 (CLUT 1 entry 0 ignored).
- Each word in transmission order is R (4 bits), G (4 bits), B (4 bits), each nibble least significant bit first ("RRRRGGGGBBBB, with ascending order of bit significance within each 4 bits").
- Extraction: build a bit stream `s[0..191]` with `s[0..7]` = T2 bits 11-18, `s[8..187]` = T3 bit 1 ... T12 bit 18, `s[188..191]` = T13 bits 1-4. Word k (0-15): `R = Σ s[12k+i]·2^i (i=0..3)`, `G = Σ s[12k+4+i]·2^i`, `B = Σ s[12k+8+i]·2^i`.

### 7.4 Colour Table Re-mapping (T13 bits 16-18) [V] (Table 4 pp.30-31)
Maps the Level 1 spacing colour attributes to other CLUTs, foreground and background independently; the 3 low bits of the spacing colour code give the entry.
| b18 b17 b16 | Foreground CLUT | Background CLUT |
|---|---|---|
| 000 | 0 | 0 |
| 001 | 0 | 1 |
| 010 | 0 | 2 |
| 011 | 1 | 1 |
| 100 | 1 | 2 |
| 101 | 2 | 1 |
| 110 | 2 | 2 |
| 111 | 2 | 3 |
NOTE (p.31): with Black Background Colour Substitution on, a Level 1 black background (entry 0) is re-mapped only if it came from 0/0 or 1/0 followed by 1/D.

INFERENCE: with background CLUT 1, a Level 1 black background maps to CLUT 1 entry 0, which is transparent, so the row colour plane shows through (section 7.8). Whether the start-of-row default colours are re-mapped is A-14.

### 7.5 Black Background Colour Substitution (T13 bit 15) [V] (Table 4 p.30)
- 0: no substitution (the black background may still be re-mapped).
- 1: on any row where the Level 1 page shows a black background because of the start-of-row default or 1/C Black Background, the background is replaced by that row's full row colour, independently of re-mapping. Not for black obtained by 0/0 or 1/0 followed by 1/D.
- Where a background colour is an input to another function (re-mapping, colour-table flash, windows), it must be set explicitly by the transmission and not rely on substitution.
- Explicit X/26 background colours are not substituted (INFERENCE: the bit names only the start-of-row default and 1/C).

### 7.6 Screen and row colours: defaults and precedence [V]
- X/28/0 F1 T13 bits 5-9: Default Screen Colour, the screen area above row 0 and below row 23 (or 24). Bits 10-14: Default Row Colour, rows 0-23 (and 24). X/26 selections take priority over both (Table 4 p.30).
- A.5 (p.122), screen colour, highest first: Full Screen Colour triplet; page packet 28 default; magazine packet 29 default; black.
- A.5, row colour, highest first: Full Row Colour triplet (latest wins); "The screen colour already invoked"; page packet 28 default row colour; magazine packet 29 default row colour; black (see A-1).
- D.2.1 (p.133), no M/29/0 and no X/28/0 F1: screen and row colour black (Colour Map entry 0); CLUTs 2, 3 from Table 30; no re-mapping (CLUT 0 for both); side panels and black-background substitution from the page's POP link in the MOT, else none.
- Table 25 (p.69): default colour is CLUT 0 entry 0, black at 2.5, redefinable at 3.5; screen/row colour also applies to side panels.

### 7.7 Defaults for DCLUTs and CLUTs 0/1 (Level 3.5) [V] (D.1.6, D.1.8, D.2.2, D.2.3 pp.132-133)
- No X/28/4 and no M/29/4: CLUTs 0 and 1 hold the Table 30 values, including transparent at CLUT 1 entry 0.
- No X/28/1 and no M/29/1: DCLUT4 (global and normal) = Colour Map entries 0-3 (CLUT 0); DCLUT16 (global and normal) = entries 0-15 (CLUTs 0 and 1, including transparent).

### 7.8 Display planes and transparency — Annex C.1, Table C.1 (p.128) [V]
Planes, highest first: foreground/background; full screen/full row colour; video. Transparent (CLUT 1 entry 0) in a higher plane exposes the plane below.

Table C.1, result for one pixel. "F/B" is the pixel's foreground or background colour; T = transparent, O = any other colour; Box = inside a boxed/windowed area; NS = header C5 or C6 set.
| F/B | Row colour | Box | NS | Result |
|---|---|---|---|---|
| O | O | 0 | 0 | F/B colour |
| O | O | 0 | 1 | Video |
| O | O | 1 | 0 | F/B colour |
| O | O | 1 | 1 | F/B colour |
| O | T | 0 | 0 | F/B colour |
| O | T | 0 | 1 | Video |
| O | T | 1 | 0 | F/B colour |
| O | T | 1 | 1 | F/B colour |
| T | O | 0 | 0 | Row colour |
| T | O | 0 | 1 | Video |
| T | O | 1 | 0 | Video |
| T | O | 1 | 1 | Row colour |
| T | T | any | any | Video |

---------------------------------------------------------------------------------------------------

## 8. Side panels (cl. 12.5, pp.89-91) [V]

### 8.1 Basics
- Levels 2.5 and 3.5 decoders "may" show 56 characters per row; side panels are optional for a decoder ("not mandatory"). 56 characters are also valid on 4:3 screens.
- Side-panel display data is sent via objects only, for compatibility (cl. 12.5 p.89; 12.5.3 p.90). Table 25 says "via packets X/26 and Objects" (A-11).
- Rows 1-24 only; nothing for row 0, whose layout over the wider display is the decoder's choice (cl. 12.5.2; Table 25).
- "Sixteen character positions are always available once a Side Panel is enabled for display" (12.5.2) (A-10).

### 8.2 Enable, position and width
Carried in X/28/0 F1 or X/28/4 (page), or M/29/0 or M/29/4 (magazine); otherwise limited information from the MOT POP link; otherwise 40 columns only (12.5.1 p.90).
- T2 bit 4: left side panel displayed (1) or not.
- T2 bit 5: right side panel displayed (1) or not.
- T2 bit 6: Side Panel Status Flag: 0 = side panels required at Level 3.5 only; 1 = at Levels 2.5 and 3.5.
- T2 bits 7-10 (LSB first): number of columns in the left panel. If the right panel is shown its width is 16 minus this value. With only one panel in use, 0 means 16 columns.

Derived widths (INFERENCE where noted):
| Left bit | Right bit | n = bits 7-10 | Left width | Right width |
|---|---|---|---|---|
| 0 | 0 | any | 0 | 0 |
| 1 | 0 | 0 | 16 | 0 |
| 1 | 0 | 1-15 | n | 0 (INFERENCE) |
| 0 | 1 | 0 | 0 | 16 |
| 0 | 1 | 1-15 | 0 | 16 − n (INFERENCE from "16 minus this value") |
| 1 | 1 | 1-15 | n | 16 − n |
| 1 | 1 | 0 | GAP (A-10) | GAP |

MOT fallback (Table 21 byte N+4, p.54; POP links only, reserved in GPOP links):
- bit 1: 1 = default is no side panel and no black-background substitution; 0 = defaults from bits 2-4;
- bits 3, 2: 00 none; 01 left, 16 columns; 10 right, 16 columns; 11 left and right, 8 columns each;
- bit 4: 0 = no black-background substitution; 1 = black background replaced by full screen/row colour.

### 8.3 Column addressing (12.5.3 p.90)
- Normal page: columns 0-39.
- Right panel: columns 40-55, first column always 40; fewer columns if a left panel is also used.
- Left panel: columns 56-71, last column always 71; its first column depends on the width of any right panel. INFERENCE: left panel columns = 72 − left width ... 71.
- Examples given: full right panel = 40-55; full left panel = 56-71; split 8 + 8 = left 64-71, right 40-47.
- Objects reach these columns through the Origin Modifier column offset (0-71); unless an Origin Modifier adjusts them, the row addresses inside the object place elements vertically within the panel.
- An object may not cross the page/side-panel boundary or address more than one display area (Table 25 p.68; A.4 p.121; 12.5.4 rule 4).
- Screen address order for A.4: 0-39, then 40-55, then 56-71.
- Any X/26 character or non-spacing attribute may be used in side panels, except screen and row colour selection (12.5.3).

### 8.4 Display rules (12.5.4 p.91)
1. Each side-panel row starts with the Level 1 start-of-row defaults (white CLUT 0 entry 7, normal size, steady, boxing off, not concealed, contiguous), except that the background defaults to the full row colour, not black. Every cell defaults to SPACE (G0 0x20) until written.
2. Attributes at the end of a left panel do not carry into the normal page; a right panel does not inherit attributes from the normal page.
3. No double width/size spreading between page and panel.
4. No object spreading between page and panel, or between panels.
5. Header bits C5, C6, C7 and C10 of the page also govern the side panels.
6. With no side panels, a 4:3 decoder should keep the correct aspect ratio.

---------------------------------------------------------------------------------------------------

## 9. Page-level packets, field by field

General: all are Hamming 8/4 designation code plus 13 Hamming 24/18 triplets (cl. 9.4 Figure 11 p.27). `T1`..`T13` below are the triplets' 18-bit values; "bits a-b" are spec bit numbers within the triplet (1 = LSB).

### 9.1 Page Function and Page Coding — Table 3 (p.29) [V]
Used as T1 bits 1-7 of X/28/0 F1, X/28/3 and X/28/4.
- Bits 1-4 Page Function (bit 4 MSB): 0000 basic Level 1 page (LOP); 0001 data broadcasting (EN 300 708); 0010 GPOP; 0011 POP; 0100 GDRCS; 0101 DRCS; 0110 MOT; 0111 MIP; 1000 BTT; 1001 AIT; 1010 MPT; 1011 MPT-EX; 1100 trigger messages; others reserved.
- Bits 5-7 Page Coding (bit 7 MSB): 000 7 data bits + odd parity; 001 8 data bits; 010 per packet one Hamming 8/4 byte then 13 Hamming 24/18 triplets; 011 all Hamming 8/4; 100 per packet 8 Hamming 8/4 bytes then 12 odd-parity bytes, repeated for the remaining 20 bytes; 101 per packet first byte Hamming 8/4 defining the coding of the other 39 bytes per the first five entries; others reserved.

### 9.2 X/28/0 Format 1 for basic Level 1 pages — Table 4 (pp.30-31) [V]
Applies when Page Function = 0000 (cl. 9.4.2.2 p.29).
| Triplet | Bits | Field | Code |
|---|---|---|---|
| 1 | 1-4 | Page Function = 0000 | `T1 & 0xF` |
| 1 | 5-7 | Page Coding = 000 | `(T1 >> 4) & 7` |
| 1 | 8-14 | Default G0/G2 designation and national option (Table 32; bit 14 MSB; bits 10, 9, 8 = C12, C13, C14) | `(T1 >> 7) & 0x7F` |
| 1-2 | T1 15-18, T2 1-3 | Second G0 designation and national option (Table 33; T2 bit 3 MSB, T1 bit 15 LSB) | `((T1 >> 14) & 0xF) \| ((T2 & 7) << 4)` |
| 2 | 4 | Left side panel | `(T2 >> 3) & 1` |
| 2 | 5 | Right side panel | `(T2 >> 4) & 1` |
| 2 | 6 | Side Panel Status Flag (0 = 3.5 only, 1 = 2.5 and 3.5) | `(T2 >> 5) & 1` |
| 2 | 7-10 | Number of left-panel columns, LSB first | `(T2 >> 6) & 0xF` |
| 2-13 | T2 11-18, T3-T12, T13 1-4 | CLUT 2 and 3 entries (section 7.3) | |
| 13 | 5-9 | Default Screen Colour (5-bit index) | `(T13 >> 4) & 0x1F` (significance: A-13) |
| 13 | 10-14 | Default Row Colour (5-bit index) | `(T13 >> 9) & 0x1F` |
| 13 | 15 | Black Background Colour Substitution | `(T13 >> 14) & 1` |
| 13 | 16-18 | Colour Table Re-mapping (bit 18 MSB) | `(T13 >> 15) & 7` |
No checksum or "do not use" fields; there are no reserved bits in this layout.

### 9.3 X/28/0 Format 1 for other pages — Tables 5 and 6 (p.31) [V]
- Page Function 0001 (data broadcasting): T1 bits 8-18 = 11111111100 (bits 8 to 18) so EN 300 708 decoders ignore the page; T2-T13 defined by the application.
- Any other non-LOP function: T1 bits 8-18 = 11111111100; T2-T13 reserved.
- X/28/0 Format 2 (Table 7 p.32) is EN 300 708 Page Format - CA: T1 bits 1-8 function, 9-14 zero, 15-18 coding; T2-T13 reserved. Not used by the presentation levels.

### 9.4 X/28/1 — Table 8 (p.33) [V]
| Triplet | Bits | Field |
|---|---|---|
| 1 | 1-2 | '00' |
| 1 | 3-9 | Character Set Code for G0 (earlier specifications; Level 1.5 only) |
| 1 | 10 | '0' |
| 1 | 11-17 | Character Set Code for G1 (earlier specifications) |
| 1 | 18 | '0' |
| 2-3 | T2 1-18, T3 1-2 | DCLUT4 for global 12x10x2 DRCS: 4 words × 5 bits, LSB first |
| 3-4 | T3 3-18, T4 1-4 | DCLUT4 for normal 12x10x2 DRCS: 4 × 5 bits |
| 4-8 | T4 5-18, T5-T7, T8 1-12 | DCLUT16 for global 12x10x4 and 6x5x4 DRCS: 16 × 5 bits |
| 8-13 | T8 13-18, T9-T12, T13 1-2 | DCLUT16 for normal 12x10x4 and 6x5x4 DRCS: 16 × 5 bits |
| 13 | 3-18 | Reserved |
- Triplet 1 is "not intended for use by Level 2.5 and 3.5 decoders" (Table 8 NOTE). DCLUTs are Level 3.5 only (cl. 10.3.3 p.48). Each 5-bit DCLUT entry is a Colour Map index 0-31 (Figure 28 p.99). The DCLUT use belongs to the DRCS topic.
- Bit-stream extraction (INFERENCE from the bit ranges): concatenate T2 bit 1 ... T13 bit 2 (20 + 20 + 80 + 80 = 200 bits) and take 5-bit LSB-first words in order: 4 global DCLUT4, 4 normal DCLUT4, 16 global DCLUT16, 16 normal DCLUT16.

### 9.5 X/28/3 — Table 9 (p.34) [V] (DRCS topic owns the detail)
- DC = 0011, on DRCS downloading pages, Levels 2.5 and 3.5.
- T1 bits 1-7: Page Function 0100 or 0101, Page Coding 000. T1 bits 8-18 reserved.
- T2-T11 bits 1-18 and T12 bits 1-12 (192 bits): 48 Mode Identification codes of 4 bits, one per PTU: 0000 mode 0 (12x10x1), 0001 mode 1 (12x10x2), 0010 mode 2 (12x10x4), 0011 mode 3 (6x5x4), 1110 subsequent PTU of a mode 1 or 2 character, 1111 no data; others reserved.
- T12 bits 13-18 and T13 reserved.
- Absent X/28/3 on a DRCS page: 12x10x1 only (D.1.7 p.132; cl. 14.1 p.96).

### 9.6 X/28/4 — Table 10 (p.35) [V]
- DC = 0100, Level 3.5. T1 bits 1-7 page function and coding (Table 3). Remaining layout identical to Table 4, except the 16 colour words are CLUT 0 entries 0-7 then CLUT 1 entries 0-7; CLUT 1 entry 0's bits are ignored (always transparent).
- X/28/0 F1 has precedence over X/28/4 for every field except the colour map.

### 9.7 M/29/0 — Table 11 (p.36) [V]
- T1 bits 1-7 Packet Function: 0000000 = rest coded as X/28/0 F1, applying to all pages in magazine M; other values reserved (INFERENCE: ignore the packet).
- T1 bits 8-14 default G0/G2; T1 15-18 + T2 1-3 second G0; T2 4, 5, 6, 7-10 side panels; T2 11-18, T3-T12, T13 1-4 CLUTs 2 and 3; T13 5-9 screen colour, 10-14 row colour, 15 black background substitution, 16-18 re-mapping.
- Overridden for a page by that page's X/28/0 F1 (cl. 9.5.1). Table 32 NOTE 1: bits 8-10 should be the magazine's most common C12-C14.

### 9.8 M/29/1 — Table 12 (p.36) [V]
Same coding as X/28/1 (T1 earlier-specification character set codes; T2-T13 bit 2 DCLUTs; T13 3-18 reserved), for all pages in the magazine unless the page has X/28/1.

### 9.9 M/29/4 — Table 13 (p.37) [V]
- T1 bits 1-7 Packet Function 0000000 = rest coded as X/28/4, for all pages of the magazine; other values reserved.
- Rest as M/29/0 but CLUTs 0 and 1 (CLUT 1 entry 0 ignored). Overridden for a page by that page's X/28/4. M/29/0 has precedence over M/29/4.

### 9.10 Precedence summary
- Character sets (C.4 rule 1 p.130): X/28/0 F1, then X/28/4 (3.5 only), then M/29/0, then M/29/4 (3.5 only), then Latin (D.2.1). Header C12-C14 and Table 32 bits 8-10: see Annex E.9 and `notes-level15.md` section 9.
- Side panels, screen/row colours, substitution, re-mapping: X/28/0 F1 over X/28/4; X/28/0 F1 over M/29/0; X/28/4 over M/29/4; M/29/0 over M/29/4; then the MOT fallback (side panels, substitution only); then the D.2.1 defaults. The relative order of a page's X/28/4 and a magazine's M/29/0 is not stated outside character sets (A-15).
- CLUTs 2/3: X/28/0 F1 > M/29/0 > Table 30. CLUTs 0/1: X/28/4 > M/29/4 > Table 30.
- DCLUTs: X/28/1 > M/29/1 > D.2.2 defaults.

---------------------------------------------------------------------------------------------------

## 10. Effects on existing project decisions (INFERENCE, for the owner)
- Decision E.1 (Level 1.5 as a pass over the decoded Level 1 grid): at Levels 2.5/3.5 non-spacing attributes change the Level 1 cells' attributes along the row (C.2, 13.3), so the 2.5 pass must re-run attribute resolution per row with both spacing and non-spacing events, not only overwrite glyphs.
- Decision E.3 (row from any address 40-63): must be the strict 12.3.2 rule at 2.5/3.5 (section 2.3).
- Decision E.4 (national option from header only): at 2.5/3.5, Annex E.9 allows decoders to use bits 8-10 of the highest-priority X/28 or M/29; Annex E.9 recommendation 3 says broadcasters keep them equal to C12-C14.
- Decision E.5(c): at 2.5/3.5, G1 characters written by X/26 are mosaics (section 5.3), and G0/G2 characters can be underlined (section 5.8).
- Decisions E.5(f) and E.15: at 2.5/3.5, enhancement characters on the lower row of a double-height row are displayed (section 6.5).
- Decision E.12 (flash 0.75 Hz, 3:1 from the SAA5050): Table 29 defines a slow rate of 1 Hz and a fast rate of 2 Hz for X/26 flash (A-5).
- REQ-SEC-005 (row 0 columns 0-7 belong to the receiver): Address Display Row 0 can colour row 0 (section 4.4), and side panels widen row 0 (section 8.1); both touch that area.

---------------------------------------------------------------------------------------------------

## 11. Annex and clause material useful for tests [V]
- **C.2 contention examples** (p.129): three cases with expected colours (section 6.2). Directly usable as unit tests for attribute resolution.
- **Cl. 13.9 object example** (pp.94-95): one Level 1 row "αB THE FAST DOG AND αR CAT αG RAN IN" overwritten by an object writing LAZY over FAST, COW over DOG (with Foreground Pink at C), FOX over CAT, SAT over RAN (with Foreground Yellow at S). Expected foreground per word and object type:

  | Word | Active | Adaptive | Passive |
  |---|---|---|---|
  | THE | Blue | Blue | Blue |
  | LAZY | Blue | Blue | White (passive starts at defaults) |
  | COW | Pink | Pink | Pink |
  | AND | Pink (object colour reaches the Level 1 word in the gap) | Pink | Blue (gap not affected by passive) |
  | FOX | Red (object text adopts the Level 1 αR) | Pink (later Level 1 colour changes ignored inside the adaptive object) | Pink (own attribute set) |
  | SAT | Yellow | Yellow | Yellow |
  | IN | Yellow (active attribute continues past the object) | Green (adaptive ends at its last cell) | Green |

  Notes 1-8 on p.95 give the reason for each cell. "Pink" here means a non-spacing foreground colour; the example does not give its CLUT index.
- **Table C.1** (p.128): a 16-row truth table for plane composition (section 7.8).
- **A.5** precedence lists (p.122) for screen and row colour.
- **Cl. 12.5.3 side-panel examples** (p.90): three column layouts (section 8.3).
- **Figure A.1** (p.124): the "open area" of an object, including a case with an Origin Modifier into the right side panel (objects topic).
- **Figure 28** (p.99): an example DCLUT assignment from DRCS bit planes to the 32-entry Colour Map (DRCS topic).
- **Table 30**: exact default RGB values (section 7.1).
- No annex gives a rendered Level 2.5 page, a byte-level X/26 or X/28 test vector, or a flash timing diagram.

---------------------------------------------------------------------------------------------------

## 12. Ambiguities, contradictions and gaps (owner decisions)

A-1. **Row colour precedence (A.5 vs Table 4).** A.5 puts "The screen colour already invoked" above the packet 28 default row colour; Table 4 says only X/26 row selections beat the default row colour. If "screen colour already invoked" included the X/28 default screen colour, the default row colour field could never apply. Options: (a) only an explicit Full Screen Colour triplet makes rows default to the screen colour; (b) ignore item 2 of the A.5 row list.

A-2. **Left and right screen borders.** Full Screen Colour and the default screen colour cover the area "above row 0 and below row 23 (or 24)". Nothing says what colours the area left and right of the rows (and of the side panels). Options: screen colour, or each row's row colour.

A-3. **"Down to the bottom" row colour overridden by a "this row only" command.** Full Row Colour S1S0 = 11 lasts "unless subsequently overridden by a further Full Row Colour command on a subsequent row". Whether a later S1S0 = 00 on a lower row also stops the fill below it is not stated. Natural reading: commands apply in order; S1S0 = 00 changes only its row.

A-4. **Duration of Additional Flash Functions.** Table 29 gives no persistence rule for mode 00111 (foreground and background colour say "to the end of a display row"). Cl. 12.3.1 and A.7.1 imply to end of row or until another flash attribute (spacing 0/8 or 0/9, or non-spacing).

A-5. **Flash rates and phases.** Slow 1 Hz and fast 2 Hz are given, but not the duty cycle, the meaning of phases 1-3, how "incremental"/"decremental" assign phases to successive characters, what rate and mode Level 1 Flash 0/8 uses at Level 2.5, or how 1 Hz relates to decision E.12 (0.75 Hz, 3:1). "Flash to background colour" for Level 1 and X/26 normal flash presumably share timing (INFERENCE).

A-6. **Display Attributes bits at 0.** A triplet sets all six functions at once, with 0 meaning "off". Whether a 0 bit cancels an attribute that the Level 1 page set (e.g. spacing Conceal, spacing Double Height, Start Box) is implied for underline/separated (stated) and size (NOTE 2 to 0/D), but not stated for conceal and boxing. Also, "in most cases" a spacing attribute can cancel a non-spacing one, without listing which. Options: treat each display attribute as its own attribute type under C.2 (latest wins), or treat the whole group as one type (A.7.1 groups them).

A-7. **C.2 rule 3 between Level 1 and X/26.** When a set-at spacing attribute (1/8, 1/C, 1/D, 0/9, 0/C, 1/9, 1/A) and a non-spacing attribute are at the same cell, rule 3 uses transmission order, and B.2 sends X/26 before X/1-X/25, so the Level 1 attribute would always win. The C.2 examples never cover this case. Options: Level 1 wins (literal), or enhancement wins (consistent with 13.2 priority).

A-8. **Conceal interplay.** Level 1 conceal ends at a colour code (Table 26 1/8); non-spacing conceal lasts until cleared or end of row. Whether a Level 1 colour code ends a non-spacing conceal is not stated.

A-9. **Set Active Position with data ≥ 40.** The column is "reserved"; whether the column is kept, set to 0, or the triplet's column part is ignored is not said.

A-10. **Side panel widths.** (a) Both panels enabled with n = 0 is undefined (left width 0, right 16?). (b) Only-left with n ≠ 0 and only-right with n ≠ 0 are inferred. (c) Cl. 12.5.2 says sixteen extra positions are "always available" once a side panel is enabled: is the display always 56 wide (unused panel columns blank, in which colour?), or 40 + actual widths?

A-11. **Side-panel data source.** Cl. 12.5 and 12.5.3 say side-panel data comes only via objects; Table 25 says "via packets X/26 and Objects". Local X/26 cannot address columns ≥ 40 without an Origin Modifier, which only applies to an object invocation, so the clauses agree in practice (INFERENCE). Out-of-range results (Active Position column + offset > 71, row + offset > 24) are undefined.

A-12. **Font style.** No glyph metrics, spacing algorithm or interaction with the fixed grid, attributes, double width or mosaics are given for proportional spacing; "at the same column positions" for the row extension is not explained (from that column to the end of each following row, INFERENCE).

A-13. **Bit significance of Default Screen Colour and Default Row Colour.** Table 4 does not say LSB/MSB for T13 bits 5-9 and 10-14 (it does for bits 7-10 of T2). INFERENCE: lowest bit number is least significant, matching every other multi-bit field.

A-14. **Re-mapping of start-of-row defaults.** Re-mapping is defined for "spacing colour attributes". Whether the start-of-row default white foreground and black background are also re-mapped is not stated (the black-background NOTE implies the black background is affected unless substitution is on). Side-panel rule 1 and Passive Objects name CLUT 0 entry 7 explicitly.

A-15. **Precedence between page X/28/4 and magazine M/29/0** for side panels, colours, substitution and re-mapping. Only character sets have a full order (C.4 rule 1: X/28/0, X/28/4, M/29/0, M/29/4).

A-16. **Cl. 9.4.7 text reverses the CLUTs.** It says X/28/4 "redefines CLUTs 2 and 3 instead of CLUTs 0 and 1"; Table 10, cl. 10.3.4, Table 25 and Table 30 all say X/28/4 defines CLUTs 0 and 1. Table 10 is taken as correct. Cl. 9.4.7 also says X/28/4 carries "Level 2.5 and 3.5" data while Table 1 marks it 3.5 only.

A-17. **"All 32 colours are redefinable" at Level 3.5** (cl. 12) conflicts with CLUT 1 entry 0 being fixed transparent (Table 10, Table 30). 31 are redefinable.

A-18. **Default screen/row colour "black".** A.5 and D.2.1 say black; Table 25 and D.2.1 itself say "Colour Map entry 0", which at 3.5 may have been redefined by X/28/4 or M/29/4. Options: use entry 0 (follows Table 25) or literal black.

A-19. **Figure 28 numbers the CLUTs 1-4** (indices 0-7 "CLUT1" … 24-31 "CLUT4") while Table 30 uses 0-3; its caption says "Defined by X/28/0 (16-31) and X/28/4 (0-15)", consistent with Table 30 once renumbered.

A-20. **Table 8 copy error.** The "DCLUT16 for Normal" row's text says it defines the DCLUT "for use with global 12x10x4 and 6x5x4 DRCS". The heading says normal; the heading is taken as correct.

A-21. **X/28/0 Format 1 versus Format 2.** No bit distinguishes them; Table 1 NOTE 1 says the page type decides. INFERENCE: on pages intended for display, treat X/28/0 as Format 1.

A-22. **X/28/3 Mode Identification bit order.** Table 9 shows the 4-bit codes "(MSB LSB)" but not their transmission order; the general LSB-first rule (cl. 8) is assumed.

A-23. **Errors in X/28, M/29 and X/26.** Annex D explicitly does not cover packets lost or corrupted by errors. Whether a partly uncorrectable X/28/0 F1 still "exists" for precedence, and what to do with an uncorrectable triplet in local enhancement data, are not stated.

A-24. **Staleness of M/29.** M/29 applies to every page of the magazine and must be re-sent at least every 20 s (B.6 rule 1), but nothing says when a decoder should discard a stored M/29 (for example after a channel change or a timeout).

A-25. **Address Display Row 0 with S1 ≠ S0.** Reserved. The triplet has no "address only, no colour" value. INFERENCE: S1 S0 = 01 or 10 positions to row 0 without setting a colour. The same applies to Full Row Colour (row and column are still set "for all" cases per 12.3.2).

A-26. **Origin Modifier not followed by an invocation.** Behaviour undefined (INFERENCE: discard the offsets at the next non-invocation triplet).

A-27. **Modified G0/G2 designation beyond the Level 2.5 limit.** What a 2.5 decoder does with a third pair is not stated.

A-28. **Window function on ordinary pages.** Annex E.13 warns that Level 1 Start Box on non-newsflash, non-subtitle pages can open a "spurious window" at Level 2.5, implying spacing boxes act as windows at 2.5. Whether the receiver should do so (it shows video only where foreground or background is transparent, which a Level 1-only page never selects unless re-mapping to CLUT 1) is an owner choice.

A-29. **Underline and separated at Level 1 versus X/26.** Non-spacing D5 changes Level 1 mosaics to separated and underlines Level 1 text, but Level 1 1/9 and 1/A do not affect X/26 characters (NOTE 11, 12). The asymmetry is stated; the rendering of underline (position, thickness) is a decoder option (G.2 p.140).

A-30. **Row 0 and receiver-owned columns.** Address Display Row 0 can set a row colour for row 0, and side panels widen row 0, while Annex E.7(1) and REQ-SEC-005 reserve columns 0-7. The standard leaves the row 0 layout with side panels to the manufacturer (Table 25; 12.5.2).
