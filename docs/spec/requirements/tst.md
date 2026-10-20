# F.16 Testing and verification (TST)

**REQ-TST-001 — Test framework.**
Tests shall be written in Rust and run with `cargo test`:
- unit tests inside each crate;
- integration tests in each crate's `tests` directory;
- property-based tests with `proptest` where a property can be stated, such as encode-then-decode round trips;
- snapshot tests with `insta` for text and terminal output.

Tests of the `no_std` crates shall run on the host, and, where REQ-GEN-015 says so, on emulated targets. External tools used as reference oracles are:
- zvbi;
- vhs-teletext;
- vbit2;
- QTeletextMaker.

These tools shall be invoked as separate programs at test time. Their source code and test files shall not be copied into the repository.
*Verification:* Inspection.

**REQ-TST-002 — Exhaustive Hamming 8/4 test.**
A test shall check all 256 byte values against the decoding rules of clause 8.2. It shall confirm that:
- each of the 16 valid codes decodes to its value;
- every single-bit error of every valid code is corrected;
- every double-bit error is reported as uncorrectable.
*Verification:* Test.

**REQ-TST-003 — Exhaustive Hamming 24/18 test.**
A test shall confirm that:
- every one of the 262,144 18-bit values encodes and decodes back to itself;
- all 24 single-bit errors of each are corrected;
- a large random sample of double-bit errors is reported as uncorrectable.
*Verification:* Test.

**REQ-TST-010 — Packet and header vectors.**
Tests shall decode headers, rows, links and service data packets built by the encoder and taken from real captures, and check every field.
*Verification:* Test.

**REQ-TST-011 — Character mapping against a reference.**
A test shall compare the Unicode mapping of every character set and national option with the reference output published with zvbi (its `unicode-out-ref.txt`), running zvbi's own program at test time where possible. Every difference shall be explained in the test source or fixed.
*Verification:* Test.

**REQ-TST-012 — Order of the national option bits.**
Tests shall pin down the ordering of decision E.21 using:
- captured broadcast pages whose language is known, such as a German page showing Ä, Ö and Ü at 5/B to 5/D;
- the result of a TTI file written by vbit2 with a known page status value.
*Verification:* Test.

**REQ-TST-013 — Level 1 attribute tests.**
Tests shall cover every spacing attribute, including set-at and set-after timing, and every example in Annex G:
- the boxing example of G.3.1;
- the separated mosaics example of G.3.2;
- the hold mosaics example of G.3.3.

They shall also cover these combinations:
- hold with size changes;
- hold with mode changes;
- double height with its ignored lower row;
- conceal ended by a colour attribute;
- New Background after a colour change;
- blast-through with national options.
*Verification:* Test.

**REQ-TST-014 — Level 1.5 tests.**
Tests shall cover:
- every triplet mode in both row-setting settings;
- the "@" special case;
- every diacritical mark;
- data values below 2/0;
- missing designation codes;
- the Termination Marker and the triplets after it;
- X/26/15 under each profile;
- each rule of decision E.5.
*Verification:* Test.

**REQ-TST-015 — Level 2.5 and 3.5 enhancement tests.**
Tests shall cover, at both Level 2.5 and Level 3.5:
- every row mode of Table 28 and every column mode of Table 29, with each class of data value, including reserved values;
- every rule of decisions E.43 to E.50, each in a test of its own;
- every requirement of area L25.

Each test page shall be built with the authoring builder of REQ-INP-025, so that the test states the page by function.
*Verification:* Test; the traceability table of REQ-DOC-003 shall link each decision rule to its test.

**REQ-TST-016 — The worked examples of the standard.**
Tests shall reproduce every worked example and truth table of EN 300 706 that concerns Levels 2.5 and 3.5:
- the three contention examples of Annex C.2, which give red, blue and yellow;
- the object example of clause 13.9, for each of the Active, Adaptive and Passive types, with the word colours of its notes 1 to 8. The example's "pink" shall be CLUT 2 entry 0 (red 15, green 0, blue 5), because the standard does not give its index;
- all rows of Table C.1;
- the precedence lists for screen and row colour of Annex A.5;
- the three side-panel column layouts of clause 12.5.3;
- the open area of Figure A.1;
- the default values of Table 30.
*Amended 20 October 2026 (decision E.95):* DRCS is not decoded, and the parts of this requirement that concerned it are removed.
*Verification:* Test.

**REQ-TST-017 — Object tests.**
Tests shall cover every requirement of area OBJ and every rule of decisions E.51 to E.53, including:
- every source and type of invocation, and every permitted and forbidden pair of invoker and type;
- every case in which a pointer, a definition or a link is unusable;
- each limit of Annex A.8, reached and exceeded;
- the reading of the Magazine Organization Table at both levels, with each Level 3.5 packet missing in turn;
- default objects, with and without local display data, and the order of placement;
- objects placed at every edge of the page and of the side panels.
*Verification:* Test.

**REQ-TST-018 — Withdrawn: DRCS tests.** Withdrawn on 20 October 2026: DRCS is not decoded (decision E.95).

**REQ-TST-019 — Bounded work and memory of enhancement processing.**
Tests shall build worst-case pages: every object type invoked up to the limits of Annex A.8, nesting to the greatest depth allowed, objects of 507 triplets, all sixteen X/26 packets and full side panels. They shall check that building the grid stays within a stated bound on operation counts (REQ-SEC-012), uses only the memory the caller provided, and never panics. The performance tests (REQ-TST-050) shall report the time to build such a grid at Level 3.5.
*Verification:* Test.

**REQ-TST-020 — Round-trip tests.**
For each page file format, a test shall read a set of pages, encode them to packets, decode the packets, write the pages back in the same format, and confirm that the result matches the original.
*Verification:* Test.

**REQ-TST-021 — Comparison with other decoders.**
A test shall decode the same inputs with this project and with zvbi and vhs-teletext, and compare the results. The inputs are the T42 and transport stream files of the real-broadcast corpus (REQ-TST-022). The comparison covers:
- the set of pages found;
- their text content, row by row.

Differences shall be listed and explained.
*Verification:* Test.

**REQ-TST-022 — Real-broadcast test corpus.**
The tests shall use a corpus of real broadcast teletext. It shall be downloaded at test time and not stored in the repository, and each file shall be pinned by its checksum. The corpus consists of:

- **Off-air digital television recordings** from the FFmpeg sample archive, at `https://samples.ffmpeg.org/ffmpeg-bugs/trac/ticket2086/`:

  | File | Size | Teletext track |
  |---|---|---|
  | `teletextsubtitles.ts` | about 96 megabytes | Teletext track whose descriptor lists Italian and English entries |
  | `France2.m2ts` | about 109 megabytes | French teletext in 192-byte packets |
  | `dreambox-2.ts` | about 98 megabytes | English teletext |
  | `RBT_20100801_1835.ts` | about 47 megabytes | English teletext |

  Each was confirmed on 1 October 2026 to contain a DVB teletext track. The same folder's `Dreambox-1.ts` and `IRIB602MHz.ts` should be checked and added if they carry teletext. Tests may use only the first few megabytes of each file, fetched with ranged requests, to keep test runs short.
- **Recovered service snapshots in T42 form** from the Teletext Archaeologist archive (`https://archive.teletextarchaeologist.org`). The selection shall cover:
  - several broadcasters;
  - several decades;
  - both clean and noisy recoveries.
- **Recent off-air recordings** from the TSDuck stream repository (`https://tsduck.io/streams/`). Each carries full teletext services, not only subtitles, as its published analysis file shows. They are the newest real broadcast packets found (research record: `research/answers-teletext-sources.md`).

  | File | Recorded | Teletext services |
  |---|---|---|
  | `poland-dttv-warsaw/PL_DVB-T_2023-11-10_MUX3_522MHz.ts` | 10 November 2023 | TVP1 and three other TVP services |
  | `netherlands-dttv/digitenne-482-2020-06-21.ts` | 21 June 2020 | NOS Teletekst on NPO 1, 2 and 3, and two more |
  | `italy-sardinia-dttv/mux1rai.ts` | not stated | RAI Televideo |
  | `hotbird-13.0E/hotbird130E-ts6000-2018-05-11.ts` | 11 May 2018 | Mediaset |

  The same rules on ranged requests apply. The repository's licence shall be checked and recorded before the files are used.
- **Restored pages with verified checksums** from the teletext-restoration repository (`https://github.com/teletext-restoration/teletext-restoration`). The restored single-page T42 files whose names carry neither `-maybe` nor `-nocrc` match the page checksum that was broadcast with them. They shall be used for the page checksum test of REQ-ECC-007. The full recordings behind them may be fetched only one at a time, as the maintainers ask.
- **The raw sampled captures published with vhs-teletext**, for the slicer (REQ-TST-061).

- **A local corpus of recovered broadcasts.** The workspace already holds T42 recoveries made by Dan Farrimond from VHS recordings, in `~/devel/jellytext/dan/`:
  - BBC1, 31 October 1991;
  - TV-am, 20 October 1992;
  - The Children's Channel, January 1993 (several captures);
  - Sky One, 16 February 1993;
  - GMTV, 22 February 1993.

  There are full and "squashed" versions, where squashed means the duplicate sub-pages have been merged. The tests shall read local corpus files from a directory given by the environment variable `NEOTELETEXT_CORPUS`, and shall also keep a local cached copy of every downloaded corpus file there. The tests then do not depend on outside websites being available. Corpus files shall never be committed, because the page content belongs to the broadcasters.

Expected results for the corpus are the outputs of zvbi and vhs-teletext (REQ-TST-021), plus page counts and spot checks recorded by hand. If a source disappears and no cached copy exists, the test shall report the missing file clearly and skip, not fail. A test run that skipped corpus files shall say so in its summary, so that lost coverage is visible.
*Verification:* Test.

**REQ-TST-023 — zvbi as the oracle at Levels 2.5 and 3.5.**
A test shall decode the same streams with this project and with zvbi 0.2.44 at its Level 2.5 and Level 3.5 settings, run as a separate program (REQ-TST-001 and REQ-SEC-022), and compare, cell by cell:
- the character, as Unicode, with zvbi's private value 0xEF00 plus the code mapped to G3 positions;
- the foreground and background colours, as Colour Map indices and not as colours, because zvbi's values for CLUT 1 entries 5 and 6 differ from Table 30 (decision E.43);
- the size;
- the underline, flash, conceal, bold, italic and proportional flags;
- the screen colour index.

The streams shall be the authored streams of REQ-TST-024 and the real data of REQ-TST-025, limited to streams whose triplets keep the order of Annex A.4 (decision E.46). The test shall also check the reading of the magazine of a Magazine Organization Table link (decision E.52), and record the result in that decision. DRCS cells shall not be compared, since this receiver does not decode DRCS and shows the Level 1 character there (decision E.95). Every difference shall be explained in the test source or fixed.
*Amended 20 October 2026 (decision E.95):* DRCS is not decoded, and the parts of this requirement that concerned it are removed.
*Verification:* Test.

**REQ-TST-024 — Authored Level 2.5 and 3.5 test streams with golden rasters.**
A set of test streams shall be authored with the builder of REQ-INP-025. Between them they shall use every triplet mode, every object source and type, Public and Global Object Pages, the Magazine Organization Table, and every variant of X/28 and M/29 that is read; and the DRCS triplets, sent as a service sends them, to show that they are not acted on. Each stream's pages shall be kept as TTI source in the repository, with golden checksums of their rendered rasters in both cell sizes, under the format version rule of REQ-TST-030.
*Amended 20 October 2026 (decision E.95):* DRCS is not decoded, and the parts of this requirement that concerned it are removed.
*Verification:* Test.

**REQ-TST-025 — Real Level 2.5 and 3.5 data.**
The local corpus (REQ-TST-022) shall gain real pages that carry Level 2.5 and 3.5 data:
- captures from the Teefax and NMS Ceefax feeds of REQ-INP-022, recorded through the feed reader into `$NEOTELETEXT_CORPUS`, of pages that carry packets X/26 or X/28, with any object and Magazine Organization Table pages they use;
- the Polish TVP pages with packets X/26 in the recording already listed in REQ-TST-022.

These files shall never be committed. The tests shall decode them at Levels 2.5 and 3.5, compare them with zvbi (REQ-TST-023), and skip clearly when a file is absent.
*Verification:* Test.

**REQ-TST-030 — Reference images.**
Golden raster images shall be kept for:
- the Level 1.5 example pages published with QTeletextMaker (for example `Level1p5-Defcon.tti`, `Level1p5-KLF.tti` and `Level1p5-PleaseHammer.tti`), downloaded at test time;
- the edit.tf engineering test card;
- test pages written for this project that exercise every attribute and mosaic form.

Golden images and golden cell-raster outputs shall carry a format version number. A change to rendering that alters pixels shall raise the version and regenerate the golden files in the same change, with the reason recorded, so that stale golden files are never mistaken for regressions. This is a lesson of Doom's recorded demos, which silently went out of step between program versions.

The SAA5050-style rendering, including rounding and separated mosaic geometry (decisions E.13 and E.14), shall be compared by eye against reference images of real SAA5050 output from a documented emulator or photographs. The findings shall be recorded.
*Verification:* Test and inspection.

**REQ-TST-040 — Fuzz testing.**
Every reader, the packet decoder, the slicer, and the HTTP, WebSocket, server-sent event, descriptor, form and address (REQ-WEB-016) parsers of the web module shall have a fuzzing target for `cargo fuzz` (libFuzzer). The targets shall run in continuous integration for a fixed time on each change, and for longer periods on a schedule. Every input that has ever found a fault shall be kept in the repository as a regression test.
*Verification:* Inspection; continuous integration logs.

**REQ-TST-041 — Coverage.**
The core library tests shall reach at least 90 percent line coverage.
*Verification:* Coverage report.

**REQ-TST-042 — Fuzz targets for the data of Levels 2.5 and 3.5.**
Each new parser and processor of Levels 2.5 and 3.5 shall have a fuzzing target under REQ-TST-040:
- packets X/27/4 and X/27/5, X/28/0, X/28/3 and X/28/4, and M/29/0 and M/29/4;
- the packets of object pages, including pointer tables;
- the packets of the Magazine Organization Table;
- the enhancement lines of TTI files (REQ-INP-023);
- the whole grid build at Level 3.5, from a displayed page and a set of linked object pages and a Magazine Organization Table, all built from the fuzzer's input.

The last target shall also check that the work stays within the bound of REQ-TST-019, and that row 0, columns 0 to 7, are never written (REQ-L25-050).
*Verification:* Inspection; continuous integration logs.

**REQ-TST-050 — Performance tests.**
Tests shall measure the throughput requirements of REQ-GEN-013 and report the results.

They shall also report hardware-independent drawing costs on the build machine, with and without the glyph line cache of REQ-RND-026:
- cell-line draws per second;
- the average number of machine instructions per cell-line draw, where the platform's performance counters allow it.

These figures let each port estimate its own budget. Redrawing every row of every field, for example, takes 500,000 cell-line draws per second.
*Verification:* Test.

**REQ-TST-060 — Synthetic signal tests.**
A test signal generator shall produce sampled teletext lines to EN 300 706 at each sample rate of REQ-SLC-002, with controllable:
- level;
- black-level offset;
- timing;
- bit-rate error;
- high-frequency loss;
- echo;
- noise.

It shall be used to verify REQ-SLC-002 to REQ-SLC-013.
*Verification:* Test.

**REQ-TST-061 — Real capture tests.**
The slicer shall be tested on the sample captures published with vhs-teletext, downloaded at test time, and compared with that tool as REQ-SLC-014 describes. The packets it recovers shall then be decoded in full, and the resulting pages compared with those vhs-teletext produces from the same capture.
*Verification:* Test.

**REQ-TST-063 — Live feed soak test.**
A scheduled test, run outside the normal test suite, shall connect to each live feed of REQ-INP-022 for at least one hour and walk the whole page list. It shall check:
- that no crash, memory growth or unbounded delay occurs;
- that every page in the feed's page list is eventually received;
- that the cross-check of REQ-INP-022 between the decoder's header fields and the server's decoded fields finds no disagreement.

A session shall also be recorded to a file so that it can be replayed in the normal test suite.
*Verification:* Test on a schedule; recorded results.

**REQ-TST-070 — Withdrawn (hardware demonstration).**
*Withdrawn on 1 October 2026.* It depended on specific hardware (a Raspberry Pi inserter and a television), which is outside the library (REQ-ARCH-010). The number is kept and will not be reused.

**REQ-TST-080 — Continuous integration.**
Continuous integration shall run on every change:
- the target matrix of REQ-GEN-015;
- the minimum-version and current-stable builds (REQ-GEN-001);
- `rustfmt` and `clippy` (REQ-GEN-006);
- the feature combinations (REQ-GEN-010);
- the Miri runs (REQ-GEN-006);
- `cargo-deny` (REQ-GEN-017);
- the size budget (REQ-GEN-009);
- the fuzzing runs (REQ-TST-040).
*Verification:* Inspection.

**REQ-TST-081 — End-to-end tests of the browser against local channels.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.
