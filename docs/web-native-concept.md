# Web-native teletext: concept

The agreed design for browsing teletext over the web. Every open question was settled on 11 October 2026 (section 15). It is not yet part of REQUIREMENTS.md: each decision becomes a Part E decision, and the behaviour becomes requirements in area WEB, before it is built.

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

The rule the rest of this document serves: a page number belongs to the current site unless the page or the site links it elsewhere, and leaving the site is always shown and confirmed. Sites join up into a web, and the viewer always knows when they are leaving one.

### Pages may contain addresses

A page can link a Fastext key, a run of its text, or a typed page number to another site, in either of two forms (section 3):

- **an address written in the page**, such as `https://weather.example/300.t42`. Sites without a descriptor can link elsewhere this way;
- **a named service**, such as `weather:300`, which the site's descriptor maps to an address (REQ-WEB-002). This suits destinations many pages share, which then change in one place.

There is no setting that forbids addresses in pages. A site that publishes other people's content removes their links before publishing, as on the ordinary web. The safeguards are the tuning screen (section 6), which every link to another site passes through, and the page checker (section 12).

Appendix E now says that addresses never appear in pages. This design amends that rule (section 15, decision 1).

## 2. Sites and resources

### The descriptor

1. **One optional descriptor per site, for site-wide settings only.** The service descriptor of REQ-WEB-002 already declares:
   - the service name and initial page;
   - the page address template;
   - push, other origins for forms, and the default character set region;
   - other services that links may lead to.

   It is extended here rather than replaced by a second manifest. A site needs one only for these settings; files alone are a working site (item 4). It gains three members:
   - its well-known location (item 2), so typing a bare host finds it;
   - aliases: page numbers that lead to another site on every page of the site (section 5);
   - a refresh interval for live pages on sites without push.

   A list of supported features and a search page were considered and left out. Nothing would read the features list, and a search page is an ordinary page that a site links to.
2. **Where it lives.** At `/.well-known/teletext.json`. A file at the site root such as `index.json` could collide with the site's own files.
3. **Sites without a descriptor.** `/index.t42` is the front door, and page 100. Typing `140` fetches `/140.t42` relative to the site's base address. A simple site is then a folder of files on any static host.

### Resources

4. **Separate pages and bundles both work, with no setting.** A site is whatever files it has:
   - **A page file** (`/140.t42`, or wherever the descriptor's template says) holds that page and its sub-pages. Any other page in it is ignored, apart from the dependencies of item 6, so a page file cannot overwrite the rest of the site.
   - **The front door** (`/index.t42`, or the descriptor's initial page) may hold any number of pages. A whole capture or carousel dropped in as `index.t42` is then a working site, and every page number in it is instant after one fetch, as on broadcast.

   A typed number is looked for in this order:
   1. the store, if the page is there and confirmed current (REQ-WEB-004);
   2. the page's own file;
   3. otherwise "not found".

   A page that came from the front door but also has its own file is refreshed from its own file, so an author can update one page of a bundled site without rebuilding the bundle.
5. **Why this order.** It is what an author expects from the web: the front door is the site, a file named after a page is that page, and nothing needs declaring. Large sites use page files, small sites and archives use one file, and mixed sites work too.
6. **Enhancement dependencies: both ways, with no setting.** At Levels 2.5 and 3.5, a page can use objects, DRCS characters and default objects stored on other pages:
   - the Magazine Organization Table at page xFE;
   - the object pages and DRCS pages it points to.

   The browser finds them as a broadcast receiver does:
   1. **In the page's own file.** An author can make a page self-contained by putting the pages it uses in its file. These are the only other pages a page file may carry.
   2. **Otherwise, by following the table.** The browser fetches the magazine's table (`/1FE.t42` for magazine 1) and then the object and DRCS pages it names, from their own files. It keeps them in the store, so they are fetched once per magazine, not once per page.

   An author who publishes a broadcast's pages as files therefore needs to do nothing special. While dependencies load, the page is shown at Level 1.5 and redrawn when they arrive. A dependency that cannot be fetched removes only what depends on it, as a fault does on broadcast.
7. **Two page formats.**
   - `.tti` is for authoring; it carries links (Appendix E) and forms (Appendix C).
   - `.t42` is for captures and archives; it carries links in packet X/27/0.

   The browser chooses by the `Accept` header or the file extension (REQ-WEB-003 accepts both). Packets cannot hold an address, so links to other sites from a `.t42` resource come from the descriptor's aliases, and no companion file is needed.

## 3. Link targets

No new address syntax is needed. A link target in a page is one of:

| Target | Form | Example |
|---|---|---|
| A page of the current site | `mpp` or `mpp:ssss` (Appendix E) | `140`, `140:0003` |
| A page of a named service | `name:mpp` (Appendix E) | `weather:300` |
| A page of any site | An ordinary `https://` or `http://` address of the page's file | `https://weather.example/300.t42` |

- **One new rule tells them apart:** a target containing `://` is an address. Only `https` and `http` are accepted, so `weather:300`, which has no `//`, is always a named service.
- **An address is just the page's file.** The browser fetches it and shows it as that page of that site, as a link works on the ordinary web. The site's other pages then follow the site's descriptor, or `/{mpp}.t42` without one.
- **Appendix E's forms and `ttx`'s `PAGE` argument are unchanged.**

## 4. The location bar: row 0

Row 0, the header row, is the natural location bar. On a television it has always been the "where am I" row, with the page number, the service name and the clock.

### While browsing

| Columns | On broadcast | On a web site |
|---|---|---|
| 0 to 7 | The receiver's alone: the page number searched for, and the security indicators (REQ-SEC-005) | Unchanged: the page number, and the encrypted or unencrypted marker |
| 8 to 31 | The service name, sent by the broadcaster | The site's host, written by the browser |
| 32 to 39 | The clock | The clock while the host fits in columns 8 to 31; otherwise the rest of the host |

### While entering an address

The Address key (section 7) turns all of row 0 into an editable field. It takes an ordinary address, such as `https://weather.example/300.t42`, or a host and a page number, such as `weather.example 300`, which is a convenience of the field and not a format pages use. An address without a page number opens the site's initial page. Long addresses scroll sideways. Enter goes there; Escape restores the current location. The full address of the current page is shown in the same field when it is opened.

### Showing what the keys are doing

The same keys do different things at different times. Digits choose a page, but type text in a form field or an address. The arrows change page, but move between links in page catching and move the cursor in a form. The viewer must always be able to see which of these is happening. Row 0 says so, in the receiver's columns 0 to 7:

| What is happening | Row 0 |
|---|---|
| Browsing | The page number, as now: `P140` |
| Typing a page number | The digits so far: `P14-` |
| Typing an address | All of row 0 is the address field, with a cursor |
| Editing a form | `FORM`, and the cursor in the field being edited |
| Page catching | `LINK`, the selected link highlighted, and its destination in columns 8 to 39 |
| Waiting for confirmation | `GO?`, and the destination |

Pages cannot write row 0, so they cannot fake any of these.

### Why row 0

- **It is already partly the receiver's.** Columns 0 to 7 belong to the receiver alone, so owning the whole row on web sites extends an existing rule rather than adding a concept.
- **Entry already happens there.** On a television, the page number being typed appears in the header.
- **The page loses nothing.** Rows 1 to 23 are content, and row 24 stays the Fastext row.

### What makes it trustworthy

- **The browser writes all of row 0 for web pages.** On broadcast, columns 8 to 31 come from the service. If a web page's header text were shown there, a page could write `bank.example`. Header text in a web page file is ignored.
- **Nothing else may draw on row 0.** That includes Level 2.5 and 3.5 enhancement data, objects and side panels, the same rule columns 0 to 7 already need.
- **Shorten from the left.** 24 characters is short. The registrable domain is always shown in full and right-aligned, for example `…news.example.co.uk`. The clock stays while the host fits in columns 8 to 31, and gives way when it does not, so the row has 32 characters for the host.

A port with space outside the 40 by 25 grid, such as a terminal line below row 24 or window chrome, may show the full address there as well. A page cannot reach that space. It is an extra, not a replacement, because an embedded port or a real television has no such space.

## 5. Links

Every link target is a site and a page, written in one of the forms of section 3.

### Where links come from

1. **The Fastext keys.** Red, green, yellow, cyan and Index lead to any target of section 3: a page of the site, a named service or an address (Appendix E). Links within the site are also written to packet X/27/0, so the same pages work on a real Fastext television. The browser, not the page, adds a consistent marker to prompts that lead to another site (Appendix E's "»").
2. **Numbers in the text (page catching).** Some Philips televisions had a cursor that jumped between the three-digit numbers printed on a page; pressing OK went there. Here, OK (Enter) turns page catching on, and Cancel turns it off.
   - A same-site link needs no markup: the "140" in "SPORT 140" is the link. Typing 140 works whether or not the viewer selects it.
   - A name followed by a number, as in `WEATHER 300`, is a link to another site when the descriptor names that service, or when the page links that number (item 3). Other text is not a link.
   - An optional TTI line, for example `LT,row,column,length,https://weather.example/300.t42`, makes any run of text a link to any target, including text that does not look like a page number.
   - The arrow keys or a pointer move between links, and Enter follows the selected one (section 7). Typing the number stays the fastest route.
   - **How links are shown.**
     - Outside page catching, the page looks exactly as its author drew it.
     - In page catching, the selected link is shown with its colours swapped, drawn by the browser. The cursor only ever stops on real links, so a number that is not a link can never be selected.
     - Row 0 shows the selected link's destination, as a web browser's status bar does on hover: `LINK  P300` for the site's own page, `LINK »weather.example 300` for another site.
     - A *show links* key underlines every link on the page at once, using the receiver's own underline.
     - With a pointer, links are highlighted as the pointer passes over them.

     A page can draw text that looks like a highlighted link. It cannot be selected, and row 0 never shows a destination for it.
3. **Page number links.** A page links a number to any target with a TTI line such as `LK,300,https://weather.example/300.t42` or `LK,300,weather:300`. While that page is on screen, typing 300, or selecting the 300 printed on it, leads there through the tuning screen. The link applies to that page only.
4. **Number aliases.** The descriptor links a single number on every page of the site, for example `"aliases": {"300": "weather:300"}`. An alias is always one page number, never a range. Every way of reaching that number then leads to the other site: typed digits, page catching, next and previous, and X/27/0 links.

### Which link a typed number follows

1. The current page's own link for that number (item 3).
2. Otherwise, the descriptor's alias for it (item 4).
3. Otherwise, the site's own page with that number.

This order is chosen because it is what the viewer sees: the most specific link wins. A page that says `WEATHER 300` and links 300 to the weather site does what it shows: what the viewer sees on the page is what typing the number does. Because a page's link can make 300 mean different things on different pages, the page checker warns when a page links a number that it does not show.

### Broadcast televisions

- **Same-site links** work everywhere: through X/27/0 links, and through the page numbers printed in the text.
- **Cross-site links** cannot work on a broadcast television. The page encoder writes a Fastext link to another site as an empty X/27/0 link with its prompt in white (Appendix E rule 4). A number that leads to another site, through a page link or an alias, holds a small stub page on the site, for example "Weather is on the WEATHER service, page 300", with a QR code drawn in mosaic graphics. A web browser never shows the stub, because it follows the link; a television shows it. Appendix E rule 5 ("reachable without Fastext") still holds.

## 6. Leaving a site: the tuning screen

Before any page from another site is shown, the browser clears rows 1 to 24 and shows the tuning screen. It stays on screen while the page loads.

- **The host comes first.** The registrable domain is shown large, for example in double height, and the full address is wrapped across the rows below it. Deceptive addresses hide in long paths, so the domain must stand out from the rest.
- **International domain names** are shown in their ASCII form, because teletext character sets cannot show most of Unicode. Names that imitate other domains are flagged.
- **Who declared the link.** A named service or an alias shows the descriptor's name for the service with its domain. An address written in a page shows the domain only, marked as not declared by the site.
- **Encryption is stated.** The screen says whether the connection is encrypted or plain HTTP (decision E.32).
- **Actions.** Enter or F4 confirms, and Cancel (Delete or Escape) returns to the page you came from. A typed address or an unfamiliar site asks for confirmation. Going back, or forward, to a site already confirmed in this session does not.
- **Redirects.** REQ-WEB-003 follows up to five redirects. A redirect to another host changes the destination: the tuning screen is updated, and confirmed again, before that host's content is shown. Otherwise a redirect would get around the tuning screen. The final address is what row 0 shows afterwards.
- **It cannot be forged.** A 40 by 25 page can draw anything in rows 1 to 24, including a fake tuning screen. Row 0 is what makes the real one recognisable, on every port:
  - **Row 0 always tells the truth.** Pages cannot write row 0 on web sites (section 4), so it always shows the site the viewer is really on. A fake tuning screen still has the real site in row 0.
  - **The real tuning screen is announced in row 0.** Row 0 shows `GO?` in columns 0 to 7 and the destination in the rest of the row. No page can produce that.
  - **Only the receiver accepts the confirmation.** A fake screen cannot take the viewer anywhere: pressing Enter on it does nothing the page did not already link, and that link has its own real tuning screen. Pasted text never confirms (`docs/hmi.md` section 2.5).
  - **Ports with space outside the grid** also show the destination there.

  The rule for viewers is short: the site is whatever row 0 says.

## 7. The keyboard

The keyboard works like a teletext remote control. The teletext keys stay the same in every mode, and the navigation keys work whether they come from the dedicated cluster or from the numeric keypad.

Two rules shape every choice below:

- **Every function has a plain letter key that works on every keyboard.** Function keys, the navigation cluster and the keypad are shortcuts for those who have them. No function depends on a key that Mac laptops lack, and the viewer never asks anyone to change a system setting.
- **Escape cancels and never quits.** When there is nothing to cancel, Escape does nothing, so a habit from other programs cannot close the viewer by accident. `q` and Ctrl+C quit.

### The coloured keys

| Key | Letter | Function |
|---|---|---|
| F1 | `r` | Red |
| F2 | `g` | Green |
| F3 | `y` | Yellow |
| F4 | `c` | Cyan |

F1 to F4 are the coloured keys everywhere, including inside forms and on the tuning screen, where F4 confirms as it does for forms today. The viewer already does this.

### Digits

The top-row digits, and the numeric keypad with Num Lock on, enter page numbers. While a number is being typed, `.` starts a sub-page (`140.3` asks for sub-page 0003), and Enter goes without waiting for the third digit. Inside a form field, digits are text.

### The navigation keys

The keypad with Num Lock off sends the same keys as the dedicated cluster, so one mapping covers both:

| Key | Keypad | Letter | Function | Change from today |
|---|---|---|---|---|
| Up | 8 | `+` | Next page | None |
| Down | 2 | `-` | Previous page | None |
| Right | 6 | | Next sub-page | None |
| Left | 4 | | Previous sub-page | None |
| Home | 7 | `h` | The site's initial page | None |
| End | 1 | `i` | Index, the fifth Fastext key | End is new |
| Page Up | 9 | Backspace | Back through the history | Page Up is new |
| Page Down | 3 | `f` | Forward through the history | Both are new; Alt+Right also stays |
| Insert | 0 | `H` | Hold | Insert is new |
| Delete | . | Escape | Cancel: erase the number being typed, stop loading, leave page catching, or leave the tuning screen without going | New |
| Enter | Enter | | Confirm: go to the typed number, follow the selected link, confirm the tuning screen or a form, or turn on page catching | Was forms and confirmations only |

Why these:

- **Up and Down are the page keys and Left and Right the sub-page keys** because the viewer already works that way.
- **Home and End are the two "go to a known page" keys**, and they sit together in the keypad's left column.
- **Page Up and Page Down are Back and Forward**, in the keypad's right column. The arrows already step pages, and history needs keys without modifiers, which a keypad does not have. Backspace stays Back, except while a number or address is being typed, when it deletes the last character.
- **Insert is a toggle**, as Hold is.
- **Delete undoes**: it erases, stops, or cancels.

When page catching is on (section 5), the arrow keys move between links on the page, and Enter follows the selected one. Page Up and Page Down stay Back and Forward, so leaving the mode is never needed to go back.

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

| Key | Letter | Function |
|---|---|---|
| F6 | Ctrl+L | Address: open the location bar for typing |
| F5 | `u`, after the Update key of teletext remote controls | Refresh the current page |
| Alt+Left, Alt+Right | Backspace, `f` | Back and Forward |
| | `K` | Show the keys: a receiver-drawn page that lists the letter for each function first, so it is right on every keyboard |
| | `q`, Ctrl+C | Quit |

The other letters stay as they are: `?` Reveal, `m` Mix, `z` Zoom, and Tab to enter a page's form.

### Limits of real keyboards, and how the keys handle them

| Limit | How it is handled |
|---|---|
| Mac laptops have no Insert, and need Fn with the arrows for Home, End, Page Up and Page Down. | The letters. |
| Mac keypads have no Num Lock and always send digits. | On a Mac the keypad is the remote-control layout only. |
| On a Mac, F1 to F4 control brightness and media unless Fn is held or the system setting is changed. | The letters `r`, `g`, `y`, `c`. |
| iTerm2 does not send Option as Alt unless the profile says so, so Alt+Left and Alt+Right may never arrive. | Backspace and `f`. |
| The keypad cannot be told apart from the main keys without the kitty keyboard protocol. | Never needed: each keypad key means the same as the main key it sends. |
| Release events, which holding Index for Back needs, arrive only with the kitty keyboard protocol. | Used where the terminal offers it (`docs/hmi.md`). Everything else works without it. |

## 8. Loading, errors and updates

- **The header searches.** During a fetch, the requested page number rolls in the header. Waiting feels like teletext, not like an error.
- **Readable error pages.** A missing page, or a failed fetch, becomes a teletext page drawn by the browser. It says which page was asked for, from which site, and offers Retry, Back (Page Up) and Home.
- **Conditional refresh.** Pages already stored are shown at once, and a conditional request checks they are current (REQ-WEB-004, REQ-WEB-007). F5 or `u` asks again now. A live page's refresh interval is declared in the descriptor.
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
- **Shareable addresses.** Every page has an ordinary address, its page file, for example `https://example.org/140.t42`. It can be copied, bookmarked or opened directly, and it leads to the same page whatever page the sender was on.

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
- **Links written in pages.** There is no switch to forbid addresses in pages. A site that publishes other people's content, such as guest books or form responses, removes their links before publishing, and the server guidance of REQ-WEB-012 says so. A named service that the descriptor does not name is a link with no target (Appendix E rule 2).
- **A whole-site checker.** `ttx check` already checks pages. It is extended to crawl a site and report:
  - links that lead nowhere;
  - names that resolve to nothing;
  - pages missing from a bundle;
  - missing enhancement dependencies;
  - numbers that lead to another site without a stub page for televisions;
  - numbers a page links but does not show;
  - every address written in pages, with a suggestion to move an address several pages repeat into the descriptor;
  - labels that disagree with their link;
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
| Type `140` | Follow the page's link for 140, otherwise the descriptor's alias, otherwise show the site's page 140. A link to another site goes through the tuning screen. |
| Type an address and a page number | Show the tuning screen, then the page. |
| Follow a link to another site | Show the tuning screen, then the page. |
| Page Up, Page Down | Back and Forward, restoring the screen and location. |
| Insert | Hold: freeze the screen while acquisition continues. |
| F5 or `u` | Fetch the current page again. |
| Delete or Escape | Cancel what is in progress. Escape never quits. |
| `K` | Show the keys. |

## 15. Decisions

Settled on 11 October 2026. Decisions 1 to 5 are the owner's; decisions 6 to 14 were recommended and accepted by the owner.

1. **Addresses in pages.** Pages may link Fastext keys, text and page numbers to addresses as well as to named services, with no setting to forbid it (sections 1 and 5). This amends Appendix E's rule that addresses never appear in pages, and adds the `LK` and `LT` lines to the TTI Link Extension.
2. **Row 0 on web sites.** The browser owns the whole row (section 4).
3. **Aliases.** An alias is one page number, never a range (section 5).
4. **Back across sites.** Going back, or forward, to a site already confirmed in the session skips the tuning screen (section 6).
5. **Sub-pages in addresses.** Not needed. An address leads to a page's file and shows its newest sub-page (section 3).
6. **Resources.** Page files and a front-door bundle both work with no setting. Lookup order: the store, then the page's own file (section 2 items 4 and 5).
7. **Enhancement dependencies.** Both: in the page's own file, otherwise found through the Magazine Organization Table and fetched once per magazine (section 2 item 6).
8. **The tuning screen.** Row 0 always shows the real site and announces the tuning screen, so a fake screen in rows 1 to 24 is recognisable on every port (section 6).
9. **Which link a number follows.** The page's link, then the descriptor's alias, then the site's own page: the most specific wins (section 5).
10. **The clock in row 0.** It stays while the host fits in columns 8 to 31, and gives way when it does not (section 4).
11. **Page catching.** Swapped colours on the selected link, its destination in row 0, a key to underline every link, and pointer highlighting (section 5).
12. **Showing what the keys are doing.** Row 0 columns 0 to 7 show the current activity: page number entry, address, form, page catching or confirmation (section 4).
13. **The descriptor.** It gains three members: its well-known location, aliases, and a refresh interval for live pages on sites without push. A features list and a search page are left out (section 2 item 1).
14. **Keys.** Page Up and Page Down are Back and Forward, End is Index, Insert is Hold, Delete is Cancel, every function has a plain letter key, `K` shows the keys, and Escape never quits (section 7).

## 16. What comes next

1. **Record decisions 1 to 14 in REQUIREMENTS.md Part E**, and write the behaviour as requirements in areas WEB and NAV.
2. **Amend Appendix E:** addresses as link targets, the `://` rule, and the `LK` and `LT` lines.
3. **Amend REQ-WEB-002:** the well-known location, aliases and the refresh interval.
4. **Amend the row 0 requirements** (REQ-SEC-005 and the header rules) for web sites.
5. **Then build**, with regression tests, in the TTI reader, `neoteletext-web`, the viewer and `ttx`. The input structure of `docs/hmi.md` is the place to start for the key changes.
