# F.9c Dynamically redefinable characters (DRC)

This area specifies Dynamically Redefinable Characters (DRCS, clause 14): the pages that carry them, the four modes, their colours and how they are drawn. Decision E.54 applies.

**REQ-DRC-001 — DRCS pages.**
A page shall be treated as a DRCS page when its X/28/0 or X/28/3 page function is 0100 (Global DRCS) or 0101 (normal DRCS), or when a GDRCS or DRCS link names it (REQ-OBJ-030 and REQ-OBJ-031). Each sub-page, identified by S1, is one sub-table of 48 Pattern Transfer Units (PTUs) (clause 14.1). Packets X/1 to X/24 each carry two PTUs: PTU 2k is bytes 6 to 25 of packet X/(k + 1), and PTU 2k + 1 is bytes 26 to 45 (Figure 27). Packet X/25 is not used.
*Verification:* Test with the layout of Figure 27 (REQ-TST-016).

**REQ-DRC-002 — D-bytes.**
Each PTU byte shall be checked for odd parity. A valid byte, called a D-byte, has a code from 4/0 to 7/F, and its six least significant bits are six dots, the most significant of them the leftmost (clause 14.2.1). D-byte i of a PTU, numbered 0 to 19, covers row i / 2 of the 12 by 10 matrix, and columns 0 to 5 when i is even or 6 to 11 when i is odd. A byte that fails its parity check, or whose code lies outside 4/0 to 7/F, shall make every character using that PTU invalid (decision E.54).
*Verification:* Test with the worked PTUs of `research/notes-level25-drcs.md` section 3.5.

**REQ-DRC-003 — Mode identification.**
The mode of each PTU shall be taken from the page's X/28/3 (REQ-PKT-014), in the order decision E.54 gives. Without X/28/3 every PTU is mode 0 (Annex D.1.7). The first PTU of a character carries its mode. Each further PTU of a mode 1 or mode 2 character shall be marked 1110, and a PTU marked 1111 has no data (Table 9). A character shall be invalid when its first PTU is marked 1110, 1111 or a reserved value, when a further PTU is not marked 1110, or when it would run past PTU 47. At Level 2.5, a PTU not marked mode 0 shall be unusable.
*Verification:* Test with the worked encoding of `research/notes-level25-drcs.md` section 4, and with each invalid case.

**REQ-DRC-004 — Pixel values of each mode.**
The pixels of a DRCS character shall be decoded according to its mode (Table 31; clauses 14.2.1 to 14.2.4):

| Mode | Format | PTUs | Pixel value |
|---|---|---|---|
| 0 | 12 by 10 by 1 | 1 | One bit: 1 for foreground, 0 for background |
| 1 | 12 by 10 by 2 | 2 | Plane 0 from the first PTU plus 2 × plane 1 from the second, indexing a four-entry DRCS colour table |
| 2 | 12 by 10 by 4 | 4 | Planes 0 to 3 from the four PTUs, the first the least significant, indexing a sixteen-entry DRCS colour table |
| 3 | 6 by 5 by 4 | 1 | D-byte 4r + k is plane k of row r, with six pixels each, the most significant bit leftmost; the four planes index a sixteen-entry DRCS colour table |

The global or normal colour table is used according to whether the character comes from the global or the normal table (REQ-DRC-007).
*Verification:* Test for each mode with characters built by the encoder.

**REQ-DRC-005 — The DRCS Mode triplet.**
Row mode 11000 shall choose the sub-table, T3 to T0, of the normal table when D6 is 1 and of the global table when D6 is 0, for later DRCS characters. It shall not move the active position. When its level bits L1 and L0 exclude the presentation level, the triplet shall be ignored; the reserved value 00 shall count as both levels (decision E.54). The chosen sub-table shall persist to the end of the local enhancement data or of the object in which the triplet occurs. It shall not pass into an invoked object, nor back from one (Table 28 note; Annex C.4 rule 3). At the start of the local enhancement data and of each object, the normal and global sub-tables shall both be 0.
*Verification:* Test with the worked DRCS Mode triplets of `research/notes-level25-drcs.md` section 5.1, and with objects that do and do not choose their own sub-tables.

**REQ-DRC-006 — The DRCS Character Invocation triplet.**
Column mode 01101 shall set the column and write a DRCS character: from the normal table when D6 is 1 and from the global table when it is 0, in the sub-table currently chosen, with the character number in D5 to D0. A number from 48 to 63 shall make the character invalid. The cell shall record the table, the sub-table and the character (REQ-L25-006). The DRCS page shall be the one the displayed page's GDRCS or DRCS link names (REQ-OBJ-030), also for characters inside objects (Annex E.6; Table 23 note 2).
*Verification:* Test with the worked invocations of `research/notes-level25-drcs.md` section 5.2.

**REQ-DRC-007 — DRCS colour lookup tables.**
At Level 3.5, the four DRCS colour lookup tables shall be taken from the page's X/28/1, or, when it is absent or invalid, from the magazine's M/29/1, or otherwise from the defaults of Annex D.2.2: entries 0 to 3 of the Colour Map in both four-entry tables, and entries 0 to 15 in both sixteen-entry tables (decision E.54). Each entry is a Colour Map index (Figure 28). The tables, and the patterns of every DRCS character that the page's cells use, shall travel with the grid (REQ-L25-006), so that the renderer needs nothing else. The capacity for those patterns shall be set by the caller (REQ-GEN-003); a character beyond it shall be treated as missing, and counted.
*Verification:* Test with the Figure 28 example values (REQ-TST-016), the defaults and each precedence case.

**REQ-DRC-008 — Colours and attributes of DRCS characters.**
A mode 0 character shall take the foreground and background colours at its cell, and every attribute shall act on it as on any other character (clause 14.2.1). For modes 1 to 3, each pixel's colour shall be its colour table entry, and an entry of CLUT 1 entry 0 shall be transparent (Annex C.1). For those modes (decision E.54):
- flash shall follow decision E.48;
- invert shall have no effect;
- conceal shall show a space in the background colour;
- boxing shall follow the cell.

Underline shall not be drawn on any DRCS character, and font styles shall not apply to them. Enlarged characters shall be drawn by exact pixel replication (REQ-RND-038).
*Verification:* Test for each attribute and each mode.

**REQ-DRC-009 — Missing, unusable and invalid DRCS data.**
A DRCS sub-page shall be usable once every PTU that a referenced character needs has been received; later partial receptions replace the PTUs they carry (decision E.54). A cell whose DRCS data is missing or not yet usable shall show a space in its background colour. A cell whose DRCS data is invalid shall show a space, and a diagnostic shall be traced (decision E.50).
*Verification:* Test with missing packets, partial receptions and each invalid case of REQ-DRC-002 and REQ-DRC-003.

**REQ-DRC-010 — Limits on DRCS use.**
The Level 2.5 limits of two sub-tables and 24 characters per page, and the Level 3.5 limit of 384 PTUs, shall not be enforced; they bind the broadcaster (decision E.54). The only limits shall be the capacity of the enhancement page store (REQ-ACQ-019) and the pattern capacity of REQ-DRC-007.
*Verification:* Test with a page that uses more than the limits of each level.
