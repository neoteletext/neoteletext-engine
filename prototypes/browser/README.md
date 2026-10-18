# The browser prototype

A windowed browser for teletext channels, built on the neoteletext engine: tabs, an address bar, back and forward, bookmarks, the tuning screen, the contents view and the font panels. It was moved here on 16 October 2026, when the repository was refocused on the core teletext rendering engine (REQUIREMENTS.md decision E.76).

It is a Cargo workspace of its own, built against the engine's crates in `../../crates` by path. It is outside the engine's workspace, so the engine's gates (`scripts/check.sh`) neither build nor test it.

```sh
cd prototypes/browser
cargo build --release -p neoteletext-gui
target/release/neoteletext https://example.org/
scripts/macos-app.sh            # builds neoteletext.app in target/
```

| Path | Contents |
|---|---|
| `crates/neoteletext-browser` | Everything the browser decides, with no operating-system access: addresses, history, the tuning screen, the contents view and the rows the receiver draws itself. |
| `crates/neoteletext-gui` | The `neoteletext` window, and its decoder process (`decoder`, formerly in `neoteletext-app`). Its fetcher and font helpers are the engine's, in `neoteletext-app`. |
| `docs/requirements.md` | The browser's requirements and decisions, moved out of REQUIREMENTS.md with their numbers. |
| `docs/` | The browser's design, the web teletext concept, channels, discovery and the macOS build decisions. |
| `tests/sites.py` | The local test channels the window's end-to-end tests run against. |
| `TECH-DEBT.md` | What was owed on the browser when it moved. |
