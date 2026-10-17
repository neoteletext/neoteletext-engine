# F.9b Objects (OBJ)

This area specifies objects (clause 13), the pages that carry them (clause 10.5), and the links that lead to those pages: the Magazine Organization Table (clause 10.6) and packets X/27/4 and X/27/5 (clause 9.6.2). Decisions E.51 to E.53 apply. Objects are processed by the walk of REQ-FSM-011.

**REQ-OBJ-001 — Object invocation.**
An Object Invocation triplet (row modes 10001 Active, 10010 Adaptive and 10011 Passive) shall invoke the object it names, from the source its bits S1 and S0 give: 01 local, 10 the page's Public Object Page (POP), or 11 the page's Global Public Object Page (GPOP). Source 00 is illegal, and the triplet shall be skipped. The invocation shall run the object's triplets once, from the origin of REQ-OBJ-002, subject to the nesting rules of REQ-OBJ-008 and the limits of REQ-OBJ-020 (clauses 13.6 and 13.7).
*Verification:* Test.

**REQ-OBJ-002 — The origin and the Origin Modifier.**
An object's origin shall be the active position at its invocation plus the offsets of an Origin Modifier (row mode 10000) that immediately precedes the invocation. The row offset is the modifier's address minus 40, and the column offset is its data, 0 to 71. Inside the object the active position shall start at row 0, column 0, relative to the origin. Every coordinate in the object shall be relative to the origin, including Address Display Row 0 and row address 40 (decision E.51). When the object ends, the active position shall return to its value at the invocation, and the Origin Modifier shall be cancelled. An Origin Modifier followed by any triplet other than an invocation shall be discarded (decision E.46). A column offset above 71 places every element beyond column 71, where REQ-OBJ-014 discards it.
*Verification:* Test, including the worked Origin Modifier of `research/notes-level25-objects.md` section 13.2.

**REQ-OBJ-003 — Finding a local object.**
For a local invocation, bits N7 to N4 of the object number shall give the designation code of the packet X/26 that holds the definition, and bits N3 to N0 the triplet within it, numbered 0 to 12 (Table 28). Bit N8 shall be 0. An invocation shall be skipped when N8 is 1, when the triplet number is 13 to 15, or when the packet with that designation code was not received (decision E.51). The triplet found shall then be checked as REQ-OBJ-005 describes.
*Verification:* Test, including each case that is skipped.

**REQ-OBJ-004 — Finding a public or global object.**
For a POP or GPOP invocation, the definition shall be found as clause 10.5.1.2 and Table 28 describe:
1. the object page is the POP or GPOP that the page's links name (REQ-OBJ-030);
2. the sub-page is the one whose S1 equals N3 to N0 (REQ-OBJ-032);
3. the pointer packet is packet 1 + (N8 N7), and its byte 6 shall say that it carries pointer data;
4. the pointer triplet, numbered from 0, is 1 + 3 × (N6 N5) + k, where k is 0 for Active, 1 for Adaptive and 2 for Passive objects;
5. the pointer is bits 1 to 9 of that triplet when N4 is 0, and bits 10 to 18 when N4 is 1;
6. the pointer value is an absolute triplet number, converted to a packet and triplet as REQ-PKT-017 describes.

The object shall be absent when the pointer is 0x1FF (not used), when its value is 507 to 510, when it points into a pointer packet or into packets 1 or 2, or when the pointer packet is missing or does not carry pointer data (decision E.51).
*Verification:* Test with the worked pointer of `research/notes-level25-objects.md` section 13.2, and with each case that makes the object absent.

**REQ-OBJ-005 — Checking the definition.**
The triplet found shall be an Object Definition (row modes 10101 to 10111) of the same type as the invocation, whose usage bits include the presentation level: 01 for Level 2.5, 10 for Level 3.5, and 11 for both. Otherwise the invocation shall be skipped. The object number bits of the definition shall not be compared with those of the invocation (decision E.51).
*Verification:* Test with a definition of each other type, and with each usage value at both levels.

**REQ-OBJ-006 — The body of an object.**
An object's body shall be the triplets that follow its definition, up to the next Object Definition or Termination Marker triplet (clause 13.1). Triplet 12 of a packet is followed by triplet 0 of the next packet, and packet 25 is followed by packet X/26 with designation code 0. A body shall be at most 507 triplets, and shall never continue onto another sub-page (clause 10.5.1.1).

Before an object runs, its body shall be scanned to find where it ends and which cells it addresses on each row (REQ-OBJ-010 and REQ-OBJ-020). When the scan meets a packet that has not been received before the end of the body, the object shall be absent; other objects on the same page are unaffected (decision E.51). Uncorrectable triplets in the body shall be skipped.
*Verification:* Test with bodies that cross packets, that reach packet X/26, and that meet a missing packet.

**REQ-OBJ-008 — Which objects may invoke which.**
Invocations shall be permitted only as this table shows (clauses 13.3 to 13.6; decision E.51). Any other invocation shall be skipped.

| Invoker | May invoke |
|---|---|
| Local enhancement data | Active, Adaptive and Passive objects, from local, POP or GPOP sources |
| Active object | Adaptive and Passive objects |
| Adaptive object | Passive objects |
| Passive object | Nothing |
| The default-object mechanism (REQ-OBJ-040) | POP and GPOP objects of any type, never local ones |

A local object shall be invoked only by the page's own local enhancement data, or by another local object, and never by a public or global object (clause 13.6). An object invoked from inside a public or global object shall be found through the displayed page's POP and GPOP links.
*Verification:* Test for every pair of invoker and type, and for each source.

**REQ-OBJ-009 — Active objects.**
An Active object's triplets shall act as if they were local enhancement data at the active position (clause 13.3). The attributes at its starting position shall be those of the Level 1 page, as already modified there by local enhancement data and earlier Active objects. The attributes it sets shall act to the end of the row, on every character, whether the character comes from the object or from the Level 1 page, until a later attribute of the same type supersedes them.
*Verification:* Test with the Active case of the clause 13.9 example (REQ-TST-016).

**REQ-OBJ-010 — Adaptive objects.**
An Adaptive object shall start with the underlying attributes at its position: those of the Level 1 page after local enhancement data and Active objects (clause 13.4). On each row, the attribute changes it makes shall affect both its own characters and the Level 1 characters between the first and the rightmost cell that any of its triplets addresses on that row (decision E.51). For an attribute type that the object has changed, later changes made by the Level 1 page inside that range shall be ignored. To the right of the rightmost addressed cell, the underlying attributes shall apply again. A character set designation made by the object shall last to the last character cell of the object.
*Verification:* Test with the Adaptive case of the clause 13.9 example (REQ-TST-016).

**REQ-OBJ-011 — Passive objects.**
Each invocation of a Passive object shall start from these attributes (clause 13.5): white foreground (CLUT 0 entry 7), black background (CLUT 0 entry 0), normal size, steady, boxing off, not concealed and contiguous mosaics. Its attributes shall persist across changes of row and of active position until the object changes them or ends. They shall apply only to the cells where the object writes a character, and the underlying attributes shall not affect those cells. Cells that the object does not write shall show the underlying page unchanged. Transparency inside a Passive object is CLUT 1 entry 0, as everywhere else (decision E.51).
*Verification:* Test with the Passive case of the clause 13.9 example (REQ-TST-016), and with two invocations of one object.

**REQ-OBJ-012 — Display priority.**
Where several sources write a character to one cell, the character shown shall be chosen by the priority of clause 13.2: Passive objects, then Adaptive objects, then the local enhancement data, then Active objects, then the Level 1 page. Between two objects of the same type, the latest invoked shall win (Annex A.7.1); this includes default objects (decision E.53). Ties between attributes shall be resolved by Annex C.2, with the order in which the walk of REQ-FSM-011 meets them as the order of transmission. The displayed attributes of a cell shall be those of the plane whose character it shows, as REQ-FSM-012 describes. Transmissions that break the overlap and interleave rules of Annex A.7 shall be displayed by these same rules.
*Verification:* Test with overlapping objects of each pair of types.

**REQ-OBJ-013 — Triplets restricted to local data and Active objects.**
Full Screen Colour, Full Row Colour and Address Display Row 0 triplets inside Adaptive or Passive objects shall be ignored entirely (decision E.51). Inside an object whose origin lies in a side panel, Full Screen Colour and Full Row Colour shall be ignored (clause 12.5.3).
*Verification:* Test.

**REQ-OBJ-014 — Where objects may write.**
An element of an object, meaning a character or an attribute, that would be placed outside rows 0 to 24 or columns 0 to 71 shall be discarded, and the object shall continue (decision E.51). Elements placed in row 0, columns 0 to 7 (REQ-L25-050), and elements that decision E.49 discards in or across the side panels, shall be discarded in the same way.
*Verification:* Test with objects placed at every edge.

**REQ-OBJ-020 — Limits on objects.**
The limits of Annex A.8 shall be enforced as fixed bounds (decision E.51):
- at most 127 invocations of each object type per page, side panels and default objects included;
- at most 15 objects open at once, where an object is open from the active position at its invocation up to and including the last position it addresses, in scan order (Annex A.8, Figure A.1);
- at most 15 objects opened in succession without a displayable character or attribute being written in between.

Invocations beyond a limit shall be ignored and counted. The work done to build a page's grid shall be bounded by these limits (REQ-SEC-012), and enhancement processing shall use no allocation.
*Verification:* Test with pages that reach and exceed each limit; the bounded-work test of REQ-TST-019.

**REQ-OBJ-030 — The enhancement pages a page uses.**
The GPOP, POP, Global DRCS (GDRCS) and DRCS pages that a page uses, with their sub-pages, shall be taken from the page's own X/27/4, and at Level 3.5 also X/27/5, when those are present, and from the Magazine Organization Table otherwise (Annex D.1.4). The links of X/27/4 and X/27/5 shall take precedence whatever the table's X/27/4,5 flag says, and shall be read as decision E.52 describes:
- X/27/4 links 0 to 3 are, by position, the GPOP, POP, GDRCS and DRCS links. A link whose function bits do not match its position shall be ignored.
- At Level 3.5, X/27/4 links 4 and 5 and X/27/5 links 0 and 1 take their role from their function bits.
- A link shall be used only when its Format 1 marker bits are as Table 15 requires, and its page validity includes the presentation level.
- Page validity 00, all sixteen sub-page flags at 0, or page number FF shall mean no link.
- A link that duplicates one already accepted shall be ignored.
- The sub-page flags shall be used only as hints for acquisition.
*Verification:* Test with the worked link of `research/notes-level25-objects.md` section 13.2, and with each case that makes a link unusable.

**REQ-OBJ-031 — The Magazine Organization Table.**
For a page P of magazine m, the decoder shall use the stored Magazine Organization Table of m, page mFE, sub-page S1 = 0. A table on another sub-page shall be ignored (decision E.52). From P's byte pair (REQ-PKT-018) it shall take the global object flag, the POP link number, the global DRCS flag and the DRCS link number. The links shall be read:
- at Level 2.5, from X/19 to X/21;
- at Level 3.5, from X/22 to X/24. When X/22 or X/24 is missing, the Level 2.5 links shall be used, and the page shall be processed at Level 2.5 (clause 10.6.1). When only X/23 is missing, POP links 4 to 7 shall be taken from X/20.

The magazine of a link is absolute, with 0 meaning magazine 8. A link to page FF means "not used". The byte pair for page xFF shall be ignored. The global flags shall be used only as hints for acquisition, and a GPOP or GDRCS reference shall be resolved through the magazine's link whatever the flag says. The fallback bits of P's POP link shall supply side panels and Black Background Colour Substitution when REQ-L25-031 reaches them; with no POP link, they shall be clear (decision E.53).
*Verification:* Test with the table positions of `research/notes-level25-objects.md` sections 7.2 and 13.3, at both levels and with each Level 3.5 packet missing.

**REQ-OBJ-032 — Sub-pages of enhancement pages.**
An invocation shall name the sub-page of its object page by S1 directly. A link's sub-page count n shall mean that sub-pages 0 to n are assumed; missing ones are absent. A sub-page beyond the advertised count shall be used if it is stored (decision E.52).
*Verification:* Test.

**REQ-OBJ-040 — Default objects.**
When a page has no packet X/26 that carries a display triplet, the decoder shall place the default objects that the Magazine Organization Table gives (clause 13.8; decision E.53):
- up to two from the GPOP link of the page's magazine, which apply to every such page of the magazine whatever the page's global object flag says;
- up to two from the page's POP link.

They shall be placed in the order GPOP 1, GPOP 2, POP 1, POP 2, each with its origin at row 0, column 0, and the active position shall be row 0, column 0 before and after each. Each default object number shall be resolved as REQ-OBJ-004 describes, with N8 = 0, so that only the first 16 objects of each type on each sub-page can be reached. A page whose X/26 packets carry only Programme Delivery Control triplets shall keep its default objects.
*Verification:* Test with the worked default object of `research/notes-level25-objects.md` section 13.4, and with pages that have and lack local display data.

**REQ-OBJ-041 — Enhancement pages are never shown as text.**
Object pages, DRCS pages and the Magazine Organization Table shall never receive Level 1 national option or display semantics, and shall not be rendered as text pages (REQ-ACQ-014). On these pages, bit C12 means a continued fragment and bit C13 a partial page (Annex B.6, Table B.1).
*Verification:* Test.
