# Open questions raised by T42 Browser

Written on 15 October 2026.

*Note of 18 October 2026:* neoteletext's own browser was removed from this repository and is maintained in the separate neoteletext-browser repository (decision E.76). The questions below that compare T42 Browser with neoteletext's browser or its prototype documents are closed here; the questions about what the engine offers a browser, such as the web module's interfaces, remain open. The `prototypes/browser/` paths below name documents now in the neoteletext-browser repository.

T42 Browser (`~/devel/t42-browser`) is a teletext browser that uses neoteletext as its engine and replaces the egui shell with a Tauri shell. Its design is in three documents in that repository:

- `docs/architecture.md`, with its decision register (decisions 1 to 25);
- `docs/requirements.md`;
- `docs/security-analysis.md`.

This document lists what that design needs from neoteletext, or conflicts with, so that each point can be decided here and recorded in REQUIREMENTS.md Part E before any behaviour changes. It follows this project's rules: every point is written in full sentences, and terms are spelled out. Section 4 lists the questions for the owner in one place.

Nothing in this document changes neoteletext. Every item waits for the owner.

---

## 1. Conflicts with decisions already made in neoteletext

Each of these is a decision recorded in neoteletext that a T42 Browser decision contradicts. Each needs the owner to say which applies, and whether it applies to neoteletext's own browser, to T42 Browser only, or to both.

### 1.1 The interface toolkit

- **In neoteletext:** on 11 October 2026 the owner decided on one interface toolkit, egui, on all five platforms, with no Slint (`prototypes/browser/docs/desktop-browser.md` section 9.1; item 15 of `docs/spec/open-questions.md`).
- **In T42 Browser:** the interface is a HyperText Markup Language, Cascading Style Sheets and TypeScript application in a system webview, through Tauri version 2, with React as its framework (T42 decisions 1, 3 and 19). It runs in its own process with no network, no files and no secrets, and every security indicator and decision is in a small native strip drawn by a trusted core with no web engine (T42 decision 18).
- **Checked fact:** no neoteletext crate except `neoteletext-gui` depends on egui or any other interface toolkit (checked with `cargo tree` on 15 October 2026). The engine is unaffected by the choice.
- **Question:** is `neoteletext-gui` retired, kept as neoteletext's own viewer alongside T42 Browser, or kept only until T42 Browser replaces it? Does section 9.1 of `prototypes/browser/docs/desktop-browser.md` need amending?

### 1.2 Where the tuning screen is drawn

- **In neoteletext:** "the tuning screen stays inside the page area (owner's instruction)" (`docs/spec/open-questions.md` item 15; `prototypes/browser/docs/desktop-browser.md` section 8).
- **In T42 Browser:** the site-change screen, its name for the tuning screen, is shown only in the native trusted strip, never in the page area, so that no page image and no compromised interface can imitate it (T42 decision 24; T42 requirements 8.4 and 8.5).
- **Reason for the difference:** in T42 Browser the page area is drawn by an interface process treated as hostile, and pages are images that can show anything; only an area outside both can carry a confirmation that cannot be faked.
- **Question:** does the owner's instruction stand for neoteletext's own browser while T42 Browser does otherwise, or does T42 Browser's decision replace it? If the tuning screen's rules move into the trusted strip, `neoteletext-browser`'s model of the tuning screen (section 8 of `prototypes/browser/docs/desktop-browser.md`) must still drive it; only the drawing moves.

### 1.3 Fetching that depends on viewing

- **In neoteletext:**
  - a page is found in the store, or else fetched from its own file, `NNN.tti` then `NNN.t42`, when it is asked for (`prototypes/browser/docs/channels.md` section 3.3);
  - a page from the bundle that also has its own file is refreshed from its own file when viewed (`prototypes/browser/docs/web-native-concept.md`);
  - the Fastext targets and the next and previous pages are fetched ahead of time (REQ-NAV-004, `prototypes/browser/docs/web-native-concept.md`);
  - "viewers' browsers never crawl" (`prototypes/browser/docs/channels.md` section 4.1).
- **In T42 Browser:** fetching never depends on viewing (T42 decision 22). When a channel opens, its pages are fetched in an order fixed by the channel's own data: the whole bundle `index.t42`, or every page listed in `contents.txt`, in order. Pages are refreshed only by the channel's declared schedule or its push feed. Keys and interface requests only select among pages already held. Prefetching by viewing is not used. The one exception: a channel that publishes only separate page files, with no bundle and no contents file, has each page fetched when it is selected (T42 decision 23).
- **Reason for the difference:** the interface process in T42 Browser is treated as hostile and can see every open channel's pages. If its key presses could cause requests, it could encode one channel's pages in the pattern of requests it makes to another channel's server. Fetching that does not follow viewing leaves nothing to encode a message in. It also stops a channel's server from seeing which pages a viewer reads.
- **Questions:**
  1. Should this become a fetching policy in `neoteletext-web` and `neoteletext-browser`, switched on by T42 Browser, so that T42 Browser does not contain a second implementation of channel fetching? (T42 requirement 2.1 forbids a second implementation.)
  2. Should neoteletext's own browser use the same policy, for the privacy it gives?
  3. Fetching every page listed in a contents file when a channel opens is up to 800 requests at the existing rate of one request every 0.2 seconds (`DEFAULT_FETCH_INTERVAL_US`): about 160 seconds. Is that acceptable load for a hobby server, or should a contents-listed channel be fetched more slowly, or encouraged to publish a bundle?

### 1.4 Where the cookie store lives

- **In neoteletext today:** the fetcher process runs the web module's state machines, the cookie store included (`crates/neoteletext-app/src/fetch.rs`, REQ-SEC-018).
- **In neoteletext's browser design:** the interface process holds cookies (`prototypes/browser/docs/desktop-browser.md` section 3.1).
- **In T42 Browser:** the trusted core owns the cookie store. For each request the fetcher is given only the cookies that request carries, and returns any `Set-Cookie` headers to the trusted core, which applies them (T42 requirement 7.5). A compromised fetcher then sees only the cookies of requests in flight.
- **Question:** should `neoteletext-web`'s user agent be split so that the cookie store can live in a different process from the protocol state machines? This agrees with `prototypes/browser/docs/desktop-browser.md` but differs from the current code.

---

## 2. Changes T42 Browser needs in neoteletext

Each item is work in neoteletext that T42 Browser depends on. None is started.

### 2.1 The fetching policy of section 1.3

- **Where:** `neoteletext-web` (channel fetching) and `neoteletext-browser` (when a channel opens, refreshes and closes).
- **What:**
  - a mode in which a channel's pages are fetched in a fixed order: the bundle, or every page in `contents.txt` in its order;
  - refreshing only by the declared schedule or push feed;
  - no fetch and no prefetch caused by viewing, except fetching a selected page on a channel with only separate page files.
- **Verification T42 Browser requires:** a test in which arbitrary page selections, with arbitrary timing, leave the sequence and timing of requests reaching a test server unchanged, for bundled channels and channels with a contents file (T42 requirements 4.7 and 5.15 to 5.17).

### 2.2 A grace period after the last tab on a channel closes

- **Where:** `neoteletext-browser`, which T42 Browser reuses (T42 decision 24).
- **What:** when the last tab showing a channel closes, the channel keeps refreshing on its schedule for a fixed grace period, a named constant, before its decoder stops.
- **Reason:** otherwise the moment a tab closes is visible to the channel's server, and an interface that can close tabs could signal through it (T42 requirement 5.17).
- **Question:** the length of the grace period.

### 2.3 A limit on malformed packets in a live feed

- **Where:** the live input handling: `neoteletext-app/src/live.rs`, and the diagnostics that the readers of `neoteletext-formats` already report.
- **What:** a live feed that produces malformed packets beyond a fixed threshold, a named constant, is closed and reported.
- **Reason:** a hostile feed must not keep the decoder busy indefinitely (T42 requirement 6.5).
- **Question:** the threshold, set from measurements of real feeds. No such limit was found in the code on 15 October 2026.

### 2.4 A default refresh schedule

- **Where:** `neoteletext-web`, beside `MIN_REFRESH_SECONDS` and `MAX_REFRESH_SECONDS` in `descriptor.rs`.
- **What:** the refresh schedule used when a channel's `channel.json` declares none.
- **Question:** does one already exist under another name? None was found by name on 15 October 2026. If not, its value.

### 2.5 The decoder as a separate program, inside the WebAssembly interpreter

- **In neoteletext today:**
  - the fetcher is a second run of `neottx`, so a full build's decoder process still contains Transport Layer Security code (item 17 of `docs/spec/open-questions.md`);
  - decision E.69 item 5 records "no WebAssembly wall yet".
- **What T42 Browser needs:**
  - a decoder that runs the `neoteletext` decoder compiled to WebAssembly inside `wasmi`, with limits on memory, tables and fuel (T42 requirement 3.4);
  - a fetcher that contains no decoder code (T42 requirement 3.3);
  - the decoder messages of `prototypes/browser/docs/desktop-browser.md` section 3.4.

### 2.6 Sandboxes on macOS and Windows

- **In neoteletext today:** the fetcher and decoder are separate processes on macOS but are not sandboxed there. macOS and Windows sandboxes are Phase 2 (REQ-SEC-017; item 21 of `docs/spec/open-questions.md`). Decision E.69 item 2 uses `sandbox-exec` instead of services with entitlements.
- **What T42 Browser needs:** the fetcher and decoder sandboxed on macOS before T42 Browser's first release, as services with entitlements (T42 requirements 3.7 and 3.8). The same on Windows, with AppContainers.
- **Question:** whether services with entitlements replace `sandbox-exec` in neoteletext as well.

### 2.7 Page text for display and accessibility

- **What:** a function that gives a page's text, row by row, with control characters, bidirectional overrides and zero-width characters removed and the length bounded.
- **Uses in T42 Browser:** tab titles, row 0, link labels and the accessible label of each page image (T42 requirements 11.5 and 12.9).
- **Question:** does `neoteletext-core` or `neoteletext-browser` already provide this, for example for the text output that REQ-SEC-010 sanitises?

---

## 3. Limits T42 Browser will take from neoteletext

T42 Browser defines none of these values itself. It uses neoteletext's constants, so that each value is defined once (REQ-GEN-014).

| Limit | Constant in neoteletext | Value on 15 October 2026 |
|---|---|---|
| Pages in a contents file | `neoteletext_web::contents::MAX_PAGES` | 800 |
| Size of a contents file | `neoteletext_web::contents::MAX_BYTES` | 64 kilobytes |
| Size of a font file | `neoteletext_web::fonts::MAX_BYTES` | 64 kilobytes |
| Size of a page response | `DEFAULT_PAGE_LIMIT` in `neoteletext-web/src/consts.rs` | 256 kilobytes |
| Size of a push message | `DEFAULT_PUSH_LIMIT` | 64 kilobytes |
| Size of a channel descriptor | `DEFAULT_DESCRIPTOR_LIMIT` | 16 kilobytes |
| Size of response headers | `DEFAULT_HEADER_LIMIT` | 16 kilobytes |
| Size of a form submission | `DEFAULT_FORM_LIMIT` | 4 kilobytes |
| Interval between requests | `DEFAULT_FETCH_INTERVAL_US` | 0.2 seconds |
| Time to connect | `DEFAULT_CONNECT_TIMEOUT_US` | 10 seconds |
| Time to a response | `DEFAULT_RESPONSE_TIMEOUT_US` | 30 seconds |
| Shortest and longest declared refresh | `MIN_REFRESH_SECONDS`, `MAX_REFRESH_SECONDS` in `descriptor.rs` | 5 seconds and 24 hours |
| Pages in a zxnet walk | `ZXNET_MAX_PAGES` | 2,048 |
| Font cache | `neoteletext_browser::fonts::CACHE_LIMIT_BYTES` | 4 megabytes |

Not found, and needed (section 2): the malformed-packet threshold of a live feed, the default refresh schedule, and the grace period after the last tab on a channel closes.

---

## 4. Questions for the owner, in one list

1. Is `neoteletext-gui` retired, kept beside T42 Browser, or kept until T42 Browser replaces it? Is `prototypes/browser/docs/desktop-browser.md` section 9.1 amended? (Section 1.1)
2. Does the instruction that the tuning screen stays inside the page area still apply to neoteletext's own browser, now that T42 Browser draws it in a native trusted strip? (Section 1.2)
3. Should "fetching never depends on viewing" become a policy in `neoteletext-web` and `neoteletext-browser`, and should neoteletext's own browser use it too? (Section 1.3)
4. Is fetching up to 800 contents-listed pages at one request every 0.2 seconds, when a channel opens, acceptable load for a hobby server? (Section 1.3)
5. Should the cookie store be separable from the web protocol state machines, so that it can live in a trusted process? (Section 1.4)
6. The grace period after the last tab on a channel closes. (Section 2.2)
7. The malformed-packet threshold of a live feed. (Section 2.3)
8. The default refresh schedule for a channel that declares none, if one does not already exist. (Section 2.4)
9. Do services with entitlements replace `sandbox-exec` on macOS? (Section 2.6)
10. Is there already a function that gives a page's cleaned, bounded text? (Section 2.7)
