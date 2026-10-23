# Teletext specification audit — 11 October 2026

*Note of 20 October 2026:* DRCS is no longer decoded (decision E.95), and the Level 3.5 font styles, proportional spacing among them, are no longer drawn (decision E.96), so this audit's findings about them no longer apply.

## Assessment

This is a substantial software teletext implementation. Source inspection supports strong coverage of Level 1 and Level 1.5 behavior and broad implementation of Level 2.5/3.5 packet decoding and presentation. However, “implements the project's requirements” and “fully conforms to EN 300 706” are different claims. The project explicitly omits proportional layout, makes choices where the standard is ambiguous, and has weaker independent validation for objects, DRCS and side panels.

I would describe it as a capable receiver with well-tested basic presentation and extensive advanced functionality, with a documented Level 3.5 presentation limitation and outstanding conformance evidence. A numerical compliance percentage would imply a clause-by-clause certification that this investigation has not performed.

## Scope and sources

Reviewed the packet/ECC, acquisition, character-set, Level 1, Level 1.5, enhancement, glyph, rendering, slicer and transport code, their associated tests, and the external comparison harness. Inspected selected clauses directly in these local documents, extracting text only into `/private/tmp`:

- `~/devel/microcontrollers/teletext/specifications/EN_300_706_v1.2.1_Enhanced_Teletext.pdf`: packet protection, presentation, active position, Font Style and transmission sequencing.
- `~/devel/microcontrollers/teletext/philips-saa5050/SAA5050-55_Teletext_Character_Generator_Philips.pdf`: character generator behavior, flash and rounding.
- `~/devel/microcontrollers/teletext/philips-saa5246a/PHILIPS-SAA5246A-INTEGRATED-VIP-TELETEXT-(IVT1.0)-HANDBOOK.PDF`: receiver and page-memory context.

Other manufacturers' ICs were not individually audited. The software's named behavior baselines are SAA5050 and SAA5246A. This assessment does not establish electrical, register, bus or cycle equivalence to either device. Web forms and service-link extensions are project features, not evidence of broadcast teletext conformance.

## Coverage and confidence

| Area | Source evidence | Assessment |
|---|---|---|
| Signal and framing | `core/src/consts.rs`, `slicer/src/consts.rs`, `slicer/src/slicer.rs` | Correct nominal 6,937,500 bit/s, 16-bit run-in, 0x27 framing in LSB-first representation, 42 packet bytes following framing. Synthetic noise and timing tests exist; real analog robustness needs more evidence. |
| Error protection | `core/src/ecc.rs` | Hamming 8/4 and 24/18 correction/rejection and odd parity are implemented. Hamming 8/4 tests cover single and double errors; 24/18 tests cover every payload, with corruption testing on a sampled subset. |
| Acquisition | `core/src/acq.rs`, `core/src/fsm.rs` | Explicit magazine and page state machines, serial/parallel handling, erase/merge, subpages, independent service packets and bounded storage. Error recovery policies are configurable project choices. |
| Level 1 | `core/src/level1.rs` | Careful set-at/set-after sequencing, conceal/flash, stored held-mosaic form, blast-through text, boxing and lower halves of double-height rows. Tests include Annex G examples. |
| Level 1.5 | `core/src/level15.rs` | Separate X/26 overwrite pass with G2/G3/diacritics, row/column positioning and profile-dependent designation handling. Explicit handling of the Annex A.3 versus clause 12.3.2 interpretation. |
| Levels 2.5/3.5 | `core/src/enhance.rs`, `objects.rs`, `drcs.rs`, `packet/page_data.rs`, `packet/links.rs` | Substantial implementation of colors, panels, objects, DRCS and level gating. Advanced interoperability confidence is lower than basic-page confidence. |
| Rendering and IC profiles | `core/src/render.rs`, `config.rs`, `glyphs.rs` | Useful behavioral profiles, replaceable glyph data and explicit mosaic geometry. Pixel-perfect chip fidelity is not established. |
| Broadcast ingress | `formats/src/pes.rs`, `transport.rs`, `t42.rs` | Streaming parsers and DVB bit-order normalization are present. This review is not a complete EN 300 472 transport conformance audit. |

Paths in this table are relative to `crates/tellytxt-` plus the named crate; for example `core/src/ecc.rs` means `crates/tellytxt-core/src/ecc.rs`.

## Findings

### 1. External comparison can pass when no pages are decoded — high priority

`tests/oracles/compare_cells.py:166` compares only the intersection of page/subpage keys. Missing pages are printed but never contribute to failure. At line 179 it compares rows using `zip`, so missing or truncated rows/cells can also escape comparison. Line 217 fails only for unexplained differences among compared cells.

Reproduced using an empty ttx output and a reference output containing page 100 and one cell. It reported `ttx 0, zvbi 1, compared 0`, `cells compared: 0`, and exited **0**.

`tests/oracles/run_cells.sh` additionally ignores failure of the ttx command with `|| true`. Together, these behaviors permit a decoder failure with empty output to be accepted as an oracle success.

Fix the acceptance criteria to require expected page sets, row and cell counts, nonzero comparisons, and successful decoder execution. Explicitly scoped exceptions may be needed for genuine receiver-policy differences.

### 2. Known differences exempt entire page numbers — high priority

`compare_cells.py:195` discards every mismatch on any page listed in `zvbi-known-differences.txt`. Exemptions are keyed by page number, so they also cover all subpages. The file documents specific reasons, but the executable rule does not limit exceptions to those reasons, coordinates or fields.

For example, a documented Local Object disagreement can mask an unrelated color or sizing regression anywhere else on that page. This does not prove the listed explanations are wrong; it means the harness cannot establish that *all* differences have those explanations.

Replace whole-page waivers with expected mismatch records or narrowly defined predicates, and fail on additional differences.

### 3. Font attributes are emitted but excluded from comparison — medium priority

`compare_cells.py:52` defines `COMPARED_FLAGS = set("ufc")`, checking underline, flash and conceal only. Both `tests/oracles/zvbi_cells.c` and `crates/tellytxt-cli/src/output/cells.rs` emit bold, italic and proportional flags. Nevertheless, these flags are discarded before comparison.

This falls short of REQ-TST-023, which explicitly requests comparison of those attributes. The comparator also limits itself to the 40-column main area; it does not independently validate side-panel presentation or RGB palette values.

Include `b`, `i` and `p` in attribute comparison and add appropriate palette/panel checks. Cell agreement alone cannot validate glyph pixels, flash timing or DRCS bitplanes.

### 4. Proportional spacing is decoded but never laid out — presentation limitation

EN 300 706 Table 29, column mode 01110, defines a proportional-spacing flag alongside bold and italics. `enhance.rs` preserves the flag, but `render.rs:393` applies only bold and italic transformations. The renderer uses fixed-width cells.

This is intentional: REQ-RND-039 and decision E.50 expressly retain fixed pitch. The standard's lack of a prescribed layout algorithm gives implementation freedom, but does not make fixed pitch evidence of proportional display support.

Qualify Level 3.5 support as having fixed-pitch rendering, or implement proportional layout with a documented positioning policy. This is not an accidental failure to satisfy the project's own requirements.

### 5. Advanced features lack sufficient independent broadcast validation — confidence limitation

The implementation log explicitly records that no real broadcast containing complete object pages, DRCS pages or side panels was available. Existing real-data tests check selected Polish X/26 letters and Teefax G3 cells. These are useful, but do not validate the advanced feature combinations.

Authored streams are often created using this project's own encoder and decoded by its own receiver. A shared misunderstanding of a bit field can therefore survive round-trip tests. Golden rasters detect regressions, but do not establish normative correctness unless their initial expected results are independently checked.

Priority targets are independently sourced object/DRCS/MOT packets, cross-magazine links, multi-plane DRCS and mode 3 pixel order, and side-panel addressing. The open questions in REQUIREMENTS.md Appendix D already identify several of these risks. External zvbi differences should be adjudicated against specific clauses or independent vectors, not resolved simply by choosing either decoder.

### 6. IC profiles should be described as behavioral approximations — fidelity limitation

`config.rs` deliberately changes national-option selection, row 24, black attributes, reveal and X/26 handling between profiles. This is useful compatibility behavior. It is not a complete simulation of the SAA5050 or SAA5246A hardware.

G0/G2 glyphs still use the temporary Bedstead source. The SAA5050 datasheet's double-height rounding wording differs from the emulator-derived interpretation adopted by the project; REQUIREMENTS.md explicitly leaves photograph verification open. Parity recovery and several receiver details are also project decisions where hardware evidence is absent.

Separate specification-level semantic tests from chip-specific pixel and recovery tests. Use generated test streams with documented hardware captures to resolve the remaining chip behavior questions.

## Recommended order

1. Repair the oracle's acceptance criteria and narrow its exceptions. This makes subsequent validation meaningful.
2. Compare all font flags and add independent tests for palette and panel output.
3. Qualify the fixed-pitch Level 3.5 claim in public documentation.
4. Add independently checked raw vectors for compositional links, MOT, objects and every DRCS mode; obtain complete real enhancement-page captures when available.
5. Compare chip-profile output against hardware photographs/captures, including double-height diagonals and held-mosaic transitions.

The findings above distinguish confirmed harness defects, deliberate presentation limitations, and missing evidence. They do not establish a new packet-decoding defect in the basic teletext path.

## Verification performed

`cargo test --workspace --locked --offline` completed with exit status 0: **602 passed, 0 failed, 7 ignored** across the reported test results. The initial sandboxed attempt could not bind the local HTTP servers; rerunning with that permission resolved the environmental failure. A separate run of the six non-CLI workspace packages also completed successfully, with 575 passed and 7 ignored.

These totals count successful Rust test functions, including any corpus tests that return early when their data is absent. They must not be interpreted as 602 independent conformance assertions or proof that all external recordings were exercised. The historical implementation log's figure of 606 was not reproduced by this command.

The external zvbi oracle was inspected and its missing-page false success was reproduced with local fixtures. The full external zvbi comparisons, hardware captures, target-platform checks and coverage measurements were not rerun in this investigation. No implementation code was changed; this report is the only repository addition from the audit.
