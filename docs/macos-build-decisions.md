# Decisions taken while building the macOS browser

The owner asked on 12 October 2026 for a working macOS browser with tabs and an address bar, built to completion with no questions asked, and for every decision to be recorded for later review. This is that record. Each entry says what was decided, why, and what it would take to change it.

These are now in REQUIREMENTS.md, recorded on 12 October 2026 as decisions E.63 to E.69, where each is marked *(build)* with its number here; section 4 says where each part went. The project's rule is that behaviour is recorded in Part E before it is built (REQ-DOC-004); this build went first because the owner asked for it, and the entries were written afterwards. This document keeps the reasons in full for the owner's review (issue #162); a change after review is recorded as an amendment in Part E.

The design is `docs/desktop-browser.md`; the concept is `docs/web-native-concept.md`. "Design decision n" means section 15 of the design.

---

## 1. Architecture

**D1. One fetcher process and one decoder process per tab.**
- The design's first version (design decision 1). One decoder per site, shared by tabs, is later work; the messages are the same either way.
- The interface routes every frame between them (design section 3.2), and reads all of it as untrusted.

**D2. The helpers are confined on macOS by Seatbelt profiles applied with `/usr/bin/sandbox-exec`, not XPC services.**
- Design decision 6 recommended XPC services. XPC needs an app bundle with nested services and Objective-C or `unsafe` code, which only the C interface crate may contain (REQ-SEC-015). `sandbox-exec` needs neither, works for the bare binary and the bundle, and confines the helper before its first instruction.
- `sandbox-exec` is deprecated by Apple but present and working on macOS 15.6. If Apple removes it, XPC services are the replacement.
- The profiles deny everything, then allow only what was measured to be needed (`crates/neoteletext-app/src/sandbox.rs`):
  - **decoder:** run itself, read the system libraries, its own file and the root directory, read `sysctl` values. No files, no network, no programs.
  - **fetcher:** the same, plus outgoing connections, name resolution through the system resolver, and the resolver's files.
- `helpers_are_confined` (`crates/neoteletext-gui/tests/e2e.rs`) proves it on every run: under the decoder profile a file read, a connection and starting a program all fail; under the fetcher profile only the connection works; unconfined, all three work, so the checks are real.

**D3. The interface process itself is not sandboxed.**
- The App Sandbox needs a signed bundle with entitlements. It is part of the release work (design section 12.3) and needs the Apple Developer Program membership (design decision 8).

**D4. The WebAssembly wall (design decision 12) is not in this build.**
- On macOS the decoder already runs in a process with no file or network access. The interpreter is the only wall available on iOS, and it is tracked in issue #160.

**D5. All browser behaviour is in `neoteletext-browser`, which has no operating-system access.**
- Design decision 11. It holds address parsing, sites, history, the tuning screen's rules and its page, row 0, the start, keys and error pages, and the saved-file formats. `xtask check-io` treats it as a library crate.
- `neoteletext-gui` holds the window, the tabs' processes and storage.

**D6. The decoder's engine is a new module, `neoteletext_app::decoder`, ported from `ttx view`'s loop rather than replacing it.**
- The terminal viewer still has its own loop. Moving `ttx view` onto the engine is later work; the shared pieces (`interact`, `present`) were moved down to the app crate now, so they are not duplicated.

## 2. Behaviour

**D7. A site is an origin: scheme, host and port.**
- The design uses the registrable domain (design section 7.2). An origin is stricter, so more moves show the tuning screen, and it needs no public suffix list.
- Revisit when sites on subdomains of one domain become common.

**D8. Sites live at the root of their origin.**
- The descriptor is at `/.well-known/teletext.json`. Typing that address names the site itself.
- **A site without a descriptor** (404 or 410 for the well-known address) gets defaults, built as a descriptor so REQ-WEB-002's checks apply:
  - its name is the host;
  - its initial page is 100;
  - its pages are `/{mpp}.t42`;
  - its front door is `/index.t42`.
- **Any other failure** (a server error, an invalid descriptor) is reported, not replaced by the defaults, so a broken site never silently changes meaning.

**D9. The front door is bounded by the page limit (256 KB).** A bundled site larger than that publishes page files instead.

**D10. A page's own file fills only the pages asked for** (concept section 2 item 4).
- Any other page in it is dropped, by its header. The exception is enhancement pages, whose numbers are not decimal and which a page's file may carry for its objects (item 6).
- A front door, push messages, a form's answer and files fill every page they hold.
- `ttx`'s web mode follows the same rule, since it shares the code.

**D11. Redirects to another site are refused and shown as a tuning screen.**
- In the browser the agent reports `RedirectToOtherSite` instead of following (concept section 6). An upgrade from `http` to `https` on the same host is not another site.
- `ttx` keeps following redirects as before.
- Form submissions keep their existing rule, under which a redirect to another origin makes the submission cross-site for cookies.

**D12. When the tuning screen is shown.** It is shown for:
- a typed address, including in a new tab;
- a bookmark or history entry;
- a restored tab;
- any link;
- any redirect.

It is skipped only for back and forward to a site confirmed this session (concept decision 4). A link to a site already confirmed still asks; the concept leaves this open, and asking is the safer reading.

**D13. Row 0 is drawn by the interface on every page.**
- The top 20 pixel lines of every decoder frame are replaced, across their whole width.
- On web sites, row 0 shows the receiver's status, the site's name, and the clock (concept section 4). The status is `P` and the page digits, with the red `!` marker for plain HTTP.
- For files and live feeds, it shows the status and the broadcast's own header text, without the browser's clock.
- A decoder's reported digits are accepted only as three hexadecimal digits or `-`; anything else shows `---`.
- `a_decoder_cannot_draw_row_0` and `a_hostile_page_cannot_forge_row_0_or_the_tuning_screen` test this.

**D14. Only the interface confirms.** A form submission's confirmation is a bar outside the page, answered by the user. A decoder's `Confirm` frame is dropped, never passed on. The tuning screen is answered the same way.

**D15. The pages the receiver draws itself** (start, keys, tuning, errors, loading) go through the same Level 1 decoding as broadcast pages, but are built in the interface and never pass through a decoder.

**D16. Files.**
- They open from a typed path or `file://` address, or by dropping them on the window. There is no file dialog, which would need another library.
- T42 files are fed as packets, up to 256 MB. Anything else is treated as TTI, up to 256 KB.
- Other formats `ttx` reads (transport streams, sliced VBI, EP1, edit.tf) are not opened by the browser yet.

**D17. A file or front door without the page asked for opens at its own first page,** unless the user has already chosen a page.

**D23. Addresses in pages.** Concept decision 1, built as the concept describes:
- An `LN` target containing `://` is an address. It may be `http` or `https`, printable ASCII without spaces or commas, at most 256 bytes; anything else makes the link invalid.
- `LK,mpp,target` links a page number, at most 8 per page.
- `LT,row,column,length,target` links a run of text on rows 1 to 23, at most 8 per page.
- The TTI writer keeps all three.
- The page checker warns when a page links a number it does not show (concept section 5).
- An address always goes through the tuning screen, marked "not declared by the site".
- A typed number follows, in order:
  1. the page's own link, which the decoder checks;
  2. otherwise the descriptor's alias, which the interface checks when the decoder asks for the page;
  3. otherwise the site's own page.

**D24. Descriptor aliases and refresh** (concept decision 13).
- **`aliases`:** an object of page numbers and targets. A target is `name:mpp` or an address; a relative address is refused, and an address in an `https` descriptor must be `https`. At most 16 aliases.
- **`refresh`:** a whole number of seconds from 5 to 86 400. It is used only when the service has no push, and asks for the page on screen again at that interval.

**D25. Page catching by keyboard** (concept decision 11).
- **Turning it on:** Enter turns catching on at the first link in reading order: page numbers printed on rows 1 to 23, and `LT` runs.
- **Moving:** Right and Down move forward; Left and Up move back.
- **Following:** Enter follows the selected link.
- **Turning it off:** Escape or Delete, or any other key, which then does what it does.
- **On screen:** the selected link has its colours swapped. Row 0 shows `LINK` and the destination as the decoder reports it, written only after the receiver's `LINK`, so it cannot be mistaken for the tuning screen's `GO?`.
- **Pages with side panels** are not caught, because their text columns are not the grid's.
- **With the pointer:** a number under the pointer names its page in the status line.
- **Show links:** `l` underlines every link with the receiver's own underline, and `l` again stops. The concept names no key, so `l` was chosen as the letter for "links".

**D26. Enhancement dependencies** (concept section 2 item 6). When a page is shown at Level 2.5 or 3.5, the decoder asks once for its magazine's Organization Table (`xFE`), then for the object and DRCS pages the table and the page name. Pages already stored are not asked for again. These requests do not change which page is kept fresh.

**D27. Screen readers** get the page's text as the page area's label, besides egui's own description of the chrome.

**D18. Keys follow the concept's section 7,** with these differences:
- **`q` does not quit;** Cmd-Q does, as on every Mac application, so a stray key cannot close the browser.
- **Cmd-Left and Cmd-Right** go back and forward, as well as Cmd-[ and Cmd-].
- **Cmd-Shift-C** copies the page's text.
- **Clicking with the pointer:** a three-digit number goes there, a Fastext prompt is followed, and an `LT` run follows its link. Keyboard page catching is D25.
- **`.` for sub-page entry** is not built.

**D19. The window's fonts are the system's, read at start;** egui's bundled fonts are left out. Their licences (Ubuntu Font Licence, OFL) are not on REQ-GEN-017's list. Pages never use these fonts; they are drawn with the teletext glyph sets.

**D20. The menus are inside the window,** not in the macOS menu bar, which would need another library (design decision 3 lists one).

**D21. Saved files.**
- They live in `~/Library/Application Support/neoteletext`, in the formats of `neoteletext_browser::saved`, written whole by renaming a temporary file.
- History keeps 30 days (design decision 10). Confirmed sites are kept for the session only.
- Restored tabs reopen at their tuning screens.

**D22. Cookies live in each tab's fetcher, in memory.** Leaving a site in a tab ends that tab's session with it. Keeping cookies between runs stays off (REQ-SEC-013's default).

## 3. Libraries and licences

| Library | Version | Licence | Why |
|---|---|---|---|
| `eframe` and `egui` | 0.36.2 (8 September 2026) | MIT or Apache-2.0 | The toolkit the owner chose (design decision 2). Features: `wgpu`, `accesskit`, `x11`, `wayland`; no default fonts, no `links`. |
| `chrono` | 0.4.45 (4 June 2026) | MIT or Apache-2.0 | The local time for row 0's clock; the standard library has no time zones. |
| `egui_kittest` (tests only) | 0.36.2 | MIT or Apache-2.0 | Renders the whole window without a display for the window tests. |
| `image` (tests only) | 0.25.10 | MIT or Apache-2.0 | Saves the rendered window as PNG. |

egui's tree adds two crates whose licence, BSL-1.0, is not on REQ-GEN-017's list: `clipboard-win` and `error-code`, both used only on Windows. `deny.toml` now allows BSL-1.0 for those two crates only, in the same way as the two exceptions of IMPLEMENTATION-LOG.md row 32. This is for the owner to confirm, with issue #145.

## 4. Where each decision went in REQUIREMENTS.md

Recorded on 12 October 2026.

| Here | Part E | Requirements |
|---|---|---|
| D1, D5, D6, D15, D16, D19 to D22, D27, libraries | E.68 | REQ-BRW-001 to REQ-BRW-008, REQ-SEC-025 |
| D2, D3, D4 | E.69 | REQ-SEC-026 |
| D7, D8, D9, D10, D17, D24, D26 | E.63 | REQ-WEB-002, REQ-WEB-018 to REQ-WEB-020, REQ-WEB-022 |
| D11, D12, D13, D14 | E.65 | REQ-WEB-021, REQ-NAV-014, REQ-SEC-025, REQ-SEC-026 |
| D18 | E.67 | REQ-BRW-004 |
| D23 | E.64, amending E.36 | REQ-NAV-013, REQ-SEC-024, REQ-CLI-006, Appendix E |
| D25 | E.66 | REQ-NAV-015 |
| The concept's decisions 1 to 14 | E.63 to E.67 | as above |
| The tests | — | REQ-TST-081 |

The browser is milestone M7, Phase 1 on macOS and Linux and Phase 2 on Windows, Android and iOS (sections F.0.1 and F.0.2).

## 5. Not built yet

- **One decoder per site, and a back-forward cache** that keeps the previous site's processes.
- **The macOS menu bar, multiple windows, a file dialog, printing.**
- **The App Sandbox, Developer ID signing and notarisation, and Sparkle updates.**
- **Accessibility of the page itself** beyond its text.
- **The WebAssembly wall** (#160) and the phones (#159, #161).
