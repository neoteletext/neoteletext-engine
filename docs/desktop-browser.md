# The neoteletext browser: design

A design for a teletext browser for Windows, macOS, Linux, Android and iOS, with tabs, an address bar, back and forward, bookmarks and the rest of what people expect of a browser. It browses the web-native teletext of `docs/web-native-concept.md`, as well as files and live feeds.

This is a design, not a requirement. Nothing here changes REQUIREMENTS.md until its decisions (section 15) are recorded in Part E and its behaviour becomes requirements. It follows the research of 11 October 2026 into how browsers are built (section 2, sources in section 17).

*Revised 11 October 2026:* Android and iOS joined the targets. The revision:
- moves all browser behaviour into a platform-free crate, so each platform's shell is thin (section 3.5);
- runs the decoder inside a WebAssembly interpreter on every platform, because iOS gives an ordinary app no second process (sections 3.6 and 12.8);
- compares five architectures and records the owner's choice: one Rust app on egui, with no Slint (section 9.1);
- adds the Android and iOS sandboxes, touch input, mobile layouts and packaging.

The file keeps its name because the GitHub plan links to it.

## Contents

1. Goals and constraints
2. What existing browsers teach
3. Architecture
4. The engine: what is shared and what is new
5. The tab object
6. Navigation and history
7. Addresses: parsing and display
8. Leaving a site: the tuning screen
9. The user interface
10. Input
11. What is stored
12. Security per platform
13. Packaging and updates
14. Testing and gates
15. Decisions needed
16. Phased plan
17. Sources

---

## 1. Goals and constraints

### Goals

- **A real browser for teletext.** Tabs, an address bar, back and forward, reload, home, bookmarks, history, and keyboard shortcuts, on Windows, macOS, Linux, Android and iOS from one code base.
- **The same behaviour everywhere.** Navigation, the tuning screen, address display and every security rule are one body of Rust code, tested once, on all five platforms. A platform supplies only a window, input, storage and its sandbox.
- **Everything the concept settled.** Sites, links to page numbers and addresses, aliases, the tuning screen, row 0 as location bar, page catching, presets, and the keyboard mapping of `docs/web-native-concept.md`.
- **Every input.** Web teletext sites, zxnet live feeds, and local files in every format `ttx` reads (T42, TTI, transport streams and the rest).
- **Faithful pages.** The page area is the same raster `ttx` renders: 12 by 20 cells with rounding, Levels 1 to 3.5, side panels, flash, reveal, mix, zoom, and the glyph switch between the original set and Bedstead.

### Constraints from the project

- **Security overrides everything** (decision E.32, area SEC). All input is untrusted. No executable content, ever. Row 0 columns 0 to 7 belong to the receiver.
- **The decoder never touches the network** (REQ-SEC-018); the fetcher process does networking.
- **Rust**, under the existing rules: no `unsafe` outside the C interface crate (REQ-SEC-015), strict lints, the feature rules of decision E.61, and every gate.
- **No GPL code**, and library choices need the owner's approval.
- **The library stops at the row renderer** (REQ-ARCH-010): windows, toolkits and presentation belong to the application.

### Not goals

- **No HTML.** The browser shows teletext only. An address that serves anything else gets a clear refusal page.
- **No scripts or plug-ins.**
- **No alternative web browser engine entitlement on iOS.** Apple's BrowserEngineKit gives multiple processes only to HTML engines distributed solely in the EU or Japan (section 2.12). We ship one app worldwide and design for a single process on iOS.

---

## 2. What existing browsers teach

The research covered:
- Chromium, Firefox and WebKit;
- the newer browsers Ladybird, Servo and Verso;
- the small browsers NetSurf and Dillo;
- the Gemini "small web" browsers Lagrange, Kristall and Amfora.

Each lesson below is followed by what we do with it.

### 2.1 Every serious browser separates three kinds of process

- **Chromium.** The browser process "runs the UI and manages renderer and other processes" [1]. Renderers never draw the browser's own interface. They talk to it over Mojo, which is typed interfaces with generated bindings [2]. The network service runs in its own process, and "is the default on most platforms" [3].
- **Firefox.** A parent process holds the interface, profiles and navigation. Content processes are split per site (Fission). A sandboxed socket process holds networking, with the move of all TCP and UDP there still in progress [11][12].
- **WebKit.** One WebContent process per page, "tightly sandboxed". A shared Networking process. The UI process brokers anything sensitive [14].
- **Ladybird.** A UI process, a WebContent process per tab, and RequestServer for networking, plus helper services [22][23]. Its Linux renderer sandbox is **Landlock plus seccomp** [24], exactly what `ttx` already uses.

**What we do:** three kinds of process: a trusted interface process, the existing fetcher, and sandboxed decoder processes (section 3).

### 2.2 The browser assumes the content process will be compromised

- Chromium's site isolation starts from the assumption that "determined attackers will be able to find a way to compromise a renderer process" [4]. Sites are therefore kept apart by process.
- A "site" is the scheme plus the registrable domain (eTLD+1) [4][11].

**What we do:** one decoder process per site, so a hostile page can reach only that site's pages (section 3.3).

### 2.3 Trusted state lives only in the trusted process

- In September 2026, Ladybird stopped its renderer processes linking the library that holds "bookmarks, history, settings, and session stores" [25].
- Chromium keeps history in the browser process's NavigationController [8].

**What we do:** history, bookmarks, presets, cookies, settings and session state exist only in the interface process. Decoder processes never link that code.

### 2.4 History is a flat list per tab

- Chromium's NavigationController keeps a flat list of entries per tab, "tracked as a list and not as a tree" [8].
- There is one pending entry and one committed index.
- A new navigation after going back prunes the forward entries.

**What we do:** the same model (section 6).

### 2.5 Navigations the page starts are less trusted than the user's

- Chromium treats browser-initiated navigations (address bar, bookmarks) as more trustworthy.
- For navigations a page starts, the address bar keeps showing the last committed address until the new page commits, to prevent address spoofing [9].
- When an error page or warning replaces an entry, the original entry is kept [9].

**What we do:** the same rules (section 6.3).

### 2.6 The "line of death"

- The browser owns the pixels above the page, and the page owns everything below. "If an attacker has full control of a block of pixels, he can make those pixels look like anything he wants" [20].
- Chrome removed a trusted-interface marker in Chrome 70 because pages could copy it [20].
- Chromium's security FAQ: there can be "no trustworthy security indicators elsewhere" than the top of the window [21].

**What we do:** the toolbar is our line of death, and the true site is always there. Inside the page grid, the browser owns row 0 on web sites (concept decision 2), so the page cannot write the row that names the site. The tuning screen stays inside the grid, as the concept decided, and is backed by both (section 8).

### 2.7 Address display follows written rules

Chromium's URL display guidelines [18]:
- Show the origin, not the full URL, wherever the user makes a trust decision.
- Shorten a hostname from the front, keeping the registrable domain whole.
- Omit `https://` and default ports, and mark plain HTTP with a security indicator.

Its internationalised domain name policy [19] shows a label as punycode when:
- it mixes scripts;
- it contains characters confusable with others (Unicode UTS 39);
- it is a whole-script lookalike on a top-level domain that doesn't match the script;
- its "skeleton" matches a top domain.

**What we do:** the same rules (section 7).

### 2.8 One interface toolkit everywhere

- On 28 September 2026 Ladybird removed its macOS AppKit port, because it "has fallen substantially behind the Qt port", to "focus our efforts on making the Qt port look and work well on macOS" [25].
- Chrome moved its macOS interface from native Cocoa to its own cross-platform Views toolkit [16][17].
- Servo's demo browser servoshell is built with egui and winit [26].
- Lagrange draws its whole interface itself on every platform [31][32].

**What we do:** one toolkit, egui, on all five platforms, over a platform-free core (sections 2.12, 3.5 and 9.1).

### 2.9 Front ends plug into a shared core, and a headless front end drives the tests

- NetSurf has one core and per-toolkit front ends, which plug in through operation tables.
- A headless "monkey" front end runs its integration tests.
- Its documentation warns that "attempting to port to a platform and implement a toolkit at the same time generally results in failure to achieve either goal" [28].

**What we do:** the engine is driven through one interface, the tab object (section 5). It gets a headless test driver and the existing terminal viewer as front ends before any window exists.

### 2.10 Small-web browsers show the scale and the features

Lagrange is the closest analogue to this browser:
- **Language and interface:** C with SDL, and a custom-drawn interface on every platform.
- **Features:** tabs, split view, and a sidebar of bookmarks, identities, page outline and history.
- **Distribution:** prebuilt for Windows, macOS and Linux, on Flathub, with Sparkle and WinSparkle updates [31][32].

Other small-web browsers:
- **Kristall** (Qt) and **Amfora** (terminal) offer history, favourites and feed subscriptions [33][34].
- The **Gemini protocol** recommends trusting a server's certificate on first use [30]. We use the ordinary web certificate authorities instead (section 12.5).

### 2.11 Per-platform sandboxes have known patterns

- **macOS:** XPC services each have their own sandbox and are started and stopped by the system [35]. Chromium uses Seatbelt profiles per process type, and opens resources before the sandbox closes [6].
- **Windows:** Chromium uses a broker and target, with a restricted token, a job object, an alternate desktop and Untrusted integrity. An AppContainer token without the internet-client capability blocks the network [7][36].
- **Linux and Flatpak:**
  - Flatpak turns the network off unless asked [37].
  - Its seccomp filter blocks creating namespaces. Stacked seccomp filters still work [38].
  - Landlock needs no privileges; whether it works inside Flatpak is unverified.

**What we do:** section 12.

### 2.12 Mobile: what the same browsers do on Android and iOS

**How much is shared:**
- **Chrome** shares its business logic. On iOS it cannot ship Blink: `ios/web` wraps WKWebView in place of `content/`, and the interface is Objective-C++ and Swift [39][40].
- **Firefox** writes separate mobile apps (Kotlin on GeckoView for Android, Swift on WebKit for iOS). They share Rust components, such as sync and logins, through UniFFI-generated Kotlin and Swift bindings [41][42].
- **Lagrange**, the closest analogue, ships on iOS (App Store) and Android (F-Droid and an APK) from one C code base. It draws its whole interface itself, has small per-platform shims (`android.c`, `ios.m`), and picks a phone or tablet layout at compile time [31].
- **Servo** ships servoshell for Android and OpenHarmony, but not iOS [43].
- **Ladybird** has an Android port and no iOS port [22].
- **No mobile version:** NetSurf, Dillo, Kristall and Amfora.

**Processes on Android:**
- **Chrome** runs renderers and sandboxed services as `android:isolatedProcess` services, preloaded by an app zygote, with seccomp-BPF as a second layer [44][45].
  - An isolated process has its own user ID and no permissions, and talks only through the service interface [46].
  - Chrome also declares a native-only sandboxed service. Android API level 37 added pure native services (`android:nativeService`), which need no Java runtime in the process [47].
- **Firefox** documents isolated tab processes and app zygote preloading, behind build flags [48].
- **Ladybird's** Android services are separate processes, but not isolated ones [22].
- **Servo** spawns unsandboxed child processes on Android: its sandbox code says "Sandboxed multiprocess is not supported on this platform" [49].

**Processes on iOS:**
- **Ordinary apps cannot start processes.** Apple's developer technical support states that `posix_spawn` and the other ways of launching a child process are not supported on iOS [50]. Writable executable memory, and so just-in-time compilation, is reserved to Apple [51].
- **BrowserEngineKit** gives a browser multiple processes (content, network and rendering extensions over XPC) and just-in-time compilation [52][53]. It is only for engines that:
  - are web browsers in the HTML sense;
  - pass 90% of the Web Platform Tests;
  - are distributed solely in the EU (iOS 17.4 and later) or solely in Japan (iOS 26.2 and later).
- **Chrome and Firefox on iOS** both use WebKit for this reason.
- **Lagrange on iOS** is a single process [31].

**What we do:**
- **Android:** the same three kinds of process as the desktop, with the decoder as an isolated service, as Chrome does (section 12.6).
- **iOS:** one process, with the decoder walled off by a WebAssembly interpreter, which needs no second process and no just-in-time compilation (section 12.7). The interpreter runs on the other four platforms too, so the decoder's wall is the same code everywhere (section 3.6).
- **Following Lagrange and NetSurf:** one engine with thin per-platform shims, and a layout chosen for phones and tablets.

---

## 3. Architecture

### 3.1 Processes

```
 ┌───────────────────── interface process (trusted) ─────────────────────┐
 │ windows, tabs, toolbar, address bar, menus, dialogs, accessibility     │
 │ navigation controllers (one per tab), history, bookmarks, presets,     │
 │ cookies, settings, session restore, address display policy             │
 │ draws: the browser chrome; presents decoder frames                     │
 └────────┬───────────────────────────────────────────┬───────────────────┘
          │ fetch requests / page bytes                │ commands / frames + events
          │ (framed pipe, crates/neoteletext-app/pipe.rs) │ (framed pipe, new message kinds)
 ┌────────▼─────────────────┐                ┌────────▼──────────────────────────┐
 │ fetcher process          │                │ decoder process, one per site      │
 │ HTTP(S), WebSocket, SSE, │  page bytes →  │ format readers, decoder, store,    │
 │ zxnet, TLS, redirects    │  (via the      │ Levels 1–3.5, forms model, render  │
 │ network only             │  interface)    │ no network, no files, no state     │
 └──────────────────────────┘                └────────────────────────────────────┘
```

| Process | Holds | May access |
|---|---|---|
| **Interface** | Everything the user trusts: what site is shown, history, bookmarks, cookies, settings | Its own data directory; the display; spawning the other processes |
| **Fetcher** | Network connections and TLS | The network only. It sees bytes but never decodes them. |
| **Decoder** | One site's pages: decoding and rendering | Nothing: no network, no files. It receives bytes and commands and returns frames and events. |

### 3.2 Why the bytes pass through the interface process

The fetcher does not talk to the decoders directly. The interface process sends each fetched resource to the decoder of the site it belongs to, because only the interface process knows:
- which site a tab is on;
- which redirects were confirmed;
- which decoder belongs to which site.

The interface process only routes the bytes; it never parses them.

### 3.3 Decoder processes per site

- **A site** is the scheme plus the registrable domain (section 7.2).
- **Tabs share a decoder** when they show the same site.
- **Leaving a site** sends the tab to that site's decoder, starting one if needed. The decoder of the previous site keeps its stored pages while any tab or history entry still uses it, then exits.
- **Files and broadcast captures** each get their own decoder, named by the file.
- **First version:** one decoder per tab is simpler and almost as safe. Moving to one per site is a later step, because the protocol of section 3.4 is the same either way.

### 3.4 Messages between processes

The existing framed pipe protocol (`crates/neoteletext-app/src/pipe.rs`: a kind byte, a 32-bit length and a payload, strictly parsed with bounded lengths) gains message kinds for the decoder.

**Interface to decoder:**

| Message | Meaning |
|---|---|
| `Resource { id, url, media_type, bytes }` | A fetched page file or bundle for this site |
| `PushData { feed, bytes }` | Bytes from a live feed |
| `Show { tab, page, subcode }` | Make this page the tab's current page |
| `Key { tab, function }` | A function from section 10, after the interface's own handling |
| `Tick { now }` | The time, for flash and timeouts |
| `Settings { level, profile, glyphs, reveal, mix, zoom, … }` | Display settings |
| `Close { tab }` | Forget the tab's state |

**Decoder to interface:**

| Message | Meaning |
|---|---|
| `Frame { tab, width, height, palette, indices }` | A rendered page: 8-bit indices (480 by 500, or wider with side panels) and its palette. About 240 KB. Sent only when the page changes or flash changes phase. |
| `Links { tab, keys, catches, aliases }` | The page's links: Fastext keys, page-catching targets, page links. The interface needs them to show the destination before following. |
| `Wants { tab, page }` | A page the decoder needs: following a link, or an enhancement dependency (section 4.2) |
| `Form { tab, form }` | The page has a form; its fields and destinations |
| `Status { tab, … }` | Searching, found, stale, the page title from row 0 for the tab |

Constraints on these messages:
- **Every message is bounded.**
- **The interface checks everything a decoder says.** For example, a `Wants` naming another site is a cross-site navigation, so it goes through the tuning screen, not straight to the fetcher.
- **A decoder that sends a malformed message is closed** and its tabs show an error page.

### 3.5 Where the code lives

| Crate | Change |
|---|---|
| `neoteletext-core` | The input router of `docs/hmi.md`. Navigation stays in the viewer's existing state machine where it is per page; history moves to the tab model (section 6). |
| `neoteletext-web` | The concept's link targets (`LK`, `LT`, addresses), descriptor aliases and well-known location, bundles, enhancement dependencies. The address display policy (section 7) lives here too, because it is security policy (`core::policy` holds the rules; the data source is a decision). |
| **`neoteletext-browser`** (new) | Everything the browser decides, with no operating-system access, like the library crates: the tab object (section 5), the navigation controller (section 6), history, bookmarks and presets as data, the tuning screen, the chrome's state (what the address bar, tab strip and buttons show), and the decoder protocol of section 3.4 as a sans-IO state machine. It compiles for every target, including iOS and Android. |
| **`neoteletext-decoder-wasm`** (new) | The decoder of section 3.1 built as a WebAssembly module (`wasm32-unknown-unknown`, `no_std`), exporting only the messages of section 3.4 over its linear memory. |
| `neoteletext-app` | The platform side: spawning and sandboxing processes, the WebAssembly host (section 3.6), storage, the secret store, and the new pipe message kinds. Each platform's sandbox sits behind one trait. |
| `neoteletext-cli` | `ttx view` becomes a front end of the tab object, so the terminal viewer gains tabs, history across sites and the tuning screen. |
| **`neoteletext-gui`** (new) | The egui interface (section 9): draws the chrome from `neoteletext-browser`'s state and presents frames. One crate for all five platforms. |
| **`platform/android`, `platform/ios`** (new) | The thin native shells: a Gradle project and an Xcode project, each a few hundred lines of Kotlin or Swift. They hold what Rust cannot do on that platform: the app entry point, the Android service stubs (section 12.6), the system text field and keyboard where egui falls short (section 9.1), and the share and file-open hooks. They contain no browser behaviour. |

Notes:
- **The shells and the toolkit crate never decide anything.** A rule about trust, navigation or display that lived in Swift or Kotlin would have to be written and tested twice. Every such rule lives in `neoteletext-browser`.
- **The layer table of `ARCHITECTURE.md`** gains `neoteletext-browser` at L8 beside `neoteletext-app`, and `neoteletext-gui` at L9.

### 3.6 The process model on each platform

The decoder handles untrusted data, so it is the part to wall off. It gets two walls where the platform allows and one where it does not:

| Platform | Interface | Fetcher | Decoder |
|---|---|---|---|
| Linux | Main process | Its own process, Landlock and seccomp | Its own process, Landlock and seccomp, **and** inside the WebAssembly interpreter |
| macOS | Main app, App Sandbox | XPC service with the network entitlement only | XPC service with no entitlements, **and** the interpreter |
| Windows | Main process | AppContainer with the internet-client capability | AppContainer without it, low integrity, job object, **and** the interpreter |
| Android | Main app process | A `:fetcher` service process | An isolated service process with seccomp, **and** the interpreter |
| iOS | The app | Inside the app process | Inside the app process, **inside the interpreter only** |

**The WebAssembly wall:**
- **The interpreter is wasmi** [54]: no just-in-time compilation (so it is allowed on iOS), `no_std`, MIT or Apache licensed, with fuel metering and limits on memory and tables.
- **The module is our own decoder, compiled into the signed app.** Nothing is downloaded, and no page ever supplies code: the "no executable content" rule is unchanged. App Review guideline 2.5.2 forbids code that is downloaded or changes the app's features [55]. A bundled interpreter running bundled code is the case older Apple terms allowed by name [56]. That the current terms still allow it is likely but not confirmed.
- **What it gives:** a decoder bug, even one that corrupts the decoder's memory, stays inside the module's linear memory. The module imports nothing but the message channel, so it has no way to reach the network, files or the interface's state.
- **What it costs:** the interpreter is slower than native code. A teletext page is 1,000 cells, so this should be small, but it must be measured (section 14) on the slowest phone we support.
- **Why on every platform:** the wall is then one piece of code, tested and fuzzed everywhere, rather than a special case on iOS. On four platforms it sits inside an operating-system sandbox as well.
- **Audit:** wasmi was audited at versions 0.31 and 0.36 to 0.38. Version 2.0 has not been audited. Our fuzzing of the host side covers our use; an outside audit is a release question (section 15).

**Messages are the same everywhere.** The decoder protocol of section 3.4 is carried over a pipe between processes, or over the module's memory inside one process. `neoteletext-browser` checks every decoder message in the same way in both cases.

---

## 4. The engine: what is shared and what is new

### 4.1 Already built

- **Decoding:** every format reader, the decoder and page store, Levels 1 to 3.5, and the renderer with the original and Bedstead glyphs.
- **Web:** the sans-IO web user agent (HTTP, WebSocket, server-sent events, cookies, forms, descriptors) and the zxnet feed.
- **Processes:** the fetcher process, the framed pipe, and the Linux sandbox.
- **Viewing:** the viewer state machine (page entry, Fastext, sub-pages, hold, reveal, mix, zoom) and the forms model.

### 4.2 To build: the web-native model

This comes from `docs/web-native-concept.md` sections 2 to 9, and needs its decisions recorded as requirements first.

- **Sites:**
  - the descriptor at `/.well-known/teletext.json`, or the defaults `/index.t42` and `/{mpp}.t42` without one;
  - the descriptor's aliases and refresh interval.
- **Resources:**
  - a page file holds its page only;
  - the front door may be a whole-site bundle;
  - a typed number is looked up first in the store, then in the page's own file.
- **Enhancement dependencies:** fetch the Magazine Organization Table and the object and DRCS pages it names, once per magazine.
- **Links:**
  - TTI `LK` and `LT` lines, and addresses as targets (the `://` rule);
  - page catching;
  - which link a typed number follows: the page's own link, then the alias, then the site's page.
- **Row 0 as the location bar:** the browser writes the whole row on web sites; it shows the current activity in columns 0 to 7 and the host in the rest.
- **The tuning screen** (section 8).

### 4.3 To build: the input router

From `docs/hmi.md`:
- one `Function` enum;
- press, repeat and release;
- binding tables as data;
- the router owns the modes (browse, number entry, address entry, form, confirm, page catching), so every front end behaves alike;
- pasted input never confirms.

---

## 5. The tab object

The one interface every front end drives: the headless test driver, `ttx view` and the app on every platform. The tab object lives in `neoteletext-browser` and runs in the interface process. It holds the tab's navigation controller and talks to the fetcher and decoder.

```rust
/// One browser tab (interface process).
pub struct Tab { /* navigation controller, current site, decoder handle, state */ }

impl Tab {
    /// Starts a navigation the user asked for: an address, `host page`, a
    /// bookmark, a preset. Browser-initiated (section 6.3).
    pub fn open(&mut self, target: Target) -> Result<(), OpenError>;
    /// A function from the input router (section 10).
    pub fn input(&mut self, function: Function, now: Instant);
    /// Back and forward through this tab's history (section 6).
    pub fn back(&mut self);
    pub fn forward(&mut self);
    pub fn reload(&mut self);
    pub fn stop(&mut self);
    /// Advances time: flash, timeouts, refresh intervals.
    pub fn tick(&mut self, now: Instant);
    /// Drains events for the front end.
    pub fn events(&mut self) -> impl Iterator<Item = TabEvent> + '_;
    /// The current frame, if it changed since the last call.
    pub fn frame(&mut self) -> Option<&Frame>;
    /// What the address bar shows: the committed address, or the typed one.
    pub fn location(&self) -> Location;
}

pub enum TabEvent {
    /// The committed address changed: update the address bar and the tab title.
    Committed { location: Location },
    /// Loading started or stopped; the requested page and how long.
    Loading { page: PageAddress, since: Instant },
    Loaded,
    /// The tuning screen is shown and waits for confirmation (section 8).
    Tuning { destination: Location },
    /// The page could not be found or fetched; an error page is shown.
    Failed { reason: Failure },
    /// The security state: encrypted, plain HTTP, offline copy.
    Security { state: SecurityState },
    /// A form on the page wants confirmation before sending (REQ-SEC-009).
    ConfirmSubmission { destination: Location },
    /// The tab's title: the site name and page number.
    Title { text: String },
}
```

The front end calls `tick` and `frame` on its own timer, sends user input through `input`, and redraws the chrome from `events`. It never decodes anything. The headless test driver (section 14) does exactly the same.

---

## 6. Navigation and history

### 6.1 The navigation controller

Each tab has one, modelled on Chromium's [8][9]:
- **Entries:** a flat list of `Entry { site, page, subcode, title, scroll of sub-pages, decoded page snapshot }`.
- **Committed index:** the entry shown.
- **Pending entry:** a navigation in progress, either new (index −1) or to an existing entry (back or forward).
- **New navigations** after going back remove the forward entries.
- **Limit:** a fixed number of entries per tab (REQ-NAV-010 says 32 by default), oldest dropped first.

### 6.2 What counts as a navigation

| Action | New entry? |
|---|---|
| Typing a page number, following a link, page catching, opening an address, a bookmark or a preset | Yes |
| Next and previous page | Yes, as on a television, where each page step is a page change |
| Changing sub-page, automatic sub-page rotation | No: the entry's sub-page is updated |
| A page updated in place by a live feed | No |
| Reload | No: the same entry is fetched again |
| A form response page (REQ-WEB-011) | Yes; Back returns to the form with its values |

### 6.3 Trust of navigations

**Navigations the user starts:**
- **Which:** the address bar, bookmarks, presets, Back and Forward, Home.
- **How they're handled:** shown in the address bar as typed while pending.

**Navigations the page starts:**
- **Which:** Fastext keys, page catching, typed numbers that follow a page link, and redirects.
- **How they're handled:**
  - The address bar keeps showing the committed address until the new page commits [9].
  - Any that leave the site go through the tuning screen (section 8).

**Error pages and the tuning screen:**
- They replace the pending entry's display without losing it.
- Cancelling the tuning screen returns to the committed entry unchanged.

### 6.4 The back-forward cache

- **Each entry keeps its decoded page snapshot,** so Back and Forward show the page at once, before any network check. The concept's rule still holds: show at once, then check it is current (REQ-WEB-007).
- **Live push feeds** (WebSocket, server-sent events) are closed when the user leaves the entry's page and reopened when they return [10]. A held page (Hold) keeps its snapshot frozen.

### 6.5 Going back across sites

**Decided** (concept decision 4): going back, or forward, to a site already confirmed in the session skips the tuning screen.

---

## 7. Addresses: parsing and display

### 7.1 What the address bar accepts

- **Accepted:**
  - an ordinary address, such as `https://weather.example/300.t42`;
  - an address with no page, which opens the site's initial page;
  - a host and page number, such as `weather.example 300`;
  - a bare page number, for the current site;
  - a preset name or number;
  - a `teletext:` link handed over by the operating system;
  - a local file, dropped or opened.
- **Not accepted:** `javascript:`, `data:` or any scheme other than `http`, `https`, `wss`, `zxnet` and `file`.
- **Normalisation:** everything typed is turned into an ordinary address before the navigation starts. The address bar then shows that address.

### 7.2 Site and registrable domain

The registrable domain is the public suffix plus one label, from the Public Suffix List. Where the list comes from is a decision (section 15): a maintained crate, or the list fetched and pinned as data. A site is the scheme plus the registrable domain.

### 7.3 Display rules

| Rule | Source |
|---|---|
| Show the site, not the full address, wherever a trust decision is made: the address bar while not editing, the tab title, the tuning screen | Chromium [18] |
| Keep the registrable domain whole, shortening the hostname from the front with an ellipsis | [18] |
| Omit `https://` and default ports; show a plain-HTTP indicator instead of the scheme (decision E.32 allows plain HTTP) | [18] |
| Show a label as punycode when it mixes scripts, contains confusable characters, or its skeleton matches a well-known domain | Chromium IDN policy [19] |
| Show the full address only while editing (click or ⌘L / Ctrl+L) | [18] |
| Row 0 inside the page applies the same rules within its 24 to 32 characters | concept section 4 |

### 7.4 Security indicators

The toolbar shows:
- **Encrypted** (a closed padlock);
- **plain HTTP** (an open padlock in the warning colour);
- **offline copy** ("saved at 14:32");
- **local file.**

Row 0 columns 0 to 7 show the receiver's matching indicator, as now (REQ-SEC-005).

---

## 8. Leaving a site: the tuning screen

**Decided** (concept section 6, decisions 2, 4 and 8). The tuning screen stays inside the page area, drawn by the browser in rows 1 to 24, with the browser-owned row 0 above it. It is not a separate dialog.

### 8.1 What it shows

- **The destination:**
  - the registrable domain shown large, in double height;
  - the full address wrapped below it;
  - the descriptor's service name when the link names a service, or "not declared by the site" when the address was written in a page.
- **The encryption state** of the destination.
- **The actions:** Enter or F4 confirms; Delete or Escape cancels.

### 8.2 Why it cannot be forged

Inside the page area:
- **Row 0 belongs to the browser** on web sites (concept decision 2). While the tuning screen is up, row 0 shows `GO?` in columns 0 to 7 and the destination in the rest. A page cannot write row 0, so a fake tuning screen drawn in rows 1 to 24 still has the real site and no `GO?` above it.

Outside the page area, which the desktop has, as the line of death [20][21]:
- **The address bar** keeps showing the committed site (section 6.3).
- **A destination strip** appears in the toolbar, drawn by the interface process and outside the page's pixels, while the tuning screen waits. It repeats the destination and the same two actions.
- **In fullscreen,** a permanent badge with the current site stays at the top edge. Fullscreen removes every other trusted pixel [20].

The rule for viewers stays short: the site is whatever row 0 and the address bar say.

### 8.3 Confirmation rules

- **Only the interface accepts the confirmation.** A key arriving from pasted text never confirms (`docs/hmi.md`).
- **Typed addresses and unfamiliar sites** always ask.
- **Back or forward to a confirmed site** does not ask (decision 4).
- **A redirect to another host** updates the destination and asks again before any content from the new host is shown (concept section 6).

---

## 9. The user interface

### 9.1 Toolkit

The page area is our own raster, so the toolkit draws only the chrome: tabs, buttons, an address field, menus and lists. What separates the toolkits on five platforms is the phone: the on-screen keyboard and text editing, screen readers, safe areas and the app lifecycle.

**The state of each option, October 2026:**

| Option | Desktop | Android | iOS | Text input on phones | Screen readers on phones | Licence |
|---|---|---|---|---|---|---|
| **egui and eframe** 0.36 [57] | Yes | Supported | Not supported officially; third-party ports only | Weak: winit's soft-keyboard and input-method work is unfinished [58][59] | AccessKit now has Android and iOS back ends [60]; eframe's use of them on phones is unverified | MIT or Apache |
| **Slint** 1.18 [61] | Yes | Since 1.5 | Since 1.12 | Keyboard, input method and safe-area fixes through 1.18 | An accessibility layer; phone coverage unverified | GPLv3, or royalty-free with conditions, or commercial [62]; **rejected by the owner** |
| **Native per platform** over a Rust core (Firefox's model) [41] | Five interfaces | Kotlin | Swift | Native, complete | Native, complete | UniFFI is MPL-2.0 [63] |
| **Flutter** with flutter_rust_bridge | Yes | Yes | Yes | Native, complete | TalkBack and VoiceOver | BSD and MIT; adds the Dart runtime and Flutter's C++ engine to the trusted code |
| **Compose Multiplatform** | On the JVM | Yes | Yes | Native | Unverified | Adds Kotlin and, on desktop, a JVM |
| **Tauri, Dioxus** | | | | | | Rejected: they put the system web view, a full HTML and JavaScript engine, inside the trusted process |
| **Iced, Makepad** | | | | | | Rejected: no iOS, or iOS not a standard target |

**Decided by the owner on 11 October 2026: one Rust app with egui on all five platforms, and no Slint.** Of the five architectures compared that day, this is option 1:
- one interface codebase in Rust (`neoteletext-gui`) on egui and eframe;
- thin Swift and Kotlin shells on the phones (section 3.5);
- the other four were rejected:
  - native interfaces per platform, for cost and drift;
  - an interface drawn as teletext, for the text-editing and accessibility work it needs;
  - Flutter, for the outside code in the trusted process;
  - a web app inside the user's browser, because cross-origin rules, cookie blocking and the lack of raw TCP break the open web of teletext sites.

**egui's gaps on phones, and how the shells fill them.** egui is supported on Android and not officially on iOS, and its phone text input is weak (the table above). The shells fill each gap without holding any browser behaviour:

| Gap | Filled by |
|---|---|
| eframe has no official iOS entry point | The iOS shell creates the window and the Metal layer and runs eframe's renderer in it, as the third-party iOS ports do. |
| The on-screen keyboard and text input | While the address bar or a form field is being edited, the shell places a system text field (`UITextField`, Android `EditText`) over it. It passes the text back to `neoteletext-browser`, which validates it like any typed input. This gives selection, paste, autocorrect control and input methods for every script. Pasted input still never confirms (`docs/hmi.md`). |
| Screen readers | AccessKit's Android and iOS back ends, wired by the shell if eframe does not wire them itself. |
| Safe areas, suspend and resume, rotation | eframe's own support (0.31 and 0.33), checked by the test app. |
| The system back gesture, the share sheet, opening files | The shell, which turns each into an input-router function or an open-file event. |

**The phone test app** proves this before phase 6 builds on it. It is a small egui app on an Android phone and an iPhone (and their emulators): a tab strip, an address field, four Fastext buttons and a page area showing a frame from `neoteletext-browser`. It must pass every one of these:
- the on-screen keyboard opens, edits the address with selection and paste, and closes, with no crash;
- an input method for a non-Latin script composes text in the address field;
- VoiceOver and TalkBack read the buttons, the address and the page text;
- the app is suspended and resumed, and rotated, without losing state;
- it respects the notch and the home indicator;
- its added size and the time to show the first frame are measured;
- `cargo-deny` passes on egui's dependency tree.

A criterion that egui and the shell together cannot meet comes back to the owner as a decision. It is not worked around.

### 9.2 Window layout

```
┌────────────────────────────────────────────────────────────────────┐
│ [ CEEFAX 100 ×] [ weather.example 300 ×] [ + ]                     │  tab strip
├────────────────────────────────────────────────────────────────────┤
│ ◀ ▶ ⟳ ⌂  🔒 weather.example  300             ★  [■][■][■][■]  ☰   │  toolbar
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│                 ┌──────────────────────────────┐                   │
│                 │  P300  weather.example  14:32│ ← row 0 (browser) │
│                 │                              │                   │
│                 │       the teletext page      │                   │
│                 │       (480 × 500 × scale)    │                   │
│                 │                              │                   │
│                 │ Red     Green  Yellow  Cyan  │ ← row 24          │
│                 └──────────────────────────────┘                   │
├────────────────────────────────────────────────────────────────────┤
│ Searching P300…                     Level 3.5 · original glyphs    │  status line
└────────────────────────────────────────────────────────────────────┘
```

- **Tab strip:**
  - drawn by the toolkit, since Windows has no system tab strip and browsers draw their own;
  - a title per tab (site name or file name, and page number), a loading indicator and a close button;
  - drag to reorder, and drag out to a new window.
- **Toolbar:**
  - Back, Forward, Reload/Stop, Home;
  - the security indicator and address bar (section 7);
  - a bookmark star;
  - the four Fastext buttons in their colours, needed on Macs, where F1 to F4 control brightness;
  - a menu button.
- **The page area:**
  - the decoder's frame as a texture with nearest-neighbour filtering;
  - whole-number scaling that fits the window;
  - optional 4:3 aspect stretch (REQ-RND-004);
  - centred on black, with the page's own border colour at Levels 2.5 and 3.5;
  - wider pages with side panels fit as wide.
- **Status line:** search progress, the presentation level, the glyph set, and errors.

**Phones**, in the pattern Safari and Chrome use on phones, and Lagrange's compile-time phone layout [31]:

```
┌──────────────────────────┐
│ ┌──────────────────────┐ │
│ │ P300 weather.ex 14:32│ │ ← row 0 (browser)
│ │                      │ │
│ │   the teletext page, │ │
│ │   whole-number scale │ │
│ │   in device pixels   │ │
│ │                      │ │
│ │ Red Green Yellow Cyan│ │ ← row 24
│ └──────────────────────┘ │
│ [red] [grn] [yel] [cyn]  │  Fastext bar
│ 🔒 weather.example  300   │  address bar (bottom, in thumb reach)
│ ◀  ▶  123  ⧉ 2  ☰        │  back, forward, number pad, tabs, menu
└──────────────────────────┘
```

- **The page** fills the width at the largest whole-number scale in device pixels. A phone 1,170 pixels wide shows the 480-pixel page at twice its size.
- **The address bar** sits at the bottom. The true site is always shown there, so it remains the line of death (section 2.6).
- **Tabs** are a grid of page thumbnails, opened from the tab button.
- **The number pad** is a sheet of ten digits plus Index, Hold and Reveal, so page numbers never need the system keyboard.
- **Landscape and tablets** use the desktop layout.

### 9.3 Menus

| Menu | Items |
|---|---|
| **File** | New Tab, New Window, Open File…, Open Address…, Close Tab, Save Page As (PNG, TTI, T42), Print |
| **Edit** | Copy page text (UTF-8, as `ttx text`), Copy address, Find in page |
| **View** | Reveal, Mix, Zoom (top, bottom, off), Hold, Flash on/off, Aspect 4:3, Glyphs (Original, Bedstead, Compare), Presentation level (1, 1.5, 2.5, 3.5), Profile, Actual size, Zoom in/out, Fullscreen, Show links (page catching) |
| **Go** | Back, Forward, Home, Index, Next/Previous page, Next/Previous sub-page, Enter page number, Presets |
| **Bookmarks** | Bookmark this page, Show bookmarks, the bookmarks themselves |
| **History** | Recently closed tabs, the history list |
| **Window / Help** | Standard; Help shows the keys page of section 10 |

On macOS the menus live in the system menu bar, through a menu crate if needed (section 15). On Linux and Windows they live in the window.

### 9.4 Mouse and touchpad

- **Click a page number on the page:** page catching (concept section 5). The link under the pointer is highlighted, and its destination shown in the status line, as browsers show a link's address on hover.
- **Click a Fastext prompt in row 24:** follow that link.
- **Scroll wheel or swipe:** next and previous page. Two-finger swipe left and right means Back and Forward on macOS.
- **Drag a T42 or TTI file onto the window:** open it in a new tab.

**Touch** (phones and tablets; these go through the input router of `docs/hmi.md`, which gains a touch device class):
- **Tap a page number on the page:** page catching, with the destination shown in the address bar before it is followed.
- **Tap a Fastext prompt in row 24,** or a button of the Fastext bar: follow that link.
- **Swipe left or right on the page:** next or previous page.
- **The system back gesture** (Android's back, the iOS edge swipe): Back.
- **Long press on the page:** reveal while held.
- **Pinch:** zoom by whole-number steps.
- **Open a `.t42` or `.tti` file** from the Files app or a share sheet: a new tab.

### 9.5 Other screens

All are drawn as teletext pages by the interface, in the same page area, so the browser keeps one look:
- **New tab:** bookmarks, presets and recently visited sites.
- **Error pages.**
- **The keys page.**

### 9.6 Accessibility

The toolkit reports its widgets to the platform: VoiceOver on macOS and iOS, Narrator on Windows, Orca on Linux, and TalkBack on Android. egui does that through AccessKit, whose phone back ends are new (section 9.1), so the test app checks them. The page area adds:
- the page's text, one row per line, from the same conversion `ttx text` uses;
- the links as buttons with their labels;
- announcements when the page changes.

---

## 10. Input

The keyboard mapping is the concept's, decided (`docs/web-native-concept.md` section 7). Every function has a plain letter key, and Escape never quits. The desktop adds the platform conventions:

| Function | macOS | Windows and Linux |
|---|---|---|
| New tab / close tab | ⌘T / ⌘W | Ctrl+T / Ctrl+W |
| Reopen closed tab | ⌘⇧T | Ctrl+Shift+T |
| Next / previous tab | ⌘⇧] / ⌘⇧[, ⌃Tab | Ctrl+Tab / Ctrl+Shift+Tab |
| Address bar | ⌘L | Ctrl+L, Alt+D |
| Back / forward | ⌘[ / ⌘], swipe | Alt+Left / Alt+Right |
| Reload | ⌘R | F5, Ctrl+R |
| Bookmark | ⌘D | Ctrl+D |
| Fullscreen | ⌃⌘F | F11 |
| Zoom | ⌘+ / ⌘− / ⌘0 | Ctrl++ / Ctrl+− / Ctrl+0 |

Notes:
- **Keys go through the input router** of `docs/hmi.md`, so number entry, address entry, forms, page catching and confirmation behave as in `ttx`.
- **The toolkit must report key presses and releases,** so holding Index for Back (REQ-NAV-010) works; the test app checks this.
- **The numeric keypad** works as in the concept: digits with Num Lock on, navigation with it off.
- **Phones and tablets with a hardware keyboard** use the same keys. iPadOS reserves some ⌘ shortcuts, which the test app checks.
- **Touch** is described in section 9.4.

---

## 11. What is stored

All of it is in the interface process only, in the platform's application data directory:
- **macOS:** the app container.
- **Linux:** `$XDG_DATA_HOME/neoteletext`.
- **Windows:** `%APPDATA%\neoteletext`.
- **Android:** the app's internal storage, which no other app can read.
- **iOS:** the app container, with data-protection class "complete until first user authentication" or stronger.

Cookies go to the platform's secret store: Keychain on macOS and iOS, Credential Manager on Windows, Secret Service on Linux, and a key held in the Android Keystore that encrypts them on Android.

| Data | Rules |
|---|---|
| Bookmarks and presets | Site, page and sub-page, a title, and a preset number |
| History | Site, page, time. Clearable. Kept for a set number of days |
| Session | Open windows and tabs, each tab's history, to restore after a restart |
| Cookies | REQ-WEB-006 and REQ-SEC-013: stored as secrets where the platform offers a store (Keychain, Credential Manager, Secret Service), and wipeable |
| Confirmed sites | The sites confirmed this session, for decision 4 (not stored between runs unless the owner decides otherwise) |
| Settings | Level, profile, glyphs, region, rounding, aspect, keys |

Decoders store nothing, and their page stores live only in memory.

---

## 12. Security per platform

### 12.1 Rules on every platform

- **The interface process starts the fetcher and the decoders** with the least access possible.
- **Each process opens what it needs before its sandbox closes,** as Chromium does on macOS [6] and `ttx` already does on Linux.
- **The interface process validates every message** from the other processes (section 3.4). A process that breaks the protocol is closed.

### 12.2 Linux

- **Landlock and seccomp,** as `ttx` has today, on the fetcher and the decoders.
- **The decoder's seccomp filter** allows no sockets, and Landlock allows no file access.
- **In Flatpak:**
  - the application gets the network permission, because the fetcher needs it;
  - our own seccomp filters stack under Flatpak's;
  - whether Landlock works inside Flatpak's sandbox is unverified, and must be tested before relying on it [37][38].

### 12.3 macOS

- **Processes as XPC services** inside the app bundle (`Contents/XPCServices`), each with its own sandbox, started by the system on demand [35]:
  - **fetcher service:** the outgoing-network entitlement only;
  - **decoder service:** no entitlements at all.
- **The interface app:** the App Sandbox, with user-selected file access (open and save) and no network.
- **Alternative:** helper executables with Seatbelt profiles, as `ttx`'s current macOS behaviour would extend to. Simpler to build and test, but not the platform's own pattern.

### 12.4 Windows

- **Decoders** run with an AppContainer token without the internet-client capability, in a job object, at low integrity, which is Chromium's pattern [7][36].
- **The fetcher** runs with an AppContainer token with the internet-client capability.
- This covers the Phase 2 gap of REQ-SEC-017 for Windows.

### 12.5 TLS and certificates

- **Ordinary web certificate authorities,** as `ttx` uses now (rustls with the Mozilla root list).
- **No trust-on-first-use,** unlike Gemini [30]: teletext sites are ordinary web servers.
- **A certificate error** shows an error page, with no way to click through by default.

### 12.6 Android

Following Chrome's Android sandbox [44][45]:
- **The interface** is the app's main process. It holds the internet permission only because the manifest is per app; the interface itself never opens a socket, and the fetcher does all networking.
- **The fetcher** is a service in its own process (`android:process=":fetcher"`).
- **Decoders** are services with `android:isolatedProcess="true"` [46]:
  - each has its own user ID and no permissions, and cannot open files or sockets of the app;
  - each is bound with `bindIsolatedService`, which needs API level 29, so that is the minimum version;
  - each installs a seccomp filter on start, as Chrome does, before it reads anything;
  - each runs the decoder inside the WebAssembly interpreter (section 3.6).
- **Rust in services:** from API level 37 a decoder can be a pure native service [47]. Below 37, each service is a stub of a few dozen lines of Kotlin that loads the Rust library and hands it a file descriptor for the pipe. The stub holds no logic.
- **The pipe** is the same framed protocol as on the desktop, over a socket pair passed through Binder.

### 12.7 iOS

- **One process** (section 2.12). Within it:
  - **the decoder** runs only inside the WebAssembly interpreter, which is its wall (section 3.6);
  - **the fetcher** is ordinary Rust in the app, running on its own thread, with the existing bounded parsers;
  - **the app** runs in the iOS app sandbox, which every App Store app has, with only the outgoing-network capability and user-chosen files.
- **No just-in-time compilation** is needed or requested.
- **What is lost:** if a bug escaped the interpreter, the attacker would hold the whole app: the network, cookies and history. On the other four platforms the operating-system sandbox would still stop them.
- **Why that is acceptable** (an owner decision, section 15):
  - the decoder is safe Rust with no `unsafe`, no allocation in the core, and fuzzing;
  - the interpreter is a second, independent wall;
  - pages contain no executable content;
  - Chrome and Firefox on iOS rely on WebKit's processes only because Apple supplies them; no third-party app can have them outside BrowserEngineKit.

### 12.8 The WebAssembly host on every platform

- **The host gives the module no imports** except reading the next message and writing a reply into its own memory.
- **Fuel:** each message has a fuel budget, so a page that would loop forever stops with an error page.
- **Memory:** the module's linear memory has a fixed maximum, set from the decoder's measured need with a margin; growth beyond it fails.
- **Every value read out of the module is checked** as if it came from another process: lengths, indices and message kinds are bounded before use.
- **Fuzzing:** the host's message handling and the module's input both get fuzz targets, run in the full gate.

---

## 13. Packaging and updates

| Platform | Package | Signing | Updates |
|---|---|---|---|
| macOS | `.app` (universal: Apple Silicon and Intel) in a disk image | Developer ID, hardened runtime, notarisation | Sparkle, as Lagrange uses [31], or none at first |
| Windows | MSI or MSIX installer | Authenticode certificate | WinSparkle [31], or the MSIX store channel |
| Linux | Flatpak (Flathub), AppImage, `.deb` | Flathub's process; signed AppImage | Flathub, or AppImage update information |
| Android | App bundle for Google Play; an APK for F-Droid and direct download, as Lagrange does [31] | Play app signing; our own key for the APK | The store |
| iOS and iPadOS | App Store, and TestFlight for testing | The same Apple Developer Program membership as macOS | The store |

The app stores update their apps themselves, so Sparkle and WinSparkle stay desktop-only. App Store and Play review add a step to every release, which the release phase allows for.

Also:
- **The `teletext:` link handler** and file associations for `.t42` and `.tti` are registered by each package.
- **Release builds** are reproducible and carry a software bill of materials (REQ-SEC-014).

---

## 14. Testing and gates

**Engine:**
- Every new part of the engine (web-native model, navigation controller, address display, input router) gets unit and property tests and fuzz targets, like the rest of the library.
- New fuzz targets: address parsing and display, descriptor aliases, and the decoder protocol messages.

**Headless test driver** (NetSurf's "monkey" [28]):
- It drives `Tab` objects from a script against the existing local test server (REQ-WEB-013).
- It covers:
  - every navigation of section 6;
  - the tuning screen, including redirects;
  - forms;
  - back and forward across sites;
  - a decoder crashing.

**Multi-process tests:**
- the interface, fetcher and decoder started as real processes;
- a decoder that sends malformed messages;
- a decoder that tries the network or the file system, which the sandbox must refuse, on each OS.

**Interface tests:**
- the toolkit's test harness renders the interface without a display and compares snapshots of the chrome, in the desktop and phone layouts;
- a smoke test per platform opens a window, loads a page and checks the frame.

**WebAssembly wall tests:**
- a decoder module built to misbehave (writing out of bounds, looping, growing memory, sending malformed replies) must be stopped by the host on every platform;
- the decoder's speed inside the interpreter is measured against native, on this Mac and on the slowest supported phone, and reported by the gate.

**Gates:**
- `scripts/check.sh` gains the new crates, and builds `neoteletext-browser` and the decoder module for every target, including `aarch64-apple-ios`, `aarch64-apple-ios-sim` and `aarch64-linux-android`.
- Each platform builds and runs its tests:
  - macOS, the iOS simulator and the Android emulator on this Mac (Xcode 16.2 with iOS 16.4 to 18.3 simulators; Android emulator 35.4.9 with an arm64 API 35 image, running natively on Apple Silicon);
  - Linux on `oldvga` (its 3.2 GB of memory is too little for an Android emulator);
  - Windows on a Windows machine or virtual machine (section 15);
  - real phones before each release, because simulators do not show sandbox or keyboard behaviour faithfully.
- `cargo-deny` covers the toolkit's and wasmi's dependency trees, and the size of each app is reported.

---

## 15. Decisions needed

| # | Decision | Recommendation |
|---|---|---|
| 1 | The process model: interface, fetcher and decoder processes, with one decoder per site, on Windows, macOS, Linux and Android | Yes; one per tab in the first version |
| 2 | The interface toolkit | **Decided** by the owner on 11 October 2026: egui on all five platforms (architecture option 1); no Slint |
| 3 | Libraries: the toolkit and its windowing and GPU tree; a menu crate for the macOS menu bar; a Public Suffix List source; a punycode and confusables source; a secret-store crate; Sparkle and WinSparkle bindings; Windows and macOS sandbox crates | Measure size and licences before choosing each |
| 4 | The input router of `docs/hmi.md`, with a touch device class added | Adopt; every front end depends on it |
| 5 | Record the web-native concept's decisions in Part E and write its requirements | **Done** 12 October 2026: decisions E.63 to E.69, area BRW |
| 6 | macOS process isolation: XPC services, or helper executables with Seatbelt profiles | XPC services |
| 7 | Where Windows builds and tests run | A Windows virtual machine reachable from the gate |
| 8 | Signing and distribution: the Apple Developer Program (macOS and iOS), a Windows code-signing certificate, a Google Play developer account, Flathub, F-Droid | Needed for the first phone builds on real devices (Apple) and for public release (the rest) |
| 9 | Mobile | Decided by the owner on 11 October 2026: Android and iOS are targets |
| 10 | Session restore and history retention: how long, and whether confirmed sites persist between runs | 30 days of history; confirmed sites per session only |
| 11 | All browser behaviour in the platform-free crate `neoteletext-browser`, with thin native shells (section 3.5) | Yes |
| 12 | The decoder runs inside the wasmi WebAssembly interpreter on every platform (sections 3.6 and 12.8) | Yes. It is the only wall available on iOS, and one wall tested everywhere is better than a special case |
| 13 | iOS runs as one process, with the interpreter as the decoder's only wall (section 12.7) | Accept; no alternative exists outside BrowserEngineKit |
| 14 | Slint's licence condition | **Withdrawn** 11 October 2026: Slint is not used |
| 15 | The interface between Rust and the Swift and Kotlin shells: UniFFI (MPL-2.0) or a hand-written C interface in `neoteletext-ffi` | The hand-written C interface; it is narrow (text in, events out) and keeps `unsafe` in the one crate allowed it |
| 16 | Minimum versions: Android API level 29 (for `bindIsolatedService`); iOS 17 | Yes |
| 17 | An outside security audit of the wasmi version we ship, or of our host | Before the first public release |
| 18 | Installing the Rust iOS and Android targets, the Android NDK, an API 29 emulator image, and a current Xcode here; freeing disk space on this Mac | Needed for the test app |

**Already decided and not reopened here:**
- **The tuning screen** stays inside the page area with row 0 above it (concept section 6, owner's instruction of 11 October 2026).
- **The other settled concept decisions** listed in `docs/web-native-concept.md` section 15.

---

## 16. Phased plan

Each phase ends with its tests passing in the gate.

| Phase | Work | Done when |
|---|---|---|
| **1. Requirements** | Record decisions 1, 2, 4, 5, 11, 12, 13, 15 and 16 in Part E; write the WEB, NAV and new BROWSER requirements; amend Appendix E and REQ-WEB-002. | Requirements reviewed |
| **2. Engine: web-native model** | Section 4.2, in `neoteletext-web` and `neoteletext-browser`, with tests against the local test server. | `ttx` browses a test site with aliases, bundles, dependencies and links to other sites |
| **3. Engine: tab and navigation** | Sections 5 and 6 in `neoteletext-browser`, the input router, the address display rules; `ttx view` rebuilt on the tab object; the headless test driver. | `ttx view` has the address bar in row 0, the tuning screen, and history across sites; the headless scripts pass |
| **3a. Phone test app** (can run beside phases 2 and 3) | Section 9.1: the egui test app with its shells, on the iOS simulator, an iPhone, the Android emulator and an Android phone, all from this Mac. | Every criterion of section 9.1 passes, or the failures are brought to the owner |
| **4. Decoder walls** | The decoder module and the wasmi host (sections 3.6 and 12.8); the decoder process and its messages (section 3.4); `ttx` uses both. | Decoders run in the interpreter everywhere and in sandboxed processes on Linux; the misbehaving-module and protocol fuzz tests pass |
| **5. App, first version** | `neoteletext-gui` on Windows, macOS and Linux: window, tab strip, toolbar, address bar, page area, Fastext buttons, keyboard, menus. | Browses files, zxnet and web sites on all three desktop platforms |
| **6. Phones** | The Android and iOS shells (section 3.5), the phone layout, touch input, the Android service stubs. | Browses the same sites on the Android emulator and phone, the iOS simulator and iPhone |
| **7. Platform security** | Section 12 on macOS, Windows, Android and iOS. | The sandbox tests of section 14 pass on each platform |
| **8. Features** | Bookmarks, presets, history, session restore, page catching with mouse and touch, accessibility, the glyph switch, fullscreen badge. | Feature tests and snapshots pass on all five platforms |
| **9. Release** | Section 13: packages, signing, store review, updates, link handlers; the audit of decision 17. | Signed packages install and update on each platform |

Phases 2 to 4 benefit `ttx` directly, and are worth doing whatever is decided about the apps.

---

## 17. Sources

1. https://www.chromium.org/developers/design-documents/multi-process-architecture/
2. https://chromium.googlesource.com/chromium/src/+/main/mojo/README.md
3. https://chromium.googlesource.com/chromium/src/+/main/services/network/README.md
4. https://www.chromium.org/Home/chromium-security/site-isolation/
5. https://chromium.googlesource.com/chromium/src/+/main/sandbox/linux/README.md
6. https://chromium.googlesource.com/chromium/src/+/main/sandbox/mac/README.md
7. https://chromium.googlesource.com/chromium/src/+/main/docs/design/sandbox.md
8. https://chromium.googlesource.com/chromium/src/+/main/docs/session_history.md
9. https://chromium.googlesource.com/chromium/src/+/main/docs/navigation_concepts.md
10. https://web.dev/articles/bfcache
11. https://hacks.mozilla.org/2021/05/introducing-firefox-new-site-isolation-security-architecture/
12. https://firefox-source-docs.mozilla.org/dom/ipc/process_model.html
13. https://briangrinstead.com/blog/xbl-replacement-newsletter-7/
14. https://docs.webkit.org/Deep%20Dive/Architecture/WebKit2.html
15. https://www.synacktiv.com/sites/default/files/2024-05/escaping_the_safari_sandbox_slides.pdf
16. https://chromium.googlesource.com/chromium/src/+/main/docs/ui/views/overview.md
17. https://chromium.googlesource.com/chromium/src/+/71.0.3578.80/docs/ui/views/macviews_release.md
18. https://chromium.googlesource.com/chromium/src/+/main/docs/security/url_display_guidelines/url_display_guidelines.md
19. https://chromium.googlesource.com/chromium/src/+/main/docs/idn.md
20. https://textslashplain.com/2017/01/14/the-line-of-death/
21. https://chromium.googlesource.com/chromium/src/+/main/docs/security/faq.md
22. https://github.com/LadybirdBrowser/ladybird
23. https://github.com/LadybirdBrowser/ladybird/tree/master/Services
24. https://github.com/LadybirdBrowser/ladybird/blob/master/Services/RendererSandboxLinux.cpp
25. https://github.com/LadybirdBrowser/ladybird/commits/master/UI/AppKit
26. https://github.com/servo/servo/blob/main/ports/servoshell/Cargo.toml
27. https://github.com/versotile-org/verso
28. https://github.com/netsurf-browser/netsurf/blob/master/docs/implementing-new-frontend.md
29. https://dillo-browser.github.io/
30. https://geminiprotocol.net/docs/protocol-specification.gmi
31. https://github.com/skyjake/lagrange
32. https://gmi.skyjake.fi/lagrange/
33. https://kristall.random-projects.net/
34. https://github.com/makew0rld/amfora
35. https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingXPCServices.html
36. https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation
37. https://docs.flatpak.org/en/latest/sandbox-permissions.html
38. https://github.com/refi64/zypak
39. https://chromium.googlesource.com/chromium/src/+/d232a5d77ae4b065bc1905a4942a518aa5e07c4d/ios/web/
40. https://developer.chrome.com/blog/chromium-chronicle-28
41. https://firefox-source-docs.mozilla.org/rust-components/developing-rust-components/index.html
42. https://hacks.mozilla.org/2023/08/autogenerating-rust-js-bindings-with-uniffi/
43. https://book.servo.org/trying/getting-servoshell.html
44. https://chromium.googlesource.com/chromium/src/+/HEAD/docs/security/android-sandbox.md
45. https://chromium.googlesource.com/chromium/src/+/main/chrome/android/java/AndroidManifest.xml
46. https://developer.android.com/guide/topics/manifest/service-element
47. https://developer.android.com/ndk/reference/group/native-service
48. https://firefox-source-docs.mozilla.org/mobile/android/geckoview/project/isolated-process/overview.html
49. https://github.com/servo/servo/blob/main/components/constellation/sandboxing.rs
50. https://developer.apple.com/forums/thread/706855
51. https://support.apple.com/guide/security/sec15bfe098e
52. https://developer.apple.com/support/alternative-browser-engines/
53. https://developer.apple.com/support/alternative-browser-engines-jp
54. https://github.com/wasmi-labs/wasmi
55. https://developer.apple.com/app-store/review/guidelines/
56. https://developer.apple.com/programs/information/Apple_Developer_Program_Information_8_12_15.pdf
57. https://github.com/emilk/egui/blob/main/crates/eframe/CHANGELOG.md
58. https://github.com/rust-windowing/winit/pull/3787
59. https://github.com/rust-windowing/winit/issues/1497
60. https://github.com/AccessKit/accesskit
61. https://github.com/slint-ui/slint/blob/master/CHANGELOG.md
62. https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md
63. https://github.com/mozilla/uniffi-rs

Claims the research could not verify: Safari's interface internals, the release in which Chrome's macOS interface became Views-only, Firefox's current `browser.xhtml` after 2019, and Landlock inside Flatpak.

Mobile claims not verified (research of 11 October 2026): whether eframe uses AccessKit on phones; the current Apple licence text on bundled interpreters; that BrowserEngineKit is still limited to the EU and Japan; whether Firefox's Android isolated processes are on by default; and whether Android's native services can be isolated processes.
