# Technical debt

Work deferred while behaviour is being shaped (the owner's instruction of 13 October 2026: iterate fast, and add tests, documentation and detail once the software works as expected). Each item says what is owed and why it was left. Remove an item when it is done; move it to a GitHub issue when it needs the owner.

## Verification

- **The full gate has not run** since 67153ff: c884152, c6b3d40, 30dc865, a5651d6, 9c989ba, b9f8ef6, f43fde7, f9fe7ea and f341e94 passed only the fast gate. Run `NEOTELETEXT_REMOTE=foundintelquiet4060 scripts/check.sh full` (the remote half needs the host set up in this session; Xtensa builds may need `~/export-esp.sh`).
- **Cortex-M0 sizes not remeasured** (REQ-GEN-009, `docs/spec/design/porting.md`): `GlyphSet` now holds 25 font slots, and loaded fonts can be rounded. `cargo xtask size` on the remote host.
- **Fuzzing not rerun** on the changed readers: `bdf`, `bdf_convert`, and the `fonts.txt` reader with `c` lines, which has no fuzz target of its own.
- **Renderer cost with per-set fonts** (issue #216) not measured: `row_renderer_cost_with_a_loaded_font` before and after.
- **No test that frame kind letters are distinct**: `FONT_TABLE` first took `b'k'`, already `INPUT`'s; only a compiler warning caught it.
- **No end-to-end test of two character sets on one real page** drawn from two user fonts; the core test covers it.

## Unfinished features

- **Audio, area AUD (issue #220):** built and tested on branch `audio`; owed:
  - live sounds (REQ-AUD-014): store them, track the sender's clock, and fade out after two seconds without a block;
  - the `.ttau` file and WebSocket record readers (REQ-AUD-008);
  - 5 ms fades at each edge of a missing block, and the `concealed` report (REQ-AUD-013);
  - passing IMA ADPCM blocks undecoded to a sink that decodes them itself, for tellyboy (REQ-AUD-018);
  - `cargo xtask size` and the Cortex-M0 flash of a receiver with audio;
  - merging `audio` into `implementation` once the other session's work there is committed, and the full gate.

- **Epics #177 (fonts) and #210 (character sets)** stay open until their work and the full gate are done.

- **The 4:3 aspect correction stretched the wrong way** (found and fixed in rapid iteration): the browser's "4:3 picture", `neottx --aspect` and the image writers made pixels 1.2 times taller; on a 4:3 PAL set each pixel of the 12-by-20 cell is wider than tall (a half-dot is 1/624 of the width, a line 1/768). The core's `ASPECT_NUMERATOR`/`ASPECT_DENOMINATOR` are now 16/13 and stretch the width; `neoteletext_image::scale` maps columns back through the stretch; the browser uses the core's ratio; the user guide's `--aspect` line is corrected. Owed: REQ-RND-004 still says "pixels about 1.2 times taller than they are wide" and needs amending, with a decision citing the timing; a regression test that fails on the old ratio (the scale tests were changed with the fix; confirm they fail on 6/5 height); check images and goldens made with `--aspect` anywhere in the docs or tests; mention it in the log. "4:3 picture" is now on by default in the browser (owner, 13 October 2026); `neottx --aspect` is still off by default; REQ-RND-004 and the settings test should say which is the default.

- **The command-line program is renamed `neottx`** (owner, 14 October 2026: "rename ttx to neottx everywhere"). The binary (`crates/neoteletext-cli/Cargo.toml`), its clap name, every message prefix, the tests (`CARGO_BIN_EXE_neottx`), the oracle and test-server scripts, REQUIREMENTS.md, ARCHITECTURE.md, CLAUDE.md, the user guide and the other documents now say `neottx`. Kept, because they are not the program's name: the form fields `ttx_token`/`ttx_action` and the C interface prefix `ttx_` (REQ-FFI), zxnet's `ttx,true` channel message and its ttx export format, CEBRA's `.ttx` extension, zvbi's `ttx_page`, DVB's TBttx/Bttx, example URL paths `/ttx/`, and third_party/. `docs/teletext-specification-audit.md` (untracked, not mine) still says `ttx`. Verified: `cargo check --workspace --all-targets` clean, `cargo test -p neoteletext-cli` passes (31 tests). Owed: a decision in Part E recording the rename; an issue; the full gate; the IMPLEMENTATION-LOG rows were renamed in place, so the log no longer shows the old name (add a row saying when it changed); the teletext-reference tooling calls `target/debug/neottx`.

## The specification split into `docs/spec/` (18 October 2026, decision E.83)

`REQUIREMENTS.md` is now an index, and the specification is in `docs/spec/`. The text moved unchanged; the language decisions E.77 to E.82 and the explainer's language section were cut to pointers to `docs/spec/design/languages.md`. `cargo xtask traceability` reads the new files (424 requirements, regenerated). Owed:
- commit, on its own;
- reword the older items of this file, `IMPLEMENTATION-LOG.md` and the code comments that say "REQUIREMENTS.md Part E" or "section F.n" as the new paths, where the index is not enough;
- `docs/t42-browser-open-questions.md` concerns the browser and belongs in `prototypes/browser/docs/`;
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
16. **Not decided: issues #187, #188, #189 and #201.** The GitHub account active on this machine (`starqueue`) cannot see crowdwave/neoteletext, so they could not be read. Log in with an account that has access (`gh auth login`).

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
- the 18 sets of groups 0101, 0111 and 1011 and the Vietnamese and Armenian G2 sets (`charset/tables.rs`), designated in Tables 32, 33 and 34 behind the core feature `extended-sets` (on by default; off in `classic` and `modern-classic`);
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
  - the group 1001 tables are now written by `scripts/arabic-sets.py` between marker lines in `tables.rs`, with ArabicShaping.txt 16.0 downloaded and checked by SHA-256. Its private-use allocation is by sorted letter, so adding a letter renumbered U+F002B onwards; make the allocation stable (append only) before the private-use table is recorded as a decision;
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

## Non-spacing attributes off by default (17 October 2026)

The owner's instruction: the Level 2.5 and 3.5 attributes that take no character cell stay in the code but are off by default. The core feature `nonspacing-attributes` gates the Table 27 column triplets for foreground and background colour, Additional Flash Functions, Display Attributes and Font Style (`ATTRIBUTES` in `crates/neoteletext-core/src/enhance.rs`); without it each only sets the column. `bold`, `italic` and `animation` now require it and left the defaults; `modern-classic` names it, since decision E.71 keeps these attributes in the profile.

- **Decision and requirements:** amend E.61 and E.71 (the defaults are no longer "all on"; the feature list), the L25 requirements that assume the attributes (REQ-L25-023 and its neighbours), and REQ-GEN-020. Row and screen colours (Table 26 row triplets) and Modified G0/G2 Designation were left on; record that.
- **Tests failing in a default-features build** (the gate tests `--all-features` only, so it still passes): in `crates/neoteletext-core/tests/level25.rs`, `objects.rs` and `drcs.rs`: additional_flash_functions, boxing_and_windows, clause_13_9_object_example, colour_triplets_set_colour_map_indices, contention_examples_of_annex_c2, display_attributes_against_level_1, font_style_extends_over_rows_at_level_35, g1_mosaics_follow_the_non_spacing_separated_attribute, levels_change_without_reacquisition, mode_0_characters_take_the_cell_colours, passive_objects_start_from_their_defaults, receiver_area_is_never_written, remapping_and_black_background_substitution, row_below_spacing_double_height, sizes_follow_annex_c3. Gate them on the feature, and add a test in `tests/capabilities.rs` that the triplets only set the column without it.
- **`cargo xtask features`** (full gate): its `level25` and `level15,level25` builds now lack the attributes, so the tests above fail there too; add a `nonspacing-attributes` build.
- **Test gating fixed along the way:** `tests/capabilities.rs` DRCS helpers and the `FontStyle` import in `src/render/level25_tests.rs` were gated as if the defaults held every capability; a default build would not compile.
- **Golden rasters and the cli and image crates' Level 2.5 tests** (`level25_corpus.rs`, `neoteletext-image/tests/level25.rs`, `enhancement_reporting.rs`): not run with the default features.
- **Documents:** `docs/spec/design/capabilities.md` (the feature table and profile), `docs/spec/design/porting.md` line 34, the user guide's feature list, ARCHITECTURE.md if it lists features.
- **Sizes:** `cargo xtask size` with the new default.
- **The `classic` profile** (core feature, 17 October 2026): Level 1.5, the renderer, Bedstead and the viewer, with nothing of Levels 2.5 and 3.5; added to `cargo xtask features`. Owed: a decision in Part E and REQ-GEN-020 (what "the vast majority of teletext" means, with evidence), `docs/spec/design/capabilities.md` and `docs/spec/design/porting.md`, a `cargo xtask size` row, and whether `neottx` should offer a classic build. `forms` is left out because broadcast teletext had no forms; confirm with the owner.

## Picture-to-page converter `scripts/image-to-teletext.py` (18 October 2026)

A standalone Python utility outside `neottx` (owner's instruction: not Rust, not part of `neottx`), run with `uv run` like `third_party/noto/convert.py`. It turns a picture into a Level 3.5 page of 80 x 72 sixels in up to 31 redefined colours. Page 100 carries the Level 1 mosaics, X/28/0 and X/28/4 colours, X/27/4 linking a POP at 1A0, and X/26 holding only Active object invocations. The objects carry Full Row Colour, the non-spacing colour changes and the mosaics of column 0. The palette is fitted to the two-colours-per-cell rule. Checked on one picture (example 04): neottx's render matches the encoder's expected sixels exactly.

- **Issue:** none opened; `gh` could not resolve crowdwave/neoteletext on 18 October 2026. Open one.
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
- **`docs/t42-browser-open-questions.md`** mixes engine and browser questions; split it, sending the browser's to the other repository.
- **The user guide (about line 245)** says the font converter refuses 12 × 20 fonts whose line pairs differ, against REQ-FNT-010 as amended by decision E.77.
- **GitHub issues** for the browser (#215, #216, and the browser parts of epics #177 and #210) are to be closed or moved to the other repository, once an account with access to crowdwave/neoteletext is logged in.

## Documentation

- **One development-process document** (the owner asked): pull CLAUDE.md's "How work proceeds", "Tracking work" and "Working with REQUIREMENTS.md" and README's "Checks" into `docs/development.md`, and leave pointers so the rules live once.
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
