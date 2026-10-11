# Open questions

Everything still open on 12 October 2026, grouped by what each item needs before it can move. Each item names where its full context lives.

## 1. Owner decisions

| # | Question | Context | Default meanwhile |
|---|---|---|---|
| 1 | **Framing tolerance (decision E.24).** Accepting one wrong framing-code bit admits false packets on 5.9% of random lines, against 0.34% strict (2.1% against 0.16% on VPS-shaped lines). Allow the tolerance only where a packet is expected? | IMPLEMENTATION-LOG.md row 21 | Tolerant, as E.24 states |
| 2 | **Two licences outside REQ-GEN-017's list.** Unicode-3.0 (`unicode-ident`, build time only) and CDLA-Permissive-2.0 (`webpki-roots`, the TLS root certificates). Confirm the per-crate exceptions, or add the licences to the list. | Row 32; `deny.toml` | Allowed for those two crates only |
| 3 | **Black colour codes (decision E.18, amended).** Alpha black 0/0 and mosaic black 1/0 are decoded, because Italian RAI and Dutch regional pages depend on them. Confirm the amendment. | Row 36; E.18 | Decoded |
| 4 | **Review decisions E.40 to E.62.** In particular E.47 (X/26 wins over Level 1 attributes on one cell), E.48 (flash timing), E.57 and E.58 (points the standard leaves open), E.61 (compile-time features) and E.62 (original glyphs). | REQUIREMENTS.md Part E | As recorded |
| 5 | **Bedstead in the repository.** Keep `third_party/bedstead/` as the reference the glyph independence check compares against, or move it outside the repository (for example fetched at test time like the corpus). | E.62; `third_party/bedstead/PROVENANCE.md` | Kept, not built by default |
| 6a | **Review decisions E.63 to E.69** (the browser). Choices marked *(build)* were taken on 12 October 2026 while you were away: in particular E.63 item 1 (a site is an origin), E.65 item 4 (a link to a confirmed site still asks), E.67 (`q` does not quit the window), E.68 item 11 (the BSL-1.0 exception), and E.69 items 2 and 5 (`sandbox-exec` instead of XPC; no WebAssembly wall yet). | REQUIREMENTS.md Part E; `docs/macos-build-decisions.md`; issue #162 | As recorded |
| 6b | **Channels (decision E.70)**, built on 12 October 2026. Review the details written to your instruction, in particular the folder-name rules (E.70 item 3), `.tti` before `.t42` (item 6), no `index.tti` (item 15), and cookies per channel (item 12). | REQUIREMENTS.md decision E.70, REQ-WEB-018 to REQ-WEB-027; `docs/channels.md` | As built |
| 6 | **Merge and push.** Merge `implementation` into `main`, and push. | — | Not merged, not pushed |

## 2. Needs outside help or data

| # | Item | Context |
|---|---|---|
| 7 | **Review of the original glyphs by fluent readers** of Greek, Cyrillic, Hebrew and Arabic, and a Latin review. Known weak glyphs: `m`, `ø`, `©` against `®`, `Ă` and `Ğ`, the fractions, the one-dot pairs `¨`/`ˇ` and `˜`/`˝`, the Arabic three-dot letters (one dot from their two-dot partners), and ح ج خ without a deep bowl. | Row 54; `cargo xtask glyph-proof OUT.png [--compare]`; `ttx … --glyphs compare` |
| 8 | **Arabic glyph independence cannot be audited**: Bedstead has no Arabic. A different reference, or a human check, is needed. | E.62; row 54 |
| 9 | **German Level 2.5 broadcast data**: a recording of ZDF or other German services from Astra 19.2°E or DVB-T2. | REQUIREMENTS.md Appendix D.2 |
| 10 | **A real broadcast with objects, DRCS pages or side panels.** None found; these are tested on the authored stream only. | REQ-TST-025; Level 2.5 and 3.5 round status |
| 11 | **TTI files written by vbit2**, to test the reader against. | REQ-INP-023 |
| 12 | **ARIB STD-B5** (Japanese teletext, System D), only if Japanese support is wanted. Japanese only, withdrawn 2023, sold by the ARIB Web Store. It is a different system, so support would be a separate decoder. | — |

## 3. Proposals waiting for a go-ahead

| # | Proposal | Next step | Context |
|---|---|---|---|
| 13 | **Web-native teletext browser.** All 14 decisions settled, built on 12 October 2026, and recorded in Part E as decisions E.63 to E.67, with Appendix E, REQ-WEB-002 and the row 0 requirements amended. | Review the build's own choices (item 6a). | `docs/web-native-concept.md`; REQUIREMENTS.md decisions E.63 to E.69 |
| 14 | **Input devices across machines.** Core-owned input routing, including the security confirmations; one `Function` enum; press, repeat and release in the port interface; binding tables as data; the dial and on-screen keyboard; pasted input never confirming. | Decide its seven decisions; the document is not yet committed. | `docs/hmi.md` section 7 |
| 15 | **Browser for Windows, macOS, Linux, Android and iOS.** Tabs, address bar, back and forward, bookmarks; all behaviour in the platform-free `neoteletext-browser`; interface, fetcher and decoder processes where the platform allows, and the decoder inside the wasmi interpreter everywhere (iOS is one process). The tuning screen stays inside the page area (owner's instruction). Mobile decided by the owner on 11 October 2026. | The toolkit is decided (egui on all five, no Slint; 11 October 2026). Remaining decisions, chiefly: the WebAssembly wall; accepting one process on iOS; the C interface to the phone shells; minimum Android and iOS versions; the Apple, Play and Windows accounts; a Windows build machine. | `docs/desktop-browser.md` sections 15 and 16 |

## 4. Known technical gaps

Recorded, not blocking.

| # | Gap | Context |
|---|---|---|
| 16 | The full `ttx` grew by 16 KB of code when its application layer moved into `neoteletext-app`; the cause was not found. | Row 50 |
| 17 | The fetcher is a second run of `ttx`, not a separate program, so a full build's decoder process still contains TLS code (its sandbox keeps it off the network). | E.61 |
| 18 | The web agent's buffers (`agent::Storage`) do not shrink with its features. | Row 50 |
| 19 | The Cortex-M0 test image is linked and measured but never run; core tests under QEMU system emulation remain to do. | REQ-GEN-019; round status |
| 20 | The zvbi comparisons are run by hand on `oldvga`, not by the gate. | Rows 37 and 45 |
| 21 | Sandboxes for macOS and Windows (Phase 2); kitty and Sixel terminal graphics (deferred). | REQ-SEC-017; decision 3 of the log |
| 22 | Characters with a dot below show a full stop on the baseline, because the renderer overlays `.` for the mark; a proper dot-below mark is a small change. | E.26; `glyphs::SPACING_MARKS` |
| 23 | The interactive viewer does not print state-machine traces; the other commands do with `-vv`. | Level 2.5 and 3.5 round status |
