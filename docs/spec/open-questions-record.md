# Appendix D — Open questions and research record

This appendix lists the questions raised while writing this document. For each, it says whether research has answered it and where the evidence is. The research reports are in `research/`.

## D.1 Answered by research (1 October 2026)

| Question | Answer | Evidence | Recorded in |
|---|---|---|---|
| Which way round are the national option bits C12, C13 and C14? | Index = 4×C12 + 2×C13 + C14 | Decoding the headers of French and Italian broadcast recordings, where only this order gives the right language. zvbi uses the same order. SPB 492 makes C12 the most significant bit. | E.21, REQ-TST-012 |
| What is the exact SAA5050 character rounding rule? | The candidate rule is exactly the MAME emulator's, and jsbeeb, beebjit, Bedstead and the decapsulated-chip analysis are equivalent. No rounding across the cell edge. | `research/answers-saa5050-emulators.md` | E.13 |
| What are the separated mosaic dimensions? | Leftmost dot column and bottom line of each block blanked. Blocks 3/4/3 lines by 3/3 dots. | As above | E.14 |
| What is the real flash timing? | 48 fields shown, 16 hidden (0.78 hertz, 3 to 1) | As above | E.12 |
| Does a held mosaic keep its stored separated or contiguous form? | Yes, on the real chip and in the 1976 specification | As above; `research/answers-historic-specifications.md` | E.16 |
| Is one Start Box enough? | zvbi and vhs-teletext require two, as the standard's protection note intends | `research/answers-saa5050-emulators.md` | E.17 (default changed to two) |
| What do the X/28/1 and M/29/1 character set codes mean? | EBU SPB 492 Figure 14 | `research/answers-historic-specifications.md` | E.4 |
| Where does Annex A.3's row-address rule come from? | EBU SPB 492 clause 14.6.4 | As above | E.3 |
| Which Unicode characters represent mosaics? | Sextants U+1FB00 to U+1FB3B, plus the half and full blocks; separated sextants U+1CE51 to U+1CE8F | `research/answers-saa5050-emulators.md`; Bedstead | REQ-CHR-005 |
| What are four hard-to-read national option glyphs? | Ƶ, ł, the em dash and ‖ | zvbi and Bedstead | REQ-CHR-002 |
| Which Rust targets, tiers and toolchains apply? | See REQ-GEN-015. Xtensa needs Espressif's pinned fork. | `research/answers-rust-targets.md` | REQ-GEN-001, REQ-GEN-015 and REQ-GEN-016 |

## D.1a Settled for Levels 2.5 and 3.5 (10 October 2026)

The research notes `research/notes-level25-enhancement.md`, `research/notes-level25-objects.md` and `research/notes-level25-drcs.md` list the places where the standard is silent or contradicts itself at these levels. The owner's rulings of 10 October 2026 settle them in decisions E.40 to E.56. The questions that matter most are these:

| Question | Answer | Evidence | Recorded in |
|---|---|---|---|
| What are the default colours of CLUT 1 entries 5 and 6? | Half magenta (7, 0, 7) and half cyan (0, 7, 7), as Table 30 gives. zvbi 0.2.44 differs, so comparisons with it use colour indices. | The page image of Table 30, page 89; a zvbi run on `oldvga` | E.43, REQ-TST-023 |
| Which rows does the row count of a Font Style triplet reach? | The next R rows, from the same column to the end of each row | A zvbi run on `oldvga` | E.50, REQ-L25-028 |
| How does zvbi report G3 characters? | As the private value 0xEF00 plus the code | A zvbi run on `oldvga` | REQ-TST-023 |
| Does the strict row rule of clause 12.3.2 apply at Levels 2.5 and 3.5? | Yes. Object triplets carry object numbers in their address field. | Clause 12.3.2 and Table 28 | E.3 (amended), E.46 |
| Which row colour applies when only default colours are sent? | The default row colour; the default screen colour does not replace it | Annex A.5 and Table 4 | E.45 |
| How wide are the side panels, and is the display always 56 columns? | As the table of decision E.49, and 40 columns plus the panel widths | Clause 12.5 and Table 4 | E.49 |
| What do the flash phases and rates mean? | A one-second cycle of twelve units | Table 29 gives only the rates | E.48 |
| How are the G3 characters drawn? | From the sextant geometry of Unicode document L2/19-025, which matched the standard's figure on 95 to 100 percent of pixels | `research/notes-level25-drcs.md` section 9.3 | E.50, REQ-RND-037 |
| Which page links decide the object and DRCS pages? | X/27/4 and X/27/5 before the Magazine Organization Table, whatever its flag says | Clauses 10.5.1.4 and 10.5.2.2 | E.52 |
| What do objects do with damaged, missing or out-of-range data? | The fault is confined to the smallest unit, and the Level 1 page always shows | The owner's principle | E.41, E.51 |

## D.2 Still open

| Question | Why it matters | How it will be settled |
|---|---|---|
| **What is the neoteletext product?** Who writes pages, what the server side is, and who the users are. | This document specifies the receiver and library only. The web-native design implies servers and an authoring workflow that are not specified. | A short product brief, written by the project owner, before milestone M2. |
| Should the receiver read the live page feeds of SRG SSR (EP1 inside JSON) and NOS (Level 1 rows)? | They are the only live sources of a real broadcaster's teletext bytes found (decision E.28, amended 10 October 2026). They are whole pages, not packets, and the NOS form has already converted accented letters. | A decision in Part E, including the broadcasters' terms of use, before any reader is written. |
| Where can raw teletext from German broadcasters, ZDF in particular, be obtained? | German public broadcasters are the main remaining users of Level 2.5, so their data best tests what lies beyond Level 1.5. This matters more now that Levels 2.5 and 3.5 are in scope (decision E.40). Their websites publish only HTML, and ZDF's runs a bot check that must not be circumvented. Archives of the web pages (defgsus `teletext-archive-unicode` and its successors, from 2022) hold Unicode text, not teletext bytes. | A recording from a DVB receiver in Europe tuned to Astra 19.2°E or German DVB-T2. |
| Does double-height rounding alternate its neighbour on every line (the datasheet's wording), or repeat each rounded line (all the emulators)? | Visible only on double-height diagonals | Comparison with photographs of a real SAA5050 (REQ-TST-030). E.13 follows the emulators until then. |
| What are G2 diacritical marks 4/9 and 4/C exactly? | Rarely used; the references disagree | A real transmission that uses them (REQ-CHR-008). |
| Is the zxnet live feed protocol stable? | It is undocumented and could change | The tolerant reader of REQ-INP-022, plus the recorded session of REQ-TST-063. The site's operator could also be asked. |
| What noise performance should the slicer reach? | The 20 decibel target of REQ-SLC-013 is an estimate | Measurement in milestone M4 |
| How much flash does the core really need on a Cortex-M0? | The 64 kilobyte budget rests on an estimate | The size check of REQ-GEN-009, from milestone M1 |
| Will RFC 6265bis change before it is published? | REQ-WEB-006 follows the draft of 30 September 2026 | Recheck at each milestone's security review, and when it is published as an RFC (decision E.35). |
| Which Xtensa toolchain versions build correctly? | Espressif's fork 1.94 and 1.95 fail release builds (esp-rs/rust#277) | The pinned toolchain of REQ-GEN-001, re-tested when the issue closes. |
| How does the SAA5246A behave where its datasheet is silent? For example, parity errors, serial mode and erase. | It limits how exactly the SAA5246A profile can be reproduced | Decisions E.6 to E.8 define the behaviour. A real SAA5246A-based television, observed with a generated stream, could settle the rest. |
| Why does zvbi not load a synthetic DRCS page, and why does it then drop every X/26 enhancement of the page? | Until this is understood, zvbi cannot be the oracle for DRCS cells (REQ-TST-023) | The investigation that REQ-TST-023 requires, or a real capture with DRCS from the corpus |
| Is the magazine of a Magazine Organization Table link absolute or relative? | Decision E.52 reads it as absolute; a wrong reading sends every link to the wrong magazine | The zvbi comparison of REQ-TST-023, and real Magazine Organization Tables (REQ-TST-025) |
| Is the bit order of the mode identification codes of X/28/3, and the pixel order of DRCS mode 3, as decision E.54 reads them? | No oracle has confirmed either | A real DRCS page of Level 3.5, or zvbi once it loads DRCS pages |
| Should an invocation be skipped when the object number bits of its definition differ from its own? | Decision E.51 does not compare them, because the standard does not require it. Comparing would reject a misplaced pointer. | An owner decision |
| Should the Magazine Inventory Page be interpreted? | It could guide which enhancement pages to keep when the store is full (REQ-ACQ-019). Part G item 12 leaves it uninterpreted. | An owner decision |
| Does Level 3.5 fit the flash budget of REQ-GEN-009 on a Cortex-M0? | The enhancement processor, the G3 glyph data and the larger grid all add to the 64 kilobyte budget | The size check of REQ-GEN-009, from milestone M6 |
| Are the decisions taken while building the macOS browser right? | Decisions E.63 to E.69 include choices marked *(build)*, taken on 12 October 2026 without the owner: among them `sandbox-exec` instead of XPC services, the WebAssembly wall deferred, `q` not quitting, and the BSL-1.0 licence exception. | The owner's review (issue #162); each change is recorded as an amendment in Part E. |
| The browser design's remaining decisions | `prototypes/browser/docs/desktop-browser.md` section 15 decisions 3, 4, 6, 7, 8, 12, 13 and 15 to 18 are recommendations: libraries, the input router, process isolation on macOS, Windows test machines, signing accounts, the WebAssembly wall, iOS as one process, the interface to the phone shells, minimum system versions, an outside audit, and the phone tools. | The owner's decisions (issue #150), recorded in Part E. |
| Should a TTI bundle (`index.tti`) be allowed beside `index.t42`? | Decision E.70 keeps one bundle format to keep one request; authors of TTI pages must publish page files. | Experience with TTI-authored channels. |
| Should declared sister channels on one origin skip the tuning screen? | Decision E.70 shows it between every two channels; a broadcaster with several channels on one server would see it often. | A decision in Part E, if a broadcaster asks. |
| Should the tuning screen flag host names that imitate others? | The concept (section 6) says names that imitate other domains are flagged; nothing does this yet. It needs a source of confusable characters and of well-known names. | A decision in Part E before it is built. |
| Do the Teefax and NMS Ceefax feeds carry packets X/26 and X/28, objects or DRCS pages? | REQ-TST-025 relies on them for real data of these levels, and the feeds send only requested pages (REQ-INP-022) | Captures made for REQ-TST-025 |
