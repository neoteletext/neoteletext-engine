# tellytxt.com — Architecture

This document describes how the code of tellytxt.com is divided into layers, what each layer owns, how data flows between them, and the rules that keep them apart.

- `REQUIREMENTS.md` is the authority. This document explains how the requirements are organised in code.
- The layer rules here are made mandatory by REQ-ARCH-011.
- Where this document and `REQUIREMENTS.md` disagree, `REQUIREMENTS.md` wins, and this document shall be corrected.

---

## 1. The idea in one paragraph

Every source of teletext becomes the same thing: a 42-byte packet. The sources include broadcast recordings, raw video samples, page files and web servers. From there, one decoder builds stored pages. A pure function turns a stored page into a grid of cells, and another pure function turns a row of cells into palette indices in a buffer the port supplies.

Everything below the application layer is free of input and output ("sans-IO"). It is also free of the operating system (`no_std`) and of hidden state. That is what makes it portable to every target, fuzzable, and safe to sandbox.

---

## 2. The layers

Dependencies point **downward only**. A layer may use the layers below it and the cross-cutting modules. It may never use a layer above it.

```
 ┌───────────────────────────────────────────────────────────────────────────┐
 │ L9  Applications & ports      ttx CLI · WASM host · user MCU ports          │  std / platform
 ├───────────────────────────────────────────────────────────────────────────┤
 │ L8  Platform interface        traits: DataSource · Transport · Clock ·      │  the port boundary
 │                               Keys · CookieStorage · DeviceKey              │
 ├───────────────┬───────────────────────────────────────────────────────────┤
 │ L7  Web user  │ URL parser · origin · HTTP/1.1 · WebSocket · SSE ·          │  sans-IO, no_std
 │     agent     │ cookies · cache · descriptor · form submission              │
 ├───────────────┴───────────────────────────────────────────────────────────┤
 │ L6  Interaction               viewer FSM · navigation · history · input ·   │  no_std
 │                               form model & editing · status area            │
 ├───────────────────────────────────────────────────────────────────────────┤
 │ L5  Output adapters           pixel formats · PNG/PPM · text/ANSI (sanitised)│  helpers no_std
 ├───────────────────────────────────────────────────────────────────────────┤
 │ L4  Rendering                 cell raster · row renderer · palette ·        │  no_std, pure
 │                               glyph data, G3 included · change tracking ·   │
 │                               resolved colour mode · side-panel composition │
 ├───────────────────────────────────────────────────────────────────────────┤
 │ L3  Presentation model        charset resolution · Level 1 row machine ·    │  no_std, pure
 │                               double height · Level 1.5 walk · colour map · │
 │                               Level 2.5/3.5 walk · objects · DRCS ·         │
 │                               side panels → CELL GRID                       │
 ├───────────────────────────────────────────────────────────────────────────┤
 │ L2  Acquisition               magazine FSMs · page lifecycle · page store · │  no_std, no alloc
 │                               magazine data · service data · stats · events │
 ├───────────────────────────────────────────────────────────────────────────┤
 │ L1  Packet decoding           MRAG · header · rows · X/26 · X/27 · X/28 ·   │  no_std, pure
 │                               M/29 · 8/30  → typed Decoded                  │
 ├───────────────────────────────────────────────────────────────────────────┤
 │ L0  Ingress                   T42 variants · TS/PES demux · V4L2 sliced ·   │  no_std readers
 │     (bytes → Packet)          raw VBI → SLICER · TTI/EP1/edit.tf → ENCODER  │
 ├───────────────────────────────────────────────────────────────────────────┤
 │ Foundation                    constants · parity · Hamming 8/4, 24/18 ·     │  no_std, const tables
 │                               CRC · bit reversal · fixed collections · types│
 └───────────────────────────────────────────────────────────────────────────┘
   Cross-cutting (usable by every layer; depend only on Foundation):
   Security policy · Configuration & profiles · Diagnostics & state-machine trace
```

The Web user agent (L7) sits beside the lower layers rather than above them in the usual sense. It only hands bytes to the Ingress layer (L0), and receives requests from the Interaction layer (L6) through events that the application passes on (section 4.4). It never calls the decoding layers directly.

---

## 3. What each layer owns

### Foundation — `tellytxt-core::{consts, ecc, types}`

- **Owns:**
  - every teletext constant, each defined once (REQ-GEN-014), with both bit orders side by side where two exist;
  - odd parity;
  - Hamming 8/4 and 24/18, with tables generated at compile time from the equations of EN 300 706;
  - the page checksum;
  - bit reversal;
  - the small shared types: `Magazine`, `PageNo`, `SubCode` and `Instant` (microseconds);
  - fixed-capacity collections.
- **Must not know about:** anything above it.
- **Requirements:** GEN-014, ECC.

### L0 Ingress — `tellytxt-formats`, `tellytxt-slicer`

- **Owns:** turning any byte or sample source into `Packet { bytes: [u8; 42], meta: PacketMeta }`. This covers:
  - T42 and its variants;
  - transport streams and elementary-stream demultiplexing;
  - Linux sliced records;
  - raw VBI samples, through the slicer;
  - page files (TTI, EP1, edit.tf addresses and screen dumps), through the page encoder, so that page files take the same path as broadcasts (REQ-ARCH-004);
  - format detection.
- **Must not know about:** pages, characters, rendering or the web.
- **Key types:** `Packet`, `PacketMeta`, `Reader` (streaming, fed bytes in any chunk size), `Encoder`.
- **Requirements:** INP, SLC, FSM-002.

### L1 Packet decoding — `tellytxt-core::packet`

- **Owns:** validated, typed fields of a single packet:
  - the magazine and row address;
  - the page header;
  - display rows;
  - enhancement triplets;
  - Fastext links;
  - page and magazine enhancement data;
  - broadcast service data.
- **Must not know about:** the page being assembled, time, characters.
- **Key types:** `Decoded::{Header, Row, Enhancement, Links, LinksRaw, CompositionalLinks, PageData, Magazine, Service, Ignored}`, `TripletFunction`, `ObjectPacket`, `PacketError`.
- **Requirements:** PKT.

### L2 Acquisition — `tellytxt-core::{acq, fsm, service}`

- **Owns:**
  - which packets belong to which page (the magazine state machines);
  - the page lifecycle;
  - the page store (cache mode, and request mode in Phase 2);
  - magazine data;
  - service data;
  - statistics;
  - the events raised to the application;
  - at Levels 2.5 and 3.5, the enhancement pages (object pages, DRCS pages and the Magazine Organization Table), stored as received in a part of the store with its own capacity, and the read-only lookup through which the presentation layer reaches them (decision E.56).
- **Must not know about:** characters, pixels, the web.
- **Key types:** `Decoder<const PAGES: usize>`, `PageStore`, `StoredPage`, `MagState`, `PageState`, `Event`, `EnhancementStore`, `EnhancementLookup`.
- **Requirements:** ACQ, SVC, FSM-003, FSM-004.

### L3 Presentation model — `tellytxt-core::{charset, level1, level15, colour, drcs, objects, enhance}`

- **Owns:** one pure function, from a stored page, the magazine data and the viewer state to a `Grid` of 25 by 40 cells, with a side-panel area of columns 40 to 71 at Levels 2.5 and 3.5. Along the way it performs:
  - character set resolution;
  - the per-cell attribute machine;
  - double height;
  - the Level 1.5 overwrite walk, in `level15`, used at Level 1.5 only;
  - at Levels 2.5 and 3.5, the enhancement processor and the modules it uses, described in section 4.5.

  Every teletext feature that crosses rows is resolved here, so rows are independent from here upward (decision E.29). At Levels 2.5 and 3.5 that includes objects, row colours, side panels and the row extension of the Font Style triplet.
- **The modules of Levels 2.5 and 3.5:**
  - `colour`: the Colour Map of 32 entries, the colour index type, and the re-mapping table. Its default values are the Table 30 constants of `consts`.
  - `drcs`: decodes the stored bytes of DRCS pages into the patterns and colour tables that the grid carries for its DRCS cells (area DRC).
  - `objects`: finds objects. It reads the links of X/27/4, X/27/5 and the Magazine Organization Table, the pointer tables of object pages and the local definitions, and checks each definition (area OBJ).
  - `enhance`: the enhancement processor. It runs after the Level 1 grid is built, walks the local enhancement data, objects and default objects (REQ-FSM-011), resolves each row (REQ-FSM-012), and builds the page's Colour Map, screen and row colours and side panels (area L25).
- **Must not know about:** where the page came from, pixels, buffering. Stored bytes are decoded only through the decoders of L1, and enhancement pages are reached only through the read-only lookup of L2.
- **Key types:** `Grid`, `Cell`, `CharRef`, `RowState`, `ColourMap`, `ColourIndex`, `DrcsPatterns`, `ObjectRef`.
- **Requirements:** CHR, L1, L15, L25, OBJ, DRC, FSM-006, FSM-007, FSM-011, FSM-012.

### L4 Rendering — `tellytxt-core::{render, glyphs}`

- **Owns:**
  - the cell raster: one line of one cell becomes 12 dots plus foreground and background indices;
  - the row renderer: a row or line becomes palette indices in a caller buffer, pulled by the port;
  - the palette, with flash and reveal as palette changes, for Levels 1 and 1.5;
  - the resolved colour mode of Levels 2.5 and 3.5, which resolves flash and conceal at render time into Colour Map indices (decision E.55);
  - the composition of each row from the left panel, the main page and the right panel (decision E.49);
  - double width, underline, the Level 3.5 font styles and DRCS drawing, all inside the cell raster;
  - the glyph data, which is replaceable, including the G3 data generated by `xtask` from the project's own geometry (decision E.50);
  - the optional glyph line cache;
  - change tracking, including the rows that flash or conceal.
- **Must not know about:** pixel formats, screens, buffering strategy, timing of display.
- **Key functions:** `cell_line()`, `render_row()`, `render_line()`, `changed_rows()`, `palette()`, `flash_unit()`.
- **Requirements:** RND-011 to RND-028, RND-030 to RND-041.

### L5 Output adapters — `tellytxt-image`, text output in `tellytxt-cli`

- **Owns:**
  - conversion of palette indices to RGB565, RGB888, RGBA8888, grey and 1-bit, with byte order, stride and rotation;
  - PNG and PPM writers;
  - text and ANSI output, which always passes the sanitising rules of REQ-SEC-010;
  - terminal graphics output in the kitty, Sixel and iTerm2 protocols, with detection and fallback to ANSI output (decision E.37).
- **Must not know about:** teletext rules.
- **Requirements:** RND-005 to RND-009, RND-029, SEC-010.

### L6 Interaction — `tellytxt-core::{viewer, nav, forms}`

- **Owns:**
  - the viewer state machine;
  - page number entry;
  - Fastext, index, home and sub-page navigation;
  - back and forward history;
  - reveal, mix and zoom;
  - the form model: fields, focus, editing and validation, from the declarative commands of Appendix C;
  - the contents of the receiver-owned status area in row 0, columns 0 to 7, including every security indicator.
- **Must not know about:** HTTP, sockets, how pages are fetched.
- **Key types:** `Viewer`, `ViewerState`, `History<const N: usize>`, `FormModel`, `Key`, `Action`.
- **Requirements:** NAV, FSM-005, SEC-005, SEC-009, and the model half of Appendix C.

### L7 Web user agent — `tellytxt-web`

- **Owns:** everything web-specific, as sans-IO state machines:
  - the strict address parser;
  - origins;
  - the minimal user agent profile;
  - HTTP/1.1;
  - WebSocket;
  - server-sent events;
  - the cookie store;
  - validators and caching;
  - the service descriptor;
  - push subscriptions;
  - form encoding and submission;
  - the zxnet live feed.

  It is fed received bytes, the time and requests. It produces bytes to send, timers, TTI or T42 bytes for Ingress, and events.
- **Must not know about:** teletext decoding. Pages leave it only as bytes.
- **Key types:** `Url`, `Origin`, `UserAgent`, `CookieJar<const N: usize>`, `Input`, `Output::{Send, Timer, PageBytes, Event}`.
- **Requirements:** WEB, INP-022, SEC-006 to SEC-013.

### L8 Platform interface — traits in `tellytxt-core::port` and `tellytxt-web::port`

- **Owns:** the complete porting contract (REQ-ARCH-009), and nothing else:
  - `DataSource`;
  - `Transport`, following the `embedded-io` traits;
  - `Clock`;
  - `Keys`;
  - `CookieStorage`;
  - `DeviceKey` (Phase 2).
- **Must not know about:** any implementation.

### L9 Applications and ports — `tellytxt-app`, `tellytxt-cli`, the WebAssembly host, `tellytxt-ffi`, users' ports

- **Owns:**
  - real input and output;
  - the event loop;
  - operating-system sandboxing;
  - the privilege-separated fetcher and decoder processes;
  - the C interface.
- **This is the only layer allowed to touch the operating system** (REQ-SEC-016).
- **The split between `tellytxt-app` and `tellytxt-cli`:**
  - `tellytxt-app` holds what any program needs. That is the session, reading inputs, both processes of live mode, sandboxing, conversion and slicing, and the `Source` trait a viewer reads pages through.
  - `tellytxt-cli` holds argument parsing, terminal output and the terminal viewer.
  - A windowed viewer depends on `tellytxt-app` and routes the hidden `fetch` command to `tellytxt_app::fetch::run`, because live mode starts the fetcher as a second run of the same program.
- **Requirements:** CLI, SEC-017 to SEC-019, GEN-018.

### Cross-cutting modules — `tellytxt-core::{policy, config, trace}`

| Module | Contents |
|---|---|
| `policy` | Every security decision, as pure functions asked by the layers that need them. Examples: "is this address blocked?", "may this form submit to that origin?", "is this a downgrade?". No layer reimplements a policy decision. |
| `config` | The configuration struct, its defaults and the named profiles. |
| `trace` | The state-machine transition trace (REQ-FSM-010): `TraceEntry`, `Machine`, and `TraceSink` for machines with no event queue of their own. |

They depend only on Foundation.

---

## 4. The flows

### 4.1 Receiving

```
source bytes ──► L0 Reader ──► Packet ──► L1 decode ──► Decoded ──► L2 Decoder::push ──► StoredPage
                                                                          └──► Event ──► L6 / L9
```

Web pages take exactly this path: L7 delivers the fetched TTI or T42 bytes to an L0 reader.

### 4.2 Displaying

```
port asks L4 changed_rows()
   └─► L3 build Grid (stored page + magazine data + viewer state from L6)
          └─► L4 render_row(grid, row, mode, field, port_buffer)   ← palette indices
                 └─► L5 convert (optional) ──► port's hardware layer
```

The port pulls; the library never pushes pixels.

### 4.5 Building the grid at Levels 2.5 and 3.5

```
L3 level1: Level 1 grid from the stored page's rows
   └─► L3 enhance (presentation level 2.5 or 3.5; at 1.5, level15 runs instead)
          ├─ colour:  Colour Map from Table 30, X/28/0, X/28/4, M/29/0, M/29/4
          ├─ walk (REQ-FSM-011) over local X/26, decoded by L1 packet
          │     └─ objects: links (X/27/4, X/27/5, MOT) and definitions
          │           └─ L2 EnhancementLookup (read-only) ──► stored POP, GPOP and MOT bytes ──► L1 decoders
          ├─ drcs:    patterns and colour tables of the DRCS characters used
          │           └─ L2 EnhancementLookup (read-only) ──► stored DRCS bytes
          └─ row resolution (REQ-FSM-012) ──► Grid: cells, side-panel area, Colour Map,
                                               screen and row colours, DRCS patterns
L4 render_row(grid, row, mode, field, flash unit, reveal, port_buffer)  ← Colour Map indices
```

- **After the Level 1 grid.** The enhancement processor starts from the grid that `level1` built, and changes its characters and attributes (decision E.42).
- **Read-only lookup.** Object pages, DRCS pages and the Magazine Organization Table are read through the lookup of L2 (REQ-ACQ-020), and decoded only by the decoders of L1. Nothing in L3 changes the store.
- **Everything travels with the grid.** The Colour Map, the screen and row colours, the side-panel area and the DRCS patterns and colour tables used by the page are all part of the `Grid`. A row is therefore still rendered from the grid and the viewer state alone (decision E.29).
- **Rendering.** At Levels 2.5 and 3.5 the row renderer uses the resolved colour mode (decision E.55), composes the side panels into each row (decision E.49), and draws G3 characters from the generated G3 glyph data.
- **Redrawing.** When an enhancement page that the displayed page uses arrives or changes, L2 raises an event, the grid is built again, and change tracking reports the rows that changed (REQ-ACQ-022).

#### Interfaces for the enhancement phases

The foundation phase built the types and stores below. Three phases build on them: (a) the enhancement walk and objects, in `enhance` and `objects`; (b) DRCS, the G3 glyphs and the renderer; (c) the TTI formats and the test-page encoder.

**Packet layer (`packet`).** Every decoder works on a `Packet` or, for stored enhancement pages, on the 40 stored data bytes.

| What | Where |
|---|---|
| Triplet functions of Tables 28 and 29 (REQ-PKT-010) | `Triplet::function() -> TripletFunction`; the inverse is `TripletFunction::to_triplet()`. Fields: `ColourData` (index and D6 D5), `ObjectSource`, `ObjectKind`, `ObjectNumber` (`subpage`, `pointer_packet_index`, `pointer_group`, `pointer_odd` for POP and GPOP; `n8`, `designation`, `triplet` for local objects), `ObjectUsage`, `DisplayAttributes`, `FontStyleData`. |
| X/27/4 and X/27/5 (REQ-PKT-011) | `Decoded::CompositionalLinks`; `CompositionalLink` with `is_format_1()` and `is_null()`. Roles by position (Table 16) are for `objects` to apply. |
| X/28/0, X/28/1, X/28/3, X/28/4 (REQ-PKT-012 to 015) | `Decoded::PageData { packet: PagePacket::{Format1, DrcsColours, DrcsModes, Level35} }`; `PageEnhancement { format: Option<PageFormat>, fields: PresentationFields }`; `DrcsColourData` (four tables, each `Option`); `DrcsModes::mode(ptu)`. |
| M/29/0, M/29/1, M/29/4 (REQ-PKT-016) | `Decoded::Magazine { packet: MagazinePacket::{Data, DrcsColours, Level35} }`. |
| Object pages (REQ-PKT-017) | `ObjectPacket::new(&[u8; 40])` with `byte_6`, `triplets`, `carries_pointers()`, `pointer(triplet, odd)`; `triplet_location(number) -> TripletLocation`. |
| Magazine Organization Table (REQ-PKT-018) | `mot_position(page)`, `mot_page_entry`, `mot_object_link(row, link)`, `mot_drcs_link(row, link)`, `mot_totals`, `mot_fallback`. |
| Encoders, for tests and phase (c) | `packet::encode`: `compositional_links`, `page_enhancement`, `magazine_enhancement`, `drcs_colours`, `drcs_modes`, `triplet_packet` (X/26 and object page packets), `pointer_triplet`, `nibble_packet` (table packets), and `mot_page_entry`, `mot_object_link`, `mot_drcs_link`, `mot_totals`, which give the 4-bit values to place before coding. |

Field groups shared with the presentation layer are foundation types in `types`: `Rgb12`, `ColourWords`, `Designations`, `SidePanelFields`, `DefaultColourFields`, `PresentationFields` (each group an `Option`, `None` when damaged: decision E.44), `FallbackFields`, `DrcsTable` and `DrcsCharacter`.

**Acquisition (`acq`).**

| What | Where |
|---|---|
| A page's own enhancement packets | `StoredPage`: `enhancements` (X/26 triplets, as before), `compositional_links[0]` and `[1]` (X/27/4 and X/27/5), `page_data` (X/28/0), `drcs_colours` (X/28/1), `page_data_level35` (X/28/4). |
| Magazine packets | `MagazineData`: `data` (M/29/0), `drcs_colours` (M/29/1), `level35` (M/29/4); discarded on a new network identification or `Decoder::start_new_input` (REQ-ACQ-018). |
| Enhancement pages, stored as received | `Decoder::with_enhancement_store(pages, EnhancementStore::new(&mut sub_pages, &mut packets), config)`. Capacity is the length of the packet pool (Annex E.8: `ENHANCEMENT_PACKETS_LEVEL_25`, `ENHANCEMENT_PACKETS_LEVEL_35`). `Decoder::new` keeps no enhancement pages. |
| The read-only lookup (REQ-ACQ-020) | `Decoder::enhancement_lookup() -> EnhancementLookup`: `sub_page(magazine, page, s1)`, `magazine_organization_table(magazine)` (page mFE, S1 = 0 only), `magazine_data(magazine)`. An `EnhancementPage` gives `row(packet)`, `enhancement(designation)`, `received()`, `format()`, `drcs_modes()`, `subcode()`, `last_packet()`, `control()` and `generation()`, which changes with the content or S2. |
| Redrawing (REQ-ACQ-022) | `Event::EnhancementPageChanged(EnhancementKey)` at the end of a reception that changed a sub-page, and `Event::MagazineData(magazine)`. |
| Enhancement faults in the statistics (REQ-L25-051, REQ-ARCH-007) | `Statistics::enhancement: EnhancementFaults`, one count per kind of fault. The caller adds a build's faults with `Decoder::record_enhancement(PageBuild, &EnhancementFaults)` after building, when the lookup's borrow has ended. A `PageBuild` is the page, its reception count and the level asked for; the build recorded last is not counted again, so redraws of a page add nothing. The types live in `acq` because L2 may not use L3. |

**Presentation model (`colour`, `drcs`, `level1`) and configuration.**

| What | Where |
|---|---|
| The presentation level | `config::PresentationLevel` (`is_enhanced`, `is_level35`) in `Config::level`. The flash timing of decision E.48 is `FLASH_CYCLE_US`, `FLASH_UNITS`, `FLASH_SLOW_ON_UNITS`, `FLASH_FAST_HALF_UNITS`, `FLASH_PHASES` and `FLASH_FAST_OFF_UNITS` in `consts`. |
| Page-level presentation data | `colour::resolve(&PresentationSources, level, &base) -> PagePresentation`: `colour_map`, `default_screen`, `default_row`, `black_background_substitution`, `remapping` (`Remapping::foreground` and `background` give the index of a Level 1 colour), `side_panels` (`SidePanels` widths). The caller fills `PresentationSources` with the page's X/28/0 (only for a displayed page), X/28/4, M/29/0, M/29/4 and the POP link's `FallbackFields`. |
| Colours | `ColourIndex` (0 to 31), `colour_index(clut, entry)`, `ColourMap` (`DEFAULT` from Table 30, `entry`, `rgb888`, `set_cluts`, `is_transparent`). |
| DRCS | `drcs::DrcsPattern` (character, mode, four planes of 10 rows of 12 dots; mode 3 stored doubled), `DrcsPatterns<N>`, `DrcsMode`, `DrcsColourTables::resolve(page, magazine, level)`, `DEFAULT_DRCS_PATTERNS`. |
| A cell | `level1::Cell`: `foreground` and `background` are Colour Map indices; `attributes: CellAttributes` packs `width`, `flash_mode`, `flash_rate`, `underline`, `invert`, `font` and `source`, read with those methods and set with the matching `with_*` methods. G3 and DRCS characters are `Glyph::Char` of `CharSet::G3` and `CharSet::Drcs { table, subtable }` (`CharRef::drcs`, `CharRef::drcs_character`). |
| The grid | `level1::Grid<DRCS, PANEL_ROWS>` (defaults 48 and 24): `panels`, `panel_cell(row, column)` and `panel_cell_mut` for columns 40 to 71 of rows 1 to 24, `side_panels`, `colour_map`, `screen_colour`, `row_colours`, `drcs`, `drcs_pattern(character)`, `drcs_colours`, and `clear()`, which empties a grid in place. Functions that take a grid are generic over both capacities; `Grid` alone is the default. |

**What each phase built.**

- Phase (a): `compose::compose_level` dispatches on `Config::level` (through `PresentationLevel::available`, decision E.61): nothing at Level 1, `level15::apply` at Level 1.5, and `enhance::apply` at Levels 2.5 and 3.5; `viewer::Viewer::compose` and the command-line program's `Session::grid` both call it, and `ComposeContext` carries the `EnhancementLookup`. `enhance::apply` returns an `acq::EnhancementReport`: the `PageBuild`, the `EnhancementFaults` it met, the objects it ran and the triplets it read. `Viewer::compose` returns it in `Composed::Built`, for the port to record; `Session::grid` records it itself, and `ttx list` builds every page at Levels 2.5 and 3.5 to show the counts.
- Trace (REQ-FSM-010): `compose::compose_level_traced`, `level15::apply_traced` and `enhance::apply_traced` take a `trace::TraceSink` (the build time and a `&mut dyn FnMut(TraceEntry)`) and, when `Config::trace` is on, report the transitions of `Machine::TripletWalk` or of the page-level run of `Machine::EnhancementWalk`, with state, event and action names from `level15::walk_*_name`, `enhance::walk_*_name` and `TripletFunction::name`. A transition that met a fault names it as its action. The untraced functions are unchanged; `Session::grid` uses the traced one, so that `ttx -vv` prints the walk with the other traces.
- `enhance::apply` rebuilds each shown row from `level1::row_codes` and the walk's events, keeping row 0 columns 0 to 7 from the Level 1 grid. The walk runs once for the page-level colours and once for each row that has events, keeping only that row's events, so that a grid needs one row of event storage (decision E.41, principle 5); the limits of Annex A.8 are checked before an object is looked for, which bounds the work by `enhance::MAX_WALK_STEPS`. `objects` finds links, definitions and bodies; `drcs::decode_character` fills `Grid::drcs`.
- `level1::resolve_designation` follows the order of Annex C.4 and decision E.50 at each level, and the Level 1.5 pass reads every triplet through `TripletFunction` (REQ-PKT-010).
- Phase (b): `render` draws DRCS cells from `Grid::drcs` before any glyph lookup (`GlyphSet::glyph` gives an empty glyph for a DRCS reference), and has the resolved colour mode (`ColourMode::Resolved(FlashPhase)`, chosen by `RenderOptions::for_level`), side-panel composition (`row_width`, `source_columns`, `source_cell`), double width and size, underline, invert, the font styles and the flash timing of decision E.48. The G3 glyphs are generated by `cargo xtask glyphs` from geometry into `glyphs/g3.txt` and `glyphs/g3.rs`. The image writers take the 32-entry palette of `colour_map_palette`.
- Phase (c): `tellytxt-formats` reads and writes enhancement data and enhancement pages in TTI by page coding, sends them in the order of Annexes B.2 and B.7, and has the authoring builder of REQ-INP-025 (`author`, features `std` and `tti`), whose `streams::stream()` is the authored Level 2.5/3.5 stream; `cargo xtask level25-stream` writes it.
- Checks: `tests/golden/authored-level25.txt` pins the rendered stream (REQ-TST-024); `tests/oracles/run_cells.sh` compares it with zvbi cell by cell through `ttx text --cells` (REQ-TST-023), with `zvbi-known-differences.txt` giving the reason for each page that differs; the `enhancement` fuzz target builds grids from fuzzed page sets (REQ-TST-042).

### 4.3 Interacting

```
key ──► L6 Viewer ──► Action::{RequestPage, History, EditField, Submit, Toggle…}
            ├─► RequestPage → L2 (broadcast source) or, through L9, → L7 (web source)
            └─► Submit      → through L9, → L7 form submission
```

### 4.4 Web

```
L9 event loop:
   request from L6 ──► L7 UserAgent::handle(Input::Request)
   L7 Output::Send(bytes) ──► port Transport ──► network
   network ──► port Transport ──► L7 UserAgent::handle(Input::Received(bytes))
   L7 Output::PageBytes ──► L0 Reader ──► (flow 4.1)
   L7 Output::Event(Unencrypted | FormOrigin | …) ──► L6 status area
```

In privilege-separated mode (REQ-SEC-018), the dashed line between L7 and L0 is a pipe between two processes, carrying only TTI or T42 bytes and a small, fixed set of framed control messages.

---

## 5. Interfaces between neighbouring layers

Each boundary is one narrow type. Nothing reaches past its neighbour.

| Boundary | Type | Direction |
|---|---|---|
| L0 → L1 | `Packet` (42 bytes plus metadata) | down to up |
| L1 → L2 | `Decoded` | down to up |
| L2 → L3 | `&StoredPage`, `&MagazineData`, and at Levels 2.5 and 3.5 `&EnhancementLookup` (all read-only) | down to up |
| L6 → L3 | `&ViewerState` (read-only) | across |
| L3 → L4 | `&Grid` | down to up |
| L4 → L5 | a row of palette indices plus `&Palette` | down to up |
| L7 → L0 | page bytes (TTI or T42) | across, through L9 |
| L6 ↔ L7 | `Action` and `Event`, passed by L9 | across, through L9 |
| Any layer → L9 | `Event` callbacks and return values | up |
| L9 → L0, L2, L4, L6 and L7 | calls through the public interface | down |

---

## 6. Trust boundaries

| Boundary | What crosses it | Protection |
|---|---|---|
| Untrusted bytes into L0 readers | Files, streams, samples, page files | Strict streaming parsers, size limits, fuzzing (REQ-SEC-003, REQ-TST-040) |
| Untrusted bytes into L7 | HTTP, WebSocket, SSE, descriptors, cookies | The user agent profile, the strict address parser, policy checks, fuzzing (REQ-WEB-014 to REQ-WEB-016, SEC-006 to SEC-011) |
| L7 to L0 across processes | TTI or T42 bytes and framed control messages | Both sides validate; the decoder process has no rights (REQ-SEC-018) |
| L5 to the terminal | Text and escape sequences | Allow-list sanitising (REQ-SEC-010) |
| L8, the port contract | Everything the platform supplies | Narrow traits; the library holds no ambient authority (REQ-SEC-016) |

---

## 7. Crates

| Crate | Layers | Environment |
|---|---|---|
| `tellytxt-core` | Foundation, L1, L2, L3, L4, L6, L8 (core traits), cross-cutting | `no_std`, no allocation |
| `tellytxt-formats` | L0: readers, detection, encoder | `no_std` readers; `std` writers |
| `tellytxt-slicer` | L0: raw samples to packets | `no_std` |
| `tellytxt-web` | L7 and L8 (web traits) | `no_std`, sans-IO |
| `tellytxt-image` | L5 | Helpers `no_std`; writers `std` |
| `tellytxt-app` | L9: the application layer shared by `ttx` and a windowed viewer: sessions, inputs, live mode's two processes, sandboxing | `std` |
| `tellytxt-cli` | L9 (and text output of L5) | `std` |
| `tellytxt-ffi` | L9: the C interface (Phase 2) | The only crate with `unsafe` |
| `xtask` | Tooling, not shipped | `std` |

### Features: a build contains only the parts it uses

Decision E.61 makes each crate's optional areas Cargo features, all on by default, because the linker cannot remove code reachable through a run-time choice. The rules that keep them safe:

- **Additive and coarse:** one feature per area of REQUIREMENTS.md Part F, never one per function.
- **Types and acquisition are never optional.** Only processing is behind a feature, so enabling one never breaks code written without it, and Level 1.5 keeps reading the character set designations of X/28 and M/29.
- **Crates name what they use.** The workspace dependencies have default features off, so feature unification never brings an area back into a build that left it out.
- **A request a build cannot serve is answered, never ignored or panicked on:**
  - a configured level that is not compiled in falls back (`PresentationLevel::available`);
  - a format that is not compiled in gives `FormatNotCompiled`;
  - push, forms and cookies give their `NotCompiledIn` values;
  - a `ttx` command whose area is left out says so and exits with the usage status.

| Crate | Features |
|---|---|
| `tellytxt-core` | `level15`, `level25`, `render`, `bedstead`, `viewer`, `forms` |
| `tellytxt-formats` | `transport`, `sliced`, `pages`, `tti`, `ep1`, `edittf`, `dump`, `check`, and `std` (not default) |
| `tellytxt-web` | `push`, `forms`, `cookies` |
| `tellytxt-app` | `live`, `slice` |
| `tellytxt-cli` | `view`, `live`, `slice` |

**Two checks keep this true:**
- `cargo xtask features` lints and tests each crate with no optional area and with each area alone.
- `cargo xtask size` links the microcontroller test image in `firmware/test-image` for a fixed set of builds, measures them against the flash budget of REQ-GEN-009, and fails on floating-point, compare-and-swap or panic routines (REQ-GEN-019).

If `tellytxt-core` grows unwieldy, the natural split is `core` (Foundation to L2) and `present` (L3, L4 and L6), with the stored page as the interface between them. The layer rules do not change.

---

## 8. The rules, and how they are enforced

1. **Downward dependencies only.** Between crates this is enforced by the crate graph. Within `tellytxt-core`, a check in `xtask` reads each module's `use` statements and compares them with the allowed-dependency table in section 9. Continuous integration fails on any violation (REQ-ARCH-011).
2. **One narrow type per boundary** (section 5). Types from two or more layers down shall not appear in a layer's public interface.
3. **No input or output below L9.** Layers 0 to 7 are sans-IO and pure (REQ-SEC-016), enforced by clippy's `disallowed-methods` and `disallowed-types` lists.
4. **Security decisions in one place.** Only `policy` decides. Other layers call it, and the check in rule 1 forbids them from reimplementing it by forbidding direct use of the low-level address and origin tests outside `policy` and L7.
5. **Pure presentation.** L3 and L4 are pure functions of their inputs (REQ-ARCH-006). They can be recomputed or cached at will without touching acquisition.
6. **Data, not code, for what changes.** Glyphs, character tables and profiles are data (REQ-RND-020), so the custom glyphs replace Bedstead without logic changes.
7. **Every state machine lives in the layer that owns its state:**
   - the slicer machine in L0;
   - the magazine and page machines in L2;
   - the row and Level 1.5 machines, and the enhancement walk and row resolution of Levels 2.5 and 3.5, in L3;
   - the viewer machine in L6;
   - the protocol machines in L7.

---

## 9. Allowed dependencies inside `tellytxt-core`

A module may use the modules listed for it, and nothing else. The cross-cutting modules (`policy`, `config`, `trace`) may be used by any module, and may themselves use only Foundation.

| Module | Layer | May use |
|---|---|---|
| `consts`, `ecc`, `types` | Foundation | — |
| `packet` | L1 | Foundation |
| `fsm` | L2 | Foundation, `packet` |
| `acq` | L2 | Foundation, `packet`, `fsm` |
| `service` | L2 | Foundation, `packet` |
| `charset` | L3 | Foundation |
| `level1` | L3 | Foundation, `charset`, `acq` (read-only stored page types), `colour` (colour index and Colour Map types), `drcs` (the pattern types the grid carries) |
| `level15` | L3 | Foundation, `charset`, `level1` (grid types), `packet` (triplet types) |
| `colour` | L3 | Foundation |
| `drcs` | L3 | Foundation, `packet` (X/28/1, X/28/3 and M/29/1 field types), `acq` (read-only enhancement lookup), `colour` |
| `objects` | L3 | Foundation, `packet` (triplet, link and table types), `acq` (read-only stored page types and enhancement lookup) |
| `enhance` | L3 | Foundation, `charset`, `level1` (grid types and the Level 1 row machine), `packet` (triplet and X/28 field types), `acq` (read-only stored page types and enhancement lookup), `colour`, `objects`, `drcs` |
| `glyphs` | L4 | Foundation, `charset` (character references) |
| `render` | L4 | Foundation, `glyphs`, `level1` (grid types), `colour` (Colour Map), `drcs` (pattern types) |
| `viewer`, `nav`, `forms` | L6 | Foundation, `acq`, `level1`, `level15`, `enhance`, `colour`, `render` (change tracking), `service` |
| `port` | L8 | Foundation |

`tellytxt-formats` and `tellytxt-slicer` may use Foundation and `packet` only. `tellytxt-web` may use Foundation, `policy` and `config` only; it never uses the decoding layers. `tellytxt-image` may use Foundation, the `render` output types and the `colour` Colour Map type only.

`level15` remains the Level 1.5 pass, used only at Level 1.5. The modules `colour`, `drcs`, `objects` and `enhance` serve Levels 2.5 and 3.5 (section 4.5). The G3 glyph data generated by `xtask` belongs to `glyphs`, whose dependencies do not change. The `xtask` layer check holds its own copy of this table, which shall be updated in the same change that adds these modules (REQ-ARCH-011).
