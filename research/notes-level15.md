# ETSI EN 300 706 V1.2.1 (2003-04): Presentation Level 1.5 notes

Source: EN_300_706_v1.2.1_Enhanced_Teletext.pdf. **PDF page number = printed ETSI page number** (checked on pp. 21-25, 46, 67-88, 101-119). "p.N" below means both.
Level tick marks were read from the PDF page images, not from the text extraction. The legend in Tables 1/25/26/28/29 is: filled circle = applies at that level, open circle = does not.
Tick notation used here: `[1 1.5 2.5 3.5] = [x x x x]`, where x = ticked and o = not ticked.

---------------------------------------------------------------------------------------------------

## 1. What Level 1.5 is

### 1.1 Definitions (verbatim)
- Scope/intro (p.10 area, text line ~491): "Level 1.5: - extends the character repertoire;"
- Intro, line ~508: "Level 1.5 has evolved in the field due to the language requirements in certain countries and is documented here for the first time."
- Clause 12 (p.66): "Presentation Level 1.5: decoder responds as Level 1 but the character repertoire is extended via packets X/26."
- Clause 15.1 (p.100): "Where the local language requirements require more than 96 alphanumeric characters to provide a basic service, additional packets X/26 may be introduced to form Level 1.5 transmissions. Typically these incorporate a few characters from the G2 supplementary set of characters, plus a few G0 characters with diacritical marks."
- Clause 10.1 (p.47): "At presentation Level 1.5, only a limited number of functions from packets X/26 and X/28/1 are used:
  - X/26/0 - 15: Accessing G0 characters with diacritical marks from the G2 set. A few characters from G2 and G3 sets, depending on local language requirements and Codes of Practice, may also be accessed. Packets X/26 are used also for PDC (see EN 300 231 [1]).
  - X/28/1: Character set and national option designation (for compatibility with earlier specifications only).
  - NOTE: Some existing Level 1.5 decoders only respond to packets X/26 with designation codes in the range 0 to 14."
- Figure 17 (p.46), "System components for presentation Levels 1 and 1.5": Level 1.5 Page = LOP plus "Level 1.5 enhancements" X/26/0 … X/26/15 and X/28/1. Broadcast Service Data = 8/30 Format 1 and 8/30 Format 2. **"X/29/1 Optionally"** (this is M/29/1).
- Annex E.3 (p.134): "The packet X/26 content of Level 1.5 transmissions is determined by local Codes of Practice, subject to the local language requirements."

### 1.2 Table 1 (packet types), pp.23-24: level ticks relevant to 1.5
| Packet | Function | [1 1.5 2.5 3.5] |
|---|---|---|
| X/0, X/1-X/23 display, X/24 FLOF, X/25 labels, 8/30 F1 and F2, X/27/0 | as Level 1 | [x x x x] |
| X/26/0-14 (note 1) | "Used to carry codes for programming ancillary equipment such as video recorders, EN 300 231" (PDC) | [x x x x] |
| X/26/0-14 (second row) | "Used to address character locations within a page and define new characters to be written to these locations. This has the action of overwriting the character defined for this location on the Level 1 page. A Level 1.5 decoder may respond to some or all of the column address group triplets (see clause 12.3.4) which access the G0, G2 and G3 character sets." | **[o x o o]** |
| X/26/0-15 | full enhancement (side panels, G0/G1/G2/G3, DRCS, non-spacing attributes, objects) | [o o x x] |
| X/28/0 Format 1 (page function/coding) | | [x x x x] (note 3: not mandatory) |
| X/28/0 Format 1 (presentation: char set designation, side panels, CLUT, colours, remap) | | **[o o x x]** |
| X/28/1 (char set designation "according to earlier specifications", note 4) | | **[o x o o]** |
| X/28/1 (DCLUTs) | | [o o o x] |
| M/29/0 | | **[o o x x]** |
| M/29/1 (char set designation, earlier specs; applies to magazine unless overridden by X/28/1) | | **[o x o o]** (note 4) |
| M/29/4 | | [o o o x] |

Table 1 NOTE 4: "Function superseded by the present document."
Note: the X/26/0-14 character-overwrite row covers designation codes 0-14 only, while clause 10.1 says "X/26/0 - 15". See also A.3 and E.3 on X/26/15.

### 1.3 Table 25 (display components), pp.67-68: ticks relevant to 1.5
- Character Bytes, second row [o x x x]: "In countries where Level 1 decoders exist, a suitable fall back character should be transmitted in character locations to be overwritten by data from packets X/26. In countries where there are no Level 1 decoders, an even parity character may be transmitted in these character locations."
  - Implication (not stated as a decoder rule, INFERENCE): a Level 1.5 decoder must still show the X/26 character when the Level 1 byte at that cell fails parity.
- G0 set, [x x x x]: national option substitution "only occurs when a G0 set character is referenced from packets X/1-X/25 of the Level 1 page. It does not apply when characters are referenced via packets X/26 or objects."
- G0 designation via X/28/1 and/or M/29/1, [o x o o]: "Optionally, one G0 character set can be designated by packets X/28/1 and/or M/29/1 for compatibility with earlier specifications. The page-related data (X/28/1) takes precedence over the magazine-related data (M/29/1). Decoders may respond to this data to ensure the correct interpretation of the C12, C13 and C14 control bits when selecting national option sets related to the designated or default G0 character set. Equipment intended for operation with only a single group of character set options may ignore this data."
- Two G0 sets plus national option via X/28/0 Format 1 and M/29/0, **[o x x x]**: "Optionally, the two G0 character sets and additional data regarding the correct selection of national option subsets can be designated by packets X/28/0 Format 1 and M/29/0. The page-related data (X/28/0 Format 1) takes precedence over the magazine-related data (M/29/0). Equipment intended for operation with only a single group of character set options may ignore this data."
  - **CONFLICT:** Table 1 marks X/28/0 F1 presentation data and M/29/0 as not used at 1.5 [o o x x]. Table 25 ticks this G0 designation function at 1.5. Clause 15.2 says the designation "will be ignored by existing Level 1 and 1.5 decoders". Clause 15.3 says second G0 data "is unlikely to be interpreted by Level 1 or 1.5 decoders". E.9 says 1.5 decoders "are likely to select the national option sub-set … by inspecting only the C12, C13 and C14 control bits". Conclusion: optional for a Level 1.5 decoder.
- G0 re-designation by column triplet (mode 01000): [o o x x]. Not at 1.5.
- G0 designation via X/28/4 and M/29/4: [o o o x].
- **G2 supplementary set: [o x x x].** "Set of 95 supplementary alphanumeric characters plus SPACE (2/0). Address range 2/0 to 7/F. The Latin G2 supplementary character set, table 37, is the default. The response of a decoder to codes 5/9, 5/A, 5/B and 6/5 is not defined. NOTE: The repertoire of these additional characters available in a Level 1.5 decoder is not fixed and is determined by the language requirements in the country of use."
- G2 designation via X/28/0 F1 and M/29/0: [o o x x]. Via a column triplet: [o o x x]. Via X/28/4 and M/29/4: [o o o x].
- **G3 set: [o x x x].** "Fixed set of 95 smoothed block mosaic and line drawing characters comprising 91 symbols plus SPACE (5/F), table 48. Address range 2/0 to 7/F. The response of a decoder to codes 6/E, 6/F, 7/E and 7/E [sic] is not defined." Table 48 marks 6/E, 6/F, 7/E and 7/F as unassigned, so the second "7/E" is a typo for 7/F.
- Bold/italic/proportional [o o o x]. DRCS [o o x o]/[o o o x]. Objects [o o x x]. Side panels [o o x x]. Row/screen colours [o o x x]. Black foreground and the 32-colour map [o o x x]. None of these apply at 1.5.

### 1.4 Table 26 (spacing attributes), pp.70-74: 1.5-specific points
- 0/0 Alpha Black and 1/0 Mosaics Black: [o o x x]. "should be used with caution as it is interpreted by some existing Level 1 and Level 1.5 decoders."
- 0/E Double Width and 0/F Double Size: [o o x x]. Some Level 1/1.5 decoders interpret them, and some treat Double Size as Double Height (NOTE 4, NOTE 9).
- 0/D Double Height [x x x x]. The sub-row "Characters defined by enhancement data (local X/26 or Objects) may be addressed to locations on the lower row that are not occupied by the lower parts of double height/size characters and will be displayed" is **[o o x x]**. NOTE 3 (and NOTE 6 under 0/F): "Some existing Level 1.5 decoders will not display local X/26 enhancement data in these circumstances." So Level 1.5 behaviour for X/26 characters on the lower row of a double-height row is NOT specified. The lower row normally shows no foreground, and "Any transmitted Level 1 characters and attributes for the lower row are ignored."
- 1/B ESC [x x x x]: "Toggles between the first and second G0 sets defined by packets X/28/0 Format 1, X/28/4, M/29/0 or M/29/4. The default at the start of each row is the default G0 set. **This does not affect characters written via packets X/26 or as part of an object.**"
- 1/9, 1/A, 1/E, 1/F (NOTE 11, 12, 14, 15): "This code has effect only on characters defined at Levels 1 and 1.5." (1/E NOTE 14 adds: "Size and character set changes invoked by enhancement triplets at Levels 2.5 and 3.5 do not cancel Hold Mosaics.")
- 1/D New Background, sub-row "The foreground colour currently selected is adopted as the background colour." [x x o o]. The CLUT-remap sub-row is [o o x x].

---------------------------------------------------------------------------------------------------

## 2. Hamming 24/18 (clause 8.3, pp.21-22)

### 2.1 General
- Clause 8 (p.20): "Unless otherwise stated, bytes are transmitted least significant bit first." Definitions (clause 3.1): "bit numbering within bytes: bits of Teletext data bytes numbered 1 to 8 (LSB to MSB)".
- Clause 8.3: "Over three consecutive 8-bit bytes, bits 1, 2, 4, 8, 16, 24 are the protection bits and the remaining bits carry the data. Single bit errors can be identified and corrected. Double bit errors can be detected. A group of three consecutive bytes coded in this manner is referred to as a triplet."

### 2.2 Bit layout (transmission order; bit k is the k-th bit on air)
```
            Byte N                           Byte N+1                            Byte N+2
bit:  1  2  3  4  5  6  7  8    9 10 11 12 13 14  15 16    17  18  19  20  21  22  23 24
      P1 P2 D1 P3 D2 D3 D4 P4   D5 D6 D7 D8 D9 D10 D11 P5  D12 D13 D14 D15 D16 D17 D18 P6
```
C mapping, INFERRED from "LSB first" plus "bits 1-8 = LSB-MSB", with b0,b1,b2 being the received bytes with bit 0 = LSB = first bit on air:
- P1=b0.0, P2=b0.1, D1=b0.2, P3=b0.3, D2=b0.4, D3=b0.5, D4=b0.6, P4=b0.7
- D5..D11 = b1.0..b1.6, P5 = b1.7
- D12..D18 = b2.0..b2.6, P6 = b2.7
- 18-bit value: D1 = LSB … D18 = MSB. This is consistent with the clause 12.3.1 field diagram: bit 1 = A0 (address LSB), bit 7 = M0, bit 12 = D0.

### 2.3 Encoding equations (verbatim)
- P1 = 1 ⊕ D1 ⊕ D2 ⊕ D4 ⊕ D5 ⊕ D7 ⊕ D9 ⊕ D11 ⊕ D12 ⊕ D14 ⊕ D16 ⊕ D18
- P2 = 1 ⊕ D1 ⊕ D3 ⊕ D4 ⊕ D6 ⊕ D7 ⊕ D10 ⊕ D11 ⊕ D13 ⊕ D14 ⊕ D17 ⊕ D18
- P3 = 1 ⊕ D2 ⊕ D3 ⊕ D4 ⊕ D8 ⊕ D9 ⊕ D10 ⊕ D11 ⊕ D15 ⊕ D16 ⊕ D17 ⊕ D18
- P4 = 1 ⊕ D5 ⊕ D6 ⊕ D7 ⊕ D8 ⊕ D9 ⊕ D10 ⊕ D11
- P5 = 1 ⊕ D12 ⊕ D13 ⊕ D14 ⊕ D15 ⊕ D16 ⊕ D17 ⊕ D18
- P6 = 1 ⊕ (all other 23 bits: P1 P2 D1 P3 D2 D3 D4 P4 D5..D11 P5 D12..D18)

### 2.4 Decoding: "Six odd parity tests (A - F) on bits marked •"
Coverage was read from the PDF table on p.21 and is given here as bit positions 1-24:
- A: 1,3,5,7,9,11,13,15,17,19,21,23 (positions whose bit 0 is set)
- B: 2,3,6,7,10,11,14,15,18,19,22,23
- C: 4,5,6,7,12,13,14,15,20,21,22,23
- D: 8,9,10,11,12,13,14,15
- E: 16,17,18,19,20,21,22,23
- F: all bits 1..24

Each test is an odd-parity test, so the test passes when the XOR of the covered bits is 1.

Results table (p.22, verbatim):
| A,B,C,D,E | F | Inference | Action |
|---|---|---|---|
| All correct | Correct | No errors | Accept data bits |
| All correct | Not correct | Error in P6 | Accept data bits |
| Not all correct | Correct | Double error | Reject data bits |
| Not all correct | Not correct | Single error | Complement bit in error, see note |

NOTE 2: "The position of the bit in error is calculated from: 2^4 × Test E result + 2^3 × Test D result + 2^2 × Test C result + 2^1 × Test B result + 2^0 × Test A result where A Test result = "1" if the odd parity test is failed."
- The spec gives no worked numeric example for 24/18.
- **The spec is SILENT** on what a decoder does with a rejected triplet (skip it, abandon the packet, or something else). E.3 and A.3 give no guidance. It is also silent on a computed error position > 24, which is impossible for a single error and can only come from 3+ errors. Treating that as uncorrectable is an implementation choice.

### 2.5 Hamming 8/4 (designation code, clause 8.2, p.20-21), for completeness
- Byte bits (transmission order) 1..8 = P1 D1 P2 D2 P3 D3 P4 D4. D1 is the LSB of the 4-bit value; p.25 gives the page-number weighting example "bit 2 = 2^0 … bit 8 = 2^3".
- P1 = 1⊕D1⊕D3⊕D4; P2 = 1⊕D1⊕D2⊕D4; P3 = 1⊕D1⊕D2⊕D3; P4 = 1⊕P1⊕D1⊕P2⊕D2⊕P3⊕D3⊕D4.
- Tests: A = {P1,D1,D3,D4}; B = {D1,P2,D2,D4}; C = {D1,D2,P3,D3}; D = all 8 bits.
- Results: A,B,C all correct with D correct means no error. A,B,C all correct with D wrong means an error in P4; accept. Not all of A,B,C correct with D correct means a double error; reject. Not all correct with D wrong means a single error: "Refer to previous table to identify bit in error. Correct error if in data bit."

---------------------------------------------------------------------------------------------------

## 3. Packet X/26 structure

### 3.1 Byte layout (clause 9.4, Figure 11, p.27)
- "Packets X/26, X/28 and M/29 can carry data to enhance a basic Level 1 Teletext page. … Byte 6 is used as an additional address byte (designation code), coded Hamming 8/4. This allows up to 16 versions of each packet type. The remaining 39 bytes are Hamming 24/18 coded, grouped as 13 triplets."
- Bytes 1-2 clock run-in; byte 3 framing code; bytes 4-5 packet address (magazine and packet number = 26); byte 6 designation code (DC); triplet N (N = 1..13) occupies bytes 3N+4, 3N+5 and 3N+6. So triplet 1 = bytes 7-9 and triplet 13 = bytes 43-45.
- Clause 9.4.1 (p.28): X/26 is used "at presentation Levels 1.5, 2.5, 3.5: addressing a character location and overwriting the existing character defined on the Level 1 page"; at 2.5/3.5 for attributes and objects; "at all presentation Levels: VCR programming, see EN 300 231". "Designation code values 0000 to 1111 allow up to 16 packets with Y = 26 to be associated with a given page. Unlike other page enhancement packets, the function of a data bit within a packet X/26 is not determined by its overall position within the packet. The coding and function of the data bits of packets X/26 is described in clause 12.3."
- Clause 12.3.1 (p.74): "In general, a triplet or combination of triplets define a screen location and the enhancements to the Level 1 page to be set at that location."

### 3.2 Ordering and retransmission
- Annex B.2 (p.125), point 2: "In the case of pages intended for direct display, packets with Y = 26 shall be transmitted before any packets with Y = 1 to Y = 25. Packets with Y = 26 shall be transmitted in ascending order of designation code. If the transmission of the page is interrupted and then restarted, any packets with Y = 27, Y = 28 and Y = 26 shall be re-transmitted following the page header, regardless of the state of the C4 bit for that page." Point 1: after the header come Y = 27 and then Y = 28, "in that order".
- Annex A.3 (p.121): "The designation code of the first transmitted packet 26 of a page of any type should be '0'. Following packets 26 should use ascending, contiguous designation code values. NOTE: Some existing Level 1.5 and PDC decoders may not respond to packet X/26/15."
- Annex D.1.3 (p.131): "Packets X/26 should be transmitted with sequential designation codes, starting at 0000. Consequently, the absence of a packet with a particular designation code from a sequence of packets X/26 and prior to a Termination Marker triplet may be interpreted as an error condition by a decoder. Under these circumstances the response of a decoder is not defined here."
- Annex D.1.2 (p.131), no packets X/26: "Level 1.5 decoders should assume that the display of the page does not require characters or symbols which cannot be selected via codes in packets X/1 to X/24."
- Annex B.1: a 20 ms page-clearing interval is kept after the header for pages "intended to be interpreted by a Level 1 or 1.5 decoder".
- Annex A.4 (p.121): "local enhancement data triplets and the elements of objects shall be encoded in order of the screen address, left to right and top to bottom of the screen. … A decoder may ignore any triplet from the local enhancement data or from any object which addresses a screen location earlier than that addressed by the previous triplet from the same source." Modes 00001, 00100, 00111 and all column triplets "cause the Active Position to be modified and shall be encoded in screen address order". Row-colour triplet order: 00000 Full Screen Colour, then 00111 Address Display Row 0, then 00001 Full Row Colour.
- Processing order in the decoder is transmission order: DC 0 triplet 1..13, then DC 1, and so on. INFERENCE from B.2 and A.3. Annex C.2 rule 3 uses "relative positions in the transmission" (X/26/0 before X/26/1) to break ties.

### 3.3 Termination Marker (Table 28 mode 11111, p.83)
- Format: Address = 111111 (63), Mode = 11111, Data = D6..D0.
- Verbatim: "Since more than one packet with Y = 26 may be needed to display a given page, a terminator is provided by setting the Address and Mode Description bits all to 1, in the last triplet (bytes 43, 44 and 45) in the final packet in use. Any unused data groups between the active data groups and the termination group shall be filled with repetitions of the data in the termination group."
- "The data field bits are reserved." This sub-row is ticked **[o x o o]**.
- At 2.5/3.5 [o o x x], D2..D0 give the type of data terminated: 000/001 intermediate (G)POP; 010/011 last (G)POP; 100/101 Local Object defs; 110 "Local enhancement data. End of enhancement data, Local Object definitions follow."; 111 "Local enhancement data. End of enhancement data, no Local Object definitions follow."
- "The Termination Marker triplet may also be used as a "space-filler" between object definitions and between the end of local enhancement data and the start of Local Object definitions."
- A.3: "To ensure compatibility with Level 1.5 decoders, a Termination Marker triplet should be transmitted after the local enhancement data, prior to any Object Definition triplets."
- **Decoder consequence (INFERENCE):** a Level 1.5 decoder stops processing at the first Termination Marker. Anything after it (Local Object definitions) is Level 2.5/3.5 data. Because unused triplets repeat the terminator, the first one found ends the data.

---------------------------------------------------------------------------------------------------

## 4. Triplet fields, address groups, Active Position

### 4.1 Field layout (clause 12.3.1, p.74)
"The 18 data bits of each triplet are divided into 3 fields":
```
Address: bits 6..1 = A5 A4 A3 A2 A1 A0     (bit 1 = A0 = LSB)
Mode:    bits 11..7 = M4 M3 M2 M1 M0       (bit 7 = M0)
Data:    bits 18..12 = D6 D5 D4 D3 D2 D1 D0 (bit 12 = D0)
```
Bit n here is Hamming data bit Dn of clause 8.3.
"Triplets are divided into two groups according to the value of the address field - a Column Address Group (0 ≤ Address ≤ 39) and a Row Address Group (40 ≤ Address ≤ 63)."
Clause 12.3.1 also says the triplet coding "is also applicable to packets X/3 to X/25 of an object definition page", which is not relevant to 1.5.

### 4.2 Active Position, clause 12.3.2 (pp.75-76), in full
"The address field of certain packet X/26 triplets is used to specify the Active Position. The Active Position is defined by row and column co-ordinates and is a reference to a screen location within the normal page area. It is used to position enhancement data when overwriting a Level 1 page. The valid ranges are 0 to 24 for the row component and 0 to 39 for the column component. The range is extended to the side-panel areas through the action of the Origin Modifier triplet.

Display row 0 is addressed by explicit use of the row address group with mode value 00111. This triplet also has the action of setting both the row and column co-ordinates of the Active Position to 0.

A row co-ordinate in the range 1 to 24 is set by the address field of the most recent Full Row Colour or Set Active Position triplet. Address value 40 (decimal) implies row 24 and values 41 to 63 indicate rows 1 to 23 inclusive. A Full Row Colour triplet also sets the column co-ordinate to 0. A Set Active Position triplet sets the column co-ordinate explicitly.

Level 2.5 and 3.5 decoders should not use the address field of other row address triplets for modifying the Active Position.

The Active Position is assumed to be reset to row 0, column 0 when starting to process local enhancement data or placing default objects.

Apart from the one column address triplet used for PDC and those which are reserved, the column co-ordinate is set by the address field of column address triplets and the row co-ordinate is not modified."

Row mapping: row = (addr == 40) ? 24 : addr - 40, for addr 41..63 giving rows 1..23. Address 63 (with mode 00111) gives row 0.

### 4.3 Level 1.5 deviation (Annex A.3, p.121, verbatim)
"The most recent value of the address bits in the range 40 to 63 will be treated by existing Level 1.5 decoders as setting the row co-ordinate of the Active Position, regardless of the mode value of the triplet."
- E.3 (p.134): "Many existing Level 1.5 decoders will interpreted [sic] the address field of all row address triplets as a genuine row address."
- A.5 NOTE 1 (p.122), on Full Screen Colour: "There should be only one full screen colour definition per page and it should be the first triplet encountered that defines any attribute or character for the page. For compatibility with Level 1.5 decoders, an address field value of 41 (decimal) is recommended." (A 1.5 decoder will treat it as row 1.)
- Note the conflict: 12.3.2 says the row is set only by Full Row Colour, Set Active Position or mode 00111. A.3 and E.3 say legacy 1.5 decoders use every row triplet. Table 28 ticks only Set Active Position (row) and Address Display Row 0 at 1.5 (section 5). A requirements document must choose. A.3 is normative (Annex A is "normative") but only describes "existing" decoders.
- A.3: "For any display position there should be only one column address group selecting a character which could be interpreted by a Level 1.5 decoder."
- Column coordinate: at 1.5 it is set by the column triplet address field. The Set Active Position column sub-row is NOT ticked at 1.5 (section 5).

---------------------------------------------------------------------------------------------------

## 5. Triplet modes and Level 1.5 ticks (Tables 27/28/29, pp.75-88, ticks read from PDF)

### 5.1 Row Address Group (address 40-63), Table 28
| Mode | Function | [1 1.5 2.5 3.5] | Level 1.5 action |
|---|---|---|---|
| 00000 | Full Screen Colour | [o o x x] | ignore the function. A.3/E.3: legacy 1.5 decoders still take row = addr (hence the "41" recommendation) |
| 00001 | Full Row Colour | [o o x x] | ignore colour. 12.3.2 says it sets row and column = 0; A.3 says 1.5 decoders set row |
| 00010, 00011 | Reserved | – | ignore (A.3: row still latched by legacy decoders) |
| **00100** | **Set Active Position** | Row co-ordinate sub-row **[o x x x]**; Column co-ordinate sub-row **[o o x x]** | **Set row from address (40→24, 41-63→1-23).** Column from data C6..C0 (<40) is NOT a 1.5 function |
| 00101, 00110 | Reserved | – | ignore |
| **00111** | **Address Display Row 0** | **[o x x x]** (tick on the entry header) | see below |
| **01000-01101** | **PDC data** (EN 300 231) | **[o x x x]** | not display. See section 12 |
| 01110, 01111 | Reserved | – | ignore |
| 10000 | Origin Modifier | [o o x x] | ignore |
| 10001-10011 | Object Invocation (Active/Adaptive/Passive) | [o o x x] | ignore |
| 10100 | Reserved | – | ignore |
| 10101-10111 | Object Definition | [o o x x] | ignore. Should not be seen before the terminator (A.3) |
| 11000 | DRCS Mode | [o o x x] | ignore |
| 11001-11110 | Reserved | – | ignore |
| **11111** | **Termination Marker** | "data field bits are reserved" sub-row **[o x o o]** | end of local enhancement data (section 3.3) |

Mode 00111, Address Display Row 0 (p.78), verbatim: "When the address field is set to 111111 the Active Position is set to row 0, column 0. Subsequent column address triplets should address columns 8 to 39 only. The function of the triplet for other address values (40 - 62) is reserved and the Active Position is not modified." The data field (S1 S0 C1 C0 E2 E1 E0) selects a row-0 colour, which is a Full-Row-Colour function. The spec does not separately tick the colour part. A 1.5 decoder has no row-colour capability (Full Row Colour is [o o x x]), so it ignores the data field. INFERENCE. "NOTE: The use of double height and double size attributes in row 0 is not permitted."

Full Row Colour and Set Active Position formats (p.77):
- Full Row Colour: Addr R5..R0 (≥40), Mode 00001, Data S1 S0 C1 C0 E2 E1 E0.
- Set Active Position: Addr R5..R0, Mode 00100, Data C6..C0. "The address field sets the row co-ordinate (according to clause 12.3.2) for all data field values. If the data field value is less than 40 (decimal), bits C6 - C0 (MSB - LSB) define the column co-ordinate. All other data field values are reserved."

### 5.2 Column Address Group (address 0-39), Table 29
Clause 12.3.4 (p.83): "For all mode description values except all reserved values and the one used for PDC, the address field sets the column co-ordinate of the Active Position."

| Mode | Function | [1 1.5 2.5 3.5] | Level 1.5 action |
|---|---|---|---|
| 00000 | Foreground Colour | [o o x x] | ignore |
| 00001 | Block Mosaic Character from G1 | [o o x x] | ignore |
| **00010** | **Line Drawing or Smoothed Mosaic Character from the G3 Set at Level 1.5** | **[o x x x]** | **place G3 char** (below) |
| 00011 | Background Colour | [o o x x] | ignore |
| 00100, 00101 | Reserved | – | ignore (column not set) |
| **00110** | **PDC - Cursor Column & Announced Starting & Finishing Time Minutes** | **[x x x x]** | not display; "NOTE 2: The Active Position is not modified." |
| 00111 | Additional Flash Functions | [o o x x] | ignore |
| 01000 | Modified G0 and G2 Character Set Designation | [o o x o] (2-pair limit); "any number of pairs" sub-row [o o o x] | ignore |
| 01001 | Character from the G0 Set at Levels 2.5 and 3.5 | [o o x x] | ignore |
| 01010 | Reserved | – | ignore |
| 01011 | Line Drawing/Smoothed Mosaic from G3 at Levels 2.5 and 3.5 | [o o x x] | NOTE 3: "This command is intended for use at Levels 2.5 and 3.5 only to ensure existing decoders remain compatible with Level 2.5 and 3.5 transmissions. **Level 1.5 decoders should ignore this command.**" |
| 01100 | Display Attributes | [o o x x] | ignore |
| 01101 | DRCS Character Invocation | [o o x x] | ignore |
| 01110 | Font Style | [o o o x] | ignore |
| **01111** | **Character from the G2 Supplementary Set** | **[o x x x]** | **place G2 char** |
| **10000-11111** | **Characters Including Diacritical Marks** (10000 = G0 char with no mark) | **[o x x x]** | **place G0 char plus optional diacritic** |

Whether a 1.5 decoder must still latch the column for ignored non-reserved column modes (e.g. 00000) is not stated for 1.5. 12.3.4 says the column is set for all non-reserved, non-PDC modes. That is harmless because only character modes produce output.

**Mode 00010, G3 at Level 1.5 (p.84), verbatim:** "The 7 data field bits select a line drawing or smoothed mosaic character from the G3 set, table 48. Data field values < 20 hex are reserved but decoders should still set the column co-ordinate of the Active Position to the value of the address field. NOTE 1: This command is intended for use at Level 1.5 only to ensure existing decoders remain compatible with Level 2.5 and 3.5 transmissions but it should also be interpreted by Level 2.5 and 3.5 decoders. Level 1.5 decoders may not respond to some or all valid data field values."

**Mode 01111, G2 (p.88), verbatim:** "The 7 data field bits select a character from the current G2 set. The G2 set currently selected can be the default set, that specified by packets X/28/0 Format 1, X/28/4, M/29/0 or M/29/4, or that specified by the most recent "Modified G0 and G2 Character Set Designation" triplet. Data field values < 20 hex are reserved but decoders should still set the column co-ordinate of the Active Position to the value of the address field. NOTE 5: The repertoire of supplementary characters in a Level 1.5 decoder is not fixed and is determined by the language requirements in the country of use."

**Modes 10000-11111 (p.88), verbatim:** "These commands "compose" a display character by adding one of up to 16 diacritical marks to an alphabetic character from a G0 set. The 4 LSBs of the Mode Description value select a diacritical mark from column 4 of the G2 supplementary character set, in ascending numerical order. The associated character from the G0 set is defined by the 7 bits of the data field. No diacritical mark exists for mode description value 10000. An unmodified G0 character is then displayed unless the 7 bits of the data field have the value 0101010 (2/A) when the symbol "@" shall be displayed. Data field values < 20 hex are reserved but decoders should still set the column co-ordinate of the Active Position to the value of the address field. The current sets can be the default sets or those specified by packets X/28/0 Format 1, X/28/4, M/29/0 or M/29/4, or those specified by the most recent "Modified G0 and G2 Character Set Designation" triplet."

### 5.3 E.3, the practical Level 1.5 subset (p.134, verbatim table)
"The packet X/26 functionality is usually limited to a sub-set of the following column address triplets:"
| Mode | Function | Valid Data Range |
|---|---|---|
| 00010 | G3 set character | 5/1, 5/B, 5/C and 5/D only |
| 01111 | G2 set character | 2/0 to 7/F |
| 10000 | G0 set character (no diacritical mark) | 2/0 to 7/F |
| 10001 to 11111 | Diacritical mark + G0 set character | 2/0 to 7/F |
"In addition, many existing decoders will not respond to packet X/26/15.
Packet 26 characters which address positions in row 0 should appear when the page is displayed in full. A decoder is not required to reproduce such characters for incorrect page headers while it is searching for a page and rolling the Level 1 text from those page headers."

---------------------------------------------------------------------------------------------------

## 6. Diacritical marks (G2 column 4) for modes 10000-11111

Mode low nibble n selects G2 position 4/n. Glyph names were read from the Table 37 image (p.108) and match the ISO 6937 non-spacing diacritics (Table 37 NOTE 3 references ISO 6937/2). The spec gives no names, so treat them as identifications from the image.
| Mode | G2 pos | Mark |
|---|---|---|
| 10000 | 4/0 | none (blank cell). Unmodified G0 char; data 0x2A shows "@" |
| 10001 | 4/1 | grave accent ` |
| 10010 | 4/2 | acute accent ´ |
| 10011 | 4/3 | circumflex ^ |
| 10100 | 4/4 | tilde ~ |
| 10101 | 4/5 | macron ¯ |
| 10110 | 4/6 | breve ˘ |
| 10111 | 4/7 | dot above ˙ |
| 11000 | 4/8 | diaeresis/umlaut ¨ |
| 11001 | 4/9 | dot below (image shows a low dot; ISO 6937 leaves 4/9 unassigned). Name UNVERIFIED |
| 11010 | 4/A | ring above ˚ |
| 11011 | 4/B | cedilla ¸ |
| 11100 | 4/C | low line / underline (image shows a bar at the baseline). Name UNVERIFIED |
| 11101 | 4/D | double acute ˝ |
| 11110 | 4/E | ogonek ˛ |
| 11111 | 4/F | caron ˇ |
Data-range rules: data 0x20-0x7F is valid. Data <0x20 is reserved, but the column must still be set. The "@" rule applies only to mode 10000 with data 0101010 (0x2A).
- G0 sets: Latin Table 35 NOTE 3: "The @ symbol replaces the * symbol at position 2/A when the table is accessed via a packet X/26 Column Address triplet with Mode Description = 10 000 and Data = 0101010. See clause 12.2.4" [sic: should be 12.3.4]. The same note appears on Cyrillic Tables 38/39/40, Greek 42, Arabic 44 and Hebrew 46.
- Latin G0 via X/26: Table 35 NOTE 2: "The characters at the shaded positions apply when the set is accessed via packets X/26. Characters from a national option sub-set from table 36 occupy these positions when the set is addressed directly from a Level 1 page." The shaded X/26 glyphs (from the image, p.106) are: 2/3 #, 2/4 ¤, 4/0 @, 5/B [, 5/C \, 5/D ], 5/E ^, 5/F _, 6/0 `, 7/B {, 7/C | (drawn as a broken bar), 7/D }, 7/E ~. So X/26 G0 references never use national options (Table 25, clause 15.4 NOTE: "the set of characters is not modified by any national option sub-set selection").
- Annex E.10 (p.135): "The technique for defining a diacritical mark from a G2 character set and adding it to a character from a G0 character set … should be restricted to the Latin character sets as defined in clauses 15.6.3 and 15.6.1 respectively. Decoders are unlikely to be able to handle other G0 sets in this way." (E.10 cites "table 26, mode bits = 1000 to 11111", a typo for Table 29, 10000-11111.)
- The spec does not define how the mark is composed with the base glyph (position, capitals versus lower case). G.2: "The precise appearance of all characters and mosaics … is a decoder option." Composition is implementation-defined.

---------------------------------------------------------------------------------------------------

## 7. G2 and G3 sets

### 7.1 Latin G2 Supplementary Set, Table 37 (p.108)
Glyphs were identified from the image, cross-checked against ISO 6937.
| Row | col 2 | col 3 | col 4 | col 5 | col 6 | col 7 |
|---|---|---|---|---|---|---|
| 0 | SPACE | ° degree | (no mark) | ― horizontal bar | Ω Ohm/Omega | ĸ kra |
| 1 | ¡ | ± | grave | ¹ | Æ | æ |
| 2 | ¢ | ² | acute | ® | Đ (D stroke) | đ |
| 3 | £ | ³ | circumflex | © | ª | ð eth |
| 4 | $ | × | tilde | ™ | Ħ | ħ |
| 5 | ¥ | µ | macron | ♪ eighth note | (blank; "not defined"/not transmitted) | ı dotless i |
| 6 | # | ¶ | breve | ₠ (euro currency, NOTE 4) | Ĳ | ĳ |
| 7 | § | · middle dot | dot above | ‰ | Ŀ | ŀ |
| 8 | ¤ | ÷ | diaeresis | α | Ł | ł |
| 9 | ‘ | ’ | (low dot) | (blank; do not transmit) | Ø | ø |
| A | “ | ” | ring | (blank; do not transmit) | Œ | œ |
| B | « | » | cedilla | (blank; do not transmit) | º | ß |
| C | ← | ¼ | low line | ⅛ | Þ | þ |
| D | ↑ | ½ | double acute | ⅜ | Ŧ | ŧ |
| E | → | ¾ | ogonek | ⅝ | Ŋ | ŋ |
| F | ↓ | ¿ | caron | ⅞ | ŉ | ■ solid block (NOTE 6) |
Notes (verbatim): "NOTE 1: The character in position 2/0 is to be interpreted as SPACE. NOTE 2: Column 4 contains diacritical marks for association with G0 set characters. NOTE 3: The characters in positions 5/6, 5/7 and 5/8 are in addition to those specified in ISO 6937/2 [4]. NOTE 4: The character in position 5/6 represents the European currency symbol. NOTE 5: The characters codes 5/9, 5/A, 5/B and 6/5 should not be transmitted. NOTE 6: The character in position 7/F occupies an area equivalent to that of any character which does not include a descender. It is thus a rectangle surrounded by the background colour. NOTE 7: Bit allocations are as for table 35."
All glyph names in this table are image identifications. Any a requirements document relies on should be marked "per Table 37 image".

### 7.2 Other G2 sets
- **Cyrillic G2, Table 41 (p.112):** columns 2-5 are essentially Latin G2 (currency, arrows, fractions, column 4 diacritics, 5/0-5/8 symbols). Differences: 2/6 and 2/8 blank, and "NOTE 2: The characters codes 2/6 and 2/8 should not be transmitted". 5/9 Ł-like, 5/A Ɨ-like, 5/B β-like glyph (image). Columns 6-7 hold Latin letters D E F G I J K L N Q R S U V W Z (upper case col 6, lower case col 7), i.e. Latin letters missing from the Cyrillic G0. 5/6 is the euro currency symbol (NOTE 5).
- **Greek G2, Table 43 (p.114):** column 2/3 and columns 6/7 hold Latin letters (a b e h i k … upper/lower case) missing from Greek G0, plus accented capitals ('I 'Y 'Ω 'A 'E 'H). Column 4 holds diacritics (NOTE 2). 5/6 is the euro symbol (NOTE 4). 7/F is a block (NOTE 5).
- **Arabic G2, Table 45 (p.116):** Arabic presentation forms in cols 2-3. Columns 4-7 hold a Latin alphabet A-Z/a-z with some French accented letters (à é è ê ë î ô û ù ç â). No column-4 diacritics.
- Hebrew has no G2 table of its own. Table 32 pairs Hebrew G0 with the Arabic G2.

### 7.3 G3 Smooth Mosaics and Line Drawing Set, Table 48 (p.119)
- Structure (from the image): columns 2-3 and 6-7 are smoothed-mosaic shapes (sloped diagonal fills, rounded corners, wedges; 2/F is a 50% checkerboard, NOTE 4 "represents the activation of alternate dots of the matrix"; 3/F is a solid block). Column 4 is line-drawing curves and arcs, crosses, 4/D small dot, 4/E filled circle, 4/F open circle. Column 5 is line-drawing box pieces (vertical, horizontal, corners, tees, cross) plus arrows 5/B-5/E.
- Notes verbatim: "NOTE 1: The character at 5/F permits the display of background colour. NOTE 2: No character is assigned to the positions marked (2) and the corresponding codes should not be transmitted [6/E, 6/F, 7/E, 7/F]. NOTE 3: The dark areas of the characters are displayed in the foreground colour. NOTE 4: The character at position 2/F represents the activation of alternate dots of the matrix. NOTE 5: Bit allocations are as for table 35."
- **E.3 Level 1.5 subset, glyphs from the image:**
  - **5/1**: horizontal line across the middle of the cell (line-drawing horizontal bar)
  - **5/B**: solid arrow pointing RIGHT
  - **5/C**: solid arrow pointing LEFT
  - **5/D**: solid arrow pointing UP
  - (5/E is the down arrow, but it is not in the E.3 list.)

---------------------------------------------------------------------------------------------------

## 8. Non-Latin G0 sets (clause 15.6, pp.109-117)
| Table | Set | Notes |
|---|---|---|
| 38 (p.109) | Cyrillic G0 Option 1 - Serbian/Croatian | Cyrillic letters in cols 4-7 including Љ Њ Ћ Ђ Џ Ј; 2/4 is $; X/26 "@" note |
| 39 (p.110) | Cyrillic G0 Option 2 - Russian/Bulgarian | full Russian alphabet in cols 4-7; 2/6 is ы; X/26 "@" note |
| 40 (p.111) | Cyrillic G0 Option 3 - Ukrainian | like Option 2 with Ukrainian letters (ї, І/і, Є/є, Ї); X/26 "@" note |
| 41 (p.112) | Cyrillic G2 | see 7.2 |
| 42 (p.113) | Greek G0 | Greek upper case col 4-5, lower case col 6-7, accented/diaeresis vowels, 3/C « and 3/E »; X/26 "@" note |
| 43 (p.114) | Greek G2 | see 7.2 |
| 44 (p.115) | Arabic G0 | digits/punctuation cols 2-3, Arabic letter forms cols 4-7, 5/F is #; X/26 "@" note |
| 45 (p.116) | Arabic G2 | see 7.2 |
| 46 (p.117) | Hebrew G0 | Latin upper case in cols 4-5 (5/B-5/E arrows ← ½ → ↑, 5/F #), Hebrew letters cols 6-7. "NOTE 4: The character at 7/B is the Shekel currency sign." X/26 "@" note |
| 47 (p.118) | G1 Block Mosaics | (Level 1) |
| 48 (p.119) | G3 | see 7.3 |
All carry "The character in position 2/0 is to be interpreted as SPACE" and "Bit allocations are as for table 35". The G0 tables carry the 7/F block note. The non-Latin G0 sets have no national-option subsets: Table 32 NOTE 3, "No separate sub-sets exist for the national options shown in brackets. The characters are contained within the appropriate G0 set."
Table 36 (p.107): Latin national option sub-sets for positions 2/3, 2/4, 4/0, 5/B, 5/C, 5/D, 5/E, 5/F, 6/0, 7/B, 7/C, 7/D, 7/E. Sub-sets: Czech/Slovak, English, Estonian, French, German, Italian, Lettish/Lithuanian, Polish, Portuguese/Spanish, Rumanian, Serbian/Croatian/Slovenian, Swedish/Finnish (also Hungarian, NOTE 3), Turkish. NOTE 1: "TL is the Turkish currency symbol". NOTE 2: "In some decoders the Ë at 2/4 of the Serbian/Croatian/Slovenian set has been replaced by either $ or ½." This is Level 1 scope.

---------------------------------------------------------------------------------------------------

## 9. Default G0/G2 designation

### 9.1 X/28/0 Format 1, Table 4 (p.30)
- Triplet 1 bits 1-4 Page Function (0000 = basic Level 1 page). Bits 5-7 Page Coding (000 = 7 bits plus odd parity).
- **Triplet 1 bits 8-14: Default G0 and G2 Character Set Designation and National Option Selection.** "The 7-bit value is used to select an entry in table 32." NOTE: "The default character sets at the start of each row are the default G0 and G2 sets. In some transmissions, each "ESC" control character (code 1/B) on the Level 1 page toggles the G0 set between the default and second G0 sets for the subsequent G0 characters of the row."
- **Triplet 1 bits 15-18 plus Triplet 2 bits 1-3: Second G0 Set Designation and National Option Selection**, as in Table 33.
- Triplet 2 bits 4 onward and triplets 3-13: side panels, CLUTs, colours (Level 2.5/3.5).
- Bit order within the 7-bit values, from the Table 32/33 column headings: Table 32 value = bits 14 13 12 11 10 9 8 (bit 14 = MSB). Table 33 value = T2 bits 3 2 1 then T1 bits 18 17 16 15 (T2 bit 3 = MSB).
- Precedence: X/28/0 F1 over M/29/0 (9.5.1: M/29/0 "is overridden for a particular page if a packet X/28/0 Format 1 exists for that page"). M/29/0 over M/29/4. X/28/0 over X/28/4 "for all but the colour map entry coding".
- **M/29/0, Table 11 (p.36):** Triplet 1 bits 1-7 = Packet Function; 0000000 means "identical coding and function to those of packet X/28/0 Format 1 … except that here the data applies to all pages in magazine M" (other values reserved). Then T1 bits 8-14 are the Default G0/G2 designation, and T1 bits 15-18 plus T2 bits 1-3 the Second G0 designation, laid out as in X/28/0.
- M/29/4 has the same layout (Level 3.5).
- Clause 15.2 (p.100), verbatim extracts: "…by the same bits in a packet X/28/0 Format 1 or X/28/4. This value defines an entry in table 32. In the case of the G0 character set, this is the character set used by the Level 1 page at the start of each display row although it is recognized that this definition will be ignored by existing Level 1 and 1.5 decoders. Under these circumstances, or in the absence of a packet X/28/0 Format 1, X/28/4, M/29/0 or M/29/4, the default sets are established by a local Code of Practice. **The designated G0 and G2 character sets are also the default G0 and G2 sets to be addressed by packet 26 triplets as part of Level 1.5, 2.5 and 3.5 transmissions.**"
  "At levels 1 and 1.5 the national option sub-set in use on the page is defined by the C12, C13 and C14 control bits in the page header alone and, in theory, this will result in an ambiguous reference to an entry in table 32. At levels 2.5 and 3.5 the national option subset … can be defined explicitly by bits in a packet X/28/0 Format 1 or X/28/4 according to table 32. Bits 8 to 10 of triplet 1 are set to the values of the C12, C13 and C14 control bits."
- Header control bits (Table 2 area, text line ~1486): "C12, C13, C14 … Byte 13, bits 4, 6 and 8 … Where the decoder is capable of displaying text in more than one language these control bits are used to select G0 character set options, (see clause 15.2). The response to these control bits may be modified by packets X/28/0 Format 1, X/28/4, M/29/0 and M/29/4."
- Table 32 NOTE 1: "For packets X/28/0 format 1 and X/28/4, bits 8, 9 and 10 are set to the same values as the control bits in the page header packet, i.e. bit 10 = C12, bit 9 = C13 and bit 8 = C14. For packets M/29/0 and M/29/4 these bits should be set to the most common value of C12, C13 and C14 used in that magazine, see clause E.9."
- E.9 (p.135): "Level 1 and 1.5 decoders are likely to select the national option sub-set to be used on a given page by inspecting only the C12, C13 and C14 control bits of the page header." At 2.5, some decoders use C12-C14 and others use bits 8-10 of the highest-priority X/28/M/29. Recommendation 3: X/28 bits 8-10 should equal the header C12-C14.
- **Practical combination for 1.5 (INFERENCE):** take bits 14-11 (the "group") from X/28/0 F1, else M/29/0, else a Code of Practice or Latin default (D.2.1 gives 0000XXX). Take bits 10-8 from header C12 C13 C14. Look up Table 32 with the result.
- D.2.1 (p.133): "In the absence of a local (i.e. country specific) Code of Practice, the default G0 and G2 character sets should be assumed to be the Latin G0 Primary and Latin G2 Supplementary sets shown in tables 35 and 37 respectively. Thus the default value from table 32 … is 0000XXX. The C12, C13 and C14 controls bits select a national option sub-set." D.2 says it is "required to be implemented by Level 2.5 and 3.5 decoders only".

### 9.2 Table 32 (pp.101-102), complete
Value = bits 14..11 (group) and bits 10 9 8 = C12 C13 C14. "—" means reserved (NOTE 2: "Entries in the table with no character set or national option sub-set specified are reserved.")
| Bits 14-11 | C12 C13 C14 | G0 | G2 | National option |
|---|---|---|---|---|
| 0000 | 000 | Latin | Latin | English |
| 0000 | 001 | Latin | Latin | German |
| 0000 | 010 | Latin | Latin | Swedish/Finnish/Hungarian |
| 0000 | 011 | Latin | Latin | Italian |
| 0000 | 100 | Latin | Latin | French |
| 0000 | 101 | Latin | Latin | Portuguese/Spanish |
| 0000 | 110 | Latin | Latin | Czech/Slovak |
| 0000 | 111 | Latin | Latin | — (no option given) |
| 0001 | 000 | Latin | Latin | Polish |
| 0001 | 001 | Latin | Latin | German |
| 0001 | 010 | Latin | Latin | Swedish/Finnish/Hungarian |
| 0001 | 011 | Latin | Latin | Italian |
| 0001 | 100 | Latin | Latin | French |
| 0001 | 101 | Latin | Latin | — |
| 0001 | 110 | Latin | Latin | Czech/Slovak |
| 0001 | 111 | Latin | Latin | — |
| 0010 | 000 | Latin | Latin | English |
| 0010 | 001 | Latin | Latin | German |
| 0010 | 010 | Latin | Latin | Swedish/Finnish/Hungarian |
| 0010 | 011 | Latin | Latin | Italian |
| 0010 | 100 | Latin | Latin | French |
| 0010 | 101 | Latin | Latin | Portuguese/Spanish |
| 0010 | 110 | Latin | Latin | Turkish |
| 0010 | 111 | Latin | Latin | — |
| 0011 | 000-100 | Latin | Latin | — |
| 0011 | 101 | Latin | Latin | Serbian/Croatian/Slovenian |
| 0011 | 110 | Latin | Latin | — |
| 0011 | 111 | Latin | Latin | Rumanian |
| 0100 | 000 | Cyrillic-1 | Cyrillic | (Serbian/Croatian) note 3 |
| 0100 | 001 | Latin | Latin | German |
| 0100 | 010 | Latin | Latin | Estonian |
| 0100 | 011 | Latin | Latin | Lettish/Lithuanian |
| 0100 | 100 | Cyrillic-2 | Cyrillic | (Russian/Bulgarian) note 3 |
| 0100 | 101 | Cyrillic-3 | Cyrillic | (Ukrainian) note 3 |
| 0100 | 110 | Latin | Latin | Czech/Slovak |
| 0100 | 111 | — | — | — |
| 0101 | XXX | Reserved | | |
| 0110 | 000-101 | — | — | — |
| 0110 | 110 | Latin | Latin | Turkish |
| 0110 | 111 | Greek | Greek | (Greek) note 3 |
| 0111 | XXX | Reserved | | |
| 1000 | 000 | Latin | Arabic | English |
| 1000 | 001-011 | — | — | — |
| 1000 | 100 | Latin | Arabic | French |
| 1000 | 101-110 | — | — | — |
| 1000 | 111 | Arabic | Arabic | (Arabic) note 3 |
| 1001 | XXX | Reserved | | |
| 1010 | 000-100 | — | — | — |
| 1010 | 101 | Hebrew | Arabic | (Hebrew) note 3 |
| 1010 | 110 | — | — | — |
| 1010 | 111 | Arabic | Arabic | (Arabic) note 3 |
| Other values | | Reserved | | |
In the PDF, the group rows 0000-0011 show "Latin / Latin" once per group. For reserved sub-rows (e.g. 0000/111) the G0/G2 cell is merged across the group, so the character set is Latin but no national option is named.
"For compatibility with earlier specifications certain national option sub-sets are duplicated in table 32." (15.2)

### 9.3 Table 33, Second G0 set (pp.103-104), complete
7-bit value = T2b3 T2b2 T2b1 T1b18 T1b17 T1b16 T1b15. The rows follow the same pattern as Table 32 (G0 only):
| Value (binary) | G0 | National option |
|---|---|---|
| 0000 000..110 | Latin | English, German, Swedish/Finnish/Hungarian, Italian, French, Portuguese/Spanish, Czech/Slovak |
| 0000 111 | — | reserved |
| 0001 000..110 | Latin | Polish, German, Sw/Fi/Hu, Italian, French, (101 reserved), Czech/Slovak |
| 0001 111 | — | reserved |
| 0010 000..110 | Latin | English, German, Sw/Fi/Hu, Italian, French, Portuguese/Spanish, Turkish |
| 0010 111 | — | reserved |
| 0011 000..100, 110 | Latin | (none) |
| 0011 101 | Latin | Serbian/Croatian/Slovenian |
| 0011 111 | Latin | Rumanian |
| 0100 000 | Cyrillic-1 | (Serbian/Croatian) |
| 0100 001 | Latin | German |
| 0100 010 | Latin | Estonian |
| 0100 011 | Latin | Lettish/Lithuanian |
| 0100 100 | Cyrillic-2 | (Russian/Bulgarian) |
| 0100 101 | Cyrillic-3 | (Ukrainian) |
| 0100 110 | Latin | Czech/Slovak (Latin) |
| 0100 111 | — | reserved |
| 0101 XXX | Reserved | |
| 0110 000..101 | — | reserved |
| 0110 110 | Latin | Turkish |
| 0110 111 | Greek | (Greek) |
| 0111 XXX | Reserved | |
| 1000 000 | Latin | English |
| 1000 001..011 | — | |
| 1000 100 | Latin | French |
| 1000 101..110 | — | |
| 1000 111 | Arabic | (Arabic) |
| 1001 XXX | Reserved | |
| 1010 000..100 | — | |
| 1010 101 | Hebrew | (Hebrew) |
| 1010 110 | — | |
| 1010 111 | Arabic | (Arabic) |
| Other values | Reserved | |
| **1111111** | | **No second G0 set required** |
Clause 15.3: "The national option sub-set selected by the C12, C13 and C14 bits is not relevant to the secondary set. Where a transmission does not use two G0 sets, this second value is set to 1111111. A decoder may interpret this value to mean that the ESC function should be disabled for this page … or magazine…". Also: "This information is unlikely to be interpreted by Level 1 or 1.5 decoders but it is required by Level 2.5 and 3.5 decoders to ensure they function correctly when receiving Level 1.5 transmissions. … a G2 set is not designated by this value."
Table 34 (Modified G0/G2 triplet, p.105) is Level 2.5/3.5 only.

### 9.4 X/28/1 and M/29/1 (the actual Level 1.5 designation packets)
- Table 8 (p.33), X/28/1: T1 bits 1-2 = '00'; **bits 3-9 = Character Set Code for G0 Table**; bit 10 = '0'; **bits 11-17 = Character Set Code for G1 Table**; bit 18 = '0'. T2-T13 hold DCLUT data (Level 3.5). NOTE: "The function of these bits is defined by earlier specifications and is retained for compatibility with existing Level 1 and 1.5 decoders designed to them. They are not intended for use by Level 2.5 and 3.5 decoders designed to the present document."
- 9.4.4: X/28 DC=0001 "may be transmitted as part of any page at any presentation Level … G0 and G1 character designation (but only for compatibility with some existing Level 1 and 1.5 decoders designed to earlier Teletext specifications)".
- M/29/1 (Table 12, p.36): same coding. "This data applies to all Level 1 pages in magazine M but is overridden for a particular page if a packet X/28/1 exists for that page."
- **UNVERIFIED / NOT IN THIS DOCUMENT:** the meaning of the 7-bit "Character Set Code" values. They are defined in earlier specifications that are not included here. A requirements document should treat X/28/1 and M/29/1 as optional, or ignore them (Table 25: "Equipment intended for operation with only a single group of character set options may ignore this data").

---------------------------------------------------------------------------------------------------

## 10. Other compatibility rules
- **A.3** (p.121, normative): full text is in sections 3.2 and 4.3. Four points: DC starts at 0, ascending and contiguous; some 1.5/PDC decoders ignore X/26/15; legacy 1.5 decoders take the row from any address 40-63; only one column-group character per position that a 1.5 decoder could interpret; Termination Marker after the local enhancement data and before Object Definitions.
- **A.4**: section 3.2. **A.5 NOTE 1**: the "41" recommendation (section 4.3).
- **D.1.2 and D.1.3**: section 3.2.
- **E.3**: section 5.3.
- **E.9**: section 9.1.
- **E.10**: section 6.
- **E.11** (p.135): "Clause 13.8 requires that a decoder ignores instructions to display Default Objects if the page includes local enhancement data in packets X/26. Since decoders cannot identify when the packet X/26 data is included exclusively to achieve a Level 1.5 display, it is necessary to invoke all the required objects from within the packets X/26 of a Level 1.5 page." (Relevant to 2.5 decoders; a 1.5 decoder has no objects.)
- **E.13** (p.136): "The spacing attributes Start box and End box should not be used on level 1/1.5 pages that do not have the Newflash or Subtitle bits set to 1 to ensure that a spurious window … is not created by a level 2.5 decoder."
- **E.7(3)**, the row-0 analogue for objects: E.3 gives the same rule for X/26 characters in row 0 (display them when the page is shown in full; not needed on rolling headers during search). Mode 00111 note: columns 8-39 only. E.7(1): "Some decoders reserve the first eight character cells in the page header row for their own purposes."
- **Annex I** (p.~150): "The combination of TOP and parallel transmissions for Level 1 and 1.5 services is not recommended due to the limited performance of Level 1 and Level 1.5 decoders."
- **15.3**: second G0 data is "required by Level 2.5 and 3.5 decoders to ensure they function correctly when receiving Level 1.5 transmissions".
- **B.1**: 20 ms erasure interval for Level 1/1.5 pages.

---------------------------------------------------------------------------------------------------

## 11. How X/26 overwrites interact with the Level 1 page

What the spec says explicitly:
- Table 1 and 9.4.1: X/26 has "the action of overwriting the character defined for this location on the Level 1 page". It replaces the character only. No clause says it replaces or cancels a spacing attribute.
- 13.3 (Active Objects, p.93): "The local enhancement data can be considered equivalent to a one-time usable Active Object." Active Object property 1: "The default attribute settings at the current Active Position are those of the Level 1 page as modified by any Active Objects and any local enhancement data at that location." Property 2: attributes defined in them act to end of row "regardless of whether the characters are defined by the Active Object or by the Level 1 page".
  - **So the X/26 character takes the Level 1 attributes in force at that cell:** foreground and background colour, flash, conceal, size, and box. This is stated in the Objects clause, which is Level 2.5/3.5 ([o o x x] for Objects). Applying it to 1.5 is an INFERENCE, but it is the only statement.
- 13.2 display priority: Passive > Adaptive > Local enhancement data > Active > Level 1 page. The local X/26 character therefore wins over the Level 1 character.
- Table 26 0/D: "The whole of an enlarged character is displayed with the attributes that apply to the origin of the character." C.3(4): "All attributes applying to the origin of an enlarged character shall also apply to all parts of that character." INFERENCE: an X/26 character at a double-height origin cell is drawn double height. Lower-row placement at 1.5 is unspecified (section 1.4; NOTE 3/6 "Some existing Level 1.5 decoders will not display local X/26 enhancement data in these circumstances").
- ESC 1/B does not affect X/26 characters (Table 26). National options do not apply to X/26 G0 references (Table 25, Table 35 NOTE 2, 15.4 NOTE).
- Conceal 1/8: "The following characters … are to be displayed as SPACES until revealed." **SILENT** on X/26 characters specifically. Via the 13.3 inference they are concealed too.
- Mosaic mode: the X/26 triplet names the set explicitly (G0, G2 or G3). **SILENT** on whether a G0/G2 character written into a cell in Level 1 mosaics (G1) mode renders as alphanumerics; it follows from the explicit-set semantics (INFERENCE: yes). Table 47 NOTE 3 covers G1 via X/26, which is not at 1.5. Contiguous/Separated, Hold and Release Mosaics: "This code has effect only on characters defined at Levels 1 and 1.5" (Table 26 NOTE 11/12/14/15). The phrase "characters defined at … Level 1.5" is ambiguous: it could mean X/26 1.5 characters are subject to hold/separated, or simply "Level 1/1.5 page characters". **UNVERIFIED intent.**
- Held-mosaic: does an X/26 character overwriting a G1 mosaic change the "Held-Mosaic" character? **SILENT.** INFERENCE: hold-mosaic state is computed from the Level 1 byte stream.
- **Spacing attribute at the overwritten cell:** **SILENT.** The spec never says whether a spacing attribute byte in a cell overwritten by X/26 still acts, or whether the attribute cell becomes the X/26 glyph. By the "overwriting the character" wording plus the attribute model (attributes come from the Level 1 page), the common reading (INFERENCE) is: the Level 1 attribute still acts (Set-At/Set-After) and only the displayed glyph at that cell is replaced. Mark as a decision for the requirements.
- Parity errors: Table 25 allows an even-parity (i.e. deliberately "bad") byte at overwritten cells where no Level 1 decoders exist. So a 1.5 decoder should display the X/26 character regardless of Level 1 parity at that cell (INFERENCE from a transmission rule).
- Attribute contention (C.2) and the X/26 colour attributes are 2.5/3.5 only.
- Multiple X/26 characters for one cell: A.3 says there "should be only one". **SILENT** on what to do otherwise. C.2 rule 3 (for attributes) suggests "latest wins" (INFERENCE).
- Out-of-order addressing: A.4, "A decoder may ignore any triplet … which addresses a screen location earlier than that addressed by the previous triplet from the same source."

---------------------------------------------------------------------------------------------------

## 12. PDC triplets in X/26 (safe to ignore for display)
- Row group (address 40-63) modes **01000** Country of Origin and Programme Source, **01001** Month and Day, **01010** Cursor Row and Announced Starting Time Hours, **01011** Cursor Row and Announced Finishing Time Hours, **01100** Cursor Row and Local Time Offset, **01101** Series Identifier and Series Code. Ticks [o x x x] (p.78).
- Column group mode **00110** Cursor Column and Announced Starting and Finishing Time Minutes, [x x x x] (p.84). "NOTE 2: The Active Position is not modified."
- Coding is in EN 300 231, not in this document.
- Caution (A.3): a legacy 1.5 decoder would latch the row from PDC row-triplet addresses. The PDC "cursor row" semantics mean the address is a genuine row, per EN 300 231 (UNVERIFIED here). A spec-conformant decoder sets the row only from modes 00100 and 00111 (plus 00001 per 12.3.2).
- Table 1: X/26/0-14 carries PDC at all levels, including Level 1.

---------------------------------------------------------------------------------------------------

## 13. Gaps and UNVERIFIED items
1. Meaning of the X/28/1 and M/29/1 7-bit "Character Set Code" values: defined in earlier specs, not present here.
2. Handling of rejected (double-error) triplets and designation codes: not specified.
3. Diacritic names for G2 4/9 and 4/C, and all G2/G3 glyph names: identified from images only.
4. Row latching: 12.3.2 (only 00001/00100/00111) conflicts with A.3/E.3 (legacy 1.5 decoders: any address 40-63). The Table 28 ticks give Set Active Position (row only) and Address Display Row 0 at 1.5.
5. Table 1 vs Table 25 on X/28/0 F1 and M/29/0 use at Level 1.5 (section 1.3).
6. Spacing attribute at an overwritten cell; conceal, hold-mosaic and mosaic-mode interaction with X/26 characters: silent (section 11).
7. X/26 characters on the lower row of double height at 1.5: explicitly unticked or undefined.
8. Composition or rendering of diacritics: decoder option (G.2).
9. Whether a 1.5 decoder must process X/26/15: A.3/E.3 say many do not. Clause 10.1 lists 0-15, while Table 1's 1.5 row says 0-14.
