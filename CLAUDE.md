# neoteletext — software teletext receiver

A software receiver for World System Teletext (ETSI EN 300 706), Presentation Levels 1.0 and 1.5. The behaviour baselines are the Philips SAA5050 (rendering) and SAA5246A (receiver).

The library and `neottx` are implemented; work continues feature by feature. This repository is the core teletext rendering engine only (decision E.76). The windowed browser built on it is maintained in its own repository, `~/devel/neoteletext-browser`; it was removed from this one on 18 October 2026. Do not add browser code to this repository.

## How we work

@~/devel/neoteletext-common/DEVELOPMENT.md

That is the one document of how we work, for every NeoTeletext repository, with rdm as the record of all work (DEC-2162). This file says only what is particular to the engine; where the two differ, that document holds.

- **The engine's old identifiers still resolve in rdm:** `REQ-INP-023`, `E.30`, `F.0.1`, `G.3`, `Open question 4`, `TECH-DEBT 12`. A new engine record's title starts with its area code (`[INP] …`).
- **No file records a requirement, decision or to-do:** there is no `TECH-DEBT.md`, `docs/spec/decisions.md`, `docs/spec/requirements/` or `open-questions.md` any more, and a hook refuses edits to those paths. If a merge brings one back, record what it adds in rdm and delete the file.

## Key documents

- **Requirements and decisions are in rdm:** the requirements (Part F, area SEC overriding every other), the decisions (Part E), the phase plan and milestones (decisions `F.0.1`, `F.0.2`), what is out of scope (decisions `G.n`), the owner's open questions (decisions waiting for confirmation) and the work owed (to-dos).
- **`docs/spec/`** holds how things work: `design/*.md` (Levels 1.0 and 1.5, the SAA5050 and SAA5246A, the TTI Link and Form Extensions; character sets and languages, including those beyond the standard's sets, are in rdm only: decisions DEC-599 and DEC-601 to DEC-606 with their evidence, the requirements they decide, and the protocol document's items under DOC-1845), `glossary.md`, `source-documents.md` and `module-sources.md`; `docs/spec/README.md` is the map.

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
  - Library crates have no operating-system access; enforced by `cargo xtask check-io`.
  - `neottx` sandboxes itself: Landlock and seccomp on Linux.
  - Web mode runs as a fetcher process and a decoder process.
  - Plain HTTP is allowed by the owner's decision (E.32), with the safeguards of REQ-SEC-008.

- **No copyrighted source material in this repository.** Do not copy standards PDFs or datasheets here. Link to them or cite them by clause.
- **No GPL code.** vbit2 and QTeletextMaker may be run as external programs, or used as test oracles at test time. Their code and test files must never be copied into this repository.
- **One font: Noto Teletext** (DEC-1761, REQ-FNT-001).
  - `third_party/noto/*.bdf` hold it; `cargo xtask glyphs` compiles them into `crates/neoteletext-core/src/glyphs/noto.rs`. Its licence (SIL OFL 1.1) and provenance are in `third_party/noto/`, with the converter that makes it.
  - It is the only glyph data in the repository. Mosaics and G3 characters are drawn by rule.
  - Never add any other font, or any glyphs from character-generator ROMs, to the repository. Other fonts are 12 × 10 or 12 × 20 BDF files loaded at run time (area FNT); the owner's font collection lives in a separate repository.
  - Keep glyphs as replaceable data, never hard-coded.
- **The test corpus is downloaded at test time and pinned by checksum** (REQ-TST-022). Local corpus files, including Dan Farrimond's recovered T42 captures in `~/devel/jellytext/dan/`, are read from `$NEOTELETEXT_CORPUS`. Never commit sample streams or captures.
- **Every teletext constant is defined once** (REQ-GEN-014). Never restate a value such as 0x27 or 0xE4 outside the constants module.
- **The library stops at the row renderer.** It must have no display, video, timing or hardware-specific code (REQ-ARCH-010, decision E.29). Buffering and pixel delivery belong to the port.

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
  - the web-native module (area WEB, section F.19). It is written sans-IO. It reads pages from web servers over HTTP or HTTPS and WebSockets, keeps cookie sessions, runs forms, and includes the zxnet live feed (REQ-INP-022). Web pages arrive as TTI or T42 and go through the same decoder. The port supplies the network transport through `embedded-io`-style traits.
  - the pixel-format helpers;
  - the image writers;
  - the command-line program `neottx`.
