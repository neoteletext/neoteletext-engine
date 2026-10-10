# Porting guide

This is the whole of a port (REQ-ARCH-009, decision E.29). A port supplies the items below and nothing else; the library never touches the hardware.

## The checklist

| Item | How | Required |
|---|---|---|
| A data source | Implement `tellytxt_core::port::DataSource` (modelled on `embedded-io` `Read`), or call the format readers with bytes yourself. | Yes |
| Somewhere to put pixels | Call the row renderer with your own buffer: `tellytxt_core::render::render_row` (a text row) or `render_line` (one screen line). The port asks; the library answers. | Yes |
| A time source | Implement `tellytxt_core::port::Clock`: monotonic microseconds. Used for page completion, flash and reveal. | Yes |
| Key input | Implement `tellytxt_core::port::Keys` returning `tellytxt_core::viewer::Key`. | For interactive ports |
| Sleep | Implement `tellytxt_core::port::Sleep` if your platform must yield. | Optional |
| A network transport | Implement the transport trait of `tellytxt-web` (connect, send, non-blocking receive, close; TLS is yours). | Only for web sources |
| Cookie storage | Implement the cookie storage trait of `tellytxt-web`. Store saved cookies as secrets (REQ-SEC-013). | Optional |

A complete minimal port is `crates/tellytxt-core/examples/minimal_port.rs`, under 100 lines.

## Memory

All state lives in memory you provide (REQ-GEN-003):

- The page store is a slice of `StoredPage` you allocate: `[StoredPage::EMPTY; N]` in a `static`, or on the heap. Each slot is about 2.7 kilobytes.
- For Levels 2.5 and 3.5, the enhancement store is two more slices: `EnhancementSubPage::EMPTY` entries of 160 bytes, and `EnhancementPacket::EMPTY` slots of 42 bytes, one per packet. Annex E.8 bounds a service at 500 packets at Level 2.5 and 1200 at Level 3.5, about 21 and 50 kilobytes, plus 24 packets for each magazine's Magazine Organization Table. Pass them to `Decoder::with_enhancement_store`; `Decoder::new` keeps no enhancement pages.
- A `Grid` is 1000 cells of 16 bytes plus, by default, a side-panel area of 768 cells and room for 48 DRCS patterns: about 32 kilobytes. A port that shows only Levels 1 and 1.5 can use `Grid<0, 0>`, about 16 kilobytes. Build it in place with `build_grid(&mut grid, ...)`.
- A rendered text row at 480 by 20 needs 9.6 kilobytes at 8 bits per pixel or 4.8 kilobytes at 4 bits.

## Pixels and colour

- The renderer writes palette indices, never colours (REQ-RND-023): 0 to 7 the colours, 8 transparent, 16 to 79 flashing, 80 to 143 concealed.
- Flash and reveal are palette changes: refill your palette with `tellytxt_core::render::fill_palette` when the flash phase (`render::flash_visible`) or reveal changes. If your display has a hardware palette, no redraw is needed.
- `tellytxt_image::convert` turns indices into RGB565, RGB888, RGBA8888, grey or 1-bit, with byte order, stride and rotation.

## Aspect ratio

Teletext pixels are about 1.2 times taller than wide on a 4:3 television (REQ-RND-004). The library always renders 480 by 500 (or 480 by 250); scale in the port, for example with `tellytxt_image::scale::scale_indices(..., aspect: true)`, or the picture will look squashed.

## What the library will not do

It does not drive displays, generate video, keep display timing, buffer frames or contain code for any particular chip (REQ-ARCH-010). Buffering strategy, direct memory access and scan-out are yours.

## Isolation on microcontrollers

Where the chip has a memory protection unit, run the network stack and the decoder in separate regions (REQ-SEC-021, Phase 2 guidance).
