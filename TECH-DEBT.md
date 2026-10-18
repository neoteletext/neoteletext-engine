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

- **Epics #177 (fonts) and #210 (character sets)** stay open until their work and the full gate are done.

- **The 4:3 aspect correction stretched the wrong way** (found and fixed in rapid iteration): the browser's "4:3 picture", `neottx --aspect` and the image writers made pixels 1.2 times taller; on a 4:3 PAL set each pixel of the 12-by-20 cell is wider than tall (a half-dot is 1/624 of the width, a line 1/768). The core's `ASPECT_NUMERATOR`/`ASPECT_DENOMINATOR` are now 16/13 and stretch the width; `neoteletext_image::scale` maps columns back through the stretch; the browser uses the core's ratio; the user guide's `--aspect` line is corrected. Owed: REQ-RND-004 still says "pixels about 1.2 times taller than they are wide" and needs amending, with a decision citing the timing; a regression test that fails on the old ratio (the scale tests were changed with the fix; confirm they fail on 6/5 height); check images and goldens made with `--aspect` anywhere in the docs or tests; mention it in the log. "4:3 picture" is now on by default in the browser (owner, 13 October 2026); `neottx --aspect` is still off by default; REQ-RND-004 and the settings test should say which is the default.

- **The command-line program is renamed `neottx`** (owner, 14 October 2026: "rename ttx to neottx everywhere"). The binary (`crates/neoteletext-cli/Cargo.toml`), its clap name, every message prefix, the tests (`CARGO_BIN_EXE_neottx`), the oracle and test-server scripts, REQUIREMENTS.md, ARCHITECTURE.md, CLAUDE.md, the user guide and the other documents now say `neottx`. Kept, because they are not the program's name: the form fields `ttx_token`/`ttx_action` and the C interface prefix `ttx_` (REQ-FFI), zxnet's `ttx,true` channel message and its ttx export format, CEBRA's `.ttx` extension, zvbi's `ttx_page`, DVB's TBttx/Bttx, example URL paths `/ttx/`, and third_party/. `docs/teletext-specification-audit.md` (untracked, not mine) still says `ttx`. Verified: `cargo check --workspace --all-targets` clean, `cargo test -p neoteletext-cli` passes (31 tests). Owed: a decision in Part E recording the rename; an issue; the full gate; the IMPLEMENTATION-LOG rows were renamed in place, so the log no longer shows the old name (add a row saying when it changed); the teletext-reference tooling calls `target/debug/neottx`.

## The specification split into `docs/spec/` (18 October 2026, decision E.83)

`REQUIREMENTS.md` is now an index, and the specification is in `docs/spec/`. The text moved unchanged; the language decisions E.77 to E.82 and the explainer's language section were cut to pointers to `docs/spec/design/languages.md`. `cargo xtask traceability` reads the new files (424 requirements, regenerated). Owed:
- commit, on its own;
- reword the older items of this file, `IMPLEMENTATION-LOG.md` and the code comments that say "REQUIREMENTS.md Part E" or "section F.n" as the new paths, where the index is not enough;
- `docs/t42-browser-open-questions.md` concerns the browser and belongs in `prototypes/browser/docs/`;
- `docs/teletext-specification-audit.md` (untracked, not mine) is unplaced.

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
  - the tables and designations;
  - the `joining` module, which goes into the `xtask` layer table when it is created;
  - marks from the page's G2 set;
  - the Thai and Lao tables;
  - `neoteletext-app::write_text` and `neottx write-text`, with `unicode-bidi` added to the dependency records.
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

## The move of the browser to `prototypes/browser/` (16 October 2026, decision E.76)

- **Not committed.** The moves, decision E.76 and the edits below are in the working tree with the earlier uncommitted work. Owed: an issue for the refocus, and commits that keep the move separate from that work.
- **Requirements that stay but still describe the browser in part:** REQ-FNT-003 to REQ-FNT-009 and REQ-FNT-011 (the browser's font loading, cache, libraries and settings), REQ-WEB-014 (fonts from other sites), REQ-SEC-026 (the browser's helper processes), decision E.70 item 8 (the contents view), decisions E.72 and E.75 where they name the browser, and the rows of the phase table in section F.0.1 for the moved requirements. Reword each for the engine, or move it, with the owner.
- **Engine code that cites moved requirements:** `neoteletext-core` (`level1.rs` and `acq.rs` cite REQ-BRW-011 for the designation report; `glyphs/mod.rs` and `render.rs` cite REQ-BRW-010 for drawing a font's sample) and `neoteletext-app/src/pipe.rs` (REQ-BRW-009, REQ-BRW-011). Give the engine features their own requirements and cite those.
- **Browser-only code still in `neoteletext-app`:** `font.rs` and `fetch::run_font` (the browser's font helpers), `sandbox::helper_command` and `Helper` (Seatbelt profiles for the browser's helpers), and the pipe frames only the browser's decoder and interface exchange. They stayed because `fetch.rs` and `pipe.rs` are shared with `neottx`; move them into the prototype or keep them, with the owner.
- **`cargo deny` not rerun** after the egui licence exceptions (`clipboard-win`, `error-code`) were removed from `deny.toml`; it is not installed on this machine.
- **Documents still describing the browser:** `docs/user-guide.md` (the browser's sections), `docs/spec/design/font-security.md`, `docs/spec/design/hmi.md`, `docs/spec/open-questions.md` and `docs/t42-browser-open-questions.md` (its paths now point into the prototype). `docs/traceability.md` is to be regenerated (`cargo xtask traceability`). IMPLEMENTATION-LOG.md and `scripts/github-plan/` keep the old paths as history; add a log row for the move.
- **GitHub issues** for the browser (#215, #216, and the browser parts of epics #177 and #210) are to be labelled or closed for the prototype.
- **The full gate** has not run on the engine without the browser.

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
