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
 │                               glyph data · change tracking                  │
 ├───────────────────────────────────────────────────────────────────────────┤
 │ L3  Presentation model        charset resolution · Level 1 row machine ·    │  no_std, pure
 │                               double height · Level 1.5 walk → CELL GRID    │
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
- **Key types:** `Decoded::{Header, Row, Enhancement, Links, PageData, Magazine, Service, Ignored}`, `PacketError`.
- **Requirements:** PKT.

### L2 Acquisition — `tellytxt-core::{acq, fsm, service}`

- **Owns:**
  - which packets belong to which page (the magazine state machines);
  - the page lifecycle;
  - the page store (cache mode, and request mode in Phase 2);
  - magazine data;
  - service data;
  - statistics;
  - the events raised to the application.
- **Must not know about:** characters, pixels, the web.
- **Key types:** `Decoder<const PAGES: usize>`, `PageStore`, `StoredPage`, `MagState`, `PageState`, `Event`.
- **Requirements:** ACQ, SVC, FSM-003, FSM-004.

### L3 Presentation model — `tellytxt-core::{charset, level1, level15}`

- **Owns:** one pure function, from a stored page, the magazine data and the viewer state to a `Grid` of 25 by 40 cells. Along the way it performs:
  - character set resolution;
  - the per-cell attribute machine;
  - double height;
  - the Level 1.5 overwrite walk.

  Every teletext feature that crosses rows is resolved here, so rows are independent from here upward (decision E.29).
- **Must not know about:** where the page came from, pixels, buffering.
- **Key types:** `Grid`, `Cell`, `CharRef`, `RowState`.
- **Requirements:** CHR, L1, L15, FSM-006, FSM-007.

### L4 Rendering — `tellytxt-core::{render, glyphs}`

- **Owns:**
  - the cell raster: one line of one cell becomes 12 dots plus foreground and background indices;
  - the row renderer: a row or line becomes palette indices in a caller buffer, pulled by the port;
  - the palette, with flash and reveal as palette changes;
  - the glyph data, which is replaceable;
  - the optional glyph line cache;
  - change tracking.
- **Must not know about:** pixel formats, screens, buffering strategy, timing of display.
- **Key functions:** `cell_line()`, `render_row()`, `render_line()`, `changed_rows()`, `palette()`.
- **Requirements:** RND-011 to RND-028.

### L5 Output adapters — `tellytxt-image`, text output in `tellytxt-cli`

- **Owns:**
  - conversion of palette indices to RGB565, RGB888, RGBA8888, grey and 1-bit, with byte order, stride and rotation;
  - PNG and PPM writers;
  - text and ANSI output, which always passes the sanitising rules of REQ-SEC-010.
- **Must not know about:** teletext rules.
- **Requirements:** RND-005 to RND-009, SEC-010.

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

### L9 Applications and ports — `tellytxt-cli`, the WebAssembly host, `tellytxt-ffi`, users' ports

- **Owns:**
  - real input and output;
  - the event loop;
  - operating-system sandboxing;
  - the privilege-separated fetcher and decoder processes;
  - the C interface.
- **This is the only layer allowed to touch the operating system** (REQ-SEC-016).
- **Requirements:** CLI, SEC-017 to SEC-019, GEN-018.

### Cross-cutting modules — `tellytxt-core::{policy, config, trace}`

| Module | Contents |
|---|---|
| `policy` | Every security decision, as pure functions asked by the layers that need them. Examples: "is this address blocked?", "may this form submit to that origin?", "is this a downgrade?". No layer reimplements a policy decision. |
| `config` | The configuration struct, its defaults and the named profiles. |
| `trace` | The state-machine transition trace (REQ-FSM-010). |

They depend only on Foundation.

---

## 4. The four flows

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
| L2 → L3 | `&StoredPage`, `&MagazineData` (read-only) | down to up |
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
| `tellytxt-cli` | L9 (and text output of L5) | `std` |
| `tellytxt-ffi` | L9: the C interface (Phase 2) | The only crate with `unsafe` |
| `xtask` | Tooling, not shipped | `std` |

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
   - the row and Level 1.5 machines in L3;
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
| `level1` | L3 | Foundation, `charset`, `acq` (read-only stored page types) |
| `level15` | L3 | Foundation, `charset`, `level1` (grid types), `packet` (triplet types) |
| `glyphs` | L4 | Foundation, `charset` (character references) |
| `render` | L4 | Foundation, `glyphs`, `level1` (grid types) |
| `viewer`, `nav`, `forms` | L6 | Foundation, `acq`, `level1`, `level15`, `render` (change tracking), `service` |
| `port` | L8 | Foundation |

`tellytxt-formats` and `tellytxt-slicer` may use Foundation and `packet` only. `tellytxt-web` may use Foundation, `policy` and `config` only; it never uses the decoding layers. `tellytxt-image` may use Foundation and the `render` output types only.
