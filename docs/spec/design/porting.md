# Porting guide

This is the whole of a port (REQ-ARCH-009, decision E.29). A port supplies the items below and nothing else; the library never touches the hardware.

## The checklist

| Item | How | Required |
|---|---|---|
| A data source | Implement `neoteletext_core::port::DataSource` (modelled on `embedded-io` `Read`), or call the format readers with bytes yourself. | Yes |
| Somewhere to put pixels | Call the row renderer with your own buffer: `neoteletext_core::render::render_row` (a text row) or `render_line` (one screen line). The port asks; the library answers. | Yes |
| A time source | Implement `neoteletext_core::port::Clock`: monotonic microseconds. Used for page completion, flash and reveal. | Yes |
| Key input | Implement `neoteletext_core::port::Keys` returning `neoteletext_core::viewer::Key`. | For interactive ports |
| Sleep | Implement `neoteletext_core::port::Sleep` if your platform must yield. | Optional |
| A network transport | Implement the transport trait of `neoteletext-web` (connect, send, non-blocking receive, close; TLS is yours). | Only for web sources |
| An audio sink | Implement `neoteletext_audio::port::VoiceSink`, and pass each packet to `neoteletext_audio::receiver::Receiver::push_packet` before the decoder; on a desktop, `neoteletext-audio-mix` mixes for you (area AUD, `audio-platforms.md`). | Only for sound |
| Cookie storage | Implement the cookie storage trait of `neoteletext-web`. Store saved cookies as secrets (REQ-SEC-013). | Optional |

A complete minimal port is `crates/neoteletext-core/examples/minimal_port.rs`, under 100 lines; it needs the core features `level15` and `bedstead`.

## Choosing what to compile in

Every crate's optional areas are Cargo features, all on by default (decision E.61). A small port turns the defaults off and names what it uses:

```toml
neoteletext-core = { version = "...", default-features = false, features = ["level15", "render"] }
neoteletext-formats = { version = "...", default-features = false }  # T42 only
```

| Crate | Features |
|---|---|
| `neoteletext-core` | `level15`, `level25`, `render`, `bedstead`, `viewer`, `forms`; the capabilities of Levels 2.5 and 3.5 `objects`, `side-panels`, `colours32`, `animation`; and the profile `modern-classic` |
| `neoteletext-formats` | `transport`, `sliced`, `pages`, `tti`, `ep1`, `edittf`, `dump`, `check`, and `std` |
| `neoteletext-web` | `push`, `forms`, `cookies` |

`level25` alone gives the enhancement processor's non-spacing attributes and characters. Each other capability of Levels 2.5 and 3.5 is a feature of its own (decision E.71); a capability left out is ignored in the page data, never an error. [capabilities.md](capabilities.md) explains the mechanism, which capabilities the profile keeps and why. The `modern-classic` profile keeps the classic display with objects, and leaves out the 32-colour palette, side panels and the additional flash functions:

```toml
neoteletext-core = { version = "...", default-features = false, features = ["modern-classic"] }
```

Measured flash on Cortex-M0 on 12 October 2026 (`cargo xtask size`, `opt-level = "z"`, link-time optimisation):

| Build | Flash |
|---|---|
| Core with no optional area: acquisition, the Level 1 grid, composing | 26,198 bytes |
| Level 1.5 receiver with the renderer and the viewer; your own glyphs | 43,002 bytes |
| The same at Level 3.5, every capability | 65,758 bytes |
| The same with the `modern-classic` profile's capabilities | 62,894 bytes |
| Every core area, with Bedstead | 111,258 bytes |
| Format detection reading T42 only, with the bare core | 29,126 bytes |
| Format detection reading every format, with the bare core | 58,737 bytes |

Notes on the builds:
- **A configured level that is not compiled in** falls back to the highest one that is (`PresentationLevel::available`).
- **Glyphs:** `bedstead` is Bedstead 3.261, the bundled font (`GlyphSet::bedstead`), on by default. Without it, supply your own glyphs with `GlyphSet::from_entries`.
- **The minimal port example** needs `level15` and `bedstead`.

## Memory

All state lives in memory you provide (REQ-GEN-003):

- The page store is a slice of `StoredPage` you allocate: `[StoredPage::EMPTY; N]` in a `static`, or on the heap. Each slot is about 2.7 kilobytes.
- For Levels 2.5 and 3.5, the enhancement store is two more slices: `EnhancementSubPage::EMPTY` entries of 160 bytes, and `EnhancementPacket::EMPTY` slots of 42 bytes, one per packet. Annex E.8 bounds a service at 500 packets at Level 2.5 and 1200 at Level 3.5, about 21 and 50 kilobytes, plus 24 packets for each magazine's Magazine Organization Table. Pass them to `Decoder::with_enhancement_store`; `Decoder::new` keeps no enhancement pages.
- A `Grid` is 1000 cells of 16 bytes plus, by default, a side-panel area of 768 cells: 28,392 bytes, measured on 20 October 2026. A port that shows only Levels 1 and 1.5 can use `Grid<0>`, 16,104 bytes. Build it in place with `build_grid(&mut grid, ...)`.
- A rendered text row at 480 by 20 needs 9.6 kilobytes at 8 bits per pixel or 4.8 kilobytes at 4 bits.

## Pixels and colour

- The renderer writes palette indices, never colours (REQ-RND-023): 0 to 7 the colours, 8 transparent, 16 to 79 flashing, 80 to 143 concealed.
- Flash and reveal are palette changes: refill your palette with `neoteletext_core::render::fill_palette` when the flash phase (`render::flash_visible`) or reveal changes. If your display has a hardware palette, no redraw is needed.
- `neoteletext_image::convert` turns indices into RGB565, RGB888, RGBA8888, grey or 1-bit, with byte order, stride and rotation.

## Aspect ratio

Teletext pixels are about 1.2 times taller than wide on a 4:3 television (REQ-RND-004). The library always renders 480 by 500 (or 480 by 250); scale in the port, for example with `neoteletext_image::scale::scale_indices(..., aspect: true)`, or the picture will look squashed.

## What the library will not do

It does not drive displays, generate video, keep display timing, buffer frames or contain code for any particular chip (REQ-ARCH-010). Buffering strategy, direct memory access and scan-out are yours.

## Isolation on microcontrollers

Where the chip has a memory protection unit, run the network stack and the decoder in separate regions (REQ-SEC-021, Phase 2 guidance).
