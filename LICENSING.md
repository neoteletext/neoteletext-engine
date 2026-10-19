# Licensing

neoteletext is licensed under the MIT licence ([LICENSE](LICENSE)). This document says which other licences the project may use, and why each one leaves the MIT licence intact. T42 Browser (the neoteletext-browser repository) follows the same rule; its own details are in that repository's `docs/licensing.md`.

This is the project's reading of the licences, not legal advice.

## 1. The rule

Using third-party material must never compromise the MIT licence. Anyone who receives neoteletext's code must be able to use, change and ship it, open or closed, under the MIT licence alone. They must never become bound by another licence's terms just because they took our code.

A licence is acceptable if, used the way we use it, it puts no conditions on our code beyond keeping a notice.

## 2. What is acceptable

| Kind | Licences | Why our licence is not compromised | Conditions |
|---|---|---|---|
| **Code compiled into our programs** | MIT, Apache-2.0 (also with the LLVM exception), BSD-2-Clause, BSD-3-Clause, Zlib, ISC, CC0-1.0: the list of REQ-GEN-017, which `deny.toml` enforces | They are permissive: they ask for a notice and nothing more of the code that uses them | Keep their notices in any binary distribution |
| **Data compiled into our programs** | Unicode-3.0 (Unicode's tables), CDLA-Permissive-2.0 (Mozilla's root certificates) | Permissive data licences with no copyleft | Allowed one crate at a time in `deny.toml` (decision 32 of IMPLEMENTATION-LOG.md) |
| **Fonts shipped with or converted by the project** | CC0-1.0 (Bedstead), SIL Open Font License 1.1 (Noto) | The OFL binds the font, not the program that draws it. A modified font must stay under the OFL, but the program's code is untouched | Ship the OFL text with the font; never sell the font on its own |
| **System libraries loaded at run time and not shipped** | LGPL-2.1-or-later (glibc, ALSA's `libasound`), GPL with the GCC Runtime Library Exception (`libgcc_s`) | Linked dynamically from the user's own system, so the user can always replace them, which is all the LGPL asks. The GCC exception exists to allow any program to link the runtime | Never link them statically and never bundle them (see section 3) |
| **A choice of licences** | For example `r-efi` (MIT, Apache-2.0 or LGPL-2.1-or-later) or `byteorder` (Unlicense or MIT) | We take it under the permissive choice | None beyond that choice's own |

The owner accepted LGPL system libraries on 19 October 2026: the system's `libasound` for sound on Linux (decision D3 of `docs/spec/design/audio-platforms.md`, and T42 Browser's decision 40).

## 3. What is not acceptable

- **GPL and AGPL code in anything we ship** (REQ-GEN-017). Linking it into a program puts the whole program under the GPL. The repository keeps no GPL code. The GPL decoders in CREDITS.md are used only as test oracles: they run as separate programs, and nothing we ship contains their code. The same goes for libzvbi (LGPL), which our own oracle programs call at test time.
- **LGPL code compiled in or linked statically.** That would oblige anyone who ships our program to let users relink it against their own copy of the library.
- **LGPL libraries bundled with a release** (for example inside an AppImage or Flatpak) unless the release also carries the LGPL text and offers that library's source. The preferred course is not to bundle them: a package lists `libasound` as a dependency instead.
- **File-level copyleft code compiled in,** such as MPL-2.0 or EPL-2.0. It would not spread to our files, but changed files would have to be published. It needs the owner's decision first.
- **Licences that are not open source:** the Business Source License, the Server Side Public License, the Commons Clause, non-commercial or field-of-use limits.
- **Code with no licence.** It cannot legally be copied at all.

## 4. How it is enforced

- **Crates:** `cargo-deny` checks every crate's licence against the allow list in `deny.toml` on every full gate (`scripts/check.sh full`). An exception names one crate and one licence, with its reason.
- **System libraries** are outside cargo-deny's view. A new one is recorded here before it is used.
- **Bundled material** (`third_party/`) keeps its licence and provenance beside it, and is credited in CREDITS.md.

## 5. What is in use

Measured on 20 October 2026 from `cargo metadata --all-features`.

- **Crates:** about 110, all under the licences of section 2: MIT or Apache-2.0 for most; `ring` (Apache-2.0 and ISC), `rustls` and `rustls-webpki` (ISC among their choices), `subtle` (BSD-3-Clause), `unicode-ident` (adds Unicode-3.0), `webpki-roots` (CDLA-Permissive-2.0). The library crates that run on a microcontroller have no dependencies at all (REQ-GEN-017).
- **Fonts:** Bedstead 3.261 (CC0-1.0, `third_party/bedstead/`), and fonts converted from Noto (SIL OFL 1.1, `third_party/noto/`).
- **System libraries:** the C library of each platform. The engine itself plays no sound; `neoteletext-audio-mix` only mixes.
