# F.1 General, language and quality requirements (GEN)

**REQ-GEN-001 — Language and toolchain.**
All code shall be written in Rust, edition 2024.

- **Minimum supported Rust version.** The minimum supported Rust version shall be stated in the workspace manifest. It shall be kept at no more than three stable releases behind the current stable release, and shall be tested in continuous integration. On 10 October 2026, stable Rust is 1.99.0 (released on 1 October 2026), so the minimum shall be no older than 1.96.
- **Stable Rust only.** The library shall use only stable Rust features. The one exception is the Xtensa targets (REQ-GEN-015), which need Espressif's fork of the toolchain and `build-std` (below).
- **Pinning the Xtensa toolchain.** The Xtensa toolchain version shall be pinned in the repository and changed only deliberately. Espressif's fork versions 1.94 and 1.95 fail to compile release builds for Xtensa, an open issue (esp-rs/rust#277) as of 1 October 2026. Version 1.93 is known to work.

*Reason:* A single language and one pinned toolchain per target family keep builds reproducible across four instruction-set architectures.
*Verification:* Inspection; continuous integration builds with the stated minimum version and with the current stable version.

**REQ-GEN-002 — A `no_std` core with no allocation.**
The core crate (REQ-GEN-010) shall declare `#![no_std]`. It shall not use the `alloc` crate, file or console input and output, any clock, or any operating-system facility. The core comprises:
- the teletext constants;
- the error-protection codes;
- packet decoding;
- character tables;
- the state machines;
- acquisition and the page store;
- service data;
- Level 1.0 and Level 1.5 decoding;
- Level 2.5 and 3.5 decoding: the Colour Map, the enhancement processor and objects (areas L25 and OBJ);
- navigation;
- the cell grid, the cell raster and the row renderer.

The core shall build and run on every target of REQ-GEN-015, including microcontrollers with no operating system.
*Reason:* A teletext decoder fitted into small integrated circuits, so its software equivalent should fit into small microcontrollers.
*Verification:* Inspection; continuous integration builds of the core for every bare-metal target of REQ-GEN-015.

**REQ-GEN-003 — Caller-provided memory.**
The core shall keep all of its state in values whose storage the caller provides. Every configurable capacity shall be a const generic parameter or a caller-supplied slice. Examples are the number of stored pages, the number of sub-pages per page and the size of the cookie store in the web module. The size in bytes of every state type, for each documented configuration, shall be reported by a test and published in the documentation (REQ-GEN-009).
*Verification:* Inspection; a test that runs a decoder whose entire state is in a `static` value created at compile time.

**REQ-GEN-004 — No hidden global state.**
The core shall have no `static mut` items, no mutable statics behind interior mutability, and no lazily initialised globals. Several decoder instances shall be able to run independently in one program. Decoder state types shall be `Send` so that a port may move a decoder between threads or tasks. They need not be `Sync`, because a decoder is used from one thread at a time.
*Verification:* Inspection; a test that runs two decoders on different inputs and compares each result with a single-decoder run.

**REQ-GEN-005 — Determinism and caller-supplied time.**
The core shall produce the same output for the same input and the same configuration. Wherever timing matters, the caller shall supply the current time as a monotonic count of microseconds. This covers the page-completion timeout, flash phase, the reveal timeout and packet time stamps.
*Verification:* Test that runs identical input twice and compares the results.

**REQ-GEN-006 — Lint-clean and free of unsafe code.**
- **Formatting and lints.** The code shall be formatted with `rustfmt`, and shall build without warnings under `clippy`, including the `clippy::pedantic` group, with any exceptions listed and justified in the workspace configuration.
- **No unsafe code.** Every crate except the optional C interface crate (REQ-GEN-018) shall declare `#![forbid(unsafe_code)]`.
- **No panicking constructs in the core.** The core shall also deny these lints, so that no panic can be written by accident (REQ-GEN-008):
  - `clippy::indexing_slicing`;
  - `clippy::unwrap_used`;
  - `clippy::expect_used`;
  - `clippy::panic`;
  - `clippy::arithmetic_side_effects`.
- **Checked under Miri.** The core's tests shall also run under Miri, the Rust interpreter that detects undefined behaviour and out-of-bounds access, on a 64-bit, a 32-bit (`i686-unknown-linux-gnu`) and a big-endian (`s390x-unknown-linux-gnu`) target.

*Verification:* Continuous integration.

**REQ-GEN-007 — Portable bit handling.**
- **Unsigned types for bits.** Bit manipulation shall use unsigned integer types.
- **Explicit byte order.** Multi-byte values from files and streams shall be assembled explicitly in the byte order the format specifies, using `from_le_bytes`, `from_be_bytes` or shifts, never by reinterpreting memory.
- **No endianness assumption.** Every target in REQ-GEN-015 happens to be little-endian, including the ESP32's Xtensa cores, but the code shall not rely on that. The Miri run on a big-endian target (REQ-GEN-006) checks it.

*Verification:* Inspection; the big-endian Miri run.

**REQ-GEN-008 — Robustness against any input, without panics.**
No input shall cause any of the following:
- a panic;
- an out-of-bounds access;
- an arithmetic overflow;
- an unbounded loop;
- unbounded memory use.

This covers every packet, file, stream, sample buffer and network message, however malformed, truncated or hostile. Malformed input shall be reported through the error types and events of the library, and skipped. Builds for bare-metal targets shall use `panic = "abort"`.
*Verification:* Fuzz testing (REQ-TST-040), the lints of REQ-GEN-006 and Miri.

**REQ-GEN-009 — Documented resource use.**
The documentation shall state:
- the memory required by a decoder instance in its minimum configuration, which is one requested page with Level 1.5 enabled;
- the memory required in the default configuration of the command-line program;
- the memory required for presentation at Level 3.5, including the store for enhancement pages (REQ-ACQ-019) and the side-panel area (REQ-L25-006);
- the memory required per stored page;
- the flash (code and constant data) size of the core on `thumbv6m-none-eabi`, built with `opt-level = "z"` and link-time optimisation.

Continuous integration shall measure the core's flash size on that target and fail if it exceeds a budget stated in the repository. The budget shall start at 64 kilobytes. The estimate is 15 to 40 kilobytes, of which the character tables are 10 to 20 kilobytes, and the measured figure replaces the estimate. Measured on 11 October 2026 by `cargo xtask size` (decision E.61): 26,198 bytes for the core with no optional area, 45,154 bytes for a Level 1.5 receiver with the renderer and the viewer, the build the budget applies to, 65,786 bytes at Level 3.5, and 111,294 bytes with every area and the temporary Bedstead glyphs. With the project's own glyphs (decision E.62) the full core is 86,706 bytes. *Remeasured on 12 October 2026,* with Bedstead as the bundled font (decision E.72): 26,198, 43,002, 65,758 and 111,258 bytes.
*Verification:* Test and continuous integration size check.

**REQ-GEN-010 — The Cargo workspace.**
The project shall be a single Cargo workspace with these crates. Each optional crate depends only on crates above it in this table.

| Crate | Contents | Environment |
|---|---|---|
| `neoteletext-core` | Everything listed in REQ-GEN-002 | `no_std`, no allocation |
| `neoteletext-formats` | The streaming readers, writers and page encoder of area INP | `no_std` readers. Writers and file helpers behind an `std` feature. |
| `neoteletext-slicer` | The software data slicer (area SLC) | `no_std`; a `fixed-point` feature (REQ-SLC-015) |
| `neoteletext-web` | The web-native protocols: HTTP, WebSockets, server-sent events, cookies and forms (area WEB), and the live feed reader of REQ-INP-022 | `no_std`, written "sans-IO": it consumes and produces bytes and events, and never performs input or output itself |
| `neoteletext-image` | The pixel-format helpers (REQ-RND-018) and the image writers (REQ-RND-007) | Helpers `no_std`; image writers need `std` |
| `neoteletext-app` | The application layer shared by `neottx` and a windowed viewer: sessions, inputs, the two processes of live mode, sandboxing (decision E.61) | `std` |
| `neoteletext-cli` | The command-line program `neottx` (area CLI) | `std` |
| `neoteletext-ffi` | The optional C interface (REQ-GEN-018) | Phase 2 |

A development-tools package (`xtask`) holds project automation that is not part of the library, such as the glyph export, fetching the corpus and checking sizes.

Features shall be additive. Each crate's optional areas are features, as decision E.61 lists. `cargo xtask features` builds, lints and tests each library and application crate with no optional area and with each area alone, and the workspace tests run with every feature. The mapping of the core modules of REQ-ARCH-001 to Rust modules shall be one module each within `neoteletext-core`.
*Verification:* Continuous integration.

**REQ-GEN-011 — Public interface conventions.**
- **Conventions.** The public interface shall follow the Rust API Guidelines.
- **Errors.** Errors are enumerations implementing `core::error::Error`, never strings.
- **Configuration.** Configuration is a plain struct with a `Default` implementation and named profile constructors (area PRF).
- **Documentation.** Every public item is documented.
- **Stability.** Crates follow semantic versioning, checked with `cargo-semver-checks` before each release.
- **Identifier prefixes.** The prefix `ttx_` is kept only for identifiers visible outside Rust: the C interface and the form field names `ttx_token` and `ttx_action` of Appendix C.

*Verification:* Inspection; `cargo-semver-checks` in the release process.

**REQ-GEN-012 — Provenance and licensing.**
The repository shall contain a licence file for the project. It shall also contain a provenance record listing:
- the source of every table transcribed from a standard, identified by clause and table number;
- the source and licence of every glyph set.

No code from GPL-licensed teletext software shall be included.
*Verification:* Inspection.

**REQ-GEN-013 — Throughput.**
- The packet-level decoder, which covers everything from 42-byte packets to stored pages including Level 1.5, shall process at least 17,500 packets per second on a single core of a current desktop computer. That is twenty times the maximum broadcast rate of 35 packets per frame at 25 frames per second.
- The raw-sample slicer shall process at least ten times the real-time rate of one stream of 35 lines per frame at 25 frames per second on a single core of the same computer.
*Verification:* Performance test (REQ-TST-050).

**REQ-GEN-014 — Every teletext constant is defined once.**
Every teletext value shall be defined in exactly one place, a constants module of the core, and read by name everywhere else. This covers every timing, level, code value, bit pattern, packet length, line allocation and tolerance. Where a value is needed in more than one form, both forms shall be defined side by side in that module with the conversion stated. The main example is the framing code, which is 0x27 in transmission order (T42) and 0xE4 in the bit order of digital television. No other module may restate, round, bit-reverse or re-derive a teletext value privately.
*Reason:* This is requirement R1 of the tellyboy project in the same workspace. Its recorded history shows that every teletext defect not caused by signal levels was a number existing in two places. In one case the framing code lived as 0xE4 in one file and 0x27 in another, both correct in their own bit order, with nothing to compare them.
*Verification:* Inspection; a test that searches the source outside the constants module for literal teletext values such as 0x27, 0xE4, 6937500 and 0x55.

**REQ-GEN-015 — Target platforms.**
The library shall support these targets, grouped by the four instruction-set families. "Build" means that continuous integration compiles the listed crates for the target; "test" means that it also runs the test suite there.

| Family | Target triple | Examples | Rust support tier | Continuous integration |
|---|---|---|---|---|
| ARM | `thumbv6m-none-eabi` | Cortex-M0 and M0+ | 2 | Build core, formats, slicer and web. Size check (REQ-GEN-009). |
| ARM | `thumbv7em-none-eabihf` | Cortex-M4F and M7F | 2 | Build; run the core tests under QEMU with semihosting |
| ARM | `thumbv8m.main-none-eabihf` | Cortex-M33 | 2 | Build |
| ARM | `aarch64-unknown-linux-gnu` | 64-bit Arm Linux boards | 1 | Build and test (native runner or emulated with `cross`) |
| ARM | `armv7-unknown-linux-gnueabihf` | 32-bit Arm Linux boards | 2 | Build and test with `cross` |
| ARM | `aarch64-apple-darwin` | Development on Apple computers | 1 | Build and test |
| RISC-V | `riscv32imc-unknown-none-elf` | ESP32-C2 and ESP32-C3 | 2 | Build |
| RISC-V | `riscv32imac-unknown-none-elf` | ESP32-C6, ESP32-H2 and other RV32IMAC parts | 2 | Build; run the core tests under QEMU |
| RISC-V | `riscv32imafc-unknown-none-elf` | ESP32-P4 | 2 | Build |
| RISC-V | `riscv64gc-unknown-linux-gnu` | 64-bit RISC-V Linux boards | 2 | Build and test with `cross` |
| Xtensa | `xtensa-esp32-none-elf` | ESP32 | 3 | Build with the pinned Espressif toolchain and `build-std = ["core"]` (REQ-GEN-001) |
| Xtensa | `xtensa-esp32s3-none-elf` | ESP32-S3 | 3 | Build, as above |
| Xtensa | `xtensa-esp32-espidf` and `xtensa-esp32s3-espidf` | ESP32 and ESP32-S3 with the ESP-IDF operating system, where `std` is available | 3 | Build, as above |
| AMD64 | `x86_64-unknown-linux-gnu` | Linux personal computers and servers | 1 | Build and test, including fuzzing and Miri |
| AMD64 | `x86_64-pc-windows-msvc` | Windows | 1 | Build and test |
| AMD64 | `x86_64-apple-darwin` | Intel Apple computers | 2 | Build and test |
| WebAssembly | `wasm32-unknown-unknown` and `wasm32-wasip1` | Isolated execution inside a WebAssembly runtime, in browsers or other hosts (REQ-SEC-019) | 2 | Build `neoteletext-core` and `neoteletext-formats` |

The rules for these targets are:
- **Which crates build where.** On bare-metal targets, the crates without `std` shall build: `neoteletext-core`, `neoteletext-formats`, `neoteletext-slicer`, `neoteletext-web` and the helpers of `neoteletext-image`. Targets with an operating system shall also build the `std` crates and the command-line program.
- **Where tests run.** Tests of logic run on the host. Cross-target tests prove that the code builds and behaves the same on each instruction-set family.
- **Xtensa testing.** Running tests on Xtensa, in Espressif's QEMU fork or the Wokwi simulator, may be added later. Xtensa is build-only in Phase 1.
- **Scope.** None of these targets implies any hardware support in the library (REQ-ARCH-010). The examples only indicate the class of processor.

*Verification:* Continuous integration matrix.

**REQ-GEN-016 — Rules for code that must run on small processors.**
Code in the `no_std` crates shall follow these rules, because some targets lack hardware for them.

| Rule | Why | Where it applies |
|---|---|---|
| **No floating point** | The Cortex-M0, M3 and RV32IMC have no floating-point unit, and the ESP32 emulates double precision in software. | The core, the web module and the formats. The slicer may use floating point, with a fixed-point form as REQ-SLC-015 requires. |
| **No atomic operations** | The Cortex-M0 and RV32IMC have no compare-and-swap. | Everywhere; the library needs no concurrency of its own. |
| **No division or 64-bit arithmetic in per-packet and per-pixel paths** | The Cortex-M0 has no hardware divide, and 64-bit operations are slow on 32-bit processors. | Per-packet and per-pixel code. Time values may be 64-bit, but arithmetic on them stays out of inner loops. |
| **No unaligned access** | The Cortex-M0 faults on it. | Everywhere; safe Rust already guarantees this. |
| **No formatting machinery on hot paths** | It adds several kilobytes to a small device. | The core shall not use `core::fmt` except in `Debug` implementations and behind an optional feature, so a small build can leave it out. |
| **No deep recursion or large stack buffers** | Small devices have small stacks. | Everywhere. The largest stack frame of the core shall be documented. |

*Verification:* Inspection; the size check of REQ-GEN-009; the builds of REQ-GEN-015.

**REQ-GEN-017 — Dependency policy.**
Dependencies are judged by the microcontroller, as decision E.38 describes.

- **Microcontroller crates.** These are the crates that run on bare-metal targets (REQ-GEN-015): `neoteletext-core`, `neoteletext-formats` without its `std` feature, `neoteletext-slicer`, `neoteletext-web` and the helpers of `neoteletext-image`.
- **Crates with no dependencies.** `neoteletext-core`, `neoteletext-formats` and `neoteletext-slicer` shall have no dependencies outside the Rust distribution's `core` library. Fixed-capacity collections and bit flags shall be written in the Foundation layer.
- **The web module.** `neoteletext-web` may depend only on `embedded-io`, with default features off. WebSocket framing, SHA-1, base64, the descriptor reader and the HTTP response-head parser shall be written in the project. Each shall be tested against the published test vectors of its standard, where the standard publishes them.
  *Amended 11 October 2026:* `httparse` was previously permitted as well. It was removed because of its linked panic paths (decision E.38, as amended).
- **Rules for any new dependency.** A new dependency of a microcontroller crate, direct or indirect, needs a decision recorded in Part E. It shall:
  - build as `no_std` without allocation for `thumbv6m-none-eabi`, `riscv32imc-unknown-none-elf` and `xtensa-esp32-none-elf`;
  - use no compare-and-swap atomic operations and no floating point;
  - have no build script and no native code;
  - have its flash cost measured by the size check of REQ-GEN-009;
  - pass the symbol check of REQ-GEN-019.
- **Other crates.** The command-line program, the image file writers, `xtask` and the C interface never run on a microcontroller. They may use well-maintained crates where these save real work, for example `png` for image writing, `clap` for the command line, and `ratatui` and `crossterm` for the interactive viewer.
- **Licences.** Every dependency shall have a licence compatible with the project's own: MIT, Apache 2.0, BSD, zlib, ISC or CC0. GPL-licensed crates are excluded (REQ-GEN-012). The rule shall be enforced with `cargo-deny`, which shall also check for security advisories. LICENSING.md sets out every licence the project accepts, including system libraries outside cargo-deny's view.
  *Amended 20 October 2026:* LGPL system libraries are accepted when linked dynamically and not shipped (LICENSING.md, section 2).
- **Enforcing the list.** The permitted dependency list of each microcontroller crate shall be recorded in the repository. An `xtask` check shall compare it with the resolved dependency graph, and shall fail on any difference.

*Verification:* `cargo-deny` and the `xtask` dependency check in continuous integration; the symbol check of REQ-GEN-019.

**REQ-GEN-019 — Symbol check of the linked microcontroller image.**
Continuous integration shall link a test image for `thumbv6m-none-eabi`. The image shall contain the microcontroller crates of REQ-GEN-017 and call every public entry point, so that no code is removed as unused. The image shall be built with `opt-level = "z"`, link-time optimisation and `panic = "abort"`.

The check shall fail if the linked image contains:
- any software floating-point routine, such as `__aeabi_fadd` or `__aeabi_dmul`, outside the slicer's floating-point feature (REQ-SLC-015);
- any compare-and-swap atomic routine;
- any reachable panic other than the image's own panic handler.

The size check of REQ-GEN-009 shall measure the same image.
*Reason:* The lints of REQ-GEN-006 and REQ-GEN-016 cover only the project's own source. Only the linked image shows what dependencies and the compiler actually brought in (decision E.38).
*Verification:* Continuous integration, including a test that a deliberately added floating-point operation makes the check fail.

**REQ-GEN-020 — Capability features and the modern-classic profile.**
`neoteletext-core` shall offer the four capability features of decision E.71, `objects`, `side-panels`, `colours32` and `animation`, each needing `level25`. A build without one of them shall contain neither the decoding nor the drawing of that capability, and shall treat its page data as decision E.71 item 1 says, never as an error. The feature `modern-classic` shall turn on `level15`, `level25`, `render`, `original`, `viewer`, `forms` and `objects`, and nothing else.
*Amended 20 October 2026 (decision E.95):* DRCS is not decoded, and the parts of this requirement that concerned it are removed.
*Amended 20 October 2026 (decision E.96):* the Level 3.5 font styles are not drawn, and the parts of this requirement that concerned them are removed.
*Verification:* `cargo xtask features` builds, lints and tests the core with `level25` alone and with `modern-classic`; a test checks each capability compiled out; `cargo xtask size` reports the image of the profile beside the full one.

**REQ-GEN-018 — Optional C interface.**
*Phase:* Phase 2 (whole requirement).
A separate crate, `neoteletext-ffi`, shall offer the core's interface to ports written in C. Its header shall be generated with `cbindgen`, and it shall be built as a static library. The crate shall be the only one allowed to contain `unsafe` code, and it shall document:
- that a panic aborts, because panics cannot unwind across the C boundary;
- the requirement for a panic handler on bare-metal targets;
- matching the floating-point calling convention of the C port.

Its public identifiers shall use the prefix `ttx_`.
*Verification:* Test that builds and runs a small C program against the static library on the host.
