# tellytxt

A software receiver for World System Teletext (ETSI EN 300 706, Presentation Levels 1.0, 1.5, 2.5 and 3.5), written in Rust. The core library is `no_std` with no allocation and runs on microcontrollers; the `ttx` command-line program decodes recordings, live feeds and web teletext services, and shows pages in the terminal (iTerm2 inline images or coloured text), as text, or as images.

```sh
cargo build --release -p tellytxt-cli
target/release/ttx show recording.t42 100
target/release/ttx view zxnet:teefax
```

Each crate's optional parts are Cargo features, on by default, so a port or a smaller `ttx` compiles in only what it uses. The features are listed in the user guide and the porting guide.

## Documents

| Document | Contents |
|---|---|
| [docs/user-guide.md](docs/user-guide.md) | The `ttx` program: commands, inputs, outputs, profiles and settings. |
| [docs/porting.md](docs/porting.md) | Writing a port of the library for a new display or device. |
| [REQUIREMENTS.md](REQUIREMENTS.md) | The specification: requirements, decisions and open questions. |
| [ARCHITECTURE.md](ARCHITECTURE.md) | The layers, their boundaries and allowed dependencies. |
| [THREAT-MODEL.md](THREAT-MODEL.md), [SECURITY.md](SECURITY.md) | Security design and reporting. |
| [docs/traceability.md](docs/traceability.md) | Each requirement and the files that implement and test it (`cargo xtask traceability`). |
| [docs/sources.md](docs/sources.md) | Where test data and live feeds come from. |
| [IMPLEMENTATION-LOG.md](IMPLEMENTATION-LOG.md) | Questions met during implementation and the decisions taken. |

## Crates

| Crate | Role |
|---|---|
| `tellytxt-core` | Error protection, packets, acquisition, the page and enhancement stores, character sets, Level 1.0 and 1.5 decoding, the Level 2.5 and 3.5 enhancement processor with objects, DRCS and the Colour Map, the state machines, the row renderer and the viewer model. `no_std`, no allocation, no panics. |
| `tellytxt-formats` | Readers and writers: T42, transport streams, PES, sliced VBI, TTI (with enhancement data and enhancement pages), EP1, edit.tf and screen dumps; and a builder for Level 2.5 and 3.5 test pages. |
| `tellytxt-slicer` | A software data slicer for raw sampled video. |
| `tellytxt-web` | The sans-IO web agent: service descriptors, HTTP, WebSockets, server-sent events, cookies, forms and the zxnet feed. |
| `tellytxt-image` | Palettes, pixel formats, scaling, PNG and PPM writers. |
| `tellytxt-cli` | The `ttx` program. |
| `xtask` | Project checks: layers, dependencies, constants, glyphs (including the generated G3 set), the test corpus and the traceability table; `cargo xtask level25-stream` writes the authored Level 2.5 and 3.5 test stream. |

## Checks

`scripts/check.sh` runs the fast checks and `scripts/check.sh full` runs everything. The git hooks in `.githooks` run them before each commit and push. Tests that need the real broadcast corpus read it from `$TELLYTXT_CORPUS` (see `cargo xtask corpus`) and say so when it is absent.

## Licensing of data

No standards documents, character-generator ROM glyphs, GPL code or broadcast recordings are kept in this repository. The glyphs are temporarily taken from the Bedstead font; see `third_party/bedstead/PROVENANCE.md`.
