# neoteletext — software teletext receiver

A software receiver for World System Teletext (ETSI EN 300 706), Presentation Levels 1.0 and 1.5. The behaviour baselines are the Philips SAA5050 (rendering) and SAA5246A (receiver).

The library and `neottx` are implemented; work continues feature by feature. This repository is the core teletext rendering engine only (decision E.76). The windowed browser built on it is maintained in its own repository, `~/devel/neoteletext-browser`; it was removed from this one on 18 October 2026. Do not add browser code to this repository.

**The development process is `~/devel/neoteletext-common/DEVELOPMENT.md`**, shared with the browser since 21 October 2026. Where this file and that document differ, that document holds; its appendix B lists what this repository still has to change to follow it.

## Requirements, decisions, checklists and to-dos: rdm only

They are records in rdm, reached through its MCP tools (search, get_record, list_records, create_record, revise_record, set_status, link_records), since 30 October 2026 (rdm decision DEC-1389). **Never create or edit a file to record one:** there is no `TECH-DEBT.md`, `docs/spec/decisions.md`, `docs/spec/requirements/` or `open-questions.md` any more, and a hook refuses edits to those paths. Old identifiers still work in rdm: `REQ-INP-023`, `E.30`, `F.0.1`, `G.3`, `Open question 4`, `TECH-DEBT 12`. If a merge brings one of those files back, record what it adds in rdm and delete the file.

## Gates and the work branch

@~/devel/neoteletext-common/gates.md

## How work proceeds: rapid iteration, then integration

The owner shapes behaviour by asking for changes and looking at the result. Work therefore runs in two modes (owner's instruction, 13 October 2026).

**Rapid iteration (the default while a feature's behaviour is being shaped).**
- Make the change, then rebuild `neottx` with the fast `iterate` profile, `cargo build --profile iterate -p neoteletext-cli` (seconds, not the release profile's minute and more), and show the result with `target/iterate/neottx` (for example `show` to an image). Check the output, then tell the owner it is ready to look at.
- Do not write tests, user guide text, requirement prose or implementation log rows during this mode, and commit whenever a step compiles: the pre-commit gate is formatting and Clippy only (IMPLEMENTATION-LOG.md row 96). Never bypass the hooks.
- Run the existing tests of the code touched when that is quick, to catch breakage early.
- **Record everything owed as a to-do in rdm** as you go (`create_record`, type `todo`, linked to the requirement or decision it concerns): tests to write, requirements and decisions to record, documents to update, verification to run (full gate, sizes, fuzzing), shortcuts taken and loose ends noticed. Nothing deferred may exist only in a reply.

**Integration (when the owner says a feature works as they expect).**
- Work through that feature's open to-dos in rdm: record the decision and the requirements in rdm, write the tests (each failing on the old code, each naming the requirement it verifies), update the user guide, ARCHITECTURE.md and the other documents, add the implementation log row, and open or close the GitHub issues.
- Commit through the gates in small commits that close their issues, and run the full gate before merging to `main`. Mark each to-do done in rdm (`set_status`, lifecycle `done`, naming the commit) when it is done.

The rules below about issues and requirements apply at integration: behaviour may be tried before it is recorded, but none is committed without its decision and requirements in rdm.

## Key documents

- **Requirements and decisions are in rdm** (above): the requirements (Part F, area SEC overriding every other), the decisions (Part E), the phase plan and milestones (decisions `F.0.1`, `F.0.2`), what is out of scope (decisions `G.n`), the owner's open questions (decisions waiting for confirmation) and the work owed (to-dos).
- **`docs/spec/`** holds how things work: `design/*.md` (Levels 1.0 and 1.5, the SAA5050 and SAA5246A, character sets and fonts, languages, the TTI Link and Form Extensions), `glossary.md`, `source-documents.md` and `module-sources.md`; `docs/spec/README.md` is the map.

  **Each fact has one home.** A requirement states behaviour; a decision states the choice and its reason and links to the design; a design document explains; `ARCHITECTURE.md` says where code lives; `research/` holds the evidence. Link rather than restate.

- `ARCHITECTURE.md` describes the code's layers (Foundation, then L0 Ingress to L9 Applications), their boundary types, trust boundaries and the allowed-dependency table. REQ-ARCH-011 makes it mandatory, and it is enforced by an `xtask` check. Keep it in step with `docs/spec/`.
- `research/notes-*.md` are the cited research notes behind Parts A to E. `research/answers-*.md` are the reports that answered the open questions: SAA5050 behaviour from emulators, the historic specifications, and Rust targets. Check a claim here before changing it.
- `research/jellytext-tti-form-system-spec-v2.md` is the original forms specification from the earlier jellytext project, kept for history. The corrected version 3 is `docs/spec/design/tti-form-extension.md` (formerly Appendix C); decision E.31 explains the changes.
- The source standards and datasheets are in `~/devel/teletext-reference/`. All folder paths cited in `docs/spec/` (`teletext-standards/`, `teletext-fonts/manufacturers/philips/datasheets/saa5050/` and so on) are relative to that directory.

## Rules that must not be broken

- **Security overrides every other requirement** (area SEC, section F.0.3; decision E.32).
  - All input is untrusted.
  - No executable content, ever.
  - Row 0 columns 0 to 7 belong to the receiver alone.
  - Library crates have no operating-system access; enforced by clippy's disallowed lists.
  - `neottx` sandboxes itself: Landlock and seccomp on Linux.
  - Web mode runs as a fetcher process and a decoder process.
  - Plain HTTP is allowed by the owner's decision (E.32), with the safeguards of REQ-SEC-008.

- **No copyrighted source material in this repository.** Do not copy standards PDFs or datasheets here. Link to them or cite them by clause.
- **No GPL code.** zvbi, vhs-teletext, vbit2 and QTeletextMaker may be run as external programs, or used as test oracles at test time. Their code and test files must never be copied into this repository.
- **One bundled font: Bedstead** (decisions E.72 and E.77, REQ-FNT-001).
  - `third_party/bedstead/bedstead-glyphs.txt` is the only font compiled into the receiver. Its licence and legal basis are in `third_party/bedstead/PROVENANCE.md`.
  - The only other glyph data in the repository is the Noto-derived 12 × 20 fonts in `third_party/noto/`, with their converter (decision E.77, REQ-FNT-012). They are loaded at run time, never compiled in.
  - Never add any other font, or any glyphs from character-generator ROMs, to the repository. Other fonts are 12 × 10 or 12 × 20 BDF files loaded at run time (area FNT); the owner's font collection lives in a separate repository.
  - Keep glyphs as replaceable data, never hard-coded.
- **The test corpus is downloaded at test time and pinned by checksum** (REQ-TST-022). Local corpus files, including Dan Farrimond's recovered T42 captures in `~/devel/jellytext/dan/`, are read from `$NEOTELETEXT_CORPUS`. Never commit sample streams or captures.
- **Every teletext constant is defined once** (REQ-GEN-014). Never restate a value such as 0x27 or 0xE4 outside the constants module.
- **The library stops at the row renderer.** It must have no display, video, timing or hardware-specific code (REQ-ARCH-010, decision E.29). Buffering and pixel delivery belong to the port.

## Tracking work: GitHub issues

Work to be done is tracked in GitHub issues on crowdwave/neoteletext-engine (private). What the system must do, and why, stays in rdm; an issue links to the requirement or decision it serves rather than restating it.

- **Work starts from an issue.** If none exists, open one first. During rapid iteration, one issue per feature is enough; small changes are rdm to-dos.
- **The commit closes it.** End the commit message with `Closes #123`; partial progress gets a comment naming the commit.
- **Log what you find.** Something discovered along the way (a bug, a gap, a decision for the owner) gets an issue, not just a mention in a reply. Decisions for the owner carry the `decision` label.
- **Keep the milestones true.** Close finished work with the commit that did it; close work the design has overtaken as "not planned" with the reason. Delete an issue only with the owner's agreement.

## Working with requirements and decisions (rdm)

- **Identifiers are permanent.** Original identifiers (`REQ-<AREA>-<NNN>`, `E.n`) are never reused and keep resolving in rdm; a new record takes rdm's ID, with its area code at the start of the title (`[INP] …`).
- **A dropped requirement is withdrawn, never deleted:** `set_status` lifecycle `cancelled` with the date's reason, or `superseded` naming its replacement.
- **Record decisions before committing behaviour.** Any behaviour the standard leaves open must be recorded or revised as a decision in rdm (REQ-DOC-004) before the change is committed; during rapid iteration, record it as a to-do and decide it at integration. A decision taken without the owner is created with `needs_confirmation`.
- **Tag phases.** A Phase 2 requirement says so in its body, and decision `F.0.1` lists it. Phase 1 requirements have no tag.
- **Keep state machine tables and prose in step.** The tables in area FSM are the normative form of acquisition, slicer, lifecycle, viewer, row-attribute and Level 1.5 behaviour. If a table and a prose requirement disagree, revise both.
- **Every reference resolves.** A requirement or decision cited in code, tests or documents must exist in rdm (`get_record`); tests name the requirement they verify, which is how rdm links them (REQ-DOC-003).
- **Writing style.**
  - Write every requirement as full narrative sentences, not terse fragments.
  - Spell terms out rather than abbreviating them; the glossary is `docs/spec/glossary.md`.
  - Cite the standard by clause or table, for example "EN 300 706 clause 9.3.1".

## Implementation language and targets

- **Language.** Rust, edition 2024 (REQ-GEN-001). The minimum supported Rust version is at most three stable releases old. Use stable Rust only.
- **Targets** (REQ-GEN-015). They are grouped into four families:
  - ARM: Cortex-M0 to M33, and Arm Linux;
  - RISC-V: RV32IMC, RV32IMAC and RV32IMAFC bare metal, and 64-bit Linux;
  - Xtensa: ESP32 and ESP32-S3;
  - AMD64: Linux, Windows and macOS.
- **Xtensa toolchain.** Xtensa builds need Espressif's toolchain fork, installed with `espup` and pinned. Fork versions 1.94 and 1.95 fail release builds (esp-rs/rust#277); 1.93 works.
- **The workspace** (REQ-GEN-010):
  - `neoteletext-core` (`no_std`, no allocation);
  - `neoteletext-formats`;
  - `neoteletext-slicer`;
  - `neoteletext-web` (sans-IO);
  - `neoteletext-image`;
  - `neoteletext-cli` (the `neottx` binary);
  - `neoteletext-ffi` (Phase 2);
  - `xtask`.
- **Rules for the `no_std` code** (REQ-GEN-006 and REQ-GEN-016):
  - `#![forbid(unsafe_code)]`;
  - no panics, with the clippy panic lints denied in the core;
  - no floating point or atomic operations;
  - no division or 64-bit arithmetic on hot paths;
  - no `core::fmt` on hot paths.
- **Where to start.** Start implementation with milestone M1 (rdm decision `F.0.2`).

## Architecture in brief

The implementation follows the requirements in rdm:

- **Core library** (`neoteletext-core`). It is `no_std`, with no allocation, no input or output, and no clock (REQ-GEN-002). It contains:
  - the error-protection codes;
  - packet decoding;
  - acquisition and the page store;
  - service data;
  - character tables;
  - Level 1.0 and 1.5 decoding into a cell grid;
  - the explicit state machines of area FSM (section F.18).
- **Rendering.**
  - A pure cell-raster function (REQ-RND-016).
  - A pull-style row renderer that draws palette indices into buffers supplied by the caller (REQ-RND-017).
  - Flash and reveal done as palette changes (REQ-RND-023).
- **Optional modules:**
  - input format readers and writers;
  - the software data slicer;
  - the web-native module (area WEB, section F.19). It is written sans-IO. It reads pages from web servers over HTTP or HTTPS, WebSockets and server-sent events, keeps cookie sessions, runs forms, and includes the zxnet live feed (REQ-INP-022). Web pages arrive as TTI or T42 and go through the same decoder. The port supplies the network transport through `embedded-io`-style traits.
  - the pixel-format helpers;
  - the image writers;
  - the command-line program `neottx`.
