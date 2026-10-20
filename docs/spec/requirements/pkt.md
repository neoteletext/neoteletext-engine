# F.4 Packet decoding (PKT)

**REQ-PKT-001 — Packet address.**
The decoder shall Hamming-decode bytes 4 and 5 of each packet and compute:
- the magazine as the low three bits of the first value;
- the packet number as the top bit of the first value plus the second value shifted left by one.

Magazine value 0 shall be reported as magazine 8. If either byte is uncorrectable, the packet shall be discarded and counted (decision E.7).
*Verification:* Test with all 256 combinations of valid address values, and with injected errors.

**REQ-PKT-002 — Page header.**
For packet number 0, the decoder shall decode bytes 6 to 13 as Hamming 8/4 and extract:
- the page units and tens digits;
- the four sub-code digits (S1 with 4 bits, S2 with 3, S3 with 4, S4 with 2);
- control bits C4 to C14 at the exact bit positions of clause 9.3.1;
- the 32 display bytes 14 to 45 with their parity status.

A validity flag shall be kept separately for the page number, the sub-code and the control bits, so that decision E.7 can be applied.
*Verification:* Test against headers built by the encoder, and against captured headers.

**REQ-PKT-003 — Display rows.**
For packet numbers 1 to 25, the decoder shall extract the 40 bytes 6 to 45 as 7-bit characters, each with its parity status.
*Verification:* Test.

**REQ-PKT-004 — Enhancement packets.**
For packet number 26, the decoder shall:
- Hamming 8/4 decode byte 6 as the designation code;
- Hamming 24/18 decode the thirteen triplets in bytes 7 to 45;
- split each valid triplet into its 6-bit address, 5-bit mode and 7-bit data fields (clause 12.3.1);
- mark each triplet valid or invalid.
*Verification:* Test.

**REQ-PKT-005 — Editorial link packets.**
For packet number 27 with designation code 0, the decoder shall extract the six links. For each link it shall:
- decode the page number and sub-code;
- compute the linked magazine by exclusive-OR of the packet's own magazine with the relative magazine bits M1, M2 and M3 (clause 9.6.1);
- mark the link as null when the page number is FF and the sub-code is 3F7F.

It shall also extract:
- the Link Control Byte, including the "display row 24" flag in data bit 4 of byte 43;
- the two checksum bytes.

Packets 27 with designation codes 1 to 3 shall be stored without interpretation. Designation codes 4 and 5 are the compositional links of Levels 2.5 and 3.5, decoded as REQ-PKT-011 describes. Designation codes 6 to 15 shall be ignored.
*Amended 10 October 2026:* designation codes 4 and 5 were previously ignored with the rest (decision E.40).
*Verification:* Test.

**REQ-PKT-006 — Page enhancement packet X/28.**
For packet number 28 with designation code 0 in format 1 (as identified by clause 9.4.2), the decoder shall Hamming 24/18 decode triplets 1 and 2 and extract:
- the page function (bits 1 to 4 of triplet 1);
- the page coding (bits 5 to 7 of triplet 1);
- the 7-bit default G0 and G2 designation (bits 8 to 14 of triplet 1);
- the 7-bit second G0 designation (bits 15 to 18 of triplet 1 and bits 1 to 3 of triplet 2), laid out as Tables 32 and 33 define them.

The remaining fields of X/28/0 format 1 are decoded as REQ-PKT-012 describes. Packets X/28/1, X/28/3 and X/28/4 are decoded as REQ-PKT-013 to REQ-PKT-015 describe; the first triplet of X/28/1 is used for diagnostics only (decision E.4). Other designation codes shall be ignored.
*Amended 10 October 2026:* X/28/1 was previously stored without interpretation (decision E.40).
*Verification:* Test.

**REQ-PKT-007 — Magazine enhancement packet M/29.**
For packet number 29 with designation code 0 and packet function 0000000, the decoder shall extract the default G0 and G2 designation and the second G0 designation in the same way as for X/28/0. It shall store them for the magazine. The remaining fields, and packets M/29/1 and M/29/4, are decoded as REQ-PKT-016 describes. Other designation codes shall be ignored.
*Amended 10 October 2026:* M/29/1 was previously stored without interpretation (decision E.40).
*Verification:* Test.

**REQ-PKT-008 — Packets that do not belong to pages.**
Packets numbered 30 in magazines other than 8, and all packets numbered 31, shall be counted and ignored. Packets numbered 29 and 30 shall never start, end or attach to a page (clauses 7.3, 7.4 and 9.7).
*Verification:* Test that inserts these packets in the middle of a page and checks that the page is unaffected.

**REQ-PKT-009 — Reserved bits.**
Bits and bytes that the standard marks as reserved shall be ignored (clause 9.2).
*Verification:* Inspection.

**REQ-PKT-010 — Triplet functions at Levels 2.5 and 3.5.**
For every valid triplet of a packet X/26, and of packets X/1 to X/25 of an object page, the decoder shall provide the triplet's function as a typed value. The value shall cover every row mode of Table 28 and every column mode of Table 29, with the fields those tables define. In particular:
- for Object Invocation (row modes 10001 to 10011): the source (local, POP or GPOP, with S1 S0 = 00 marked illegal), the type, and the object number N8 to N0, split as Table 28 defines it for local and for public objects;
- for Object Definition (row modes 10101 to 10111): the usage level bits, the type and the object number;
- for the Origin Modifier: the row offset (the address minus 40) and the column offset (the data);
- for the Termination Marker: the type in data bits 12 to 14;
- for every colour field: the 5-bit Colour Map index, with data bits D6 and D5 kept so that reserved values can be recognised.

Reserved modes shall be reported as reserved, and so shall DRCS Mode (row mode 11000). DRCS Character Invocation (column mode 01101) shall be reported as a column triplet not acted on, with its column, mode and data. The Level 1.5 pass, the enhancement processor and the object resolver shall all use this one decoding.
*Amended 20 October 2026 (decision E.95):* DRCS is not decoded, and the parts of this requirement that concerned it are removed.
*Verification:* Test with one triplet of every mode, built by the encoder, including the worked encodings of `research/notes-level25-objects.md` section 13.2 and `research/notes-level25-drcs.md` section 11.

**REQ-PKT-011 — Compositional link packets X/27/4 and X/27/5.**
For packet number 27 with designation code 4 or 5, the decoder shall Hamming 24/18 decode triplets 1 to 12, and extract the six links of two triplets each that Table 15 defines. For each link it shall extract:
- the link function and the page validity, two bits each;
- the page units and tens;
- the magazine, computed by exclusive-OR of the packet's own magazine with the relative magazine bits, as for X/27/0;
- the sixteen sub-page flags;
- the two Format 1 marker bits: bit 11 of the first triplet and bit 1 of the second.

A link with an uncorrectable triplet shall be marked invalid. The roles of the links are applied by REQ-OBJ-030 (decision E.52).
*Verification:* Test with links built by the encoder, including the worked link of `research/notes-level25-objects.md` section 13.2, and with injected errors.

**REQ-PKT-012 — Packet X/28/0 format 1 in full.**
For packet X/28/0 format 1 (Table 4), the decoder shall extract, in addition to the fields of REQ-PKT-006:
- the left and right side-panel enable bits, the Side Panel Status Flag and the number of left-panel columns (triplet 2, bits 4 to 10);
- the sixteen 12-bit colour words for CLUTs 2 and 3, from the 192-bit stream formed by triplet 2 bits 11 to 18, triplets 3 to 12, and triplet 13 bits 1 to 4. Each word is red, green and blue, four bits each, with each group least significant bit first;
- the default screen colour (triplet 13, bits 5 to 9) and the default row colour (bits 10 to 14);
- Black Background Colour Substitution (triplet 13, bit 15);
- Colour Table Re-mapping (triplet 13, bits 16 to 18).

Each field group of decision E.44 shall carry its own validity flag.
*Verification:* Test with packets built by the encoder covering every class of field value, and with an error injected into each field group in turn.

**REQ-PKT-013 — Withdrawn: Packet X/28/1: DRCS colour lookup tables.** Withdrawn on 20 October 2026: DRCS is not decoded, and X/28/1 is ignored (decision E.95).

**REQ-PKT-014 — Packet X/28/3: the page function.**
For packet X/28/3 (Table 9), the decoder shall extract the page function and page coding of triplet 1, so that a DRCS page that declares itself only there is never shown as text. The mode identification codes of its other triplets shall not be read.
*Amended 20 October 2026 (decision E.95):* the 48 mode identification codes are no longer extracted.
*Verification:* Test with a packet built by the encoder, and with triplet 1 uncorrectable.

**REQ-PKT-015 — Packet X/28/4.**
For packet X/28/4 (Table 10), the decoder shall extract the same fields as REQ-PKT-012, except that the sixteen colour words are CLUT 0 entries 0 to 7 followed by CLUT 1 entries 0 to 7. The word for CLUT 1 entry 0 shall be ignored.
*Verification:* Test.

**REQ-PKT-016 — Magazine packets M/29/0 and M/29/4.**
For packet number 29, the decoder shall extract:
- with designation code 0 and packet function 0000000, every field of REQ-PKT-012 except the page function and page coding (Table 11);
- with designation code 4 and packet function 0000000, the fields of REQ-PKT-015 (Table 13).

A packet with any other packet function, and M/29/1 (the DRCS colour lookup tables of Table 12), shall be ignored.
*Amended 20 October 2026 (decision E.95):* DRCS is not decoded, and the parts of this requirement that concerned it are removed.
*Verification:* Test.

**REQ-PKT-017 — Packets of object pages.**
For a stored packet X/1 to X/25 of an object page, the decoder shall Hamming 8/4 decode byte 6 and Hamming 24/18 decode the thirteen triplets. For packets 1 to 4, bit 1 of byte 6 shall say whether the packet carries pointer data (clause 10.5.1.2). From a pointer packet, the decoder shall extract the 24 nine-bit pointers of the triplets numbered 1 to 12, counting from 0 as clause 10.5.1.2 does: bits 1 to 9 hold the pointer to the even-numbered object, and bits 10 to 18 the pointer to the odd-numbered one. Triplet 0 is reserved. An uncorrectable pointer triplet shall make both of its pointers unused (decision E.51).

The decoder shall also convert an absolute triplet number from 0 to 506 into a packet and a triplet: number t below 299 is triplet t mod 13 of packet 3 + t / 13, and any higher number is triplet (t − 299) mod 13 of packet X/26 with designation code (t − 299) / 13.
*Verification:* Test with pointer tables built by the encoder, including the worked pointer of `research/notes-level25-objects.md` section 13.2, and with the extreme triplet numbers 0, 298, 299 and 506.

**REQ-PKT-018 — Packets of the Magazine Organization Table.**
For packets of page xFE, every byte of which is Hamming 8/4 coded, the decoder shall extract:
- from packets X/1 to X/14, the byte pair of any page number, at the position Figure 20 gives;
- from packets X/19 and X/20 (Level 2.5) and X/22 and X/23 (Level 3.5), the GPOP link and POP links 1 to 7 of Tables 20 and 21, including the fallback bits and the two default objects of each link;
- from packets X/21 (Level 2.5) and X/24 (Level 3.5), the totals of object pages (clause 10.6.6). Their DRCS links, the DRCS total, and the second byte of each byte pair, which names a DRCS link, shall not be read.

An uncorrectable byte shall make the link or byte pair it belongs to invalid (decision E.7); for a byte pair, that is its first byte.
*Amended 20 October 2026 (decision E.95):* DRCS is not decoded, and the parts of this requirement that concerned it are removed.
*Verification:* Test with the positions of `research/notes-level25-objects.md` sections 7.2 and 13.3, and the default object of section 13.4.
