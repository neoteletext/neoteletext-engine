# F.5 Page acquisition and the page store (ACQ)

**REQ-ACQ-001 — Tracking the page in progress for each magazine.**
The decoder shall keep, for each of the eight magazines, a record of which page (if any) is currently being received in that magazine. It shall also record whether the service is in serial or parallel mode, taken from bit C11 of the most recent valid header.
*Verification:* Test.

**REQ-ACQ-002 — Starting a page.**
A header whose magazine, page number, sub-code and control bits are all valid (decision E.7) shall start the reception of that page in its magazine. Packets numbered 1 to 28 that follow shall be attached to the page as clause 7.2.1 defines:
- in parallel mode, packets of the same magazine;
- in serial mode, packets of the same magazine, received before the next header of any magazine.

In serial mode, packets numbered 1 to 28 from any other magazine shall be discarded and counted, because Annex B.3 does not permit them.
*Verification:* Test with interleaved serial and parallel streams.

**REQ-ACQ-003 — Ending a page.**
The reception of a page shall end:
- in serial mode, when any header of any magazine arrives;
- in parallel mode, when a header of the same magazine arrives (decision E.6).

The following headers end a page but do not start a displayable page:
- headers with invalid page number or control bytes;
- time-filling headers with page number FF.

In addition, a page shall be marked complete when 100 milliseconds pass without a packet for it (decision E.19). The timeout shall be configurable.
*Verification:* Test.

**REQ-ACQ-004 — Erase bit.**
When a header has bit C4 set, the stored rows 1 to 28 of that page and sub-code shall be cleared before any new rows are stored. This includes its enhancement and link packets.
*Verification:* Test.

**REQ-ACQ-005 — Page store in cache mode.**
The page store shall support a "cache" mode that stores every displayable page and sub-page received, up to a configured capacity. When the store is full, the least recently received page shall be replaced first. The capacity shall be set when the decoder is created, and the store shall use only the memory the caller provides.
*Verification:* Test that fills the store beyond its capacity.

**REQ-ACQ-006 — Page store in request mode.**
*Phase:* Phase 2 (whole requirement). Phase 1 uses cache mode only (REQ-ACQ-005), with a small capacity where memory is tight.
The page store shall also support a "request" mode like the SAA5246A. A configured number of page requests (four by default) is active at any time, and only pages matching a request are stored. Each request shall name:
- the magazine and page number;
- optionally, a sub-code pattern in which each of the four sub-code digits may be marked "don't care" (decision E.20).
*Verification:* Test.

**REQ-ACQ-007 — What is stored for a page.**
For each stored page and sub-page, the store shall keep:
- the header's page address and control bits;
- the header's 32 display bytes with parity flags;
- rows 1 to 25 with parity flags, and which rows have been received;
- up to sixteen packets X/26 with their designation codes;
- packet X/27/0 as decoded links;
- packets X/27/1 to X/27/3 raw;
- packets X/28/0 format 1, X/28/1, X/28/3 and X/28/4;
- packets X/27/4 and X/27/5;
- the time of the first and most recent reception;
- the number of receptions;
- counts of errors.
*Verification:* Inspection; test.

**REQ-ACQ-008 — Merging repeated receptions.**
When a page and sub-page that is already stored is received again without the erase bit, each received byte shall be merged into the store according to the parity-failure policy of decision E.8. A newly received row shall replace the stored row byte by byte. Rows not received in the new transmission shall keep their stored content.
*Verification:* Test with deliberately corrupted retransmissions.

**REQ-ACQ-009 — Magazine data.**
The decoder shall store, for each magazine, the most recently received valid packets M/29/0 and M/29/1. At Levels 2.5 and 3.5 it shall also store M/29/4, and shall discard them all when the service changes (REQ-ACQ-018).
*Verification:* Test.

**REQ-ACQ-010 — Update indicator.**
When a page with bit C8 set is received, or when the content of a displayed page changes, an event shall be raised so that a viewer program can redisplay or prompt.
*Verification:* Test.

**REQ-ACQ-011 — Rolling header and clock.**
The decoder shall provide the rolling header text and the clock text as decision E.11 defines. They are to be displayed while a requested page is being searched for.
*Verification:* Test with serial and parallel streams, including headers with C9 set.

**REQ-ACQ-012 — Search status.**
For the page the viewer has requested, the decoder shall report one of these states:
- "searching";
- "found";
- "found but not in the most recent cycle", meaning it is stored but has not been received for a configurable time.

The row 0 status column uses this state (decision E.10).
*Verification:* Test.

**REQ-ACQ-013 — Pages with hexadecimal numbers.**
Headers with hexadecimal page digits (A to F) shall be processed for page termination like any other header. Such pages shall not be stored in cache mode unless a diagnostic option enables it, and viewers shall not be able to select them by digit entry (EN 300 706 Table 1 note). At Levels 2.5 and 3.5 they are stored, because enhancement pages usually have hexadecimal numbers (REQ-ACQ-017).
*Verification:* Test.

**REQ-ACQ-014 — Non-text pages.**
If a page carries packet X/28/0 format 1 whose page function is not "basic Level 1 page" (0000), or whose page coding is not "7 bits plus odd parity" (000), the page shall be flagged as not displayable as text and shall not be rendered as a text page.
*Verification:* Test.

**REQ-ACQ-015 — Subtitle and newsflash pages.**
Pages with C5 or C6 set shall be stored and displayed like other pages. Their display shall use the boxed mode of decision E.22. Each reception replaces the previous content (normally with C4 set), so that subtitles update as they are transmitted.
*Verification:* Test with a subtitle stream.

**REQ-ACQ-016 — Enhancement pages are stored as received.**
At Levels 2.5 and 3.5, the store shall keep object pages, the Magazine Organization Table, and every other page whose function is not "basic Level 1 page", DRCS pages among them (decision E.56). The rows of these pages shall be stored as received, so that each reception replaces the stored bytes of the packets it carries, with no merge by parity (REQ-ACQ-008 and decision E.8 do not apply):
- a page whose X/28/0 or X/28/3 page function is not "basic Level 1 page";
- page xFE of each magazine;
- a page that a stored link of a Magazine Organization Table, or of a packet X/27/4 or X/27/5, names as an object page.

A page that becomes an enhancement page only after it was first stored shall be stored as received from its next reception on. Each packet's presence shall be recorded.
*Amended 20 October 2026 (decision E.95):* DRCS is not decoded, and the parts of this requirement that concerned it are removed.
*Verification:* Test with repeated receptions containing Hamming-coded bytes, and with a page that is named by a link after it was first stored.

**REQ-ACQ-017 — Hexadecimal pages at Levels 2.5 and 3.5.**
At Levels 2.5 and 3.5, pages with hexadecimal page numbers shall be stored, because enhancement pages usually have them (Annex E.1; decision E.56). Viewers shall still be unable to select them by digit entry, and navigation shall still skip them, unless the diagnostic setting of REQ-ACQ-013 is on.
*Verification:* Test at each presentation level.

**REQ-ACQ-018 — Magazine data at Levels 2.5 and 3.5.**
The decoder shall store, for each magazine, the most recent valid packets M/29/0, M/29/1 and M/29/4. Each new reception shall replace the stored packet of the same designation code. Every stored M/29 packet shall be discarded when the service changes, that is when packet 8/30 gives a new network identification, or when a new input is started. There shall be no time-out (decision E.56).
*Verification:* Test with a change of network identification and with a change of input.

**REQ-ACQ-019 — Capacity for enhancement pages.**
Enhancement pages shall be kept in a part of the store whose capacity, in packets, is set by the caller (REQ-GEN-003). The documented default shall be the totals of Annex E.8: 500 packets at Level 2.5 and 1200 at Level 3.5, over all magazines. When that part of the store is full, the least recently received enhancement sub-page shall be replaced first. Nothing shall be discarded when a Magazine Organization Table changes, beyond this eviction (decision E.56). The memory used shall be stated as REQ-GEN-009 requires.
*Verification:* Test that fills the enhancement store beyond its capacity.

**REQ-ACQ-020 — The read-only enhancement lookup.**
The page store shall offer a read-only lookup that the presentation layer uses to reach enhancement data (decision E.42). Given a magazine, a page number and an S1 sub-page number, it shall return the stored packets of that enhancement sub-page, with the record of which packets were received. Given a magazine, it shall return that magazine's Magazine Organization Table and its stored M/29 packets. The lookup shall have no side effects. Its cost shall not grow with the number of stored pages faster than a bounded search (REQ-SEC-012).
*Verification:* Inspection; test.

**REQ-ACQ-021 — Updates of enhancement pages.**
For each stored enhancement sub-page, the store shall apply the page status of Annex B.6 and Table B.1:
- a header with C4 set on a full transmission shall erase the stored packets before new ones are stored;
- a partial transmission (C13 set) and a continued fragment (C12 set) shall replace only the packets they carry;
- a change of the update indicator S2 shall be reported, so that decoded data taken from the sub-page is not reused.

The sub-code's last-packet number (S4 and S3) shall be recorded, so that completeness can be judged.
*Amended 20 October 2026 (decision E.95):* DRCS is not decoded, and the parts of this requirement that concerned it are removed.
*Verification:* Test for every row of Table B.1.

**REQ-ACQ-022 — Redrawing when enhancement data arrives.**
When an enhancement page, a Magazine Organization Table, or a magazine packet M/29 that the displayed page uses is stored or changed, the decoder shall raise an event, so that the page's grid is built again and its changed rows are drawn again (decision E.41, principle 6). The Level 1 page shall be shown at once, without waiting for enhancement data.
*Verification:* Test that a page is shown before its object page arrives, and is redrawn when it arrives.
