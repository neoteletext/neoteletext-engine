# neoteletext

A software receiver for World System Teletext (ETSI EN 300 706, Presentation Levels 1.0, 1.5, 2.5 and 3.5), written in Rust. The core library is `no_std` with no allocation and runs on microcontrollers; the `ttx` command-line program decodes recordings, live feeds and web teletext services, and shows pages in the terminal (iTerm2 inline images or coloured text), as text, or as images.

```sh
cargo build --release -p neoteletext-cli
target/release/ttx show recording.t42 100
target/release/ttx view zxnet:teefax
```

The `neoteletext` browser opens web teletext channels, files and live feeds in a window with tabs, an address bar, back and forward, and bookmarks. Each tab's network and page decoding run in separate processes, confined on macOS so the decoder can reach neither files nor the network. Leaving a channel always shows the tuning screen first. A channel is a folder of page files on any web server, static or generated: see [docs/channels.md](docs/channels.md) to publish one.

```sh
cargo build --release -p neoteletext-gui
target/release/neoteletext https://example.org/
scripts/macos-app.sh            # builds neoteletext.app in target/
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
| [docs/channels.md](docs/channels.md) | Teletext channels on the web: how a television channel's pages are laid out, why a channel is a folder, the contents file, publishing as static files or from a program, and media types. |
| [docs/desktop-browser.md](docs/desktop-browser.md), [docs/web-native-concept.md](docs/web-native-concept.md) | The browser's design and the web teletext concept it browses. |
| [docs/macos-build-decisions.md](docs/macos-build-decisions.md) | The decisions taken while building the macOS browser, for review. |

## Crates

| Crate | Role |
|---|---|
| `neoteletext-core` | Error protection, packets, acquisition, the page and enhancement stores, character sets, Level 1.0 and 1.5 decoding, the Level 2.5 and 3.5 enhancement processor with objects, DRCS and the Colour Map, the state machines, the row renderer and the viewer model. `no_std`, no allocation, no panics. |
| `neoteletext-formats` | Readers and writers: T42, transport streams, PES, sliced VBI, TTI (with enhancement data and enhancement pages), EP1, edit.tf and screen dumps; and a builder for Level 2.5 and 3.5 test pages. |
| `neoteletext-slicer` | A software data slicer for raw sampled video. |
| `neoteletext-web` | The sans-IO web agent: service descriptors, HTTP, WebSockets, server-sent events, cookies, forms and the zxnet feed. |
| `neoteletext-image` | Palettes, pixel formats, scaling, PNG and PPM writers. |
| `neoteletext-cli` | The `ttx` program. |
| `neoteletext-browser` | Everything the browser decides, with no operating-system access: addresses, sites, history, the tuning screen and the rows the receiver draws itself. |
| `neoteletext-gui` | The `neoteletext` browser window and its fetcher and decoder helper processes. |
| `xtask` | Project checks: layers, dependencies, constants, glyphs (including the generated G3 set), the test corpus and the traceability table; `cargo xtask level25-stream` writes the authored Level 2.5 and 3.5 test stream. |

## Checks

`scripts/check.sh` runs the fast checks, and the git hooks in `.githooks` run them before each commit and each push. `scripts/check.sh full` runs everything, including every target and fuzzing on the Linux build host (`NEOTELETEXT_REMOTE`); it takes about 40 minutes and is run before merging to `main` and before a release. Tests that need the real broadcast corpus read it from `$NEOTELETEXT_CORPUS` (see `cargo xtask corpus`) and say so when it is absent.

## Licence

The project is licensed under the MIT licence; see [LICENSE](LICENSE). Bundled third-party material keeps its own licence, recorded beside it.

## Licensing of data

No standards documents, character-generator ROM glyphs, GPL code or broadcast recordings are kept in this repository. The glyphs are the project's own (`crates/neoteletext-core/src/glyphs/original.txt`, decision E.62). The Bedstead font in `third_party/bedstead/` is no longer used by default; it stays as the reference the independence check compares against.
