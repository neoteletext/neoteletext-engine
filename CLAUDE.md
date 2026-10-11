# neoteletext — software teletext receiver

A software receiver for World System Teletext (ETSI EN 300 706), Presentation Levels 1.0 and 1.5. The behaviour baselines are the Philips SAA5050 (rendering) and SAA5246A (receiver).

No code exists yet. The project is at the requirements stage.

## Key documents

- `REQUIREMENTS.md` is the specification and the single source of truth. Its parts:

  | Part | Contents |
  |---|---|
  | A and B | Walkthroughs of Levels 1.0 and 1.5 |
  | C and D | The SAA5050 and SAA5246A |
  | E | The decision register |
  | F | The numbered requirements, including the phase plan (F.0.1) and the milestones M1 to M5 (F.0.2) |
  | G | What is out of scope |
  | Appendix C | The TTI Form Extension, version 3 (normative) |
  | Appendix D | Open questions: which are answered, with evidence, and which remain open |

- `ARCHITECTURE.md` describes the code's layers (Foundation, then L0 Ingress to L9 Applications), their boundary types, trust boundaries and the allowed-dependency table. REQ-ARCH-011 makes it mandatory, and it is enforced by an `xtask` check. Keep it in step with `REQUIREMENTS.md`.
- `research/notes-*.md` are the cited research notes behind Parts A to E. `research/answers-*.md` are the reports that answered the open questions: SAA5050 behaviour from emulators, the historic specifications, and Rust targets. Check a claim here before changing it.
- `research/jellytext-tti-form-system-spec-v2.md` is the original forms specification from the earlier jellytext project, kept for history. The normative, corrected version 3 is REQUIREMENTS.md Appendix C; decision E.31 explains the changes.
- The source standards and datasheets are in `~/devel/microcontrollers/teletext/`. All folder paths cited in `REQUIREMENTS.md` (`specifications/`, `philips-saa5050/` and so on) are relative to that directory.

## Rules that must not be broken

- **Security overrides every other requirement** (area SEC, section F.0.3; decision E.32).
  - All input is untrusted.
  - No executable content, ever.
  - Row 0 columns 0 to 7 belong to the receiver alone.
  - Library crates have no operating-system access; enforced by clippy's disallowed lists.
  - `ttx` sandboxes itself: Landlock and seccomp on Linux.
  - Web mode runs as a fetcher process and a decoder process.
  - Plain HTTP is allowed by the owner's decision (E.32), with the safeguards of REQ-SEC-008.

- **No copyrighted source material in this repository.** Do not copy standards PDFs or datasheets here. Link to them or cite them by clause.
- **No GPL code.** zvbi, vhs-teletext, vbit2 and QTeletextMaker may be run as external programs, or used as test oracles at test time. Their code and test files must never be copied into this repository.
- **No glyphs from character-generator ROMs.**
  - During development, `third_party/bedstead/bedstead-glyphs.txt` is a **temporary** glyph source. Read `third_party/bedstead/PROVENANCE.md` for why it is only temporary.
  - Keep glyphs as replaceable data, never hard-coded.
  - Original glyphs must replace Bedstead before any release (REQ-RND-020, decision E.25).
- **The test corpus is downloaded at test time and pinned by checksum** (REQ-TST-022). Local corpus files, including Dan Farrimond's recovered T42 captures in `~/devel/jellytext/dan/`, are read from `$NEOTELETEXT_CORPUS`. Never commit sample streams or captures.
- **Every teletext constant is defined once** (REQ-GEN-014). Never restate a value such as 0x27 or 0xE4 outside the constants module.
- **The library stops at the row renderer.** It must have no display, video, timing or hardware-specific code (REQ-ARCH-010, decision E.29). Buffering and pixel delivery belong to the port.

## Working with REQUIREMENTS.md

- **Identifiers are permanent.**
  - Requirement numbers (`REQ-<AREA>-<NNN>`) are never reused.
  - Gaps in the numbering are deliberate.
  - A dropped requirement is marked "Withdrawn", with its date and reason, and keeps its number.
- **Record decisions before changing behaviour.** Any behaviour the standard leaves open must first be recorded or amended in Part E (REQ-DOC-004).
- **Tag phases.** A Phase 2 requirement carries a `*Phase:*` line directly under its title, and is listed in the table in section F.0.1. Phase 1 requirements have no tag.
- **Keep state machine tables and prose in step.** The tables in area FSM (section F.18) are the normative form of acquisition, slicer, lifecycle, viewer, row-attribute and Level 1.5 behaviour. If a table and a prose requirement disagree, fix both.
- **Check references after editing.** Every `REQ-` reference must resolve to a definition:

  ```sh
  grep -o 'REQ-[A-Z0-9]*-[0-9]*' REQUIREMENTS.md | sort -u > /tmp/refs
  grep -o '^\*\*REQ-[A-Z0-9]*-[0-9]*' REQUIREMENTS.md | sed 's/\*\*//' | sort -u > /tmp/defs
  comm -23 /tmp/refs /tmp/defs   # must print nothing
  ```

- **Writing style.**
  - Write every requirement as full narrative sentences, not terse fragments.
  - Spell terms out rather than abbreviating them; the glossary is Appendix A.
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
  - `neoteletext-cli` (the `ttx` binary);
  - `neoteletext-ffi` (Phase 2);
  - `xtask`.
- **Rules for the `no_std` code** (REQ-GEN-006 and REQ-GEN-016):
  - `#![forbid(unsafe_code)]`;
  - no panics, with the clippy panic lints denied in the core;
  - no floating point or atomic operations;
  - no division or 64-bit arithmetic on hot paths;
  - no `core::fmt` on hot paths.
- **Where to start.** Start implementation with milestone M1 (section F.0.2).

## Architecture in brief

The implementation follows REQUIREMENTS.md Part F:

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
  - the command-line program `ttx`.
