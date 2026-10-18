# The browser prototype: requirements and decisions

Moved out of the engine's REQUIREMENTS.md on 16 October 2026, when the repository was refocused on the core teletext rendering engine. The browser in this folder is a prototype built on the engine; it is not covered by the engine's gates. Each item keeps its original number, and REQUIREMENTS.md keeps a one-line entry for it so that the number is never reused and references to it still resolve. References here to other requirements, decisions and documents are to REQUIREMENTS.md and the engine's `docs/` unless they name this folder.

## 1. Decisions (from REQUIREMENTS.md Part E)

**E.63 — Web-native sites: what a site is, where its descriptor lives, and what its files may hold.**
1. **A site is a web origin that serves teletext** (REQ-WEB-015). *(build, D7)* The browser treats every origin, that is scheme, host and port, as its own site. The concept named the registrable domain; the origin is stricter, so more moves show the tuning screen (decision E.65), and it needs no list of public suffixes. This should be revisited if sites spread over several subdomains of one domain become common.
2. **The descriptor lives at `/.well-known/teletext.json`** (concept decision 13). A file at the site root, such as `index.json`, could collide with the site's own files. Typing the well-known address names the site itself.
3. **A site needs no descriptor.** *(build, D8)* When the well-known address answers 404 or 410, the site gets defaults, built as a descriptor so that REQ-WEB-002's checks apply to them: its name is the host, its initial page is 100, its pages are `/{mpp}.t42`, and its front door is `/index.t42`. Any other failure, such as a server error or an invalid descriptor, is reported and not replaced by the defaults, so that a broken site never silently changes meaning.
4. **Page files and a front door both work, with no setting** (concept decision 6). A typed number is looked for in the store, if the page is there and confirmed current (REQ-WEB-004), then in the page's own file, and is otherwise "not found".
   - *(build, D10)* A page's own file fills only pages that have been asked for, and enhancement pages, whose numbers are not decimal and which a page file may carry for its objects (item 5). Any other page in it is dropped by its header, so a page file cannot overwrite the rest of the site. The front door, pushed messages, form responses and local files fill every page they hold. `neottx`'s web mode follows the same rule, because it shares the code.
   - *(build, D9)* The front door is bounded by the page response limit of REQ-WEB-008, 256 kilobytes by default. A bundled site larger than that publishes page files instead.
   - *(build, D17)* A file or front door that does not hold the page asked for opens at its own first page, unless the viewer has already chosen a page.
5. **Enhancement dependencies are found both ways** (concept decision 7). A page's own file may carry the object and DRCS pages it uses. Otherwise, *(build, D26)* when a page is shown at Level 2.5 or 3.5, the decoder asks once for its magazine's Organization Table (page xFE), then for the object and DRCS pages that the table and the page name. Pages already stored are not asked for again, and these requests do not change which page is kept fresh.
6. **The descriptor gains aliases and a refresh interval** (concept decisions 3 and 13). A features list and a search page were considered and left out: nothing would read the features list, and a search page is an ordinary page that a site links to.
   - An **alias** maps one page number, never a range, to a target that is `name:mpp` or an address. *(build, D24)* A relative address is refused, an address in a descriptor fetched over `https` must itself be `https`, and a descriptor has at most 16 aliases.
   - **`refresh`** is a whole number of seconds. *(build, D24)* It is accepted from 5 to 86 400, and is used only when the service has no push.
7. **Sub-pages in addresses are not needed** (concept decision 5). An address leads to a page's file and shows its newest sub-page.

*Reason:* `prototypes/browser/docs/web-native-concept.md` section 2. A simple site is then a folder of files on any static host, and nothing needs declaring. The page-file rule keeps one page's file from rewriting another page of the site.
*Amended 12 October 2026 by decision E.70:* items 1, 2 and 3 are replaced. A site is now a channel, which is a folder; its descriptor is `channel.json` in that folder, not `/.well-known/teletext.json`; and its files have fixed names in the folder, with no page address template. Items 4 to 7 stand, applied to the channel's folder: the front door is the channel's `index.t42`, which every channel may have, with or without `channel.json`.

**E.64 — Links may lead to addresses; page numbers and runs of text may be links.**
This amends decision E.36 and Appendix E.
1. **Addresses in pages** (concept decision 1, the owner's). A Fastext key, a page number or a run of text may link to an address as well as to a named service, and no setting forbids it. A site that publishes other people's content removes their links before publishing, as on the ordinary web.
2. **One rule tells targets apart:** a target containing `://` is an address. *(build, D23)* An address must be `http` or `https`, printable ASCII without spaces or commas, and at most 256 bytes; anything else makes the link invalid. `weather:300`, which has no `//`, is always a named service.
3. **Two new TTI lines.** `LK,mpp,target` links a page number shown on the page, and `LT,row,column,length,target` links a run of text on rows 1 to 23. *(build, D23)* A page has at most eight of each. The TTI writer keeps all three link lines.
4. **Which link a typed number follows** (concept decision 9): the current page's own link for that number, otherwise the descriptor's alias for it, otherwise the site's own page with that number. The most specific link wins, so what the viewer sees on the page is what typing the number does. *(build, D23)* The decoder checks the page's own link; the browser's interface checks the alias when the decoder asks for the page.
5. **The page checker warns** when a page links a number that it does not show.
6. **Every link to another site goes through the tuning screen** (decision E.65). An address written in a page is marked there as not declared by the site.

What stays from decision E.36: the four coloured keys and Index remain the Fastext model, row 24 is drawn by the receiver from the link data, and pages never draw link markers. Text links look exactly as the author drew them until the viewer asks to see them (decision E.66).

*Reason:* `prototypes/browser/docs/web-native-concept.md` sections 1, 3 and 5. Sites without a descriptor can link elsewhere only by writing addresses, and the tuning screen, not a ban, is the safeguard.

**E.65 — Leaving a site: the tuning screen, and row 0 on every page.**
1. **The browser owns all of row 0 on web sites** (concept decision 2, the owner's). Header text in a web page file is ignored. *(build, D13)* In the browser, row 0 is drawn by the interface on every page, whatever its source: the top 20 pixel lines of every frame a decoder sends are replaced across their whole width.
   - **On web sites,** row 0 shows the receiver's status area, the site's name and the clock. The clock stays while the name fits in columns 8 to 31, and gives way when it does not (concept decision 10). A name too long for the row is shortened from the left and marked with `<`, so the end of the host, which names the site, always shows.
   - **For files and live feeds,** row 0 shows the status area and the broadcast's own header text, without the browser's clock.
   - **A decoder's reported page digits** are accepted only as three hexadecimal digits or `-`; anything else is shown as `---`.
2. **Row 0 shows what the keys are doing** (concept decision 12): `P` and the page digits while browsing, the digits so far while one is being typed, `FORM` while a form has the keys, `LINK` in page catching, and `GO?` on the tuning screen. A page over plain HTTP shows the red marker and `!` of REQ-SEC-005.
3. **The tuning screen** (concept decision 8). Before any page from another site is shown, the browser clears rows 1 to 24 and shows the destination's host in double height, the full address, whether the connection is encrypted, and how the viewer came to be going there, with row 0 showing `GO?` and the destination. It stays inside the page area, with row 0 above it (the owner's instruction of 11 October 2026). Only the receiver accepts the confirmation, so a fake screen drawn by a page leads nowhere.
4. **When it is shown** (concept decision 4, the owner's). *(build, D12)* It is shown for a typed address, including in a new tab; a bookmark or history entry; a restored tab; every link to another site; and every redirect to another site. It is skipped only when going back or forward to a site confirmed earlier in the session. A link to a site already confirmed still asks: the concept leaves this open, and asking is the safer reading.
5. **Redirects to another site are not followed.** *(build, D11)* In the browser the web module reports a redirect to another origin instead of following it, and the browser shows the tuning screen for the new destination. An upgrade from `http` to `https` on the same host is not another site. `neottx` keeps following redirects as REQ-WEB-003 says, and form submissions keep their existing rule, under which a redirect to another origin makes the submission cross-site for cookies.
6. **Only the interface confirms.** *(build, D14)* A form submission's confirmation is a bar outside the page that the viewer answers. A confirmation request from a decoder is dropped, never passed on.
7. **The receiver's own pages** *(build, D15)*, which are the start page, the keys page, the tuning screen, error pages and the loading page, go through the same Level 1 decoding as broadcast pages but are built in the interface and never pass through a decoder.

International domain names are shown in their ASCII form. Flagging names that imitate other domains (concept section 6) is not built; Appendix D.2 keeps it open.

*Reason:* `prototypes/browser/docs/web-native-concept.md` sections 4 and 6. A 40 by 25 page can draw anything in rows 1 to 24, including a fake tuning screen; row 0, which no page can write, is what makes the real one recognisable on every port. The rule for viewers is short: the site is whatever row 0 says.

**E.66 — Page catching and showing links.**
Concept decision 11 settled the model: the selected link has its colours swapped, its destination appears in row 0, a key underlines every link, and the pointer highlights links. *(build, D25)* The details:
- **Turning it on.** Enter turns page catching on at the first link in reading order. Links are the page numbers printed on rows 1 to 23 that lead somewhere, and the runs of `LT` lines.
- **Moving.** Right and Down move forward; Left and Up move back. Enter follows the selected link.
- **Turning it off.** Escape or Delete turns it off; any other key turns it off and then does what it does.
- **Row 0** shows `LINK` and the destination as the decoder reports it, written only after the receiver's `LINK`, so it can never be mistaken for the tuning screen's `GO?`.
- **Pages with side panels** are not caught, because their text columns are not the grid's.
- **The pointer.** A page number under the pointer names its destination in the window's status line, and a click follows it, as it follows a Fastext prompt or an `LT` run.
- **Show links.** `l` underlines every link with the receiver's own underline, and `l` again stops. The concept names no key; `l` was chosen as the letter for "links".

*Reason:* `prototypes/browser/docs/web-native-concept.md` section 5 item 2. The cursor stops only on real links, so text a page draws to look like a selected link can never be selected.

**E.67 — The browser's keys.**
The keys follow concept decision 14: Page Up and Page Down are Back and Forward, End is Index, Insert is Hold, Delete is Cancel, every function has a plain letter key, `K` shows the keys, and Escape never quits. *(build, D18)* The windowed browser differs as follows:
- **`q` does not quit;** Command-Q does, as in every macOS program, so a stray key cannot close the browser. `neottx` keeps `q`.
- **Command-Left and Command-Right** go back and forward, as do Command-[ and Command-].
- **Command-Shift-C** copies the page's text.
- **`G`** switches the glyph set (decision E.68).
- **`.` for sub-page entry** is not built.

*Reason:* `prototypes/browser/docs/web-native-concept.md` section 7, and the conventions of macOS programs.

**E.68 — The browser program.**
1. **One Rust program on egui for all five platforms** (the owner's decision of 11 October 2026, `prototypes/browser/docs/desktop-browser.md` section 15 decisions 2 and 9): Windows, macOS, Linux, Android and iOS. Slint is not used.
2. **All browser behaviour is in a crate with no operating-system access** (design decision 11, built in *(build, D5)*): `neoteletext-browser` holds address parsing, sites, history, the tuning screen's rules and page, row 0, the receiver's pages and the formats of saved files, and `xtask check-io` treats it as a library crate. `neoteletext-gui` holds the window, the tabs' processes and storage, and its program is called `neoteletext`.
3. **Processes.** *(build, D1)* Each tab has its own fetcher process and decoder process. One decoder per site, shared by tabs, is later work (design decision 1); the messages are the same either way. The interface routes every frame between them and reads all of it as untrusted.
4. **The decoder's loop** *(build, D6)* is a new module, `neoteletext_app::decoder`, ported from `neottx view`'s. Moving `neottx view` onto it is later work; the shared pieces were moved into `neoteletext-app` so that they are not duplicated.
5. **Files.** *(build, D16)* Files open from a typed path, a `file://` address, or by dropping them on the window. T42 files are read up to 256 megabytes and anything else as TTI up to 256 kilobytes. The other formats `neottx` reads are not opened by the browser yet, and there is no file dialog.
6. **Fonts and glyphs.** *(build, D19)* The window's own text uses the system's fonts, read at start; egui's bundled fonts are left out because their licences are not on REQ-GEN-017's list. Pages are always drawn with the teletext glyph sets, and the viewer can choose the project's own glyphs, Bedstead, or both side by side.
7. **Menus are inside the window** *(build, D20)*, not in the macOS menu bar, which would need another library.
8. **Saved data** (design decision 10, built in *(build, D21)*). Bookmarks, history, open tabs and settings are kept in the platform's application data directory, written only by the interface process, each written whole by renaming a temporary file. History keeps 30 days. Confirmed sites are kept for the session only, and restored tabs reopen at their tuning screens.
9. **Cookies** *(build, D22)* live in each tab's fetcher, in memory only. Leaving a site in a tab ends that tab's session with it, and saving cookies between runs stays off (REQ-SEC-013).
10. **Screen readers** *(build, D27)* are given the page's text as the page area's label, besides egui's own description of the window.
11. **Libraries** *(build)*. `eframe` and `egui` 0.36.2, with the features `wgpu`, `accesskit`, `x11` and `wayland` and without default fonts; `chrono` 0.4.45 for the local time of row 0's clock; and, for tests only, `egui_kittest` 0.36.2 and `image` 0.25.10. All are MIT or Apache-2.0. egui brings in two crates licensed BSL-1.0, `clipboard-win` and `error-code`, used only on Windows; `deny.toml` allows that licence for those two crates only. The owner is to confirm this exception (issue #145).

The design's other decisions (section 15: decisions 3, 4, 6, 7, 8, 12, 13 and 15 to 18) were recommendations still awaiting the owner (issue #150), and Appendix D.2 lists them.

*Reason:* `prototypes/browser/docs/desktop-browser.md`. Keeping every browser decision in a crate with no operating-system access lets one set of tests cover all five platforms, and keeps the shells thin.

**E.69 — Confining the browser's helper processes.**
1. **The browser runs privilege-separated web mode** (REQ-SEC-018) for every tab, with the decoder holding no rights and the fetcher holding only the network.
2. **On macOS the helpers are confined by Seatbelt profiles applied with `/usr/bin/sandbox-exec`.** *(build, D2)* Design decision 6 recommended XPC services, which need an app bundle with nested services and Objective-C or `unsafe` code, and only the C interface crate may contain `unsafe` code (REQ-SEC-015). `sandbox-exec` needs neither, works for both the bare program and the bundle, and confines the helper before its first instruction. The profiles deny everything, then allow only what was measured to be needed on macOS 15.6:
   - **the decoder:** running itself, reading the system libraries, its own file and the root directory, and reading `sysctl` values; no files, no network and no other programs;
   - **the fetcher:** the same, plus outgoing connections, name resolution through the system's resolver, and the resolver's files.

   `sandbox-exec` is deprecated by Apple but present and working on macOS 15.6. If it is removed, XPC services are the replacement.
3. **On Linux** each helper applies its own Landlock and seccomp sandbox, as `neottx` does (REQ-SEC-017).
4. **The interface process itself is not sandboxed.** *(build, D3)* The App Sandbox needs a signed bundle with entitlements, which is part of the release work and needs membership of the Apple Developer Program (design decision 8).
5. **The WebAssembly wall around the decoder** (design decision 12) **is not built.** *(build, D4)* On macOS the decoder already runs in a process with no file or network access. The interpreter is the only wall available on iOS; it is tracked in issue #160.

*Reason:* Security overrides every other requirement (decision E.32), and the helpers parse untrusted input. A test on every run proves the confinement, by trying to read a file, connect and start a program under each profile (REQ-SEC-026).

**E.73 — Discovery: an ordinary web address leads to a channel by pointing to it.**
Decided by the owner on 12 October 2026, who asked whether teletext pages could be carried inside ordinary web responses, in the HTTP headers or in the HTML `<head>`, and then asked for this design. This amends decision E.34 (REQ-WEB-014 handles one more response header, `Link`, and scans the start of an HTML response for one element) and decision E.70 item 5 (a folder that holds no channel leads to discovery instead of a failure). `prototypes/browser/docs/web-discovery.md` is the design and `prototypes/browser/docs/web-discovery-security.md` its security analysis.

1. **Web addresses point; they never carry pages.** A web page announces a teletext edition with a pointer to a channel, or to a page of one, and the pages still arrive as a channel's files (REQ-WEB-019), through every check they pass today (REQ-WEB-027). These were considered and refused:
   - *Pages in an HTTP header.* A page is about 1 kilobyte as T42 and 1 to 3 kilobytes as TTI, more with sub-pages; proxies and content delivery networks commonly limit all of a response's headers to 4 to 8 kilobytes, static hosts such as GitHub Pages cannot set a header per file, and every ordinary visitor would download the page without using it.
   - *Pages embedded in the HTML,* in a data block such as `<script type="text/x-teletext-tti">`. It would need an HTML reader that faces the network, the page would belong to no channel and so have no place for its cookies, forms and links, and its two copies, the HTML and the teletext, would drift apart.
   - *`HEAD` requests.* A `HEAD` response has only headers, so it can carry only a `Link` header, which a `GET` brings in the same round trip. Servers often answer `HEAD` differently from `GET`, and hosts that cannot set headers can only point from the HTML, which `HEAD` never returns.
   - *A DNS TXT record,* which the transport of REQ-WEB-001 cannot look up, and *a file under `/.well-known/`,* which belongs to the root of a server (decision E.70 item 7) and would add a second mechanism for the same thing.
2. **Discovery runs only after the viewer has confirmed the address,** on the tuning screen as today (REQ-NAV-014), never before, and never for pre-fetching (REQ-SEC-013). It runs for:
   - a folder address that holds no channel: `channel.json` answers 404 or 410, or with a redirect that leaves the folder, or with an HTML page; and the bundle and the asked-for page are missing in the same way. The four requests of E.70 item 5 are made first, unchanged;
   - an address that is not a channel's: a file name that is not one of a channel's (`story.html`), or an address with a query.

   It never runs for a channel's own file names, which are explicit, and never for the address a pointer leads to: there is one step, never a chain.
3. **One request, with the redirects it needs.** The browser asks for the address with `GET`, `Accept: text/x-teletext-tti, application/x-teletext-t42, text/html;q=0.5`, no cookies and no `Referer`, and stores no cookie from the answer. It follows at most 5 redirects, to any origin, each checked for the local network and for downgrade. It reads the headers and at most the first 8,192 bytes of the body, then closes the connection.
4. **Three ways to point, tried in this order:**
   1. *Content negotiation through a redirect.* When the final address is a page file of a channel and the answer is a page in one of the accepted media types, that is the page. A dynamic server that sees the teletext types first in `Accept` answers `303 See Other` with the page file's address. A teletext answer at any other address is refused: pages live in channels.
   2. *The `Link` header* of RFC 8288, with `rel="alternate"` and one of the two teletext media types as its `type`.
   3. *A `<link>` element* with the same `rel` and `type`, within the first 8,192 bytes of an HTML answer. HTML already requires `<meta charset>` within its first 1,024 bytes for the same reason: a reader can decide from a fixed prefix. Measured on 12 October 2026, the head of the BBC News home page is 198,533 bytes long and the Guardian's 741,595, with its alternate link at byte 741,499; a pointer is therefore looked for only where the publisher has been told to put it, not wherever a head may end.

   The first pointer that passes REQ-WEB-030 wins; the others are ignored.
5. **What a pointer may lead to:** a channel's folder or one of its files, as the address bar takes them (REQ-WEB-023, REQ-BRW-002), and nothing else. A pointer from an `https` answer to an `http` target is refused (no downgrade, REQ-SEC-007); a target in the local network is refused unless the configuration allows it (REQ-SEC-006). An address holding `&`, a byte outside printable ASCII, or anything the strict parser refuses is refused rather than decoded, so no HTML character references are ever read. A relative address is resolved against the final address of the answer, and refused if a `<base>` element comes before it.
6. **The scan is not an HTML parser.** It reads bytes, skips comments and the contents of `script`, `style`, `noscript`, `template` and `title`, reads the attributes of `link` elements, and stops at `<body`, `</head>`, the 8,192-byte limit, or the first byte it cannot read exactly. It runs in the tab's confined fetcher, and the interface receives only the address it found, which it parses again. Nothing of the HTML is shown, stored or used for anything else.
7. **The tuning screen names what was found.** When the target is on the same origin as the address the viewer confirmed, and in its folder or one inside it, it opens at once: the viewer has already agreed to go there. Otherwise the tuning screen is shown again, giving the target and the address that announced it, with the reason "announced by". A redirect to another origin during discovery, such as `example.org` to `www.example.org`, therefore costs the viewer a second key press; that is deliberate.
8. **The author's tool.** `neottx discover ADDRESS` makes the same request and reports the pointer it finds, every candidate it ignored and why, and where in the answer each was, so a publisher can test a page.
9. **Phases.** As the browser: Phase 1 on macOS and Linux, Phase 2 on Windows, Android and iOS. `neottx discover` is Phase 1.

*Reason:* The niche format's main obstacle is being found. One line in a web page's head, which every host can serve, makes an ordinary address lead to its teletext edition, per site or per story, while pages stay files in channels and every rule of decision E.70 is unchanged. Pointing, rather than carrying, costs a small, bounded scanner instead of an HTML reader, and keeps the web page and the teletext page each in one place. `rel="alternate"` is how web pages already announce their feeds and translations.

**E.74 — The font browser: choosing the user's font by moving through a list.**
Decided by the owner on 13 October 2026, who asked for a panel listing every font, to be arrowed through or clicked to choose the user's default font, and who ruled that the user's font is used only where neither the site nor the page names one. This adds to decision E.72 and changes no rule of area FNT.

1. **A view of the libraries, not a new source.** The panel lists the fonts of the libraries the user has added (REQ-FNT-004), in each library's order, below a first entry for Bedstead. It fetches nothing to list them beyond each library's index, as the Fonts settings do.
2. **Moving is choosing.** Moving to an entry with the up and down arrow keys, or clicking it, chooses that font as the user's font exactly as choosing it in the Fonts settings does: it is fetched and read in a confined process (REQ-FNT-003), cached (REQ-FNT-009), saved once it has loaded, and the current page is drawn again in it (REQ-FNT-008). Choosing an entry while another is still loading makes the newer one the choice; the older one still reaches the cache. There is no separate preview: a preview would need a font that is in use without being chosen, which REQ-FNT-003 does not allow.
3. **The order of REQ-FNT-005 is kept.** The user's font draws a page only when neither the page nor the channel names one, unless the user has set "always use my font". When the page's or the channel's font is in use, the panel says so, so that moving through the list without a visible change is not mistaken for a fault.
4. **The keyboard.** While the panel has the keyboard, the up and down arrow keys move through the list instead of changing page; clicking the page or pressing Esc gives the keys back to the page. Left and right keep their meaning.
5. **Its place.** The panel is on the left, so it can be open with the side panel of bookmarks, history and fonts on the right. It is turned on and off from the menu, and whether it is on is saved with the settings.
6. **Duplicates** *(added 13 October 2026 by the owner, issue #207)*. A library may list the same font more than once under different names; in the owner's library 34 of 148 fonts draw exactly as another does. When a font loads, the interface computes the SHA-256 of its checked glyph table, each code point and its ten rows in code-point order, so that names, comments and licence lines do not count, and keeps it in the cache's index. A "Show duplicates" setting, off by default, decides whether a listed font whose digest equals that of a font listed before it is shown. The user's chosen font is always shown. A font not yet loaded has no digest and stays listed: finding duplicates before loading would mean fetching every font of a library, which REQ-FNT-004 forbids, so duplicates disappear as the user browses and stay hidden afterwards. A digest in the library's index was considered and not chosen.
7. **The sample** *(added 13 October 2026 by the owner, issue #208)*. A "Show sample" setting, off by default, replaces the page area, while the panel is open, with a sample of the chosen font: every glyph in code-point order, forty to a row on alternating cell backgrounds so that each glyph's extent shows, then, where the twenty-five rows leave room, sample text: the English pangram in both cases, digits and signs, and Cyrillic, Greek and Hebrew pangrams when the font draws most of their letters, with a missing character drawn as the dimmed placeholder (REQ-CHR-004). The interface draws it from the checked glyph table, as it already receives it (`docs/font-security.md` section 3.1), not through a decoder: a loaded font may hold glyphs no teletext character set reaches, so a teletext page could not show them all. Bedstead's sample is its own glyphs. The sample is no page: clicking it follows nothing.

*Reason:* With many fonts, choosing one at a time by title and then looking at the page is slow; moving through a list while the page changes shows each font on real pages. Keeping the order of REQ-FNT-005 means a channel's or page's own font is never displaced by browsing.

## 2. Area BRW (formerly REQUIREMENTS.md section F.20)

This area is the windowed browser, `neoteletext`, designed in `prototypes/browser/docs/desktop-browser.md`, `prototypes/browser/docs/web-native-concept.md` and `prototypes/browser/docs/channels.md`, and recorded in decisions E.63 to E.70. Its security requirements are REQ-SEC-025 and REQ-SEC-026, and its navigation requirements REQ-NAV-013 to REQ-NAV-015.

Every requirement in this area carries the same phase: Phase 1 on macOS and Linux, and Phase 2 on Windows, Android and iOS (section F.0.1).

**REQ-BRW-001 — One program, with its behaviour in a platform-free crate.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser shall be one Rust program built on egui for every platform (decision E.68 item 1). Every decision about what the browser shows and does (addresses, channels, history, the tuning screen, row 0, the receiver's own pages, the contents view and the formats of saved files) shall be made in `neoteletext-browser`, a library crate with no operating-system access (REQ-SEC-016). `neoteletext-gui` holds only the window, the helper processes and storage. The program shall also be its own helpers: started with a hidden command, it runs as a tab's fetcher or decoder (REQ-SEC-026).
*Verification:* Inspection; `xtask check-io` classes `neoteletext-browser` as a library crate.

**REQ-BRW-002 — Tabs and the address bar.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser window shall hold any number of tabs. Each tab has its own location, history, helper processes and cookies. The window shall have a tab strip; a toolbar with back, forward, reload and home buttons, the connection's security, the address bar and buttons for the four coloured keys; a menu with bookmarks, history, copying and settings; and a status line.

The address bar shall accept:
- **a channel's folder:** `https://host/`, `https://host/ceefax/`, or a bare `host` or `host/ceefax/`, which mean `https`. The channel opens at its initial page (REQ-WEB-018);
- **a folder without its trailing slash:** `https://host/ceefax` is the folder `ceefax/` when the last segment contains no `.`. A last segment with a `.` is a file name, and must be one of the names below;
- **a channel's file:** `https://host/ceefax/140.tti` or `140.t42` is that page of that channel; `index.t42` and `channel.json` are the channel at its initial page; `contents.txt` is the channel's contents view (REQ-BRW-009);
- **a channel and a page number:** `host/ceefax 140`, a folder address, a space and three digits;
- **a descriptor service:** `https://host/path/name.json`, for any file name other than `channel.json`, is the service that descriptor names (REQ-WEB-018);
- `zxnet:name`: a known zxnet live feed (REQ-INP-022);
- `file:///path` or `/path`: a local file (REQ-BRW-005);
- **an ordinary web address:** any other `http` or `https` address, such as `https://host/news/story.html` or one with a query, whose teletext edition is found by discovery (REQ-WEB-028). A folder address that holds no channel is also given to discovery.

Every address shall pass the strict parser of REQ-WEB-016; channel addresses shall also pass the channel rules of REQ-WEB-023, and only `http` and `https` lead to channels or discovery. A page number must be three digits of a real page. While a page is shown, the address bar shows the address of its page file, in the extension that answered, which typed back leads to the same page.
*Amended 12 October 2026* (decision E.73): ordinary web addresses lead to discovery, where any other file name was refused.
*Verification:* Test of each accepted form and of rejected ones; a window test.

**REQ-BRW-003 — Each tab's history.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
Each tab shall keep a history of the pages it has shown, across channels, of at most 100 entries, the oldest dropped first. Each entry records the location, the page and a title from the page's first line of text. A new page after going back drops the entries ahead of it. Going back or forward to another channel shows the tuning screen unless that channel was confirmed earlier in the session (REQ-NAV-014).
*Verification:* Test of back, forward, the dropped entries and the case of another channel.

**REQ-BRW-004 — Keys.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser's keys shall follow decision E.67:

| Function | Letter | Other keys |
|---|---|---|
| Page number | `0` to `9` | |
| Coloured keys | `r`, `g`, `y`, `c` | F1 to F4 |
| Index | `i` | End |
| Initial page | `h` | Home |
| Next and previous page | `+`, `-` | Up, Down |
| Sub-pages | | Right, Left |
| Back and forward | Backspace, `f` | Page Up, Page Down; Command-Left, Command-Right; Command-[, Command-] |
| Hold | `H` | Insert |
| Reveal, mix, zoom | `?`, `m`, `z` | `*`, `/` |
| Reload | `u` | F5, Command-R |
| Page catching, show links | Enter, `l` | |
| Into the page's form | Tab | |
| Confirm | Enter | F4 |
| Cancel | Escape | Delete |
| Address bar | | Command-L, F6 |
| New tab, close tab, next tab | | Command-T, Command-W, Control-Tab |
| Bookmark | | Command-D |
| Copy the page's text | | Command-Shift-C |
| Glyph set | `G` | |
| The keys page | `K` | |
| The channel's contents | `C` | |
| Quit | | Command-Q |

Escape never quits. `K` shows a page, drawn by the receiver, that lists the letter for each function first, each line fitting the 40 columns and using no character that differs between national options. While a form has the keys, typing is text, and F1 to F4 remain the coloured keys.
*Verification:* A test that every line of the keys page fits; window tests of the main keys.

**REQ-BRW-005 — Files.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser shall open a local file given in the address bar or dropped on the window (decision E.68 item 5). A file whose name ends in `.t42` shall be read as T42 packets, up to 256 megabytes; any other file as TTI, up to 256 kilobytes. Every page a file holds shall be stored, and the file opens at the page asked for or, failing that, its first page (REQ-WEB-019). A file is its own channel for the tuning screen.
*Verification:* Test of opening each format by path; test of the first-page rule.

**REQ-BRW-006 — Saved data.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser shall keep bookmarks, history, the open tabs and its settings between runs, in the platform's application data directory: Application Support on macOS, `$XDG_DATA_HOME` or `~/.local/share` on Linux, `%APPDATA%` on Windows (decision E.68 item 8). Only the interface process shall write them. Each file shall be written whole, by writing a temporary file and renaming it.
- History shall keep 30 days.
- Confirmed channels shall not be saved.
- Restored tabs shall reopen at their tuning screens.
- Cookies shall not be saved (REQ-SEC-013).

Reading shall be strict and bounded, because the files could have been changed by anything: at most 4 megabytes per file and 10 000 records, and every stored address read back through the address bar's parser, so a stored address can never mean more than a typed one.
*Verification:* Tests of writing and reading back, of the history limit, and of malformed files; a window test that a bookmark is kept and listed on a new tab's start page.

**REQ-BRW-007 — Fonts and the window's own text.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
Pages shall be drawn with teletext fonts only: Bedstead, and the fonts of area FNT. The settings shall let the user choose a font for each character set (REQ-FNT-005), by address, from a font library (REQ-FNT-004) or from a local file; set "always use my fonts" (REQ-FNT-005); allow or refuse fonts from other sites (REQ-FNT-003), allowed by default; and see, pin and remove cached fonts (REQ-FNT-009). These settings apply to every tab and are saved. The window's own text shall use the platform's fonts, read at start, and no bundled font whose licence is not on REQ-GEN-017's list.
*Amended 12 October 2026* (decision E.72): the switch between the original glyphs and Bedstead is replaced by the fonts of area FNT. *Amended 13 October 2026* (decision E.75): a font per character set.
*Verification:* Window tests of choosing a font for a character set and of the override.

**REQ-BRW-008 — Accessibility.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The window's controls shall be described to the platform's screen reader. The page area shall be labelled with the page's text, as `neottx text` gives it. The page's text shall be copyable. The rest of `prototypes/browser/docs/web-native-concept.md` section 11 (navigation row by row and link by link, and link descriptions) is not yet required.
*Verification:* Inspection with the platform's screen reader.

**REQ-BRW-009 — The contents view.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser shall show a channel's contents (REQ-WEB-025) as teletext screens drawn by the receiver (decision E.70 item 8).
- **Opening it.** The `C` key, the menu's "Contents" item, and the address of the channel's `contents.txt` open the contents view of the current channel. It replaces the page area until the viewer leaves it.
- **Fetching.** `contents.txt` shall be asked for once in each visit to the channel, the first time the view is opened; opening the view again in the same visit shows the same contents. It is fetched by the tab's fetcher and read by the interface; it never passes through a decoder, and its screens are built in the interface process like the receiver's other pages (REQ-SEC-025).
- **Row 0** shows `LIST` in the status area, then the channel's host and folder and the clock, as REQ-SEC-025 says.
- **Rows 1 and 2** show "Contents" in double height, with the screen number and the number of screens, such as `1/3`, at the right.
- **Rows 4 to 23** show twenty entries per screen, in the file's order:
  - **a section heading** in yellow, from column 1. A heading that would fall on row 23 starts the next screen instead, so that a heading is never separated from its first entry;
  - **a page** in white: column 1 holds `*` when the entry's date is later than the viewer's previous visit to this channel recorded in the history, and a space otherwise; its title starts at column 2; dots fill the row up to column 35; and the page number is in columns 37 to 39;
  - **another channel** in cyan: column 1 holds the receiver's mark for leaving, the right guillemet `»` from the G2 set; the title starts at column 2; and the page number, when the target names one, is in columns 37 to 39.
- **Row 24** is drawn by the receiver: "Enter go   Left Right more   Esc back".
- **Keys.** Up and Down move a selection, drawn with swapped colours as in page catching (REQ-NAV-015); Enter follows the selected entry; Left and Right show the previous and next screen; digits type a page number as on any page; a pointer click follows the entry under it; and Escape or Delete returns to the page shown before.
- **Following an entry.** A page entry is found as any page of the channel (REQ-WEB-019). An entry for another channel goes through the tuning screen (REQ-NAV-014), as a link on this channel, marked as declared when its target is a `name:mpp` that `channel.json` names.
- **Without a contents file.** When `contents.txt` answers 404 or 410, the view shall list instead the pages of this channel the tab has shown, in page number order, each titled as its history entry is (REQ-BRW-003), cut to 32 characters, with row 3 saying "No contents file: pages received so far". When `contents.txt` fails in any other way, row 3 shall say that the contents could not be used and why, and the same list follows.
- **Faults.** The number of lines `contents.txt` had that were ignored (REQ-WEB-025) is shown in the window's status line, never on the page.
- **Text.** Titles are drawn by the receiver with the characters every national option shows the same; other characters are drawn as `?`.
*Verification:* Tests of the screens drawn from a contents file with each kind of entry, a section on the last row, more than one screen and a dated entry; of each key; of the list drawn without a contents file; and a window test.

**REQ-BRW-010 — The font browser.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The menu shall have a "Font browser" item that shows or hides a panel on the left of the window, saved with the settings (decision E.74). The panel shall be organised as EN 300 706 organises character sets (decision E.75 item 4):
- **At its top**, the current page's designation (REQ-BRW-011).
- **Then the entries of Table 32 by group**, each group with its four-bit value and a name for its region, each entry with its national option, its G0 set and the G2 set it designates. The groups and entries containing the page's sets are open and marked.
- **Under each set**, "Bedstead (bundled)", and then the fonts of the user's libraries (REQ-FNT-004) that conform for that set (REQ-FNT-011), by title, with the font chosen for the set marked (REQ-FNT-005). A set appears under every entry that names it; its fonts and its choice are the same wherever it appears. G1 mosaics, G3 characters and DRCS characters are listed as drawn by the receiver, with no fonts.
- **Choosing.** Clicking a font under a set, or moving to it with the up and down arrow keys while the panel has the keyboard, chooses it as the user's font for that set; "Bedstead (bundled)" clears the set's choice. The arrow keys move through the fonts of the set last chosen in. Clicking the page or pressing Esc shall return the arrow keys to the page. When the page's or the channel's font draws the page (REQ-FNT-005), the panel shall say so. With no library added, the panel shall say how to add one.
- **Conformance.** A "Conformant only" setting, on by default and saved, lists under a set only the fonts that conform for it; off, every font that draws any of the set's characters is listed with its count of positions, such as `85/96`. A "SAA5050 cell" setting, off by default and saved, lists only fonts that also meet the profile. Before a font is loaded, its conformance is the library's `c` line; once loaded, the conformance computed from its glyph table.
- **Duplicates.** A "Show duplicates" setting, off by default and saved, hides under each set every font whose glyph table, once loaded, has the same SHA-256 as that of a font listed before it, except the font chosen for the set; the arrow keys move through the fonts shown, and the panel says how many are hidden (decision E.74 item 6).
- **The grid.** A "Show character set" setting, off by default and saved, replaces the page area while the panel is open with the 96-position grid of the set last chosen in, as the standard draws its tables: columns 2 to 7, rows 0 to F, each position drawn in the font chosen for the set, or the font under the arrow, with rounding as REQ-FNT-008 draws it. Beside it is the grid of the set the same Table 32 entry pairs with it. A position the font lacks is marked as a hole; a position the standard leaves unassigned is marked differently; the 13 positions of a Latin national option sub-set are marked. Each grid's heading gives its set and its count of positions drawn, such as `96/96`. Where rows remain, sample text drawn in the font follows.
*Amended 13 October 2026* (issue #207): duplicates hidden. (Issue #208): the sample. (Decision E.75, issue #215): organised by Table 32 group, entry and set, conformance, and the grid in place of the sample.
*Verification:* Window tests that show the panel, open the page's group and entry, choose a font for a set with a click and with the arrow keys and check the saved choice and that the page follows it, hide a non-conformant font and show it with its count, hide a duplicate, show the grid with a hole, an unassigned position and the national option positions, and clear a set's choice with Bedstead; unit tests of the grid's layout.

**REQ-BRW-011 — The page's designation.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The font browser shall show how the current page's character sets were chosen, as the decoder resolved them (decision E.75 item 5): the national option bits C12 to C14 of the page header; the four-bit group and where it came from, the page's packet X/28/0 Format 1 or X/28/4, its magazine's packet M/29/0 or M/29/4, or the receiver's setting when the page sends none; the Table 32 entry, with the fallback of REQ-CHR-003 when the entry is reserved; the second G0 set and national option of Table 33, if any; the number of rows with ESC toggles; and the Modified G0 and G2 Character Set Designation triplets of X/26 (Table 34). It shall then name every character set the page uses. The decoder shall send this with the page's status as a bounded report; the interface shall draw it as its own text, outside the page.
*Verification:* Decoder tests of the report for a Level 1 page, a page with X/28/0 Format 1, a magazine with M/29/0, a reserved entry, a second G0 set, ESC and an X/26 designation; a window test that the panel shows them.

## 3. Requirements from other areas

**REQ-SEC-025 — The browser draws row 0 on every page.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
In the browser, all of row 0 belongs to the receiver on every page, not only columns 0 to 7 (decision E.65 items 1 and 2). The interface shall replace row 0 of every frame a decoder sends, across its whole width, with the row it draws itself:
- columns 0 to 6, the status area of REQ-SEC-005, showing the activity: the page digits, `FORM`, `LINK` or `GO?`, and the red marker for plain HTTP;
- on channels, the channel's host and folder from the location the interface holds, never from the page or from `channel.json` (decision E.70 item 13). The name is the host, then the folder path without its last `/`, such as `user.github.io/ceefax`; the root folder adds nothing. The clock is shown in columns 32 to 39 while the name fits in columns 8 to 31. A name too long for columns 8 to 39 is shortened: the folder's last segment is kept whole, with its own start cut and marked `<` if it alone is longer than 20 characters; segments between the host and the last are replaced by `/.../`; and the host is cut from the left, marked `<`, so that its end shows;
- for files and live feeds, the broadcast's own header text.

Page digits reported by a decoder shall be shown only when they are three hexadecimal digits or `-`, and as `---` otherwise. The pages the receiver draws itself (the start page, the keys page, the tuning screen, error and loading pages) shall be built in the interface process and never pass through a decoder.
*Reason:* Row 0 is how the viewer knows which site they are on, and what the keys are doing. A decoder handling a hostile page must not be able to write it.
*Verification:* Tests in which a decoder sends a forged row 0 and a hostile site draws a fake header and a fake tuning screen; the row shown must be the receiver's.

**REQ-NAV-013 — Links to addresses, page numbers and runs of text.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
A link target may be a page of the current service, a page of a named service, or an address (Appendix E.1, decision E.64). The browser shall follow:
- **coloured keys and Index** to any of these targets;
- **page-number links** (`LK`): typing that number, or choosing the number printed on the page, while the page is shown;
- **text links** (`LT`): choosing the linked run of text.

A typed number shall follow the current page's own link for it, otherwise the channel's alias for it (REQ-WEB-020), otherwise the current channel's page with that number. A link to a page of the current channel is followed at once. A link to another channel, whether by a named service or an address, shall go through the tuning screen (REQ-NAV-014); an address in the current channel's own folder is a page of the current channel (REQ-WEB-026). A link to an ordinary web address goes through the tuning screen like any other, and then to discovery (REQ-WEB-028). `neottx view` keeps REQ-NAV-012 for named services, and does not yet follow addresses, page-number links or text links.
*Verification:* Test of each link source and target form, and of the order of page link, alias and channel page.

**REQ-NAV-014 — The tuning screen.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
Before the browser shows anything from another channel (REQ-WEB-018), it shall clear rows 1 to 24 and show the tuning screen (decision E.65 item 3). Another folder on the same origin is another channel. The screen shall show:
- the destination's host, in double height, shortened from the left so that its end always shows, and the destination's folder on the row below it;
- the channel's folder address, followed by the page number when there is one, wrapped over up to four rows, with a mark when it is cut. Which of a page's two files answers is not known until it is fetched, so the page is given by its number;
- whether the connection is encrypted; a plain HTTP destination is stated in red;
- why the browser is going there: a typed address, a choice from bookmarks or history, a restored tab, a redirect and the channel that sent it, a link and the channel it is on, or a pointer and the web address that announced it (REQ-WEB-028), with the address the viewer asked for when it differs. For a link, the name the channel's `channel.json` gives the destination, or that the address was written in the page and not declared. An ordinary web address, before discovery, is shown as such, with the note that the browser will look for its teletext edition;
- the keys: Enter to go, and Escape or Delete to stay.

Row 0 shall show `GO?` and the destination. The screen shall be shown for a typed address, a bookmark or history entry, a restored tab, a link to another channel, and a redirect to another channel. It shall be skipped only when going back or forward to a channel confirmed earlier in the same session, and when discovery leads to a channel on the same origin as the address the viewer has just confirmed, in that address's folder or a folder inside it (decision E.73 item 7). For an address that is not a folder, its folder is its path up to and including the last `/`. Confirmed channels shall not be remembered between runs. Only the viewer's own key press confirms; nothing a page or a decoder sends can.
*Verification:* Test of each reason for the screen, of the skipped case, and that a page drawing a copy of the screen cannot confirm anything (the hostile test channel of REQ-TST-081).

**REQ-NAV-015 — Page catching and showing links.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The viewer shall offer page catching (decision E.66):
- Enter, when nothing else uses it, turns page catching on at the first link in reading order. The links are the page numbers on rows 1 to 23 that lead to a page, and the runs of `LT` lines.
- Right and Down move to the next link, Left and Up to the previous one, and Enter follows the selected link.
- Escape or Delete turns page catching off. Any other key turns it off and then does what it does.
- The selected link is shown with its foreground and background colours swapped, drawn by the receiver. Row 0 shows `LINK` and the selected link's destination.
- Pages with side panels are not caught.

A show-links key, `l`, shall underline every link on the page with the receiver's own underline until it is pressed again. With a pointer, the destination of a page number under the pointer shall be shown outside the page, and a click shall follow it.
*Verification:* Test of turning on, moving, following and turning off; test that only real links can be selected.

**REQ-CLI-009 — Testing a web page's pointer.**
`neottx discover ADDRESS` shall make the discovery request of REQ-WEB-028 for an address, and print to standard output:
- the address asked for, every redirect, and the final address;
- the result: the target found, how it was found, and the channel it leads to; or that nothing was found;
- every candidate that was not used, with where it was (the header field, or the byte offset in the body) and why: wrong `rel` or `type`, an `anchor` parameter, past the 8,192-byte limit or after `<body` or `</head>`, inside a comment or a script, or a target refused by REQ-WEB-030 with the rule;
- whether the body held a `<link rel="alternate">` for a teletext type beyond the first 8,192 bytes, which the browser would not see, found by reading on to at most 1 megabyte for this report only.

It shall not open the channel, and shall show nothing of the HTML beyond the candidates' attribute and parameter values. Every value taken from the answer, including addresses, header values and attribute values, shall be printed by the allow-list of REQ-SEC-010: printable ASCII as it is, and every other byte as `\xHH`, cut to 200 characters with a mark. It needs only the network: it applies the sandbox of REQ-SEC-017 with network access and no file access before it connects, and runs the scan in the same confined process. Exit status (REQ-CLI-005) is 0 when a target was found, 1 when none was, 2 for a wrong argument, and 3 when the address could not be reached or its answer could not be read.
*Reason:* A publisher must be able to see why the browser does or does not find their pointer.
*Verification:* Tests against the local test server for each kind of candidate.

**REQ-TST-081 — End-to-end tests of the browser against local channels.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
The browser shall be tested end to end, with its real helper processes, against local test channels served on 127.0.0.1. The server is test code (`tests/web-server/sites.py`), and generates every file on request, which also shows that a dynamic server and a static one look the same to the browser (decision E.70 item 10). It shall serve:
- **a news channel** at the root of its own port, with `channel.json`, `contents.txt`, TTI pages, aliases, `LN`, `LK` and `LT` links to other channels, and a form;
- **a weather channel** without `channel.json`, with `index.t42` and T42 page files;
- **two channels in two folders of one port,** `/one/` and `/two/`, each with its own pages and cookies, and links and a redirect from one to the other;
- **a channel served with the generic media types** `text/plain` and `application/octet-stream`, and one that answers a missing file with an HTML page and status 200;
- **a redirecting channel,** whose `channel.json` redirects to another channel;
- **a hostile channel,** whose pages try to forge row 0 and the tuning screen, and one of whose responses never ends;
- **addresses with path tricks:** `%2F`, `..`, doubled slashes, and `/ab/` against `/a/`;
- **web pages for discovery** (REQ-WEB-028 to REQ-WEB-030): a page with a `Link` header; a page with a `<link>` early in its head; a page whose only `<link>` is past 8,192 bytes, and one inside a comment and a script; a root folder that answers every file with an HTML page and status 200; a redirect to another port before the pointer; a program that answers `303 See Other` to a page file when `Accept` puts teletext first, and one that answers teletext at a web address; pointers that are relative, relative after `<base>`, `&amp;`-encoded, to `http` from `https`, to 127.0.0.1, and to an address that is not a channel; and a page whose pointer leads to a missing channel.

The tests shall drive the browser's tabs through the same code the window uses, without a display. They shall cover typed addresses in every form of REQ-BRW-002, history, links, aliases, page catching, the contents view, redirects, forms, cookies kept apart between two channels on one origin, files, the hostile channel, the path tricks, the media types, discovery and its tuning screens, and the helpers' confinement (REQ-SEC-026). Window tests shall render the whole window without a display and check what it shows.
*Amended 12 October 2026* (decision E.70): channels in folders.
*Verification:* The tests run in the gate.

**REQ-WEB-028 — Discovery: from a web address to a channel.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
The web module shall be able to find the teletext edition that an ordinary web address announces, and the browser shall use it, as decision E.73 sets out. The design is `prototypes/browser/docs/web-discovery.md`.

**When it runs.** Discovery shall run only after the viewer has confirmed the address on the tuning screen (REQ-NAV-014), and only for:
- **a folder that holds no channel.** The browser first opens the folder as a channel (REQ-WEB-018). When every one of its requests (`channel.json`, `index.t42`, and the asked-for page as `.tti` and as `.t42`) answers 404, 410, a redirect that leaves the channel's folder, or status 200 with the media type `text/html`, the folder holds no channel, and discovery runs on the address as the viewer gave it, before any trailing `/` was added. A refused media type is decided from the headers, and the body of such an answer shall not be read;
- **an address that names no channel** (REQ-WEB-023): one whose last segment is a file name that is not one of a channel's, or one with a query.

It shall never run for an address that is a channel's file (REQ-WEB-019), for pre-fetching (REQ-SEC-013), for an address a pointer led to, or because of anything a page or a decoder sends.

**The request.** The web module shall make one `GET` request for the address, with these headers and no others: `Host`, the fixed `User-Agent`, `Accept: text/x-teletext-tti, application/x-teletext-t42, text/html;q=0.5`, and `Accept-Encoding: identity`. It shall send no cookie and no `Referer`, and shall ignore every `Set-Cookie` in the answers. It shall follow at most 5 redirects, to any origin, applying the local-network checks (REQ-SEC-006) to every address reached and refusing a redirect from `https` to `http` (REQ-SEC-007). The time limits of REQ-SEC-011 apply.

**What it reads.** From the final answer it shall read the headers, within the limit of REQ-WEB-008, and, when the answer is status 200 with the media type `text/html`, at most the first 8,192 bytes of the body after any chunked encoding is removed. It shall then close the connection without reading further, and the connection shall not be reused. A `Content-Encoding` other than `identity` ends discovery with nothing found.

**The order of the answer.** The first of these that gives a target passing REQ-WEB-030 is the result:
1. **A page.** When the final address is a page file of a channel (`NNN.tti` or `NNN.t42`, REQ-WEB-023), and the answer is status 200 with a media type that REQ-WEB-027 accepts for that file, the result is that page of that channel. The body read for discovery is not used; the channel then opens and fetches the page as usual. A teletext media type at any other address shall be refused with a diagnostic saying that pages must be served from a channel's page files.
2. **The `Link` header pointers** of the final answer, in the order they appear (REQ-WEB-029).
3. **The `<link>` element pointers** in the first 8,192 bytes of the body, in the order they appear (REQ-WEB-029).

When none gives a target, the result is "no teletext edition found", shown with the address asked for and, where it differs, the final address.

**The result.** Discovery shall return to the browser the target address, the final address that announced it, and how it was found. The browser then goes to the target as REQ-NAV-014 says. A target that is not a channel, or whose channel cannot be opened, shall be reported as a failure; discovery shall never run on it.
*Reason:* Decision E.73. The four requests of decision E.70 item 5 are kept first, so a channel at a folder address behaves exactly as before.
*Verification:* Tests against the local test server (REQ-TST-081) for each case of "When it runs" and each of the three results; that no request carries a cookie or `Referer`; that a redirect to a local address, and from `https` to `http`, is refused; that no more than 8,192 bytes of a body are read, measured at the transport; and that a target that fails to open does not lead to a second discovery.

**REQ-WEB-029 — Pointers: the `Link` header and the `<link>` element.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
A pointer is a link relation of type `alternate` whose media type is `text/x-teletext-tti` or `application/x-teletext-t42`, compared without regard to letter case, and with any parameters ignored. Both readers below shall be `no_std`, shall not allocate, shall be fuzzing targets (REQ-TST-040), and shall refuse what they cannot read exactly rather than guess.

**The `Link` header** (RFC 8288 section 3). Every `Link` header field of the final answer shall be read, in order. Each link value is `<` target `>` followed by parameters, separated by `;`, with link values separated by `,` outside quoted strings. A parameter value is a token or a quoted string. A link value is a pointer when:
- its `rel` parameter, split at spaces, contains `alternate`;
- its `type` parameter is one of the two media types;
- it has no `anchor` parameter, since that would make it describe some other resource.

A link value that is malformed is ignored, and the reader continues with the next value of the field; a field that cannot be split into values is ignored whole. At most 16 link values shall be examined in all.

**The `<link>` element.** The first 8,192 bytes of an HTML body shall be scanned as bytes, comparing names without regard to ASCII letter case. The scanner shall:
- **stop** at a byte-order mark for UTF-16, at a zero byte, at `<body` followed by white space, `/` or `>`, at `</head`, and at the end of the 8,192 bytes, including in the middle of a construct;
- **skip** comments from `<!--` to the next `-->`, `<!` and `<?` constructs to the next `>`, end tags to the next `>`, and the contents of the elements `script`, `style`, `noscript`, `template`, `textarea`, `title` and `xmp`, up to their end tag, as the HTML standard's raw text and escapable raw text elements are read;
- **read the attributes of every start tag**, so that a `>` inside a quoted value does not end it: a name runs to `=`, white space, `/` or `>`; a value is double-quoted, single-quoted, or unquoted up to white space or `>`. Of two attributes with the same name, the first is used, as HTML does;
- **note a `<base>` element** with an `href` attribute;
- **take a `<link>` element** as a pointer when its `rel` attribute, split at white space, contains `alternate`, its `type` attribute is one of the two media types, and its `href` attribute is present. Leading and trailing white space is removed from the `href` value.

At most 16 `<link>` elements with `rel` containing `alternate` shall be examined. Character references are never decoded: an `href` that holds `&` is not a valid target (REQ-WEB-030). Nothing else in the HTML is read, kept or shown.
*Reason:* A small, exact reader of two constructs, bounded by the 8,192-byte prefix, is enough to find a pointer that the publisher has put where decision E.73 says, and is far smaller than an HTML parser.
*Verification:* Tests of every rule above: pointers in headers and in the body, in both letter cases, with parameters, with `anchor`, malformed values, quoted commas; `<link>` elements inside comments, scripts, styles and titles (not taken), after `<body` or `</head>` (not taken), split by the 8,192-byte limit (not taken), with `>` in a quoted value, with duplicate attributes, unquoted values, and a UTF-16 body; the 17th candidate; and fuzzing.

**REQ-WEB-030 — The target of a pointer.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
A pointer's target shall be checked before it is used, and refused, with a diagnostic naming the reason, when any check fails:
1. **Characters.** The target holds only printable ASCII, and no `&`.
2. **Resolution.** An absolute target is used as it is. A relative target from a `Link` header, or from a `<link>` element with no `<base>` element before it, is resolved against the final address of the answer by RFC 3986 section 5. A relative target from a `<link>` element after a `<base>` element is refused, and the publisher is told to write an absolute address.
3. **The strict parser.** The result passes REQ-WEB-016, and its scheme is `http` or `https`.
4. **A channel.** The result is a channel's folder, or one of a channel's files, as REQ-WEB-023 and the address bar (REQ-BRW-002) take them. A folder without its trailing `/` is refused, so that a target is never guessed.
5. **No downgrade.** When the answer that gave the pointer came over `https`, the target is `https` (REQ-SEC-007).
6. **The local network.** The target's host is checked by REQ-SEC-006 when it is connected to, as every address is; a target that is a literal address in a blocked range is refused at once.

The browser shall receive the target from the fetcher as an address, and shall parse and check it again by rules 3 to 5 before acting on it.
*Reason:* A pointer is input from an arbitrary web server. Refusing every target that is not plainly a channel keeps discovery from leading anywhere a typed address could not.
*Verification:* A table of targets, valid and invalid, covering each rule, including `&amp;` in a target, a relative target with and without `<base>`, a folder without its trailing `/`, an `https` answer pointing to `http`, a target at 127.0.0.1 and at 192.168.1.1, a `javascript:` target, and a target that is a file name of no channel.
