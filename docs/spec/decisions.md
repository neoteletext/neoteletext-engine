# Part E — Decision Register

Each entry records a place where the standard is silent, contradicts itself, or differs from the reference chips. It states the decision taken and why. Requirements in Part F refer to these decisions by number. Where a decision has a reasonable alternative, the alternative is offered as an option in a compatibility profile (see area PRF in Part F).

**E.1 — Scope of presentation levels and how Level 1.5 is layered.**
The receiver implements Level 1.0 and Level 1.5 only. Level 1.5 is implemented as a separate pass. It takes the fully decoded Level 1 page, meaning the grid of cells with their attributes, and applies the packet X/26 character overwrites to it. The raw received page is never modified.
*Reason:* This is exactly how the reference SAA5246A receiver worked, with Level 1.5 done in microcontroller software over the stored page. It also keeps the Level 1 decoder simple and testable on its own. And it lets Level 2.5 be added later as a further pass without redesign.
*Amended 10 October 2026:* The receiver now implements Levels 1.0, 1.5, 2.5 and 3.5, chosen by the presentation level setting (decision E.40). The pass described above is the Level 1.5 pass and is unchanged. At Levels 2.5 and 3.5 the enhancement pass of decision E.42 runs in its place. That pass changes attributes as well as glyphs: the Level 1 spacing attributes and the non-spacing attributes of each affected row are resolved together, as Annex C.2 describes (decision E.47). The raw received page is still never modified.

**E.2 — Packet X/26 with designation code 15.**
By default, X/26/15 is processed like designation codes 0 to 14. The "SAA5246A" compatibility profile ignores it.
*Reason:* Clause 10.1 includes it in Level 1.5, but the standard also warns that many existing decoders ignore it, and the SAA5246A does not even store it. Processing it is the more correct default. Ignoring it reproduces period hardware.

**E.3 — Which row-address triplets set the active row.**
By default, the receiver follows Annex A.3: every triplet with an address from 40 to 63 sets the active row, whatever its mode. Address 40 means row 24, addresses 41 to 63 mean rows 1 to 23, and mode 00111 with address 63 means row 0. A "strict" option follows clause 12.3.2 instead. Under that option only modes 00001 (Full Row Colour), 00100 (Set Active Position) and 00111 (Address Display Row 0) set the row.
*Reason:* Annex A.3 is normative and describes how the Level 1.5 decoders that broadcasters tested against actually behave. The rule comes from EBU SPB 492 clause 14.6.4, the predecessor of EN 300 706. There, a row-address triplet of any mode sets the row for the character triplets that follow, which is how those decoders were built. Broadcasters were told to keep their data compatible with it; for example, Annex A.5 asks for Full Screen Colour to be sent with address 41. The two behaviours give the same result on correctly formed transmissions, so the choice only matters for malformed data.
*Amended 10 October 2026:* At Levels 2.5 and 3.5 the strict rule of clause 12.3.2 always applies, whatever this setting says (decision E.46). Object Invocation and Object Definition triplets carry object numbers in their address field, so the Annex A.3 rule cannot be used there. The Annex A.3 rule remains the default at Level 1.5 only.

**E.4 — Default character set designation at Level 1.5.**
The national option subset is always taken from the header bits C12, C13 and C14, following Annex E.9. The "group", which is bits 11 to 14 of the Table 32 value, is resolved in this order:
1. from packet X/28/0 format 1 of the page, if present and correctly decoded;
2. otherwise, from the latest packet M/29/0 of the page's magazine;
3. otherwise, from a configured default region, which is itself 0000 (Western European) if not configured.

Packets X/28/1 and M/29/1 are parsed only for diagnostic output and do not affect character sets.

The designation found this way also sets:
- the default G0 and G2 sets used by X/26 characters (clause 15.2);
- the second G0 set used by the ESC attribute (Table 33).

*Reason:*
- Table 25 permits X/28/0 and M/29/0 designation at Level 1.5.
- Using the group from them fixes the "ambiguous reference" that clause 15.2 describes, at no cost when they are absent.
- The codes carried by X/28/1 and M/29/1 are defined in EBU SPB 492 (1992), Figure 14, which is now available locally. They are still used only for diagnostics, for three reasons:
  - The newer X/28/0 and M/29/0 designation covers the same need.
  - One of the Latin codes (70) is contradicted by its own table in SPB 492.
  - EN 300 706 reuses the second triplet of X/28/1 for colour tables of redefinable characters.

  Only the first triplet of X/28/1 and M/29/1 shall be read. Table 25 allows single-region equipment to ignore this data.

*Amended 10 October 2026:* At Levels 2.5 and 3.5 the national option is taken from bits 8 to 10 of the highest-priority X/28 or M/29 designation when one is present (Annex E.9), and from C12 to C14 otherwise. The order of priority is that of Annex C.4 rule 1: X/28/0 format 1, X/28/4 (Level 3.5 only), M/29/0, M/29/4 (Level 3.5 only) (decision E.44). The first triplet of X/28/1 and M/29/1 remains for diagnostics only. Their other triplets carry the DRCS colour tables at Level 3.5 (decision E.54). The rule above is unchanged at Level 1.5.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.5 — How a Level 1.5 overwrite interacts with the Level 1 page.**
The standard is silent on the details (Part B.9). The receiver adopts the following rules:

- (a) All Level 1 attribute processing is done first, from the raw Level 1 bytes, exactly as if no X/26 existed. A spacing attribute in a cell that is later overwritten still has its full effect on the row, at its set-at or set-after timing.
- (b) The overwrite replaces only the glyph shown in the cell. The character keeps every display property that the Level 1 pass computed for that cell: foreground colour, background colour, flash, conceal, box membership and size. This follows the principle in clause 13.3.
- (c) The overwriting character is always drawn as an alphanumeric-style glyph from the character set the triplet names, even if the cell is in mosaic mode. Separated and contiguous mosaic modes do not apply to it.
- (d) Overwrites never change the held mosaic character. The held mosaic is computed from the Level 1 bytes only. If an overwrite lands on a cell that would show a held mosaic, the overwrite wins, because local enhancement data has higher priority than the Level 1 page (clause 13.2).
- (e) An overwrite at the top half of a double-height character is drawn double height, and its bottom half appears in the row below.
- (f) An overwrite addressed to a cell in the lower row of a double-height row is not displayed.
- (g) An overwrite is displayed whether or not the Level 1 byte at that cell passed its parity check. Broadcasters may deliberately send bad parity there (Table 25).
- (h) If two overwrites address the same cell, the later one in transmission order wins.
- (i) Overwrites in row 0 are applied when the page is displayed, but not to headers shown in the rolling header during a search (Annex E.3).
- (j) Overwrites addressed to row 0, columns 0 to 7, are always discarded, because that area belongs to the receiver (REQ-SEC-005).

*Reason:* Rules (a), (b) and (e) follow the only relevant statements in the standard. The others are the simplest choices consistent with it. Rule (f) matches the behaviour the standard itself attributes to existing Level 1.5 decoders (Table 26 notes 3 and 6).
*Amended 10 October 2026:* The rules above are unchanged at Level 1.5. At Levels 2.5 and 3.5 these rules change:
- (a) and (b): the Level 1 spacing attributes and the non-spacing attributes are resolved together, row by row, under Annex C.2, so enhancement data changes attributes as well as glyphs (decisions E.42 and E.47).
- (c): a G1 character written by X/26 is a mosaic, and G0 and G2 characters can be underlined (decision E.50).
- (f): enhancement characters written to the lower row of a double-height row are displayed, in the cells not covered by lower halves (Table 26 codes 0/D and 0/F; decision E.15, as amended).
- (h): the later of two writes to the same cell wins only within one display priority; between priorities, clause 13.2 decides (decision E.51).

Rules (d), (e), (g), (i) and (j) apply unchanged at every level.

**E.6 — Where a page ends.**
The receiver follows clause 7.2.1 and Annexes B.3 and B.4:
- In serial mode (C11 = 1), the next header of any magazine ends the current page.
- In parallel mode (C11 = 0), only the next header of the same magazine ends it.

The simplified wording in Table 2 is not used.
*Reason:* Clause 7.2.1 is the precise definition, and two annexes agree with it.

**E.7 — Uncorrectable Hamming 8/4 errors.**
- If either packet address byte (byte 4 or 5) is uncorrectable, the whole packet is discarded, because its magazine and row are unknown.
- If a header's magazine is known but any of bytes 6 to 13 is uncorrectable, the header is used to **end** the current page, as the termination rules require. It does **not** start the reception of a new page, and its displayable bytes are not used for the rolling header.
- In a Fastext link, an uncorrectable byte makes that single link invalid (treated as "no link").
- In packet 8/30, an uncorrectable byte makes the field it belongs to invalid.

*Reason:* The standard says only "reject data bits". These rules never act on an address that might be wrong, and never lose a page ending.

**E.8 — Characters that fail their parity check.**
Every stored character carries a flag saying whether it passed its parity check. By default:
- A new character that fails its check does not replace a character that passed its check in an earlier reception of the same page and sub-page, unless the page was erased in between (C4 set, or a new sub-code).
- A cell that has never held a character that passed its check is displayed as a space.

The two alternative policies are "always show a space" and "show a visible error marker", the latter for diagnostics.
*Reason:* The standard is silent. Keeping the last good character is how a receiver builds a clean page out of several noisy retransmissions, which is common on weak signals and on recovered video recordings.

**E.9 — Uncorrectable Hamming 24/18 errors.**
A triplet that fails its Hamming 24/18 check is ignored completely. It neither moves the active position nor writes anything. A single-error position greater than 24 is treated as uncorrectable. An uncorrectable designation code makes the whole X/26 packet unusable.
*Reason:* The standard is silent. Ignoring a single triplet loses at most one character, which is the least harmful result.

**E.10 — Columns 0 to 7 of row 0.**
Following the SAA5246A:
- Columns 0 to 6 are a status area written by the receiver. By default it shows the page number being requested or displayed, for example "P100".
- Column 7 is an alphanumeric colour attribute generated by the receiver. It is white when the displayed page has been found, and green while the requested page is being searched for.

The header's own bytes 6 to 13 are never displayed. The status area content is configurable.
*Reason:* The standard leaves these columns to the decoder (Annex E.7). The SAA5246A convention is the most widely seen one.

**E.11 — Rolling header and clock.**
While a requested page has not yet been received, row 0 columns 8 to 31 show the displayable text of incoming headers:
- In serial mode, from every magazine.
- In parallel mode, from the requested magazine only.
- Headers with C9 set, and headers rejected under decision E.7, are excluded.

Columns 32 to 39, the clock, are updated from every valid header of any magazine, whether or not the page has been found. This matches the SAA5246A's "always rolling" clock. Rolling can be disabled.
*Reason:* This follows Annexes B.3 and B.4 and the SAA5246A.

**E.12 — Flash timing.**
Flashing characters are shown for 48 fields and hidden for 16, as the real SAA5050 does. That is 960 milliseconds shown and 320 milliseconds hidden at 50 fields per second, which the datasheet describes as 0.75 hertz with a 3 to 1 ratio (Part C.8). When time is supplied in milliseconds rather than fields, those durations are used. The timing is configurable. In the hidden phase the character's foreground pixels are drawn in the background colour.
*Reason:* The standard gives no rate. The SAA5050 is the rendering baseline.
*Amended 10 October 2026:* Unchanged at Levels 1 and 1.5. At Levels 2.5 and 3.5, Table 29 defines a slow rate of 1 hertz and a fast rate of 2 hertz, and flash follows decision E.48, which keeps the 3 to 1 ratio of this decision within a one-second cycle.

**E.13 — Character rounding rule.**
The following rule is used for the 12-by-20 rendering mode. The SAA5050 datasheet describes rounding only in words (Part C.4). This exact rule is the one used by the MAME emulator (its SAA5050 device, source lines 433 to 436), and jsbeeb uses the same expression. beebjit, Bedstead and the published analysis of a decapsulated SAA5050 are equivalent to it. It is therefore confirmed, not assumed.

Each 5-dot glyph row is first doubled to 10 half-dots. Each glyph row then produces two output lines:
- **The upper line** of the pair takes the row above as its neighbour B.
- **The lower line** takes the row below as its neighbour B.
- **Interlaced output:** these are the first field's and the second field's lines respectively.

For the row being drawn (A) and its neighbour (B), a half-dot is added wherever all three of these hold:
- A has a set dot;
- B has a set dot diagonally adjacent to it;
- the dot orthogonally between them in B is clear.

In bit-mask form, working on half-dot rows:

    A | ((A >> 1) & B & ~(B >> 1)) | ((A << 1) & B & ~(B << 1))

Further rules:
- **No rounding across the cell edge.** A neighbour row outside the 9-row glyph counts as empty.
- **Scope.** Rounding applies to alphanumeric glyphs, including blast-through capitals in mosaic mode, and never to mosaics.
- **Double height.** Each line of the rounded 12-by-20 glyph is repeated twice, which is what all four emulators do.

The datasheet's wording ("referenced alternately up and down changing every line") could also be read as alternating the neighbour on every output line in double height. That difference is only visible on double-height diagonals. It shall be checked against photographs of a real SAA5050 (REQ-TST-030), and the rule changed if they disagree.

**E.14 — Mosaic geometry.**
In a cell of W half-dots by H lines, the geometry is as follows.

*Contiguous mosaics:*
- The two block columns are each W/2 wide.
- The three block rows are 3/10, 4/10 and 3/10 of H, taken from the SAA5050 drawing. That is 6, 8 and 6 lines in the 20-line mode, and 3, 4 and 3 lines in the 10-line mode.

*Separated mosaics:*
- Each block loses its leftmost dot column and its bottom line, drawn in background colour.
- In the 6-dot by 10-line SAA5050 cell, that is dot columns 0 and 3 and lines 2, 6 and 9.
- In the 12-by-20 cell, it is half-dot columns 0, 1, 6 and 7 and lines 4, 5, 12, 13, 18 and 19.
- The 3/4/3 block heights and 3/3 block widths are also confirmed.
- This agrees with the MAME, jsbeeb and beebjit emulators and with the decapsulated-chip analysis (Part C.6).

**E.15 — The row below a double-height row.**
The transmitted content of that row is ignored entirely.
- Each cell below a double-height character shows the character's bottom half.
- Each cell below a normal-height character shows that character's background colour with no foreground.
- Double height in rows 0, 23 and 24 is treated as normal height.

*Reason:* Table 26 and Annex C.3. The SAA5050 system behaves the same way. Annex C.3 lets the decoder choose for illegal rows, and normal height is the safest choice.
*Amended 10 October 2026:* Unchanged at Levels 1 and 1.5. At Levels 2.5 and 3.5 (Table 26 codes 0/D and 0/F, Table 29 mode 01100 and Annex C.3):
- enhancement characters written to the lower row, by local X/26 data or by objects, are displayed in the cells not covered by lower halves;
- a row whose only double height or double size comes from non-spacing attributes does not suppress the row below;
- double width and double size follow Annex C.3 (REQ-L25-025 and REQ-L25-026).

Double height in rows 0, 23 and 24 is still treated as normal height.

**E.16 — Hold mosaics details.**
The receiver implements the Table 26 text exactly, including the rule that "The 'Held-Mosaic' character is always displayed in its original contiguous or separated form regardless of the mode prevailing at the time of substitution."

When a set-after attribute changes alphanumeric or mosaic mode, or changes size:
- the attribute's own cell is displayed with the state before the change, so it can still show the held mosaic;
- the held mosaic is reset to space immediately after that cell.

When a set-at attribute changes size (Normal Size), the reset takes effect before its own cell is displayed.
*Reason:* This is consistent with the worked example in Annex G.3.3 and with set-at and set-after timing. The decapsulated-chip analysis confirms that the real SAA5050 also shows the held mosaic in its stored contiguous or separated form, and that Hold Mosaics is set-at and Release Mosaics set-after. The 1976 Broadcast Teletext Specification (clause 3.1.7) already defined the held character the same way: the most recent character with bit 6 set, kept in its original form, reset by a change between alphanumeric and graphics or by a change of height.

**E.17 — Start Box.**
A box starts only when two Start Box attributes appear in consecutive cells, and it starts at the second of them. End Box ends a box from the next cell. An option allows a single Start Box to start a box from the next cell.
*Reason:*
- The standard says that "protection against false operation is provided by double transmission of Start Box control characters, with the action taking place between them".
- The widely used decoders zvbi and vhs-teletext both require the pair.
- On noisy signals, a single corrupted byte could otherwise make part of a subtitle or newsflash page appear.
- The worked example in Annex G.3.1 (`x 0B 0B 61 0A 0A`, with the second 0B, the "a" and the first 0A boxed) gives the same result under either rule.

**E.18 — Codes with no Level 1 function.**
By default, codes 0/0 and 1/0 are alpha black and mosaic black: set-after attributes like the other colour codes, which select alphanumeric or mosaic mode and a black foreground. Codes 0/E (double width) and 0/F (double size) are displayed as spaces and change no state. The "SAA5050" profile displays 0/0 and 1/0 as spaces with no effect, as that chip does.
*Reason:* Table 26 gives these codes no response at Levels 1 and 1.5, but current services transmit Level 1 pages that depend on them. A comparison with zvbi on off-air recordings (REQ-TST-021) found them on Italian RAI pages from 2021 (page 517 writes "SETTIMANA DAL 24/01 AL 30/01" after alpha black, in a row that starts in mosaic mode; pages 509 to 513 draw mosaics after mosaic black) and on a Dutch regional service from 2020 (page 100). Treating them as having no function turned that text into mosaics and those mosaics into letters. zvbi and the SAA5246A decode them, and decision E.38 gives recent broadcasts priority.
*Amended 10 October 2026:* previously the default ignored 0/0 and 1/0, and only the "SAA5246A" profile decoded them.

**E.19 — When a page is complete.**
The receiver shows rows as they arrive, as hardware decoders do. A page is complete when one of these happens:
- its terminating header is received (decision E.6);
- 100 milliseconds pass with no packet of the page.

Completion raises an event.
*Reason:* Annex B.9 allows a software decoder to use this 100 millisecond limit.

**E.20 — Sub-pages.**
The page store keeps each sub-page separately, keyed by its full sub-code. A request can name:
- a specific sub-code;
- "any sub-page", which shows the most recently received one;
- a digit-masked pattern, where each of the four sub-code digits may be marked "don't care", as the SAA5246A allows.

The viewer can step through the stored sub-pages of a page.

**E.21 — Bit order of C12, C13 and C14.**
The national option index is 4×C12 + 2×C13 + C14, as the Table 32 column heading and its note 1 give. C12 is data bit 2 of the header byte 13, C13 is bit 3 and C14 is bit 4, so the decoded 4-bit value of that byte must be bit-reversed within those three bits to form the index.

**Confirmed by experiment on 1 October 2026.** Page headers were decoded from two off-air digital television recordings in the FFmpeg sample archive (REQ-TST-022), where the two possible orders predict different languages:

| Page | Header bits (C12, C13, C14) | This rule gives | The reversed order would give |
|---|---|---|---|
| France 2, subtitle page 888 | 1, 0, 0 | index 4, **French** (correct) | index 1, German |
| An Italian channel whose descriptor lists Italian subtitles, subtitle page 777 | 0, 1, 1 | index 3, **Italian** (correct) | index 6, Czech and Slovak |

The reversed reading gives the wrong language in both cases, so it can be ruled out.

Because this ordering is easy to get wrong, it is pinned down by tests (REQ-TST-012), using these same recordings. In the page-file format "TTI", the page status field puts C12 in bit 7, C13 in bit 8 and C14 in bit 9, following the most widely used implementation.

**E.22 — Newsflash and subtitle pages.**
When a displayed page has C5 or C6 set, the receiver automatically switches to boxed display: only boxed areas are drawn, and the rest of the screen is "picture".
- When rendering to an image with no video underneath, "picture" is rendered as transparent pixels (alpha zero), or as a configurable fill colour if the output format has no transparency.
- When rendering on top of supplied video, the video shows through.

*Amended 10 October 2026:* At Levels 2.5 and 3.5, the boxing bit of an X/26 Display Attributes triplet also starts a boxed area on these pages. On other pages it starts a window, inside which the full row colour becomes transparent. Which of the foreground or background colour, the row or screen colour, and the picture each pixel shows follows Table C.1 (decision E.47 and REQ-L25-029).

**E.23 — Row 24.**
By default, row 24 is displayed only when the page carries packet X/27/0 with the "display row 24" flag set (Annex H). Pages whose links come from the TTI Link Extension have their row 24 drawn by the receiver instead (decision E.36, REQ-NAV-011). The "SAA5246A" profile shows row 24 whenever the page carries packet X/24, like that chip's automatic Fastext row.

**E.24 — Framing code tolerance in the software data slicer.**
By default, a framing code that differs from 0x27 in one bit is accepted, but only if the two packet address bytes that follow both decode without an uncorrectable error. A "strict" option requires an exact framing code.
*Reason:* Annex P records that existing decoders accept a one-bit framing error. Requiring valid Hamming in the address bytes guards against the false detections the same annex warns about.

**E.25 — Glyph source.**
**During development**, the glyphs are temporarily taken from the "Bedstead" font by Ben Harris (version 3.261). They are exported to `third_party/bedstead/bedstead-glyphs.txt`, and their provenance is recorded in `third_party/bedstead/PROVENANCE.md`.

- **What Bedstead is.** Bedstead's own work is dedicated to the public domain under CC0 1.0. However, many of its glyphs reproduce the SAA5050 typeface designs, taken from the datasheet code tables.
- **Why that is permitted.** Copyright in those designs still exists. Using them as a typeface is permitted in the United Kingdom by section 55 of the Copyright, Designs and Patents Act 1988, because the SAA5050 was first marketed more than 25 years ago.
- **Debian's view.** Debian classes Bedstead as "non-free" for exactly this reason.

**For release**, the glyphs shall be drawn originally for this project, and Bedstead shall be removed. The provenance of every glyph set is recorded. No bitmap is copied from any character generator ROM.
*Reason:* Legal basis (see "How to read this document"). A complete, well-tested glyph set lets development proceed now; original glyphs remove the remaining legal doubt before release.
*Amended 12 October 2026 by decision E.72:* Bedstead is the bundled font for release too, and the original glyphs are removed. The owner accepts the legal position described above.

**E.26 — Composition of diacritical marks.**
When rendering pixels:
- If the glyph set contains a precomposed glyph for the base letter plus mark, that glyph is used.
- Otherwise, the mark's glyph is overlaid on the base glyph.

When producing text, the base letter and the Unicode combining character for the mark are emitted, normalised to the precomposed form (Unicode Normalization Form C) where one exists.

**E.27 — Scope exclusions decided here.** The following are out of scope (see Part G):
- 525-line teletext (ITU-R BT.653 System B for 525 lines);
- the Table Of Pages navigation scheme;
- Programme Delivery Control decoding;
- Level 2.5 and 3.5.

*Amended 10 October 2026:* Levels 2.5 and 3.5 are no longer excluded; the owner brought them into Phase 1 (decision E.40). Only a layout algorithm for proportional spacing remains excluded, because the standard defines none (decision E.50).

*Note of 20 October 2026:* what this decision says of the Level 3.5 font styles (bold, italic and proportional spacing) and the Font Style triplet no longer applies; the triplet is not acted on (decision E.96).

**E.28 — Where real teletext data comes from, and what the receiver must read.**
A search on 1 October 2026 found that no television channel publishes its live teletext on the open internet as raw data. Real teletext data is available only in the following forms, and the input requirements are built around them.

1. **Digital television transport streams.** European broadcasters that still run teletext carry it inside their digital broadcasts as DVB teletext (EN 300 472). Recordings of these broadcasts are the most common form of real, raw teletext data.
   - The FFmpeg project's public sample archive contains off-air recordings with teletext tracks from Italian, French and English-language channels. They include a file that uses the 192-byte packet variant.
   - Some recordings list several languages in one teletext descriptor.
   - Requirements REQ-INP-006 to REQ-INP-008 cover this format.
2. **Broadcasters' own internet streams carry no teletext.** The live web stream of tagesschau24 (ARD) contained only video and audio when examined. The Das Erste and ZDF streams refused access from outside their countries. A receiver cannot rely on broadcasters' web streams for teletext.
3. **T42 archives recovered from recordings.** The Teletext Archaeologist archive holds over 2000 recovered service snapshots. REQ-INP-002 covers this format.
4. **A live feed of services over WebSockets.** The zxnet online teletext viewer receives live services from servers over the WebSocket protocol:
   - hobby services, including Teefax and a recreation of Ceefax;
   - a replay of a capture of Channel 4 teletext.

   Each message carries the 40 data bytes of a packet exactly as transmitted, with parity and Hamming coding intact, but the messages are wrapped in a request-and-response protocol of JSON messages. This protocol is not formally documented. It was established by reading the viewer's public JavaScript and by connecting to the servers, so it may change without notice. REQ-INP-022 covers it and requires the reader to be tolerant of unknown messages.
5. **Most broadcaster page interfaces are not raw data.** Most publish rendered pages:
   - ZDF, ARD, ORF, SVT, WDR, NDR and others publish HTML;
   - RAI Televideo publishes PNG images;
   - the Finnish broadcaster YLE publishes XML behind an API key.

   ZDF's site also runs a bot check, so it must not be read automatically. These interfaces are not supported.

Live raw teletext from a real channel therefore needs a digital television receiver tuned to a European broadcast that still carries teletext. Its output is the transport stream format of item 1.

*Amended on 10 October 2026* (research record: `research/answers-teletext-sources.md`). Two broadcasters do publish live pages with their teletext bytes intact, although as whole pages rather than packet streams:
- **SRG SSR** (SRF, RTS and RSI): the public site `teletext.ch` loads each page from `api.teletext.ch` as JSON. The page is held in EP1 format: a header, 23 rows of 40 bytes, and a per-page teletext level field.
- **NOS**: `teletekst-data.nos.nl/page/<page>` returns navigation lines followed by 25 rows of 40 bytes of Level 1 codes. NOS has already converted accented letters to ISO 8859-1, so they are not transmitted codes.

Item 5's earlier statement about NOS was wrong for this address. No broadcaster was found publishing a live packet stream. The newest real broadcast packets found are off-air recordings of TVP (Poland, November 2023) and NPO (the Netherlands, June 2020). The zxnet feed named `ch4` is a real-time replay of a Channel 4 broadcast of 15 July 2007.

**E.29 — How pixels are produced, and what earlier designs teach.**
Producing pixels portably is a solved problem, and the rendering design copies the solutions.

- **The SAA5050 itself.** It never held a picture in memory. For each television line it took a character code and the line number within the character cell, and produced one row of dots plus a foreground and background colour, in step with the scanning beam. The natural unit of teletext drawing is therefore **one cell, one scanline**.
- **zvbi.** Its page-drawing function writes a page region into memory supplied by the caller, in a pixel format and row stride the caller chooses. The caller owns memory and pixel format; the library only writes pixels.
- **Embedded graphics libraries.** They show how to fit small memories:
  - LVGL draws into a small partial buffer and hands each finished area to one port callback.
  - u8g2 draws a screen in horizontal strips.
  - The Rust `embedded-graphics` crate draws to anything that implements one drawing trait, which hundreds of display drivers do.
- **Doom**, the best-known example of software that runs on everything:
  - **A small platform layer.** It renders into a fixed-size 8-bit indexed framebuffer, and a port implements only a handful of platform functions. The "doomgeneric" project reduced these to six callbacks: initialise, draw frame, get time, sleep, get key and set window title.
  - **Colour at the edge.** The engine never handles real colours; the palette is applied only at output.
  - **Palette effects.** Screen-wide effects, such as the red damage flash, are done by changing the palette, not by redrawing.
  - **Scaling at the edge.** The engine always renders at one resolution and ports scale it. Ports that forgot Doom's pixels are 1.2 times taller than wide showed squashed pictures, which is the same 1.2 aspect factor as teletext (REQ-RND-004).
  - **Integer arithmetic only**, and byte order handled explicitly.
  - **Display decoupled from simulation**, with deterministic replay of recorded input.

The decisions taken from this are:

1. The core never produces real colours. It produces **palette indices**, and colour conversion happens only at the edge, in optional helpers or in the port (REQ-RND-018 and REQ-RND-023).
2. All teletext-specific drawing lives in one pure function that draws one line of one cell (REQ-RND-016). Every renderer and every port uses it, which makes the output identical everywhere.
3. **Once a page is decoded, every text row can be rendered on its own.** Every teletext feature that crosses rows is resolved while the page is decoded into the cell grid, before any pixels are drawn:
   - Double height suppresses the row below and places bottom halves there.
   - Level 1.5 overwrites address any position.

   Everything that remains is local to one row, or to one cell within it:
   - attributes, hold mosaics, conceal and boxes all reset at each row;
   - character rounding looks only at glyph lines within one cell;
   - flash and reveal depend only on time;
   - the row 0 status area is row-local;
   - page zoom is a re-mapping of rows, handled like double height.

   The only cross-row effect left is change tracking: a change to double height in one row also changes the row below (REQ-RND-019).

   So the library's whole pixel output is **"render text row r (or screen line y) into this buffer"**. The port asks for pixels; the library never pushes them. This is the same model as zvbi's page drawing (REQ-RND-017). A port's whole job is defined by a platform checklist in the style of doomgeneric (REQ-ARCH-009).
4. **Buffering belongs to the port.** Whether the port keeps a whole-screen framebuffer as Doom does, a ring of two or three row buffers, or a single row buffer, and when it renders relative to its hardware, is the port's decision. A port with a framebuffer just renders each changed row at the right offset in its own memory. The library offers a convenience loop for this (REQ-RND-022), but does not require it. An optional precomputed glyph line cache (REQ-RND-026) reduces drawing cost for ports that redraw often.
5. Flash and reveal are done by **changing the palette**, not by redrawing (REQ-RND-023).
6. The core never scales. Scaling and aspect correction happen in the output stage, with a helper so ports get the 1.2 aspect factor right (REQ-RND-024).
7. Decoding and drawing are decoupled. Drawing happens only when a row has changed or the flash state changes (REQ-RND-019).
8. **The library stops at the row renderer.** It writes pixels into buffers supplied by the target port's hardware layer, and does nothing else with them. It does not put pixels on a screen, generate video signals, keep to display timing, or contain code for any specific hardware. Those are the port's responsibility, reached only through the platform interface (REQ-ARCH-009 and REQ-ARCH-010).

*Amended 10 October 2026:* At Levels 2.5 and 3.5 (decisions E.42, E.49 and E.55):
- **Item 3.** Objects, row colours, side panels and the row extension of the Font Style triplet are further features that cross rows. They are all resolved while the grid is built, so rows stay independent. A row is 40 cells plus the side-panel widths. The patterns and colour tables of the DRCS characters a page uses travel with the grid, so a row still renders from the grid and the viewer state alone.
- **Item 5.** Flash and reveal are resolved at render time in the resolved colour mode, instead of by palette changes. The change tracker limits the redrawing to rows that flash or conceal. The palette technique remains for Levels 1 and 1.5.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

*Note of 20 October 2026:* what this decision says of the Level 3.5 font styles (bold, italic and proportional spacing) and the Font Style triplet no longer applies; the triplet is not acted on (decision E.96).

**E.30 — The web-native design, and what the jellytext project teaches.**
neoteletext shall read pages from ordinary web servers as well as from broadcast data. It does so over:
- HTTP;
- WebSockets;
- server push;
- cookies.

An earlier project in the same workspace, jellytext (August 2025, suspended), sketched this architecture:

```
web server → proxy (HTTP, sessions, cookies, TTI cache, TTI-to-T42 encoder) → packet stream → decoder → renderer
```

The decisions taken from it are listed below.

1. **Teletext's own formats all the way to the decoder.** A web source delivers TTI pages or T42 packets, and nothing else. Web pages enter the decoder exactly as page files and broadcast packets do (REQ-ARCH-004), so there is one decoder and one set of tests. All web-specific complexity stays in the web module (area WEB).
2. **Stateless servers, with the session in a versioned cookie.** jellytext's notes record the idea to "store session data in cookie with versioning". A neoteletext client treats cookies as opaque and only stores and returns them. The versioning is a server design rule, given as guidance for page servers (REQ-WEB-012).
3. **Asynchronous throughout.** The jellytext notes say the "application can be fully async by using callbacks". Every network operation shall be non-blocking and event-driven. The decoder is fed bytes as they arrive (REQ-INP-001).
4. **Interactive forms drawn as teletext pages.** jellytext's TTI Form System adds form commands after ordinary TTI rows. Decoders that do not know the commands ignore them and still show a readable page. neoteletext adopts it, corrected as decision E.31 describes.
5. **The transport belongs to the port.** Network sockets and encryption are supplied by the port through the platform interface (REQ-ARCH-009). This keeps the library free of any particular network stack, and is consistent with REQ-ARCH-010. jellytext planned to embed a network stack and an encryption library on each microcontroller, which is heavy.
6. **One platform at a time.** jellytext decided to "write for 3 platforms at once" (composite video from an ESP32, HDMI from an RP2350, and a video stream to YouTube) shortly before it was suspended. neoteletext keeps all platforms outside the library (REQ-ARCH-010), and streaming rendered pages as video is not part of it.
7. **Geometry settled once.** jellytext's notes changed their minds several times about pixel sizes. neoteletext settles this in REQ-RND-002 to REQ-RND-004 and REQ-RND-024.

**E.31 — Corrections made in adopting the TTI Form System.**
jellytext's TTI Form System version 2.0 is kept unchanged in `research/jellytext-tti-form-system-spec-v2.md`. Version 3, in Appendix C, is normative for neoteletext. It keeps the core ideas:
- the form is drawn as ordinary TTI rows;
- field definitions follow the row they belong to;
- unknown commands are ignored by older decoders;
- submission is by HTTP, followed by a success or error page.

It makes these corrections:

1. **Field positions are stated, not guessed.** Version 2.0 found fields by scanning rows for runs of underscores and brackets. That fails on real teletext for two reasons.
   - The underscore (5/F) and square brackets (5/B and 5/D) are **national option positions**. With the English option, which is the SAA5050 default, 5/F is drawn as "#" and 5/B and 5/D as arrows. So `_____` appears as `#####` and `[ ]` as `← →`. The Greek set likewise replaces "<" and ">" with guillemets.
   - A run of underscores inside ordinary text is indistinguishable from a field.

   In version 3, every field definition states its column and width. The drawn placeholder characters are purely visual, and the parser never relies on them. Authoring tools may still find fields by pattern and write the columns in.
2. **No credentials in pages.** Version 2.0's `FH` command could put headers such as `Authorization: Bearer …` into page content that anyone can download. Version 3 withdraws `FH`. Authentication comes only from the session cookie, which the server sets and the client returns.
3. **Protection against forged submissions.** A new `FT` command carries a server-issued form token, bound to the session. The client returns it with the submission (REQ-WEB-010).
4. **An input method.** Version 2.0 assumed a keyboard. A teletext viewer may only have a remote control: digits, four colour keys and a few others. Version 3 defines how fields are edited from both a keyboard and a remote control (REQ-NAV-009).
5. **Characters the page can show.** A value entered in a field is drawn in the page's own G0 character set. Characters the page cannot show are refused at entry.
6. **Where a form may submit to.** A form may submit only to the same origin (scheme, host and port) as the page that contains it, unless the service descriptor lists other permitted origins (REQ-WEB-008).

**E.32 — Security overrides every other requirement; plain HTTP is allowed.**
Security is the overriding requirement of this project. Every requirement in area SEC takes precedence over any requirement it conflicts with. Any such conflict is resolved in favour of security and recorded as a decision in this part.

The project owner decided on 1 October 2026 that **plain HTTP is allowed in full**. Plain HTTP may be used for reading pages, and also for cookies, sessions and form submissions. This is the only place where a security recommendation was deliberately not followed, so the residual risk is recorded here:

- **What an attacker on the network path can do.** They can read and change pages, session cookies and submitted form values on plain-HTTP services.
- **How the risk is reduced:**
  - the receiver-owned security indicator (REQ-SEC-005);
  - a warning and confirmation before every form submission over plain HTTP (REQ-SEC-008);
  - `Secure` cookies never sent over plain HTTP;
  - no downgrade of a service first reached over HTTPS (REQ-SEC-007).
- **What remains.** The user is told, but not prevented. Services that need protection should use HTTPS.

**E.33 — How the software is sandboxed.**
Sandboxing is layered, and each layer assumes the one inside it might fail:

1. **No authority in the library.** The library crates have no access to files, network, processes, environment or clock (REQ-SEC-016). Even a logic bug in the decoder cannot open a file or a socket, because the code to do so does not exist in it.
2. **No executable content.** Pages, forms, descriptors and pushed messages are data. Nothing received is ever run, interpreted as a script or loaded as code (REQ-SEC-004).
3. **Memory safety** inside the process: `forbid(unsafe_code)`, freedom from panics, Miri and fuzzing (REQ-GEN-006 and REQ-GEN-008).
4. **Bounded resources:**
   - fixed memory;
   - bounded work per packet and per message;
   - a fair share of processing time for each input source (REQ-SEC-012).
5. **An operating-system sandbox** for the command-line program and hosted ports. They restrict themselves after opening their inputs and outputs (REQ-SEC-017).
6. **Privilege separation for web mode.** A fetcher process holds the network rights, and a decoder process holds none. They are joined only by a stream of T42 or TTI bytes, which every other part of the design already uses (REQ-SEC-018).
7. **WebAssembly as a portable sandbox** for any host, including C programs, as an alternative to the unsafe C interface (REQ-SEC-019).
8. **Hardware isolation** on microcontrollers with a memory protection unit, as guidance for ports (REQ-SEC-021).
9. **Sandboxed test tools.** Third-party tools that parse untrusted corpus files run with no network access and read-only inputs (REQ-SEC-022).

**E.34 — A deliberately minimal browser.**
With web-native sources, neoteletext is a browser for a teletext web. Like any browser, it fetches, caches, keeps cookies, follows links, submits forms, applies an origin security model, sandboxes and renders. It can stay very small because teletext removes almost everything that makes browsers large:
- no layout engine (the page is a fixed 40 by 25 grid);
- no scripting (REQ-SEC-004);
- no font shaping;
- no images or media;
- pages of about one kilobyte.

Three earlier systems teach how to keep it that way:

- **Viewdata, Prestel and Minitel** were teletext's interactive relatives. They had character-cell pages, navigation and forms over a network on cheap terminals. They proved the product shape at national scale without anything resembling a modern browser.
- **Gemini**, a modern minimal alternative to the web, writes down what it refuses (no scripting, no cookies, no complex markup) so that features cannot creep back in.
- **WAP and WML**, the early mobile web, show the opposite. It grew scripting and complex page decks, became half a browser, and failed under its own complexity.

The decisions taken are:
1. **One written profile.** neoteletext's web behaviour is defined as one written "minimal user agent" profile (REQ-WEB-014). It lists what is supported and, as firmly, what is permanently excluded.
2. **One security unit.** The origin is the single security unit, defined once (REQ-WEB-015), as browsers do.
3. **A strict URL parser.** URL parsing, a classic source of browser bugs, is done by one small, strict, fuzzed parser (REQ-WEB-016). It prefers rejecting unusual addresses to being compatible with every form the web allows.
4. **History.** Back and forward history (REQ-NAV-010) is the one basic browser function the teletext model lacked.

*Amended 12 October 2026 by decision E.73:* the profile handles the `Link` header, and scans the first 8,192 bytes of an HTML answer for pointers, during discovery only (REQ-WEB-028, REQ-WEB-029).

**E.35 — Cookies follow current practice, not the 2011 standard.**
The cookie specification of 2011, RFC 6265, is out of date. Its replacement, RFC 6265bis, remains an IETF draft; the latest version checked is dated 30 September 2026. The major browsers already implement it. Newer practice has been added on top since:
- the `__Http-` and `__Host-Http-` name prefixes, shipped in Chrome 140 and Firefox 143 in 2025;
- Device Bound Session Credentials, a W3C design generally available in Chrome on Windows from 2026, which binds a session to a key that cannot leave the device, so stolen cookies quickly become useless;
- partitioned cookies ("CHIPS").

Third-party cookies remain enabled by default in Chrome after Google reversed its plan to remove them in April 2025. Safari, Firefox and Brave block them by default.

neoteletext adopts:
- **RFC 6265bis in full**, as REQ-WEB-006 lists: size limits, the 400-day cap, rejection of control characters, name prefixes, SameSite with `Lax` as the default, and the rule that plain HTTP cannot touch `Secure` cookies.
- **The new prefixes.**
- **Rules stricter than any browser** wherever its model allows: host-only cookies, and no third-party context at all.
- **Device Bound Session Credentials as a Phase 2 requirement** (REQ-WEB-017), where the port can keep a key that cannot leave the device.

Because RFC 6265bis is still a draft, REQ-WEB-006 shall be checked against the published RFC when it appears, and at each milestone's security review (REQ-SEC-023).

**E.36 — Links are Fastext: four coloured links per page, drawn by the receiver.**
neoteletext's only hyperlinks are the Fastext coloured keys.
- **Up to four links.** Each page may carry at most four links, one per coloured key: red, green, yellow and cyan.
- **Index.** The standard Index key is available as well. It goes to the service's index page, and is not a content link.
- **Typing a page number** remains ordinary teletext navigation, not a hyperlink.

No other link mechanism exists. The alternatives that were considered and rejected are:
- links marked inside the page text;
- link markers drawn by the page, such as `>>`, `[]` or `LINK<>`;
- whole-row links;
- numbered link hints;
- further sets of links beyond four.

*Reason:*
- **Familiar.** Fastext is the link model teletext viewers already know.
- **Remote-friendly.** It needs only the coloured keys, with no cursor and no mode.
- **Compatible.** It is the link model a real Fastext television of the late 1980s supports, so a page broadcast through the page encoder keeps working links on old sets (REQ-INP-019).
- **A useful limit.** Four links keep pages simple.
- **Trustworthy:**
  - **Markers.** Any marker a page draws, a malicious page can draw where there is no link. Bracket characters are also national option positions, which show as arrows or accented letters in some languages (decision E.31).
  - **Prompts.** Instead, **the receiver draws row 24 itself** from the link data, as Table Of Pages decoders drew their own prompt row. The page supplies only targets and short labels. Each prompt is therefore always shown whole, in a fixed style the page cannot alter, and links that leave the service are always marked as such.
  - **Destinations.** The first use of a link to another service needs confirmation.

Pages received from broadcasts keep their transmitted row 24 (decision E.23). Their links can only lead to pages in the same service, so the destination is always fully determined by the packet X/27/0.

*Amended 12 October 2026 by decision E.64:* links may also lead to addresses, and a page may link page numbers (`LK`) and runs of text (`LT`) as well as its coloured keys. The four coloured keys, the receiver-drawn row 24 and the ban on page-drawn link markers are unchanged. In the browser, a link to another service goes through the tuning screen (decision E.65) instead of the confirmation in row 24 of REQ-NAV-012.
*Note of 18 October 2026:* decision E.64 was withdrawn when the browser was removed from this repository (decision E.76). The tuning screen went with it, and REQ-NAV-012's confirmation in row 24 applies to every link to another service.

**E.37 — Terminal output: exact pixels where the terminal can show them, characters everywhere else.**
A terminal can show a teletext page in two ways:
- **As characters**, drawn by the terminal's own font with colour escape sequences (REQ-RND-009).
- **As pixels**, rendered by the library and sent through a terminal graphics protocol (REQ-RND-029).

Character output cannot be exact:
- the shapes depend on the user's font, so SAA5050 rounding (decision E.13) is lost;
- the separated mosaic characters (REQ-CHR-005) were added only in Unicode 16 and are missing from most fonts;
- few terminals support the double-height line escape sequences;
- many terminals ignore the blink attribute.

The pixel output of the row renderer (REQ-RND-017) is exact by construction, and the same on every port (REQ-RND-015).

The receiver therefore supports both, as separate output adapters (`ARCHITECTURE.md` layer L5). When the terminal offers one, a graphics protocol is used; otherwise the characters are used. The user can always force either. Three protocols are supported, in this order of preference:

1. **The kitty graphics protocol**, supported by kitty, Ghostty, WezTerm and Konsole. It accepts compressed images, places them by terminal cell, and keeps transmitted images so that the terminal can switch between them. Both flash phases can therefore be sent once and swapped.
2. **Sixel**, supported by xterm, foot, WezTerm, Windows Terminal, iTerm2 and mlterm. It has the widest support. Its colours are numbered registers, which match the palette indices of REQ-RND-023 directly.
3. **The iTerm2 inline image protocol**, supported by iTerm2, WezTerm and Visual Studio Code. It is a fallback for terminals with neither of the others.

The alternative of shipping a teletext font for the terminal to use was rejected:
- it looks right only where the user has installed it;
- it still cannot show double height or exact rounding;
- glyphs in the Private Use Area would tie the text output to that one font.

*Reason:*
- **No new capability in the core.** Both forms are callers of the existing outputs: the cell grid for characters, and the row renderer for pixels. The core does not change, which confirms the boundary of decision E.29.
- **Each form keeps its strength.** The pixel form is the faithful one. The character form works everywhere: over a serial line, in a terminal multiplexer, and as text that can be copied and searched.

**E.38 — Dependencies are judged by the microcontroller, not by the desktop.**
Running on microcontrollers is a critical requirement (REQ-GEN-002 and REQ-GEN-015). Every crate that runs there, which means every crate except the command-line program, the image file writers, `xtask` and the C interface, is therefore held to the rules of the smallest target, and so is everything it depends on.

The project's own safeguards stop at its own source:
- the panic lints (REQ-GEN-006);
- the bans on floating point and atomic operations (REQ-GEN-016);
- fuzzing (REQ-TST-040).

A dependency can contain any of what they forbid, and each one adds to the 64 kilobyte flash budget of REQ-GEN-009.

Eight candidate crates were tested on 10 October 2026. Each was built alone as `no_std` without allocation for `thumbv6m-none-eabi`, `riscv32imc-unknown-none-elf` and `xtensa-esp32-none-elf`. All eight built:
- `heapless` 0.9.3;
- `bitflags` 2.13.2;
- `httparse` 1.10.1;
- `embedded-io` 0.7.1;
- `base64` 0.23.1;
- `sha1` 0.11.0;
- `serde-json-core` 0.7.0, with `serde`;
- `embedded-websocket` 0.9.5.

Building proves little, though. A first attempt to detect panic and floating-point code by reading the symbols of the unlinked libraries gave the same result for every crate, including one with no code, so it measured nothing. The reliable test is the linked image, with unused code removed (REQ-GEN-019).

The decisions are:
- **The core, the format readers and the slicer have no dependencies.** The fixed-capacity collections and bit flags the core needs are a few hundred lines in the Foundation layer, written and tested under the project's own lints.
- **The web module uses only `embedded-io` and `httparse`.**
  - `embedded-io` is a set of traits with no code, and it is how a port connects its own network stack (REQ-WEB-001).
  - `httparse` is mature, small and has no dependencies of its own.
- **The project writes its own WebSocket framing, SHA-1, base64 and descriptor reader.**
  - `embedded-websocket` brings in five further crates, including copies of the other three.
  - `serde-json-core` brings serde's generated code into the flash image to read one small document (REQ-WEB-002).
  - Each replacement is short, bounded, and testable against the published vectors of its standard: RFC 6455 for WebSocket, RFC 3174 for SHA-1, RFC 4648 for base64 and RFC 8259 for JSON.
- **Crates that never run on a microcontroller are judged normally.** These are crates such as `clap`, `png`, `ratatui` and `crossterm`.

*Reason:* A dependency is code the project ships to a microcontroller but cannot lint, fuzz or budget. On the smallest targets the cost of writing a small, well-specified piece is lower than the cost of trusting it.

*Amended 11 October 2026:* `httparse` is no longer used. `embedded-io` is now the only crate from outside the project that a microcontroller crate depends on.
- **Why.** The symbol check of the linked image (REQ-GEN-019) found two panic paths that `httparse` 1.10.1 brings into every web build: an `unreachable!()` in its SWAR scanner and a range index of a header value. No panic-free alternative exists. `ntex-httparse` inherits the same code, `reqwless` and `edge-http` are built on `httparse`, and `thhp` is unmaintained and full of unsafe code. This is the case the reason above describes.
- **What replaces it.** On the owner's decision, the web module parses response heads with its own parser (`http::head`, RFC 9112 sections 2 to 5). It links no panic path, and is tested, fuzzed and compared with `httparse` on generated heads.
- **Behaviour.** It accepts and rejects what `httparse` did, with one exception: a version `HTTP/x.y` other than 1.0 and 1.1 is now reported as an unsupported version rather than as a malformed head. Where RFC 9112 leaves the recipient a choice, it keeps the choices `httparse` made:
  - a bare LF ends a line (RFC 9112 section 2.2);
  - empty lines before the status line are skipped (section 2.2);
  - the reason phrase may be missing together with the space before it (section 4).

  Obsolete line folding is rejected (section 5.2, REQ-SEC-011).

**E.39 — Details of the web protocols that the requirements leave open.**
Decided while implementing `neoteletext-web` on 10 October 2026:

1. **Descriptor members** (REQ-WEB-002). The JSON members are:
   - `name`;
   - `initial_page`, a string such as `"100"`;
   - `page_template`;
   - `push`, an object with `address` and `method` (`"websocket"` or `"sse"`);
   - `form_origins`;
   - `region`, 0 to 15;
   - `services`, a list of objects with `name`, `display_name` and `descriptor`.

   Unknown or repeated members make the descriptor invalid. Addresses may be relative to the descriptor's own address. Template placeholders may appear only in the path and query, never in the host.
2. **Subscriptions** (REQ-WEB-005). Over WebSocket the client sends `subscribe all` or `subscribe 100 1A0`. For server-sent events the push address gets the query parameter `subscribe=all` or `subscribe=100,1A0`.
3. **`{sub}` without a sub-page** expands to `0000`.
4. **Server-sent event connections send `Accept: text/event-stream`.** REQ-WEB-014 is amended to permit it.
5. **The server-sent events `retry` field is ignored**, because REQ-WEB-005 fixes the reconnection delays.
6. **A response body ended by the connection closing is accepted**, as RFC 9112 section 6.3 allows.
7. **Retries.** A GET is retried once if a kept-alive connection closes before answering. A POST is never retried.
8. **`Expires` cookies.** Until the port supplies calendar time, a cookie with only `Expires` is kept as a session cookie, and session cookies are never saved (REQ-SEC-013).
9. **IPv6 hosts are compared exactly as written**, so `[::1]` and `[0:0:0:0:0:0:0:1]` are different origins. This is the stricter reading of REQ-WEB-015.

*Reason:* Each is the simplest choice consistent with the requirements and the relevant RFCs, and REQ-DOC-004 requires behaviour the requirements leave open to be recorded here.

**E.40 — Presentation Levels 2.5 and 3.5 are in scope, and the presentation level is a setting.**
On 10 October 2026 the project owner asked for complete Presentation Levels 2.5 and 3.5, with comprehensive tests, as Phase 1 work. The receiver therefore implements Levels 1.0, 1.5, 2.5 and 3.5.

- **One setting.** The presentation level is a configuration value with four values: Level 1, Level 1.5, Level 2.5 and Level 3.5. It replaces the earlier on and off switch for Level 1.5 (REQ-L15-001 and REQ-PRF-006).
- **Profile defaults.** The default profile ("EN 300 706") presents at Level 3.5. The "SAA5050" profile presents at Level 1.0, and the "SAA5246A" profile at Level 1.5, as those chips did.
- **Command line.** `neottx --level 1|1.5|2.5|3.5` overrides the level of the profile. The former `--level1-only` option is removed in favour of `--level 1` (REQ-CLI-007).
- **What each level reads.** At Level 1 the page is decoded from its Level 1 bytes alone. At Level 1.5 the Level 1.5 overwrite pass runs (decision E.1). At Levels 2.5 and 3.5 the enhancement processor of decision E.42 runs instead of the Level 1.5 pass. The features that Tables 1 and 25 tick for Level 3.5 only are used only when the level is 3.5: packet X/27/5, the DRCS colour tables of X/28/1 and M/29/1, packets X/28/4 and M/29/4, the Font Style triplet, DRCS modes 1 to 3, and Modified G0 and G2 designation beyond two pairs.

*Reason:* EN 300 706 clause 12 defines the levels as cumulative, so one ordered setting describes a receiver exactly. Tables 1, 25, 28 and 29 say which packets and triplets each level acts on (`research/notes-level25-enhancement.md` sections 1.2 to 1.4). Presenting at the highest level by default shows broadcasts as their authors intended, while the chip profiles keep their period behaviour.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

*Note of 20 October 2026:* what this decision says of the Level 3.5 font styles (bold, italic and proportional spacing) and the Font Style triplet no longer applies; the triplet is not acted on (decision E.96).

**E.41 — Principles for reading Levels 2.5 and 3.5.**
The clauses for Levels 2.5 and 3.5 leave many details open. The research notes list them: `research/notes-level25-enhancement.md` section 12, `research/notes-level25-objects.md` section 14 and `research/notes-level25-drcs.md` section 15. Each is settled by one of decisions E.42 to E.56, and each of those decisions follows these principles:

1. **Normative text beats informative text, and tables beat prose.** Where a normative clause and an informative annex disagree, the normative clause is followed. Where a table and the prose that describes it disagree, the table is followed, as decisions E.16 and E.21 already do.
2. **Where the standard is silent, follow zvbi, or else the most literal reading.** zvbi 0.2.44 is the test oracle (REQ-TST-023), so its observable behaviour is followed where it has one. Otherwise the most literal reading of the standard is followed. Where zvbi contradicts the standard, the standard is followed. The known case is Table 30, CLUT 1 entries 5 and 6 (decision E.43).
3. **The receiver's status area is never written.** Row 0, columns 0 to 7, belong to the receiver at every level (REQ-SEC-005). Nothing from packets X/26, objects, side panels or row colours may write or colour them.
4. **Faults are contained at the smallest unit.** An uncorrectable or reserved triplet is skipped. An object that cannot be resolved is skipped. A missing enhancement page means that the page is shown without that enhancement. The Level 1 page is always shown.
5. **The limits of Annex A.8 are fixed resource bounds.** Beyond them, further invocations are ignored. Enhancement processing uses no allocation.
6. **Show at once, redraw when complete.** Rows are drawn as soon as the Level 1 page allows, and redrawn when enhancement data arrives or completes. The receiver never waits for enhancement pages, which may take up to 20 seconds to arrive (Annex B.6).

*Reason:* These are the owner's rulings of 10 October 2026. Principle 3 applies REQ-SEC-005, which overrides every other requirement (decision E.32). Principle 4 extends the approach that decision E.9 takes for Level 1.5. Principle 6 answers a question that the standard leaves open (`research/notes-level25-objects.md` section 14 item 30).

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.42 — How Levels 2.5 and 3.5 are layered, and what a cell holds.**
- **A further pass over the Level 1 grid.** Enhancement processing is a module of the presentation layer that runs after the Level 1 grid has been built. It processes, in this order:
  1. the page's local enhancement data, from its packets X/26;
  2. the objects that the local enhancement data invokes, whether Active, Adaptive or Passive, and whether local, public (from a Public Object Page) or global (from a Global Public Object Page);
  3. when the page has no local enhancement data that displays anything, the default objects of decision E.53.

  It writes both characters and attributes, and resolves contention between attributes as Annex C.2 describes (decision E.47).
- **Read-only access to other pages.** Object pages, DRCS pages and the Magazine Organization Table are read from the page store through a read-only lookup. The enhancement processor never decodes packets itself: it applies the packet layer's decoders to the stored bytes, and it never changes what is stored.
- **What a cell gains.** Each cell of the grid gains:
  - a size made of a vertical part (normal, top half or bottom half) and a horizontal part (normal, left half or right half);
  - underline;
  - invert;
  - a flash mode, with its rate and phase (decision E.48);
  - the Level 3.5 font styles: bold, italic and proportional;
  - glyph variants for G3 characters and for DRCS characters, the latter named by table (normal or global), sub-table and character;
  - its colours as Colour Map indices 0 to 31 (decision E.43).

  The colours of multi-colour DRCS characters are resolved at render time, from the DRCS colour lookup tables that travel with the grid (decision E.54).
- **Side panels** are held beside the 40-column grid, as decision E.49 describes.

*Reason:* Decision E.1 chose a separate pass so that Level 2.5 could be added later without redesign. Clause 13.3 describes local enhancement data as "a one-time usable Active Object", so one walk serves the local data and every object (`research/notes-level25-objects.md` section 2.3). Leaving the stored bytes untouched keeps acquisition and presentation apart (`ARCHITECTURE.md` section 8, rule 5).

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

*Note of 20 October 2026:* what this decision says of the Level 3.5 font styles (bold, italic and proportional spacing) and the Font Style triplet no longer applies; the triplet is not acted on (decision E.96).

**E.43 — The Colour Map and colour indices.**
- **One colour number everywhere.** Every colour in the grid is a 5-bit Colour Map index from 0 to 31, equal to 8 × CLUT + entry, where CLUT is the colour lookup table. This is how every colour field of Tables 28 and 29 is coded, and it is the numbering of Figure 28. The Level 1 colours are CLUT 0 entries 0 to 7, so existing values keep their meaning. Index 8, which is CLUT 1 entry 0, is transparent, and that is already the renderer's transparent index (REQ-RND-023).
- **The map.** The Colour Map has 32 entries of 12-bit colour, four bits for each primary. Its defaults are those of Table 30, verified on the page image of page 89. In particular, CLUT 1 entry 5 is 7, 0, 7 (half magenta) and CLUT 1 entry 6 is 0, 7, 7 (half cyan). zvbi 0.2.44 reports different values for those two entries, and the standard is followed (decision E.41, principle 2).
- **Which CLUTs can change.**
  - CLUTs 2 and 3 are redefined by X/28/0 format 1 or by M/29/0, at Levels 2.5 and 3.5.
  - CLUTs 0 and 1 are redefined by X/28/4 or by M/29/4, at Level 3.5 only. Table 10 is followed where clause 9.4.7 says the reverse.
  - At Level 3.5, 31 entries can be redefined. CLUT 1 entry 0 always stays transparent, and its twelve bits in X/28/4 and M/29/4 are ignored.
- **Bit significance.** In every multi-bit colour field the lowest-numbered bit is the least significant. This includes the default screen colour and default row colour of Table 4, triplet 13.
- **Default black.** "Black" as a default screen or row colour means Colour Map entry 0 (Table 25), which is black unless it is redefined at Level 3.5.
- **Re-mapping and substitution.** The Colour Table Re-mapping of Table 4 (triplet 13, bits 16 to 18) applies to the Level 1 spacing colour attributes, and also to the start-of-row default colours (white foreground and black background), unless Black Background Colour Substitution replaces the background.
- **Numbering errors in the standard.** Figure 28 numbers the CLUTs 1 to 4; the numbering 0 to 3 of Table 30 is used. Where the text of a row of Table 8 contradicts the row's heading, the heading is followed.

*Reason:* EN 300 706 Tables 4, 8, 10, 25, 28, 29 and 30, and Figure 28 (`research/notes-level25-enhancement.md` sections 3.2 and 7.1 to 7.4, and ambiguities A-13, A-14 and A-16 to A-20; `research/notes-level25-drcs.md` section 13.2 for the zvbi values). One index space means that the grid, the renderer and the DRCS colour tables (decision E.54) all use the same colour numbers.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.44 — Page-level presentation data: format, precedence and errors.**
- **Format 1 for display.** Packet X/28/0 is read as format 1 on every page that is displayed. Format 2 (Table 7) is never used for display.
- **Precedence.** Side panels, the default screen and row colours, Black Background Colour Substitution, Colour Table Re-mapping and the colour tables are taken from the page before the magazine, and from the magazine before the defaults. The page's own X/28/0 format 1 and X/28/4 therefore beat the magazine's M/29/0 and M/29/4. Within the page, X/28/0 beats X/28/4, and within the magazine M/29/0 beats M/29/4, for every field except the colour tables, where each packet defines different CLUTs.
- **Character set designation** follows Annex C.4 rule 1: X/28/0 format 1, then X/28/4 (Level 3.5 only), then M/29/0, then M/29/4 (Level 3.5 only), then the defaults (decision E.4, as amended).
- **DRCS colour tables** come from the page's X/28/1, then from the magazine's M/29/1, then from the defaults of Annex D.2.2 (decision E.54).
- **Errors.** An X/28 or M/29 packet in which any triplet of a field group is uncorrectable is treated as absent for that field group. It does not take precedence, and the next source in order applies. The field groups are:
  - the page function and page coding;
  - the default G0 and G2 designation and the second G0 designation;
  - the side-panel fields;
  - the sixteen colour words;
  - the default screen and row colours, substitution and re-mapping;
  - each DRCS colour table.

  Inside local enhancement data and objects, an uncorrectable triplet is skipped (decision E.9).
- **Mode identification in X/28/3.** The mode of each PTU is read as decision E.54 describes.

*Reason:* Clauses 9.4.2 to 9.5.3, Tables 4 and 7 to 13, Annex C.4 and Table 1 note 1 (`research/notes-level25-enhancement.md` sections 7.2 and 9.1 to 9.10, and ambiguities A-15 and A-21 to A-23). The standard states that the page overrides the magazine for each pair of packets. Extending that to the pair X/28/4 and M/29/0 is the reading that lets the page's own data win, as it does everywhere else. Treating a damaged field group as absent stops a corrupted packet from overriding good magazine data.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.45 — Screen colour, row colours and borders.**
- **Screen colour.** Highest first: a Full Screen Colour triplet; the page's default screen colour; the magazine's default screen colour; Colour Map entry 0 (Annex A.5).
- **Row colour.** Highest first:
  1. Full Row Colour triplets, applied in transmission order;
  2. the screen colour, but only when an explicit Full Screen Colour triplet set it;
  3. the page's default row colour;
  4. the magazine's default row colour;
  5. Colour Map entry 0.

  A default screen colour from X/28 or M/29 does not replace the default row colour.
- **Full Row Colour.** Commands apply in transmission order. "This row only" (S1 S0 = 00) changes only its own row. "Down to the bottom" (S1 S0 = 11) colours its row and every row below it. An earlier "down to the bottom" fill continues below a later "this row only" row.
- **Reserved row selections.** Address Display Row 0 and Full Row Colour with S1 S0 = 01 or 10 set the active position but set no colour.
- **Borders.** The screen areas above row 0, below the last row, to the left and right of the rows, and outside the side panels all use the screen colour.
- **Row 0.** Row colours never colour row 0, columns 0 to 7, of the main page (decision E.41, principle 3). The side-panel area of row 0 shows the screen colour (decision E.49).

*Reason:* Annex A.5 and Table 4 (`research/notes-level25-enhancement.md` sections 4.1 to 4.4 and 7.6, and ambiguities A-1 to A-3, A-25 and A-30). Reading the phrase "the screen colour already invoked" of Annex A.5 as the Full Screen Colour triplet is the only reading under which the default row colour field of Table 4 can ever take effect.

**E.46 — The active position, triplet order and damaged triplets at Levels 2.5 and 3.5.**
- **The strict row rule.** At Levels 2.5 and 3.5 only Full Row Colour (mode 00001), Set Active Position (mode 00100) and Address Display Row 0 (mode 00111 with address 63) set the active row, as clause 12.3.2 requires. The Annex A.3 rule of decision E.3 stays the default at Level 1.5.
- **Set Active Position with a column of 40 or more** sets the row only, and leaves the column unchanged.
- **The column** is set by every column triplet except the reserved modes and the Programme Delivery Control mode 00110. This includes character modes whose data is below 2/0 (clause 12.3.4).
- **Out-of-order triplets.** A triplet that addresses a position earlier than the previous triplet from the same source is applied at the position it addresses. Annex A.4 permits ignoring it. zvbi instead writes the character at the current position, so comparisons with zvbi use only streams that keep the order of Annex A.4 (REQ-TST-023).
- **Termination.** Any triplet with address 63 and mode 11111 ends the data it is in. Its type bits are only hints. An Object Definition triplet met in the local enhancement data also ends the local enhancement data.
- **Origin Modifier.** An Origin Modifier that is not immediately followed by an object invocation is discarded.
- **Damaged triplets.** An uncorrectable triplet is skipped, and does not move the active position (decision E.9).

*Reason:* Clause 12.3.2, Annexes A.3 and A.4, and Table 28 (`research/notes-level25-enhancement.md` sections 2.3, 4.3, 4.6 and 4.11, and ambiguities A-9, A-23 and A-26; `research/notes-level25-objects.md` section 14 items 13, 25, 26 and 31). Object Invocation and Object Definition triplets carry object numbers in their address field, so the Annex A.3 rule would move the row at every invocation. It cannot be used at these levels.

**E.47 — Attribute contention and the display attributes.**
- **Attribute types.** Foreground colour, background colour and flash are attribute types, as Annex A.7.1 lists them. Each bit of the Display Attributes triplet (double height, double width, boxing or window, conceal, invert, and underline or separated mosaics) is an attribute type of its own.
- **Contention.** When two attributes of one type contend for a cell, the latest wins, judged as Annex C.2 says: first by the cell where each takes effect, then by the cell where each is placed, then by the order of transmission.
- **A Level 1 attribute and an X/26 attribute at the same cell.** When a set-at Level 1 spacing attribute and an X/26 attribute of the same type are placed at the same cell, the X/26 attribute wins. This follows the priority of clause 13.2. Read literally, rule 3 of Annex C.2 would always let the Level 1 attribute win, because packets X/26 are sent before the rows (Annex B.2), and that would make enhancement attributes on attribute cells useless.
- **A 0 bit cancels.** A Display Attributes triplet sets all six functions at once. A bit at 0 cancels its function from that cell onward, including a function set by a Level 1 spacing attribute.
- **Conceal.** A Level 1 colour code ends only a Level 1 conceal. A non-spacing conceal lasts until a Display Attributes triplet clears it or the row ends.
- **Boxing.** At Levels 2.5 and 3.5, a Level 1 Start Box on a page that is neither a newsflash nor a subtitle page does not open a window; boxing keeps its Level 1 meaning (decision E.17). Windows come only from the boxing bit of an X/26 Display Attributes triplet, and from boxed pages (decision E.22).
- **Underline** is drawn on the last dot line of the cell, one line thick, on both fields in the 12 by 20 cell. The asymmetry of Table 26 notes 11 and 12 is followed: a non-spacing Underline or Separated attribute affects Level 1 characters, but the Level 1 codes 1/9 and 1/A do not affect enhancement characters. Underline is not drawn on G3 or DRCS characters, for which Table 29 leaves the response undefined.
- **Invert** swaps the cell's foreground and background colours. It is applied before flash and conceal, which then act on the colours displayed.

*Reason:* Annexes A.7.1, C.2 and E.13, clause 13.2, Table 26 notes 11 and 12, and Table 29 mode 01100 (`research/notes-level25-enhancement.md` sections 5.8 and 6.1 to 6.6, and ambiguities A-6 to A-8, A-28 and A-29). Applying invert before flash is the literal reading of Table 29: invert swaps the colours, and flash "affects the foreground colour only".

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.48 — Flash at Levels 2.5 and 3.5.**
- **The cycle.** One flash cycle lasts one second, and is divided into twelve units of one twelfth of a second, numbered 0 to 11.
  - **Slow flash (1 hertz)** is on during units 0 to 8 and off during units 9 to 11. That is the 3 to 1 ratio of decision E.12.
  - **Fast flash (2 hertz)** treats each half-second of six units alike. With phase p, which is 1, 2 or 3, the character is off during units 2(p − 1) and 2p − 1 of each half-second, and on otherwise.
  - **Incremental flash** gives successive characters, left to right from the triplet's column, the phases 1, 2, 3, 1, 2 and so on. **Decremental flash** gives them the phases 1, 3, 2, 1, 3 and so on.
- **Modes.**
  - Normal flash shows the background colour in the off phase.
  - Inverted flash swaps the on and off phases.
  - Adjacent-CLUT flash shows, in the off phase, the colour whose index is the cell's foreground index exclusive-ORed with 8. That is the same entry in the neighbouring CLUT: CLUT 0 with CLUT 1, and CLUT 2 with CLUT 3.
- **Level 1 Flash.** At Levels 2.5 and 3.5 the Level 1 Flash attribute 0/8 means slow flash in normal mode. At Levels 1 and 1.5, decision E.12 is unchanged.
- **Duration.** An Additional Flash Functions attribute lasts to the end of the row, or until another flash attribute: the Level 1 codes 0/8 and 0/9, or another Additional Flash Functions triplet.
- **Multi-colour DRCS characters.** In the off phase of normal or inverted flash, a multi-colour DRCS character shows a space in the background colour (clause 14.3.2). Adjacent-CLUT flash maps the index of each of its pixels by exclusive-OR with 8.

*Reason:* Table 29 mode 00111 gives the two rates and names the modes, but gives no duty cycle and no meaning for the phases (`research/notes-level25-enhancement.md` section 5.7, and ambiguities A-4 and A-5; `research/notes-level25-drcs.md` section 6.3 and ambiguity A-9). Twelve units divide both rates and the three phases evenly, and the slow rate keeps the ratio that decision E.12 measured on the SAA5050.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.49 — Side panels.**
- **Storage.** Side panels are held beside the 40-column grid, in a panel area addressed by columns 40 to 71 and rows 1 to 24 (clause 12.5.3). The right panel occupies columns 40 to 40 + right width − 1, and the left panel columns 72 − left width to 71.
- **Widths.** The widths follow from the left and right enable bits and from the number n in bits 7 to 10 of triplet 2 of Table 4:

  | Left panel | Right panel | Left width | Right width |
  |---|---|---|---|
  | Off | Off | 0 | 0 |
  | On | Off | n, or 16 when n is 0 | 0 |
  | Off | On | 0 | 16 − n, or 16 when n is 0 |
  | On | On | n | 16 − n, so that n = 0 gives a left width of 0 and a right width of 16 |

- **Status flag.** At Level 2.5 side panels are shown only when the Side Panel Status Flag is 1. At Level 3.5 they are shown whenever they are enabled.
- **Sources.** Side-panel data is taken as decision E.44 describes. When no X/28 or M/29 packet gives it, the fallback bits of the page's POP link in the Magazine Organization Table apply (decision E.53). With no POP link, there are no side panels.
- **Display width.** The display is 40 columns plus the left and right widths, and so not always 56. The renderer composes the left panel, the main page and the right panel, in that order.
- **Content.** Side-panel content comes only from objects placed by Origin Modifiers into columns 40 to 71, in rows 1 to 24.
  - Each panel row starts from the start-of-row defaults of clause 12.5.4 rule 1, with the full row colour as its background and a space in every cell.
  - Attributes do not pass between a panel and the main page, in either direction (rule 2).
  - Writes outside the enabled panel columns, to row 0, below row 24 or beyond column 71 are discarded.
  - An object element that would cross between the main page and a panel is discarded.
- **Row 0** has no transmitted panel content. Its panel area shows the screen colour.

*Reason:* Clause 12.5, and Tables 4 and 21 (`research/notes-level25-enhancement.md` section 8, and ambiguities A-10, A-11 and A-30; `research/notes-level25-objects.md` section 2.9, and section 14 items 12 and 38). The width table is the literal reading of "16 minus this value" and "0 means 16 columns".

**E.50 — Character repertoire, font styles and G3 drawing.**
- **National option at Levels 2.5 and 3.5.** The national option comes from bits 8 to 10 of the highest-priority X/28 or M/29 designation when one is present (Annex E.9), and from the header bits C12 to C14 otherwise (decision E.4, as amended).
- **Modified G0 and G2 designation.** At Level 2.5, designations beyond two G0 and G2 pairs on a page, counting the default pair, are ignored. At Level 3.5 there is no limit.
- **G1 characters written by triplets.** A G1 character written by an X/26 triplet is a mosaic, drawn contiguous or separated according to the non-spacing Underline or Separated attribute. Its codes in columns 4 and 5 are the characters of the current G0 set, without national option (Table 47 note 3).
- **G3 and hold.** A G3 character written by X/26 does not change the held mosaic, and Hold Mosaics never replaces a character written by X/26 (decision E.5 (d)).
- **G3 drawing.** The smoothed mosaics and line-drawing characters of Table 48 are drawn from the project's own geometry: the sextant geometry of Unicode document L2/19-025, as polygons on halves and thirds of the cell.
  - A generator in `xtask` rasterises them into replaceable glyph data for the 12 by 10 and 12 by 20 cells, in the same way as the Bedstead glyphs are exported.
  - Points at one third and two thirds of the cell height are rounded down to whole lines.
  - Light strokes are 2 half-dots wide and heavy strokes 4.
  - Position 2/F, "alternate dots", is a single-dot checkerboard on the 12-dot grid, as the words of Table 48 say, and not the 2 by 1 pattern of its figure.
- **Undefined codes.** G3 positions 6/E, 6/F, 7/E and 7/F, the undefined Latin G2 positions, and invalid DRCS data show a space, and a diagnostic is traced.
- **Font styles (Level 3.5).**
  - **Bold** is the glyph combined by OR with itself shifted one half-dot to the right.
  - **Italic** shifts each glyph line to the right by the number of lines below it in the cell divided by 4, in whole half-dots, rounded down.
  - **Proportional spacing** is recorded in the cell but drawn at the fixed pitch. The standard defines no spacing algorithm, Annex G.2 leaves appearance to the decoder, and zvbi only sets a flag.
  - Styles apply to G0 and G2 characters only (Table 25), never to G1, G3 or DRCS characters.
  - A Font Style triplet acts from its column to the end of its row. Its row count R extends the style, from the same column to the end of the row, over each of the next R rows. A later Font Style triplet replaces the style from its own position onward.

*Reason:* Clause 15, Tables 25, 29, 34, 47 and 48, and Annexes E.9 and G.2 (`research/notes-level25-drcs.md` sections 9 and 10, and ambiguities A-12 to A-16; `research/notes-level25-enhancement.md` sections 5.3 to 5.11). The geometry agreed with the standard's figure on 95 to 100 percent of pixels for every polygon glyph (`research/notes-level25-drcs.md` section 9.3). Drawing it from the project's own geometry keeps glyph provenance clean (decision E.25).

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

*Note of 20 October 2026:* what this decision says of the Level 3.5 font styles (bold, italic and proportional spacing) and the Font Style triplet no longer applies; the triplet is not acted on (decision E.96).

**E.51 — Objects: resolution, nesting and limits.**
- **Pointers.** A pointer in a Public Object Page or Global Public Object Page whose value is not a valid triplet address means that the object is absent. That covers the values 507 to 510, pointers into the pointer packets, and pointers to a triplet that is not an Object Definition. The value 0x1FF means "not used". Definitions in packets 1 and 2 of an object page cannot be reached by any pointer, and are ignored.
- **Checks.** An invocation whose definition has a different type, or whose usage bits exclude the presentation level, is skipped. The object number bits N8 to N0 of the definition are not compared with those of the invocation.
- **Local invocations** of triplet numbers 13 to 15, or of designation codes that were not received, are skipped.
- **Objects inside public objects** refer to the displayed page's POP and GPOP links.
- **Nesting.** The local enhancement data may invoke objects of every type. Local enhancement data and Active objects may invoke Adaptive and Passive objects; Adaptive objects may invoke Passive objects; Passive objects invoke nothing. Any other invocation is skipped. These rules bound the depth at three below the local enhancement data.
- **Coordinates.** Every coordinate inside an object is relative to its origin, including Address Display Row 0 and row address 40.
- **Writes out of range.** An element that would write outside rows 0 to 24 or columns 0 to 71 is discarded, and the object continues. So are writes to row 0, columns 0 to 7 (decision E.41, principle 3), and the side-panel writes that decision E.49 discards.
- **Adaptive objects.** Their attribute effects on a row end at the rightmost cell that any triplet of the object addresses on that row. The object is scanned before it runs, to find those cells.
- **Passive objects.** Each invocation starts from the default attributes of clause 13.5. Transparency inside them is CLUT 1 entry 0, as everywhere else.
- **Priority.** Characters follow clause 13.2: Passive objects over Adaptive objects, over local enhancement data, over Active objects, over the Level 1 page. Ties between attributes follow Annex C.2, with the order of transmission last.
- **Restricted triplets.** Full Screen Colour, Full Row Colour and Address Display Row 0 inside Adaptive or Passive objects are ignored.
- **Damage.** An uncorrectable triplet inside an object is skipped. In a pointer table, an uncorrectable triplet makes both of its pointers unused. When packets of an object page are missing, only the objects that need them are dropped.
- **Definitions after the marker "no Local Object definitions follow"** are still used if they are invoked.
- **Limits.** Annex A.8 is enforced as a fixed bound: at most 127 invocations of each object type per page, at most 15 objects open at once, and at most 15 objects opened in succession. Further invocations are ignored.

*Reason:* Clauses 10.5.1 and 13.1 to 13.8, Table 28, and Annexes A.4, A.7, A.8 and C.4 (`research/notes-level25-objects.md` sections 2 to 5, and section 14 items 6, 7, 9 to 17, 25 to 29, 32, 35, 36, 42 and 45). Each choice confines a fault to the object that has it (decision E.41, principle 4). The standard does not require the object number check, so it is not made.

**E.52 — Links to object and DRCS pages: the Magazine Organization Table and packets X/27/4 and X/27/5.**
- **Precedence.** The links in the page's own X/27/4, and in X/27/5 at Level 3.5, take precedence over the Magazine Organization Table, whatever the X/27/4,5 flag of the table's link says.
- **X/27/4 links 0 to 3** are taken by position, as Table 16 fixes them: GPOP, POP, GDRCS and DRCS. Their function bits are still checked, and a link whose bits do not match its position is ignored. A link that duplicates one already accepted is ignored. At Level 3.5, X/27/4 links 4 and 5 and X/27/5 links 0 and 1 are taken from their function and validity bits.
- **Format 1 marker.** A link is accepted only when bit 11 of its first triplet is 1 and bit 1 of its second triplet is 0, as Table 15 requires. The link function and the page validity are two bits each, as Table 15 shows, whatever the bit count of clause 9.6.2 says.
- **No link.** Page validity 00, all sixteen sub-page flags at 0, or page number FF means no link.
- **Sub-page flags** of X/27/4 are used only as hints for acquisition. Invocations name the sub-page directly by its S1 value.
- **Object pages are identified by the links that name them.** X/28/0 is not required on an object page.
- **Magazine of a table link.** The magazine of a link in the Magazine Organization Table is absolute, with 0 meaning magazine 8. This reading is checked against zvbi (REQ-TST-023), and any difference is recorded here.
- **Global flags.** The global object flag and the global DRCS flag of a page's entry are used only as hints for acquisition. A reference to the GPOP or GDRCS is resolved through the magazine's GPOP or GDRCS link whatever the flag says, as decision E.53 does for default objects.
- **Level 3.5 links.** A Level 3.5 decoder uses packets X/22 to X/24 of the table. When X/22 or X/24 is missing, it uses X/19 to X/21 and processes the page at Level 2.5 (clause 10.6.1). When only X/23 is missing, POP links 4 to 7 are taken from the Level 2.5 links in X/20.
- **Page xFF.** The table's byte pair for page xFF is ignored.
- **Sub-page counts.** A link's sub-page count n means that sub-pages 0 to n are assumed, and missing ones are absent. Object sub-pages beyond the advertised count are used if they are stored.
- **Enhancement pages are not text.** Object pages, DRCS pages and the Magazine Organization Table never receive Level 1 national option or display semantics, and their bits C12 and C13 mean "fragment" and "partial page" (Annex B.6). A Magazine Organization Table whose sub-page S1 is not 0 is ignored.

*Reason:* Clauses 9.6.2, 10.5.1.4, 10.5.2.2 and 10.6, Tables 15, 16 and 20 to 23, and Annexes A.1 and B.6 (`research/notes-level25-objects.md` sections 7 and 9, and section 14 items 1 to 5, 8, 22 to 24, 34, 40, 41 and 43).

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.53 — Default objects.**
- **Suppression.** Default objects are suppressed only by packets X/26 that carry display triplets. A page whose X/26 packets hold only Programme Delivery Control triplets keeps its default objects.
- **Magazine-wide default objects.** The default objects of the GPOP link apply to every page of the magazine that has no local display enhancement, whatever the page's global object flag says.
- **Order.** GPOP default object 1, GPOP default object 2, POP default object 1, POP default object 2, as Annex G.5 recommends. Where two of the same type write the same cell, the latest invoked wins (Annex A.7.1).
- **Origin.** Each default object is placed with its origin at row 0, column 0 (clause 13.8).
- **Fallback flags.** The fallback bits for side panels and Black Background Colour Substitution are read from the page's POP link. With no POP link they are clear, which means no side panels and no substitution.

*Reason:* Clause 13.8, Table 21, and Annexes E.11, E.12 and G.5 (`research/notes-level25-objects.md` sections 7.5, 7.6 and 8, and section 14 items 18 to 21 and 33).

**E.54 — DRCS pages, modes and colours.** Withdrawn on 20 October 2026: dynamically redefinable characters are no longer decoded (decision E.95).

**E.55 — The resolved colour mode of the renderer.**
- **Two colour modes.**
  - **The palette mode** of REQ-RND-023, with the indices 16 + 8f + b and 80 + 8f + b, stays as the mode for Levels 1 and 1.5. Ports with hardware palettes keep the benefit that flash and reveal need only a new palette.
  - **The resolved mode** resolves flash and conceal at render time, from a flash phase and the reveal setting, and gives Colour Map indices 0 to 31 directly.
- **When each is used.** The resolved mode is used at Levels 2.5 and 3.5. The palette mode is refused for any grid that uses a colour outside CLUT 0, or any flash mode of Level 2.5.
- **Change tracking.** The change tracker reports which rows hold flashing or concealed cells, so that a port redraws only those rows when the flash phase or the reveal setting changes.

*Reason:* With 32 colours and four flash modes, the palette encoding would need far more entries than a hardware palette holds. Resolving at render time keeps the pull model of decision E.29, and costs only the redrawing of the rows that flash or conceal.

**E.56 — Storing enhancement pages and magazine data.**
- **Stored as received.** Object pages, DRCS pages and the Magazine Organization Table are kept in the page store. Each reception of such a page replaces the stored bytes of the packets it carries, with no merge by parity (decision E.8 does not apply). This applies to the rows of:
  - a page whose X/28/0 page function is not "basic Level 1 page";
  - page xFE, the Magazine Organization Table;
  - a page that a stored Magazine Organization Table link, or a stored X/27/4 or X/27/5 link, names as an object or DRCS page (decision E.52).
- **Hexadecimal pages.** At Levels 2.5 and 3.5, pages with hexadecimal numbers are stored, because enhancement pages usually have them (Annex E.1). Navigation still skips them, unless the diagnostic setting of REQ-ACQ-013 is on.
- **Replacement.** Stored object pages are replaced by new receptions. Nothing is discarded when the Magazine Organization Table changes, beyond the store's normal eviction.
- **Magazine data.** Packets M/29/0, M/29/1 and M/29/4 are stored for each magazine. Each new reception replaces the stored packet. All stored M/29 packets are discarded when the service changes, meaning a new network identification in packet 8/30 or a new input. There is no time-out.

*Reason:* Annexes A.1, B.6, E.1 and E.8, and clause 10.6 (`research/notes-level25-objects.md` sections 6 and 11, and section 14 item 39; `research/notes-level25-enhancement.md` ambiguity A-24). Object pages carry Hamming 24/18 triplets, whose bytes do not follow the odd parity rule, so a merge by parity would keep stale bytes or reject good ones.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.57 — Points of the enhancement processor, objects and DRCS that decisions E.40 to E.56 leave open.**
Recorded on 11 October 2026, when the processor was written (REQ-DOC-004). Each point is commented where the code applies it.
- **Damaged triplets and the Origin Modifier.** A damaged triplet met while an Origin Modifier waits for its invocation discards the offsets.
- **DRCS invocation and data below 2/0.** The rule that a character triplet whose data is below 2/0 writes nothing does not apply to the DRCS Character Invocation, because global DRCS characters 0 to 31 have data below 2/0.
- **Counting invocations.** Every permitted invocation counts toward the limit of 127 per object type, whether or not its object is found, so that failed lookups also bound the work (REQ-OBJ-020).
- **Objects open at once (Annex A.8).** Counted at the invocation point against the open areas of all earlier objects. An object's area runs from its invocation to the last position any of its moving triplets reaches. The count of successive opens restarts at any character or attribute actually placed.
- **Tables without Level 3.5 links.** At Level 3.5, a stored Magazine Organization Table that lacks packet X/22 or X/24 makes the page fall back to Level 2.5 links, whatever X/27/4 says (clause 10.6.1).
- **Links role by role.** X/27/4 and X/27/5 take precedence over the table one role at a time; a role they do not give comes from the table.
- **Default objects** are found through the table's own GPOP and POP pages (clause 13.8). A table link to page FF gives no fallback bits and no default objects. A display triplet, which suppresses the default objects, is any character or attribute column triplet, screen or row colour, Address Display Row 0 or invocation met before the first Termination Marker or Object Definition (with decision E.53).
- **An object's area.** An object writes only into the area of its origin column: the main page or one side panel. Elements that fall outside it are discarded, which covers an object that would cross from the page into a panel.
- **Box codes at Levels 2.5 and 3.5.** On a page with neither C5 nor C6 set, both Level 1 box codes are ignored. On a boxed page, one boxed state is shared with the window bit of the Display Attributes triplet, and the later of the two wins.
- **Side-panel rows** start with the re-mapped white foreground (decision E.14) and the row colour as their background.
- **Overlapping Adaptive objects** keep one merged attribute record per cell, owned by the latest object to place there.
- **Passive objects** apply their attributes in transmission order, as the walk meets them; this is scan order for streams that keep the order of Annex A.4. A Passive attribute at a discarded position is dropped.
- **Incremental and decremental flash** count phases by column from the triplet's column, spaces included.
- **Which rows are rebuilt.** Row 0 only when C7 is clear, always keeping columns 0 to 7. Row 24 only when it is shown and not drawn by the receiver. A page with C10 set, or with an X/28/0 page function other than basic text, keeps its Level 1 rows and gets no side panels.
- **Enhancement pages shown as text (REQ-OBJ-041).** A page is taken as an enhancement page, and its rows 1 to 24 are blanked, when its number is xFE or its stored copy carries an X/28 page function of GPOP, POP, GDRCS, DRCS or MOT. A page named only by a link is still shown, so that a wrong link cannot hide a display page.
- **DRCS faults (REQ-DRC-009).** An uncorrectable X/28/3 mode code counts as missing data, not invalid data. An invalid character shows as a space and is counted. A missing character keeps its DRCS reference with no pattern, and is drawn empty. Patterns beyond the grid's capacity count as missing.
- **X/26/15** is always processed at Levels 2.5 and 3.5; the profile option of decision E.2 concerns Level 1.5 only.
- **Lower-row fill cells** of double height take the background of the cell above and keep its box membership; flash and invert are cleared.
- **Reporting.** `enhance::apply` returns an `EnhancementReport` that counts each kind of fault and the triplets read. The caller adds the fault counts to the decoder's statistics after building (REQ-L25-051, REQ-ARCH-007). A build is the page, its reception count and the level asked for, and the build recorded last is not counted again. A viewer builds the shown page on every redraw, and counting each redraw would make the counts measure how long the page stayed on screen; counting once per reception matches the acquisition counts, which count a damaged packet each time it is received. The command-line `list` command builds every page once to show the counts. With tracing on, the walk reports each transition of its page-level run as a trace entry of the enhancement walk machine (REQ-FSM-010); the runs for each row repeat the same transitions, so they are not traced. A transition that met a fault names the fault as its action. The Level 1.5 walk reports its transitions in the same way.

*Reason:* Clauses 12.3, 13 and 14, Annexes A and C, and the research notes' ambiguity lists (`research/notes-level25-enhancement.md`, `research/notes-level25-objects.md`, `research/notes-level25-drcs.md`). Each choice keeps the work bounded, keeps faults contained to the cell or object concerned (decision E.41), or takes the reading that never hides displayed text.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.58 — Points of rendering at Levels 2.5 and 3.5 that decisions E.47 to E.55 leave open.**
Recorded on 11 October 2026 (REQ-DOC-004).
- **Choosing the colour mode.** `RenderOptions::from_config` keeps the palette mode at every level, so Level 1 output is unchanged; `RenderOptions::for_level` chooses the resolved mode at Levels 2.5 and 3.5, and the command-line program uses it. The palette mode refuses a grid it cannot draw (`RenderError::NeedsResolvedMode`), and the 4-bit format refuses indices above 15 in the resolved mode.
- **Underline** is drawn on the last line of the cell, for G0 and G2 characters only, and on double height only on the bottom half, enlarged to two lines. It is not drawn on empty cells.
- **Italic** uses the position of the normal-height line, so a double-height italic character is the normal one enlarged.
- **Transparent pixels** of multi-colour DRCS characters show the plane below (the row colour or the picture, Table C.1), not the cell's background. A transparent colour in row 0, columns 0 to 7, shows the picture, never the row colour.
- **Mix mode** makes gaps the picture; a dot hidden by flash or conceal therefore also shows the picture.
- **Side panels.** Panel widths are clamped to 16 columns each. A grid with no panel storage but panels enabled shows the screen colour there. Borders are the caller's to fill, in the colour `border_index` gives.
- **Still images** (`neottx show` and `neottx render`) are drawn at flash unit 0, in the "on" phase of every rate.
- **G3 glyphs.** A pixel is set when its centre, nudged upwards by an infinitesimal amount, lies inside the shape, which makes mirrors and complements exact. 2/E and 3/E are drawn at half-dots 1–2 and 9–10; 4/8 to 4/B are straight diagonals; diagonal strokes are two half-dots wide; the checkerboard alternates per row of the 12×10 matrix. The generated glyphs are used at every level, so G3 characters at Level 1.5 also change from the temporary glyph source.
- **Eight-colour terminals** show each primary when it is above half intensity.

*Reason:* Table C.1, clauses 12.3 and 15, and the rendering requirements REQ-RND-030 to REQ-RND-041. Keeping the palette mode as the default of `from_config` keeps the golden rasters of Levels 1 and 1.5 unchanged (REQ-TST-030).

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

*Note of 20 October 2026:* what this decision says of the Level 3.5 font styles (bold, italic and proportional spacing) and the Font Style triplet no longer applies; the triplet is not acted on (decision E.96).

**E.59 — Points of TTI files and the authoring builder that REQ-INP-023 to REQ-INP-025 leave open.**
Recorded on 11 October 2026 (REQ-DOC-004).
- **The page function in TTI files.** A function carried by an `OL,28` line (X/28/0, X/28/3 or X/28/4) wins over a `PF` line, and the lowest designation code wins among them, whatever the order of the lines, so that a page read, written and read again is unchanged.
- **Building X/28/0 from `RE` and `PF`.** A function other than 0 gives the Table 6 form. A basic page with `RE`, or with a coding other than 0, gives the Table 4 form with every other field at its default, including the colours of Table 30. `PF,0,0` alone gives no packet, so that it does not override a magazine's M/29/0.
- **The Table 6 marker** "11111111100" of bits 8 to 18 is read bit 8 first.
- **Page codings 1, 4 and 5** are sent as parity text.
- **Page mFF** carries the magazine packets: it is sent as a time-filling header after its M/29 packets, and converting a stream to TTI puts each magazine's stored M/29 packets back on it.
- **Builder defaults.** Pages are sent with C4 set. Enhancement sub-codes follow Annex A.1, with the last packet sent in S4 and S3. DRCS pages pad unused PTUs with spaces and send X/28/3 unless told not to. A page with no local data sends no packet X/26 (clause 13.8: any X/26 cancels the default objects). Table entries for both levels repeat the Level 2.5 links in X/22 to X/24.
- **Converting an enhancement page with no X/28** writes rows that all pass parity as parity text; otherwise the page is taken to be a POP with coding 2.

*Reason:* Clauses 9.4, 10 and 13.8, Annexes A.1, B.2 and B.7, and the conventions of existing TTI files (vbit2 read as documentation only, decision E.31).

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.60 — zvbi as the oracle at Levels 2.5 and 3.5.**
zvbi is the oracle of REQ-TST-023, compared cell by cell by `tests/oracles/run_cells.sh`. Tracing every difference on the authored stream with a debug build of zvbi 0.2.44 and 0.2.45 showed these zvbi faults, which `tests/oracles/zvbi-known-differences.txt` names page by page:
- it does not store a sub-page when the next header in the magazine has the same page number (the test harness puts a time-filling header between them, which changes nothing for a decoder that follows clause 7.1.2);
- it reads the page number of an X/27/4 or X/27/5 link one bit too high (EN 300 706 Table 15), so no such link is followed;
- it starts a Local Object at its definition triplet and stops there, so Local Objects never draw;
- it drops all of a page's enhancement at its first fault, such as one invalid DRCS character, where this receiver contains the fault (decision E.41);
- it applies a Display Attributes size only to characters that enhancement data writes;
- it flashes for the reserved flash rates;
- it never invokes the default objects of the table's global link;
- it lets M/29/0 beat X/28/4 (against decision E.44);
- it takes page 1F0 to be the Basic TOP Table, so the authored stream puts its GPOP elsewhere;
- its default colours differ from Table 30 at CLUT 1 entries 5 and 6 (decision E.40), so colours are compared as indices.

Where zvbi follows a different reading from decisions E.45 (the full row colour behind substituted black backgrounds) and E.53 (X/26 with no displayed data), the page is listed with that reason. Authored pages 119 and 124 exercise DRCS and sizes without these faults, so zvbi can check them.

*Reason:* REQ-TST-023 requires an independent oracle; recording its faults keeps the comparison strict everywhere else, since a page not in the list must agree in every compared cell.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.61 — Compile-time features: a build contains only the parts it uses.**
Recorded on 11 October 2026 (REQ-DOC-004), on the owner's instruction that the code be organised so that only the parts a build needs are compiled in.

The linker removes code nothing calls, but not code reachable through a choice made at run time. Measured on 11 October 2026 with a linked image built with `opt-level = "z"` and link-time optimisation (`x86_64-unknown-none`), whose roots are the functions a port calls:
- a decoder that composes its pages through the configured level carries the Level 2.5 and 3.5 processor, about 27 kilobytes of code, even when it is configured for Level 1.5, which needs 1.7;
- format detection reaches every reader: about 40 kilobytes of code in a port that reads only T42, which needs 1;
- the renderer adds 6 kilobytes of code and the Bedstead glyphs 33 kilobytes of data;
- forms add 8 kilobytes of code and 6 of data.

Only compile-time features can remove these. The rules:
1. **Three tools.** Crates separate different dependencies or environments. Cargo features separate the optional areas within a crate. The configuration chooses behaviour among what is compiled.
2. **Features are additive and coarse**: one per area of Part F, never one per function. A feature never changes what another feature does.
3. **Data types and acquisition are never behind a feature.** Level 1.5 reads the character set designations of X/28/0 and M/29/0 (decision E.4), so acquisition stays whole at every level. Keeping types unconditional keeps the features additive. Only processing is optional.
4. **A configured level that is not compiled in** falls back to the highest compiled level below it (`PresentationLevel::available`).
5. **Each crate's default is every feature**, so that a plain dependency works. Library crates depend on one another with default features off and name what they use, so that Cargo's feature unification never brings in an area a port did not ask for.
6. **The gate builds and tests a fixed list of builds** from `xtask`, and measures the linked test image of REQ-GEN-009 and REQ-GEN-019.

The features of `neoteletext-core`, all on by default:

| Feature | Contents |
|---|---|
| `level15` | The Level 1.5 pass (area L15) |
| `level25` | The enhancement processor for Levels 2.5 and 3.5 (areas L25, OBJ and DRC) |
| `render` | The row renderer and the generated G3 glyphs (area RND) |
| `original` | The project's own glyph set (REQ-RND-028, decision E.62); removed by decision E.72, when `bedstead` becomes a default feature and part of the modern-classic profile |
| `bedstead` | The temporary Bedstead glyph set (decision E.25); not a default since decision E.62, kept for comparison |
| `viewer` | The viewer state machine, navigation and history (area NAV) |
| `forms` | The forms model (Appendix C) |

`compose_level`, which applies the level's pass, moves from the viewer to its own module `compose`, and `Zoom` from the renderer to the Level 1 grid, because a port without the viewer or the renderer still composes pages.

The features of `neoteletext-formats`, all on by default except `std`. T42 is always compiled. Detection recognises only the formats compiled in, and a format asked for by name that is not compiled in gives the diagnostic `FormatNotCompiled`:

| Feature | Contents |
|---|---|
| `transport` | Transport stream and PES readers (REQ-INP-006 to 009) |
| `sliced` | Linux sliced VBI (REQ-INP-010) |
| `pages` | The page encoder (REQ-INP-019) |
| `tti`, `ep1`, `edittf`, `dump` | One page-file reader each; each needs `pages` |
| `check` | The page checker (REQ-CLI-006); needs `tti` |
| `std` | The writers over `std::io`; the TTI writer and the authoring builder also need `tti` |

Detection reading T42 only is 4.3 kilobytes of code, down from 46.8.

The features of `neoteletext-web`, all on by default. A request a build cannot serve gives an event or failure saying so: `Event::PushNotCompiledIn`, `FormRefused::NotCompiledIn`, `CookieRejected::NotCompiledIn`. The zxnet feed is not a feature: nothing in the user agent reaches it, so the linker already leaves it out.

| Feature | Contents | Code |
|---|---|---|
| `push` | WebSocket and server-sent events (REQ-WEB-005) | 12.3 kilobytes |
| `cookies` | The cookie store's use by the agent (REQ-WEB-006) | 7.6 kilobytes |
| `forms` | Form submission and its response (REQ-WEB-010, REQ-WEB-011) | 4.0 kilobytes |

The application layer of `neottx` is a library crate of its own, `neoteletext-app`: sessions, inputs, both processes of live mode, and sandboxing, which a windowed viewer will share. Its features are `live` (the web module, TLS and the fetcher process) and `slice` (the slicer). `neottx`'s features are `view` (the interactive viewer and its terminal library), `live` and `slice`. Every command stays defined, so the help text never changes, and a command whose area is left out exits with the usage status and says so. A `neottx` without `live` contains no TLS code and is about half the size. The fetcher stays a second run of the same program: a separate fetcher binary would leave TLS out of the decoder process too, but is deferred for its packaging cost, since the decoder's sandbox already keeps it off the network.

*Reason:* The microcontroller targets (REQ-GEN-015) and the flash budget of REQ-GEN-009.

**E.62 — How the original glyphs are drawn and accepted.**
Recorded on 11 October 2026 (REQ-DOC-004), on the owner's instruction to write the checks first and then start the glyphs of REQ-RND-028.

**What is drawn.** The required characters are generated, not chosen: every code point the character set tables produce for every G0 set, every national option and every G2 set, codes 2/0 to 7/F, plus the spacing forms of the diacritical marks that decision E.26 overlays. `cargo xtask glyph-inventory` writes them to `crates/neoteletext-core/src/glyphs/required.txt`. Block mosaics are computed (REQ-CHR-005), G3 is generated from geometry (REQ-RND-037), and DRCS characters come from the service, so none of them is drawn.

**How it is drawn.** The glyphs are kept in `crates/neoteletext-core/src/glyphs/original.txt`, one glyph per entry, drawn as nine rows of five characters. An entry is one of three kinds:
- drawn;
- a copy of another entry, for letters that share a shape across scripts, such as Latin A and Cyrillic А;
- a base letter with a spacing mark overlaid, as decision E.26 composes at run time.

The rules were written in words in `docs/glyph-style-guide.md` (removed with decision E.72; in the git history before commit 091741a), from first principles: the grid, cap height, x-height, descenders, stroke, marks. They are never written from the SAA5050 or Bedstead glyphs.

**Who draws.** The designer works from a design pack: the style guide, the required list and the file format. The designer never sees the SAA5050 datasheets, the Bedstead files, the generated Bedstead table, golden rasters or rendered samples.

**How the glyphs are accepted.** `cargo xtask glyph-check` checks:
1. **Format:** every entry parses, and no code point appears twice.
2. **Well-formedness:** no ink in the descender rows without the `descender` tag, no isolated dot without the `dots` tag, and no empty glyph except a space.
3. **Confusables:** no two characters of one set have the same glyph, and the listed confusable pairs differ by at least two dots.
4. **Independence:** every drawn glyph is compared with the Bedstead glyph of the same code point, the stand-in for the SAA5050. An exact match, or a difference of one dot, fails unless `crates/neoteletext-core/src/glyphs/original-audit.txt` gives the reason. Shapes such as a full stop or a hyphen have only one sensible form.
5. **Coverage:** reported always; required to be complete with `--complete`, which becomes part of the release check of REQ-RND-020.

Recognition by people fluent in each script, and proofs of real pages, are reviewed by hand.

*Amended 11 October 2026:* all 530 required characters are drawn (427 drawn, 45 copies, 58 overlays), `glyph-check --complete` passes, and the original set is the core's default; `bedstead` is no longer a default feature. The full gate runs `glyph-check --complete`, which also fails if the defaults include Bedstead. The review by readers of each script has not been done (IMPLEMENTATION-LOG.md row 54).

*Reason:* REQ-RND-020 and REQ-RND-028. Writing the checks before the glyphs gives the designer feedback while drawing, and the independence check makes the clean room something the gate measures rather than a promise.
*Superseded on 12 October 2026 by decision E.72:* Bedstead is the bundled font, and the original glyph set and its checks are removed.

**Decisions E.63 to E.70: the web-native browser.**
*Note of 18 October 2026:* the browser and the documents named below were removed from this repository and are kept in the separate neoteletext-browser repository (decision E.76). Decisions E.63 to E.69 are withdrawn; decision E.70 stays for the web module's channels.
These record the design of `docs/web-native-concept.md` of the neoteletext-browser repository (its decisions 1 to 14, settled on 11 October 2026: decisions 1 to 5 are the owner's, and 6 to 14 were recommended and accepted by the owner), the owner's decisions on the browser in `docs/desktop-browser.md` of the neoteletext-browser repository section 15, and the decisions taken while building the macOS browser on 12 October 2026, when the owner asked for the build to be completed without questions. The build's decisions are marked *(build)* below. They are listed with their reasons in `docs/macos-build-decisions.md` of the neoteletext-browser repository (D1 to D27) and await the owner's review (issue #162). The behaviour was built before these entries were written, against REQ-DOC-004; they were recorded on 12 October 2026 to restore the rule.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.63 — Web-native sites: what a site is, where its descriptor lives, and what its files may hold.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**E.64 — Links may lead to addresses; page numbers and runs of text may be links.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**E.65 — Leaving a site: the tuning screen, and row 0 on every page.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**E.66 — Page catching and showing links.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**E.67 — The browser's keys.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**E.68 — The browser program.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**E.69 — Confining the browser's helper processes.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**E.70 — A channel is a folder.**
Decided by the owner on 12 October 2026, who asked for solutions that are opinionated and easy to understand and to build. The details below were written to that instruction. This decision replaces items 1, 2 and 3 of decision E.63 and refines its items 4 and 6, and the browser's "site" becomes the "channel" throughout. The reasoning, the alternatives considered and the measurements behind it are in `docs/channels.md` of the neoteletext-browser repository.

1. **A channel is a folder on a web server.** Its address is an `http` or `https` address whose path ends in `/`, such as `https://example.org/` or `https://user.github.io/ceefax/`. There is no other kind of web-native site. One server may hold any number of channels, in different folders. A folder inside a channel is a different channel, and inherits nothing from the folder that contains it.
2. **A channel is identified by its origin and its folder.** Two addresses are in the same channel only when they have the same origin (REQ-WEB-015) and the same folder path, compared exactly, letter case included, since paths on the web are case-sensitive. The identity decides the tuning screen, the name in row 0, cookies, history, bookmarks, the confirmed channels of a session, where forms may submit, and which redirects are followed. The origin stays the unit for the protections that concern the server as a whole: the plain-HTTP marker, no downgrade, and the local-network checks.
3. **Folder paths are restricted, so that two spellings can never name one folder, or one spelling two.** Each segment of a channel's folder path is 1 to 64 characters from the unreserved set of RFC 3986 section 2.3 (ASCII letters, digits, `-`, `.`, `_` and `~`), and is not `.` or `..` or made only of dots. Percent-encoding is not allowed in a folder path at all. A folder has at most 8 segments and its path at most 256 characters. An address that breaks these rules is refused, never repaired. Hosting services' own naming fits inside this set: GitHub user and repository names, and `~user` folders.
4. **Every file of a channel has a fixed name, in the channel's folder.** There is no page address template for channels, and no setting for file names.

   | File | Holds | Required |
   |---|---|---|
   | `NNN.tti` or `NNN.t42` | The page `NNN`, written as three hexadecimal digits with `A` to `F` in capitals, such as `100.tti` or `1FE.t42` | Only the initial page |
   | `index.t42` | Any number of pages at once: a bundle, a capture or an archive | No |
   | `channel.json` | The channel's settings | No |
   | `contents.txt` | The channel's list of pages and sections | No |

5. **Opening a channel is three requests, in order.** First `channel.json`: a valid file gives the settings, 404 or 410 means the defaults, and any other answer is a failure that stops the channel opening. Then `index.t42`: its pages are stored, 404 or 410 means there is no bundle, and any other failure is reported and the channel opens without it. Then the initial page, from the store or from its own file. The defaults are: initial page 100, no push, no linked channels, no aliases, no refresh, the default character set region, and the name shown being the channel's host and folder.
6. **A page is found by one fixed rule.** A page in the store and confirmed current is shown from the store (REQ-WEB-004). Otherwise the browser asks for `NNN.tti`, and on 404 or 410 for `NNN.t42`, and on 404 or 410 again the page is "not found". The browser tries first the extension that last succeeded in that channel, which changes only the number of requests, never the result. A page's own file fills only pages that have been asked for, and enhancement pages (decision E.63 item 4).
7. **`channel.json` is the service descriptor of REQ-WEB-002, with three differences.** Every member is optional. `page_template` is not allowed in it, and a file that has one is refused, because a channel's files have fixed names. A linked channel is given by its folder's address in a member `channel`. The descriptor is no longer looked for at `/.well-known/teletext.json`: RFC 8615 reserves `.well-known` for the root of a server, which is what kept a site from living in a folder.
8. **`contents.txt` is the only map of a channel.** It is a list, one entry per line, of three kinds: a section heading, a page of the channel, and another channel. The receiver draws it as a teletext index. Nothing else is used to learn a channel's structure. The browser never lists folders, never probes page numbers and never crawls. A channel without `contents.txt` has a contents view listing the pages the tab has shown on the channel.
9. **The author's tool writes the contents.** `neottx contents FOLDER` reads the author's own folder of page files and writes a `contents.txt` for it. The only crawling is done by the owner, of their own files, before publishing.
10. **Static and dynamic servers are the same to the browser.** "Folder" is a rule about addresses, not about files on disk. A server program may generate any channel's files on request, and any number of channels from a database. The browser sees only addresses and responses.
11. **Media types are generic in practice, so the content decides.** No teletext media type is registered with IANA, and none is in `mime-db` or the common `mime.types` files. Measured on 12 October 2026, raw.githubusercontent.com serves `.tti` as `text/plain; charset=utf-8` and `.t42` as `application/octet-stream`, jsDelivr serves both as `application/octet-stream`, and NMS Ceefax's feed serves `.tti` as `text/plain`. The extensions are also used by other formats: `.tti` by Template Toolkit and `.t42` by PostScript Type 42 fonts. A channel's files are therefore accepted with this project's own types and with these generic types, and each file must then pass the check for the format its name says. Any other type, including `text/html`, is refused, which also catches hosts that answer a missing file with an HTML page and status 200.
12. **The channel is a boundary for everything a site was.**
    - **Cookies** are kept per channel: a cookie set by a response from inside the folder belongs to that channel, a `Path` outside the folder is narrowed to the folder, and the channel's cookies are sent only with requests inside the folder, with its push connection, and with its form submissions.
    - **Forms** may submit only to an address inside the channel's folder, or on an origin that `channel.json` lists in `form_origins`.
    - **The push connection** named in `channel.json` may be anywhere on the channel's origin, because servers usually serve push from a fixed path.
    - **Redirects** of the channel's own requests are followed only within the folder. A redirect from `http` to `https` keeps the channel when the host and folder are the same and both ports are the schemes' defaults. Any other redirect is another channel, and shows the tuning screen.
    - **Links** to a page file in the same folder stay in the channel. Every other address is another channel.
13. **Row 0 names the channel by its host and folder,** such as `user.github.io/ceefax`, never by the name the channel gives itself. A name too long for the row keeps the last segment of the folder and as much of the end of the host as fits.
14. **Descriptor services keep working.** A descriptor given by an address whose file name is not `channel.json` is a service of REQ-WEB-002, with its page address template. Its channel identity is the folder that holds the descriptor, and its contents are the `contents.txt` in that folder. `neottx` keeps opening services this way. New channels are not told about templates.
15. **What is given up, deliberately.**
    - Collections whose files have other names, such as SPARK's `P100.tti`, are not served as they are; they are converted once into a channel folder, or read through a descriptor service (item 14).
    - A broadcaster with several channels on one server shows the tuning screen between them, since each is a channel.
    - A typed address without a trailing slash whose last segment contains a dot is taken as a file name, and is refused unless it is one of the channel's file names. A folder whose name contains a dot must be typed with its trailing slash.
    - There is no bundle in TTI form: `index.t42` is the only bundle. An author of TTI pages publishes page files.

*Reason:* The site as an origin could not hold a channel in a folder, which is how the easiest free hosts publish (a GitHub Pages project site is `user.github.io/project/`), and it made strangers on one shared host one site. A channel as a folder works on every static host with no configuration. Fixed names, one request per question, no inference and failing closed keep it simple to learn, to build, and to make safe.

*Amended 12 October 2026 by decision E.73:* a folder whose four requests all show that it holds no channel leads to discovery (REQ-WEB-028), instead of a failure.
*Amended 18 October 2026* (decision E.76): the browser was removed from this repository. The rules above stand for the web module and `neottx`. What concerned only the browser no longer applies here: the tuning screen (items 2, 12 and 15), the name in row 0 (items 2 and 13), bookmarks and the confirmed channels of a session (item 2), the contents view (item 8), and discovery (decision E.73, withdrawn). The requests that items 6 and 10 give to the browser are made by the web module. `channels.md` is kept in the neoteletext-browser repository.

**E.71 — Capability features within Levels 2.5 and 3.5, and the modern-classic profile.**
Recorded on 12 October 2026, on the owner's instruction to add teletext feature flags so that capabilities can be compiled out, with this "modern-classic" profile: keep the 40 by 24 grid, the classic eight-colour palette by default, contiguous and separated mosaics, no proportional spacing, non-spacing colour and character attributes, the additional international character sets, monochrome downloadable characters, reusable page objects and bold characters; leave out the 32-colour palette, italic characters, side panels, multicolour downloadable characters and animation. This amends rule 2 of decision E.61: within the enhancement area, there is one feature for each capability the owner named, not one for the whole area.

1. **Eight features of `neoteletext-core`, on by default.** Each needs `level25`, and the decoding and the drawing of its capability are both behind it, so a build without it contains neither:

   | Feature | Capability | When it is compiled out |
   |---|---|---|
   | `objects` | Active, Adaptive and Passive objects, and default objects (area OBJ) | Object invocations and default objects are ignored; a page's own enhancement data still applies |
   | `side-panels` | Side panels (clause 12.5) | There are no side panels; a page is always 40 columns, and anything placed in a panel is outside the page, as for a panel that is not enabled |
   | `colours32` | The Colour Map's CLUTs 2 and 3, and every redefinition of a CLUT by X/28/0, X/28/4, M/29/0 or M/29/4 (clause 12.4) | No CLUT is redefined, and CLUTs 2 and 3 are copies of CLUTs 0 and 1, so a page shows at most the sixteen default colours of CLUTs 0 and 1, and the eight classic colours stay the default |
   | `animation` | Additional Flash Functions: the flash modes, the fast rates and phases, incremental and decremental flash (Table 29) | The triplet is ignored; the Level 1 flash, slow and normal (decision E.48), is unchanged |

2. **The profile is a feature too.** `modern-classic` turns on `level15`, `level25`, `render`, `original`, `viewer`, `forms` and `objects`. A port uses it with the default features off.
3. **What is not a feature.**
   - The 40 by 24 grid, the eight-colour default and the mosaics are Level 1, which every build has.
   - Non-spacing attributes are the enhancement area itself, the `level25` feature.
   - Proportional spacing is never laid out (Part G item 1, decision E.50).
   - The additional international character sets: their glyphs are one generated table of every set (decision E.62), and their code tables are small, so compiling them out needs the glyph generator to write a table per script. That is later work.
4. **Gate builds.** `cargo xtask features` adds the core with Levels 1.5 and 2.5 and every capability off, and with `modern-classic`. The tests of a capability need its feature; a test file checks that each capability compiled out is ignored as item 1 says.

5. **Measured on 12 October 2026** with `cargo xtask size`, on the linked Cortex-M0 image (`thumbv6m-none-eabi`, `opt-level = "z"`, link-time optimisation) of a receiver with Level 1.5, the renderer and the viewer: 65,758 bytes with every capability of Levels 2.5 and 3.5, and 62,894 bytes with the modern-classic profile's, 2,864 bytes or about 4 percent less. Most of the enhancement processor is the non-spacing attributes and the walk, which the profile keeps.

*Reason:* Teletext's distinctive look comes from its constraints: a fixed 40 by 24 grid, eight saturated colours, block mosaics, attributes set within the text, and no proportional spacing or pictures. The profile keeps the capabilities that preserve those constraints, and those that make pages more usable without changing them: non-spacing attributes, more languages, monochrome custom characters in the grid, reusable objects and bold for legibility. It switches off, without removing, the capabilities that would dilute the style: a redefinable palette of 4,096 colours, italics, a page wider than 40 columns, per-pixel colour characters that tile into bitmap images, and animation. Each remains implemented and tested, and a port that needs one compiles it in. One feature per named capability keeps each one switchable alone, while the profile names the recommended choice once. `docs/spec/design/capabilities.md` explains the mechanism.

*Note of 20 October 2026:* the `drcs` and `drcs-multicolour` features were removed with the decoding of dynamically redefinable characters (decision E.95); the profile's monochrome downloadable characters went with them.

*Note of 20 October 2026:* the `bold` and `italic` features were removed with the Level 3.5 font styles (decision E.96); the profile's bold characters went with them.

**E.72 — Fonts: Bedstead as the one bundled font, and every other font a 12 × 10 BDF loaded when it is named.**
Decided by the owner on 12 October 2026. This supersedes decision E.62 and amends decisions E.25 and E.34 (REQ-WEB-014 now permits one kind of third-party request: a font named with its checksum). `docs/spec/design/font-security.md` is the security analysis of this decision.

1. **One bundled font: Bedstead 3.261.** It is the receiver's own font and the default for every page. It keeps the 5-dot by 9-line form and the SAA5050 character rounding (REQ-RND-010, REQ-RND-012). The project's original glyph set of decision E.62, its drawing rules and its checks are removed; they remain in the git history up to commit 9dc932d. The glyph switch between sets is removed.
   - *Licence:* CC0 1.0 for the Bedstead authors' work, and section 55 of the United Kingdom Copyright, Designs and Patents Act 1988 for the SAA5050 designs it reproduces, which permits making and using typefaces from them because the SAA5050 was marketed in 1982 or earlier. Debian files Bedstead as non-free because uses other than as a typeface might infringe (`third_party/bedstead/debian-copyright.txt`). The owner accepts that position.
   - *Gaps:* Bedstead has no Arabic, and lacks six Latin national characters (Ă ă Ğ ğ Ƶ ₺) and two G2 characters (― ™). *Corrected 12 October 2026:* the decision first said seven national characters; Bedstead draws Đ (U+0110), and `crates/neoteletext-core/tests/coverage.rs` lists the gaps as measured. Where a character can be composed from a base letter and a spacing mark (decision E.26), it is; otherwise it is drawn as the visible placeholder of REQ-CHR-004, until a loaded font supplies it.
2. **Every other font has one form: a BDF file of 12 × 10 cells.** 12 × 10 is the cell of the standard's own downloadable characters (EN 300 706 clause 14.1). A font is drawn exactly as given, as monochrome DRCS characters are, without character rounding: each line twice in the 12 × 20 cell, and enlarged for double height, width and size. The profile is REQ-FNT-002; anything else is refused.
3. **Fonts are loaded when they are named, and only then,** from any `http` or `https` address: by the user, by the channel, or by the page. The repository ships only Bedstead. The owner's collection of fonts is kept in a separate repository and published as font libraries.
4. **Who chooses.** The page's font, then the channel's, then the user's chosen font, then Bedstead; and a user setting makes the user's font win over every channel and page. Row 0, the tuning screen, the contents view and every page the receiver draws itself always use Bedstead. Characters missing from the font in use come from the user's font, then from Bedstead, then the placeholder.
5. **Font libraries.** A folder with a fixed-name index, `fonts.txt`, lists any number of fonts, each with its SHA-256, so fonts can be found without knowing their names.
6. **The cache.** Fonts are stored by content, named by their SHA-256; fonts named by a channel or a page are kept apart per channel; at most 4 MB in all and 256 KB per font, the least recently used removed first; checked again whenever loaded; written only by the interface process.
7. **The converter.** `neottx font convert` turns a BDF of another size into the 12 × 10 form, offline, only where the fit is exact and reversible: doubling every pixel's width, adding the blank top line where the SAA5050 leaves it, or halving a 12 × 20 font whose line pairs are identical. Anything else is refused. *Amended 13 October 2026 by the owner (issue #206):* a 5-pixel cell goes in the right of the 6, so its blank column is at the left, where the renderer leaves Bedstead's (REQ-RND-016) and where the SAA5050 dumps of 6-dot cells have it; placed at the left, the same design was drawn one dot to the left of Bedstead and matched none of its glyphs.
8. **Not fonts:** mosaics and G3 characters are drawn from geometry whatever the font (REQ-CHR-005, REQ-RND-037), and DRCS characters come from the page.

   *Amended 13 October 2026 by the owner (issue #209):* a loaded font whose every row is made of identical pixel pairs is a 5- or 6-dot matrix with each dot drawn two pixels wide, as REQ-FNT-010 converts one. The SAA5050 never showed its matrix unrounded (Part C.4), and drawn so, the 76 such fonts of the owner's collection looked coarsely scaled beside Bedstead. Such a font is drawn with the character rounding of decision E.13, exactly as the bundled glyphs are, whenever rounding is on; a font drawn at half-dot resolution, whose designer drew any smoothing into its pixels, stays unrounded.
9. **Features.** The core's `original` feature is removed; `bedstead` becomes a default feature and part of the modern-classic profile (decision E.71). The fonts of items 2 to 7 are the browser's, and `neottx`'s converter.
10. **Phases.** Item 1 and the converter are Phase 1. Loading, choosing and caching fonts follow the browser: Phase 1 on macOS and Linux, Phase 2 on Windows, Android and iOS.

*Amended 18 October 2026* (decision E.76): the browser was removed from this repository. Loaded fonts are now the engine's alone, used as `neottx render --font` and the web module use them, and they are Phase 1 (item 10). The cache of item 6 was the browser's and is withdrawn with it (REQ-FNT-009); items 9 and 10 no longer refer to the browser.

*Amended 18 October 2026* by decision E.77: loaded fonts may also be 12 × 20 (items 2 and 7), and the Noto-derived fonts and their converter are kept in the repository, though not compiled in (items 1 and 3).

*Reason:* Bedstead is the most complete openly licensed teletext font and the only one of the 317 collected text fonts with a licence the project can ship that also fits the receiver. One small, exact font form keeps the reader that faces the network simple, keeps every font in the grid and look of teletext (decision E.71), and lets the 151 collected fonts that fit be used without the project distributing them.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).


**E.73 — Discovery: an ordinary web address leads to a channel by pointing to it.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**E.74 — The font browser: choosing the user's font by moving through a list.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**E.75 — Character sets as the teletext hardware expected.**
Decided by the owner on 13 October 2026, who set the aim of the font work as matching "what the teletext hardware expected to find" and asked for the font browser to be organised as teletext organises character sets, to show only fonts that meet hard, numbered technical rules, and to show how a page designates its character sets. This adds to decisions E.72 and E.74. The words keep their meanings: a *character set* is a code table of EN 300 706 (G0 Latin with the German national option, G2 Greek); a *font* is a loaded BDF file; a font taken from a chip is described as a dump of its character ROM.

1. **The unit is the character set.** A decoder's character generator held, for each character set it supported, a glyph at every one of the 96 code positions 2/0 to 7/F, and never mixed styles: every glyph came from one ROM. A font is therefore judged for each character set separately, by the rules of REQ-FNT-011, and one font may conform for several sets. The 24 sets fonts draw are the G0 Latin set, its 13 national option sub-sets, the three G0 Cyrillic options, G0 Greek, G0 Arabic, G0 Hebrew, and the G2 Latin, Cyrillic, Greek and Arabic sets. G1 mosaics, G3 characters and DRCS characters are never drawn from fonts (decision E.72 item 8).
2. **Conformance is exact and decided from the glyphs alone** (REQ-FNT-011). The standard defines which character sits at each position, not how it looks, so the rules check the format, every assigned position, the blank space and inked characters, and nothing about shape. The SAA5050's cell, a blank top line and a blank leftmost dot column, is an optional profile, not a rule: measured on the owner's collection, 57 of the 157 pairs of a font and a set it is complete for fail it, among them fonts of real television decoders drawn at half-dot resolution. Whether a glyph depicts the right character cannot be decided by rule and is not claimed.
3. **A library states conformance** (REQ-FNT-004). Listing a library fetches no font, so the sets a font conforms for must be in `fonts.txt` for the browser to organise fonts before loading them. The browser computes conformance again from the glyph table when a font loads and believes the computation, not the index. `neottx font sets` computes it for publishers with the same code.
4. **The browser follows the standard's organisation** (REQ-BRW-010). Its tree is the entries of Table 32 by group, each naming a G0 set with its national option and the G2 set the standard pairs with it, then the fonts conformant for each set. A character set appears under every designation that names it, as in the table. The 96-position grid of the chosen fonts replaces the sample of decision E.74 item 7: a hole at a position is what the hardware would have shown.
5. **The page's designation is shown** (REQ-BRW-011). A Level 1 page sends only the three national option bits C12 to C14 in its header; the four-bit group came from the receiver's setting, which is where a real receiver showed the wrong characters. Packets X/28/0 Format 1, X/28/4, M/29/0 and M/29/4 send the whole designation, a second G0 set (Table 33), and ESC and the X/26 Modified Designation triplet change sets within a page. The browser shows each step as the decoder resolved it, and where the group came from.
6. **One user font per character set** (REQ-FNT-005). As a decoder was fitted with a ROM for each set, the user chooses a font for each character set, and each character of a page is drawn from the font chosen for its own set; a set with no choice is drawn in Bedstead. This replaces the single user font with fallback, which mixed styles whenever the font lacked a character. Fonts named by a channel or a page stay whole fonts (REQ-FNT-006, REQ-FNT-007). The single user font of earlier settings is not carried over; the user chooses again.

*Amended 18 October 2026* (decision E.76): the browser was removed from this repository, so items 4 and 5, which concerned its font tree and its display of the page's designation (REQ-BRW-010 and REQ-BRW-011, withdrawn), no longer apply here. Item 3's listing of a library is done by a port; the core's conformance rules and `neottx font sets` are unchanged.

*Reason:* A receiver showed complete character sets from its ROMs, in one style per set. Judging fonts per set, by rules that can be checked exactly, and choosing one per set reproduces that; organising the browser by the standard's own tables, and showing the designation the decoder resolved, lets the user see why a page shows what it shows.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.76 — The repository is the core teletext engine; the browser is a prototype.**
The owner decided on 16 October 2026 that this repository is focused on the core teletext rendering engine. The windowed browser, which was an attempt to put the engine into a web browser for teletext, moved to `prototypes/browser/`:
1. **Code.** The crates `neoteletext-gui` and `neoteletext-browser`, the browser's decoder process (formerly `neoteletext-app`'s `decoder` module), `scripts/macos-app.sh` and the test sites `sites.py` form a Cargo workspace of their own there, built against the engine's crates by path. They are outside the engine's workspace and its gates.
2. **Requirements.** Area BRW, decisions E.63 to E.69, E.73 and E.74, and REQ-SEC-025, REQ-NAV-013 to REQ-NAV-015, REQ-TST-081, REQ-WEB-028 to REQ-WEB-030 and REQ-CLI-009 moved to `docs/requirements.md` of the neoteletext-browser repository. Each keeps a one-line entry here, so its number is not reused and references still resolve.
3. **What stays.** Everything `neottx` and the library use stays, including the web module, channels (decision E.70), the fetcher and decoder processes of web and live mode, and the fonts of area FNT. Requirements that stay but still describe the browser in part are listed in `TECH-DEBT.md` for rewording.

*Amended 18 October 2026:* the owner removed the browser from this repository: the prototype folder `prototypes/browser/` was deleted, together with the code in the engine's crates that only the browser used. The browser is maintained in the separate neoteletext-browser repository. The requirements and decisions item 2 moved are withdrawn, and so are REQ-SEC-026 and REQ-FNT-009; the requirements that mixed engine and browser behaviour (area FNT, REQ-WEB-014, the channel requirements REQ-WEB-018 to REQ-WEB-027 and REQ-NAV-012) were reworded for the engine alone.

*Reason:* The owner's instruction of 16 October 2026: "after this point this repo will be focused only on the core teletext rendering engine". Keeping the prototype buildable, rather than deleting it, preserves the work and lets it serve as an example of a port.

**E.77 — Loaded fonts of 12 × 20, and the Noto-derived fonts kept in the repository.**
Decided by the owner on 18 October 2026, for the languages of decision E.78. This amends decision E.72 items 1, 2, 3 and 7. The design is `design/languages.md` sections 4 and 5.
1. **A loaded font may be 12 × 20 as well as 12 × 10,** drawn line for line in a normal cell (REQ-FNT-002, REQ-FNT-008). 12 × 10 fonts, Bedstead and DRCS characters are drawn as before.
2. **The Noto-derived fonts and their converter are kept in this repository,** in `third_party/noto/` (REQ-FNT-012). They are the only glyph data besides Bedstead. They are loaded at run time and never compiled in, and the Noto sources are downloaded pinned by checksum, never committed.
3. **The converter's settings** are those measured in the October 2026 prototypes (`design/languages.md` section 5).

*Reason:* At 12 × 10 the new scripts were not legible in the prototypes, and at 12 × 20 every one was. Clause 15.6 leaves the appearance of characters to the decoder. Keeping the converter and its settings with the fonts makes them reproducible.

*Note of 20 October 2026:* what this decision says of DRCS characters, pages, links or colour tables no longer applies; they are not decoded (decision E.95).

**E.78 — Languages beyond the standard's character sets.**
Decided on 18 October 2026: the scope, the allocation of reserved values and Kazakh by the owner, the rest by the designer on the owner's instruction. The design is `design/languages.md`, and the evidence `research/notes-languages.md`.
1. **Scope:** about 55 languages whose letters fit sets of 96 characters with one byte per cell. They are listed in `design/languages.md` section 2. Kazakh is written in its Latin alphabet, which the standard's sets already serve.
2. **Conformance comes first, as far as it can go.** Every language uses the standard's own mechanisms, and no structural departure remains: every row of every set is one byte per cell, read as EN 300 706 defines.
3. **New designation codes are unavoidable,** and use only values Tables 32 to 34 reserve, in groups 0101, 0111, 1001, 1011 and 1100 (REQ-CHR-012). The allocation is neoteletext's own, so other decoders fall back as REQ-CHR-003 describes.
4. **Every new set has Latin English as its second G0 set,** switched by ESC.
5. **Characters beyond a set's 96 come from its G2 set through X/26,** never from a wider code.
6. **Where hardware or a standard chose the codes, they are kept:** Thai from the Philips SAA5246AP/S, Lao from the 1990 World System Teletext specification, the Serbian and Macedonian sets from Table 38, and Ukrainian from Table 40.
7. **Out of scope** (`requirements/out-of-scope.md` items 14 to 16): Chinese, Japanese, Korean, Ge'ez script, the Indic scripts, Burmese, Khmer and Sinhala. The codes China's CCST uses are kept free so that it can be added later. Inuktitut syllabics was taken out of scope by the owner on 18 October 2026 (item 16), and its value, group 1011 option 011, is reserved again.
8. **Phase 2** (section F.0.1).

*Reason:* These sets extend the receiver to about 55 languages without changing how a page is sent, and without leaving the standard's mechanisms.

**E.79 — Arabic-script languages: plain letters, joined by the receiver.**
Decided on 18 October 2026 by the designer, the owner having left it to the designer. The design is `design/languages.md` section 6, and the rule REQ-CHR-015.
1. **The sets of group 1001 hold each letter once,** and the receiver chooses its joined form from its neighbours in the row. Rows stay in visual order, as Table 44's do. Arabic itself keeps Table 44.
2. **Digits keep the codes 3/0 to 3/9,** drawn in each language's own shapes.

*Reason:* Sending each form, as Table 44 does, cannot fit Urdu, Pashto or Sindhi in 96 positions. One code is still one letter in one cell, and drawing its joined form is its appearance, which clause 15.6 leaves to the decoder. This is the method EBU Tech 3232 proposed in 1982.

**E.80 — Thai and Lao: the Philips SAA5246AP/S scheme.**
Decided on 18 October 2026 by the designer, on the owner's instruction to follow the Thai sources and put conformance first. It replaces the designer's earlier choice of marks that take no cell. The design is `design/languages.md` section 7, and the rule REQ-CHR-016.
1. **Thai follows the SAA5246AP/S,** the Thai variant of this project's baseline receiver chip:
   - its G0 set and option bits (010 Thai, 000 English);
   - ESC between Thai and English;
   - its 32 rarer mark glyphs as a G2 set placed by X/26;
   - marks on rows of their own above and below the letters.
2. **The receiver does nothing new:** a character's column is always its byte position.
3. **Lao** keeps the 1990 World System Teletext specification's code table and follows the same layout.
4. **Not adopted:**
   - the signalling of the Chulalongkorn decoders, which is outside the standard;
   - the 1990 specification's Thai set and its overlay format, which EN 300 706 dropped and no hardware is known to have used.

*Reason:* The chip shows how a real decoder of the family this project follows carried Thai within the standard, with no departure from it and no new receiver rule.

**E.81 — Diacritical marks from the page's own G2 set.**
Decided on 18 October 2026 by the designer, on the owner's instruction that conformance comes first. The design is `design/languages.md` section 8.1, and the rule REQ-CHR-014.
1. **A mark comes from column 4 of the page's current G2 set** when that set's column 4 holds marks, as Table 29 says, and from Latin G2 otherwise. Pages with the standard's existing sets are unaffected.
2. **Vietnamese is a whole Latin-script G0 set** with a Vietnamese G2 set holding the hook above, its tones added by X/26. Yoruba and Igbo get their tones the same way, from their own whole set. Both were first designed as national options; they are whole sets because an X/26 mark is placed on the G0 set's base form, never on a national option's character (REQ-CHR-002), so a tone on a national position would land on the wrong letter. Amended on 18 October 2026.

*Reason:* This is the standard's own Level 1.5 mechanism, used as Table 29 and Annex E.10 describe. Spain's 1986 teletext order met its regional languages the same way.

**E.82 — Writing pages in the new sets, and the `unicode-bidi` crate.**
Decided on 18 October 2026: the library by the designer on the owner's instruction. The design is `design/languages.md` section 9, and the command REQ-CLI-010.
1. **The writer lives in `neoteletext-app`** (layer L9), behind its feature `write-text`, because it needs the core's character tables, which layer L0 may not use.
2. **It reorders right-to-left text with the `unicode-bidi` crate** (MIT or Apache-2.0), an implementation of UAX #9. The crate is used only in the writer; the core and the receiver never reorder text.

*Reason:* Pages must be writable for a set to be usable, and bidirectional reordering is best taken from a maintained implementation.

**E.83 — The specification is a folder, with one home for each fact.**
Decided by the owner on 18 October 2026, who asked for "a clear and well isolated set of design and requirements docs that clearly identify the behaviour of the system", and chose to split `REQUIREMENTS.md` into a folder.
1. **`docs/spec/` is the specification.** `docs/spec/README.md` is its map, and `REQUIREMENTS.md` is kept as a one-page index of where each former part went. The text moved unchanged, and its part, section and decision numbers keep identifying it.
2. **Each fact has one home:**
   - behaviour in `requirements/<area>.md`, the only normative text;
   - choices and their reasons in `decisions.md`, each pointing to the design for detail;
   - explanation in `design/`;
   - code placement in `ARCHITECTURE.md`;
   - evidence in `research/`.

   Other documents link rather than restate.
3. **The other design documents moved in too:**
   - `capabilities.md`, `font-security.md`, `porting.md` and `hmi.md` into `design/`;
   - `sources.md` became `module-sources.md`;
   - `open-questions.md` moved to the folder's top level.

   `docs/` keeps the user guide and the generated traceability table.
4. **`cargo xtask traceability` reads the requirement files** in the order of the area table in `requirements/README.md`.

*Reason:* One file of 6,800 lines held walkthroughs, decisions, requirements and appendices together, and a design change had to be repeated in several places. A folder with one home per fact keeps the behaviour clear and the documents from drifting apart.

**E.84 — Audio: PCM and IMA ADPCM, carried on EN 300 708's audio data lines.**
Decided on 19 October 2026. The owner asked for PCM and ADPCM ("i want pcm and adpcm ideally"), authorised "any of the common audio bit rates even if not in the standards", ruled out MIDI and MUSCOT, and asked for the older standards' codecs only if they gave compatibility. The detail is `design/audio.md`; the rules are area AUD.
1. **Two formats:** 16-bit linear PCM and IMA ADPCM in WAVE blocks. ETS 300 149's G.711 and G.726 are not carried: no teletext service ever carried audio in them, and without that standard's framing they would be compatible with nothing. IMA ADPCM was chosen over G.726 because each block decodes on its own, so a receiver can join a carousel at any block and a lost block costs only its own samples.
2. **Eight sample rates** from 8 to 48 kHz, the common ones, whether or not a teletext standard names them.
3. **Broadcast on data channel 12** (packet 4/31), EN 300 708 clause 6.7.3's programme-independent audio, as a user-defined service of Table 21, with the 16-bit check of clause 6.5.8.1. IDL Format B's Reed–Solomon code would correct more, but needs an EBU Application Identifier this profile does not have.
4. **Records of at most 1 KB** with a CRC-32, the same on every carriage, so a microcontroller reassembles in a small buffer.
5. **Ducking uses EN 300 708 Table 19's steps,** Audetel's fade byte, applied to the other channels.

*Reason:* The standards give the channel and its framing but no codec that suits both a lossy broadcast and the microcontrollers' tellyboy driver, which plays 16-bit PCM and IMA ADPCM natively.

**E.85 — Audio: two engine crates, and the port supplies a voice sink.**
Decided on 19 October 2026, while the owner was away, as the owner had instructed.
1. **`neoteletext-audio`** (`no_std`, no allocation) holds the carriage, records, the store, control and the IMA ADPCM decoder; its `encode` feature, which allocates, writes sounds for publishing tools.
2. **`neoteletext-audio-mix`** (`no_std` with `alloc`) is a software mixer for ports with an operating system's audio output. It behaves as tellyboy does: four voices, whole-percent gains, every change ramped over at least 5 ms, and a clamp after the sum. It resamples with a windowed-sinc filter, better than tellyboy's linear interpolation, because desktop speakers reproduce what linear interpolation leaves; the filter is a table written by `tools/sinc_table.py`, so the crate has no floating point.
3. **The port passes packets to the audio receiver** before the teletext decoder, which keeps ignoring packet 31 (REQ-PKT-008). EN 300 708's numbers are constants of `neoteletext-core::consts` (REQ-GEN-014).
4. **The porting contract gains an optional audio sink** (`neoteletext_audio::port::VoiceSink`), shaped like tellyboy's voice calls, so a microcontroller port maps onto that driver directly.
5. **Live sounds are not built yet;** a receiver counts them as unsupported.

*Reason:* Every receiver needs the same rules, so they belong in the engine, beside the decoder; the hardware stays the port's, as REQ-ARCH-010 requires.

**E.86 — Audio: the latest command wins, and a page's sound plays while the page is open.**
Decided on 19 October 2026.
1. **Each channel keeps one sequence number,** compared by RFC 1982 serial arithmetic, in place of the sessions, command history and cancellation barriers of the profile's first version: repeats never replay, and a stop arriving before the older play it cancels still wins, in constant memory.
2. **A sound bound to a page starts whenever the page is opened** and fades out when it is left, while the user's page sounds setting is on. A sound for the whole service plays once, when its command is accepted.
3. **No change of level is faster than 5 ms,** tellyboy's measured limit for a click.

*Reason:* Teletext attaches things to pages; and a microcontroller cannot keep state that grows with the history of a carousel.

**E.87 — Audio: live sounds join at the newest block, and the sink follows the sender's clock.**
Decided on 19 October 2026.
1. **The receiver holds a live sound's newest 32 blocks** in a ring in the store, with a table of the index in each slot, and gives the sink the blocks in order through two further `VoiceSink` operations, `stream` and `stream_samples`. A play joins at the newest block, as a radio is heard as it is now. A missing block is waited for until three newer ones are held, then sent as silence.
2. **The sink, not the receiver, follows the sender's clock,** as tellyboy's stream voice does: the mixer buffers 400 ms and trims its rate by up to ±5 % in proportion to the buffer's distance from that level. The receiver stays free of sample timing.
3. **The port gives the receiver the time** (`Receiver::tick`), which it needs only for the two-second stall of EN 300 708 clause 6.7.1.1. A stalled sound fades over 100 ms, is forgotten at the sink, and its play waits for blocks again.
4. **`neottx sound --live` paces a file for a player,** filling the gaps with repeats of the announcement and the latest block, as a carousel repeats records, so a file channel carries a live sound.

*Reason:* A live sound's end is unknown, so it cannot be held whole, and a receiver that waits to resynchronise loses more than it would by joining late. Rate trimming in the sink keeps the receiver the same on every port.

**E.88 — Links a receiver draws: the `web:` target, and an underline the port can turn off.**
Decided on 20 October 2026, for T42 Browser's hyperlinks (its `hyperlinks.md`, decision 41 there).
1. **A `web:` target** in the TTI Link Extension (Appendix E, section E.1): `web:` followed by an `http` or `https` address names a page on the web, which a receiver opens outside teletext, after the user agrees. It is allowed in `LN` and `LT` lines and never in `LK`, because typing a page number must never leave teletext. The page encoder writes it as null, as it writes every address.
2. **An `underline` render option** in `RenderOptions`, on by default, so every port renders REQ-RND-035 as the standard asks. A port that draws its own link underline over the page turns it off, so that within that port an underline has one meaning. Only G0 and G2 characters are affected; separated mosaics, which share the attribute bit, are not.
3. **Showing `LT` and `LK` links,** which REQ-NAV-013 and REQ-NAV-015 once required of the engine's browser, is the browser's own concern, in its repository. The engine parses and keeps them, and nothing more.

*Reason:* A channel's folder is an ordinary web address (decision E.70), so no rule can tell a teletext address from a web page: a link to the web has to say so. A receiver that draws its own underline needs the page's underline out of the way, and no other port should change; an option, on by default, costs every other port nothing. Decision E.36's principle holds: the receiver draws every link marker, and the page draws none.


**E.89 — Audio: no speech codec, no simultaneous start, and the codec's sources obtained.**
Decided by the owner on 20 October 2026.
1. **There will be no speech codec.** The profile carries 16-bit PCM and IMA ADPCM and nothing else; codec code 2, held for a speech codec for live broadcast, is reserved like codes 3 to 255. Live sound over broadcast is IMA ADPCM at a low rate, or is not broadcast.
2. **No simultaneous start of several voices** is added to tellyboy. Version 1's cue groups stay dropped; each play is its own CONTROL record and starts within a few milliseconds of another.
3. **The IMA 1992 recommendation (revision 3.00) and Microsoft's 1994 Multimedia Standards Update** are kept in `teletext-reference/teletext-audio/codecs/`, so the ADPCM decoder is checked against the text that defines it rather than against itself.
4. **Level A's memory numbers stay proposals** until the microcontroller build exists to measure them against.

*Reason:* A speech codec would be a library needing approval, with an unmeasured cost on the smallest part, for a use (live talk over broadcast) nobody has asked for; IMA ADPCM already carries live sound over the network. A simultaneous start needs state the profile was redesigned to avoid. The codec's sources were the one thing the decoder's tests could not be checked against.

**E.90 — Input: keys sent back to a channel, as a terminal sends them (area INT).**
Decided by the owner on 20 October 2026, as T42 Browser's decision 44 (its `docs/interaction.md`). The detail is `design/input.md`.
1. **A channel that says `"input": true`** in its descriptor, allowed only with push over a WebSocket, takes the viewer's keys back on that socket, one binary message a key. There is no other way in: no request per key, no forms over this path.
2. **The key record is 14 bytes,** with up to 32 bytes of committed text, carrying a sequence number and the receiver's millisecond clock, and naming keys by the W3C `KeyboardEvent.key` and `KeyboardEvent.code` values in two numbered tables that only grow at the end.
3. **The cursor record is optional,** as a cursor is: a game sends none and keys go just the same.
4. **A new crate, `neoteletext-input`** (`no_std`, no allocation), holds both records, so a microcontroller can send keys and read cursors. It depends on the core and on the audio profile's carriage, which it shares (E.91); the WebSocket, TLS and the phone keyboard's cryptography stay out of the microcontroller crates (decision E.38).

*Reason:* A terminal sends bytes and lets the host decide what they mean; a receiver that does the same executes nothing and keeps every page a plain teletext page, and a game, a form and a shell are the same to it.

**E.91 — Input: the cursor record is type 4 of the audio profile's frame, on data channel 12.**
Decided on 20 October 2026. The browser's design proposed data channel 13, to be checked against EN 300 708 first.
1. **EN 300 708 Table 16 gives data channel 13 to Datavideo** (clause 6.6), so it is not free. The cursor record is instead record type 4 of the audio profile's frame (REQ-AUD-003), on the profile's own channel 12, a user-defined service of clause 6.7.3.2.
2. **It is always one fragment** (16 bytes), so a receiver reads it from its packet without reassembly, and the audio receiver steps over it before reassembly, so a cursor's record id never meets a sound's. The audio crate exposes the frame (`record::split`, `record::write`) and the packet writer (`idl::packet`) that both use, so neither is written twice.
3. **A TTI page carries it as a `CU` line,** kept by the reader and the writer. The engine's encoder does not turn it into a packet: the formats crate takes nothing from the input profile, and a receiver reads the line from the page it already has.

*Reason:* Taking an allocated data channel would collide with a real service on air; the audio profile's carriage is proven, and one fragment needs nothing new.

**E.92 — Push: a heartbeat, the input flag, and a binary message is packets.**
Decided on 20 October 2026, for the browser's typing back (its `interaction.md` section 12, decision 44 there).
1. **`push.input`.** A channel says it takes keys with `"input": true` inside its `push` object, which is refused with server-sent events: keys go back only on the push WebSocket. The design proposed a top-level member; inside `push` the rule that it needs a WebSocket is the object's own.
2. **`Input::Send`.** The agent writes one message of the input profile, at most `INPUT_MESSAGE_MAX` (46) bytes, as a binary WebSocket message on the open push connection of a channel that declares input; anything else is dropped with `Event::InputDropped`. Nothing it sends changes what is fetched.
3. **A heartbeat** (REQ-WEB-005): a WebSocket silent for 25 seconds is pinged and lost without a pong within 10 more; an event stream silent for 60 seconds is made again. The backoff and the subscription follow as for any lost connection.
4. **REQ-AUD-008 amended:** a binary WebSocket message is packets, as REQ-WEB-005 says; sounds travel over a WebSocket as their broadcast packets.
5. **Cookies go with the upgrade,** as REQ-WEB-006 already had them, so a channel knows a returning viewer the way a web application does; the key records identify no one.

*Reason:* A connection that died silently, behind a NAT that dropped it, would leave a viewer typing into nothing; the 25 seconds is the keepalive qrkeyboard's relay uses for the same reason. Two meanings for a binary message cannot both stand, and packets carry everything records can.

**E.93 — A phone as the keyboard: `neoteletext-qrkeyboard`.**
Decided on 20 October 2026, for T42 Browser's phone keyboard and `neottx keyboard` (the browser's `interaction.md` section 9, decision 44 there).
1. **A std application crate,** `neoteletext-qrkeyboard`, speaks qrkeyboard's protocol 3 as the screen: the instance, the QR address, pairing with or without a PIN, sealed frames and the application messages. Its own phone side is for tests and for checking a relay. It holds sockets, TLS and cryptography, so it is never a microcontroller crate (decision E.38).
2. **Libraries,** each a choice made for this: RustCrypto's `aes-gcm`, `hkdf`, `hmac`, `sha2` and `p256` (approved by the owner as the browser's decision 44 item 8); `rustls` and `webpki-roots`, as neottx's fetcher already uses; `getrandom` for the operating system's random numbers; and Project Nayuki's `qrcodegen` (MIT, no dependencies) for the QR code. The relay's own SDK uses a QR library too; drawing one by hand would be a large encoder of no interest.
3. **HTTP heads, WebSocket frames and JSON** are read by the web module's own bounded code, not by more libraries.
4. **Checked against qrkeyboard's own code,** not against itself: hashes, a SPAKE2 run and sealed frames computed by its `crypto.ts`, and pairing through a running copy of its relay.

*Reason:* Without the screen's side in the engine, every channel author who wants a phone as a keyboard would implement qrkeyboard's pairing and cryptography; and the browser's phone keyboard needs the same code.

**E.94 — neottx: `qr`, `keyboard`, and typing back in `view`.**
Decided on 20 October 2026, as the browser's decision 44 item 11.
1. **`neottx qr`** writes any text, a qrkeyboard join address above all, as a QR code in a page's mosaics, so a channel can be the screen a phone scans, on any receiver. With one sixel a module a join address of about 100 bytes is a version 5 code at the low correction level: 23 columns by 15 rows with its quiet zone. A teletext sixel is 6 by 6⅔ dots, so the modules are nearly square; the rendered page was read by a QR reader (jsQR) on 20 October 2026.
2. **`neottx keyboard`** is the bridge `interaction.md` section 9 describes: it pairs a phone through qrkeyboard and sends its keys to the channel's push WebSocket as key records, exactly as a browser would, so a channel takes a phone and a browser alike without implementing qrkeyboard. It holds the network, and runs without the sandbox, as `neottx fetch` holds it.
3. **`neottx view` types back** on a channel that takes input: every key goes, as from a terminal, and Control-] leaves, as telnet's escape character does; the app's pipe carries the records (`Frame::Key`, at most 46 bytes) and the fetcher's word that keys may go (`Notice::Input`). A terminal reports a key once, so each goes down then up.
4. **The command-line program asserts** that the web module's, the app pipe's and the input profile's bounds on a key record agree.

*Reason:* Authors test a channel that takes input without the browser, and a channel author gets a phone keyboard by running one command.

**E.95 — Dynamically redefinable characters are not decoded.**
Decided on 20 October 2026, on the owner's instruction to remove DRCS completely.
1. **Nothing of clause 14 is decoded.** No DRCS page is read for its patterns, no DRCS character is drawn, and there are no DRCS colour tables. The `drcs` and `drcs-multicolour` features, the core's `drcs` module, its character set, its fault counts and the renderer's multi-colour lines are gone. Decision E.54 is withdrawn, and what decisions E.4, E.29, E.40 to E.44, E.47, E.48, E.50, E.52, E.56 to E.60, E.62, E.71, E.72, E.75 and E.77 say of DRCS no longer applies.
2. **A page that uses DRCS shows as a receiver without DRCS shows it.** A DRCS Character Invocation (Table 29 mode 01101) places nothing: the Level 1 character beneath shows. It still moves the active position, as every column triplet does (clause 12.3.4), so the triplets after it are placed as the service meant. A DRCS Mode triplet (Table 28 mode 11000) is read as a reserved row triplet, which does nothing.
3. **The packets that carry DRCS are read past.** X/28/1 and M/29/1 are ignored. Of X/28/3 only the page function and coding are read, so that a DRCS page that says what it is only there is still never shown as text (REQ-OBJ-041). The Global DRCS and DRCS links of X/27/4, X/27/5 and the Magazine Organization Table (its X/21 and X/24, the second byte of each page's byte pair, and the DRCS page total) are not used. X/24 must still be present for a Level 3.5 decoder to use the Level 3.5 part of the table (clause 10.6.1).
4. **DRCS pages are still kept as enhancement pages,** as is every page whose function is not "basic Level 1 page" (decision E.56), and never shown as text. When a set of pages is written, a DRCS page goes with the other data pages, after the object pages, rather than before them as Annex B.7 orders DRCS pages.
5. **The authored stream** keeps its DRCS triplets on page 113, sent as a service sends them, so that its every-mode page shows them ignored. Its DRCS pages and the display pages that showed them (114 to 116 and 119) are removed; their numbers are left unused. The Magazine Organization Tables it writes carry unused DRCS links, page FF.
6. **Requirement area DRC is withdrawn,** with the requirements of other areas that concerned only DRCS.

*Reason:* The owner's instruction. Decision E.54's readings of the X/28/3 bit order and the mode 3 pixel order had never been confirmed against a real Level 3.5 DRCS page (`open-questions-record.md`).

**E.96 — The Level 3.5 font styles are not drawn.**
Decided on 20 October 2026, on the owner's instruction to remove bold, italic and proportional spacing, set with the Font Style triplet, completely, keeping underline for links.
1. **The Font Style triplet (Table 29 mode 01110) is not acted on,** at any level: it moves the active position, as every column triplet does (clause 12.3.4), and does nothing else; its row extension goes with it. A Level 2.5 decoder already read it this way.
2. **Bold, italic and proportional spacing are gone** from the cell (`FontStyle` and its three bits), the renderer and the terminal output, with the `bold` and `italic` features; the modern-classic profile no longer has bold. Requirement REQ-RND-039 is withdrawn, and what decisions E.27, E.29, E.40, E.42, E.50, E.58 and E.71 say of the font styles no longer applies.
3. **Underline stays,** as decision E.47 has it: the Display Attributes triplet's underline (Table 27), drawn on the last dot line of G0 and G2 characters (REQ-RND-035), and the render option that a port drawing its own link underline turns off (decision E.88).
4. **The authored stream** keeps a Font Style triplet on page 113, sent as a service sends it, so that its every-mode page shows it ignored. Page 111, which showed the styles, is removed, and its number is left unused.

*Reason:* The owner's instruction.
