# neoteletext — specification

**Status:** Draft 2 (consolidated) for review
**Date:** 1 October 2026
**Implementation language:** Rust, edition 2024 (REQ-GEN-001), for ARM, RISC-V, Xtensa (ESP32) and AMD64 targets (REQ-GEN-015)
**Presentation levels in scope:** Levels 1.0, 1.5, 2.5 and 3.5 of the World System Teletext (System B) as defined by ETSI EN 300 706 (Levels 2.5 and 3.5 added on 10 October 2026, decision E.40)
**Baseline reference hardware:** Philips SAA5050 (rendering) and Philips SAA5246A (receiver behaviour)

---

## How the specification is organised

This folder holds how neoteletext works: the design, the terms and the sources. Its requirements and decisions are records in rdm, reached through rdm's MCP tools, since 30 October 2026 (rdm decision DEC-1389); before that they were files here (and, until 18 October 2026, one file, `REQUIREMENTS.md`). Their old identifiers still work in rdm: `REQ-L1-012`, decision `E.30`, `F.0.1` (the delivery phases), `G.3` (an out-of-scope item), `Open question 4`.

### What goes where

Each fact is written once, in the one document whose job it is, and every other document links to it rather than repeating it.

| Kind of fact | Its home | Was |
|---|---|---|
| **What the system shall do:** numbered, testable requirements (`REQ-<AREA>-<NNN>`), security (area SEC) overriding every other (decision E.32) | rdm | Part F |
| The delivery phases and milestones | rdm decisions `F.0.1` and `F.0.2` | Part F, sections F.0.1 and F.0.2 |
| What is deliberately out of scope | rdm decisions `G.1` to `G.16` | Part G |
| **Why:** each decision where the standard is silent, contradicts itself, or differs from the reference chips, with its reason (`E.n`). A decision says what was chosen and points to the design for the detail. | rdm | Part E |
| Questions for the owner, and work owed (outside help, known gaps, technical debt) | rdm decisions waiting for confirmation, and to-do records | `open-questions.md`, `TECH-DEBT.md` |
| The TTI extensions: the Link Extension, and the Form Extension (withdrawn on 27 October 2026 by decision E.126, kept as history) | `design/tti-link-extension.md`, `design/tti-form-extension.md` | Appendices E and C |
| **How a feature works,** explained from the beginning: walkthroughs of the standard and the reference chips, and the design of larger features | `design/*.md` | Parts A to D, and documents formerly in `docs/` |
| Terms and abbreviations | `glossary.md` | Appendix A |
| The standards and datasheets cited | `source-documents.md` | Appendix B |
| The standards clauses and tables each code module is written from (REQ-DOC-005) | `module-sources.md` | `docs/sources.md` |
| **Where the code for each part lives** | `ARCHITECTURE.md` (repository root) | |
| **The evidence:** research notes, with a citation for every fact | `research/` (repository root) | |

The design documents are:

| File | Contents |
|---|---|
| `design/level1.md` | Walkthrough of Presentation Level 1.0 (EN 300 706) |
| `design/level15.md` | Walkthrough of Presentation Level 1.5 |
| `design/saa5050.md` | The Philips SAA5050 character generator, the rendering baseline |
| `design/saa5246a.md` | The Philips SAA5246A decoder, the receiver baseline |
| `design/character-sets-and-fonts.md` | How teletext chooses characters and how neoteletext turns them into glyphs |
| `design/languages.md` | Languages beyond the standard's character sets: the sets, designations, Thai and Lao, joining, fonts and the page writer |
| `design/capabilities.md` | The decoder's optional capabilities, switched at compile time, and the `modern-classic` profile |
| `design/font-security.md` | The security analysis of the font strategy (decision E.72) |
| `design/porting.md` | What a port supplies, and nothing else |
| `design/audio.md` | The teletext audio profile: its sources, its records, carriage and control, and where it departs from the standards (area AUD) |
| `design/audio-platforms.md` | How the audio profile is built on each platform: the engine crates, NeoTeletext on desktops, and microcontrollers through tellyboy |
| `design/hmi.md` | A proposal, not yet a requirement: how the code takes input from different controls |

**Only the requirements are normative.** A requirement wins over anything in the decisions or the design documents. The decisions are binding choices, recorded in rdm before the behaviour they govern is committed (REQ-DOC-004). rdm links each requirement to the tests that name it (REQ-DOC-003).

### Requirement wording

Each requirement has an identifier of the form `REQ-<AREA>-<NUMBER>`, for example `REQ-L1-012`. The area code is the identifier's middle part. Requirement identifiers are never reused. A requirement that is withdrawn keeps its number and is marked as withdrawn. Numbers within an area are not always consecutive; gaps are deliberate and leave room for related requirements to be added later.

- **shall** marks a mandatory requirement.
- **should** marks a recommendation. It is expected to be implemented unless there is a documented reason not to.
- **may** marks a permitted option.

Each requirement carries a **verification method**, which is how compliance will be demonstrated. The methods are:

- **Test**: an automated test in the project's test suite.
- **Inspection**: a review of the source code or the documentation.
- **Demonstration**: running the program and observing the result.

### Citation conventions

**Where the sources are.** The standards and datasheets cited in this document are not kept in the project directory (`~/devel/neoteletext-engine`), because they must not be redistributed (see "Legal basis" below). They are in `~/devel/teletext-reference/`. Every folder path in this document, such as `teletext-standards/`, `teletext-fonts/manufacturers/philips/datasheets/saa5050/` or `catalog/indexes/DATASHEET-INDEX.md`, is relative to that directory.

The research notes behind Parts A to E are in `research/` in the project directory, with a citation for each fact.

Citations take the form "EN 300 706 clause 9.3.1" or "EN 300 706 Table 26". Printed page numbers of EN 300 706 V1.2.1 match the page numbers of the PDF file in `teletext-standards/`.

- Citations of the SAA5050 refer either to the Signetics datasheet in `teletext-fonts/manufacturers/philips/datasheets/saa5050/` or to the Philips 1979 databook in `teletext-fonts/manufacturers/philips/datasheets/databooks/`. For the databook, the number given is the PDF page number.
- Citations of the SAA5246A refer to the Philips product specification of January 1993 in `teletext-fonts/manufacturers/philips/datasheets/saa5246a/`.

Character codes are written as column and row, as the standard does. "1/E" means column 1, row 14, which is hexadecimal 0x1E. Hexadecimal is also used where it is clearer.

### Terminology

The standard names things with abbreviations, such as the Magazine and Row Address Group and the Fastext linking scheme called "Full Level One Features". This document spells each term out in full at its first use in each part. The Glossary in Appendix A lists every abbreviation that still appears.

### Legal basis of the implementation

The implementation will be written from the published standards (EN 300 706, EN 300 472 and ITU-R BT.653) and from behaviour described in published datasheets. The following restrictions apply:

- No manufacturer's code will be copied or disassembled. This includes the mask ROM contents of any microcontroller.
- Character glyph bitmaps will not be copied from any character generator ROM. The glyphs will be drawn originally or taken from a public-domain recreation (see REQ-RND-020).
- No code will be copied from GPL-licensed teletext decoders. Such decoders may be used as external test oracles (see Part F, area TST).
- The standards and datasheets themselves will not be redistributed in the project repository.
