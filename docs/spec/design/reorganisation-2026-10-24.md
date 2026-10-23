# The source reorganisation of 24 October 2026

The lasting record of the engine's half of the structure plan (`~/devel/neoteletext-common/structure/`, documents `00-conventions.md`, `01-method-and-order.md`, `10` to `13` and `30-cross-cutting.md`), carried out while the owner was away, on the owner's instruction to carry out the whole plan without stopping. `IMPLEMENTATION-LOG.md` row 88 summarises it; `ARCHITECTURE.md` sections 3 and 7 describe the layout it left; `TECH-DEBT.md` ("The source reorganisation") lists what it left undone. Nothing here is normative.

## 1. What was done

- **Moves, not rewrites.** Every source file of the engine was read against `00-conventions.md`: one concept per file (150 to 400 lines, 500 the ceiling), functions under about 80 lines, `impl` blocks under about 300, the 2018 module layout (no `mod.rs`), unit tests in `tests.rs` beside the code, the narrowest visibility. Files were cut into directories by concept; no public path changed except `neoteletext_app::live::descriptor_service_names`, now `neoteletext_app::descriptor::descriptor_service_names` (its one importer, the CLI, moved with it). The browser imports no path that moved.
- **Four branches, one per document**, in four worktrees from `e7d8927`, each passing the pre-commit gate at every commit, merged into `implementation`: the core (`10`), the formats, slicer and image crates (`11`), the web crate (`12`), and the application, command-line, audio, input and qrkeyboard crates (`13`). The integrator then carried out `13`'s `xtask` steps X1 to X4 and closed the gate gaps the plan found (section 3).
- **Batching (decision of the brief, recorded):** where a document had one commit per file, a branch could batch one commit per directory or concept, so that each document took about 5 to 16 commits; a batch never mixed two concepts' directories, and every function split and every shared-form extraction was a commit of its own.
- **One form for moved tests (30-cross-cutting.md section 6.4, binding on every branch):** each `tests.rs` begins with its own `#![allow(...)]` and `use super::*;`, and the parent declares `#[cfg(test)] mod tests;` as its last item. Until the integrator's change to the checks (section 3), `xtask`'s checks cut a file at its first `#[cfg(test)]`, so no `#[cfg(test)]` item could stand above production code; the core's files also mark their first `use` line `#[cfg(test)]` so that the old checks skipped them.
- **Visibility narrower than the documents in most places:** a child module reaches its parent's private items, so where a document asked for `pub(super)` on a parent item that only its children use, the item stayed private. `pub(super)` was used between siblings only. The places where fields were widened against the conventions are listed in `TECH-DEBT.md`.

## 2. The numbers

| Crate | Files before → after | Largest file after | Files over 500 before → after | Functions over 80 before → after | `impl` over 300 before → after |
|---|---|---|---|---|---|
| core | 36 → 138 | `glyphs/bedstead.rs` 2,076 (generated); code `charset.rs` 467 | 17 → 6 (the generated table and five test files) | 3 → 3 | 4 → 1 |
| formats | 46 → 94 | `consts.rs` 410 | 9 → 0 | 2 → 0 | 2 → 0 |
| slicer | 8 → 25 | `generator.rs` 368 | 2 → 0 | 2 → 2 | 1 → 0 |
| image | 7 → 9 | `tests/level25.rs` 491 | 0 → 0 | 1 → 0 | 0 → 0 |
| web | 24 → 73 | `json.rs` 496 | 10 → 0 | 9 → 7 | 4 → 2 |
| app | 14 → 40 | `fetch/transport.rs` 330 | 3 → 0 | 2 → 2 | 0 → 0 |
| cli | 15 → 41 | `view.rs` 469 | 3 → 0 | 2 → 2 | 0 → 0 |
| audio | 10 → 24 | `receiver.rs` 295 | 1 → 0 | 1 → 1 | 2 → 0 |
| audio-mix | 2 → 8 | `mixer.rs` 284 | 1 → 0 | 0 → 0 | 1 → 0 |
| input | 11 → 24 | `wire.rs` 280 | 2 → 0 | 1 → 1 | 0 → 0 |
| qrkeyboard | 10 → 21 | `net/websocket.rs` 297 | 2 → 0 | 0 → 0 | 1 → 0 |
| xtask | 10 → 26 | `g3/geometry.rs` 235 | 0 → 0 | 2 → 2 | 0 → 0 |

Each branch measured with its own brace-matching script, so the columns agree within a crate, not always across crates; the branches' notes below give their exact counts. Test counts are unchanged in every crate. The functions and `impl` blocks still over the limits are listed in `TECH-DEBT.md`.

## 3. The integrator's work

- **`xtask` (document `13` steps X1 to X4):** `checks.rs` into `checks/{layers,deps,constants,io}.rs`, with the `DATA_FILES` test that `10` step 1 had added moving to `checks/constants.rs` (correction 11 of `30-cross-cutting.md`: the line ranges were taken from the file as step 1 left it); `g3.rs` into `g3/{geometry,table,tests}.rs`; `glyphs.rs` into `glyphs/{bedstead,g3_table,required,tests}.rs`; `size.rs`'s ELF reader into `size/elf.rs`. The shared items of `checks.rs` stayed private (children reach them). Step X2's reason for `shape`'s `too_many_lines` allow was added in a commit of its own. The optional shared form of pair 23 (`trace.rs` using `checks::rust_files`) was not taken.
- **The checks no longer cut at the first `#[cfg(test)]`.** `xtask/src/test_code.rs` decides by module: a file is test code when its module is declared under a `cfg` that holds only under `test`, in its parent or any module above it, inline modules included; in any other file each test-only item, field, statement or expression is blanked where it stands, on the code with comments and literals blanked first, line numbers kept. `check-layers`, `check-constants`, `check-io` and the traceability table use it. Before, the checks did not read `neoteletext-core/src/glyphs.rs` past line 157, the core's `lib.rs` past 37, `neoteletext-app/src/fetch/wakes.rs` past 33, nor the audio, audio-mix, input and slicer `lib.rs` files past their test-only `extern crate` lines, and they read 80 `tests.rs` files whole. Each check has a regression test with an early test-only item, which fails with the old cut. This covers the app implementer's suggestion of skipping files named `tests.rs`, by module rather than by name.
- **The no-panic list** (`DEVELOPMENT.md` section 6): `neoteletext-app` and `neoteletext-cli` (its `lib.rs` and `main.rs`) gained the list, every lint hit bounded in place (29 in the app, 63 in the CLI), none allowed; `scripts/check.sh fast` checks that the twelve crate roots carry one identical list and that no crate root is left out; and, for finding F-105 of the security review of 24 October 2026, `clippy::string_slice` joined every list.
- **Documentation in the gate:** `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` runs in `fast`; the three warnings the app implementer found and eleven from moved items were fixed.
- **Documents:** `docs/traceability.md` regenerated; the line citations the branches listed fixed (`docs/spec/design/hmi.md`, `languages.md`, `ARCHITECTURE.md`, `IMPLEMENTATION-LOG.md` row 9, `TECH-DEBT.md`, `docs/teletext-specification-audit.md`); `third_party/bedstead/PROVENANCE.md` names no moved path.
- **Not run by the integrator:** `scripts/check.sh full`, on the coordinator's instruction that it run once on the final merged result after the security-fix branches merge.

## 4. The branches' notes

Copied as each branch's implementer wrote them (untracked `notes.md` files, collected in `tmp/notes-structure-*.md`), headings demoted. Each records its departures from its document and why, its numbers and its gate times; "for the integrator" items are done as section 3 says unless `TECH-DEBT.md` lists them.

### 4.1 `10-engine-core.md` (branch `structure/core`, merged at a3b2f3e)

#### notes.md: 10-engine-core.md carried out on branch structure/core

Base: e7d8927 (implementation at the start; the document read fb2dd8a).

##### Baseline

- `cargo fmt --check` on the untouched tree: clean, so no formatting commit.
- Feature baseline: the machine was loaded by seven other worktrees' builds;
  the first row (`neoteletext-core []`, clippy + test) took 291 s, so the
  twelve-row baseline (about an hour) was stopped after it. Row `[]`: ok.
  Instead, every commit lints all twelve core shapes of `xtask/src/features.rs`
  (clippy only, `--all-targets`), and `cargo xtask features` runs once at the end.

##### Departures and decisions

- Step 1 (cross-cutting correction on `DATA_FILES`): already met as
  committed in f4167a0. `DATA_FILES` is matched against the path relative to
  `crates/neoteletext-core/src/` only (`consts.rs`, `consts/`, `charset/`,
  `glyphs/bedstead.rs`), so the `consts.rs` of any other crate is scanned
  again (the old suffix match exempted them; none holds a watched literal,
  `check-constants` passes). `charset/` is a directory prefix, as the
  document's step 1 says, so the twelve table files of step 15 need no
  further change; `charset.rs` (the parent, code) is scanned.
- Moved tests (cross-cutting correction): every `tests.rs` starts with its
  doc line and the `#![allow(...)]` its old `mod tests` carried, then the
  `#[cfg(test)]`-marked `use super::*;` that 10-engine-core.md section 4
  asks for (the marker makes `check-layers`, `check-constants` and
  `check-io` skip the file); every parent's `#[cfg(test)] mod tests;` is its
  last item.
- Step 3: no visibility changes were needed: a child `tests` module sees
  its parent's private items, so the `pub(super)` the document lists for
  `ecc`, `service` and `level15` (`build_hamming84_table`, `date_time`,
  `write`, ...) were not made.
- Tests that compiled through their parent's imports: `use super::*` in a
  moved test file brings the parent's `use` imports too, and pruning the
  parent's imports to what its own code uses (rustc's unused-import fixes)
  removed names only the tests used. Each such test file gains those
  imports explicitly; the tests themselves are unchanged. So far:
  `packet/tests.rs` (`ControlBits`, `PageNumber`, `SubCode`, `TRIPLET_BYTES`).
- Step 6 (packet), done as one commit for the directory:
  - 6a: the field constructors made `pub(super)` are exactly the nine the
    step names, less `flag`, which only `DisplayAttributes::data` in the same
    file calls, so it stays private.
  - 6b: `DATA_FIRST_BYTE_NUMBER` moves to `magazine_table.rs` with its only
    two users (`mot_object_link`, `mot_totals`) instead of going to
    `object_pages.rs` as `pub(super)`: no visibility change needed.
  - 6c: `pub(super)` only where a sibling or the parent calls: `decode_header`,
    `HEADER_UNITS` (the tests), `triplets_of`, `triplets`, `triplet_values`,
    `page_reference` (the decoder; `service_data` reads it), `decode_links`,
    `compositional_links`, `decode_service`; `value_packet` is
    `pub(in crate::packet)` (called from three sibling encode modules); `set`
    in `packet/encode.rs` is `pub(super)`. The other HEADER_* indices,
    `values_of`, `triplet_body`, `nibble` and `designation_code` stay private
    (children see their parent's private items; `presentation_values` keeps
    its `pub` inside the private `page_data::encode`).
  - 6d: the doc comment of `encode` stays on the `pub mod encode;` line in
    `packet.rs`, so its link to `decode` still resolves there.
  - Checked after the step: `cargo build -p neoteletext-formats
    --all-features` and `cargo check` in `fuzz/` pass.
- Step 7 (acq), one commit: as written. `pub(super)` on `StoredPage::{erase,
  merge}`, `PageStore::{allocate, page_mut}`, `Decoder::{magazine_event,
  page_event}` (reception.rs; called from `tick` and enhancement_pages.rs)
  and `Decoder::{page_format, start_enhancement, name, route,
  store_enhancement}` (enhancement_pages.rs; called from reception.rs).
  The step's list named `end` too; it is called only from `magazine_event`
  in the same file, so it stays private, and `name` is added (called from
  `enhancement_data`). `SearchStatus` stays in acq.rs. `acq/tests.rs` gains
  the imports it took from acq.rs.
- Step 8 (level1), one commit: as written, with one addition the step
  implies: to call `RowContext::new` from page.rs the struct itself must be
  nameable there, so `struct RowContext` is `pub(super)` (its fields stay
  private). `decode_row` is `pub(super)`. The new constructor (four
  arguments) is the only new code; `build_grid` calls it where it built the
  literal. `level1/row/tests.rs` is the stated third level. Both level1 test
  files gain the imports they took from level1.rs (`crate::charset`,
  `Config`, `MagazineData`, `StoredPage`, `G0`, `second_g0` and some
  constants), and each keeps only the constants it uses from the shared
  import line of the old test module.
- Step 9 (colour, objects): two commits, one per directory. Colour: as
  written, no visibility changes; `colour/tests.rs` gains `PresentationLevel`,
  `FallbackFields`, `PresentationFields`, `SidePanelFields` imports.
  Objects: as written; no visibility changes were needed (every item a
  sibling reads was already public). The module doc's link `[`links`]` is
  written `[`links()`]` because `objects::links` is now both a private
  module and the re-exported function. `objects/tests.rs` gains
  `StoredPage`, `ROW_ADDRESS_BASE`, `ROW_ADDRESS_ROW_24`, `Triplet`,
  `TripletFunction`.
- Step 10 (enhance), one commit for the directory (10a to 10d):
  - Departure (10c): `RowCells` is defined in enhance.rs beside the other
    records the children share, with its `impl` (new, next_row, forget,
    place) in sizes.rs, instead of gaining the three accessors the step
    proposed. `Rows::row` (row.rs) writes `cells.resolved` and the tests read
    `cells.cells`, `cells.resolved` and `cells.claims`, so the accessors
    would not have been enough; with the struct in the parent every file
    under enhance/ sees its fields and no new code is written.
  - 10a: `flash_of` is in `enhance/additional_flash.rs` behind
    `#[cfg(feature = "additional-flash-functions")] mod additional_flash;`,
    without its old `if !cfg!(...) { return None; }`; attributes.rs holds the
    `#[cfg(not(...))] pub(super) const fn flash_of(..) -> Option<Flash> { None }`
    stub (with the comment the old guard carried) and re-exports the gated
    one under the same name, so callers name one path. Without the feature
    nothing constructs `Rate::Incremental`/`Decremental` any more, so `Rate`
    carries `#[cfg_attr(not(feature = "additional-flash-functions"),
    allow(dead_code, reason = "..."))]`.
  - Visibility: what the files read of each other is `pub(super)`, found by
    compiling and then narrowed by removing each and recompiling (all
    features, and every feature shape after): in attributes.rs `Rate`,
    `Flash`, `ATTRIBUTES`, `LEVEL_1_FLASH`, `Attrs` (and its fields
    foreground, background, flash, conceal, boxed, double_height,
    double_width, underline, invert; `PASSIVE`, `apply`), `AttrChange` (its
    fields foreground, background, flash, display, designation; `then`),
    `flash_rate`, `flash_of`; characters.rs `EnhChar`, `enhancement_glyph`,
    `is_text`; planes.rs `Plane` (`of`, `may_invoke`, `sets_colours`,
    `source`), `type_index`, `Area`, `area_of`, `scan_index`; machine.rs
    `traced_action`; row.rs `Resolved` (fields cell, double_height,
    double_width), `cell_of`, `Rows::row`; sizes.rs `RowCells::{new,
    next_row, forget, place}`; walk.rs `Collector::new`, `Walker::{new,
    walk}`; walk/invocation.rs `default_object`, `invoke`;
    walk/placement.rs `column_triplet`. Fields of `Attrs`, `AttrChange` and
    `Resolved` are `pub(super)`: they are plain records read and built by
    row resolution, a departure from "fields stay private" that the step's
    seams imply.
  - `enhance/tests.rs` imports `Flash`, `Rate`, `flash_of`, `flash_rate`,
    `enhancement_glyph`, `is_text`, `area_of`, `scan_index`, `cell_of` from
    their files and the names it took from enhance.rs's use lines.
  - `cargo clippy --fix` rewrote the new `impl<'a, 's> Walker<'a, 's>`
    headers of placement.rs and invocation.rs as `impl<'s> Walker<'_, 's>`
    (pedantic `elidable_lifetime_names`).
- Step 11 (render), one commit for the directory:
  - 11a as written. Of the step's `pub(super)` list only these were needed
    (each removed and recompiled otherwise): `resolved_colours`,
    `palette_row_allowed`, `flash_shown`, `widen`, `glyph_line_20`,
    `CELL_MASK`, `SEPARATED_COLUMNS`, `mosaic_block`, `mosaic_line`,
    `PANEL_AREA_END`, plus `CELL_LAST_HALF_DOT`, which `CellLine::index` in
    render.rs reads. `displayed_colours`, `rate_bit`, `panel_widths`,
    `check_mode` and `write_line` are called only within their own file and
    stay private. `font_line_20` keeps `pub(crate)`; its unused re-export in
    render.rs is gone, and the font tests import it from raster.
  - Departure (11b): the tests are not redistributed function by function.
    The three test modules each define their own `options` helper with a
    different signature (none, `(mode)`, `(mode, unit, reveal)`), and the
    concept files the step names would put two of them in one file
    (`changes/tests.rs`: Level 2.5 change tracking with `options(mode, unit,
    reveal)` and render.rs's with `options()`; `raster/tests.rs`: Level 2.5
    raster tests and the font tests), which only renaming, that is rewriting,
    the tests would resolve. Instead, every test moved unchanged:
    render.rs's tests to `render/tests.rs`, `render/level25_tests.rs` to
    `render/tests/level25.rs` and `render/font_tests.rs` to
    `render/tests/fonts.rs`, children of `render::tests` (their `use
    super::*;` reaches render's items through it). The `*_tests.rs` form
    leaves the crate as the step wants; `render/test_grids.rs` is not made.
    This is a variant of the step's own fallback (one `render/tests.rs`).
    `render/tests.rs` imports the private raster and row items the three
    use; each file gains the imports it took from render.rs.
  - Measurements (release, ignored tests; machine load average about 12,
    other agents building): before, flash_cost per phase change, 3 rows:
    rebuild + all rows 796.4 µs, all rows 744.9, changed rows 124.2; 23 rows:
    799.3, 748.5, 722.8; worst-case Level 3.5 grid 0.1 ms, 204 triplets.
    After: 3 rows 796.1, 745.5, 124.3; 23 rows 802.0, 748.2, 722.0;
    worst-case grid 0.1 ms, 204 triplets. (The 1-row case varied between
    runs under load: 855.5/1187.4/134.7 before, 864.7/748.5/64.5 after.)
    The other performance tests skip without NEOTELETEXT_CORPUS.
- Step 12 (viewer): as written, with no visibility change: `select` is a
  private method of `Viewer` in viewer.rs, which keys.rs, a child, already
  sees, so it needs no `pub(super)`. `Key` is still at viewer.rs:20, so
  `docs/spec/design/hmi.md:18` stays right. `viewer/tests.rs` gains
  `StoredPage`, `SPACE`, `Grid`.
- Step 13 (forms): as written. `pub(super)`: `FieldSpec::{cells, cells_at}`
  (forms.rs's `new` and draw.rs), `MARKER` (draw.rs), `chosen_option`
  (draw.rs), and `valid_email`, `valid_number`, `valid_date`, `valid_time`
  for the tests; `valid_tel` is not called by the tests and stays private.
  `field` is a private method in forms.rs, already visible to its child
  files, so it needs no `pub(super)`. `forms/tests.rs` gains `to_unicode`,
  `Glyph`, `Grid` and the four validators. Line citations to fix
  (integrator): `docs/spec/design/hmi.md:19` cites `forms.rs:250` for
  `FormKey`, now `forms/editing.rs:13`; `hmi.md:20` cites `forms.rs:34` for
  `MULTITAP_US`, now `forms.rs:33`.
- Step 14 (glyphs): `glyphs/mod.rs` -> `glyphs.rs` (git mv); `glyphs/loaded_font.rs`
  as written. Departure: `GlyphSet::loaded` in glyphs.rs read the private
  field `LoadedFont::dot_matrix`; it now calls the existing public
  `LoadedFont::is_dot_matrix()`, which returns that field, so the field
  stays private (one expression changed, same value). Departure: the tests
  of `glyphs.rs` go to `glyphs/tests.rs` (gated on `bedstead` as before),
  but `conformance.rs`'s tests go to `glyphs/conformance/tests.rs`, not into
  the same file: they are gated on `test` alone and run in the `render`
  build without Bedstead, which a shared bedstead-gated file would drop, and
  as a child of `conformance` they need no visibility change
  (`FIRST_CODE`, `LAST_CODE` stay private). Integrator:
  `docs/spec/design/languages.md:172` cites `glyphs/mod.rs:140`
  (`LoadedGlyph`), now `glyphs/loaded_font.rs:42`.
- Step 15 (charset): `charset/mod.rs` -> `charset.rs` (git mv); `tables.rs` is
  replaced by the thirteen files (the step's twelve plus
  `extended_designations.rs`, gated) by the step's line mapping, each
  `#[rustfmt::skip]` on its `mod` line; the section banners are dropped,
  each file's doc says what it holds. Line mapping as written except that
  designation.rs takes 44-110 and 164-245 and extended_designations.rs
  112-163 (the step's "41-113" and "114-163" straddled the
  EXTENDED_TABLE_32 doc comment). The `tables::X` paths in charset.rs name
  the new files (17 sites). `scripts/arabic-sets.py`'s `TARGET` is now
  `charset/arabic_script.rs`; regeneration is byte-identical.
  - `pub(super)`: the tables and functions the other table files, charset.rs
    or the table tests read (the list is in the commit message); the empty
    `cfg(not(extended-sets))` twins stay private in designation.rs.
  - Departure: the two test modules are not merged into one
    `charset/tests.rs`: both define a `latin` helper (one returns `G0` from
    a `NationalOption`, the other `CharSet` from an `Option`), so merging
    would mean renaming. `charset.rs`'s tests go to `charset/tests.rs`, and
    `tables.rs`'s tests to `charset/tests/tables.rs`, a child of it declared
    `#[rustfmt::skip] mod tables;` because the test module was inside the
    rustfmt-skipped `tables` before (the file keeps its exact layout). The
    table tests import the items they read from the table files
    (`ENGLISH_DESIGNATION`, `NATIONAL_BITS_MASK`, `base_unicode`,
    `COMPOSITIONS`, `EXTENDED_COMPOSITIONS`, `search`, `COMBINING_MARKS`,
    `diacritic_combining`, `FULL_BLOCK`, `MOSAIC_LEFT_COLUMN`,
    `MOSAIC_RIGHT_COLUMN`, `mosaic_unicode`) and the constants tables.rs had
    imported.
  - The import blocks of the rustfmt-skipped table files were formatted by
    running rustfmt on the blocks alone.
  - Integrator: `ARCHITECTURE.md:372`, `IMPLEMENTATION-LOG.md:17` and
    `TECH-DEBT.md:85` name `charset/tables.rs` (step 15 says to update them;
    this branch does not touch those files).

##### Numbers (crates/neoteletext-core/src, measured by a script that counts
##### brace-matched spans; test files are tests.rs, *_tests.rs and files under
##### a tests/ directory)

| | before (e7d8927) | after (c4b8958) |
|---|---|---|
| files | 36 | 138 |
| lines | 25,556 | 26,289 |
| largest file | charset/tables.rs 2,414 (code); glyphs/bedstead.rs 2,076 generated | glyphs/bedstead.rs 2,076 (generated); largest code file acq/enhancement.rs 464 (unchanged) and charset.rs 467 |
| median file | 487 | 141 |
| files over 500 | 17 | 6: bedstead.rs (generated) and five test files (render/tests/level25.rs 847, forms/tests.rs 678, charset/tests/tables.rs 602, acq/tests.rs 539, packet/tests.rs 514) |
| functions over 80 lines | 3 (push 84, every_transition_of_the_walk 104, Viewer::key 97) | 3, the same functions, moved whole (push 84, the test 99, key 97); no function was split, as the document asks |
| impl blocks over 300 lines | 4 (Decoder 724, Walker 453, FormModel 457, Viewer 331) | 1 (the reception `impl Decoder` 422 in acq/reception.rs) |
| test lines inline / in test files | 5,028 / 1,185 | 12 (lib.rs's error_trait test) / 6,306 |
| core unit tests (default features / all features) | 240 / 247 | 240 / 247 |

##### Gate times (machine shared with other agents' builds, load average 11 to 15)

- The commit hook (`scripts/check.sh fast`, whole workspace) took about 22
  minutes for the first commit (cold target directory, heavy load) and
  roughly 5 to 15 minutes for later ones.
- Clippy of the twelve core feature shapes (scratch script, warm): 48 s to
  a few minutes per run.
- `cargo test -p neoteletext-core`: about 1 to 12 s of test time.
- Release performance tests: 2 min 15 s (flash_cost's 100 s dominates).

##### Final gates (after c4b8958)

- `cargo xtask features`: all 46 builds ok (the twelve core rows and every
  other crate's), 23 min 17 s under load. No row failed, so nothing is
  blamed on the base tree either (only the `[]` core row was baselined).
- `cargo xtask size --target x86_64-unknown-none`: ok, 3 min 21 s. flash /
  static RAM: level1 27597/872, level15 64445/904 (budget 65536), level35
  91477/2552, modern-classic 88725/1208, core-all 139189/4168, t42
  31309/1336, formats 73228/1096, web 88957/1112, web-all 107333/3256.
  Not measured before the moves (the load made a second full build
  impractical), so these are after-numbers only; the Cortex-M0 run
  (`cargo xtask size` on the Linux host) that the document's risk table asks
  for before step 4 and after steps 10, 11 and 14 is the integrator's.
- Every commit passed the pre-commit hook (`scripts/check.sh fast`); no
  `--no-verify`.

##### For the integrator

1. Regenerate `docs/traceability.md` (`cargo xtask traceability`): about 244
   core paths moved files.
2. Line citations: `docs/spec/design/hmi.md:19` (`forms.rs:250`, FormKey ->
   `forms/editing.rs:13`), `hmi.md:20` (`forms.rs:34`, MULTITAP_US ->
   `forms.rs:33`), `hmi.md:18` (`viewer.rs:20`, still right),
   `docs/spec/design/languages.md:172` (`glyphs/mod.rs:140`, LoadedGlyph ->
   `glyphs/loaded_font.rs:42`); `ARCHITECTURE.md:372`,
   `IMPLEMENTATION-LOG.md:17`, `TECH-DEBT.md:85` (`charset/tables.rs` -> the
   thirteen files under `charset/`, the Arabic block in
   `charset/arabic_script.rs`); `third_party/bedstead/PROVENANCE.md:14` if it
   names `glyphs/mod.rs`; the browser's `docs/hyperlinks.md:67` names
   `level1.rs` (at the re-pin).
3. `ARCHITECTURE.md` section 3: one sentence per new module directory
   (step 16); section 9 and the xtask layer table need no change (first path
   component unchanged; `cargo xtask check-layers` passes).
4. The implementation-log row for phase E1 and `TECH-DEBT.md` entries (none
   added here).
5. No new crate root, so no `NO_PANIC_ROOTS` change; no public path changed,
   so no importer in the other crates, the fuzz targets, the firmware image
   or the browser changes. No Cargo.lock changed.
6. `scripts/check.sh full` (including the Cortex-M0 size check on the Linux
   host) before `implementation` moves.

### 4.2 `11-engine-formats-slicer-image.md` (branch `structure/formats`, merged at aee10fe)

#### Notes: 11-engine-formats-slicer-image.md carried out

Branch `structure/formats`, worktree `.claude/worktrees/structure-formats`, from `e7d8927` (one commit after the document's reading at `fb2dd8a`; that commit touched none of the three crates, so the document's line numbers hold).

##### Baseline (untouched tree, 23 October 2026)

- `cargo fmt --all --check`: clean.
- `cargo xtask features` row `neoteletext-formats [std,tti]` (TECH-DEBT issue 221 says it fails): **passes** on the untouched tree (clippy clean, 82 + 2 + 1 + 2 + 4 tests ok). Nothing to fix; the issue's cause is not in this tree.
- `cargo xtask size --target x86_64-unknown-none` (there is no `formats-all` row on this target; the rows that exercise this crate are `t42` and `formats`):
  `t42 31313 flash / 1336 static RAM`, `formats 73220 / 1112`.
- Slicer throughput (`cargo test -p neoteletext-slicer --release -- --ignored throughput`): 16979 lines/s with packets, 29388 lines/s empty (required 8750). Measured while another cargo build was running on the machine.

###### Structure numbers before (measure.py, brace-matching; integration `tests/` counted in files, not in test lines)

| | formats | slicer | image |
|---|---|---|---|
| files | 46 | 8 | 7 |
| largest / median | tti.rs 955 / 278 | tests.rs 1025 / 353 | tests/level25.rs 491 / 137 |
| over 500 | 9 | 2 | 0 |
| fns over 80 | 2 (build_subpage 101, write_page 92) | 2 (slicer_transition 157, report_impairments 92) | 1 (png::write 103) |
| impls over 300 | 2 (TsReader 424, TtiReader 635) | 1 (Slicer 475) | 0 |
| test lines inline / tests.rs / test-only | 1021 / 2604 / 308 (document's count) | 254 / 1025 / 0 | 246 / 0 / 0 |

##### Departures and decisions, by step

- Step 1, `tti/tests/forms.rs`: the document lists 255–351, which ends with `considered_warnings_are_kept` (`CK`) and `content_before_any_page_number_is_reported` (`PN`), neither a Form Extension test. Moved 255–337 only; the two stay in `tti/tests.rs` with the other command tests, which the document names as what `tests.rs` keeps.
- Step 1, `bdf/convert/tests.rs`: the document says to rewrite `super::super::…` as `crate::bdf::…`. Not needed: the module path `bdf::convert::tests` is the same inline or in a file, so `super::super` still names `bdf`. Left as it was (a move, not a rewrite).
- Step 1, slicer `tests/reports.rs` calls `biphase_false_packets`, which the document places in `tests/rejection.rs`. It is now `pub(super)` there and `reports.rs` imports it from `super::rejection` (one seam; the alternative, keeping it in `tests.rs`, would put a rejection fixture among the shared ones).
- Step 1, slicer `tests/*.rs`: the section banner comments (`// ---- REQ-… ----`) that headed each section became the `//!` doc line of the file that holds the section; interior banners (timing inside `signal.rs`, untrusted input inside `rejection.rs`) were kept as they were.
- Step 1 gate: the pre-commit `scripts/check.sh fast` took 21 min 50 s (machine loaded by other worktrees' builds).
- Step 2, `PageFunction`: section 4 sends `page.rs` 626–633 (`PageFunction`) to `page/packets.rs`, section 3.2 keeps it in `page.rs`. Kept in `page.rs` (3.2): it is the `PF` line's page-level record, not a packet record.
- Step 2, `RAW_PACKETS`' doc lines (39–40) moved with it; `issues.rs`'s intra-doc link to `PageFile::considered` now names its target explicitly (`(super::PageFile::considered)`), since `PageFile` is not in scope there; the text shown is unchanged.
- Step 3: `encode/tests.rs` reached the core and page names it uses through the parent's `use` lines (`use super::*`); after the split those lines are in the children, so its import block names them (one block changed, no test). The same for `bdf/tests.rs` (step 4: three core limits) and `tti/tests.rs` (step 5: the item, page and core names it and its four children used).
- Step 5, `tti/` seams: the document names `diagnostic`, `issue`, `issue_at`, `set_text` and the nine `fields.rs` items as `pub(super)`. In fact `set_text` is called only inside `form.rs` and stays private, as do `parse_number`, `link_address` and `link_target`; and every record method the dispatcher in `commands.rs` calls in a sibling is `pub(super)`: `end_line`, `emit` (commands.rs); `packed_line`; `fastext_line`, `link_line`, `number_link_line`, `text_link_line`, `cursor_line`; `field_definition`, `option_line`, `form_submission`, `finish_form`; plus `Args` with `new`/`next`/`rest`, `hex`, `decimal`, `flag`, `page_address`, `result_page`, `subcode`, `service_name`, `REGION_LAST`, `CODING_LAST`, `PACKED_LINE`, `link_destination`. A private method in a child `impl` is private to that child, so this is the narrowest that compiles.
- Gate times: step 2 commit not timed (about 20 min, loaded); step 3 6 min 16 s; step 4 4 min 06 s.
- Step 7, `transport/reader/`: the document sends `Section` (with `Push`) to `section.rs` and `Program` to `tables.rs`, with only the types `pub(super)`. Their fields are read across the new files (`tables.rs` reads `Section`'s `active`, `continuity` and `data`; `reader.rs` reads `Program`'s `pid` and `section`), and the conventions keep fields private. Done instead: the two struct definitions stay in `reader.rs`, the spine, where both children read the fields as descendants; `impl Section` (with `new` and `push` now `pub(super)`), `Push`, `Continuity`, `check_continuity`, `crc32` and the CRC and section constants are in `section.rs`; `Program` is used by `tables.rs` from the spine. `crc32` reaches `transport.rs` by `pub use section::crc32` in `reader.rs`, not `pub(super) use` as the document has it: a `pub(super)` import cannot be re-exported `pub` (E0364); `reader` is a private module, so no new public path appears.
- Step 7, `pes/`: `hunt`, `header_byte`, `finish_header` (header.rs) and `unit_byte` (units.rs) are `pub(super)`, called from `pes.rs`'s `step` and from each other. `pes/tests.rs` names the core and constant names it had through the parent's use lines.
- Gate times: step 5 3 min 30 s; step 6 5 min 06 s.
- Step 8: nine per-item `cfg(feature = "sliced")` lines (the document counts ten, with the test module's, which became `cfg(test)`).
- Step 9, `write/pages.rs`: the document adds `pub(super) fn sink(&mut self) -> &mut W` to `write.rs` so that `write_page` need not reach `self.inner`. Not added: `pages.rs` is a child of `write`, so its `impl T42Writer` block reads the private field as a descendant, the same rule the document relies on for `tti/`, `pes/` and `slicer/`; a method would be a new item for nothing. `write/tests.rs` names `encode_page`, which it had through the parent's gated import. The in-place split of `write/tti.rs::write_page` is its own commit (the brief: a function split is always its own commit).
- Gate times: step 7 3 min 00 s; step 8 3 min 44 s.
- Step 10, `author/streams/`: `Objects` (the magazine-1 objects and their references) stays in `streams.rs`, not `magazine_1.rs`: `object_examples.rs` reads its fields, so in a sibling it would need `pub(super)` fields; in the spine both read them as descendants. The seam is 16 functions, not the document's 14+5: the eleven page builders `magazine_1` calls in `presentation.rs` (6) and `object_examples.rs` (5), and the five magazine functions `stream()` calls (`magazine_1`, `links_over_the_table`, `level_35_links`, `default_objects`, `magazine_packets`). The banner comments that headed each magazine section became the `//!` docs of the files. Proof of the unchanged stream: `cargo xtask level25-stream` writes 416 packets and 47 pages; its 46 TTI files are byte-identical to `tests/pages/level25/` (`diff -r`), and `golden_level25` (`authored_tti_source_is_current`) and `level25` pass. The T42 output was not compared with the old code (it is written from the same stream as the TTI); after: SHA-256 d3790ee92b726dee02dc07ec6de768e638444b05a3849f61de6babc29c1be5ff.
- Gate times: step 9 (pages) 4 min 27 s; step 9 (write_page split) 4 min 45 s.
- Step 10 (build_subpage split): `pointer_tables` takes the pointers as a slice, so its loop reads `in pointers` where the moved body had `in &pointers`; a `Pointer` type alias names `(ObjectKind, usize, u16)` for Clippy's `type_complexity`. `place_triplets` takes and returns `last_packet` so its body is unchanged. Stream unchanged (same T42 SHA-256, same TTI files).
- Step 11, `detect/`: `detect` stays `pub fn` in the private `scoring` module and is re-exported `pub use scoring::detect` (the document says `pub(super)` plus `pub use`, which E0364 forbids). `detect/tests.rs` names what it had through the parent's imports, gated as the tests that use them are: `DUMP_BYTES_25` under the all-formats `cfg`, the three `SLICED_*` constants under `sliced`, `encode_page` under `tti`, `Diagnostic`/`DiagnosticKind` always. The module doc's link to `DiagnosticKind::FormatNotCompiled` names its target. `cargo xtask size --target x86_64-unknown-none` after step 11: every row identical to the baseline (`t42` 31313/1336, `formats` 73220/1112). The remote Cortex-M0 measure (`formats`, `formats-all`) was not run: it is the full gate's remote stage, left to the integrator.
- Gate times: step 10 (streams) 3 min 28 s; step 10 (build_subpage) 4 min 15 s.
- Step 12, `slicer/`: every `impl Slicer` method a sibling or the spine calls is `pub(super)`: `load` (black.rs); `search`, `bit3_centre`, `measure`, `gate` (run_in.rs); `find_framing_code` (framing.rs); `collect`, `bit`, `at`, `run_in_position` (bits.rs); and `Config::nominal_amplitude` as the document says. The banner comments inside the old impl became the children's docs. `generator/rng.rs` takes `Rng` and its constants; `generator.rs` re-exports it (`generator::Rng` kept).
- Step 12, throughput (REQ-GEN-013), release, `--ignored throughput`: before step 1, 16979 lines/s with packets, 29388 empty; after step 12, 87221 / 138416. The machine's load (13 to 14, other worktrees building) swamps the difference, so the old code (HEAD before step 12, built from `git archive` in the scratchpad) and the new were run back to back three times: new 163552 / 217302, 110976 / 130236, 71285 / 165688; old 174725 / 200296, 49471 / 114979, 128984 / 74095. No difference that the noise lets one see; every run is far above the required 8750 lines/s.
- Gate time: step 11 2 min 45 s.
- Step 13, `png/deflate.rs`: one addition the move needs, `BitWriter::new` (`pub(super) const fn`), because `write` built the writer with a struct literal and its fields stay private. `png/tests.rs` imports `LENGTH_BASE` and `DISTANCE_BASE` from `super::deflate`. The split of `write` is its own commit: `write` keeps the validation and the scratch split (254–259) and calls `write_header` (260–278) and `compress_rows` (280–346); the document's `write_header` range starts at 254, but the validation and the split scratch are needed by both halves, so they stay in `write`.
- Step 12 gate 2 min 14 s; step 13 gates 3 min 28 s and 2 min 50 s.
- Step 14 (`lines.rs`, the shared bounded line reader): NOT DONE. The document marks it optional and "not a move"; it changes `TtiReader` and `EditTfReader`'s fields and asks for the `formats` and `tti_enhancement` fuzz targets to be run against their corpora, which is the full gate's remote stage. Left for the owner.
- The helper scripts used for the moves are in the session scratchpad (`structure/assemble.py`, `multimove.py`, `imports.py`, `verify.py`, `measure11.py`); `measure.py` there was overwritten by another agent during the run, so `measure11.py` is this document's copy.

##### After the last step

- `cargo xtask features` (whole workspace, 41 rows): every row ok, 16 min 42 s (loaded machine).
- `cargo xtask size --target x86_64-unknown-none`: ok; every row identical to the baseline (`t42` 31313 flash / 1336 static RAM, `formats` 73220 / 1112, and the core and web rows unchanged).

###### Structure numbers after (same script, `measure11.py`, run on the base commit `e7d8927` and on the last commit)

| | formats before | formats after | slicer before | slicer after | image before | image after |
|---|---|---|---|---|---|---|
| files | 46 | 94 | 8 | 25 | 7 | 9 |
| largest | tti.rs 955 | consts.rs 410 | tests.rs 1025 | generator.rs 368 | tests/level25.rs 491 | tests/level25.rs 491 |
| median lines | 278 | 147 | 353 | 136 | 137 | 137 |
| files over 500 | 9 | 0 | 2 | 0 | 0 | 0 |
| functions over 80 | 2 | 0 | 2 | 2 | 1 | 0 |
| impl blocks over 300 | 2 | 0 | 1 | 0 | 0 | 0 |
| test lines inline / tests.rs / test-only | 1041 / 2604 / 308 | 230 / 3499 / 308 | 203 / 1025 / 0 | 141 / 1186 / 0 | 257 / 0 / 0 | 140 / 116 / 0 |

The two slicer functions over 80 that remain are the ones the document keeps: `fsm::slicer_transition` (157, the REQ-FSM-002 table, with its `allow` and reason) and the ignored test `report_impairments` (92, now in `tests/reports.rs`). Files over 400 after: `consts.rs` 410, `bdf/source.rs` 406, `bdf/reader.rs` 403 (the document estimated ~310; it is the parser with its 261-line impl), `image/tests/level25.rs` 491. None over 500. The inline-test counts of this script differ slightly from the document's (it counts the attribute lines of each module); both columns are measured the same way.

##### For the integrator

- Regenerate `docs/traceability.md` (`cargo xtask traceability`): every moved file path changes it. Not done here, as instructed.
- No public path changed, so nothing in another crate, a fuzz target, `firmware/test-image`, `xtask` or the browser was edited; the workspace Clippy and tests at every commit, and the 41 feature rows, compiled the importers in this repository. The fuzz workspace and the firmware image were not built here (full gate).
- No new crate root, so no `NO_PANIC_ROOTS` entry.
- `ARCHITECTURE.md`'s line for phase E2, `TECH-DEBT.md` (issue 221: the `neoteletext-formats [std,tti]` row passes on the untouched tree and after) and `IMPLEMENTATION-LOG.md` are untouched.
- The remote Cortex-M0 size measure (`formats`, `formats-all`, `cargo xtask size` on the Linux host) was not run.

### 4.3 `12-engine-web.md` (branch `structure/web`, merged at 2c434ac)

#### 12-engine-web.md: notes for the integrator

Branch `structure/web`, from `e7d8927`.

##### Departures and decisions

- **Step 1 (proxy).** As written. `proxy/tests.rs` imports `crate::http::HeadCollector` itself (the parent no longer imports it). `tunnel.rs` keeps its tests inline as the document says (std-only, one `cfg`).
- **Step 2 (http).** The document said "Visibility: none beyond the re-exports". The head parser's property-test generators read `head::is_token_character` and `head::is_field_text`; with the tests in `http/tests.rs` these two became `pub(super)`. Nothing else widened.
- **Step 3 (url).** The document kept the lexical pass in the parent and estimated it at 330 lines; it measured 508, over the 500 ceiling. `check_characters`, `scheme_end`, `split_parts` and `parse_port` went to a fourth child, `url/lexical.rs` (85 lines; the first three `pub(super)`). `Parts` stayed in the parent beside `Url`, so its fields stay private.
- **Moved tests are not re-indented by hand.** Steps 1 to 3 dedented the moved test bodies with `sed`; none of those holds a multi-line string literal whose bytes that changes (checked: the one continued string in `http/tests.rs` uses `\` line continuation, which skips the leading spaces). From step 4 on, the bodies are moved as they are and `cargo fmt` re-indents the code, which leaves string literals byte for byte.
- **Step 7 (zxnet).** `Channel`'s fields stay private to `zxnet/channel.rs` ("fields stay private"); `feed.rs` reads them through the accessors `Channel` already had (`address()`, `identifier()`) where the old code read `self.channel.address` and `self.channel.identifier`. The parent imports `walk::page_index` for `page_exists`. The extraction of `send_frame`, `send_close` and `walk_deadline` is its own commit before the move.
- **Steps 8 to 10 (agent).** The shared forms (`send_push_frame`, `send_push_close`, `deliver_push_page`, `complete_form`, `push_tick`, `wipe_cookies`) are one commit before the moves. `send_push_frame` returns whether the frame fitted (`send_input` reports `InputDropped` when it does not; the document's signature had no result). `complete_form(exchange, head)` reads the body from the buffer itself (passing it would hold a borrow of the buffers across `&mut self`). `push_tick` carries `tick`'s `cfg!(feature = "push")` test with it, so that test is in `push.rs`, not in `tick`. The document's one commit per child file is one commit per step (9 and 10). Visibility is computed from the callers: `pub(super)` only where another file calls; the document's lists differ in places (for example `send_subscribe` stays private, every caller being in `push.rs`; `heartbeat_due` is `pub(super)`, which the document did not list). `agent/bound.rs`'s `use super::{...}` imports `cookie_store` from `super::cookies` (one line of a file the document called unchanged). The child `impl` blocks use `UserAgent<'_, ...>`, as Clippy asks (`needless_lifetimes`). The section banners (`// --- Requests ---` and the like) are dropped with the sections that became files.
- **Static memory after the agent's shared forms.** The size check's `web-all` image went from 107,345 bytes of flash and 3,256 of static RAM to 106,073 and 4,520, `web` from 88,961 and 1,112 to 88,945 and 1,128: about 1,270 bytes moved from flash to static memory, the total the same. No panic path in either image. The cause was not investigated (a guess: a table the compiler now emits as relocated data); it may be worth a look by whoever owns the size budget.
- **Step 10, the parent.** The document kept `handle`, `tick` and `next_deadline` in `agent.rs` at about 470 lines; with them it measured about 530, over the 500 ceiling. They went to a new child, `agent/input.rs` (117 lines; `handle` stays `pub`, the other two private). `agent.rs` is 412 lines: the state types beside the struct, `UserAgent`, construction, the accessors, `seconds`, `event`, `mod bound` with its `not(dbsc)` stub, and one sorted block of `mod` lines.
- **Step 11.** As written, with the corrected test form. `json.rs` is 496 lines and `ws.rs` 429 (the document said 495 and 430).
- **Step 12 (optional `ws/` split): not done.** The document makes it optional and leaves `ws.rs` at 429 lines of one RFC's sections 4 and 5.
- **`impl` blocks over 300 after the work:** `agent/push.rs` (357) and `zxnet/feed.rs` (335) are each one `impl` block making up the file, as the document's target layout has them (350 and 330); they are left as the document planned.
- **The fuzz targets** (eight imports of this crate) build and pass `cargo +nightly clippy --locked -- -D warnings` in `fuzz/` against the reorganised crate (38 s); `cargo fuzz` itself is not installed here and was not installed.
- **Test modules (all steps, the cross-cutting correction of 24 October).** The document puts the test `#[allow(...)]` on the parent's `mod tests;`. The binding correction moves it into each `tests.rs` as its first line (`#![allow(...)]`, the same list the inline module had), and the parent's `#[cfg(test)] mod tests;` is the last item of the parent. Steps 1 and 2 were committed with the outer form; the step 3 commit brings `proxy/tests.rs` and `http/tests.rs` to the corrected form. Each `tests.rs` keeps a `//!` line after its `#![allow]`.
- **Commit attribution.** No `Co-Authored-By` trailer: the owner's instructions forbid crediting an AI, and they take precedence over the session's default.

##### Numbers

Before (`e7d8927`, measured with the quick script `measure_web.py`, which counts raw lines from signature to closing brace and skips braces inside literals and comments):

- source files 24; lines 15,304; largest 2,767 (`agent.rs`); median 463
- files over 500: 10 (`agent` 2767, `cookie` 1411, `dbsc` 1186, `descriptor` 1056, `zxnet` 1031, `http` 899, `url` 867, `ws` 630, `proxy` 604, `json` 576)
- functions over 80 lines: 9 (`store_within` 116, `complete` 108, `dispatch` 103, `parse_with` 101, `bound_complete` 94, `parse_services` 86, `submit` 82, `push_frames` 81, `handle` 81), as the document has them
- impl blocks over 300: 4 (`impl UserAgent` 1811, `impl CookieJar` 498, `impl Descriptor` 445, `impl ZxnetFeed` 436)
- inline test module lines: 2,534 (from `mod tests {` to its brace; the document's 2,660 counts the attribute lines too); `tests.rs` lines 0
- feature `cfg` and `cfg!` sites in `src/`: 38
- integration test lines: 2,641

After (`109010b`, the same script):

- source files 73 (the document said 66; two of the extra files are the departures `url/lexical.rs` and `agent/input.rs`, and the rest were not reconciled with its count); lines 15,658; largest 496 (`json.rs`); median 178
- files over 500: 0
- functions over 80 lines: 7, all moved unchanged (`store_within` 116, `dispatch` 103, `parse_with` 101, `bound_complete` 94, `parse_services` 86, `submit` 82, `handle` 81); `complete` (108) and `push_frames` (81) dropped under 80 with the shared forms
- impl blocks over 300: 2 (`agent/push.rs` 357, `zxnet/feed.rs` 335), from 4 (largest 1,811)
- inline test module lines: 330 (the small modules that stay inline, and `proxy/tunnel.rs`'s std-only test); `tests.rs` lines 2,240
- feature `cfg` and `cfg!` sites in `src/`: 30, from 38 (`proxy.rs`'s eleven item-level `std` cfgs became one `mod tunnel`)
- integration test lines: 2,641, unchanged
- the bare-metal images: `web` 88,945 bytes of flash and 1,128 of static RAM, `web-all` 106,073 and 4,520; no `panicking`, `_fail` or `failed` symbol in either (`cargo xtask size --target x86_64-unknown-none`: ok)

##### The full checks at the end

- `cargo xtask features` (every crate's feature list): ok, 13 min 15 s.
- `cargo xtask size --target x86_64-unknown-none`: ok, 2 min 8 s (numbers above).
- `cargo +nightly clippy --locked -- -D warnings` in `fuzz/`: ok.

##### For the integrator

- Regenerate `docs/traceability.md` (`cargo xtask traceability`): it names source paths, and every file of this crate but the small ones moved. Not done here, as the brief says.
- No public path changed: every importer's path (browser, engine crates, fuzz targets, xtask) is kept by `pub use`. The browser's re-pin needs no import change for this crate.
- No new crate root, so no `NO_PANIC_ROOTS` entry.
- `ARCHITECTURE.md`'s L7 bullet names `contents`, `channel`, `dbsc` and `agent::bound`, all unchanged paths.

##### Gate times

The feature-shape script (`gate-web.sh`: fmt, Clippy default, none, and each of push, forms, cookies, dbsc, std, error-trait alone, tests with no features and with dbsc, and the size check where the document asks) and then the pre-commit hook (`scripts/check.sh fast`):

- Step 1: script 540 s (cold for most shapes); hook about 15 minutes under the load of other agents' builds.
- Step 2: script 98 s; hook about 12 minutes.
- Step 3: script 168 s; hook about 10 minutes.
- Step 4: script 378 s with the size check (`web` 88,961 bytes flash and 1,112 RAM, `web-all` 107,337 and 3,256: no panic path); hook about 10 minutes.
- Steps 5 and 6: script under a minute each (warm), the size check the same numbers after step 5; hook about 10 minutes each.
- Step 7 (two commits): script 225 s with the size check (`web-all` 107,345 flash: 8 bytes more); hook about 10 minutes each.
- Steps 8 to 11 (five commits): script 103 to 213 s each (size check in each agent step); hook about 10 minutes each.
- Every hook run passed first time; no timing-sensitive test failed.

### 4.4 `13-engine-app-cli-audio-input.md` (branch `structure/app`, merged at dd4f0b1)

#### notes.md: 13-engine-app-cli-audio-input.md on branch structure/app

Untracked; for the integrator. Base e7d8927; 16 commits, A1 to Q5 in the document's order.

##### Not done here (for the integrator)

- **X1-X4 (xtask) skipped as instructed**: another branch edits `xtask/src/checks.rs`; the integrator carries them out after merging it.
- **docs/traceability.md not regenerated**: every crate here moved files; the integrator regenerates it once.
- **ARCHITECTURE.md**: its lines on where code lives in these six crates are the integrator's; not touched.
- **NO_PANIC_ROOTS**: no new root with a deny list. The CLI gains `src/lib.rs`, a new crate root, but the document gives the CLI no panic list (section 5: app and CLI have none), so it has none; the engine's `check.sh` has no `NO_PANIC_ROOTS` list to add it to.
- **No Cargo.lock change**: the CLI's new library target is the auto-detected `src/lib.rs`; `Cargo.toml` and both lockfiles are unchanged.
- **Browser**: no path it imports from audio, audio-mix, input or qrkeyboard moved. The one changed public path is in neoteletext-app (`live::descriptor_service_names` is now `descriptor::descriptor_service_names`), whose only importer, the CLI, is updated in the same commit.
- **For X1**: `xtask`'s `non_test_source` cuts a file at its first `#[cfg(test)]`, and reads a `tests.rs` file whole (it has no such marker), in `check-constants` and, for the library crates, `check-io`. None of the tests moved here trips either check, but a future `tests.rs` using `std::thread` in audio, audio-mix or input would; `non_test_source` might skip files named `tests.rs`.

##### Departures and decisions

- **Tests last**: every `#[cfg(test)] mod tests;` line is the last line of its file, so the checks' cut leaves the code above it read. The first commit had it among the mod lines of `present.rs`; the second commit moves it to the end.
- **Batching**: 16 commits. Moves are batched per crate where the document has one commit per file (A1-A4, A5-A9, C1-C9, U1-U6, I3-I6, Q1-Q4); every shared-form extraction is its own commit (A10, C10, C11, C12, C13, C14, I2, Q5); I1 is its own commit because I2 builds on it in the document's order.
- **Visibility, narrower than the document**: a child module reaches its parent's private items, so where the document asks for `pub(super)` on a parent item used only by its children, it stays private: `Live::new` (A7); `selected`, `grid_of`, `mosaic_style`, `display`, `scale_for`, `page_file_name` (C4); the receiver's `bump`, `needs`, `duration_ms` (U5); the event constants and `check_text` (I4); screen's `connect`, `send_text`, `send_binary` (Q4). `pub(super)` is used only between siblings.
- **Visibility, wider than the document lists** (seams for the reviewer): `Sets::code_in` and `MarkedLine` (A9); `Wakes::waits`, a test-only field read by `fetch/tests.rs`, is `pub(super)` under its `#[cfg(test)]` (A8); `RawGuard::enhanced` (C9) and `Wave`'s two fields (C6) are `pub(super)`; `Fields::bytes` (U2); `Ramp::level` and `Ramp::target` (M1); `Sealer::new`, `Opener::new`, `spake2::reduce`, `spake2::SCALAR_BYTES`, `hkdf` (Q2); `net::{MAX_HEAD, READ_CHUNK, tls_config}` (Q3).
- **A2**: the side-panels test of `plain` stays in the CLI, as the document chooses; the other three moved to `present/tests.rs`. The `grid_with` helper is in both test modules (the CLI's ANSI tests still use it).
- **A3**: `feed.rs` also imports `load` from its parent (no visibility change).
- **A8**: the proxy tests go to `fetch/transport/tests.rs`, as step A8 says (section 3.1's tree puts them in `fetch/tests.rs`, which could not reach `reach`, `open` or `Connection::receive`). The field `Wakes::waits` writes `std::sync::Arc` in full, so that no `#[cfg(test)] use` line sits at the head of `wakes.rs`, where it would hide the file from the checks.
- **A9**: `set_names` stays in `write_text.rs` (section 3.1's tree); step A9's `pub use sets::{SETS, set_names}` became `pub use sets::SETS`.
- **A10**: the document's `Session::trace_sink(&mut self)` cannot be called while `self.decoder` is borrowed for `push` or `tick`; it is an associated function `trace_sink(tracing: bool, trace: &mut Vec<Event>)` instead.
- **C1**: `Cli::command`, `Command` and `FontCommand` are `pub(crate)`, since `run.rs` reads them from `args.rs`; `run` carries `#[must_use]`.
- **C3**: `frame.rs`'s flash-timing test module is `#[cfg(all(test, feature = "view"))]`, since its one test needs `Frame::at`.
- **C4**: `commands/view.rs` imports what only its `view` build needs in one `#[cfg(feature = "view")] use { ... }` group.
- **C6**: `WAVE_PCM` lives in `sound/wave.rs` (the document puts it in `sound/run.rs`), because only `read_wave` and its tests read it. The stub's two `use` lines are gated `not(feature = "sound")`.
- **C8**: `TEXT_MAX_FILE_BYTES` lives in `write_text/run.rs` (the document keeps it in the ungated parent), where a build without `write-text` would hold it dead.
- **C9**: `view/terminal.rs` is `view/terminal_io.rs`: a `mod terminal` in `view.rs` would clash with the `crossterm::terminal` that `run` uses.
- **C10**: 17 sites, all in the CLI; the document's other sites are in neoteletext-app (which cannot call a CLI function) or name a relay or a channel rather than a path.
- **C11**: the old `commands::sandbox(options, writable)` became `sandbox_writable(writable, no_sandbox)`, which `sandbox_closed(no_sandbox)` calls and `render` uses; app's `self_test` keeps its own call.
- **C12**: the three checks accepted the same pages but worded the refusal differently; `page_number` takes the form and wording two of the three had (`PageAddress::from_mpp`, "<text>: not a page number 100 to 8FF"). The one changed message is `neottx qr`'s (it said "<text>: a page is 100 to 8FF"); no test reads it. `page_number` is gated `any(keyboard, sound, write-text)`.
- **C13**: `clock.rs` is gated `any(view, keyboard)`, as `nfc.rs` is, not ungated as the document says: a build with neither would hold it dead.
- **C14**: `tests/common/mod.rs` allows `dead_code` with a reason, since each test file uses only some helpers. The variants of `neottx` in `web.rs`, `not_in_build.rs` and `enhancement_reporting.rs` return other things and stay; the unit tests' WAVE builder in `commands/sound/wave.rs` cannot reach `tests/`.
- **U1**: `pub use codec::*; pub use profile::*;` as the document writes them (clippy's `wildcard_imports` does not fire on these re-exports).
- **I2**: the document's `input_record(packet, kind: u8)` takes a test of the type byte instead (`impl Fn(u8) -> bool`), so that `is_input_packet`, which accepts either input type and runs on every packet a viewer keeps, decodes the fragment once.
- **Doc links**: moved items whose docs named something no longer imported got path links: `record.rs` (`crate::MAX_RECORD`), `record/frame.rs` (`super::parse`), audio-mix `voice.rs` (`crate::sinc`) and `mixer/stream.rs` (`crate::STREAM_TARGET_MS`), input `carriage.rs` (two), `event.rs` (five limits), qrkeyboard `screen/wait.rs` (`crate::net::Waker`).
- **Q3, Q4**: the parents' imports left unused by the moves were removed with `cargo fix --lib --tests -p neoteletext-qrkeyboard` (imports only; the diff was checked).
- **Q5**: `COLOURS` in `has_key` replaced by config's `COLOUR_KEYS`, as the document says.

##### Found, not changed (rustdoc with `-D warnings`, which `check.sh` does not run)

At the base commit already: `neoteletext-input/src/keys.rs:6` (redundant explicit link target), `neoteletext-input/src/event.rs:5` (`write` is both a function and a macro), `neoteletext-qrkeyboard/src/json.rs:82` (public doc links to private `MAX_ITEMS`). App, CLI, audio and audio-mix document cleanly with `-D warnings`.

##### Before and after (measured by a script over each crate's `src/`, matching each block's closing brace at its opener's indentation)

| Crate | Files | Lines | Largest file | Files > 500 | Functions > 80 | `impl` > 300 | Test lines inline / in tests.rs |
|---|---|---|---|---|---|---|---|
| app before | 14 | 4,742 | fetch.rs 1,071 | 3 (fetch 1,071, write_text 874, live 501) | 2 (run_web 203, live_loop 83) | 0 | 1,514 / 0 |
| app after | 40 | 4,967 | fetch/transport.rs 330 | 0 | 2 (run_web 209, live_loop 83) | 0 | 123 / 837 |
| cli before | 15 | 3,850 | view.rs 866 | 3 (view 866, main 549, commands 540) | 2 (view run 386, sound 94) | 0 | 458 / 0 |
| cli after | 41 | 4,056 | view.rs 469 | 0 | 2 (view run 386, sound 90) | 0 | 302 / 126 |
| audio before | 10 | 2,905 | store.rs 618 | 1 | 1 (define 85) | 2 (Receiver 382, Store 324) | 653 / 0 |
| audio after | 24 | 2,978 | receiver.rs 295 | 0 | 1 (define 85) | 0 | 84 / 429 |
| audio-mix before | 2 | 839 | lib.rs 757 | 1 | 0 | 1 (Mixer 309) | 731 / 0 |
| audio-mix after | 8 | 876 | mixer.rs 284 | 0 | 0 | 0 | 14 / 181 |
| input before | 11 | 3,245 | event.rs 792 | 2 (event 792, session 680) | 1 (write_event 87) | 0 | 851 / 0 |
| input after | 24 | 3,332 | wire.rs 280 | 0 | 1 (write_event 87) | 0 | 272 / 536 |
| qrkeyboard before | 10 | 2,873 | net.rs 613 | 2 (net 613, screen 586) | 0 | 1 (Screen 453) | 335 / 0 |
| qrkeyboard after | 21 | 2,970 | net/websocket.rs 297 | 0 | 0 | 0 | 250 / 87 |

The functions over 80 that remain are those the document leaves whole (view's `run`, `run_web`, `live_loop`, `define`, `write_event`); `sound` shrank to 90 with C12. Test counts are unchanged in every crate (app 28 + 3 moved from the CLI = 31; CLI 23 with the side-panels pair; audio 26; audio-mix 11; input 45; qrkeyboard 17). The lines added are file doc comments, `mod`/`use` lines and the `impl` headers of the split blocks.

##### Gate times (pre-commit `scripts/check.sh fast`, machine shared with other agents)

| Commit | Steps | Gate |
|---|---|---|
| f0da1a3 | A1-A4 | 923 s |
| df78872 | A5-A9 | not timed (about 15 min, under load) |
| c644004 | A10 | 244 s, failed (see below); then 152 s |
| 860919c | C1-C9 | 153 s |
| c86f043 | C10 | 160 s |
| 53557dd | C11 | 196 s |
| 820dad8 | C12 | 116 s |
| d1b184e | C13 | 227 s |
| 0eadec1 | C14 | 189 s |
| 3d7e155 | U1-U6 | 215 s |
| 7df3682 | M1 | 155 s |
| cab3166 | I1 | 151 s |
| e72c4a8 | I2 | 135 s, failed (see below); then 152 s |
| f674aad | I3-I6 | 215 s |
| c091180 | Q1-Q4 | 285 s |
| 5989a68 | Q5 | 217 s |

- **Timing-sensitive test under load**: twice (the commits of A10 and I2) the gate failed in `neoteletext-qrkeyboard --test wake`. Its three tests measure a wake within a limit, and waited 166-437 ms at load averages of 9 to 28. Each time the test passed alone at once (0.3 s); qrkeyboard was untouched by the first. The commit passed on the second attempt both times.
- Each U and I step also ran `cargo build --target x86_64-unknown-none -p <crate>` and `cargo +nightly check` in `fuzz/` (both pass); M1 ran the bare-metal build.

##### The final feature matrix

`cargo xtask features` after the last commit: all 41 rows ok, 2193 s (the whole matrix; the tool cannot be limited to one crate). Nothing to fix. By hand, as the document asks (no row in `features.rs`): `neoteletext-app` and `neoteletext-cli` with `--no-default-features --features write-text`, Clippy with `-D warnings` and tests, both pass; audio-mix and qrkeyboard have no features.

