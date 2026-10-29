# F.24 Page and sub-page numbering (NUM)

Every rule World System Teletext gives for numbering magazines, pages and sub-pages, as requirements of NeoTeletext: the engine, which decodes and encodes teletext, and through it the browser and the server, which take these rules from the engine and write none of their own (the browser's requirement 2.1, the server's decision 10). Decided on 28 October 2026 on the owner's instructions: "WE MUST accept every address the standard defines", "WE COMPLY ONLY TO WORLD SYSTEM TELETEXT PAGE/SUBPAGE NUMBERING" and "ENSURE ALL THE CLAUSES, RULES AND DETAILS OF THE TELETEXT SPECIFICATION BECOME REQUIREMENTS" (decision E.130).

The sources, checked against the documents in `~/devel/teletext-reference/teletext-standards` on 28 October 2026:
- **EN 300 706 V1.2.1 (2003-04),** Enhanced Teletext: clauses 7.1.2, 7.2, 7.3, 9.3.1, 9.4.2, 9.6, 9.8.1, 9.8.2, 10.5, 10.6, 11.2, 11.3, Annex A.1, A.10, B.2 to B.6, E.1, E.2 and E.15.
- **TR 100 287 V1.2.1 (2002-12),** the code of practice for enhanced Teletext: clauses 10.1, 10.5, 10.6, 10.7 and 10.12.
- **EN 300 708 V1.2.1 (2003-04),** data transmission within Teletext: clauses 4.3.1.1, 4.3.1.2 and 4.4.1.

A requirement here states the rule; the requirements that decode or display it are cited, not repeated. "Shall" in the standard or the code of practice stays "shall" here; their "should" stays "should", and a part that departs from one records why as a decision (`neoteletext-common/DEVELOPMENT.md` section 5). The terms are the standard's: a page address is a magazine, a page number and a sub-code; a display page is a page intended for display (a basic Level 1 page, page function 0000); an enhancement data page is a POP, GPOP, DRCS, GDRCS, MOT or MIP page; a data broadcasting page is one of EN 300 708.

**Where the parts stand (29 October 2026).** The engine's tests trace each requirement here (`cargo xtask traceability`). The browser takes the area at its pin of the engine (its decisions 119 and 121). The server does not yet: it holds at most 64 sub-pages a page and 800 pages a channel, and sends a page's sub-pages whole to every viewer, so REQ-NUM-002, REQ-NUM-020 and REQ-NUM-030 are not met there, nor by the TypeScript channel kit (`neoteletext-common/STANDARD-LIMITS.md` sections 7 and 7.2).

## Scope

**REQ-NUM-001 — World System Teletext numbering only.**
NeoTeletext shall number magazines, pages and sub-pages only as World System Teletext does: ITU-R BT.653 System B, specified by EN 300 706. The page addressing of the other teletext systems (ITU-R BT.653 Systems A, C and D, and the Japanese systems) shall not be supported.
*Reason:* The owner's instruction; every source the engine reads (T42, TTI, transport streams, sliced records, web channels) carries System B.
*Verification:* Inspection.

**REQ-NUM-002 — Every address the standard defines is accepted.**
Every part of NeoTeletext (the engine, the server and the browser, and every channel kit) shall accept every page address the standard defines: magazines 1 to 8, page numbers 00 to FF, and sub-codes 0000 to 3F7F, each used as the rules of this area say. No part shall refuse, drop or rewrite a page or a sub-page because of its page number or its sub-code, nor because of how many sub-pages its page has. Every sub-page is held and reachable (REQ-NUM-030); what a part keeps in memory or sends at once is a budget, never a limit on which addresses may exist.
*Reason:* The owner's instruction "WE MUST accept every address the standard defines".
*Verification:* Test, in each part: a page with every page number 00 to FE in each magazine, and a page whose sub-pages carry the first, the last and a sample of every other sub-code 0001 to 3F7E, each stored, served, listed and reachable.

## Magazines, page numbers and sub-codes

**REQ-NUM-003 — Magazines.**
The magazine is the three bits of packet address byte 4 (clause 7.1.2): values 1 to 7 are magazines 1 to 7, and value 0 is magazine 8. A service has at most 8 magazines (clause 7.4). Decoding is REQ-PKT-001.
*Reason:* Clause 7.1.2.
*Verification:* Test (REQ-PKT-001's).

**REQ-NUM-004 — Page numbers.**
A page number is two hexadecimal digits, the page units in header byte 6 and the page tens in byte 7, each Hamming 8/4 coded and each 0 to F (clause 9.3.1.1): page numbers 00 to FF, 256 in each magazine, 2,048 in a service. Decoding is REQ-PKT-002.
*Reason:* Clause 9.3.1.1.
*Verification:* Test of every value of each digit.

**REQ-NUM-005 — Sub-codes.**
A sub-code is thirteen bits in four elements (clause 9.3.1.2): S1 of 4 bits (byte 8), S2 of 3 bits (byte 9), S3 of 4 bits (byte 10) and S4 of 2 bits (byte 11), each Hamming 8/4 coded, written S4 S3 S2 S1 as four hexadecimal digits: 0000 to 3F7F, 8,192 values. The bits of bytes 9 and 11 that do not carry the sub-code carry control bits C4, C5 and C6, and shall never be read as part of it. Decoding is REQ-PKT-002.
*Reason:* Clause 9.3.1.2.
*Verification:* Test of every value of each element, with C4, C5 and C6 set and clear.

**REQ-NUM-006 — A sub-page is its full page address.**
A sub-page shall be identified by its magazine, page number and all thirteen bits of its sub-code. Two sub-pages whose sub-codes differ only in S4 or S3 are different sub-pages, and shall be stored, listed and served as such. Some receivers of other makers store sub-pages with the same S2 and S1 in one place, whatever S4 (TR 100 287 clause 10.1); NeoTeletext shall not.
*Reason:* Clause 7.2.2 defines the page address as page number plus sub-code; the code of practice describes the merging as other decoders' behaviour, not a rule.
*Verification:* Test with sub-codes 0001, 0101, 1001 and 2001 on one page.

**REQ-NUM-007 — The null page address.**
The page address XFF:3F7F is reserved as the null page address (clause 7.2.2). A page header with page number FF and sub-code 3F7F shall never be transmitted (Annex A.1, rule 3): the encoder shall refuse to write one, and the decoder shall treat one received as a header that ends a page and starts none. In a link (packets X/27/0 to X/27/3, clause 9.6.1; X/27/4 to X/27/7 Format 2, clause 9.6.3; the initial page of packet 8/30 Formats 1 and 2, clauses 9.8.1 and 9.8.2), XFF:3F7F means that no page is specified.
*Reason:* Clause 7.2.2; Annex A.1, rule 3.
*Verification:* Test of the encoder's refusal and the decoder's handling, for a header and for each kind of link.

## Page numbers

**REQ-NUM-008 — Page FF: time filling and terminating.**
Page number FF in each magazine, with any sub-code 0000 to 3F7E, is for time-filling and terminating headers only (clause 7.3; Annex A.1, rule 2; Annex A.10). Such a header shall end the page in reception as REQ-ACQ-003 says, and shall carry no data for acquisition: it shall never be stored as a page, listed, selected or served as one. A channel, a kit or the server shall never publish a page on page number FF. The server should end a page with a page FF header where a magazine has few pages and the page would otherwise not be ended promptly, as for subtitles (TR 100 287 clause 10.5). EN 300 708 clause 4.3.1.1 also forbids FF for any data service.
*Reason:* Clause 7.3; Annex A.1; Annex A.10; TR 100 287 clause 10.5; EN 300 708 clause 4.3.1.1.
*Verification:* Test that a page FF header ends a page and is not stored, and that publishing on FF is refused with the reason.

**REQ-NUM-009 — Pages that may carry data.**
Any page number 00 to FE, with any sub-code 0000 to 3F7F, may carry data (Annex A.1, rule 1). A page that a link names shall have a sub-code up to and including 3F7E (TR 100 287 clause 10.1), since 3F7F in a link means "no particular sub-code" (REQ-NUM-018). The decoder shall respond to page headers for every page number and sub-code, those with hexadecimal elements included (clause 9.1, Table 1, packet X/0).
*Reason:* Annex A.1; TR 100 287 clause 10.1; Table 1.
*Verification:* Test of reception on every page number 00 to FE and of sub-codes 0000, 0001, 3F7E and 3F7F.

**REQ-NUM-010 — Reserved page numbers.**
These page numbers shall be used solely for the information indicated (Annex A.10), in every magazine unless a magazine is named:
- 1BE: Automatic Channel Installation (ACI; Annex L);
- 1F0: the Basic TOP Table (BTT; clause 11.2.2);
- mFD: the Magazine Inventory Page (MIP; clause 11.3);
- mFE: the Magazine Organization Table (MOT; clause 10.6);
- mFF: time filling and terminating (REQ-NUM-008).

A channel, a kit or the server shall not publish a display page on any of them, and the decoder shall read a page on one only as the function it is reserved for. That leaves 8 × 253 − 2 = **2,022** page numbers in a service that may carry a page.
*Reason:* Annex A.10.
*Verification:* Test that each reserved number is refused for a display page with the reason, and read as its function by the decoder; test that the count of usable page numbers is 2,022.

**REQ-NUM-011 — The 2,022 page numbers are shared by every kind of page.**
The 2,022 page numbers carry display pages and data pages alike: the enhancement data pages (Public and Global Object Pages and DRCS pages, which the MOT names, clause 10.6), the TOP data pages (AIT, MPT and MPT-EX, which the BTT names, clause 11.2.2), data broadcasting pages (EN 300 708) and trigger pages. They are the ceiling on page numbers, not a count of display pages, and no part shall describe them as pages for content alone. A service may have up to seven POPs and one GPOP, and up to seven DRCS pages and one GDRCS page, in each magazine (clauses 10.5.1 and 10.5.2).
*Reason:* Clauses 10.5, 10.6 and 11.2; the owner's instruction that this caveat be a requirement.
*Verification:* Inspection of the documents and of `limits`' names; test of the MOT's limits.

**REQ-NUM-012 — Data pages use hexadecimal page numbers.**
Pages used for object definitions and DRCS data, and pages of data broadcasting, should have page numbers that include a hexadecimal digit, A to F, in the tens or the units, so that they are never taken for display pages (Annex E.1; TR 100 287 clause 10.1; EN 300 708 clause 4.3.1.1). The kits and the server shall give such pages such numbers by default. The decoder shall not assume that a page is a display page because its number is decimal, nor a data page because it is not (TR 100 287 clause 10.12): the page function decides (REQ-NUM-027).
*Reason:* Annex E.1; TR 100 287 clauses 10.1 and 10.12; EN 300 708 clause 4.3.1.1.
*Verification:* Test of the kits' defaults and of the decoder's classification by page function.

**REQ-NUM-013 — Recommended page numbers.**
These page numbers are recommended, not reserved (Annex E.15):
- 100: the service's home page, which packet 8/30 Format 1's Initial Teletext Page may reassign (REQ-SVC-004);
- 1DF: data broadcasting of EPG data (EN 300 707), unless a MIP names another;
- 1E7: trigger messages, unless a MIP names another;
- 1F1 to 1F4: the TOP data pages (AIT, MPT, MPT-EX), whose numbers the BTT assigns.

A channel should use them for those functions, and say in its MIP when it uses others. The decoder shall not assume them: it shall take each function's page number from the MIP, the BTT or packet 8/30 when they give it.
*Reason:* Annex E.15.
*Verification:* Test of the kits' defaults; test that the decoder follows a MIP or BTT that names other numbers.

**REQ-NUM-014 — Page numbers ending in F.**
Page numbers whose tens or units digit is F are often used for engineering or system control, and so may not be available for editorial use (TR 100 287 clause 10.1). Channels should not put display pages on them, and the kits shall warn when one does. This is advice, not a reservation: the decoder shall accept such pages as any other.
*Reason:* TR 100 287 clause 10.1.
*Verification:* Test of the kits' warning; test of reception.

**REQ-NUM-015 — Pages a viewer can key.**
A viewer selects a page by three decimal digits, 100 to 899: **800** page numbers (TR 100 287 clause 10.1; REQ-NAV-001). Pages with a hexadecimal digit are not selected directly (clause 9.1, Table 1, note) but are reached by links (TR 100 287 clause 10.1) and stored where the presentation level needs them (REQ-ACQ-017).
*Reason:* TR 100 287 clause 10.1; Table 1.
*Verification:* Test (REQ-NAV-001's and REQ-ACQ-017's).

## Sub-codes of display pages

**REQ-NUM-016 — Sub-codes of display pages.**
For a display page (Annex A.1, the rules for pages intended for display; TR 100 287 clause 10.1):
1. A page with no sub-pages shall be coded 0000.
2. A rotating set of up to 79 sub-pages whose editorial content differs from one to the next shall use sequential sub-codes, 0001 to 0009, then 0010 to 0019, and so on to 0079, the decimal values of S2 and S1.
3. A rotating set of more than 79 sub-pages shall use unique sub-codes greater than 0079 on all its sub-pages.
4. A rotating set whose content changes only slightly from one sub-page to the next, so that it is meant to be seen as one page updated (an index page that highlights a different topic each time), shall use sub-codes greater than 0079, a unique one on each sub-page.
5. A sub-page whose editorial content does not differ from the one before may be sent with the same sub-code as that one.
6. A single page that changes on each transmission, such as a rotating advertisement, should use a non-zero sub-code outside 1 to 79 (Annex A.1, note 1), so that receivers collecting sub-pages do not take it for a set.

The kits and the server shall number a channel's sub-pages by these rules (REQ-NUM-019); the decoder shall accept sub-pages that do not follow them (REQ-NUM-002).
*Reason:* Annex A.1; TR 100 287 clause 10.1, which the owner's instruction makes requirements.
*Verification:* Test of the numbering function for sets of 1, 2, 79, 80 and 8,069 sub-pages, and of each rule.

**REQ-NUM-017 — The decimal sub-codes a viewer can key.**
A viewer enters a sub-page by its decimal sub-code (REQ-NAV-005): four decimal digits, S4 0 to 3, S3 0 to 9, S2 0 to 7 and S1 0 to 9, from 0001 to 3979: **3,199** sub-codes, since S2 has three bits. Of them, 0001 to 0079 are the first set's (REQ-NUM-016 rule 2), and **3,120** are greater than 0079.
*Reason:* Annex A.1; clause 9.3.1.2.
*Verification:* Test that entry accepts exactly these 3,199 and no other.

**REQ-NUM-018 — Sub-codes in links.**
In a link that names a sub-page (packets X/27/0 to X/27/3, clause 9.6.1; X/27/4 to X/27/7 Format 2, clause 9.6.3; the initial page of packet 8/30 Formats 1 and 2, clauses 9.8.1 and 9.8.2, whose magazine is absolute, not relative):
- sub-code 3F7F means that no particular sub-code is specified, and only the three-digit page number is used (TR 100 287 clause 10.6); following such a link shows the page as REQ-NAV-005 does;
- page number FF means that no particular page number is specified;
- XFF:3F7F means that no page is specified, and the key the link belongs to is disabled while it is (TR 100 287 clause 10.6).

So a sub-page coded 3F7F cannot be named by a link on its own, and a display page that is linked to shall use sub-codes up to 3F7E (REQ-NUM-009). Decoding is REQ-PKT-005 and REQ-NAV-003.
*Reason:* Clauses 9.6.1, 9.6.3, 9.8.1 and 9.8.2; TR 100 287 clause 10.6.
*Verification:* Test of each case for each kind of link.

**REQ-NUM-019 — Numbering a page's sub-pages.**
The kits and the server shall give the nth sub-page of a display page its sub-code by one function, `limits::sub_code_of(place, count)`, with its inverse, following REQ-NUM-016:
- a page of one sub-page: 0000;
- a page of 2 to 79 sub-pages: places 1 to 79 take 0001 to 0079, in order;
- a page of more than 79 sub-pages: every place takes a sub-code greater than 0079, the 3,120 decimal ones first in ascending order (0100 to 3979), then the 4,949 with a hexadecimal digit greater than 0079 in ascending order, 3F7F left out: at most **8,069** sub-pages.

A page that grows past 79 sub-pages, or falls back to 79, is renumbered: every one of its sub-codes changes once, and the change is published as the page's change. Sub-codes a channel gives itself (in TTI's `SC` line) shall be kept as given, whatever they are (REQ-NUM-002).
*Reason:* REQ-NUM-016; one numbering, written once (decision E.122).
*Verification:* Test of the function and its inverse at every boundary, and that it never yields 0000 for a page of several sub-pages, nor 3F7F.

**REQ-NUM-020 — How many sub-pages a display page may have.**
The standard counts up to **8,190** sub-pages for a normal page: the MIP's page codes 02 to 4F give 2 to 79, code 50 gives 80 to 2^12 − 1 (4,095) and code 51 gives 2^12 (4,096) to 2^13 − 2 (8,190) (clause 11.3.3, Table 24; clause 11.3.4). Every part shall accept, hold, list and serve a display page of up to 8,190 sub-pages, whatever their sub-codes (REQ-NUM-002, REQ-NUM-030). A page whose sub-pages follow REQ-NUM-016 has at most 8,069 when it has more than 79.
*Reason:* Clause 11.3; REQ-NUM-002.
*Verification:* Test with a page of 8,190 sub-pages in each part.

**REQ-NUM-021 — Time-coded sub-codes.**
A page's sub-code may carry the time instead of a sub-page number, as for an alarm clock page: minutes units in S1, minutes tens in S2, hours units in S3 and hours tens in S4, each binary-coded decimal (Annex E.2; TR 100 287 clause 10.1, item 4). A MIP marks such a page with code 79, "page not following normal sub-code rules", needing one page of storage (Table 24). For a page so marked the decoder shall keep only the latest version and shall not list its sub-codes as sub-pages; without a MIP it shall treat it as any page (REQ-NUM-002).
*Reason:* Annex E.2; Table 24.
*Verification:* Test with and without the MIP's code 79.

## Sub-codes of other kinds of page

**REQ-NUM-022 — Sub-codes of enhancement data pages.**
The sub-code of each sub-page of an enhancement data page (POP, GPOP, DRCS, GDRCS, MOT, MIP) is coded nnXs, not as a display page's (Annex A.1; clauses 10.5.1.1, 10.5.2.1, 10.6.1 and 11.3.2; Annex B.6, rule 4):
- nn, S4 as the two most significant bits and S3 the four least, is the last packet that would be sent if the page were sent without interruption (Annex B.2), from 1 to 41: 1 to 25 are packets X/1 to X/25, 26 to 41 are X/26/0 to X/26/15. A partial page gives the last packet of this transmission, unchanged when the page is sent in fragments;
- X, S2, is an update indicator, incremented modulo 8 at each change of the sub-page (Annex B.6, Table B.1);
- s, S1, alone identifies the sub-page, from 0 for the first, incremented for each.

So a POP, GPOP, DRCS or GDRCS page has **at most 16** sub-pages, and a MOT or MIP page **one**, its S1 always 0. The encoder shall write these sub-codes; the store shall key enhancement sub-pages by S1 and record nn and X (REQ-ACQ-020, REQ-ACQ-021); and no part shall apply REQ-NUM-016's rules to them or list them as sub-pages.
*Reason:* Annex A.1; clauses 10.5, 10.6 and 11.3; Annex B.6; the owner's instruction that this caveat be a requirement.
*Verification:* Test of the encoder's sub-codes for each kind of page, of the store's keying, and of Table B.1's rows.

**REQ-NUM-023 — Links to enhancement data pages.**
- **The MOT** (clause 10.6.4): a link gives a magazine, a page number and the number of sub-pages, the highest S1 sent, 0 meaning one and F sixteen; page number XFF means the link is not used.
- **Packets X/27/4 and X/27/5 Format 1** (clause 9.6.2): a link gives a relative magazine (the packet's own magazine with each of three bits complemented where set), a page number and sixteen sub-page flags, one for each S1 value 0 to 15 naming the sub-pages required; page number FF with every flag set means no link. Where sent, they take precedence over the MOT (clauses 10.5.1.4 and 10.5.2.2); they are essential where a rotating page's sub-pages need different enhancement pages (TR 100 287 clause 10.7).

Decoding is REQ-PKT-011 and area OBJ.
*Reason:* Clauses 9.6.2, 10.5.1.4, 10.5.2.2 and 10.6.4; TR 100 287 clause 10.7.
*Verification:* Test (REQ-PKT-011's and area OBJ's).

**REQ-NUM-024 — Data broadcasting pages have no sub-pages.**
On a data broadcasting page of EN 300 708 the sub-code does not number sub-pages (clauses 4.3.1.2 and 4.4.1):
- S3 is the data stream identifier: up to 15 data streams share one page number, value F being reserved; its least significant bit says whether the 20 ms rule applies;
- S1 is the continuity index, counting 0 to F, modulo 16, for each page of different application data in its stream;
- S2 and S4 together, S2 the three least significant bits, are the last packet sent, 1 to 25 (normally 1 to 23), so 3F7F never occurs.

The decoder shall not list, step through or count such sub-codes as sub-pages, nor apply REQ-NUM-016 to them. A data broadcasting page is known by its page function 0001 in packet X/28/0 Format 1 (REQ-NUM-027) or by the MIP's codes E0 to E4.
*Reason:* EN 300 708 clauses 4.3.1.2 and 4.4.1; the owner's instruction that this caveat be a requirement.
*Verification:* Test with a page of three data streams and a continuity index that wraps.

## Tables of pages

**REQ-NUM-025 — The Magazine Organization Table's page.**
The MOT is page mFE of each magazine, its sub-code coded as an enhancement data page's with S1 0 (clause 10.6.1; REQ-NUM-022). Its packets X/1 to X/14 hold a pair of bytes for each page number X00 to XFE, in the order of the standard's figure 20, the decimal numbers first. Decoding is area OBJ.
*Reason:* Clause 10.6.1.
*Verification:* Test of the pairs' positions for every page number.

**REQ-NUM-026 — The Magazine Inventory Page.**
The MIP is page mFD of each magazine, its sub-code coded as an enhancement data page's with S1 fixed at 0 (clause 11.3.2; REQ-NUM-022). Its packets X/1 to X/14 give a page code for each page number 00 to FF, in the order of the standard's figure 26, the decimal numbers first; its packets X/15 to X/24 give the number of sub-pages for the page codes that need it, in groups of three bytes, thirteen bits where the code's least significant bit is the most significant (clause 11.3.4). The page codes are Table 24's, among them: 00 not in transmission; 01 a single normal page; 02 to 4F a normal page of 2 to 79 sub-pages, the code being the number; 50 and 51 a normal page of 80 to 8,190 sub-pages; 70 to 77 subtitles; 79 a page not following the sub-code rules; 7B to 7F, 81 to D1 TV-related and schedule pages, with their counts; E0 to E4 data broadcasting; E5 to EF DRCS and object pages; F0 to F9 system, engineering and keyword pages; FC trigger messages; FD ACI; FE TOP pages; 52 to 6F, D2 to DF, FA, FB and FF reserved. (Table 24 lists FA to FC as reserved and then FC as the trigger message page; the second, being the specific one and matching Annex E.15's trigger page, is taken.) The decoder may use a MIP to plan its storage (clause 11.3.1); a channel served by the server should send one, giving each page's code and number of sub-pages.
*Reason:* Clause 11.3; Annex B.8.
*Verification:* Test of the decoder's reading of every code and of a thirteen-bit count; test that the server's MIP counts a page's sub-pages.

**REQ-NUM-027 — The page function decides how a page's numbers are read.**
Packet X/28/0 Format 1 (and X/28/3, X/28/4) gives a page's function in its first four bits (clause 9.4.2.1, Table 3): 0000 a basic Level 1 page; 0001 data broadcasting (EN 300 708 clause 4); 0010 a GPOP; 0011 a POP; 0100 a GDRCS; 0101 a DRCS; 0110 a MOT; 0111 a MIP; 1000 a BTT; 1001 an AIT; 1010 an MPT; 1011 an MPT-EX; 1100 trigger messages; the rest reserved. The function decides which rules read the sub-code: REQ-NUM-016 for a basic page, REQ-NUM-022 for enhancement data pages, REQ-NUM-024 for data broadcasting. A page with no packet X/28/0 is a basic Level 1 page (REQ-ACQ-014), unless the MOT, the MIP or the BTT names it as another function. The page function's other bits are clause 9.4.2's (pages other than basic or data broadcasting set bits 8 to 18 to 11111111100, clause 9.4.2.4).
*Reason:* Clause 9.4.2.
*Verification:* Test of each function's reading of the sub-code.

**REQ-NUM-028 — TOP's page numbers.**
The Basic TOP Table is page 1F0 (clause 11.2.2; Annex A.10); the Additional Information Table, Multi-Page Tables and Multi-Page Extension Table have the page numbers the BTT gives them (clause 11.2.2), 1F1 to 1F4 recommended (Annex E.15); a MIP marks them with code FE. Each has packets X/0 to X/23 only, with an optional X/28/0 Format 1. The TOP tables describe the decimal pages 100 to 899 (clause 11.2, figure 24). TOP combined with parallel magazine transmission is not recommended for Level 1 and 1.5 services (Annex I). NeoTeletext does not navigate by TOP, and whether it should is not decided; it shall store and serve TOP pages as the data pages they are, at their numbers, and shall not list them as display pages.
*Reason:* Clause 11.2; Annexes E.15 and I.
*Verification:* Test that the BTT's page numbers are read and that TOP pages are not offered as display pages.

## Order and ending of pages

**REQ-NUM-029 — Sequence, serial and parallel.**
A magazine's pages are normally sent in numerical sequence (clause 7.3); a page sent out of sequence for editorial reasons has control bit C9 set, which keeps its header out of a rolling header display (clause 9.3.1.3, Table 2; Annex B.3, b; Annex B.4, b). Control bit C11 sets serial or parallel transmission, the same in every header of a service (Table 2): in serial mode a page ends at the next header of any magazine, in parallel mode at the next header of its own magazine (clause 7.2.1; Annexes B.3 and B.4). Acquisition is REQ-ACQ-001 to REQ-ACQ-003. The server shall set C9 on a page it sends out of numerical order, and C11 the same in every header.
*Reason:* Clauses 7.2.1, 7.3 and 9.3.1.3; Annex B.
*Verification:* Test of the encoder's C9 and C11 and of the decoder's rolling header.

## Every sub-page held, and one definition

**REQ-NUM-030 — Every sub-page held and reachable; budgets bound memory, never what exists.**
A display page may have 8,190 sub-pages (REQ-NUM-020), and every part shall support every one of them:
- **The server** shall hold every sub-page a channel publishes, up to 8,190 a page, on every page of a channel.
- **The browser** shall list every sub-page of a page, step through them all, and show any one chosen, at any number up to 8,190.
- **Sub-pages pass the wire only when needed** (decision E.122, the owner's instruction "I WANT SUB PAGES TO PASS THE WIRE ONLY WHEN NEEDED"): a client receives a sub-page's content only while it is shown or next to the one shown, a page's catalogue of sub-codes and their changes, and titles only for the places its list shows (`neoteletext-common/STANDARD-LIMITS.md` section 4; `SERVER.md` sections 4 to 6; REQ-WEB-019).
- **Stores are caches.** A part may bound what it holds or sends at once, as untrusted input requires (REQ-SEC-012, REQ-ACQ-005), but only as a budget of memory or traffic, never as a limit on which sub-pages exist or can be reached: a sub-page that falls out of a store is fetched again when it is needed, on a channel served over the web, or stored again when it next arrives, from a broadcast; the latest sub-page of the page shown is always kept.
- Each budget shall be a named constant, stated with the memory it costs (REQ-GEN-009), and given to channel programs by the server (REQ-NUM-031).
*Reason:* The owner's instructions that NeoTeletext implement the standard's maximum sub-pages, "i just said we are implementing the maximum sub pages", and that sub-pages pass the wire only when needed.
*Verification:* Test in each part with a channel of 8,190 sub-pages on a page: every one published, held, listed, reached and shown; bytes and memory measured when the page is opened, when a sub-page changes and when the viewer steps.

**REQ-NUM-031 — One definition.**
The numbers of this area (magazines, page numbers, the reserved and recommended page numbers, the sub-code layouts, the counts 2,022, 800, 8,190, 8,069, 3,199 and 3,120, the null address) and the numbering function of REQ-NUM-019 shall be written once, in `neoteletext_core::limits`: `no_std`, with no dependency and no feature gate, each constant with its clause (decision E.122). The server and the browser shall import them at their pinned engine commit; programs in other languages shall be given them by the server's `start` message; no part shall write any of them as a literal.
*Reason:* The owner's instruction of 26 October 2026 that the limits come from one piece of code (decision E.122).
*Verification:* Check: a search for the numbers as literals outside `limits`; test of `limits` against every row of this area.
