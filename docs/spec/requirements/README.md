# Part F — Requirements

## F.0 Requirement areas

| Area code | Area | File |
|---|---|---|
| GEN | General, language and quality requirements | `gen.md` |
| ARCH | Architecture and programming interface | `arch.md` |
| ECC | Error-protection codes | `ecc.md` |
| PKT | Packet decoding | `pkt.md` |
| ACQ | Page acquisition and the page store | `acq.md` |
| SVC | Broadcast service data (packet 8/30) | `svc.md` |
| CHR | Character sets | `chr.md` |
| L1 | Level 1.0 presentation | `l1.md` |
| L15 | Level 1.5 presentation | `l15.md` |
| L25 | Level 2.5 and 3.5 enhancement: attributes, colours, side panels and characters (section F.9a) | `l25.md` |
| OBJ | Objects, object pages and the links to enhancement pages (section F.9b) | `obj.md` |
| DRC | Dynamically redefinable characters (section F.9c), withdrawn on 20 October 2026: DRCS is not decoded (decision E.95) | `drc.md` |
| NAV | Navigation and viewer functions | `nav.md` |
| RND | Rendering and output | `rnd.md` |
| INP | Input formats | `inp.md` |
| SLC | Software data slicer for raw video samples | `slc.md` |
| PRF | Compatibility profiles | `prf.md` |
| CLI | Command-line program | `cli.md` |
| TST | Testing and verification | `tst.md` |
| DOC | Documentation | `doc.md` |
| FSM | State machines | `fsm.md` |
| WEB | Web-native page sources | `web.md` |
| BRW | The browser (section F.20), withdrawn on 18 October 2026: the browser was removed from this repository | `brw.md` |
| FNT | Fonts (section F.21) | `fnt.md` |
| AUD | Audio: sounds carried with teletext, and their playing (section F.22) | `aud.md` |
| INT | Input: keys, the mouse and text sent back to a channel, and the records it answers with (section F.23) | `int.md` |
| SEC | Security and sandboxing, which overrides every other area (section F.0.3) | `sec.md` |

## F.0.1 Delivery phases

The requirements are delivered in two phases.

**Phase 1** is a complete, correct receiver. It covers every "shall" in this part that has no phase note:
- all the input formats of area INP except those listed below;
- Level 1.0 and Level 1.5 decoding;
- Level 2.5 and Level 3.5 decoding and rendering, brought into Phase 1 at the owner's request on 10 October 2026 (decision E.40);
- the default profile;
- the text, terminal and raster outputs in the 12 by 20 SAA5050 style;
- the basic software data slicer;
- the web-native module (area WEB), including the live WebSocket feed reader (REQ-INP-022);
- the command-line commands;
- channels (REQ-WEB-018 to REQ-WEB-020 and REQ-WEB-022 to REQ-WEB-027), added on 12 October 2026 (decision E.70). The browser added with them was removed from this repository on 18 October 2026, and its requirements were withdrawn (decision E.76);
- writing a channel's contents (REQ-CLI-008);
- the full test suite for all of these.

**Phase 2** holds the optional features and nice-to-haves. A requirement, or part of one, belongs to Phase 2 when it carries a *Phase:* note saying so. Phase 1 requirements carry no phase note. Phase 1 must not depend on anything in Phase 2.

| Requirement | What moves to Phase 2 |
|---|---|
| REQ-ACQ-006 | Request mode (SAA5246A-style acquisition slots). Phase 1 uses cache mode only. |
| REQ-NAV-004 | Automatic pre-fetching of linked Fastext pages. It depends on request mode. |
| REQ-PRF-002 | The "SAA5050" compatibility profile. |
| REQ-PRF-003 | The "SAA5246A" compatibility profile. |
| REQ-RND-003 | The 12 by 10 SAA5246A-style raster mode. It belongs to that profile. |
| REQ-PRF-004 | The alternative options: strict row setting, single Start Box, exact framing code, and ignoring out-of-order triplets. |
| REQ-L15-004 and REQ-L15-009 | The strict row-setting mode and the strict option for out-of-order triplets. The default behaviours stay in Phase 1. |
| REQ-L1-020 and REQ-NAV-007 | Page zoom, and momentary (SAA5050-style) reveal. Normal, mix and boxed modes and latched reveal stay in Phase 1. |
| REQ-CHR-004 and REQ-RND-010 | Glyphs for Cyrillic, Greek, Arabic and Hebrew. The code tables and Unicode mappings stay in Phase 1, and missing glyphs are drawn as visible placeholders. The glyphs of every G3 position moved to Phase 1 on 10 October 2026 (REQ-CHR-007 and REQ-RND-037). |
| REQ-RND-009 | Terminal double-height line escape sequences. |
| REQ-INP-016 | The writer for edit.tf and zxnet page addresses. The reader stays in Phase 1. |
| REQ-INP-017 | Detection of repeated double-height rows in BBC Microcomputer dumps. |
| REQ-INP-021 | The other formats: zvbi sliced files, TTA, ivtv, SMPTE RDD 8 and ST 2031, FAB and Cebra. |
| REQ-SLC-006 | The slow phase-tracking loop. The Phase 1 slicer uses a fixed bit period with phase taken from the run-in. |
| REQ-SLC-011 | High-frequency loss compensation and echo cancellation. |
| REQ-CLI-004 | The interactive terminal viewer. |
| REQ-SLC-015 | The fixed-point form of the slicer. |
| REQ-WEB-005 | The long-polling fallback. WebSocket and server-sent event push stay in Phase 1. |
| REQ-GEN-018 | The optional C interface crate. |
| REQ-WEB-017 | Device-bound sessions. |
| REQ-SEC-017 and REQ-SEC-018 | Self-sandboxing and privilege separation on Windows, OpenBSD and macOS. Linux stays in Phase 1. |
| REQ-SEC-019 | The example WebAssembly host. The WebAssembly builds stay in Phase 1. |
| REQ-SEC-021 | Guidance on hardware isolation for microcontroller ports. |
| REQ-RND-026 | The glyph line cache. |
| REQ-AUD-001 to REQ-AUD-021 and REQ-CLI-011 | The audio profile: sounds carried on independent data lines, over a network or in a file, and played on four channels (decisions E.84 to E.86). |
| REQ-INT-001 to REQ-INT-008, REQ-INT-101 to REQ-INT-116, REQ-CLI-012 and REQ-CLI-013 | The input system: input sent back to a channel over its WebSocket, the channel's cursor and control records, and a phone as the keyboard (decisions E.90, E.91, E.93 and E.100). |
| REQ-CHR-012 to REQ-CHR-016 and REQ-CLI-010 | neoteletext's own character sets for about 55 more languages, the joining of Arabic-script letters, Thai and Lao, and writing pages in these sets (decision E.78). |

Tests for a Phase 2 feature are delivered with that feature.

## F.0.2 Milestones within Phase 1

Phase 1 is large, roughly 100 to 150 person-days, so it is delivered in milestones. Each milestone ends with working, tested software that someone can use. This follows the lesson of the earlier jellytext project, which stalled before anything worked end to end (decision E.30).

| Milestone | Delivers | Main requirements |
|---|---|---|
| **M1 — First light** | Set up the layers of `ARCHITECTURE.md` and their enforcement (REQ-ARCH-011). Read T42 and TTI files. Decode Level 1.0 and Level 1.5 into the cell grid. Write text, terminal and PNG output, and show pages in the terminal as exact pixels. The exhaustive error-code tests, the Level 1 and Level 1.5 tests, and the real-broadcast corpus decoding. | ECC, PKT, ACQ (cache mode), CHR (Latin), L1, L15, FSM-003, FSM-004, FSM-006, FSM-007, RND-001, RND-002, RND-004 to RND-024 (the raster, text and terminal renderers, the glyphs, rounding and the image writers), RND-029, INP-002, INP-013, CLI-001 (`list`, `text`, `render`, `show`), TST-001 to TST-022 |
| **M2 — Web** | Fetch TTI and T42 pages over HTTP with caching, push over WebSocket and server-sent events, cookie sessions, and the zxnet live feed. | WEB-001 to WEB-009, WEB-013 to WEB-016, INP-022 |
| **M3 — Broadcast** | Digital television transport streams, Linux sliced data, and the remaining page formats. | INP-003 to INP-011, INP-014 to INP-020, SVC |
| **M4 — Raw video** | The software data slicer and its synthetic signal generator. | SLC, FSM-002, TST-060, TST-061 |
| **M5 — Forms, links and viewer** | Interactive forms, form input, links (Appendix E), navigation and the viewer state machine. | WEB-010 to WEB-012, NAV (including history, REQ-NAV-010), FSM-005 |
| **M6 — Levels 2.5 and 3.5** | Decode and render Presentation Levels 2.5 and 3.5 completely: the packets of both levels, storage of enhancement pages, the Colour Map, non-spacing attributes, row and screen colours, side panels, objects and default objects, DRCS, G3 glyphs, font styles and flash, with the presentation level setting and its option. The tests include the worked examples of the standard, authored streams with golden rasters, zvbi as oracle at both levels, real data, and fuzzing of every new parser. | PKT-010 to PKT-018, ACQ-016 to ACQ-022, L25, OBJ, DRC, CHR-007, CHR-011, RND-030 to RND-041, PRF-006, CLI-007, INP-023 to INP-025, FSM-011, FSM-012, TST-015 to TST-019, TST-023 to TST-025, TST-042 |
| **M7 — Channels** | Channels in folders with and without `channel.json`, contents files, aliases and redirects within the channel, and `neottx contents`. A windowed browser was built on 12 October 2026 as part of this milestone (decisions E.63 to E.69); it was moved out of the engine on 16 October 2026 and removed from this repository on 18 October 2026 (decision E.76), and its requirements were withdrawn. | WEB-018 to WEB-027, CLI-008 |

M1 is expected to take 40 to 50 person-days. The order of M2 to M5 may be changed; each depends only on M1. M6 depends on M1, and on M2 for the captures from the live feed that REQ-TST-025 uses. M7 depends on M2 and M5.

Where Part E mentions a Phase 2 feature (the SAA5246A profile, or a strict option), the default behaviour it describes is the Phase 1 behaviour.


Every milestone ends with the security review of REQ-SEC-023.
