# Technical debt

Work deferred while behaviour is being shaped (the owner's instruction of 13 October 2026: iterate fast, and add tests, documentation and detail once the software works as expected). Each item says what is owed and why it was left. Remove an item when it is done; move it to a GitHub issue when it needs the owner.

## Verification

- **Issue 221 (`cargo xtask features` failing builds) is to be closed once the full gate confirms it:** the reorganisation's branches ran the whole matrix at their ends (46 builds on the core's branch, 41 on the others) and every row passed, `neoteletext-formats [std,tti]` included, on the untouched tree and after (checked on ad51bbc on 19 October 2026, `neoteletext-core [modern-classic]`, `[classic]`, `[render]` and `[bedstead]`, `neoteletext-formats [std,tti]` and the CLI builds without `write-text` had failed).

- **The full gate has not run** since 67153ff: c884152, c6b3d40, 30dc865, a5651d6, 9c989ba, b9f8ef6, f43fde7, f9fe7ea and f341e94 passed only the fast gate. Run `NEOTELETEXT_REMOTE=foundintelquiet4060 scripts/check.sh full` (the remote half needs the host set up in this session; Xtensa builds may need `~/export-esp.sh`).
- **Cortex-M0 sizes not remeasured** (REQ-GEN-009, `docs/spec/design/porting.md`): `GlyphSet` now holds 25 font slots, and loaded fonts can be rounded. `cargo xtask size` on the remote host.
- **Fuzzing not rerun** on the changed readers: `bdf`, `bdf_convert`, and the `fonts.txt` reader with `c` lines, which has no fuzz target of its own.
- **Renderer cost with per-set fonts** (issue #216) not measured: `row_renderer_cost_with_a_loaded_font` before and after.
- **No test that frame kind letters are distinct**: `FONT_TABLE` first took `b'k'`, already `INPUT`'s; only a compiler warning caught it.
- **No end-to-end test of two character sets on one real page** drawn from two user fonts; the core test covers it.

## The source reorganisation (24 October 2026)

What the reorganisation of `IMPLEMENTATION-LOG.md` row 88 left (the record is `docs/spec/design/reorganisation-2026-10-24.md`). Measured on the integrator's branch by brace matching from each `fn` or `impl` line to its closing brace.

- **The full gate has not run on the reorganised tree:** `scripts/check.sh full`, with the Linux host's stage and the Cortex-M0 `cargo xtask size` there, is to run once after the security-fix branches merge (the coordinator's instruction). No branch measured the Cortex-M0 image; each measured `x86_64-unknown-none` only.
- **Functions still over 80 lines** (`00-conventions.md` section 1), the tables and two tests only: `neoteletext-slicer` `fsm.rs` `slicer_transition` 157 (the REQ-FSM-002 table) and `xtask` `g3/table.rs` `shape` 108 (one arm per position of Table 48), each with its reasoned `too_many_lines` allow; and the tests `neoteletext-core` `enhance/tests.rs` `every_transition_of_the_walk` 99 and `neoteletext-slicer` `tests/reports.rs` `report_impairments` 92, left as they are because tests are moved, never rewritten. Every other function the reorganisation left over the limit, and the two the security fixes took past it (`agent/response.rs` `redirect`, `qrkeyboard` `screen/messages.rs` `binary`), was split on branch `structure/functions`; no `impl` block is over 300 lines and no field is widened against "fields stay private" (section 6) any more.
- **The render tests not redistributed** (document 10 step 11b): `render.rs`'s tests are `render/tests.rs`, and the old `level25_tests.rs` and `font_tests.rs` are `render/tests/level25.rs` (847 lines) and `render/tests/fonts.rs`, whole, because the three modules each define an `options` helper of a different signature and putting them by concept would mean renaming, that is rewriting, tests. The same for the charset's two test modules (`charset/tests.rs` and `charset/tests/tables.rs`, both defining `latin`). Redistributing them is a test change of its own.
- **The shared network client deferred to the owner** (document 13 section 3.7, pairs 1 to 7): `neoteletext-app` `fetch/transport.rs` and `neoteletext-qrkeyboard` `net/{stream,websocket}.rs` each hold a socket reader, a TLS pump, a proxy connect and `write_records`, kept in step by cross-reference comments. Joining them needs a new `std` transport crate, a `std` module of `neoteletext-web` exempt from `check-io`, or `qrkeyboard` depending on `app`: a crate, a dependency or a rule, which is the owner's decision. NeoTeletext's fetcher (`native/fetcher/src/transport.rs`) is a third copy.
- **The optional steps were all taken:** the formats crate's shared bounded line reader (document 11 step 14) is `lines.rs`, merged from `fix/formats` (2b2c895); whether its `formats` and `tti_enhancement` fuzz targets were rerun is not recorded there. The `ws/` split (document 12 step 12) and the shared directory walker of `trace.rs` and the checks (document 13 pair 23) are on branch `structure/functions`.
- **`cargo xtask features` has no row** for `neoteletext-slicer`, `neoteletext-image`, `write-text` alone in the app and the CLI, `neoteletext-audio-mix` or `neoteletext-qrkeyboard` (`30-cross-cutting.md` section 3.3 item 4, the owner's decision); the branches built those by hand.
- **Static memory of the `web-all` image** went from 107,345 bytes of flash and 3,256 of static RAM to 106,073 and 4,520 with the web agent's shared forms (document 12 steps 8 to 10): about 1,270 bytes moved from flash to static memory. The cause was not looked into.
- **`DEVELOPMENT.md` section 6** names the no-panic list's lints without `string_slice`, which every engine list now has (finding F-105); the common repository's own process changes it.
- **`docs/teletext-specification-audit.md` row 66** cites `render.rs:393` for bold and italic, which decision E.96 removed before the reorganisation; the audit is dated and was left as written.

## Unfinished features

- **Audio, area AUD (issue #220):** built and tested on branch `audio`; the `.ttau` and WebSocket readers, the fades at a lost block and the `concealed` report were built on 28 October 2026 (decision E.131). Owed:
  - live sounds (REQ-AUD-014) are not measured against a real sender's clock, only against a simulated 1 per cent;
  - passing IMA ADPCM blocks undecoded to a sink that decodes them itself, for tellyboy (REQ-AUD-018);
  - `cargo xtask size` and the Cortex-M0 flash of a receiver with audio, now with the microcontroller shape (`pcm16`, `ima`, `idl`) and the desktop shape (`idl`, `opus`) as separate images;
  - merging `audio` into `implementation` once the other session's work there is committed, and the full gate;
  - decision E.131 items 3 to 5 (the readers, the fades and the report) for the owner to confirm;
  - the `concealed` report of a stored sound follows the play by the receiver's clock, not the sink's: a sink's output latency, or a port that ticks late, moves the judgement by that much. A stored Opus sound's decoding that goes on after a gap writes samples the voice may meet mid-block, unfaded; a fade there needs the sink's position, which `VoiceSink` does not report (`design/audio.md` section 10.1 asks for it);
  - NeoTeletext (`native/`) builds `neoteletext-audio` with `idl`, `ima` and `opus`: it must drop `ima` (and never add `pcm16`) for the desktop shape. A workspace build there that also builds a crate with `encode` (its `site`, and the decoder's tests) unifies `ima` back into the decoder, since `encode` needs it; the release decoder must be built on its own (`-p`) or the `encode` users kept out of its build;
  - `fuzz/Cargo.lock` gains `neoteletext-opus` when the fuzz crate is next built (below), and the new targets `audio_file` and `audio_messages` have no seed corpus.

- **Epics #177 (fonts) and #210 (character sets)** stay open until their work and the full gate are done.

- **The 4:3 aspect correction stretched the wrong way** (found and fixed in rapid iteration): the browser's "4:3 picture", `neottx --aspect` and the image writers made pixels 1.2 times taller; on a 4:3 PAL set each pixel of the 12-by-20 cell is wider than tall (a half-dot is 1/624 of the width, a line 1/768). The core's `ASPECT_NUMERATOR`/`ASPECT_DENOMINATOR` are now 16/13 and stretch the width; `neoteletext_image::scale` maps columns back through the stretch; the browser uses the core's ratio; the user guide's `--aspect` line is corrected. Owed: REQ-RND-004 still says "pixels about 1.2 times taller than they are wide" and needs amending, with a decision citing the timing; a regression test that fails on the old ratio (the scale tests were changed with the fix; confirm they fail on 6/5 height); check images and goldens made with `--aspect` anywhere in the docs or tests; mention it in the log. "4:3 picture" is now on by default in the browser (owner, 13 October 2026); `neottx --aspect` is still off by default; REQ-RND-004 and the settings test should say which is the default.

- **The command-line program is renamed `neottx`** (owner, 14 October 2026: "rename ttx to neottx everywhere"). The binary (`crates/neoteletext-cli/Cargo.toml`), its clap name, every message prefix, the tests (`CARGO_BIN_EXE_neottx`), the oracle and test-server scripts, REQUIREMENTS.md, ARCHITECTURE.md, CLAUDE.md, the user guide and the other documents now say `neottx`. Kept, because they are not the program's name: the form fields `ttx_token`/`ttx_action` and the C interface prefix `ttx_` (REQ-FFI), zxnet's `ttx,true` channel message and its ttx export format, CEBRA's `.ttx` extension, zvbi's `ttx_page`, DVB's TBttx/Bttx, example URL paths `/ttx/`, and third_party/. `docs/teletext-specification-audit.md` (untracked, not mine) still says `ttx`. Verified: `cargo check --workspace --all-targets` clean, `cargo test -p neoteletext-cli` passes (31 tests). Owed: a decision in Part E recording the rename; an issue; the full gate; the IMPLEMENTATION-LOG rows were renamed in place, so the log no longer shows the old name (add a row saying when it changed); the teletext-reference tooling calls `target/debug/neottx`.

## The specification split into `docs/spec/` (18 October 2026, decision E.83)

`REQUIREMENTS.md` is now an index, and the specification is in `docs/spec/`. The text moved unchanged; the language decisions E.77 to E.82 and the explainer's language section were cut to pointers to `docs/spec/design/languages.md`. `cargo xtask traceability` reads the new files (424 requirements, regenerated). Owed:
- commit, on its own;
- reword the older items of this file, `IMPLEMENTATION-LOG.md` and the code comments that say "REQUIREMENTS.md Part E" or "section F.n" as the new paths, where the index is not enough;
- `docs/neoteletext-browser-open-questions.md` concerns the browser and belongs in `prototypes/browser/docs/`;
- `docs/teletext-specification-audit.md` (untracked, not mine) is unplaced.

## Decisions made on the owner's instruction (18 October 2026)

The owner asked the designer to decide the open questions, except that the image converter gets no `--drcs` option. Each is to be recorded in `docs/spec/decisions.md` at integration.

1. **The new character sets go to integration** as they now behave.
2. **Tatar keeps Щ at 2/7 and щ at 3/E.**
3. **REQ-CHR-013 stays as written:** one data file per set, a generator for the Rust tables, and a gate check that each language's CLDR letters are in its set, with the CLDR data pinned by checksum. Work owed.
4. **The TTI writer sends Latin English as the second set** for neoteletext's own designations from an `RE` line (done: `second_g0_value` in `neoteletext-formats/src/encode.rs`, `DESIGNATION_LATIN_ENGLISH` in the core's constants, also used by `write-text`). Test owed: a page written with `RE` in a new set, read back, keeps ESC to English.
5. **The writer's Level 1 stand-ins** (Đ as D, Ŋ as N and eight more) stay; record them in the design section 9.
6. **A loaded font draws its own mark over its own letter** where no precomposed glyph exists (amends decision E.26), so Yoruba ẹ́ matches its neighbours. Work owed.
7. **Text read back from Thai and Lao pages** gets a pass that joins each mark row to the letter row below it. Work owed.
8. **Persian's zero-width non-joiner stays a space:** a private "no join" code would leave the standard's mechanisms (decision E.78 item 2).
9. **`write-text` does not offer English with Thai as the second set;** the receiver decodes such pages.
10. **The non-spacing attributes stay off by default;** `neottx` gets no separate classic build; `forms` stays out of the classic profile.
11. **`neottx --aspect` stays off by default:** image output keeps the square pixels of the cell raster.
12. **The browser is removed from this repository** (owner): its requirements withdrawn and its code and prototype deleted; it is maintained in `~/devel/neoteletext-browser`.
13. **The private-use forms are allocated append-only** from now on, so that adding a letter never renumbers a form; `scripts/arabic-sets.py` to keep a list of allocated forms.
14. **Font collection** (`~/devel/teletext-reference/teletext-fonts/`, outside this repository):
    - the ST92195 10 × 10 font (Figure 79) and the TI Instant Text 8 × 10 GEN0 font become fonts by padding each glyph, centred, to 12 × 10: every hardware pixel kept, none invented or scaled; the letters sit wider apart than on the real chips, and each font's notes say so;
    - NationalChip: the ROM's low line is blank; the base fonts keep it blank as the hardware held it, and `nationalchip-gx3113c-completed` keeps the constructed full-width low line;
    - the hand-drawn characters in `catalog/patches/`, Bedstead complete's ™ among them, are accepted as drawn; they have not had the owner's review.
15. **The image converter at Level 3.5 keeps CLUT 0 entries 0 and 7 black and white**, so row 0's header stays white on black; 29 colours are fitted (example 04: error 800 against 795).
16. **Not decided: issues #187, #188, #189 and #201.** The GitHub account active on this machine (`starqueue`) cannot see crowdwave/neoteletext-engine, so they could not be read. Log in with an account that has access (`gh auth login`).

**Found:** the authored Level 2.5 goldens (`golden_level25`) depend on the core's `nonspacing-attributes` feature: `cargo test -p neoteletext-formats --all-features` alone leaves it off and fails 320 of 494 rasters, while the workspace run with all features passes. Make the test say which features it needs, or skip itself without them.

## Single-byte languages (18 October 2026)

**Phase 1 done (18 October 2026):**
- 12 × 20 loaded glyphs: `FontGlyph` and `LoadedGlyph` carry 20 rows and a height, a font is all one height, and 12 × 20 glyphs are drawn line for line and never rounded;
- the BDF reader, writer and converter take 12 × 20, and the pipe's glyph table carries the height;
- tests for each;
- `third_party/noto/` with the converter, sources, settings, licence, provenance and `noto-teletext.bdf` (all 24 standard sets conform);
- `cargo xtask check-noto` in the gate;
- `neottx render --font FILE`.

Phase 1 owes:
- a requirement for `render --font` (area CLI) and its line in the user guide;
- REQ-FNT-012's default library: `neottx` and the app do not yet offer `third_party/noto/` as a library;
- the browser prototype does not build against the new `FontGlyph`;
- fuzzing of `bdf` and `bdf_convert` with 12 × 20 inputs;
- `cargo xtask size` with the larger glyph table;
- regenerating traceability and adding an implementation log row.

**Phase 2 done (18 October 2026), uncommitted until the gate passes:**
- the 18 sets of groups 0101, 0111 and 1011 and the Vietnamese and Armenian G2 sets (`charset/g0_extended.rs` and `charset/g2.rs`), designated in Tables 32, 33 and 34 behind the core feature `extended-sets` (on by default; off in `classic` and `modern-classic`);
- Latin English as the second set of those designations when no Table 33 value is received;
- Vietnamese, and Yoruba and Igbo, are whole G0 sets, not national options: an X/26 mark is placed on the base form, never on a national option's character (REQ-CHR-002). The design, E.81 and REQ-CHR-012 to 014 are amended;
- marks from the page's G2 set (`charset::page_mark`): Vietnamese 4/C is the hook above, kept as mark 16, with 76 more compositions (`EXTENDED_COMPOSITIONS`);
- 44 font sets with 64-bit conformance masks, in the core, the pipe (the mask fields are now 8 bytes) and the fetcher;
- `required.txt` lists the new sets (766 characters); `noto-teletext.bdf` draws them, with Georgian, Armenian and Tifinagh sections and the Vietnamese precomposed letters (848 glyphs, all 44 sets conform);
- `neoteletext_app::write_text` and `neottx write-text` for the Phase 2 sets, behind the app's and the program's `write-text` features;
- tests: every Table 32/33/34 value, the default second set, every tone on every Vietnamese vowel, the hook only in the Vietnamese G2 set, the composition tables, the writer's encoding, refusals, line breaking and X/26 budget, and an end-to-end write and read back.

Phase 2 owes:
- **REQ-CHR-013 says each table is written from a reviewed data file checked against CLDR.** The tables are hand-written Rust constants; there are no data files and no CLDR check. Either write the data files and an `xtask` generator, or amend the requirement.
- **Inuktitut syllabics is out of scope** (owner, 18 October 2026): recorded in decision E.78 item 7, out-of-scope item 16, the design section 8.4 and REQ-CHR-012's table. Owed: comment on issue #219.
- **Row 0's receiver columns, fixed in rapid iteration (18 October 2026):** `build_grid` draws columns 0 to 7 of row 0 in Latin with the English option whatever the page designates (`draw_receiver_columns_in_receiver_set`, `level1.rs`), so "P100" reads the same on Cyrillic, Greek, Georgian, Arabic-script and Thai pages and is never joined. Owed: an amendment to REQ-SEC-005 or decision E.10 saying the status area is drawn in the receiver's set, a test that fails on the old code (a Cyrillic page's row 0 columns 0 to 3 are Latin), and an issue.
- **A TTI `RE` line writes "no second set"** in X/28/0, so a page in one of the new sets designated that way loses the default English second set; `write-text` writes English explicitly. Decide whether the TTI writer should do the same.
- **The writer's stand-ins** for G2 letters at Level 1 (Đ as D, Ŋ as N, and eight more) are the writer's own choice; record it in the design section 9.
- `neottx font sets --help` and the user guide still describe 12 × 10 fonts and 24 sets in places; the clap text is fixed.
- The design's REQ-FNT-012 default library, the user guide's language list, traceability, the implementation log row and issue #219.

**Phases 3 and 4 done in rapid iteration (18 October 2026), uncommitted:**
- **Group 1001 (Arabic script):** eight whole sets (Persian, Urdu, Pashto, Sindhi, Sorani, Uyghur, Balochi, Arabic in plain letters), generated by a scratch script from letter lists: Table 44's punctuation in columns 2 and 3 without its mirroring, the language's digits at 3/0 to 3/9, the letters in alphabet order from 4/0, then lam-alef (U+FEFB) and tatweel. The `joining` module (core feature `extended-sets`) joins each row after the level's pass; `CharRef` gains `form`; `charset::glyph_key` gives the code point a font draws; 207 forms, 44 of them private-use from U+F0000. Joining types are from Unicode 16.0 `ArabicShaping.txt`. A joined set conforms only with every form (`Failure::MissingForm`).
- **Group 1100 (Thai and Lao):** Thai G0 and G2 from the design's transcription; 1100 000 (English, Thai second), 1100 010 (Thai) and 1100 100 (Lao); two-mark G2 glyphs keyed at private-use U+F0100 (Thai) and U+F0140 (Lao), with `to_unicode` giving both marks. **The Lao G0 table was transcribed in this session from WST90 Figure 33** (PDF p. 173, hand-drawn, romanised labels): 2/7 (dots, read as …), 5/2 (read as ັ), 5/8 (read as ໍ), 5/D ("NUL"), 6/8 to 6/A (unlabelled) and 7/7 (ຫຼ, no single code point) need confirmation; the unknown ones are spaces. **The Lao G2 set is my own design**, not in any source: Latin G2 with columns 5 and 6 holding the seven upper vowel signs with each of the four tones, then ໌ ຼ ຽ.
- **Fonts:** `noto-teletext-arabic.bdf` (355 glyphs, the eight sets conform) and `noto-teletext-thai.bdf` (332 glyphs: Thai, Lao, both G2 sets and Latin English). The converter reads `crates/neoteletext-core/src/glyphs/forms.txt` (new, generated by `cargo xtask glyphs`), and gained `uses:`, `except:` and `forms:` character selectors. Its Thai and Lao mark drawing had never run and was rewritten (`mark_glyph`). Thai and Lao settings are design section 5's values (weight 800, consonants 10 lines, baseline 15).
- **Writer:** the Arabic-script sets (lam-alef, UAX #9 reordering with `unicode-bidi` 0.3.18, mirrored brackets, rows right-aligned) and Thai and Lao (mark rows, shared lower-vowel rows, two-mark G2 glyphs over the vowel alone, ำ split, Thai digits made Western). 56 font sets in all.
- Tests: joining (word, breaks, lam-alef, hamza, private forms), the missing-form rule, every designation, the writer's Arabic order and Thai layout, and a Persian end-to-end read back.

Phases 3 and 4 owe:
- **Decisions to record:** the group 1001 layout (above); the Lao G0 readings and the Lao G2 set; the private-use allocations (U+F0000 forms, U+F0100 Thai, U+F0140 Lao) as a table in the design; that `to_unicode` of a joined letter is the plain letter and of a two-mark glyph both marks; `charset::glyph_key` versus `to_unicode`; rows of right-to-left pages right-aligned by the writer.
- **Design text:** sections 7.7, 8.3, 8.5 and 14 still describe the tables as not laid out; ARCHITECTURE.md's `joining` row and the `xtask` layer table; the user guide for `write-text`.
- **Checks still to do:** a reader of each Arabic-script language to check its letter list and order (Balochi and Sorani above all); Lao by a reader of Lao; whether Noto Sans Thai's loopless glyphs suit the SAA5246AP/S look; the header's receiver columns were drawn as boxes in Arabic-script, Thai and Lao fonts (fixed, see row 0 above).
- **Not done:** `english-thai` (1100 000) is not offered by `write-text`; text read back from a Thai or Lao page gives the visual rows, marks on rows of their own, not the logical text; no test of the Thai and Lao tables position by position against a data file (REQ-CHR-013's data files are owed for every new set).

**Language measurement and samples (18 October 2026, after 5fff9cf):** against Google Fonts' language data (gflanguages 0.7.11), 581 languages are fully writable and 224 more lack at most four letters; the list and one sample page per language (804, from gflanguages' sample texts) are in `tmp/language-samples/` (not committed; the scripts are in the session scratchpad and should move to `scripts/` or `xtask` if kept). The writer, uncommitted, now also names the standard's 18 sets, writes Hebrew right to left, and writes a combining mark after a letter as that letter with an X/26 mark. Found:
- **Gaps in the new sets, fixed in rapid iteration (18 October 2026):**
  - ё and Ё are compositions of е and Е with the diaeresis (`EXTENDED_COMPOSITIONS`), so every Cyrillic set without them, the standard's Russian included, reaches them by an X/26 mark at Level 1.5 and above; Level 1 shows е, as Russian teletext did. `write-text` writes them that way;
  - Tatar has Щ at 2/7 (was the apostrophe) and щ at 3/E (was >, whose partner < is ң); the design section 8.2 table and a decision are owed;
  - Balochi gains ۏ after و and ࢩ after ی; `noto-teletext-arabic.bdf` redrawn (361 glyphs, all eight sets conform);
  - the group 1001 tables are now written by `scripts/arabic-sets.py` between marker lines in `charset/arabic_script.rs`, with ArabicShaping.txt 16.0 downloaded and checked by SHA-256. Its private-use allocation is by sorted letter, so adding a letter renumbered U+F002B onwards; make the allocation stable (append only) before the private-use table is recorded as a decision;
  - owed: tests (ё by X/26 in a Kyrgyz and a Russian page, Tatar 2/7 and 3/E, Balochi ۏ and ࢩ joined), and re-running the language measurement.
- Kazakh in Cyrillic is not covered (Kazakh is in Latin by the owner's decision).
- **Gaps in the standard's sets, for the record:** Russian ё (Russian teletext wrote е), Greek Ό, German ẞ, Ukrainian ʼ, Romanian ș ț with comma below (the option has ş ţ with cedilla), obsolete Thai ฃ ฅ ฦ.
- **A letter with a tone and no precomposed form** (Yoruba ẹ́, ọ̀) is drawn by Bedstead's overlay even when a loaded font is in use, so it differs in style from its neighbours. Decision E.26 finds loaded glyphs by precomposed form only; overlaying the loaded font's own spacing mark would fix it.
- Persian's zero-width non-joiner is written as a space.

The design is `docs/spec/design/languages.md`. Its decisions are recorded as E.77 to E.82, with:
- requirements REQ-CHR-012 to REQ-CHR-016, REQ-FNT-012 and REQ-CLI-010, the last two groups' new numbers in the Phase 2 table;
- amendments to REQ-FNT-001, 002, 003, 008, 010 and 011, and to decision E.72;
- two out-of-scope items in Part G;
- ARCHITECTURE.md section 4.6, the `joining` module and the features `extended-sets` and `write-text`;
- a new section in `docs/spec/design/character-sets-and-fonts.md`;
- the font rule in `CLAUDE.md` and `third_party/bedstead/PROVENANCE.md`.

None of it is committed or implemented. Owed:
- **Implementation, in the phases of the design's section 13:**
  - 12 × 20 loaded glyphs;
  - `third_party/noto/` with the converter and `xtask check-noto`;
  - the tables and designations (Phases 2 to 4 done in rapid iteration);
  - the `joining` module exists; it goes into the `xtask` layer table;
  - `unicode-bidi` in the dependency records.
- **Tests** for each new requirement, each failing on the old code.
- **Records:** regenerate `docs/traceability.md`; add the user guide's language list and an implementation log row; bring issue #219 in line with the decisions.
- **Research:** `research/notes-languages.md` records the sources. Still owed:
  - confirmation by a reader of Thai of seven Thai glyphs (design section 7.8);
  - a transcription of the 1990 WST Lao table (Figure 33) for phase 4;
  - the exact positions of Ukrainian Ґ ґ.

## The default features (decision E.105, 22 October 2026)

The non-spacing attributes, off by default on the owner's instruction of 17 October, are on again, with the Colour Map of Levels 2.5 and 3.5 (`level25-32colors`, `level35-32colors`); objects, side panels and the Additional Flash Functions are named by a build. The decision, the capabilities document, the porting guide, `ARCHITECTURE.md`, the user guide and REQ-GEN-020 are updated, and the default build's tests pass.

- **Sizes:** `cargo xtask size` with the new default and the split colour features (the `thumbv6m-none-eabi` target is not installed on this Mac).
- **The `classic` profile** (core feature, 17 October 2026): Level 1.5, the renderer, Bedstead and the viewer, with nothing of Levels 2.5 and 3.5; added to `cargo xtask features`. Owed: a decision in Part E and REQ-GEN-020 (what "the vast majority of teletext" means, with evidence), `docs/spec/design/capabilities.md` and `docs/spec/design/porting.md`, a `cargo xtask size` row, and whether `neottx` should offer a classic build. `forms` is left out because broadcast teletext had no forms; confirm with the owner.

## Picture-to-page converter `scripts/image-to-teletext.py` (18 October 2026)

A standalone Python utility outside `neottx` (owner's instruction: not Rust, not part of `neottx`), run with `uv run` like `third_party/noto/convert.py`. It turns a picture into a Level 3.5 page of 80 x 72 sixels in up to 31 redefined colours. Page 100 carries the Level 1 mosaics, X/28/0 and X/28/4 colours, X/27/4 linking a POP at 1A0, and X/26 holding only Active object invocations. The objects carry Full Row Colour, the non-spacing colour changes and the mosaics of column 0. The palette is fitted to the two-colours-per-cell rule. Checked on one picture (example 04): neottx's render matches the encoder's expected sixels exactly.

- **Issue:** none opened yet; `gh` now reaches crowdwave/neoteletext-engine with `gh auth switch -u crowdwave` (18 October 2026). Open one.
- **Palette fitting (19 October 2026):** several starting palettes (median cut, maximum coverage, fast octree), each refined by constrained k-means with repeated split-the-worst and drop-the-least-used swaps, the best kept; then refitted at the change cost that meets the 208-triplet budget. On the ten examples the error fell 6 to 8 per cent in every mode and was never worse, at about four times the time. Literature checked: Wu's method and libimagequant's refinement are the same family; spatial quantization (scolorq) is the one untried idea that could do better.
- **Screen shape (19 October 2026):** a photo is cropped to the display's shape (`--display neottx`, square; `tv`, 4:3) and averaged over each sixel's footprint at 480 x 480, with the sixel rows weighted 6, 8 and 6 lines as decision E.13 draws them; `--smooth lanczos` is the alternative. Pictures already 80 x 72 to 75 keep the pixel-art path. On 40 photos the on-screen error fell from 5,063 to 2,397 at Level 3.5 and from 28,423 to 23,787 at Level 1, better on all 40.
- **Evaluation:** 500 pictures (the ten examples, 400 Wikimedia Commons featured pictures, 90 Kenney CC0 game-art crops) in six modes all rendered exactly as predicted (19 October 2026). The scripts and the browsing page live in the session scratchpad only; keep them (in `tmp/` or a script) if the evaluation is to be repeatable.
- **Records:** the decision (a utility in `scripts/`, Python, its Pillow and NumPy pins) and where it is mentioned (README or user guide); CREDITS.md is not needed (no outside source).
- **Depends on the core feature `nonspacing-attributes`** (off by default, section above). A default `neottx` ignores the colour changes; say so wherever the utility is documented.
- **Row 24 needs X/27/0:** under the default Annex H policy a page's row 24 shows only when X/27/0 asks; the script writes Fastext links to page 100 itself for that. An `FL` line of null links (8FF) emits no X/27/0, so it does not work; check whether the TTI writer should still send the display flag.
- **Options (18 October 2026):** `--level 1|1.5|2.5|3.5` (Levels 1 and 1.5: the eight colours and spacing attributes; 2.5: CLUTs 2 and 3 fitted, CLUTs 0 and 1 fixed; 3.5: 31 fitted), `--hold-mosaics` (Levels 1 and 1.5), `--no-objects` (2.5 and 3.5: the page's own X/26, at most 208 triplets). All six combinations render exactly as the script predicts (example 04).
- **At Level 3.5 the header takes the redefined CLUT 0:** its white text shows in whatever colour CLUT 0 entry 7 became. Consider keeping entry 7 white, or note it.
- **Hold Mosaics in the spacing encoder** costs a second blank column at each row start and came out slightly worse on example 04 (error 17,750 against 17,515); try placing Hold Mosaics where it pays rather than at column 0.
- **Not done:** Hold Mosaics as free colour changes at Levels 2.5 and 3.5; multicolour DRCS for cells of three or more colours; perceptual (OKLab) error; dithering; row 0 columns 8 to 39.
- **Tests:** none; a round trip (picture, page, `neottx render`, sixels equal the expected PNG) would make one.

## The browser removed from the repository (18 October 2026, commit 94346b0)

The browser lives only in `~/devel/neoteletext-browser`. Left over:
- **Engine pieces with no caller now:** the descriptor's `font` member and `Descriptor::font()` with `MediaType::FontBdf` (REQ-FNT-006), the web agent's `Input::FetchContents` and `Contents*` events (REQ-WEB-025), `CHANNEL_PREFIX` (REQ-WEB-018), and `EnhancementReport::designations`, which is counted but read by nothing. Keep them as engine API for ports, or remove them with their requirements.
- **`docs/neoteletext-browser-open-questions.md`** mixes engine and browser questions; split it, sending the browser's to the other repository.
- **The user guide (about line 245)** says the font converter refuses 12 × 20 fonts whose line pairs differ, against REQ-FNT-010 as amended by decision E.77.
- **GitHub issues** for the browser (#215, #216, and the browser parts of epics #177 and #210) are to be closed or moved to the other repository, once an account with access to crowdwave/neoteletext-engine is logged in.

## Deadlines (decision E.98)

- **The core decoder's next deadline:** `Decoder` does not say when it next needs `tick` (its completion timeout and staleness), so a port still ticks it on an interval. Give it a `next_tick` as the audio receiver now has.

## Documentation

- **One development-process document** (the owner asked): written on 21 October 2026, shared with the browser, as `~/devel/neoteletext-common/DEVELOPMENT.md`. Still owed: remove from CLAUDE.md the sections it now holds ("How work proceeds", "Tracking work", the process parts of "Working with the specification") and README's "Checks", leaving pointers, and make the engine's other changes in its appendix B.
- **`docs/spec/design/character-sets-and-fonts.md`** (written at the owner's request, 13 October 2026) explains G2, glyph counts and how positions reach glyphs. Owed: link it from README's documents table and the user guide; extend it with the earlier explanations (what a usable font is, character sets per page and their maximum, designation, rounding, the 4:3 shape) if the owner wants one place for all of it; check its claims against REQUIREMENTS.md at integration. Its history section is drawn from the research notes on the 1974, 1976, 1990 and 1992 specifications and EN 300 706; the dates of the SAA5050 family and of the first Level 1.5 broadcasts are not in those notes and are not given.
- **`oldvga` is still named** in REQUIREMENTS.md, IMPLEMENTATION-LOG.md, `docs/spec/design/hmi.md`, `docs/spec/open-questions.md` and `prototypes/browser/docs/desktop-browser.md`; the build host is now `foundintelquiet4060`.
- **ARCHITECTURE.md** does not describe the new pieces: `glyphs::conformance`, `level1::designation_report`, `neoteletext_browser::charsets`, the decoder's font slots.
- **`docs/spec/design/font-security.md`** does not cover `c` lines (listing data only) or the decoder holding up to 25 fonts (about 560 KB at most).

## Font collection and library (outside this repository)

- **Everything now lives in `~/devel/teletext-reference/`** (owner, 13-14 October 2026), which holds only `teletext-fonts/`, `teletext-standards/` and `teletext-audio/` (owner, 14 October 2026). `teletext-fonts/` holds `manufacturers/<maker>/{datasheets,originals,scripts,notes}`, each font in `<source>/fontbuild/<id>/` beside what it was made from, `catalog/`, `tooling/{lib,pipelines,build}`, `research/`, `INDEX.csv` and `FONTS.txt`; its README states the scheme. There is no `library/`, `attic/` or move log any more. REQUIREMENTS.md, CLAUDE.md, the research notes and `docs/spec/module-sources.md` cite the new paths (uncommitted). `docs/teletext-specification-audit.md` (untracked, not mine) still cites `~/devel/microcontrollers/teletext/`. REQUIREMENTS.md (Part F preamble, the SAA5050 section, Appendix B) and `research/notes-saa5050.md` still cite `catalog/indexes/DATASHEET-INDEX.md`, which was deleted on 14 October 2026: their page-number corrections stand on their own, but the citations need rewording.
- **253 fonts from 10 pipelines**, each rebuilt byte-identically from its saved sources by `teletext-fonts/tooling/pipelines/tree/rebuild_all.sh` (ending with `verify_builds.py`; last run 14 October 2026 after the move into `teletext-fonts/`). Every font has its native size; a 12 x 10 `<id>.bdf` only when the conversion is exact. Each maker's library is `manufacturers/<maker>/fonts.txt`, with addresses `http://127.0.0.1:8642/manufacturers/...`, so the font server must serve `teletext-fonts/`.
- **The tree is a git repository**, private on GitHub as crowdwave/teletext-reference (14 October 2026).
- **Chip sets that are not teletext sets**: the SAA5054 differs from the French option at 5/E, and edit.tf's Belgian set at 7/C; SAA5055 and its copies hold plain US ASCII. They stay incomplete by design; say so in the reports.
- **`philips-hoard-p2000`** uses another code layout and is mapped by pixels only.
- **`espruino-teletext5x9mode7`** lacks the space.
- Owed from the font work (13 October 2026): the owner's review of the hand-drawn characters (`catalog/patches/`), especially Bedstead complete's ™; the owner's decision whether neoteletext accepts 12 x 20 fonts (REQ-FNT-002, decision E.72; open a `decision` issue); decisions on the 10 x 10 (ST92195, TPU 30xx, VCT 38xx) and 8 x 10 (TI Instant Text) designs and NationalChip's blank `_`; the drawn characters are not to be offered to Bedstead upstream (owner).

## Decisions and settings

- **The single user font of earlier settings is dropped** without migration (decision E.75 item 6): users choose again.
- **Open from before**: #187 (outside help), #188 and #189 (owner decisions), #201 (`neottx check` is not sandboxed).
- **Nothing in neoteletext is pushed.**

## Iteration tooling

- **`scripts/try.sh` was removed** (owner, 14 October 2026). CLAUDE.md now gives the rebuild and restart commands instead. The browser embeds its fonts at build time, so it no longer needs the font server on port 8642; the user guide's section on font libraries still describes serving one.

- The `iterate` profile is in place (an interface edit rebuilds in 2.7 seconds, a core edit in 3.6, against 76 for release) and described in CLAUDE.md; it still belongs in the development-process document and README.
- **The fast gate took over 10 minutes** after a core change (whole-workspace rebuild plus the window and end-to-end tests); the window and its tests left the workspace on 16 October 2026 (decision E.76), so remeasure it. Commits are batched rather than hooks skipped; making the gate faster is open.

## GitHub issues not opened (23 October 2026)

The round of 23 October 2026 (decisions E.106 onward) could not open GitHub issues: the permission system refused writes to GitHub while the owner was away. Each piece of work is recorded in its decision and in `IMPLEMENTATION-LOG.md`; issues are owed for: the qrkeyboard waker (E.106), the proxy's CONNECT reply, flash rendering, channel log records, Device Bound Session Credentials, channel-named fonts, row 24 and the keys' destinations, and the exported sub-page bound.

## Device Bound Session Credentials (decision E.110, 23 October 2026)

Built behind `neoteletext-web`'s feature `dbsc`; owed:
- **Platform key stores** for a port (macOS Secure Enclave or Keychain, Windows TPM through CNG, Linux Secret Service): the libraries proposed in E.110 item 2 need the owner's approval by name before any port builds them.
- **Sessions across runs:** an export and import of `Sessions`, as the cookie jar has, for a port to keep with its cookies.
- **Draft features not built:** RS256, federated sessions, `Secure-Session-Skipped`, `.well-known/device-bound-sessions`, `allowed_refresh_initiators`, attribute matching of credentials (needs the `Cookies` interface to show attributes), redirects of registration.
- **The draft moves:** recheck the Editor's Draft before the browser ships the feature, and amend E.110.
- **A real server:** no public server was tried; Google's test pages need a browser that sends Chrome's headers.

## Timed tests under load (23 October 2026)

`neoteletext-cli`'s `typing` test, whose median budget is 2 ms (REQ-INT-008), failed the commit gate once at 2.8 ms with the machine's load average near 20, and three times more when run alone, then passed five times in a row; nothing on its path had changed. A timed test that fails under load stops unrelated commits: consider measuring it against an unloaded baseline, or running it only in the full gate.

## The standard's maxima and sub-pages on the wire (26 October 2026)

Owed by E.122: every item of `neoteletext-common/STANDARD-LIMITS.md` section 7 that names this repository, in its order; none is started. The owner's questions are its section 6.

## The Opus decoder (decision E.123, 26 October 2026)

Built and integrated; owed:
- **Opus in the audio profile:** carried as code 2 on 26 October 2026 (decision E.124, issue #223): records, store, decoding in order. Still owed: Opus at receiver Level A (two decoders' state, about 56 KB, is not in the level's proposed budget, `audio.md` section 12, and tellyboy plays no Opus), and `neottx sound` reading an encoder's packets (it refuses Opus).
- **Hardware:** the decoder has run only under QEMU; its cycles on a real Cortex-M0 and on the ESP32 (Xtensa, not yet built: REQ-GEN-015) are unmeasured.
- **Fuzzing:** `opus_packets` has run 45 minutes with overflow checks (`cargo fuzz run -a`) on the engine's version, and the gate gives it 20 seconds; the gate's run is without `-a`, so overflow checks reach it only by hand (`docs/spec/design/opus-security.md`).
- **The porting workspace** `tmp/opus-integer/` (ignored) holds the internal oracle tests; it is now behind this crate, which is the source.

## The TTI Form Extension withdrawn (decision E.126, 27 October 2026)

Removed from the code and the specification; owed:
- **The decision's number.** The uncommitted mouse change in `neoteletext-input` cites a decision E.125 that `decisions.md` does not hold yet, so this withdrawal took E.126. Confirm the numbering with the owner before either is committed.
- **Traceability:** `docs/traceability.md` still lists the removed form files; regenerate it with `cargo xtask traceability` at integration.
- **The full gate:** `cargo xtask features`, `cargo xtask size` (the `core-all` and `web-all` images shrink) and the fuzz targets, not run for this change.
- **`fuzz/Cargo.lock`** is stale from before this change: it lacks `neoteletext-opus` under `neoteletext-audio` (decision E.124), and Cargo rewrites it on the next fuzz build.
- **`CLAUDE.md`** still calls Appendix C normative, points to it as the corrected forms specification, and says the web module runs forms; for the owner to change.
- **Outside this repository:** NeoTeletext's code maps the cookie site `Site::CrossOriginForm`, which no longer exists, so its next re-pin past this change needs that mapping removed; `neoteletext-common/INPUT.md` section 1 still puts forms out of its scope as page files and HTTP submissions.
