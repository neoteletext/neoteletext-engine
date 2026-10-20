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
| 6a | **Review decisions E.63 to E.69** Closed on 18 October 2026: the browser was removed from this repository and is maintained in the separate neoteletext-browser repository (decision E.76). | | |
| 6b | **Channels (decision E.70)**, built on 12 October 2026. Review the details written to your instruction, in particular the folder-name rules (E.70 item 3), `.tti` before `.t42` (item 6), no `index.tti` (item 15), and cookies per channel (item 12). | REQUIREMENTS.md decision E.70, REQ-WEB-018 to REQ-WEB-027 | As built |
| 6 | **Merge and push.** Merge `implementation` into `main`, and push. | — | Not merged, not pushed |

## 2. Needs outside help or data

| # | Item | Context |
|---|---|---|
| 7 | Withdrawn: the review of the original glyphs. Decision E.72 removed the original set; Bedstead is the bundled font. | E.72 |
| 8 | Withdrawn: the audit of Arabic glyph independence, for the same reason. Arabic text needs a loaded font (area FNT). | E.72 |
| 9 | **German Level 2.5 broadcast data**: a recording of ZDF or other German services from Astra 19.2°E or DVB-T2. | REQUIREMENTS.md Appendix D.2 |
| 10 | **A real broadcast with objects or side panels.** None found; these are tested on the authored stream only. | REQ-TST-025; Level 2.5 and 3.5 round status |
| 11 | **TTI files written by vbit2**, to test the reader against. | REQ-INP-023 |
| 12 | **ARIB STD-B5** (Japanese teletext, System D), only if Japanese support is wanted. Japanese only, withdrawn 2023, sold by the ARIB Web Store. It is a different system, so support would be a separate decoder. | — |

## 3. Proposals waiting for a go-ahead

| # | Proposal | Next step | Context |
|---|---|---|---|
| 13 | **Web-native teletext browser.** Closed on 18 October 2026: the browser was removed from this repository and is maintained in the separate neoteletext-browser repository (decision E.76). | | |
| 14 | **Input devices across machines.** Core-owned input routing, including the security confirmations; one `Function` enum; press, repeat and release in the port interface; binding tables as data; the dial and on-screen keyboard; pasted input never confirming. | Decide its seven decisions; the document is not yet committed. | `docs/spec/design/hmi.md` section 7 |
| 15 | **Browser for Windows, macOS, Linux, Android and iOS.** Closed on 18 October 2026: the browser was removed from this repository and is maintained in the separate neoteletext-browser repository (decision E.76). | | |

## 4. Known technical gaps

Recorded, not blocking.

| # | Gap | Context |
|---|---|---|
| 16 | The full `neottx` grew by 16 KB of code when its application layer moved into `neoteletext-app`; the cause was not found. | Row 50 |
| 17 | The fetcher is a second run of `neottx`, not a separate program, so a full build's decoder process still contains TLS code (its sandbox keeps it off the network). | E.61 |
| 18 | The web agent's buffers (`agent::Storage`) do not shrink with its features. | Row 50 |
| 19 | The Cortex-M0 test image is linked and measured but never run; core tests under QEMU system emulation remain to do. | REQ-GEN-019; round status |
| 20 | The zvbi comparisons are run by hand on `oldvga`, not by the gate. | Rows 37 and 45 |
| 21 | Sandboxes for macOS and Windows (Phase 2); kitty and Sixel terminal graphics (deferred). | REQ-SEC-017; decision 3 of the log |
| 22 | Characters with a dot below show a full stop on the baseline, because the renderer overlays `.` for the mark; a proper dot-below mark is a small change. | E.26; `glyphs::SPACING_MARKS` |
| 23 | The interactive viewer does not print state-machine traces; the other commands do with `-vv`. | Level 2.5 and 3.5 round status |
