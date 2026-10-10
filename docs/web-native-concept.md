# Web-native teletext: concept

A proposal, not a requirement. Nothing here changes REQUIREMENTS.md until it is recorded as decisions in Part E and as requirements in area WEB.

It builds on what the project already specifies:

- area WEB (REQ-WEB-001 to REQ-WEB-017): service descriptors, fetching, caching, push, cookies, navigation, security, offline use;
- Appendix C: forms;
- Appendix E: the TTI Link Extension;
- the viewer's keys, in area NAV and the user guide.

Where an idea is already specified, the requirement is named.

## 1. The model

A web-native teletext browser treats a web site as a teletext service. It keeps the teletext screen, page numbers, coloured links and compact text, but fetches pages from web servers and moves between services the way a web browser moves between sites.

- **A site is a web origin that serves teletext.** Opening `https://example.org/` shows the site's front page, page 100.
- **A page number belongs to the current site.** Typing `140` asks the current site for page 140. As on a television, the current page stays on screen while the header searches for the new one (REQ-WEB-007).
- **Leaving the site is always visible.** Typing an address with a page number, or following a link to another site, clears the screen and shows where the browser is going before anything from that site is shown (section 6).
- **Pages are pure teletext.** No scripts, no layout engine, no executable content. One decoder handles broadcast, files and the web.

The rule the rest of this document serves: a page number belongs to the current site by default, and only clearly declared links lead elsewhere. Number entry stays predictable, and sites still join up into a web.

### Pages do not contain addresses

Appendix E already says that pages never contain addresses. A page names another site by a short name, and the site's descriptor maps that name to an address (REQ-WEB-002). This proposal keeps that rule, because:

- pages cannot become phishing tools;
- every site a page can lead to is declared in one place, by the site itself;
- typing an address is the only way to go anywhere at all.

## 2. Sites and resources

### The descriptor

1. **One descriptor per site.** The service descriptor of REQ-WEB-002 already declares:
   - the service name and initial page;
   - the page address template;
   - push, other origins for forms, and the default character set region;
   - other services that links may lead to.

   It is extended here rather than replaced by a second manifest. New members proposed in this document:
   - aliases (section 5);
   - bundles (item 5);
   - supported features;
   - a search page;
   - a refresh interval for live pages.
2. **Where it lives.** At `/.well-known/teletext.json`. A file at the site root such as `index.json` could collide with the site's own files.
3. **Sites without a descriptor.** `/index.t42` is the front door, and page 100. Typing `140` fetches `/140.t42` relative to the site's base address. A simple site is then a folder of files on any static host.

### Resources

4. **One resource is one page.** A response for page 140 holds page 140 and its sub-pages, and nothing else is stored from it. A `.t42` resource is a packet stream, so it could also carry page 200. Accepting that would let any response overwrite other pages in the store. The browser checks that the response is the page it asked for.
5. **Bundles only when declared.** A site may declare a bundle in its descriptor: a `.t42` that carries a magazine or the whole site. One fetch then fills the store, and every page number after it is instant, as on a broadcast carousel. Only a declared bundle may hold more than one page.
6. **Enhancement dependencies.** At Levels 2.5 and 3.5, a page can use objects, DRCS characters and default objects stored on other pages:
   - the Magazine Organization Table at page xFE;
   - the object pages and DRCS pages it points to.

   Fetching page 140 alone shows it without them. Either the browser follows the table to fetch them, as a receiver acquires them from broadcast, or the author makes the page self-contained by putting its dependencies in its own resource. The exception to item 4 is exactly the pages the Magazine Organization Table names for the requested page.
7. **Two page formats.**
   - `.tti` is for authoring; it carries links (Appendix E) and forms (Appendix C).
   - `.t42` is for captures and archives; it carries links in packet X/27/0.

   The browser chooses by the `Accept` header or the file extension (REQ-WEB-003 accepts both). Links to other sites in a `.t42` resource come from the descriptor's aliases, so no companion file is needed.

## 3. Address grammar

A colon in an address means a port, and a URL parser reads `weather:400` as a URL with the scheme `weather`. So the colon is never used as a separator. One grammar covers every place a target can appear:

| Part | Syntax | Example |
|---|---|---|
| Page | 3 hexadecimal digits | `140` |
| Sub-page | `.` and 4 hexadecimal digits | `140.0003` |
| Another site, named in a page | `name/page[.sub]` | `weather/400` |
| Typed in the address bar | `host[:port]/page[.sub]`, or `host page` | `example.org:8080/140.0003`, `weather.example 300` |
| Shared on the ordinary web | `https://host[:port]/#page[.sub]` | `https://example.org/#140.0003` |

Rules that keep it unambiguous:

- **Inside a page, `x/400` is always a named service.** Pages never contain hosts.
- **Typed input is a host if it contains a dot or a colon.** A bare word is a channel preset or a name from the current site's descriptor. Single-label hosts such as `localhost` are refused unless `--allow-local` allows them (REQ-SEC-006).
- **Never `@`.** In a URL, `@` separates a user name from the host, which is how `bank.example@evil.example` deceives.
- **The page number goes in the fragment** of a shared address. Browsers never send the fragment to the server, so the site's descriptor still decides which file serves page 140. One address carries both site and page, and works in ordinary links and bookmarks.
- **An address typed without a page number** opens the site's initial page. An address that points directly at a `.t42` file opens the site and the page that file holds. The address bar always normalises what was typed into the shared form above.

**This changes existing work.**

- Appendix E (normative) defines link targets as `mpp:ssss` and `name:mpp`, and the TTI reader implements them.
- `ttx` documents its `PAGE` argument as `100:0003`.

The change needs a Part E decision before the reader, the page checker, the command-line program and the documentation are changed.

## 4. The location bar: row 0

Row 0, the header row, is the natural location bar. On a television it has always been the "where am I" row, with the page number, the service name and the clock.

### While browsing

| Columns | On broadcast | On a web site |
|---|---|---|
| 0 to 7 | The receiver's alone: the page number searched for, and the security indicators (REQ-SEC-005) | Unchanged: the page number, and the encrypted or unencrypted marker |
| 8 to 31 | The service name, sent by the broadcaster | The site's host, written by the browser |
| 32 to 39 | The clock | The clock, or given to the host when it is long |

### While entering an address

The Address key (section 7) turns all of row 0 into an editable field. Long addresses scroll sideways. Enter goes there; Escape restores the current location. The full address of the current page is shown in the same field when it is opened.

### Why row 0

- **It is already partly the receiver's.** Columns 0 to 7 belong to the receiver alone, so owning the whole row on web sites extends an existing rule rather than adding a concept.
- **Entry already happens there.** On a television, the page number being typed appears in the header.
- **The page loses nothing.** Rows 1 to 23 are content, and row 24 stays the Fastext row.

### What makes it trustworthy

- **The browser writes all of row 0 for web pages.** On broadcast, columns 8 to 31 come from the service. If a web page's header text were shown there, a page could write `bank.example`. Header text in a web page file is ignored. This changes how headers are displayed, so it needs a decision before it is built.
- **Nothing else may draw on row 0.** That includes Level 2.5 and 3.5 enhancement data, objects and side panels, the same rule columns 0 to 7 already need.
- **Shorten from the left.** 24 characters is short. The registrable domain is always shown in full and right-aligned, for example `…news.example.co.uk`. Without the clock the row has 32 characters.

A port with space outside the 40 by 25 grid, such as a terminal line below row 24 or window chrome, may show the full address there as well. A page cannot reach that space. It is an extra, not a replacement, because an embedded port or a real television has no such space.

## 5. Links

Every link target is a site and a page, with an optional sub-page, in the grammar of section 3.

### Where links come from

1. **The Fastext keys.** Red, green, yellow, cyan and Index lead to declared destinations (Appendix E). Links within the site are also written to packet X/27/0, so the same pages work on a real Fastext television. The browser, not the page, adds a consistent marker to prompts that lead to another site (Appendix E's "»").
2. **Numbers in the text (page catching).** Some Philips televisions had a cursor that jumped between the three-digit numbers printed on a page; pressing OK went there.
   - A same-site link needs no markup: the "140" in "SPORT 140" is the link. Typing 140 works whether or not the viewer selects it.
   - A cross-site link is a name followed by a number, as in `WEATHER 300`. It counts only if the descriptor names it, so arbitrary text cannot make a link.
   - An optional TTI line, for example `LT,row,column,length,weather/300`, makes any run of text a link, including text that does not look like a page number.
   - The arrow keys or a pointer move between links, and Enter follows the selected one (section 7). Typing the number stays the fastest route.
3. **Number aliases.** The descriptor maps one of the site's page numbers to a page on another site, for example `"aliases": {"300": "weather/300"}`. Every way of reaching that number then leads to the other site through the tuning screen: typed digits, page catching, next and previous, and X/27/0 links. Typing a number can then lead to another site with no new mechanism.

### Typed numbers mean the same on every page

An alternative rule would let a link on the current page take over the typed number. Then `300` would go to one place on page 140 and another on page 141, and any page could send a typed number to another site. Instead, a typed number means the same thing everywhere on a site:

- the site's own page;
- or the alias the descriptor declares for that number.

A label such as `WEATHER 300` on a page only names the alias. The page checker warns when a page labels a number with a site the alias does not lead to.

### Broadcast televisions

- **Same-site links** work everywhere: through X/27/0 links, and through the page numbers printed in the text.
- **Cross-site links** cannot work on a broadcast television. An alias page number therefore holds a small stub page on the site, for example "Weather is on the WEATHER service, page 300", with a QR code drawn in mosaic graphics. A web browser never shows the stub, because it follows the alias; a television shows it. Appendix E rule 5 ("reachable without Fastext") still holds.

## 6. Leaving a site: the tuning screen

Before any page from another site is shown, the browser clears rows 1 to 24 and shows the tuning screen. It stays on screen while the page loads.

- **The host comes first.** The registrable domain is shown large, for example in double height, and the full address is wrapped across the rows below it. Deceptive addresses hide in long paths, so the domain must stand out from the rest.
- **International domain names** are shown in their ASCII form, because teletext character sets cannot show most of Unicode. Names that imitate other domains are flagged.
- **Encryption is stated.** The screen says whether the connection is encrypted or plain HTTP (decision E.32).
- **Actions.** Enter or F4 confirms, and Cancel (Delete or Escape) returns to the page you came from. A typed address or an unfamiliar site asks for confirmation. Going back to a site already confirmed in this session does not.
- **Redirects.** REQ-WEB-003 follows up to five redirects. A redirect to another host changes the destination: the tuning screen is updated, and confirmed again, before that host's content is shown. Otherwise a redirect would get around the tuning screen. The final address is what row 0 shows afterwards.
- **It cannot be forged.** A 40 by 25 page can draw anything, including a fake tuning screen. The real one is marked by things no page can produce:
  - the browser-owned row 0 of section 4;
  - a receiver-only marker in row 0 columns 0 to 7;
  - the line outside the grid, where the port has one.

## 7. The keyboard

The keyboard works like a teletext remote control. The teletext keys stay the same in every mode, and the navigation keys work whether they come from the dedicated cluster or from the numeric keypad.

### The coloured keys

| Key | Fallback | Function |
|---|---|---|
| F1 | `r` | Red |
| F2 | `g` | Green |
| F3 | `y` | Yellow |
| F4 | `c` | Cyan |

F1 to F4 are the coloured keys everywhere, including inside forms and on the tuning screen (F4 confirms, as for forms today). The viewer already does this.

### Digits

The top-row digits, and the numeric keypad with Num Lock on, enter page numbers. While a number is being typed, `.` starts a sub-page (`140.3` asks for sub-page 0003), and Enter goes without waiting for the third digit. Inside a form field, digits are text.

### The navigation keys

The keypad with Num Lock off sends the same keys as the dedicated cluster, so one mapping covers both:

| Key | Keypad | Function | Today |
|---|---|---|---|
| Up | 8 | Next page | Same |
| Down | 2 | Previous page | Same |
| Right | 6 | Next sub-page | Same |
| Left | 4 | Previous sub-page | Same |
| Home | 7 | The site's initial page | Same |
| End | 1 | Index, the fifth Fastext key | `i` only |
| Page Up | 9 | Back through the history | Alt+Left or Backspace |
| Page Down | 3 | Forward through the history | Alt+Right |
| Insert | 0 | Hold | `H` only |
| Delete | . | Cancel: erase the number being typed, stop loading, or leave the tuning screen without going | New |
| Enter | Enter | Confirm: go to the typed number, follow the selected link, confirm the tuning screen or a form | Forms and confirmations only |

Why these:

- **Up and Down are the page keys and Left and Right the sub-page keys** because the viewer already works that way.
- **Home and End are the two "go to a known page" keys**, and they sit together in the keypad's left column.
- **Page Up and Page Down are Back and Forward**, in the keypad's right column. History then needs no modifier keys, which a keypad does not have. Backspace stays Back as well, except while a number or address is being typed, when it deletes the last character.
- **Insert is a toggle**, as Hold is.
- **Delete undoes**: it erases, stops, or cancels.

When page catching is turned on (section 5), the arrow keys move between links on the page, and Enter follows the selected one. Page Up and Page Down stay Back and Forward, so leaving the mode is never needed to go back.

### The rest of the keypad with Num Lock on

| Key | Function |
|---|---|
| `+` and `-` | Next and previous page (the viewer already does this) |
| `.` | Sub-page separator while typing |
| `*` | Reveal |
| `/` | Mix |
| Enter | Confirm |

With Num Lock on, the keypad is a complete remote control: page numbers, page up and down, reveal, mix and confirm.

### Browser keys

| Key | Function |
|---|---|
| Ctrl+L or F6 | Address: open the location bar for typing |
| F5 | Refresh the current page |
| Alt+Left, Alt+Right | Back and Forward (unchanged) |
| Escape | Cancel address entry, the tuning screen or a form |
| `q` or Ctrl+C | Quit |

**Escape quits today.** In a browser, Escape cancels, so this proposal makes `q` and Ctrl+C the only quit keys. That needs a decision, because it changes what existing users press.

### Limits of real keyboards

- **No Insert, Home, End, Page Up or Page Down on Mac laptops.** Fn with the arrow keys gives Home, End, Page Up and Page Down, but there is no Insert. Every function keeps a letter fallback, so no function depends on one of these keys.
- **No Num Lock on Mac keypads.** The keypad always sends digits, so on a Mac it is the remote-control layout only.
- **F1 to F4 may be taken by the system.** On Macs, the function keys control brightness and media unless Fn is held or the system setting is changed. The letter fallbacks cover this.
- **The keypad cannot be told apart from the main keys** without the kitty keyboard protocol. The mapping above never needs to tell them apart: each keypad key means the same as the main key it sends.

## 8. Loading, errors and updates

- **The header searches.** During a fetch, the requested page number rolls in the header. Waiting feels like teletext, not like an error.
- **Readable error pages.** A missing page, or a failed fetch, becomes a teletext page drawn by the browser. It says which page was asked for, from which site, and offers Retry, Back (Page Up) and Home.
- **Conditional refresh.** Pages already stored are shown at once, and a conditional request checks they are current (REQ-WEB-004, REQ-WEB-007). F5 asks again now. A live page's refresh interval is declared in the descriptor.
- **Live updates in place.** News, scores and departure boards receive new packets over server-sent events or WebSocket (REQ-WEB-005). Hold (Insert) freezes the screen while acquisition continues.
- **Prefetching.** The browser fetches the four Fastext targets and the next and previous pages ahead of time, as the SAA5246A kept four pages ready (REQ-NAV-004, Phase 2).
- **Sub-pages.** Left and Right step through sub-pages, `140.3` asks for one directly, and automatic cycling can be turned on. A refresh keeps the selected sub-page. One response can carry several sub-pages, and the browser rotates them with their timing, as on broadcast.
- **Offline.** Pages already received stay viewable, and a page not confirmed for 60 seconds shows "?" (REQ-WEB-009). The browser also keeps the pages each page depends on (section 2 item 6). A page shown from the cache says when it was saved, for example `OFFLINE  SAVED 14:32`, in the browser-owned row 0.

## 9. History, home and finding sites

- **History** holds the site, page and sub-page of each step, and covers every site. Back restores the previous screen at once from the store, then refreshes it.
- **Home** (the Home key) returns to the current site's initial page. A separate browser start page holds bookmarks, presets and recently visited sites.
- **Bookmarks** are a site and a page, on numbered favourite keys.
- **Channel presets.** A numeric keypad cannot type an address, so the browser keeps a local list of sites, as a television keeps channel presets. `P` followed by `0` `7` goes to preset 7. Presets are added from sites already visited or from a directory site.
- **Directory sites.** A site whose pages list other sites, each with a page number, is the start page of this web: its search engine and its channel guide.
- **Shareable addresses.** Every page and sub-page has the stable address of section 3. It can be copied, bookmarked or opened directly, and it leads to the same page whatever page the sender was on.

## 10. Forms and sessions

Forms (Appendix C) cover search, bookings, messages and sign-in, over ordinary web requests.

- **Before sending,** the browser shows the receiving site and asks for confirmation (REQ-SEC-009).
- **Destinations.** A form goes only to its own site or to sites its descriptor names.
- **Typing precedence.** While a field is being edited, typing is text, not page-number entry. F1 to F4 remain the coloured keys, and Escape leaves the form.
- **Sessions.** Cookies belong to the site that set them (REQ-WEB-006).

## 11. Accessibility

The teletext grid stays the visual identity, but the content is usable beyond a raster image:

- a text view with selectable text;
- navigation by screen reader, row by row and link by link;
- link descriptions taken from the Fastext prompts and labels;
- adjustable scale, which `ttx` already has.

The text output of `ttx text` already gives each page as Unicode, so it is a starting point.

## 12. Safety and privacy

- **Limits.**
  - A page is about 1 KB, so a much larger response is refused, and bundles have a limit of their own.
  - Redirects stop at five (REQ-WEB-003).
  - Local-network addresses are refused (REQ-SEC-006).
  - There is never executable content.
- **No tracking.**
  - The browser makes no requests to other sites on a page's behalf.
  - It sends no `Referer`.
  - It gives cookies only to the site that set them.
  - It has one fixed, minimal user agent (REQ-WEB-014).
- **Cross-site links are declared.** `weather/300` works only if the site's descriptor names `weather` (Appendix E rule 2). A site's links are as trustworthy as the site.
- **A whole-site checker.** `ttx check` already checks pages. It could also crawl a site and report:
  - links that lead nowhere;
  - names that resolve to nothing;
  - pages missing from a bundle;
  - missing enhancement dependencies;
  - aliases without a stub page;
  - labels that disagree with their alias;
  - links whose page numbers do not appear in the page's text.

## 13. Reach

- **Ordinary web sites can announce a teletext edition**, with `<link rel="alternate" type="application/x-teletext-t42" href="/index.t42">` or a DNS TXT record. The browser can then open `example.org` directly.
- **A `teletext:` link handler** on desktop systems opens `ttx` from an ordinary browser.
- **One file, both media.** The `.t42` a site serves can be inserted into a real broadcast signal with vbit2 or a Raspberry Pi, and the stub pages and QR codes of section 5 lead television viewers to the site.
- **Archives as sites.** A recovered capture, such as a Ceefax recording from 1990, is published as a site and browsed by page number. A parameter such as `?at=` could choose the moment of the capture to show.
- **Levels 2.5 and 3.5 unchanged.** DRCS characters, objects and the 32-colour Colour Map are page data, so sites get logos and their own fonts without scripts.
- **A browser in the browser.** The `no_std` core compiled to WebAssembly can show any site that allows cross-origin requests, so people can try it without installing `ttx`.

## 14. A first version

| Viewer action | Browser behaviour |
|---|---|
| Open a site's address | Fetch the descriptor, or `/index.t42` without one, and show the initial page. |
| Type `140` | Show the site's page 140, or follow the descriptor's alias for 140 through the tuning screen. |
| Type an address and a page number | Show the tuning screen, then the page. |
| Follow a link to another site | Show the tuning screen, then the page. |
| Page Up, Page Down | Back and Forward, restoring the screen and location. |
| Insert | Hold: freeze the screen while acquisition continues. |
| F5 | Fetch the current page again. |
| Delete or Escape | Cancel what is in progress. |

## 15. Decisions needed

1. **Pages and addresses.** Do pages ever contain addresses? This proposal says no, as Appendix E already does.
2. **The address grammar** of section 3. It replaces the colon forms of Appendix E and the `PAGE` argument.
3. **The descriptor.** What it gains:
   - its well-known location;
   - aliases;
   - bundles;
   - features;
   - a search page;
   - a refresh interval.
4. **Resources.** Is a site one bundle, separate pages, or both? If both, in what order does the browser look?
5. **Enhancement dependencies.** Does the browser follow the Magazine Organization Table, do authors make pages self-contained, or both?
6. **Row 0 on web sites.** Does the browser own the whole row, and does the clock give way to long hosts? This changes how broadcast headers are displayed.
7. **The tuning screen.** How it is made impossible to forge on a port with nothing outside the 40 by 25 grid.
8. **Aliases.** May an alias cover a whole range, for example `3xx` to `weather/3xx`, which makes a magazine of one site part of another?
9. **Back across sites.** Does going back to a site already confirmed skip the tuning screen?
10. **Page catching.** How a number that is a link is shown, so that it differs from a number that only appears on the page.
11. **Keys.** The changes of section 7:
    - Page Up and Page Down become Back and Forward;
    - End becomes Index;
    - Insert becomes Hold;
    - Delete becomes Cancel;
    - Escape no longer quits.
12. **Input modes.** How number entry, address entry, form editing and page catching are told apart on screen.
