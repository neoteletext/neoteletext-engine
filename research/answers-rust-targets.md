# Rust multi-target facts for the teletext decoder requirements (researched 2026-10-01)

Legend: **[V]** = verified against the cited source today. **[U]** = uncertain or my inference/estimate, so check it before relying on it.

Context: current stable Rust is **1.98.1** (2026-09-03). **1.99.0** is dated 2026-10-01, which is today. [V] https://raw.githubusercontent.com/rust-lang/rust/master/RELEASES.md , https://static.rust-lang.org/dist/channel-rust-stable.toml

---

## 1. Target triples and support tiers

Source for the whole table: https://doc.rust-lang.org/rustc/platform-support.html [V]

| Hardware | Triple | Tier | std? |
|---|---|---|---|
| Cortex-M0/M0+/M1 (Armv6-M) | `thumbv6m-none-eabi` | 2 (no host tools) | no_std (core+alloc) |
| Cortex-M3 (Armv7-M) | `thumbv7m-none-eabi` | 2 | no_std |
| Cortex-M4F/M7F (Armv7E-M, hard-float) | `thumbv7em-none-eabihf` | 2 | no_std |
| Cortex-M33 (Armv8-M Mainline, hard-float) | `thumbv8m.main-none-eabihf` | 2 | no_std |
| ARM64 Linux | `aarch64-unknown-linux-gnu` | **1** with host tools (kernel 4.1+, glibc 2.17+) | full std |
| ARM64 bare metal | `aarch64-unknown-none` | 2 | no_std |
| ARMv7-A Linux (e.g. Raspberry Pi 2/3 32-bit) | `armv7-unknown-linux-gnueabihf` | 2 with host tools | full std |
| ESP32-C2/C3 (RV32IMC) | `riscv32imc-unknown-none-elf` | 2 | no_std |
| ESP32-C5/C6/C61/H2 (RV32IMAC) | `riscv32imac-unknown-none-elf` | 2 | no_std |
| ESP32-P4 / H4 / S31 (RV32IMAFC) | `riscv32imafc-unknown-none-elf` | 2 | no_std |
| RISC-V 64 Linux | `riscv64gc-unknown-linux-gnu` | 2 with host tools (kernel 4.20+, glibc 2.29) | full std |
| ESP32 (Xtensa LX6) | `xtensa-esp32-none-elf` | **3** | no_std |
| ESP32-S2 | `xtensa-esp32s2-none-elf` | 3 | no_std |
| ESP32-S3 (Xtensa LX7) | `xtensa-esp32s3-none-elf` | 3 | no_std |
| ESP32 with ESP-IDF | `xtensa-esp32-espidf` (also `-esp32s2-`, `-esp32s3-espidf`) | 3 | std (through ESP-IDF) |
| ESP32-C3 with ESP-IDF | `riscv32imc-esp-espidf` | 3 | std |
| x86-64 Linux | `x86_64-unknown-linux-gnu` | **1** | full std |
| x86-64 Windows | `x86_64-pc-windows-msvc` | **1** (Windows 10+) | full std |
| Intel macOS | `x86_64-apple-darwin` | **2 with host tools**. It was demoted from Tier 1 in Rust 1.90 (2025-09-18). | full std |
| Apple Silicon macOS | `aarch64-apple-darwin` | **1** (macOS 11+) | full std |

Notes:
- The chip-to-triple mapping comes from the esp-hal README table: ESP32-C2/C3 use imc; C5/C6/C61/H2 use imac; H4/P4/S31 use imafc. [V] https://github.com/esp-rs/esp-hal/blob/main/esp-hal/README.md
- Demotion of x86_64-apple-darwin: [V] RELEASES.md 1.90.0, https://github.com/rust-lang/rust/pull/145252
- Tier 2 means rustup ships a prebuilt `core`/`alloc` (`rustup target add ...`) and the target is guaranteed to build but not guaranteed to pass tests. Tier 3 means nothing prebuilt, so you need `-Z build-std` (nightly or a fork). [V] https://doc.rust-lang.org/rustc/platform-support.html
- Rust 1.97 also promoted several Armv7-R/A and Armv8-R bare-metal thumb targets to Tier 2. They are not needed here. [V] RELEASES.md

## 2. Xtensa (ESP32 / S2 / S3)

- **Upstream rustc has the target specs but you still use the fork in practice.** Rust 1.81 (2024-09) added the Tier 3 targets `xtensa-esp32{,s2,s3}-none-elf` and `xtensa-esp32{,s2,s3}-espidf`. [V] RELEASES.md 1.81.0; https://github.com/rust-lang/rust/pull/125141 , https://github.com/rust-lang/rust/pull/126380
- The rustc book page for Xtensa still says: "To build Xtensa targets, you need the Xtensa-enabled Rust channel", meaning the esp-rs/rust fork installed through espup. Maintainers are @MabezDev and @SergioGasquez. [V] https://doc.rust-lang.org/rustc/platform-support/xtensa.html
- **LLVM status.** An Xtensa backend was added to upstream LLVM in 2023, and Clang 20 added the Xtensa target. [V] https://www.phoronix.com/news/Clang-20-Adds-Xtensa-Target
  - A rustc PR merged 2026-09-17 enables assembly tests for the xtensa targets. It states "meanwhile upstream LLVM support has landed". [V] https://github.com/rust-lang/rust/pull/162841
  - **[U]** I could not confirm that official rustup builds now enable the Xtensa LLVM backend. Even if they do, the targets stay Tier 3 (nightly plus build-std). For requirements purposes, **assume the espup "esp" fork toolchain is required for Xtensa.**
- **Stable Rust?** RISC-V ESP chips use the ordinary stable toolchain: `rustup toolchain install stable --component rust-src` and `rustup target add riscv32imc-unknown-none-elf`. Xtensa chips "will need to use a fork of the Rust compiler for now". [V] https://docs.espressif.com/projects/rust/book/getting-started/toolchain.html
  - The fork follows each stable release closely. Recent esp-rs/rust-build releases are v1.99.0.0 (2026-09-30), v1.98.1.0, v1.98.0.0 and v1.97.0.0. [V] https://api.github.com/repos/esp-rs/rust-build/releases
  - espup 0.18.0 was released 2026-09-30. [V] crates.io
  - espup installs the Espressif Rust fork, a stable toolchain for RISC-V, the Xtensa-enabled LLVM fork and a GCC linker toolchain. [V] toolchain page above.
- **Known regression (important for pinning).** esp toolchains 1.94.x and 1.95.x (LLVM 21) fail release builds with `rustc-LLVM ERROR: Cannot select ... XtensaISD::PCREL_WRAPPER`. 1.93.0.0, the last release on LLVM 20, works. [V] https://github.com/bitaxeorg/bitaxe-raw/pull/17
  - The upstream issue esp-rs/rust#277 was still **open** on 2026-10-01. [V] https://github.com/esp-rs/rust/issues/277
  - [U] It may also affect 1.96 to 1.99. Pin the esp toolchain version in `rust-toolchain.toml` and CI.
- **CI.** Use the GitHub Action `esp-rs/xtensa-toolchain`. The latest release is v1.7.0 (2026-04-20). [V] https://github.com/esp-rs/xtensa-toolchain , https://api.github.com/repos/esp-rs/xtensa-toolchain/releases
  - Inputs: `default`, `buildtargets` (default `esp32,esp32s2,esp32s3`), `version` (default `latest`, **pin this**), `ldproxy` (only for std/ESP-IDF), `override`, `export`, and `github-token` for non-GitHub runners.
- **build-std.** The official esp-generate template configures the project as follows. [V] https://raw.githubusercontent.com/esp-rs/esp-generate/main/template/.cargo/config.toml , https://raw.githubusercontent.com/esp-rs/esp-generate/main/template/rust-toolchain.toml
  - It always sets `[unstable] build-std = ["core"]`, or `["alloc","core"]` when alloc is enabled.
  - Xtensa builds use `rust-toolchain.toml` with `channel = "esp"`. Xtensa bare-metal builds add `-C link-arg=-nostartfiles`.
  - RISC-V builds use the stable channel with `rust-src`.
  - [U] My understanding is that stable cargo ignores the `[unstable]` table, and the esp fork accepts `-Z` flags.
- **Implication for the teletext core crate.** Keep it plain `core`-only with no inline asm or target-specific code. It then builds unchanged for Xtensa with the esp toolchain, and every other target builds on stock stable.

## 3. Pitfalls for the no_std core across these targets

**Atomics without CAS (thumbv6m, riscv32imc).**
- Armv6-M (and Armv4/5TE) supports only atomic `load`/`store`. There is no `fetch_add` or `compare_exchange`. [V] https://doc.rust-lang.org/rustc/platform-support/arm-none-eabi.html
- riscv32imc/riscv32i are built with `+forced-atomics`. Their atomic loads and stores become plain loads and stores, and CAS/RMW is disabled (`atomic_cas = false`). "Downstream crates that need them use a critical-section polyfill (e.g. `portable-atomic`)." [V] https://doc.rust-lang.org/rustc/platform-support/riscv32-unknown-none-elf.html
- The fix is `portable-atomic` 1.15.0 with the `critical-section` feature, which requires the platform to provide a `critical-section` 1.2.0 impl. On single-core systems only, `unsafe-assume-single-core` also works. It supports Arm, RISC-V and Xtensa. [V] https://docs.rs/portable-atomic/latest/portable_atomic/
- Recommendation: the decoder core should need **no atomics at all**. Keep state in `&mut self` and leave synchronisation to the port.

**Floating point.**
- thumbv6m has no FPU, so `f32`/`f64` are soft-float library calls. [V] https://doc.rust-lang.org/rustc/platform-support/thumbv6m-none-eabi.html
- The `eabi` vs `eabihf` distinction is explained at https://doc.rust-lang.org/rustc/platform-support/arm-none-eabi.html [V]
- ESP32 has a single-precision FPU only. `double` "is emulated in software and is very slow". [V] https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/performance/speed.html
- [U] ESP32-C3 (imc) and ESP32-S2 have no FPU.
- Requirement: the core uses integer/fixed-point only, with no float in the hot path.

**64-bit integers and division on 32-bit cores.**
- Cortex-M0 has no hardware divide; the compiler calls `__aeabi_uidivmod`/`__aeabi_idivmod`. [V] https://answers.launchpad.net/gcc-arm-embedded/+question/233519
- [U] `u64` add/shift take several instructions on all 32-bit targets. `u64` multiply and divide are library calls on Armv6-M and RV32IMC (compiler-builtins).
- Prefer `u8`/`u16`/`u32`, use shifts and masks rather than `/` and `%` in hot paths, and prefer lookup tables.

**Unaligned access.**
- Cortex-M0/M0+ (Armv6-M) raise a HardFault on unaligned access. Cortex-M3 and later tolerate unaligned 16/32-bit accesses to normal memory. [V] https://community.st.com/t5/stm32-mcus-products/hardfault-with-unsigned-short/m-p/344594 , https://answers.launchpad.net/gcc-arm-embedded/+question/239149
- Safe Rust never produces misaligned references, so the risk comes only from `unsafe` pointer casts and `#[repr(packed)]`.
- Parse with byte slices and `u16::from_le_bytes`, and use `#![forbid(unsafe_code)]`.

**Endianness.**
- All listed targets are little-endian. That covers Cortex-M in practice, RISC-V, x86-64, AArch64 as configured by these triples, and ESP32 Xtensa (ELF format `elf32-xtensa-le`; endianness is fixed per core configuration). [V for Xtensa] https://docs.micropython.org/en/v1.13/esp32/general.html , https://www.oreilly.com/library/view/designing-socs-with/9780123724984/9780123724984_ch03lev1sec6.html
- Still use explicit `from_le_bytes`/`from_be_bytes`. You can test on a big-endian target with `cargo miri test --target s390x-unknown-linux-gnu`. [V] https://raw.githubusercontent.com/rust-lang/miri/master/README.md

**Stack size.**
- [U] MCU stacks are often only 2 to 16 KB, and small MCUs have 8 to 32 KB of RAM. Avoid large stack arrays and recursion. A 25×40 page is 1 KB, which is fine. Pass buffers by `&mut`.
- `flip-link` 0.1.12 (knurling) turns a stack overflow on Cortex-M into a HardFault instead of silent memory corruption.
- esp-hal has an optional stack-smashing-protection template option (`-Z stack-protector=all`). [V] esp-generate template above.

**Code size and `core::fmt`.**
- `core::fmt` dominates small binaries. In one measurement a defmt-based panic used 1.59 KB of flash versus 13.85 KB for panic-rprintln. [V] https://ferrous-systems.com/blog/defmt/
- The Tock/LCTES'22 paper found that replacing core::fmt with ufmt saved about 7.2 KB. [V] https://sing.stanford.edu/site/assets/publications/rust-lctes22.pdf
- Background: https://jamesmunns.com/blog/fmt-unreasonably-expensive/
- Requirement: the core must not use `format!`/`write!`/`{:?}` in non-test code. Implement `Display` only behind a feature, or in the std layer.

**Panics.**
- Use `panic = "abort"` in the profiles. [V] https://github.com/johnthagen/min-sized-rust
- Aim for panic-free code:
  - Clippy lints `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`, `expect_used`, `panic`. Clippy has an `allow-indexing-slicing-in-tests` config. [V] https://raw.githubusercontent.com/rust-lang/rust-clippy/master/CHANGELOG.md
  - Use `get()`, `wrapping_*`/`checked_*` and fixed-size arrays indexed by masked values (for example `TABLE[(b & 0x7F) as usize]` over a 128-entry array, which the optimiser can prove in-bounds).
  - Optional: the `no-panic` crate (0.1.37) asserts at link time that a function cannot panic. [V] crates.io
- Every panic site pulls in panic-location strings plus `core::fmt` unless the panic handler discards them. `panic_immediate_abort` needs nightly build-std. [V] min-sized-rust.
- An uncaught panic that crosses an `extern "C"` boundary aborts since Rust 1.81. [V] RELEASES.md 1.81, https://github.com/rust-lang/rust/pull/116088

## 4. Ecosystem crates

Versions are from the crates.io API on 2026-10-01. [V] https://crates.io/api/v1/crates/<name>

| Crate | Version | no_std? | Notes |
|---|---|---|---|
| heapless | 0.9.3 (MSRV 1.87) | yes | Fixed-capacity Vec/String/Deque, no allocator. |
| embedded-io | 0.7.1 (MSRV 1.81 on crates.io; master README says 1.83) | yes; `alloc`/`std` optional | Blocking byte-stream `Read`/`Write` with an associated `Error`, plus `ReadReady`/`WriteReady`. Use it as the port-supplied transport trait. https://github.com/rust-embedded/embedded-hal/tree/master/embedded-io |
| embedded-io-async | 0.7.0 | yes | Async versions of the same traits. |
| embassy-net | 0.9.1 | yes | Async TCP/UDP on smoltcp 0.14.0 (MSRV 1.91). A good example of a port implementing the transport. |
| embedded-nal / embedded-nal-async | 0.9.0 / 0.9.0 | yes | Socket-factory traits, an alternative to a custom trait. |
| embedded-tls | 0.19.0 | yes with `default-features=false` (default includes std, tokio) | TLS 1.3 client for no_std. reqwless 0.14.0 builds on it. |
| rustls | 0.23.45 | [U] no_std + alloc possible with a custom crypto provider | Default is std with aws-lc-rs. |
| defmt | 1.1.1 | yes | Deferred-format logging. Keep it optional in the core (feature `defmt`). |
| bitflags | 2.13.2 (MSRV 1.56) | yes (`std` is off by default) | |
| thiserror | 2.0.21 (MSRV 1.77) | **yes in 2.x** with `default-features = false`; the crate is `#![no_std]` and uses `core::error::Error` (stable since 1.81) | [V] https://github.com/dtolnay/thiserror/blob/master/Cargo.toml |
| png | 0.18.1 | **no** (built on `std::io`, `#![forbid(unsafe_code)]`) | For the std layer only. [V] https://github.com/image-rs/image-png/blob/master/src/lib.rs |
| clap | 4.6.7 | no | CLI only. |
| crossterm / ratatui | 0.29.0 / 0.30.2 | no | TUI viewer. [U] ratatui 0.30 split into modular crates and has a `ratatui-core`. |
| tungstenite | 0.30.0 (MSRV 1.85) | no | std WebSocket. |
| httparse | 1.10.1 | **yes** (`#![cfg_attr(not(feature="std"), no_std)]`, zero-alloc, sans-IO) | [V] https://github.com/seanmonstar/httparse/blob/master/src/lib.rs |
| embedded-websocket | 0.9.5 | yes with `default-features=false` | Sans-IO client and server framing with arbitrarily small buffers. You implement its Read/Write traits yourself. [V] https://github.com/ninjasource/embedded-websocket |
| base64 | 0.23.1 (MSRV 1.71) | yes (`default-features=false`; `alloc` optional) | |
| serde | 1.0.229 | yes (`default-features=false`) | |
| serde-json-core | 0.6.0 (2024) | yes (heapless) | Slow release cadence. |
| miniserde | 0.1.46 | `#![no_std]` + **alloc required** | |
| proptest | 1.11.0 | host tests | |
| cargo-fuzz / libfuzzer-sys | 0.13.2 / 0.4.13 | host, nightly | |
| criterion | 0.8.2 | host | |
| insta | 1.48.0 | host | Snapshot tests, suited to rendered pages and PNG hashes. |
| cargo-deny | 0.20.2 | tool | Licences and advisories. |
| cargo-semver-checks | 0.50.0 | tool | |
| cargo-hack | 0.6.45 | tool | `--feature-powerset` to test every std/alloc/no-default combination. |
| cargo-msrv | 0.19.3 | tool | |
| portable-atomic / critical-section | 1.15.0 / 1.2.0 | yes | |
| esp-hal | 1.2.2 (MSRV 1.95) | yes | |
| esp-idf-svc | 0.53.0 | std on ESP-IDF | |
| embedded-test / defmt-test | 0.7.2 / 0.5.0 | on-target test harnesses | |
| probe-rs-tools | 0.32.0 | tool | |
| espflash | 4.6.0 | tool | |
| cbindgen | 0.29.4 | tool | |

Miri runs on nightly only (`rustup +nightly component add miri`). It interprets any target with `--target`, so you can use `i686-unknown-linux-gnu` (Tier 1) for a 32-bit `usize` and `s390x-unknown-linux-gnu` for big-endian. It cannot run bare-metal targets. [V] https://github.com/rust-lang/miri

## 5. Multi-target workspace best practice

**Edition.**
- Edition 2024 has been stable since Rust 1.85 (2025-02-20). [V] RELEASES.md 1.85.0
- Edition 2024 defaults to resolver "3", the MSRV-aware resolver; the resolver config was stabilised in 1.84. [V] RELEASES.md 1.84; [U, from memory] https://doc.rust-lang.org/cargo/reference/resolver.html
- 2024 requires `#[unsafe(no_mangle)]` and `unsafe extern` blocks, which matters for the C API crate.
- Since 1.98, members on edition 2024 can override inherited `default-features` from a workspace dependency, which is useful for `serde = { workspace = true, default-features = false }`. [V] RELEASES.md (line ~87, recent release)
- The esp fork tracks current stable, so edition 2024 is usable on every target. **Recommend edition 2024.**

**MSRV.**
- Cargo docs suggest publishing an explicit policy such as "N-2" and setting `rust-version`. [V] https://doc.rust-lang.org/cargo/reference/rust-version.html
- esp-hal requires the latest stable at release time (currently 1.95). [V] https://github.com/esp-rs/esp-hal/blob/main/esp-hal/README.md
- Suggestion: core crate MSRV = the latest stable minus 2 or 3 releases, tested in CI. Port crates follow their HAL's MSRV.

**Features.**
- Core crate: `#![no_std]` with `default = []`, and optional `alloc`, `std` (implies alloc), `defmt` and `serde`.
- Put file formats, PNG, CLI and network protocol state machines in separate crates, or behind features that add `extern crate alloc`/`std`.
- Test combinations with `cargo hack --feature-powerset`.

**Proving no_std actually holds.**
- Run `cargo build -p teletext-core --target thumbv6m-none-eabi --no-default-features` in CI. Building for a target without std fails if std leaks in through dependencies.
- Also build for `riscv32imc-unknown-none-elf`, `thumbv7em-none-eabihf` and `xtensa-esp32-none-elf` (esp toolchain).

**`#![forbid(unsafe_code)]`.**
- Recommended for the core and protocol crates. png does the same.
- Since 1.98 the `unsafe_code` lint fires on all unsafe attributes (`#[unsafe(no_mangle)]`, etc.). [V] RELEASES.md 1.98.0, https://github.com/rust-lang/rust/pull/157201 So the C-API crate must be a separate crate that allows unsafe.

**Host testing.**
- Most tests run on the host with `cargo test`. The `#![no_std]` library gets std in its test harness with `#![cfg_attr(not(test), no_std)]`, or you use integration tests.
- Add proptest, insta and cargo-fuzz (nightly) on the host, and Miri on i686 and s390x.

**QEMU for ARM Cortex-M.**
- `qemu-system-arm -cpu cortex-m3 -machine lm3s6965evb -nographic -semihosting-config enable=on,target=native -kernel <elf>`, set as the cargo `runner` for `thumbv7m-none-eabi`. [V] https://docs.rust-embedded.org/book/start/qemu.html
- Output goes through `cortex-m-semihosting` 0.6.0 or `semihosting` 0.1.25.
- [U] For Armv6-M, QEMU's `microbit` machine (Cortex-M0) is an option. For Armv8-M, the `mps2-an505` machine (Cortex-M33).

**QEMU for RISC-V.**
- [U] Use `qemu-system-riscv32 -machine virt` with semihosting, or `qemu-riscv32` user-mode for a Linux build.

**Linux ARM/RISC-V builds.**
- `cross` (Docker/Podman) builds and tests Linux targets (aarch64, armv7, riscv64gc) under QEMU user-mode. It does **not** test bare-metal thumb/riscv32 targets; those are build-only. [V] https://github.com/cross-rs/cross

**On real hardware.**
- `embedded-test` 0.7.x with probe-rs runs `#[test]`s on the device (Arm, RISC-V and Xtensa/ESP32) over the debug probe. [V] https://github.com/probe-rs/embedded-test
- The esp-generate template supports probe-rs as the runner. [V] esp-generate config above.

**Xtensa emulation.**
- Espressif's QEMU fork supports **ESP32, ESP32-S3 and ESP32-C3**. It emulates CPU, UART, GPIO, flash and more, but not Wi-Fi or Bluetooth. It is unsupported ("Espressif does not provide support for QEMU"). [V] https://github.com/espressif/esp-toolchain-docs/blob/main/qemu/README.md
- Wokwi simulates ESP32 chips and runs in CI through `wokwi-ci-action` or `wokwi-cli`. It needs `WOKWI_CLI_TOKEN`, which is a commercial/free-tier service. [V] https://docs.wokwi.com/wokwi-ci/github-actions , https://docs.espressif.com/projects/esp-idf/en/latest/third-party-tools/wokwi.html
- So yes, Xtensa can be tested in an emulator, though less smoothly than Cortex-M under QEMU.
- Pragmatic approach: host tests cover the logic, the Xtensa CI job **builds only**, and an optional Wokwi or ESP-QEMU smoke test checks for expected UART output.

## 6. C API through cbindgen (optional)

- cbindgen 0.29.4 is under active but "adhoc" maintenance. It generates C and C++11 headers from `extern "C"`/`#[repr(C)]` items and can run as a CLI or from `build.rs`. [V] https://github.com/mozilla/cbindgen
- Caveats (mostly [U], from general practice):
  - Put it in a separate `teletext-ffi` crate (`crate-type = ["staticlib"]`) that wraps the safe core. Use opaque handles plus caller-provided buffers with no allocation.
  - Generic and `Option`-heavy APIs don't translate.
  - Edition 2024 requires `#[unsafe(no_mangle)]`.
  - The panic runtime:
    - Panics must not unwind into C; `extern "C"` aborts since 1.81. [V]
    - A `no_std` staticlib must provide its own `#[panic_handler]`, which can clash if the C firmware links two Rust staticlibs.
  - The staticlib must be built with the same target triple as the C firmware, which means the esp toolchain for ESP-IDF Xtensa, and the C compiler flags must use a matching float ABI (`eabihf` ↔ `-mfloat-abi=hard`).
  - Linking two separate Rust staticlibs into one C image causes duplicate-symbol problems (compiler_builtins, the panic handler). Ship a single staticlib.

## 7. Code-size expectations (thumbv6m, opt-level "z"/"s", LTO)

- Recipe: `opt-level = "z"` (also try "s", which is sometimes smaller), `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip`/`debug = false` for measurement. [V] https://github.com/johnthagen/min-sized-rust
- Measure with `cargo size`/`cargo bloat` in CI: cargo-binutils 0.4.0, cargo-bloat 0.12.1.
- **Estimate [U, no authoritative benchmark found]:**
  - Tables land in `.rodata` at essentially their source size if declared as `static`/`const` arrays of `u8`/`u16`. 10 to 20 KB of tables gives 10 to 20 KB of flash.
  - A bit-manipulation and state-machine decoder (Hamming 8/4 and 24/18 decode, parity, packet and page assembly, a character-set mapper) should compile to about **5 to 20 KB of Thumb-1 code** without `core::fmt`.
  - The total core is roughly **15 to 40 KB of flash**.
  - Pulling in `core::fmt` (a `{:?}` or panic message formatting) typically adds **several KB to over 10 KB**. See the defmt measurement (1.59 KB vs 13.85 KB panic paths) and about 7.2 KB saved by ufmt in the LCTES'22 paper. [V for those figures]
  - RAM: the page buffer plus decoder state should be a few KB. It is caller-owned, so the port decides where it lives.
- Guidance:
  - Keep tables as `static` (not `const`, which can be duplicated at use sites).
  - Consider bit-packing G0/G2 national option tables.
  - Avoid monomorphisation blow-up by preferring `&mut dyn Trait` or few instantiations on the MCU side.
  - Add a CI size budget, for example failing if `.text+.rodata` for a thumbv6m example firmware exceeds X KB.

---

## Summary

- **Tiers.** Cortex-M, RISC-V bare-metal (imc, imac, imafc) and aarch64-none are Tier 2 no_std. aarch64-linux, x86_64-linux, x86_64-windows-msvc and aarch64-apple-darwin are Tier 1. x86_64-apple-darwin was demoted to Tier 2 in Rust 1.90. armv7 and riscv64gc Linux are Tier 2 with host tools. All Xtensa targets are Tier 3.
- **Xtensa.** The targets exist upstream since 1.81 and upstream LLVM now has an Xtensa backend. In practice you still use the espup "esp" fork, which tracks stable (v1.99.0.0 released 2026-09-30), with `-Z build-std=core`.
  - CI uses `esp-rs/xtensa-toolchain` (v1.7.0).
  - Pin the version: an open LLVM-21 ICE affects 1.94+.
- **Pitfalls.** No CAS on thumbv6m/riscv32imc (use `portable-atomic` + `critical-section`, or avoid atomics). Soft-float, and ESP32 doubles are emulated. No hardware divide on M0. HardFault on unaligned access on M0. `core::fmt` bloat of several KB to 13 KB.
  - All targets are little-endian, but test big-endian with Miri on s390x.
  - Enforce panic-free code with clippy lints.
- **Crates.**
  - no_std: heapless, embedded-io, httparse, embedded-websocket, base64, thiserror 2, bitflags, serde.
  - std-only: png, tungstenite, clap, ratatui.
- **Practice.**
  - Edition 2024 with resolver 3, an explicit MSRV policy, and a feature powerset tested with cargo-hack.
  - `forbid(unsafe_code)` in the core, with the FFI crate kept separate.
  - Cortex-M runs in QEMU through semihosting. ESP runs in the ESP-QEMU fork (ESP32/S3/C3) or on Wokwi. `embedded-test` + probe-rs runs tests on hardware.
- **Size [estimate].** About 15 to 40 KB of flash on thumbv6m, of which the tables are 10 to 20 KB, provided `core::fmt` is kept out.
