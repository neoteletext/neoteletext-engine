# ETSI EN 300 706 V1.2.1 (2003-04): objects, object pages and the links that carry them (Levels 2.5 and 3.5)

Source: EN_300_706_v1.2.1_Enhanced_Teletext.pdf, in `~/devel/teletext-reference/teletext-standards/`. **PDF page number = printed ETSI page number** for this file (checked again on pp. 3-9, 22-31, 37-59, 62-95, 104-105, 120-147, 152). "p.N" means both.
Cross-check: clauses 10.5 and 10.6 and Tables 15 and 16 were also read in ETS 300 706 (May 1997), pp. 44-45 and 54-60. They agree in substance with the 2003 text. The only difference found is a label typo in the 1997 DRCS-association figure ("POP Link" where "DRCS Link" is meant). Nothing else in the 1997 edition was compared.

Conventions:
- Triplet data bits are numbered 1 to 18, which are Hamming 24/18 data bits D1 to D18 (D1 = least significant). The address field is bits 1-6 (A0..A5), the mode field bits 7-11 (M0..M4) and the data field bits 12-18 (D0..D6 of the field) (cl. 12.3.1, p.74). Hamming 24/18 decoding is in notes-level15.md section 2.
- Hamming 8/4 bytes carry 4 data bits, numbered 1 to 4 (bit 1 = least significant) (cl. 8.2, p.20; cl. 9.3.1.1, p.25). "Byte n" counts the 45 bytes of a packet: bytes 1-5 are the prefix (clock run-in, framing code, packet address) and byte 6 is the first data byte (cl. 9.4, Figure 11, p.27).
- Triplet N of an X/26-format packet (N = 1..13) occupies bytes 3N+4..3N+6 (Figure 11, p.27). **Clause 10.5 and Table 28 number triplets 0..12 instead** (pointer-table figure p.50; "Triplet Number (range 0 - 12)" pp.55, 80). These notes use 0-based triplet numbers (`t0` = bytes 7-9) wherever the object clauses do, and say so.
- Legend: **[V]** = read from the PDF page image in this session. **[D]** = derived (computed or rearranged from the cited text, not stated as such). **[U]** = uncertain, an inference, or not stated by the standard; must be decided or tested before relying on it.
- Quotations are short. Everything else is paraphrase; cite the clause before relying on the wording.

---------------------------------------------------------------------------------------------------

## 1. Where objects sit in the standard

### 1.1 Level ticks (Table 1, pp.23-24; Table 25, p.68) [V]
Tick notation `[1 1.5 2.5 3.5]`, x = ticked, o = not ticked.

| Item | Ticks | Cite |
|---|---|---|
| X/1-X/23, X/24, X/25 "used for pages carrying enhancement data not intended for direct display, e.g. objects definitions and DRCS data" | [o o x x] | Table 1, p.23 |
| X/26/0-15 full enhancement, including objects | [o o x x] | Table 1, p.23 |
| X/26/0-15 "Used for object definition pages" | [o o x x] | Table 1, p.23 |
| X/27/4 Format 1, compositional linking to object and DRCS pages | [o o x x] | Table 1, p.23 |
| X/27/5 Format 1, the same | **[o o o x]** | Table 1, p.23 |
| X/27/6-7 Format 1, "additional compositional links" | application not defined (note 2) | Table 1, p.23 |
| X/27/4-7 Format 2, data broadcasting | [o o o o] | Table 1, p.23 |
| X/28/0 Format 1 page function and page coding | [x x x x], not mandatory (note 3) | Table 1, p.23 |
| Objects (Table 25 row) | [o o x x] | Table 25, p.68 |
| Row-group triplet modes 10000 (Origin Modifier), 10001-10011 (invocation), 10101-10111 (definition), 11000 (DRCS Mode) | [o o x x] | Table 28, pp.79-82 |
| Termination Marker with typed data field (D2..D0) | [o o x x] | Table 28, p.83 |

Table 1 note 1 (p.24): when a packet type has several entries, its meaning comes from the page type. That is found from X/28/0 Format 1 if sent, from context (for example a MOT entry pointing at an object definition page), or from a Code of Practice.

### 1.2 System components (cl. 10.2, Figure 18, p.47) [V]
A Level 2.5 or 3.5 display page is the Level 1 page plus local enhancement data (X/26/0-15, X/28/0, X/28/1, X/28/4) plus "associated pages": POP, GPOP, DRCS and GDRCS, which form the Public Enhancement Data. The DRCS pages may carry X/28/3.
Clause 10.3.1 (pp.47-48) lists what X/26 does at 2.5/3.5; the last two items are "Invoking objects" and "Defining Local Objects".

### 1.3 Purpose (cl. 13, p.91) [V]
Objects add enhancements to Level 1 pages, for example a template or logo reused across pages, or one graphic reused several times in a page. "Under some circumstances an object may invoke further objects."

---------------------------------------------------------------------------------------------------

## 2. The object model

### 2.1 Definition, sources and classes (cl. 13.1, p.91) [V]
- An object is a sequence of packet-26-style triplets that starts with an **Object Definition** triplet. It ends at the next Object Definition triplet or the next Termination Marker triplet. One object may be up to **507 triplets** long.
- Three sources: **Local** (defined in the page's own X/26 packets, after the local enhancement data), **Public** (on a POP) and **Global** (on a GPOP). Local Objects can only be used by the page that defines them. Public and Global objects may be invoked by any number of pages.
- Three types: **Active**, **Adaptive**, **Passive**. Source × type gives "nine different classifications", and all may appear on one page.
- A display page "can have access to 512 global objects and 512 public objects of each type". Local Objects are limited only by X/26 capacity. [D] 512 = 16 sub-pages × 32 objects per type per sub-page (cl. 10.5.1.1, p.49).
- In the X/26 packets, a Termination Marker separates the local enhancement data from the Local Object definitions that follow it (cl. 13.1 p.91; A.3 p.121).

### 2.2 Display priority (cl. 13.2, p.92) [V]
Highest to lowest: **Passive Objects, Adaptive Objects, local enhancement data, Active Objects, Level 1 page.**

### 2.3 Active Objects (cl. 13.3, p.92) [V]
- Intent: a reusable block of local enhancement data. Its triplets act exactly as if they were in the local enhancement data at the current Active Position. The local enhancement data itself "can be considered equivalent to a one-time usable Active Object". Example use: a page template.
- Property 1: default attributes at the Active Position are those of the Level 1 page, as already modified by any Active Objects and local enhancement data at that location. The object may redefine them.
- Property 2: attributes set (or defaulted) in an Active Object act on **every character to the end of the row**, whether the character comes from the object or from the Level 1 page. Later attributes from the object or from the Level 1 page may supersede them.
- Property 3: Active Objects may invoke **Adaptive and Passive** Objects.

### 2.4 Adaptive Objects (cl. 13.4, p.92) [V]
- Intent: overwrite parts of a page with characters that **take on the underlying attributes**. Example: swap Level 1 mosaics for DRCS characters and keep the attributes.
- Property 1: attributes at the Active Position are those of the Level 1 page after any Active Objects and local enhancement data at that position.
- Property 2: those attributes stay in force unless the Adaptive Object changes them. A change affects both the object's characters and the visible Level 1 characters, but only **between the first and last cells the object addresses on that row**.
- Property 3: a change made by the object lasts until another element of the same object changes it, or until the **rightmost cell the object addresses on that row**. To the right of that cell the underlying page's own attributes resume. Exception: a **character set redefinition is not reset at end of row**; it persists to the last character cell of the object or until redefined (also C.4 item 2, p.130).
- Property 4: Adaptive Objects may invoke **Passive Objects only**.
- Implementation consequence [D]: property 3 needs the rightmost addressed column of each row the object touches before attribute effects can be bounded. A decoder either pre-scans the object or renders it in two passes.

### 2.5 Passive Objects (cl. 13.5, p.93) [V]
- Intent: reproduce exactly the same appearance whatever lies underneath, apart from colours, which depend on the Colour Map. Example: a logo or icon.
- Property 1: attributes at the **first defined screen position** start at white foreground (CLUT 0 entry 7), black background (CLUT 0 entry 0), normal size, steady, boxing off, not concealed, contiguous mosaics. The object may redefine them.
- Property 2: attributes last until changed or until the object ends. They are **not reset by a change of Active Position or row**.
- Property 3: attributes take effect **only where the object explicitly defines a character**. Underlying attributes do not affect the object's characters. Attributes "shall be transmitted before the character information". Cells the object does not write show the underlying page unchanged (13.9 note 2, p.95).
- Property 4: Passive Objects **cannot invoke** objects of any type.
- 13.9 note 1 (p.95): "All attributes are reset to their default states at the start of a passive object."

### 2.6 Overlap and interleave (Annex A.7, normative, pp.122-123) [V]
- **Overlap** means two or more objects address the same character cell on a row.
  - Objects of different types may overlap; display priority is Passive > Adaptive > Active.
  - Two or more Active Objects may overlap only if they do not redefine attributes of the same "attribute type".
  - Adaptive Objects "shall not overlap one another".
  - Passive Objects may overlap one another.
  - Where same-type overlap is allowed, **the latest invoked object wins**.
  - The four attribute types: foreground colour; background colour; flashing; display attributes (double height, double width, boxing/window, conceal, invert, separated mosaics/underline).
- **Interleave** means an object addresses cells lying between cells addressed by another object on the same row (example: columns 23-27 = A, A, B, A, B).
  - Different types: freely. Active with Active: freely. Passive with Passive: freely.
  - Adaptive with Adaptive: only if neither redefines any attribute.
- These are transmission constraints. The response of a decoder to a violation is not stated (section 14, item 32).

### 2.7 Attribute contention (Annex C.2, normative, pp.128-129) [V]
When attributes of the same type contend at one cell, the latest wins, judged in this order:
1. where each attribute first takes effect (Set-After spacing attributes take effect one cell later);
2. where each is placed, used only if rule 1 does not decide;
3. relative position in the transmission, used only if rules 1 and 2 do not decide (the example is X/26/0 versus X/26/1).
Examples on p.129: Level 1 alpha-red at n-1 with X/26 foreground-blue at n-1 gives red at n (rule 1). Alpha-red at n-1 with foreground-blue at n gives blue (rule 2). Two X/26 foreground triplets at n from X/26/0 (blue) and X/26/1 (yellow) give yellow (rule 3).

### 2.8 Interaction with double height (Table 26 codes 0/D and 0/F, pp.71-72) [V]
- Enhancement characters, from local X/26 or from objects, may be placed on the lower row of a double-height pair, in cells not covered by the lower halves of enlarged characters, and are displayed. This sub-row is ticked [o o x x].
- For Active and Adaptive Objects, the attributes there are, unless the object defines them, the **start-of-row defaults, modified only by earlier enhancement data on that row**. The same applies to characters written by the page's own local X/26 triplets.
- Lower-row cells where the enhancement data defines no actual character are not changed in any way by the object.
- "Local background colour" for the lower row is the background resulting from Level 1 data, Active Objects and local enhancement data in the cell above.
- Size attributes, spacing or non-spacing: Annex C.3 (pp.129-130). Rule 2 gives priority in scan order; rule 4 says the attributes of an enlarged character's origin apply to the whole character.

### 2.9 Side panels and objects (cl. 12.5, pp.89-91; A.4, p.121; Table 25, p.68) [V]
- Side-panel content is sent **only via objects**. Any character or non-spacing attribute usable in X/26 may be used there, except screen colour and row colour (cl. 12.5.3, p.90).
- Column numbering: normal page 0-39; right panel 40-55, starting at 40; left panel 56-71, always ending at 71 (cl. 12.5.3, p.90). Examples: full right panel = 40-55; full left = 56-71; split 8+8 = left 64-71, right 40-47.
- Side-panel rows are 1 to 24. Nothing is sent for row 0 (cl. 12.5.2, p.90).
- Rows inside a side panel come from the row addresses in the object's elements, unless the Origin Modifier adjusts them (cl. 12.5.3, p.90).
- An object may not cross between the page and a panel, nor from one panel to the other (cl. 12.5.4 rule 4, p.91; Table 25 Objects row, p.68). Annex A.4 (p.121): an object may address only one display area. To cover a panel too, it must invoke another object.
- Panel attribute defaults (cl. 12.5.4 rule 1, p.91) are the Level 1 start-of-row defaults, except that the background is the full row colour. The default character is SPACE (2/0 of G0). A left panel's end-of-row attributes do not carry into the main page, and the right panel does not inherit attributes from the main page (rule 2).
- Screen-address order for A.4: columns 0-39, then the right panel (40-55), then the left panel (56-71) (A.4, p.121).

---------------------------------------------------------------------------------------------------

## 3. Triplet encodings for objects

All are Row Address Group triplets (address ≥ 40), Table 28, pp.79-83 [V].

### 3.1 Origin Modifier, mode 10000 (p.79)
```
Address bits 6..1 : rR5 rR4 rR3 rR2 rR1 rR0   = row offset + 40
Mode    bits 11..7: 1 0 0 0 0
Data    bits 18..12: rC6..rC0                 = column offset
```
- Row offset = address − 40, so 0..23 [D]. Column offset = data, valid 0..71 (allows side panels).
- It applies to **the object whose invocation triplet immediately follows it**. Its offsets are added to the current Active Position to form that object's origin. After one invocation both offsets return to 0 and the Active Position is restored.
- The address field here is not a row address and does not move the Active Position (cl. 12.3.2, p.75: at 2.5/3.5 only modes 00001, 00100 and 00111 set the row) [D].

### 3.2 Object Invocation, modes 10001 / 10010 / 10011 (pp.79-80)
```
Address bits 6..1 : 1  S1 S0 X  N8 N7
Mode    bits 11..7: 1  0  0  M1 M0
Data    bits 18..12: N6 N5 N4 N3 N2 N1 N0
```
- Bit 3 (X) is reserved.
- **Object source (S1 S0):** 00 = illegal; 01 = Local; 10 = POP; 11 = GPOP.
- **Object type (M1 M0):** 00 = Origin Modifier (that is mode 10000); 01 = Active; 10 = Adaptive; 11 = Passive.
- [D] Address bit 6 is always 1 and S1S0 ≠ 00, so the address is ≥ 40 and the triplet falls in the row group. Local (S1S0 = 01) gives address 40, 41, 44 or 45. POP gives 48-55. GPOP gives 56-63. With S1S0 = 00 the address would be < 40 and the triplet would be a column triplet, which is why 00 is "illegal".
- **POP or GPOP object number (N8..N0)**, p.80:
  - N3..N0 (MSB-LSB) = the S1 sub-code digit of the object-page sub-page holding the definition.
  - N8 N7 = which pointer packet: 00 → packet 1, 01 → packet 2, 10 → packet 3, 11 → packet 4.
  - N6 N5 = pointer triplet within that packet (0-based, range 0-12):

    | N6 N5 | Active | Adaptive | Passive |
    |---|---|---|---|
    | 00 | 1 | 2 | 3 |
    | 01 | 4 | 5 | 6 |
    | 10 | 7 | 8 | 9 |
    | 11 | 10 | 11 | 12 |

  - N4 = which half of the pointer triplet: 0 → bits 1-9 (LSB-MSB); 1 → bits 10-18 (LSB-MSB).
- **Local object number**, p.80:
  - N8 = 0.
  - N7..N4 (MSB-LSB) = designation code of the X/26 packet holding the Object Definition triplet (0-15).
  - N3..N0 (MSB-LSB) = triplet number within that packet, range 0-12 (0-based).
  - Note on p.80: a Local Object can be invoked only by the page's own X/26 local enhancement data, directly, or "via another Local Object of the appropriate type" (that is, as clauses 13.3 to 13.5 permit). It cannot be invoked from outside its page or from public objects. Also cl. 13.6 note (p.93): "A Local Object may not be invoked by a global or public object."
- The invocation runs the object's triplet sequence "at each invocation instance". Each element's screen position comes from its own coordinates, the Active Position at the point of invocation, and any Origin Modifier immediately before the invocation (p.79).

### 3.3 Object Definition, modes 10101 / 10110 / 10111 (pp.81-82)
```
Address bits 6..1 : 1  S1 S0 X  N8 N7
Mode    bits 11..7: 1  0  1  M1 M0
Data    bits 18..12: N6 N5 N4 N3 N2 N1 N0
```
- Bit 3 (X) is reserved.
- **Here S1 S0 is the presentation Level at which the object is required, not the source:** 00 = illegal; 01 = Level 2.5; 10 = Level 3.5; 11 = Levels 2.5 and 3.5.
- **M1 M0:** 00 = reserved (that is mode 10100); 01 = Active; 10 = Adaptive; 11 = Passive.
- **N8..N0 are set equal to the corresponding bits of any invocation triplet used to invoke this object.** On POP and GPOP pages: N3..N0 = this sub-page's S1, N8N7 = the packet holding its pointer, N6N5 and N4 as in section 3.2. For Local Objects: N8 = 0, N7..N4 = this packet's designation code, N3..N0 = this triplet's number (0-12).
- [D] A decoder can therefore check that the triplet a pointer lands on is a definition of the expected type with the expected N bits. The standard does not require the check or say what to do on a mismatch (section 14, item 9).
- The definition triplet starts the object; the triplets after it are the object's body (p.81).

### 3.4 Termination Marker, mode 11111 (p.83)
```
Address = 111111 (63), Mode = 11111, Data bits 18..12 = D6..D0
```
- At Levels 2.5 and 3.5, data bits 12-14 (D0..D2) give the type of data ended. Bits 15-18 are reserved.

| D2 D1 D0 | Meaning |
|---|---|
| 000 | Intermediate (G)POP sub-page. End of object; more objects follow on this page. |
| 001 | Intermediate (G)POP sub-page. End of last object on this page. |
| 010 | Last (G)POP sub-page. End of object; more objects follow on this page. |
| 011 | Last (G)POP sub-page. End of last object on this page. |
| 100 | Local Object definitions. End of object; more objects follow. |
| 101 | Local Object definitions. End of last object on this page. |
| 110 | Local enhancement data. End of enhancement data; Local Object definitions follow. |
| 111 | Local enhancement data. End of enhancement data; no Local Object definitions follow. |

- In the last packet in use, the last triplet (bytes 43-45) is a terminator. Unused triplets between the data and that terminator repeat the terminator (p.83).
- The marker may also be a "space-filler" between object definitions, and between the end of local enhancement data and the first Local Object definition (p.83).
- On an object page, "the last used packet of an object sub-page shall contain at least one Termination Marker triplet", with unused triplets filled by repeats (cl. 10.5.1.3, p.50).
- A.3 (p.121): for Level 1.5 compatibility, a Termination Marker should follow the local enhancement data before any Object Definition triplet. Its data field identifies type and position for 2.5/3.5 decoders.

### 3.5 Triplets whose meaning depends on the enclosing object (pp.76-88) [V]

| Triplet | Object-specific rule | Cite |
|---|---|---|
| Full Screen Colour (row 00000) | "restricted to local enhancement data and Active Objects". The address does not move the Active Position for object placement. | p.76 |
| Full Row Colour (row 00001) | Restricted to local enhancement data and Active Objects. Sets row from address, column = 0. | p.77 |
| Address Display Row 0 (row 00111, address 63) | Restricted to local enhancement data and Active Objects. Sets row 0, column 0; following column triplets should address columns 8-39 only. | p.78 |
| Set Active Position (row 00100) | Row from address (40 → 24, 41-63 → 1-23); column from data if < 40. "enables objects to be placed at defined screen locations". | p.77 |
| DRCS Mode (row 11000) | Selects the normal or global DRCS sub-table. **Scope is the object or local enhancement data it occurs in; it is not passed to an invoked object nor inherited on return.** An object that invokes a DRCS character must contain its own DRCS Mode triplet. | p.82; C.4 item 3, p.130 |
| DRCS Character Invocation (column 01101) | Without a DRCS Mode triplet in the same object or local data, both sub-tables default to 0. The default is re-assumed at the start of local data and **at each object invocation**. | p.88 |
| Modified G0/G2 designation (column 01000) | In local data or an Active Object: lasts to end of row or the next 01000. In an Adaptive or Passive Object: applies to that object's characters until its last character cell or the next 01000. Level 2.5 allows only two G0/G2 pairs per page (one is the default); Level 3.5 allows any number. | p.85; C.4 item 2, p.130 |
| Spacing attributes ESC 1/B, 1/9, 1/A, 1/E, 1/F | ESC "does not affect characters written via packets X/26 or as part of an object". 1/9, 1/A, 1/E and 1/F act only on Level 1 / 1.5 characters. | Table 26, pp.73-74 |
| G0 national options | Not applied when G0 characters come from X/26 or objects. | Table 25, p.67; cl. 15.4 note, p.104 |

---------------------------------------------------------------------------------------------------

## 4. Running an object

### 4.1 Position (cl. 13.7, pp.93-94; cl. 12.3.2, pp.75-76) [V]
1. Object origin = Active Position at the invocation + the offsets of an Origin Modifier immediately before the invocation (row and column added separately).
2. Inside the object the Active Position starts at row 0, column 0, **relative to the origin**. The object's triplets then move it as usual.
3. After an object of any type, the Active Position returns to its value at the invocation and the Origin Modifier's effect ends.
4. The Active Position is reset to row 0, column 0 when processing of the local enhancement data starts and when default objects are placed (cl. 12.3.2, p.76). Default objects always have origin 0,0 (cl. 13.8, p.94; G.5, p.141).
- [D] Absolute cell = origin + relative element position. With the Origin Modifier this can reach column 71 (left panel) or row 23+24. The standard gives no clipping or wrapping rule (section 14, item 12).

### 4.2 Nesting [V]

| Invoker | May invoke | Cite |
|---|---|---|
| Local enhancement data | Active, Adaptive, Passive; Local, POP or GPOP | 13.6, p.93; p.80 note |
| Active Object | Adaptive, Passive | 13.3 property 3, p.92 |
| Adaptive Object | Passive only | 13.4 property 4, p.92 |
| Passive Object | nothing | 13.5 property 4, p.93 |
| MOT (default objects) | POP or GPOP objects of any type; never Local | 13.6, 13.8, pp.93-94 |
| Public or global object | never a Local Object | 13.6 note, p.93; p.80 note |

- [D] This makes the nesting depth at most 3 below the local data (Active → Adaptive → Passive) when the type rules are obeyed. An Active-invokes-Active case is not listed as allowed (section 14, item 11).
- Annex A.8 (p.123; Figure A.1 and item 3, p.124) limits:
  1. At most **127 invocations of each object type** per display page, side panels included.
  2. At most **15 objects "open"** at one time, of all types. An object is open from the Active Position at invocation up to and including the last position it explicitly addresses. The open area includes side-panel locations. Figure A.1 shows the open area as a band in scan order from the invocation point to the end point.
  3. At most **15 objects opened in succession** by the local data or any object without an intervening displayable character or attribute being written.

### 4.3 End of an object [V]
- The definition ends at the next Object Definition triplet or Termination Marker (cl. 13.1, p.91; p.81).
- On return: the Active Position and Origin Modifier are restored (13.7, p.94). DRCS Mode state is not passed back (p.82, p.88).
- Attribute life: Active, until superseded or end of row (13.3). Adaptive, until the rightmost addressed cell on the row (13.4). Passive, only on cells it writes, until the object ends (13.5).
- Character-set redefinition in Adaptive or Passive objects ends at the object's last character cell (C.4 item 2, p.130).

### 4.4 Ordering inside objects (Annex A.4, normative, p.121) [V]
- Local enhancement triplets and **object elements shall be in screen-address order**: left to right, top to bottom, with columns 0-39 before 40-55 before 56-71.
- A decoder **may ignore** any triplet from the local data or from an object that addresses a location earlier than the previous triplet from the same source.
- Modes 00001, 00100 and 00111, and all column triplets, move the Active Position and must be in address order.
- Row-colour order: 00000, then 00111, then 00001 (p.122).

### 4.5 Transmission advice that limits what decoders must handle (Annex E, informative, pp.135-136) [V]
- E.6: DRCS characters used in objects invoked by a page shall be in one of the DRCS pages referenced by **that page**, through X/27/4 or X/27/5 or, without them, through its MOT entry. Table 23 note 2 (p.55) states the same as a broadcaster obligation.
- E.7: objects should avoid the first eight cells of row 0, which some decoders reserve. Objects spreading into row 24 are not recommended. Objects in row 0 should appear on the full display but need not be drawn on rolling headers while searching.
- E.12: if two default objects address the same cell, they must not refer to the same attribute nor select more than one new character.

---------------------------------------------------------------------------------------------------

## 5. Finding an object's definition

### 5.1 Local object [V] (p.80, p.82)
```
dc      = (N7..N4)            ; X/26 designation code 0..15
triplet = (N3..N0)            ; 0..12, 0-based (bytes 3*triplet+7 .. +9)
require N8 == 0
```
[D] Values 13-15 of N3..N0 cannot address a triplet (section 14, item 28).

### 5.2 POP or GPOP object [V] (cl. 10.5.1.2, p.50; Table 28, p.80)
1. Choose the page: source POP → the POP linked for the displayed page; source GPOP → the GPOP linked for it (section 7 for the MOT, section 9 for X/27/4). For an object invoked from inside a public or global object, which page's links apply is not stated (section 14, item 10).
2. Choose the sub-page whose S1 = N3..N0.
3. Pointer packet = 1 + (N8N7) (packets 1-4). Its byte 6 must have bit B1 = 1 (pointer data).
4. Pointer triplet (0-based) = 1 + 3·(N6N5) + k, with k = 0 for Active, 1 for Adaptive, 2 for Passive [D, from the table in section 3.2].
5. Pointer value: N4 = 0 → bits 1-9 (bit 1 = LSB); N4 = 1 → bits 10-18 (bit 10 = LSB).
6. Value 0x1FF (all nine bits 1) = "pointer not used": no object with that number.
7. Otherwise the value is the **absolute triplet number** (0..506) of the Object Definition triplet. Absolute triplet 0 is the first triplet of packet 3, "regardless of whether packets 3 and 4 are used to extend the pointer table". Triplet 506 is the last triplet of packet 26/15 (p.50).
8. [D] Mapping, consistent with 506 = last triplet of X/26/15 and the ascending transmission order of B.2 item 3:
   ```
   if t < 299:  packet Y = 3 + t / 13,           triplet = t % 13    ; Y = 3..25
   else:        packet 26, DC = (t - 299) / 13,  triplet = (t - 299) % 13
   ```
9. Within one sub-page and one type, object index = 8·(N8N7) + 2·(N6N5) + N4, range 0..31 [D]. This matches the pointer-table figure (section 6.4): packet 1, triplet 1 = C0 (bits 1-9) and C1 (bits 10-18).
10. Clause 10.5.1.2 (p.50): the invocation lets a decoder "locate rapidly the pointer" in the object page "specified by the MOT or packets X/27/4 or X/27/5".

### 5.3 Reading the object body [D]
Start at the definition triplet. Read the following triplets in order: triplet 12 of packet Y is followed by triplet 0 of packet Y+1, and packet 25 is followed by X/26/0. Stop at the next Object Definition or Termination Marker triplet. The 507-triplet maximum (cl. 13.1) equals the whole definition area, so an object may span every packet from 3 to 26/15. "The definition of an object cannot continue from one sub-page to the next" (cl. 10.5.1.1, p.49; 10.5.1.3, p.50).

---------------------------------------------------------------------------------------------------

## 6. POP and GPOP pages

### 6.1 Identification [V]
- Page function in X/28/0 Format 1 triplet 1 bits 1-4 (Table 3, p.29): 0000 LOP; 0001 data broadcasting; **0010 GPOP**; **0011 POP**; 0100 GDRCS; 0101 DRCS; **0110 MOT**; **0111 MIP**; 1000 BTT; 1001 AIT; 1010 MPT; 1011 MPT-EX; 1100 trigger messages; others reserved. The same coding is used in the first 7 bits of X/28/3 and X/28/4.
- Page coding, bits 5-7 (Table 3, p.29): 000 = 7 data bits + odd parity; 001 = 8 data bits; **010 = per packet, one Hamming 8/4 byte then 13 Hamming 24/18 triplets** (POP/GPOP); **011 = all bytes Hamming 8/4** (MOT, MIP); 100 = per packet, 8 Hamming 8/4 bytes then 12 parity bytes, repeated for 20 bytes; 101 = per packet, the first Hamming 8/4 byte selects one of codings 000-100 for that packet only. Others reserved.
- For page functions other than 0000 and 0001, X/28/0 Format 1 triplet 1 bits 8-18 = 11111111100, so that EN 300 708 data-broadcast decoders ignore the page. Triplets 2-13 are reserved (Table 6, cl. 9.4.2.4, p.31).
- X/28/0 is optional on a (G)POP (cl. 10.5.1.1, p.49). Annex A.6 (p.122), however, says that an object page using packets X/1-X/25 **shall** send X/28/0 Format 1 to indicate the packet format (section 14, item 8).
- Annex D.1 (p.131): a 2.5/3.5 decoder may conclude that a page is not a basic text page by looking at the MOT, at other pages' X/27/4 links, or at a Code of Practice.
- Annex E.1 (p.134): object and DRCS pages should have a hex digit in their page number so that they are not captured as display pages.
- MIP page codes (Table 24, p.64): E6 = "Object page (use not defined)"; EC-EF give the page's relationship to MOTs (section 10).

### 6.2 Page format (cl. 10.5.1.1, Figure 19, p.49) [V]
- Page header + packets 1-25 + packets 26/0-26/15, plus an optional 28/0.
- Packets 1-25 are Hamming 24/18 coded like X/26. Byte 6 of X/1-X/25 is Hamming 8/4 but is **not** a designation code (Figure 11 note, p.27).
- Unused packets need not be sent. Partial updates are allowed.
- Up to **seven POPs and one GPOP per magazine** (cl. 10.5.1, p.49).
- Up to **16 sub-pages**, told apart by S1 of the sub-code. Each sub-page may define up to **32 objects of each type**. An object cannot continue onto another sub-page.
- Triplet coding in packets 1-25 is that of clauses 12.3.3 and 12.3.4 (cl. 10.5.1.3, p.50; cl. 12.3.1, p.74).

### 6.3 Byte 6 of packets 1-4 (cl. 10.5.1.2, p.49) [V]

| B4 B3 B2 B1 | Function |
|---|---|
| X X X 0 | packet carries object definition data |
| X X X 1 | packet carries pointer table data |

- X = reserved.
- Byte 6 of packets 5-25: all four bits reserved (cl. 10.5.1.3, p.50).
- Byte 6 of packets 26: the normal designation code.
- Packets 1 and 2 are reserved for the pointer table. Packet 3 extends it when any type has 17-24 objects; packet 4 when a type has up to 32. "It is not allowed to mix both functions within one packet."
- Packet 3 may hold definitions while packet 4 holds pointers, "providing the objects are defined and terminated within packet 3".

### 6.4 Pointer table layout (p.50) [V]
Triplet 0 of each pointer packet is reserved. Triplets 1-12 each hold two 9-bit pointers to consecutive objects of one type, in the repeating order Active, Active, Adaptive, Adaptive, Passive, Passive:

| Pkt | t0 | t1 | t2 | t3 | t4 | t5 | t6 | t7 | t8 | t9 | t10 | t11 | t12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | res | C0 C1 | D0 D1 | P0 P1 | C2 C3 | D2 D3 | P2 P3 | C4 C5 | D4 D5 | P4 P5 | C6 C7 | D6 D7 | P6 P7 |
| 2 | res | C8 C9 | D8 D9 | P8 P9 | C10 C11 | D10 D11 | P10 P11 | C12 C13 | D12 D13 | P12 P13 | C14 C15 | D14 D15 | P14 P15 |
| 3 | res | C16 C17 | … | … | … | … | … | … | … | … | … | … | P22 P23 |
| 4 | res | C24 C25 | … | … | … | … | … | … | … | … | … | … | P30 P31 |

- C = Active, D = Adaptive, P = Passive.
- Bits 1-9 = pointer to the **even**-numbered object; bits 10-18 = the **odd**-numbered object.
- If a whole pointer packet is unused it need not be sent. An unused pointer is 0x1FF.
- Default objects from the MOT can reach only the first 16 objects of each type on each sub-page, that is packets 1 and 2 (p.50; section 7.6).

### 6.5 Definition area (cl. 10.5.1.3, p.50) [V]
Definitions start at the first triplet of the packet after the last pointer packet: packet 3, 4 or 5. They may fill packets up to 25 and continue in 26/0 to 26/15. Unused packets need not be sent. The last used packet carries at least one Termination Marker; unused triplets after it repeat it.

### 6.6 Sub-code of data pages (Annex A.1, p.120) [V]
For MOT, POP, GPOP, DRCS, GDRCS and MIP the sub-code is coded `nnXs`:
- **nn** = S4 (2 MSBs) and S3 (4 LSBs): a 6-bit number giving the last packet that would be sent in an uninterrupted transmission. 1-25 = X/1-X/25; 26-41 = X/26/0-X/26/15. During a partial transmission it gives the last packet of that transmission. It is not changed when the page is sent in fragments.
- **X** = S2: update indicator, incremented modulo 8 on each change of the sub-page. It may also step when no change was made, after a multiple of 8 changes since the last full transmission.
- **s** = S1: sub-page number, 0 for the first sub-page, incremented for each further one. "For MIP and MOT data pages the value should always be set to 0."

### 6.7 Transmission rules (Annex B, normative, pp.125-127) [V]
- B.2 item 3: pages for further processing, which include enhancement pages, send packets 1 to 26 in ascending packet address, with 26 in ascending designation code. After an interruption, Y = 27 and Y = 28 are re-sent after the repeated header. The header is followed by Y = 27 and then Y = 28 (B.2 item 1).
- B.6:
  1. Every enhancement page (MOT, POP, GPOP, DRCS, GDRCS) and every non-page enhancement packet (M/29) is sent **in full at least every 20 s**.
  2. No 20 ms erasure gap is needed.
  3. Pages may be sent in continuous bursts.
  4. Sub-code per A.1.
  5. With C4 set, the decoder should erase all packets of the previous version.
  6. Partial updates (header + changed packets) may be sent between full ones, with **C13 = 1 for a partial page**.
  7. A full or partial page may be sent in fragments; **C12 = 0 on the first attempt** and 1 on the headers that continue after an interruption.
  8. C14 is reserved.
- Table B.1 (p.127): C4, C8, C12, C13 and S2 give the page status, where N is the S2 of the previous header:

  | C4 | C8 | C12 | C13 | S2 | Meaning |
  |---|---|---|---|---|---|
  | 0 | 0 | 0 | 0 | N | full page, complete or first fragment, no change |
  | 0 | 0 | 1 | 0 | N | full page, second or later fragment, no change |
  | 1 | 1 | 0 | 0 | N+1 | full page, complete or first fragment, changed, erase old page |
  | 0 | 1 | 1 | 0 | N | full page, second or later fragment, changed |
  | 0 | 1 | 0 | 1 | N+1 | partial page, complete or first fragment, changes implied |
  | 0 | 1 | 1 | 1 | N | partial page, second or later fragment, changes implied |

  "A combination of this information and the S3 and S4 components of the sub-code allow a decoder to determine if all the packets of a page have been received."
- [D] On enhancement pages, C12, C13 and C14 are therefore **not** national-option bits.
- B.7: on a change from one coherent state to another the order is: no more old-state data; the updated MOT; updated DRCS pages; updated object pages; then the updated Level 1 page.
- B.9: at most **100 ms** between successive packets of a page. The note says this lets software decoders decide to display what they have if the closing header is late.
- F.2 (informative, p.138): enhancement pages may be interrupted and continued by re-sending the same header and then only the rest of the page.
- D.1.1 (p.131): object pages use X/1-X/25 sequentially from X/1; unused rows need not be sent. If a packet before the Termination Marker is missing, the decoder may treat it as an error; the response is not defined.

---------------------------------------------------------------------------------------------------

## 7. The Magazine Organization Table (MOT)

### 7.1 Identity and coding (cl. 10.6, 10.6.1, pp.51-52; A.10, p.124) [V]
- Page number **mFE** in every magazine (reserved, A.10). Each transmitted magazine contains one.
- The sub-code and control bits signal partial or full updates and the last packet, as in A.1 and B.6. S1 = 0.
- It has a normal header and packets X/1-X/24, **all Hamming 8/4**, so every byte is a 4-bit nibble.
  - X/1-X/14: one byte pair per page number.
  - X/15-X/18 and the unused bytes of X/9-X/14: reserved ("Not used").
  - X/19-X/21: Level 2.5 links. X/22-X/24: Level 3.5 links.
- An optional X/28/0 Format 1 gives page function and coding, for EN 300 708 Page Format - CA compatibility.
- Purpose:
  - link each Level 1 page to up to two object pages (a GPOP and a POP) and up to two DRCS pages (a GDRCS and a DRCS), for all its sub-pages unless X/27/4 redefines them per sub-page;
  - give default side-panel and black-background-substitution settings, so that these work where packets above 23 cannot be distributed.

### 7.2 Byte-pair position for page xTU (Figure 20, p.52) [V]
- X/1-X/5: decimal pages x00-x99, 20 per packet. X/1 = x00-x19, …, X/5 = x80-x99.
- X/6-X/8: tens A-F, units 0-9 (xA0-xF9), 20 per packet.
- X/9-X/14: units A-F for every tens digit, 18 per packet: X/9 = x0A-x0F, x1A-x1F, x2A-x2F; …; X/13 = xCA-xEF; X/14 = xFA-xFF (6 pairs).
- Byte pairs start at byte 6.

[D] Algorithm (pairs at bytes b, b+1):
```
if T <= 9 and U <= 9: i = 10*T + U;        packet = 1 + i/20; b = 6 + 2*(i%20)
elif U <= 9:          i = 10*(T-10) + U;   packet = 6 + i/20; b = 6 + 2*(i%20)
else:                 i = 6*T + (U-10);    packet = 9 + i/18; b = 6 + 2*(i%18)
```
Checks against Figure 20 [D]: x00 → X/1 b6; x19 → X/1 b44; x99 → X/5 b44; xA0 → X/6 b6; xF9 → X/8 b44; x0A → X/9 b6; x2F → X/9 b40; xFE → X/14 b14; xFF → X/14 b16.

### 7.3 The byte pair (cl. 10.6.2, 10.6.3, p.52) [V]
- **First byte, object page association:**
  - bit 4 = global object flag (1 = the page needs objects from the GPOP);
  - bits 3..1 = POP link number (MSB-LSB): 000 = no public objects; 1-7 select POP link 1-7.
- **Second byte, DRCS page association:**
  - bit 4 = global DRCS flag (1 = the page needs the GDRCS);
  - bits 3..1 = DRCS link 1-7, or 000 = none.
- [D] At Level 3.5 the same link numbers index the Level 3.5 links in X/22-X/24 (cl. 10.6.4, 10.6.5: "Each Level 1 page can point to one of these links").

### 7.4 Object links (cl. 10.6.4, Table 20, p.53) [V]
Each link is 10 bytes, numbered 1-45 within the packet.

| Link | Level 2.5 packet | Level 3.5 packet | First byte |
|---|---|---|---|
| GPOP | X/19 | X/22 | 6 |
| POP 1 | X/19 | X/22 | 16 |
| POP 2 | X/19 | X/22 | 26 |
| POP 3 | X/19 | X/22 | 36 |
| POP 4 | X/20 | X/23 | 6 |
| POP 5 | X/20 | X/23 | 16 |
| POP 6 | X/20 | X/23 | 26 |
| POP 7 | X/20 | X/23 | 36 |

### 7.5 Object link coding (Table 21, p.54) [V]
| Byte | Bits | Field |
|---|---|---|
| N | 1-3 | Magazine of the object page (section 14, item 1 on absolute versus relative) |
| N | 4 | X/27/4,5 flag: 0 = link page number valid for all sub-versions of the display page; 1 = link page number may be superseded by X/27/4 or X/27/5 data |
| N+1 | 1-4 | Page tens |
| N+2 | 1-4 | Page units. Page number xFF = link not used. |
| N+3 | 1-4 | Number of sub-pages, as the highest S1 transmitted: 0 = 1 sub-page … F = 16 |
| N+4 | 1 | Fallback protocol (POP links only; reserved in the GPOP link). When the X/28 and M/29 packets carrying side-panel and black-background-substitution data are absent: 1 = no side panel and no substitution; 0 = bits 2-4 apply |
| N+4 | 3,2 | Default side panel (only when bit 1 = 0): 00 none; 01 left, 16 columns; 10 right, 16 columns; 11 left and right, 8 columns each |
| N+4 | 4 | Default background (only when bit 1 = 0): 0 = no substitution; 1 = black background replaced by full screen/row colour |
| N+5 | 1-4 | Default object flags (section 7.6) |
| N+6, N+7 | 1-4 each | 1st default object number |
| N+8, N+9 | 1-4 each | 2nd default object number |

- D.2.1 items 3 and 7 (p.133): without M/29/0 (and X/28/0), side panels and substitution are determined from "the page's public object link in the MOT". Without a MOT: no side panels and no substitution.
- Cl. 12.5.1 (p.90): "limited side-panel requirements can be indicated through the POP links in the MOT". With no information, display 40 columns only.

### 7.6 Default objects in a link (cl. 10.6.4.1, pp.54-55) [V]
- Byte N+5:
  - bits 2,1 = type of the 1st default object;
  - bits 4,3 = type of the 2nd;
  - each pair: 00 none; 01 Active; 10 Adaptive; 11 Passive.
- Object number, 8 bits: byte N+6 bits 4..1 = N3..N0; byte N+7 bits 4..1 = N7..N4. The 2nd object uses N+8 and N+9 in the same way.
  - N3..N0 = S1 of the sub-page holding the definition.
  - **N7** = pointer packet: 0 → packet 1; 1 → packet 2.
  - N6 N5 = pointer triplet, using the same table as section 3.2.
  - N4 = bits 1-9 or bits 10-18.
- [D] This is the 9-bit invocation number with N8 = 0 and N7 restricted to packets 1-2. Hence "restricted to the first 16 objects of each type on each sub-page" (p.50).

### 7.7 DRCS links (cl. 10.6.5, Tables 22-23, p.55) [V]
- Each link is 4 bytes, in X/21 (Level 2.5) or X/24 (Level 3.5): GDRCS at byte 6; DRCS 1-7 at bytes 10, 14, 18, 22, 26, 30, 34. Bytes 38-39 are reserved.
- Coding:
  - byte N bits 1-3 = magazine; byte N bit 4 = X/27/4,5 flag (as in section 7.5);
  - N+1 = page tens; N+2 = page units (xFF = link not used);
  - N+3 = number of sub-pages (0 = 1 … F = 16).
- Table 23 note 2: the broadcaster ensures that DRCS characters used in an object invoked by a page are referenced, implicitly or explicitly, by that page's MOT link or DRCS pages.
- The DRCS page format itself is outside this note (cl. 14).

### 7.8 Number of enhancement pages (cl. 10.6.6, p.56) [V]
Bytes 40-45 of X/21 (Level 2.5 totals) and X/24 (Level 3.5 totals), all Hamming 8/4:

| Byte | bit 4 | bit 3 | bit 2 | bit 1 |
|---|---|---|---|---|
| 40 | M3 | M2 | M1 | M8 |
| 41 | M7 | M6 | M5 | M4 |
| 42 | P3 | P2 | P1 | P0 |
| 43 | P7 | P6 | P5 | P4 |
| 44 | D3 | D2 | D1 | D0 |
| 45 | D7 | D6 | D5 | D4 |

- Mn = magazine n is included in the totals.
- P7..P0 = total number of object pages, public and global (0-FF).
- D7..D0 = total number of DRCS pages.
- The totals count all sub-pages of pages referenced by MOTs and local X/27.

### 7.9 Level 3.5 use of the MOT (cl. 10.6.1, p.51) [V]
- A Level 3.5 service must send at least X/22 and X/24 (they carry the links to global data).
- A Level 3.5 decoder looks first for X/22-X/24. If **either X/22 or X/24 is absent**, it uses X/19-X/21 and processes the page to the Level 2.5 standard. X/23 may be absent.

### 7.10 Defaults when packets are absent (Annex D) [V]
- D.1.4 (p.132): with no X/27/4, 2.5/3.5 decoders assume the MOT has valid object and DRCS page numbers for the page.
- D.1.2 item 2 (p.131): with no X/26, 2.5/3.5 decoders assume no enhancement except **default objects, if any**, and defaults from X/28/0 Format 1, X/28/4 and M/29/0,4.

---------------------------------------------------------------------------------------------------

## 8. Default objects

Sources: cl. 13.8, p.94; E.11, E.12, p.136; G.5, p.141; A.2 item 3, p.121 [V].
- Default objects add objects to a page with no local X/26 at all. They are selected through MOT data.
- **Per page:** up to two, through the page's POP link (section 7.6). "The object shall exist within the set of POP sub-pages defined by the MOT link for that page."
- **Per magazine:** up to two, through the **global (GPOP) link**, applying to "all pages in a magazine". They come from the GPOP sub-pages defined by that link.
- Origin: the cursor position at the start and end of a default-object invocation is row 0, column 0. G.5 repeats that the origin is fixed at 0,0 and the Active Position returns to 0,0.
- **If the page has any local enhancement data (X/26), the decoder should ignore all default-object instructions.** All objects for that page must then be invoked from its X/26.
  - E.11 explains why: a decoder cannot tell X/26 sent only for a Level 1.5 display from X/26 meant for higher levels, so a Level 1.5 page that wants objects must invoke them itself.
- A Local Object cannot be a default object.
- G.5 (informative): with no local X/26 and default objects of the same type, the preferred invocation order is **GPOP 1, GPOP 2, POP 1, POP 2**. With different types, the type sets the priority (cl. 13.2).
- E.12: two default objects addressing the same cell must not refer to the same attribute nor select more than one new character.
- A.2 item 3: the C4 (erase) and C8 (update) bits of the displayed page also apply to data written by a default object.

---------------------------------------------------------------------------------------------------

## 9. Packets X/27/4 and X/27/5, Format 1 (compositional links)

### 9.1 Layout (cl. 9.6.2, Figure 14, Table 15, pp.39-40) [V]
- Designation codes 0100 (X/27/4) and 0101 (X/27/5). They link a page to enhancement data pages at Levels 2.5 and 3.5.
- Byte 6 = designation code, Hamming 8/4. Bytes 7-42 form **six links of two triplets each** (link 0 = triplets 1-2, link 1 = 3-4, …, link 5 = 11-12; Figure 14 triplet numbering 1-13). Triplet 13 (bytes 43-45) is Hamming 24/18 with reserved data.
- Each link carries 36 bits: relative magazine 3, page number 8, sub-page flags 16, link function 4 [sic: only 2 bits are defined below], compatibility 2, reserved 3 (p.40).

| Triplet | Bits | Field |
|---|---|---|
| 1st | 1-2 | **Link function** (bit 2, bit 1): 00 GPOP; 01 POP; 10 GDRCS; 11 DRCS |
| 1st | 3-4 | **Page validity** (bit 4, bit 3): 00 reserved; 01 required at Level 2.5 only; 10 at Level 3.5 only; 11 at both |
| 1st | 5-6 | reserved |
| 1st | 7-10 | page units (LSB-MSB) |
| 1st | 11 | **set to 1** (compatibility with Format 2) |
| 1st | 12-14 | **relative magazine** (LSB-MSB): each 1 bit complements the corresponding bit of the magazine in byte 4 of this X/27 |
| 1st | 15-18 | page tens (LSB-MSB) |
| 2nd | 1 | **set to 0** (compatibility with Format 2) |
| 2nd | 2 | reserved |
| 2nd | 3-18 | **sub-page flags**: bit k (3..18) = 1 means the sub-page with S1 = k−3 is required |

- "No particular page" is coded as page number FF with **all sub-code flags set to 1** (p.40).
- [D] The 36-bit count on p.40 lists "Link function flags: 4 bits" and "Reserved: 3 bits". Table 15 shows link function 2 bits + page validity 2 bits = 4, and reserved bits 1.5, 1.6 and 2.2 = 3. So "link function flags" means function and validity together.
- [D] Magazine: `target_mag_code = packet_mag_code XOR rel`, on the 3-bit codes where code 0 means magazine 8 (cl. 3.1, p.11 "magazine number 8"; the same rule as X/27/0, cl. 9.6.1, p.38).

### 9.2 Fixed link roles (Table 16, p.41) [V]

| Packet | Link | Function | Fixed usage | Optional usage |
|---|---|---|---|---|
| X/27/4 | 0 | GPOP | Level 2.5 | Level 3.5 |
| X/27/4 | 1 | POP | Level 2.5 | Level 3.5 |
| X/27/4 | 2 | GDRCS | Level 2.5 | Level 3.5 |
| X/27/4 | 3 | DRCS | Level 2.5 | Level 3.5 |
| X/27/4 | 4, 5 | from link function bits | – | Level 3.5 |
| X/27/5 | 0, 1 | from link function bits | – | Level 3.5 |
| X/27/5 | 2-5 | reserved | – | – |

- Note to Table 16: "Duplicate settings are invalid", for example two GPOP links.
- Text (p.41): the first four X/27/4 links are fixed for Level 2.5; page validity may also mark them for Level 3.5. X/27/4 links 4-5 and X/27/5 links 0-1 are defined by their function and validity bits and "do not contain information relevant to a Level 2.5 decoder".
- Figure 22 and cl. 10.7.2 (pp.58-59) show the same four Level 2.5 links and say that Level 3.5 might use the last two links of X/27/4 and the first two of X/27/5.

### 9.3 Precedence and use [V]
- Cl. 10.5.1.4 (p.50) and cl. 10.5.2.2 (p.51): with X/27/4 (and X/27/5 if needed) present, "the actual sub-pages carrying the objects [DRCS data] required by the page as well as the page numbers themselves can be identified", and their data **takes precedence over the MOT**.
- E.4 (informative, p.134): X/27/4 gives a complete definition of the enhancement pages, including the 16 sub-page flags that the MOT lacks, so it can speed up processing. It is needed for "news-reel" multi-page sets whose sub-pages use different POP and DRCS pages, possibly in other magazines. X/27/5 is used the same way at Level 3.5.
- Cl. 10.6 (p.51): the MOT link holds for the whole set of sub-pages of a Level 1 page "unless redefined by X/27/4 for individual sub-pages". [D] X/27/4 is page-related, so each sub-page of the display page can carry its own.
- B.2 (p.125): X/27 follows the header and precedes X/28. For display pages it is re-sent after an interruption.

### 9.4 Format 2, for recognising it only (cl. 9.6.3, Table 17, pp.41-42) [V]
- Designation codes 0100 to 0111, used for EN 300 708 Page Format - CA. The same 6 × 2-triplet layout.
- Triplet 1: bits 1-10 = link control data (part 1); bit 11 = link control (part 2); bits 12-14 = relative magazine; bits 15-18 = tens.
- Triplet 2: bit 1 = link control (part 3); bits 2-5 = units; bits 6-7 = S4; 8-11 = S3; 12-14 = S2; 15-18 = S1.
- Bits 1-10 have their defined meaning only when triplet 1 bit 11 = 0 **and** triplet 2 bit 1 = 0.
- [D] Format 1 sets these to 1 and 0 respectively, so a Format 2 decoder reads a Format 1 link as "reserved". A presentation decoder can recognise a Format 1 link by triplet 1 bit 11 = 1. Table 1 marks Format 2 as [o o o o].

---------------------------------------------------------------------------------------------------

## 10. Magazine Inventory Page (MIP), as far as it concerns object pages

Sources: cl. 11.3, pp.62-66; Annex J, pp.146-147; B.8, p.127 [V].
- Page **mFD**. Sub-code per A.1 with S1 = 0 (Annex J gives mFD:nnx0). Packets X/1-X/24 Hamming 8/4; optional X/28/0 Format 1. Sent in full every 20 s at most, with a 20 ms erasure interval (B.8).
- X/1-X/14: one byte pair per page, mapped exactly like the MOT (Figure 26, p.63; same algorithm as section 7.2). Page code = second byte (high nibble D7..D4) : first byte (low nibble D3..D0) (cl. 11.3.3, p.63).
- X/15-X/24: 3-byte groups giving the number of sub-pages for codes that need it (cl. 11.3.4, p.66). These are not relevant to object pages.
- Codes relevant here (Table 24, pp.64-65):

  | Code | Meaning |
  |---|---|
  | E5 | DRCS page, use not defined |
  | E6 | Object page, use not defined |
  | E7 | systems page without displayable element (for example MOT, MIP) |
  | E8 | DRCS page referenced in this magazine's MOT |
  | E9 | DRCS page referenced in this magazine's MOT but not required by a page in this magazine |
  | EA | DRCS page referenced in another magazine's MOT but not required by a page in this magazine |
  | EB | DRCS page not referenced in another magazine's MOT and required by a page in another magazine |
  | EC | Object page referenced in this magazine's MOT |
  | ED | Object page referenced in this magazine's MOT but not required by a page in this magazine |
  | EE | Object page referenced in another magazine's MOT but not required by a page in this magazine |
  | EF | Object page not referenced in another magazine's MOT and required by a page in another magazine |

  Note 8 (and Table J.1): the decoder can use these codes to decide the priority of storing a DRCS or object page; they complement the MOT.
- Annex J (p.146): a decoder should not take the absence of an X/1-X/14 packet to mean the pages are absent; only code 00 means "not in transmission". On each MIP update the decoder compares with the previous version.
- The MIP is optional ("may be transmitted", cl. 11.3.1). It does not replace the MOT for linking.

---------------------------------------------------------------------------------------------------

## 11. Acquisition: what a receiver must capture for a displayed page

### 11.1 Pages and packets needed for a display page P in magazine m (Level 2.5) [D, from sections 6-9]
1. P's own packets: X/0, X/1-X/25, X/26/0-15 (local data and Local Object definitions), X/27/4 (and X/27/5 at 3.5), X/28/0, X/28/1, X/28/4.
2. Magazine packets M/29/0 (and M/29/1, M/29/4 at 3.5) for magazine m.
3. **MOT m/FE, S1 = 0**: the byte pair for P, then the object links (X/19, X/20) and DRCS links (X/21). At 3.5: X/22-X/24, with fallback per section 7.9.
4. If X/27/4 is present, its links override the MOT links (section 9.3). Its sub-page flags say which S1 sub-pages are needed.
5. The GPOP (if the global flag is set, or always for magazine-wide default objects, section 14 item 19) and the POP (if the link is 1-7). The sub-pages needed are those whose S1 is named by any invocation (N3..N0) in P's local data or in objects reached from it, or by the default-object numbers. The MOT "number of sub-pages" (N+3) bounds S1. Object pages may be in another magazine; their packets then arrive with that magazine's address.
6. GDRCS and DRCS pages and sub-pages as named by DRCS Mode triplets (T3..T0) in P's local data and in the objects invoked. This is covered by the DRCS notes, not here.

### 11.2 Knowing an enhancement page is complete [V/D]
- S4S3 of the sub-code gives the last packet number (1-41) of this transmission (A.1). The C12/C13 state and S2 (Table B.1) say whether this is a full or partial transmission and whether it is a continuing fragment. Together "allow a decoder to determine if all the packets of a page have been received" (B.6, p.127) [V].
- The last used packet of an object sub-page contains a Termination Marker (cl. 10.5.1.3) [V]. Marker types 0x0 and 0x1 versus 0x2 and 0x3 mark intermediate versus last (G)POP sub-page (Table 28, p.83) [V].
- Unused packets need not be sent (cl. 10.5.1.1), so "every packet 1..nn received" is not a valid test. Only packets actually used are sent. [D] A practical test is: header received, all packets up to the one holding the termination have been seen in sequence (ascending, B.2 item 3), and no gap longer than 100 ms (B.9).
- C4 = 1 on a full transmission means: erase the stored version before storing (B.6 item 5) [V].
- S2 changes on content change (A.1) [V]. [D] A changed S2 should invalidate cached decoded objects from that sub-page.

### 11.3 Timing [V]
- Enhancement pages and M/29: in full at least every 20 s (B.6 item 1). The MIP likewise (B.8).
- The update order is MOT, then DRCS, then object pages, then the Level 1 page (B.7). [D] So when a Level 1 page changes its enhancement references, the new MOT and object pages should already be on air.
- At most 100 ms between packets of one page (B.9).
- The standard does not say whether a decoder should delay display until enhancement pages arrive, or show Level 1 first and redraw (section 14, item 30).

### 11.4 Storage bounds (informative) [V]
- E.8 (p.135): the total number of packets used for DRCS and object pages over **all magazines** shall not exceed **500 at Level 2.5** and **1 200 at Level 3.5**.
- G.4 (p.141): 2.5/3.5 decoders should have memory for the maximum number of packets used for DRCS and object pages, all sub-pages included, at their level.
- [D] Raw store: 500 × 40 data bytes = 20 000 bytes (2.5); 1 200 × 40 = 48 000 bytes (3.5), plus page and sub-page indexing. This is a fixed bound suitable for a no-allocation core.
- [D] Per page: one (G)POP sub-page holds at most 41 data packets (X/1-X/25 + X/26/0-15). 16 sub-pages give 656 packets for one object page, which exceeds the 2.5 service limit. The service-wide E.8 cap therefore governs.

---------------------------------------------------------------------------------------------------

## 12. Level 2.5 versus Level 3.5 for objects

| Topic | Level 2.5 | Level 3.5 | Cite |
|---|---|---|---|
| Object mechanism (types, invocation, definition, origin modifier) | yes | yes, identical coding | Table 28, pp.79-83 |
| Object Definition usage field S1S0 | 01 or 11 = used | 10 or 11 = used | p.81 |
| MOT links used | X/19-X/21 | X/22-X/24; fall back to X/19-X/21 if X/22 or X/24 is missing | cl. 10.6.1, p.51 |
| MOT enhancement-page totals | X/21 bytes 40-45 | X/24 bytes 40-45 | cl. 10.6.6, p.56 |
| X/27/4 links 0-3 | fixed GPOP, POP, GDRCS, DRCS | optional, by page validity bits | Table 16, p.41 |
| X/27/4 links 4-5, X/27/5 links 0-1 | ignored | defined by function and validity bits | p.41 |
| Modified G0/G2 designation inside objects | at most 2 pairs per page | any number | Table 29, p.85 |
| Font style triplet (01110) inside objects | not available | available | Table 29, p.88 |
| DRCS Mode L1L0 | 01 or 11 | 10 or 11 | p.82 |
| DRCS modes usable in objects | 12x10x1 only (24 characters) | four modes | Table 25, p.68 |
| Side-panel status flag (X/28/0 triplet 2 bit 6) | used only if flag = 1 | always | Table 4, p.30 |
| Global enhancement packet budget | 500 | 1 200 | E.8, p.135 |
| CLUTs 0 and 1 redefinable (affects passive object colours) | no | yes (X/28/4, M/29/4) | Table 30, p.89 |

[D] Apart from the links, page validity, the definition usage field and what the triplets inside may do, object handling is the same at both levels.

---------------------------------------------------------------------------------------------------

## 13. Worked examples for tests

### 13.1 Clause 13.9 object example (pp.94-95) [V; column numbers read from the figure image]
Level 1 row, columns 0-27:
```
col: 0  1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27
     αB T H E _ F A S T _ D  O  G  _  A  N  D  αR C  A  T  αG R  A  N  _  I  N
```
- αB, αR and αG are alpha blue, red and green spacing attributes (Set-After).
- Object: "LAZY" at 5-8; "COW" at 10-12 with non-spacing foreground pink (fP) at 10; "FOX" at 18-20; "SAT" at 22-24 with foreground yellow (fY) at 22. The object's range is columns 5-24.
- [U] "Pink" is the spec's name for a non-spacing foreground colour. The CLUT entry is not given; magenta, or a CLUT 2/3 entry, both fit.

Expected foreground colour of each word:

| Word | Active | Adaptive | Passive |
|---|---|---|---|
| THE | blue | blue | blue |
| LAZY | blue | blue | white (note 1: passive starts from defaults) |
| COW | pink | pink | pink |
| AND | pink (note 7: object recolours the Level 1 word in the gap) | pink (note 7) | blue (note 2: underlying word unaffected) |
| FOX | red (note 3: takes underlying attributes; αR at 17 takes effect at 18) | pink (note 8: object already changed foreground, so later Level 1 changes inside the range are ignored) | pink (note 4: own attribute set) |
| SAT | yellow | yellow | yellow |
| IN | yellow (note 5: Active attribute continues past the last addressed cell) | green (note 6: effect ends at last addressed cell 24, then αG resumes) | green (note 6) |

### 13.2 Derived triplet encodings [D]
- Computed from the clause 8.3 equations by a script in the session scratchpad. Each was checked to pass all six parity tests (A-F) and to round-trip to the same 18-bit value. Both the encoder and the checker were written from my reading of the standard, so **confirm independently before using them as golden values**.
- Bytes are listed as received, with bit 0 = first bit on air. Value = address | mode<<6 | data<<11.

| Triplet | Address | Mode | Data | 18-bit value | Bytes |
|---|---|---|---|---|---|
| Active invocation, POP, S1 = 2, pointer packet 1, N6N5 = 01, N4 = 1 (object C3) | 48 | 10001 | 0x32 | 0x19470 | 81 47 32 |
| Matching Active Object Definition, usage Levels 2.5 and 3.5 | 56 | 10101 | 0x32 | 0x19578 | 43 57 32 |
| Passive invocation, Local, DC = 1, triplet 5 | 40 | 10011 | 0x15 | 0x0ACE8 | C9 4E 15 |
| Origin Modifier, row +2, column +56 | 42 | 10000 | 0x38 | 0x1C42A | DB 42 38 |
| Termination Marker, type 111 | 63 | 11111 | 0x07 | 0x03FFF | 74 7F 87 |
| Termination Marker, type 010 | 63 | 11111 | 0x02 | 0x017FF | 76 7F 02 |
| Pointer triplet: C2 unused (0x1FF), C3 = 30 | – | – | – | 0x03DFF | FE 5F 87 |
| X/27/4 link, 1st triplet: POP, validity 11, page E3 tens/units, packet magazine 1, target magazine 2 (rel = 011) | – | – | – | 0x39CCD | 66 4C F3 |
| X/27/4 link, 2nd triplet: sub-pages S1 = 0 and 1 required | – | – | – | 0x0000C | EA 80 80 |

- Pointer resolution for the first row: packet 1, triplet 4, bits 10-18 = 30. Absolute triplet 30 → packet 3 + 30/13 = **packet 5, triplet 4** (0-based, bytes 19-21). That triplet must be the Object Definition in row 2.
- Hamming 8/4 nibble encodings, for MOT and MIP fixtures [D, agreeing with the well-known table]: 0=15 1=02 2=49 3=5E 4=64 5=73 6=38 7=2F 8=D0 9=C7 A=8C B=9B C=A1 D=B6 E=FD F=EA.

### 13.3 MOT positions [D]
x23 → X/2 bytes 12-13; x4F → X/10 bytes 28-29; xB7 → X/6 bytes 40-41; xFE → X/14 bytes 14-15 (other checks in section 7.2).

### 13.4 MOT default object decode [D]
- Link bytes N+5 = 0x9 (binary 1001): 1st default object type = 01 (Active); 2nd = 10 (Adaptive).
- N+6 = 0x2, N+7 = 0x3: object number 0x32. S1 = 2; N7 = 0 → packet 1; N6N5 = 01 → Active triplet 4; N4 = 1 → bits 10-18. That is object C3 of sub-page 2.
- Origin is 0,0 (section 8).

### 13.5 Interleave and open-area figures [V]
- A.7.2 (p.123): columns 23-27 = A A B A B is an interleave.
- Figure A.1 (p.124): an object invoked at I and ending at T has an open area running in scan order from I to T across the full 72-column width. It does not include the left panel on T's row beyond T. With an Origin Modifier (O,I) the area starts at the modified origin.

### 13.6 Contention examples [V]
C.2, p.129 (section 2.7): three cases with expected colours red, blue, yellow.

---------------------------------------------------------------------------------------------------

## 14. Ambiguities, contradictions and gaps (owner decisions)

Each item states what the text says and the options. "Spec silent" means no clause addresses it.

1. **MOT link magazine field: absolute or relative?** Tables 21 and 23 (pp.54-55) say "Magazine", bits 1-3, without "relative". X/27/0 and X/27/4 use relative magazines. Options: absolute 3-bit code, with 0 = magazine 8 (cl. 3.1); or relative (XOR with the MOT's magazine). [U] Absolute is the natural reading. Test against an oracle.
2. **MOT X/27/4,5 flag = 0 versus X/27/4 precedence.** Table 21 says flag 0 means the link holds for all sub-versions. Clause 10.5.1.4, 10.5.2.2 and E.4 say X/27/4 data takes precedence over the MOT, without condition. Options: X/27/4 always wins; or obey the flag.
3. **X/27/4 links 0-3: fixed roles versus link function bits.** Table 16 fixes their roles for Level 2.5, but the function bits are still sent. Spec silent on a mismatch: trust position or bits? "Duplicate settings are invalid": the decoder response is not stated. At Level 3.5 it is unclear whether a GPOP link with validity "3.5 only" in link 4 duplicates link 0 when link 0 is "2.5 only".
4. **Format 1 recognition.** Must a Level 2.5 decoder check triplet 1 bit 11 = 1 and triplet 2 bit 1 = 0 before accepting a link? Spec silent. Options: require both (rejecting Format 2 or corrupt links), or ignore them.
5. **Page validity 00 (reserved)** and **all sub-page flags 0 on a valid page number**: treat as no link, or as all sub-pages? Spec silent.
6. **Pointer values 507-510**, pointers into pointer packets, and pointers to a non-definition triplet: only 0x1FF is defined as "not used". Spec silent on the rest. Options: treat as no object; or scan.
7. **Packets 1 and 2 with byte 6 B1 = 0.** They are "reserved for" the pointer table, yet byte 6 "identifies the function of the packet". Can a (G)POP with very few objects put definitions in packet 1? Spec ambiguous. Pointers are relative to packet 3, so definitions in packets 1-2 would be unreachable [D]. Option: ignore definitions in packets 1-2.
8. **Is X/28/0 required on object pages?** Clause 10.5.1.1: "Optionally". A.6 (normative): "shall" be sent when X/1-X/25 are used, which is always, since pointers live in packets 1-2. D.1 lets the decoder infer page type from the MOT or X/27/4. Decision: do not require X/28/0; identify object pages by links.
9. **Mismatch between invocation and definition.** Different type (M1M0), different N bits, or a definition whose usage level excludes the decoder's level (for example "3.5 only" reached by a 2.5 decoder). Spec silent. Options: skip the object; or render anyway.
10. **Which POP or GPOP an object inside a public object refers to.** Spec silent. Table 23 note 2 and E.6 tie DRCS references inside objects to the displayed page's links. By analogy [U]: use the displayed page's POP and GPOP links. Record a decision.
11. **Active invoking Active; recursion and cycles.** Clause 13.3 permits Active → Adaptive and Passive only. The p.80 note says a Local Object may invoke another "of the appropriate type". Spec silent on self-invocation or cycles. A.8's 15-open-object limit bounds depth. Options: reject disallowed type pairs; cap depth at 15 (or 3) and abort beyond.
12. **Out-of-range positions.** Origin + Origin Modifier + element position can exceed row 24 or column 71, or cross the page/panel boundary (forbidden for transmitters by 12.5.4 rule 4 and A.4). Column offset data 72-127 is invalid. Spec silent on decoder response. Options: clip; discard the element; discard the whole object.
13. **Origin Modifier not followed immediately by an invocation.** It applies to "the object whose invocation triplet immediately follows". Spec silent if another triplet intervenes. Options: cancel the modifier; or keep it until the next invocation.
14. **Row coordinates inside objects.** Address Display Row 0 (00111) is "restricted to local enhancement data and Active Objects". Its "columns 8 to 39 only" rule refers to screen row 0. Spec silent on whether, inside an object, it means relative row 0 or screen row 0. Same question for address 40 = row 24 (relative or absolute). [U] All coordinates in an object are relative to its origin (13.7).
15. **Adaptive Object lookahead.** Property 3 bounds attribute effects at "the rightmost cell addressed by the object on a row". This needs a pre-scan. Spec silent on whether "addressed" includes cells addressed only by attribute triplets, or only by character triplets. A.8 "open" uses "last position explicitly addressed", which suggests any triplet.
16. **Passive attributes across rows.** They persist across rows and are not reset by Active Position changes (13.5 property 2). The 13.9 note 1 says all attributes reset "at the start of a passive object". Consistent, but say explicitly that **each invocation** of the same passive object starts from defaults.
17. **Priority of local data versus Active Objects.** Clause 13.2 puts local enhancement data above Active Objects, which suggests local data wins for characters. Clause 13.3 says Active Objects act "as if" placed in the local data, and C.2 rule 3 resolves attribute ties by transmission order. A.7.1 says the latest invoked same-type object wins. Decide whether 13.2 governs characters only and C.2 governs attributes, and whether an Active Object's attributes beat a later local attribute at the same cell.
18. **Default objects suppressed by any X/26.** Clause 13.8 and E.11: any local enhancement data suppresses default objects. A page whose X/26 carries only PDC triplets (Table 1: X/26 PDC at all levels) would lose them. Options: literal (any X/26 packet); or only X/26 with display triplets.
19. **GPOP default objects and the page's global flag.** Clause 13.8 says GPOP-link default objects apply to "all pages in a magazine", but each page has a "global objects required" flag (cl. 10.6.2). Options: apply to every page in the magazine; or only when the flag is 1. Also: pages whose MOT byte pair is not sent.
20. **Default-object order.** G.5 (informative) suggests GPOP 1, GPOP 2, POP 1, POP 2. A.7.1 makes the latest invoked same-type object win, so POP 2 would be on top. Type priority (13.2) governs across types. Adopt G.5 order plus A.7.1, or another.
21. **Fallback flags when the page has no POP link.** The fallback bits are only in POP links; GPOP links have them reserved. D.2.1 uses "the page's public object link". Spec silent when the POP link is 000. [U] No side panel and no substitution.
22. **Level 3.5 with X/22 and X/24 present but X/23 missing,** and a page using POP link 4-7. Spec says X/23 "may not be required". Options: treat as no POP; or fall back to the Level 2.5 link in X/20.
23. **MOT coverage of page xFF.** Clause 10.6.1 says pairs for "X00 to XFE"; Figure 20 includes xFF. FF is a time-filling or terminator page (A.1). Decision: ignore the xFF pair.
24. **Sub-pages beyond the advertised count.** An invocation's S1 may exceed MOT N+3, or not be flagged in X/27/4. Spec silent. Options: acquire anyway; or treat as missing.
25. **Termination Marker type bits.** The intermediate versus last (G)POP sub-page and "more objects follow" distinctions have no stated decoder use. Spec silent on a wrong or missing type, or a marker with reserved bits 15-18 set. [U] Treat any mode-11111/address-63 triplet as a terminator and use the type only as a completeness hint.
26. **Object Definition triplet met while processing local enhancement data,** with no Termination Marker before it. Spec silent. [U] Treat it as the end of local data, since definitions end objects (13.1).
27. **Missing packets** before the terminator in an object page, or an object running into a packet not received. D.1.1 item 3 and D.1.3 say "response … not defined". Options: drop only the affected object; or drop the sub-page.
28. **Local invocation triplet numbers 13-15,** or a designation code not received. Spec silent. [D] They cannot address a triplet. Options: ignore the invocation.
29. **Uncorrectable Hamming 24/18 triplets** (double errors) inside objects or pointer tables. Spec silent (same gap as notes-level15.md section 13 item 2). Options: skip the triplet; abort the object; treat the pointer as unused.
30. **Display before enhancement data arrives.** Enhancement pages may take up to 20 s (B.6). Spec silent on whether to show the Level 1 page at once and redraw, or wait. [U] Show Level 1 at once and redraw when the data completes. This needs a requirement and a flicker policy.
31. **Out-of-order triplets.** A.4: a decoder "may ignore" a triplet addressing an earlier location than the previous triplet from the same source. "May" means the owner must pick: ignore or apply.
32. **Limits exceeded** (A.8: 127 invocations per type, 15 open, 15 successive opens) and overlap or interleave violations (A.7). Spec silent on decoder response. Options: stop processing further invocations; or process all, within a fixed resource bound.
33. **Two default objects writing the same cell** contrary to E.12. Spec silent. Apply A.7.1 latest-wins.
34. **Control bits on enhancement pages.** B.6 reuses C12 (fragment) and C13 (partial). C14 is reserved. MOT and MIP must have S1 = 0. Decide: never apply national-option or Level 1 display semantics to enhancement pages; behaviour for a MOT with S1 ≠ 0 (ignore, or accept).
35. **Full Screen and Full Row Colour, and Address Display Row 0, inside Adaptive or Passive Objects.** These are "restricted to" local data and Active Objects. Spec silent on the decoder response. [U] Ignore them.
36. **Objects writing row 0 columns 0-7.** E.7 says objects should avoid them. The project rule (CLAUDE.md, area SEC) says these cells belong to the receiver. Decision: discard object writes there (consistent with the existing rule for X/26).
37. **Two-pair limit of Modified G0/G2 designation at Level 2.5,** counted per page across all objects. Spec silent on a third pair. Options: ignore it; or honour it.
38. **Objects addressing side-panel columns when no side panel is enabled** (by X/28/0, M/29/0 or the MOT fallback). Clause 12.5.1: with no information, display 40 columns. Spec silent on discarding. [U] Discard writes to columns 40-71 when the panel is off.
39. **Validity and expiry of stored POP/GPOP data.** Beyond C4 erase and S2 change, there is no time-out rule. Spec silent on discarding object pages no longer linked after a MOT change (B.7 implies the MOT is updated first). Options: invalidate on MOT change; LRU within the E.8 bound.
40. **MOT "number of sub-pages" = highest S1 transmitted.** This implies sub-pages 0..n are contiguous, which is not stated. Spec silent if sparse.
41. **X/27/4 sub-page flags versus S1 in invocations.** Flags say which sub-pages "are required", but invocations name S1 directly. Spec silent if they disagree. [U] Use flags for acquisition only.
42. **Termination 111 ("no Local Object definitions follow") followed by definitions anyway.** Spec silent. Options: ignore the definitions; or accept them if invoked.
43. **Text inconsistency.** Clause 9.6.2 lists "Link function flags: 4 bits", while Table 15 gives 2 function bits + 2 validity bits (section 9.1). This is a wording issue only; follow Table 15.
44. **"Pink" in clause 13.9** has no colour definition (section 13.1). Choose a concrete CLUT entry for the test fixture.
45. **Colour Map dependence and transparency in passive objects.** Clause 13.5 says passive objects look the same everywhere "apart from its colours which may be Colour Map dependent". There is no rule for CLUT 1 entry 0 (transparent) inside objects beyond C.1. Out of scope here; the colour notes must cover it.

---------------------------------------------------------------------------------------------------

## 15. Not covered here
- DRCS page format, PTU coding, DRCS modes and DCLUTs (cl. 14, X/28/1, X/28/3): only their links and the object-scope rules are given above.
- The full Table 29 column-triplet semantics (colours, flash, display attributes, font style) beyond their object-specific scope rules. See the Level 2.5 attribute notes.
- Side-panel enable bits in X/28/0 and M/29/0 (Table 4, Table 13), apart from the MOT fallback.
