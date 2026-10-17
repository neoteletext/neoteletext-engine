# F.2 Architecture and programming interface (ARCH)

**REQ-ARCH-001 — Module structure.**
The implementation shall be divided into these core modules, with dependencies flowing only in the order listed:
1. the teletext constants (REQ-GEN-014);
2. error-protection codes;
3. packet decoding;
4. character tables;
5. the state machines (area FSM);
6. acquisition and page store;
7. broadcast service data;
8. Level 1.0 decoding;
9. Level 1.5 decoding;
10. navigation;
11. the cell grid, the cell raster and the row renderer (REQ-RND-016 and REQ-RND-017).

Levels 2.5 and 3.5 add four core modules beside Level 1.5 decoding: the Colour Map, the decoding of DRCS characters, the resolution of objects, and the enhancement processor (areas L25, OBJ and DRC). `ARCHITECTURE.md` section 9 places them in the presentation layer, and gives the modules each may use.

The optional modules are:
- input format readers and writers, including the page encoder;
- the raw-sample data slicer;
- the web-native module (area WEB), including the live feed reader of REQ-INP-022;
- the pixel-format helpers and image writers;
- text and terminal rendering;
- the command-line program.

Optional modules may depend on the core modules, but the core modules shall not depend on them. Section F.1 says how these modules map onto packages of the Rust workspace (REQ-GEN-010).
*Verification:* Inspection of the dependency graph (`cargo tree`).

**REQ-ARCH-002 — One packet representation.**
Every input source shall be converted to the same internal packet form: the 42 bytes that follow the framing code (bytes 4 to 45 of EN 300 706 clause 7.1), each byte in transmission order with its first transmitted bit as bit 0, and parity and Hamming bits retained. All decoding shall work from this form.
*Reason:* This is the T42 convention, and it gives a single path through the decoder.
*Verification:* Inspection; test.

**REQ-ARCH-003 — Packet metadata.**
Each packet passed to the decoder shall be accompanied by metadata. It shall have fields for:
- the television line number and field, where known;
- a time stamp in microseconds;
- the input source;
- slicer quality information, where the packet came from raw samples, including whether the framing code needed correction.

Unknown values shall be marked as unknown and shall not be set to zero.
*Verification:* Inspection; test.

**REQ-ARCH-004 — Page files enter as packets.**
Page files, such as the TTI, EP1 and edit.tf formats, shall be converted into sequences of packets by an encoder that applies correct parity and Hamming coding. These packets shall then pass through the same acquisition path as broadcast packets.
*Reason:* A single decode path means page files test the real decoder, and the encoder doubles as a teletext generator.
*Verification:* Test (REQ-TST-020).

**REQ-ARCH-005 — Event notification.**
The decoder shall report the following through a caller-registered callback:

| Event | Details |
|---|---|
| Page header received | For the rolling header. |
| Page started | |
| Page updated | A row changed. |
| Page completed | |
| Page erased | |
| Update indicator | C8 seen. |
| Service data received | |
| Magazine data received | |
| Decoding errors | Discarded packets, uncorrectable codes and parity failures, with counts. |

Each event shall identify the page (magazine, page number and sub-code) where applicable.
*Verification:* Test.

**REQ-ARCH-006 — Displaying a page is a pure function.**
Producing the displayable cell grid shall be a function with no side effects. It shall take these inputs and produce the same result every time for the same inputs:
- the stored page;
- the stored magazine data;
- at Levels 2.5 and 3.5, the stored object pages, DRCS pages and Magazine Organization Tables, reached through the read-only lookup of REQ-ACQ-020;
- the decoder configuration;
- the viewer state, which covers reveal, mix mode, zoom, flash phase and the status area text.
*Verification:* Test.

**REQ-ARCH-007 — Statistics.**
The decoder shall keep counts that the caller can read and reset:
- packets received per magazine and per packet number;
- Hamming 8/4 corrections and failures;
- Hamming 24/18 corrections and failures;
- parity failures;
- discarded packets;
- pages started and completed.
*Verification:* Test.

**REQ-ARCH-008 — Configuration.**
All behaviour choices in Part E shall be exposed as fields of a configuration structure. A single function shall fill it with documented defaults, and a function shall apply a named compatibility profile (area PRF). Changing the configuration shall not require re-acquiring pages already stored.
*Verification:* Inspection; test.

**REQ-ARCH-009 — The platform interface: the whole of a port.**
Everything a new platform must supply shall be gathered into a single platform interface. A port shall need to implement only these (decision E.29):
- **a data source**, which supplies input bytes in any of the formats of area INP;
- **somewhere to put pixels.** The port calls the row renderer (REQ-RND-017) with its own buffers, whenever its hardware layer wants them. The port does not implement a callback for pixels: it asks, and the library answers;
- **a time source**, which returns monotonic microseconds for flash, timeouts and reveal (REQ-GEN-005);
- **key input**, which returns viewer key presses as the numbered functions of area NAV;
- **optionally, a sleep function**, for platforms that must yield;
- **optionally, a network transport** for the web module (REQ-WEB-001), and a save-and-load pair for the cookie store (REQ-WEB-006).

In Rust, each of these is a trait in the core or in the web crate:
- the data source and transport follow the `embedded-io` traits, so that existing embedded network stacks and the standard library can both implement them easily;
- time, keys and storage are small traits of their own.

The documentation shall include a porting guide that lists these items and nothing else. It shall come with a complete minimal example port in fewer than 200 lines.
*Reason:* This is the lesson of doomgeneric: a small, fixed porting surface is what makes software run everywhere.
*Verification:* Inspection; tests that drive the library through the reference callers of REQ-RND-021, which use only this interface.

**REQ-ARCH-010 — The boundary with the port's hardware layer.**
The library shall contain no code that:
- puts pixels on a screen;
- drives a display or display controller;
- generates a video signal;
- depends on display or scan timing;
- is written for any specific hardware, such as a particular microcontroller (for example the RP2040 or RP2350), display, capture card or television.

The library's pixel output ends when the row renderer (REQ-RND-017) has written a row or line into a buffer supplied by the port. Everything after that point belongs to the port, including:
- buffering, whether a framebuffer, a ring of row buffers or a single row;
- the order and timing of rendering;
- display drivers;
- direct memory access transfers;
- palettes held in hardware;
- scan-out;
- timing;
- what to do if the hardware is not ready.

Input devices are treated the same way: the port supplies input bytes through the data source of REQ-ARCH-009.
*Verification:* Inspection of the source for device, operating-system display or timing dependencies.


**REQ-ARCH-011 — The layers, and their enforcement.**
The code shall be organised into the layers described in `ARCHITECTURE.md`:
- Foundation;
- L0, Ingress;
- L1, Packet decoding;
- L2, Acquisition;
- L3, Presentation model;
- L4, Rendering;
- L5, Output adapters;
- L6, Interaction;
- L7, Web user agent;
- L8, Platform interface;
- L9, Applications and ports;
- the cross-cutting modules (security policy, configuration and trace).

The following rules shall hold:
- **Downward dependencies only.** A layer may depend only on the layers below it and on the cross-cutting modules. The cross-cutting modules may depend only on Foundation.
- **Allowed modules.** Within `neoteletext-core`, each module may use only the modules listed for it in section 9 of `ARCHITECTURE.md`.
- **Narrow boundaries.** Neighbouring layers communicate only through the boundary types listed in section 5 of `ARCHITECTURE.md`. Types from two or more layers down shall not appear in a layer's public interface.
- **The web stays separate.** `neoteletext-web` shall not depend on the decoding layers (L1 to L4). Pages leave it only as bytes, which enter through L0.
- **One place for security decisions.** Security decisions are made only in the security policy module (REQ-SEC-001).

An `xtask` check shall enforce the dependency rules. It reads the crate graph and each module's `use` statements, compares them with the tables in `ARCHITECTURE.md`, and fails continuous integration on any violation.

`ARCHITECTURE.md` and this requirement shall be kept in step. A change to the layers is made in both in the same change, with the reason recorded as a decision in Part E.
*Reason:* Clear layers keep the code portable, testable and safe to sandbox. Enforcing them mechanically stops them eroding, which is how layered designs usually decay.
*Verification:* The `xtask` layer check in continuous integration, including a test that a deliberately forbidden dependency is detected.
