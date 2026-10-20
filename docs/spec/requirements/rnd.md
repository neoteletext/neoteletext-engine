# F.11 Rendering and output (RND)

**REQ-RND-001 — The cell grid as the primary output.**
The cell grid of REQ-L1-001 shall be the primary output of the core library. All other renderers shall be built on it and shall not decode packets themselves.
*Verification:* Inspection.

**REQ-RND-002 — Raster rendering in the SAA5050 style.**
An optional raster renderer shall draw the cell grid in the SAA5050 style:
- Each cell is 12 pixels wide (half-dots of the 6-dot cell) by 20 lines high (the 10 lines of both interlaced fields).
- The whole 25-row page is therefore 480 by 500 pixels.
- Character rounding follows REQ-RND-012.
*Verification:* Test against golden images (REQ-TST-030).

**REQ-RND-003 — Raster rendering in the SAA5246A style.**
*Phase:* Phase 2 (whole requirement), because it belongs to the SAA5246A profile.
The raster renderer shall also offer a 12 by 10 cell mode, which gives a page of 480 by 250 pixels. In this mode, each 5 by 9 glyph dot is two pixels wide and one line high, and there is no rounding.
*Verification:* Test against golden images.

**REQ-RND-004 — Scaling and aspect.**
The raster renderer shall accept whole-number horizontal and vertical scale factors. The documentation shall explain that the 12-by-20 cell corresponds to a 4:3 television display when the image is shown with pixels about 1.2 times taller than they are wide, so that users can correct the aspect ratio.
*Verification:* Demonstration.

**REQ-RND-005 — Colours.**
The eight Level 1 colours shall be rendered from colour lookup table 0 of Table 30, with each primary either 0 or full intensity (255 in 8-bit output). The colour values shall be configurable for users who want a different palette. At Levels 2.5 and 3.5, all 32 entries of the page's Colour Map are rendered as REQ-RND-030 describes.
*Verification:* Test.

**REQ-RND-006 — Transparency.**
Areas that show the picture shall be rendered as fully transparent pixels in formats that support transparency. This covers areas outside boxes on newsflash and subtitle pages, and backgrounds in mix mode (decision E.22). Otherwise they shall be rendered in a configurable fill colour.
*Verification:* Test.

**REQ-RND-007 — Image files.**
The raster renderer shall write the following formats. Each shall be implemented as an ordinary caller of the row renderer (REQ-RND-017):
- the Portable Pixmap format (binary "P6");
- the Portable Network Graphics format with an alpha channel.

The PNG writer may use the `png` crate (REQ-GEN-017).
*Verification:* Test that the files open in standard image viewers and decode to the expected pixels.

**REQ-RND-008 — Plain text output.**
A text renderer shall write a page as 25 lines of UTF-8 text. Characters shall be converted with REQ-CHR-010, and mosaics with REQ-CHR-005. Concealed characters shall be written as spaces unless reveal is on. Attributes shall be written as spaces. All output shall pass the sanitising rules of REQ-SEC-010.
*Verification:* Test.

**REQ-RND-009 — Terminal output with colour.**
*Phase:* Phase 1 for 24-bit and 8-colour output; Phase 2 for the optional double-height line escape sequences.
A terminal renderer shall write a page using ANSI terminal escape sequences, in both 24-bit colour and 8-colour modes. Flash shall use the terminal's blink attribute where available. Double height shall be approximated: top halves are drawn as normal characters and bottom halves as blank cells in the background colour, unless the terminal supports the double-height line escape sequences, in which case those may be used.
All output shall pass the sanitising rules of REQ-SEC-010, and the renderer shall emit only its own escape sequences.
*Verification:* Demonstration.

**REQ-RND-029 — Terminal graphics output.**
A terminal graphics renderer shall show a page as exact pixels inside a terminal, as decision E.37 describes. It shall be an ordinary caller of the row renderer (REQ-RND-017) and of the scaling helper (REQ-RND-024). It shall add nothing to the core.

- **Protocols.** It shall write the kitty graphics protocol, Sixel, and the iTerm2 inline image protocol.
- **Choosing a protocol.** By default it shall pick the first protocol the terminal supports, in the order of decision E.37. It shall fall back to the character renderer of REQ-RND-009 when the terminal supports none of them, or when the output is not a terminal. A command-line option shall force any one of the three protocols or the character renderer.
- **Detection.** Support shall be found by querying the terminal:
  - the kitty graphics query;
  - the Sixel attribute in the response to Primary Device Attributes;
  - the terminal identification, for iTerm2.

  Every reply shall be treated as untrusted input (REQ-SEC-010): bounded in length, parsed strictly, and abandoned after a timeout. A terminal that does not reply shall be treated as having no graphics support.
- **Size.** The page shall be drawn at the largest whole-number scale that fits the terminal, using the cell size the terminal reports in pixels. A command-line option shall set the scale instead. The 4:3 aspect correction of REQ-RND-004 shall be available as an option.
- **Flash and reveal.**
  - With the kitty protocol, the renderer shall transmit each flash phase once and switch between them.
  - With the other protocols, it shall redraw only the rows that REQ-RND-019 marks as changed, where the protocol and the terminal allow a partial redraw, and the whole page otherwise.
- **Terminal multiplexers.** When the renderer detects tmux or GNU screen and cannot confirm that graphics pass through them, it shall use the character renderer and say why in its diagnostics.
- **Escape sequences.** It shall emit only the escape sequences it generates itself (REQ-SEC-010). Image data is encoded from the rendered pixels, so no byte of page content is ever written to the terminal directly.

*Verification:*
- Test that compares the output for a set of pages with recorded output, for each protocol.
- Test that feeds the detection malformed, oversized and missing replies.
- Demonstration in kitty, WezTerm, xterm in its VT340 mode, and iTerm2.

**REQ-RND-010 — Glyph data.**
*Phase:* Phase 1 for the listed Latin, supplementary and G3 coverage; Phase 2 for glyphs of the other sets.
The raster renderer shall use one bundled glyph source, Bedstead (REQ-FNT-001), in which each character is a 5-dot by 9-line pattern placed in a 6 by 10 cell, the SAA5050 geometry (Part C.3). Loaded fonts have their own 12 by 10 form (REQ-FNT-002, REQ-FNT-008). The bundled source shall cover:
- the Latin G0 set with every national option character;
- the Latin G2 set;
- the diacritical marks;
- G3 positions 5/1, 5/B, 5/C and 5/D.

Glyphs for the other sets should be added over time.
*Verification:* Inspection; test of coverage.

**REQ-RND-011 — Double-height glyphs.**
For double height, each glyph line shall be drawn on two output lines. The top half of the cell shall show glyph lines 1 to 5 and the bottom half lines 6 to 10.
*Verification:* Test.

**REQ-RND-012 — Character rounding.**
In the 12-by-20 mode, alphanumeric glyphs shall be rounded using the rule of decision E.13. Rounding shall be switchable off. It shall never be applied to mosaics.
*Verification:* Test against golden images and comparison with SAA5050 reference images (REQ-TST-030).

**REQ-RND-013 — Mosaic geometry.**
Contiguous and separated mosaics shall be drawn with the geometry of decision E.14 in both cell sizes.
*Verification:* Test.

**REQ-RND-014 — Diacritic composition in raster output.**
Characters with diacritical marks shall be drawn as decision E.26 describes.
*Verification:* Test.

**REQ-RND-015 — Pixel-exact determinism.**
For the same cell grid and settings, the raster renderer shall produce identical pixels on every platform.
*Verification:* Golden-image tests on at least two platforms.

**REQ-RND-016 — The cell raster primitive.**
All teletext-specific pixel logic shall be in one pure function, the cell raster.

It takes:
- a cell of the cell grid (REQ-L1-001);
- the line number within the cell (0 to 19 in the 12 by 20 mode, 0 to 9 in the 12 by 10 mode);
- the field (first or second);
- the glyph set;
- the cell mode.

It returns:
- the 12 dots of that line as a bit mask;
- the palette index for the dots (REQ-RND-023);
- the palette index for the gaps (REQ-RND-023).

It shall do all of the following, and nothing outside it shall repeat any of this:
- glyph lookup;
- character rounding (REQ-RND-012), which needs the field and the neighbouring glyph line;
- mosaic geometry (REQ-RND-013);
- the choice of half for double-height characters (REQ-RND-011);
- diacritic composition (REQ-RND-014).

It shall use no memory other than its arguments and the constant glyph data.

At Levels 2.5 and 3.5 the cell raster shall also do, and nothing outside it shall repeat: double width and double size (REQ-RND-034), underline (REQ-RND-035), G3 drawing (REQ-RND-037) and the font styles (REQ-RND-039). In the resolved colour mode its indices are Colour Map indices (REQ-RND-031).
*Amended 20 October 2026 (decision E.95):* DRCS characters are not decoded, and the parts of this requirement that concerned them are removed.
*Reason:* Every renderer and every port calls the same function, so the pixels are the same everywhere (REQ-RND-015). This mirrors the SAA5050, which produced exactly this unit of output for each television line (decision E.29).
*Verification:* Test. The golden tests of REQ-TST-030 shall include tests at this level, independent of any output format.

**REQ-RND-017 — The row renderer: the whole of the pixel output.**
The library's pixel output shall consist of these calls, all of which draw into memory supplied by the caller (decision E.29):

1. **Changed rows.** Returns which of the 25 text rows have changed since the port last asked. This includes rows affected by a change of flash phase or reveal setting, and rows below a changed double-height row (REQ-RND-019).
2. **Render one text row.** Draws text row r into a caller-supplied buffer, with a row stride chosen by the caller, as palette indices (REQ-RND-023) at 8 or 4 bits per pixel. It takes these parameters:
   - the row number;
   - the cell mode: 12 by 20, giving 480 by 20 pixels, or 12 by 10, giving 480 by 10 pixels;
   - the field: both fields, or the first or second field only, giving 10 lines per field in the 12 by 20 mode. The rounding of REQ-RND-012 differs between fields.
   - optionally, whether to draw the top or bottom half of the row, for page zoom (REQ-L1-020).
3. **Render one screen line.** Draws screen line y into a caller-supplied buffer, for ports that work a line at a time. It is built directly on the cell raster (REQ-RND-016).
4. **Current palette.** Returns the palette for the current flash phase and reveal setting (REQ-RND-023).

Each call shall draw only from the cell grid of the displayed page and the viewer state. It shall need nothing from any other row's pixels and shall keep no pixel memory of its own.

At Levels 2.5 and 3.5, a row is wider when side panels are shown (REQ-RND-033), and indices are written in the resolved colour mode (REQ-RND-031).

The library shall not decide how, when or how often the port calls these, nor how the port stores the results. A whole-screen framebuffer, a ring of row buffers and a single row buffer are all equally valid port designs (REQ-ARCH-010).

Memory the port needs for one text row at 480 by 20 pixels is:

| Pixel format | One row |
|---|---|
| 4 bits per pixel | 4.8 kilobytes |
| 8 bits per pixel | 9.6 kilobytes |
| 16-bit colour, after conversion by the port | 19.2 kilobytes |

*Reason:* Every cross-row feature of teletext is resolved when the page is decoded into the cell grid (REQ-L1-008 and REQ-L15-008). A rendered row therefore depends only on its own cells. A pull interface, where the port asks and the library draws into the port's memory, is the smallest possible surface. It is the model of zvbi's page drawing.
*Verification:* Test that renders every row of a set of pages:
- in order, in reverse order, in random order and individually;
- one line at a time;
- one field at a time.

All of these must give identical pixels, which confirms that rows are independent.

**REQ-RND-025 — Withdrawn (underrun policy).**
*Withdrawn on 1 October 2026.* Running out of time against a display or scan is a property of the port's hardware layer, which is outside the library (REQ-ARCH-010). Buffering and timing belong to the port, which renders rows when it chooses (REQ-RND-017), so it can detect and handle this itself. The number is kept and will not be reused.

**REQ-RND-026 — The glyph line cache.**
*Phase:* Phase 2 (whole requirement).
An optional cache shall hold the finished 12-dot mask for every character, every line of the cell and each field, with rounding and mosaic geometry already applied.
- It shall be generated from the cell raster (REQ-RND-016) when the decoder starts, or at build time as constant data.
- For the Latin G0 set with its national option characters and the G1 mosaic set, it shall need about 8 kilobytes, or about 4 kilobytes when only one field is drawn.
- With the cache in use, drawing one line of one cell becomes a table lookup and a colour fill.

The cell raster remains the reference. A test shall compare every cache entry with the cell raster's output. Characters not in the cache, such as G2 and G3 overwrites, shall fall back to the cell raster.
*Verification:* Test comparing the cache with the cell raster for every entry, and comparing whole screens drawn with and without the cache.

**REQ-RND-027 — Withdrawn (processor budget on RP2040 hardware).**
*Withdrawn on 1 October 2026.* The library contains no hardware-specific code or targets (REQ-ARCH-010). A hardware-independent measure of drawing cost is now part of REQ-TST-050, so that ports can budget for their own hardware. The number is kept and will not be reused.

**REQ-RND-018 — Pixel format helpers.**
The library shall provide optional helper functions that convert a rendered row or line of palette indices into these pixel formats:
- 16-bit red-green-blue (5, 6 and 5 bits);
- 24-bit and 32-bit red-green-blue, with alpha for the 32-bit form;
- 8-bit grey;
- 1-bit black and white.

Each helper shall support:
- both byte orders, where the format has more than one byte;
- a row stride chosen by the caller;
- rotation by 90, 180 and 270 degrees, for hardware layers that expect pixels in column order.

Ports are free to use their own conversion, or a palette in their hardware, instead.
*Verification:* Test for each format, byte order and rotation.

**REQ-RND-019 — Drawing only what changed.**
The decoder shall mark each screen row as changed whenever its content changes. For a row containing double height, the row below shall be marked too. Rows containing flashing or concealed cells shall be recorded, so that a change of flash phase or reveal state affects only those rows. The renderers shall draw only changed rows unless a full redraw is requested. Drawing shall be decoupled from decoding: the program draws when rows have changed or the flash state changes, not once per packet received. In the resolved colour mode of Levels 2.5 and 3.5, change tracking follows REQ-RND-032.
*Verification:* Test that counts the rows drawn for a flash change and for a single-row update.

**REQ-RND-021 — Reference callers.**
The library shall include these reference users of the row renderer and the platform interface (REQ-ARCH-009). None of them shows pixels on a screen or touches hardware (REQ-ARCH-010):
- the whole-screen convenience loop (REQ-RND-022);
- the image file writers (REQ-RND-007);
- a test harness that calls the row renderer in unusual orders and records the results, used for the independence test of REQ-RND-017 and to test change tracking.

Ports for specific screens, operating systems or devices are written by their users against the platform interface, and are not part of this library.
*Verification:* Test.

**REQ-RND-022 — Whole-screen convenience loop.**
The library shall provide a convenience function that renders all changed rows, or all rows, into a whole-screen indexed framebuffer supplied by the caller, at 8 or 4 bits per pixel. It does this by calling the row renderer for each row at the right offset. In the 12 by 20 mode, a 480 by 500 pixel screen needs 240 kilobytes at 8 bits per pixel, or 120 kilobytes at 4 bits. With both side panels at their widest (REQ-RND-033), a 672 by 500 pixel screen needs 336 kilobytes at 8 bits per pixel, or 168 kilobytes at 4 bits.

It is a convenience for ports that keep a framebuffer, as a Doom port does. It adds no capability that a port could not write itself in a few lines, and ports need not use it.
*Verification:* Test that the loop's output equals rendering each row separately.

**REQ-RND-023 — Palette indices, with flash and reveal as palette changes.**
The renderer shall produce palette indices, never final colours. The index assignment shall be:

| Index | Meaning |
|---|---|
| 0 to 7 | The eight teletext colours |
| 8 | Transparent ("picture") |
| 16 + 8 × f + b | A foreground pixel of a **flashing** cell whose foreground colour is f and background colour is b |
| 80 + 8 × f + b | A foreground pixel of a **concealed** cell whose foreground colour is f and background colour is b |

The indices of flashing and concealed pixels do not depend on the flash phase or the reveal setting. The library shall provide a function that fills a palette of 144 real colours for a given flash phase and reveal setting:
- For flashing pixels, index 16 + 8 × f + b maps to colour f in the visible phase and colour b in the hidden phase (decision E.12).
- For concealed pixels, index 80 + 8 × f + b maps to colour b normally and to colour f when revealed.
- A cell that is both flashing and concealed shall use the concealed range while reveal is off and the flashing range while reveal is on. Changing reveal therefore redraws the rows that hold such cells, which REQ-RND-019 already tracks. Every other flash or reveal change is a palette change only.

Wherever the port's hardware layer or framebuffer uses a palette, flash and reveal therefore need no redrawing at all, only a new palette. This is the technique Doom used for its screen-wide colour effects.

The palette's base colours shall come from REQ-RND-005. Colour conversion shall happen only in the helpers of REQ-RND-018 or in the port.

This palette mode is the mode of Levels 1 and 1.5. At Levels 2.5 and 3.5 the resolved colour mode of REQ-RND-031 is used, and the palette mode is refused for grids that use colours outside CLUT 0 or flash modes of Level 2.5 (decision E.55).
*Verification:* Test that a flash change and a reveal change alter only the palette, and that the result matches a full redraw.

**REQ-RND-024 — No scaling in the core; aspect correction at the edge.**
The core renderer shall only ever produce the two fixed resolutions of REQ-RND-002 and REQ-RND-003. All scaling shall happen in the port, optionally using the helper below. The library shall provide a helper that scales by whole numbers and optionally stretches the height by 1.2 to give correct 4:3 proportions (REQ-RND-004). It shall use a nearest-pixel method by default and a smoothed method as an option. The porting guide shall warn about the aspect factor explicitly.
*Verification:* Test of the helper; inspection of the porting guide.

**REQ-RND-028 — Withdrawn (the original glyph set).**
*Withdrawn on 12 October 2026* by decision E.72: Bedstead is the bundled font, and other fonts are loaded at run time (area FNT). The original set drawn under decision E.62 remains in the git history up to commit 9dc932d. The number is kept and will not be reused.

**REQ-RND-020 — Glyph provenance.**
The bundled glyph data is Bedstead 3.261 (decision E.72, REQ-FNT-001), whose provenance and licence are recorded in `third_party/bedstead/PROVENANCE.md`. The glyph loader shall treat the glyph set as replaceable data. No other glyph set shall be included in the repository; other fonts are loaded at run time (area FNT). No glyph may be copied from the contents of a character generator ROM into the repository other than through Bedstead.
*Amended 12 October 2026* (decision E.72): Bedstead is the bundled font, not a temporary substitute.
*Verification:* Inspection of the repository's glyph data and its provenance.

**REQ-RND-030 — The 32-entry Colour Map in output.**
At Levels 2.5 and 3.5, colours shall be produced from the page's Colour Map (REQ-L25-030), which travels with the grid. Each 12-bit entry shall be converted to 8 bits for each primary by multiplying each 4-bit value by 17, so that 15 gives 255. Index 8, CLUT 1 entry 0, shall be transparent (REQ-RND-006). The pixel format helpers of REQ-RND-018 and the image writers shall take the Colour Map as their palette. The user's replacement colours of REQ-RND-005 shall replace the Table 30 defaults before any page data redefines them.
*Verification:* Test that every index converts as Table 30 gives, and that a page redefining CLUTs 0 to 3 converts with its own values.

**REQ-RND-031 — The resolved colour mode.**
The row renderer shall offer a resolved colour mode as well as the palette mode of REQ-RND-023 (decision E.55). In the resolved mode, it shall take a flash phase (REQ-RND-040) and the reveal setting, apply them to each cell as REQ-RND-036 describes, and write Colour Map indices 0 to 31 into the caller's buffer. The resolved mode shall be used at Levels 2.5 and 3.5. The palette mode shall be refused, with an error, for a grid that uses any colour outside CLUT 0 or any flash mode of Level 2.5.
*Verification:* Test that the resolved mode matches the palette mode on Level 1 pages, and that the palette mode is refused for each case named.

**REQ-RND-032 — Change tracking in the resolved colour mode.**
The change tracker of REQ-RND-019 shall record which rows hold flashing cells, of any flash mode, and which hold concealed cells. When the flash phase moves from one twelfth-of-a-second unit to the next, it shall report as changed exactly the rows that hold a cell whose displayed colours differ between the two units. When the reveal setting changes, it shall report the rows that hold concealed cells. When the grid is built again because enhancement data arrived (REQ-ACQ-022), it shall report the rows whose cells changed.
*Verification:* Test that counts the rows reported for each unit of the flash cycle, for a reveal change and for the arrival of an object page.

**REQ-RND-033 — Side panels and the width of a row.**
At Levels 2.5 and 3.5, the row renderer shall compose each row from the left panel (columns 72 − left width to 71), the main page (columns 0 to 39) and the right panel (columns 40 to 40 + right width − 1), in that order, so that a row is (40 + left width + right width) × 12 pixels wide (decision E.49). In row 0, the panel cells shall be drawn in the screen colour. The renderer shall report the row width, and the grid shall report the screen colour, so that the port and the whole-screen loop of REQ-RND-022 can fill the borders around the rows in the screen colour. With both panels at their widest, a 12 by 20 row of 56 columns needs 672 by 20 pixels: 6.7 kilobytes at 4 bits per pixel, or 13.4 kilobytes at 8 bits.
*Verification:* Test for each width of the table of decision E.49.

**REQ-RND-034 — Double width and double size.**
A cell whose horizontal part is "left half" shall show the left six half-dots of the character's normal-width line, each drawn twice, and a cell whose horizontal part is "right half" shall show the right six half-dots, each drawn twice. The normal-width line shall first be produced with rounding, mosaic geometry and font styles applied. Double size shall combine this with the vertical parts of REQ-RND-011.
*Verification:* Test against golden cell-raster outputs for alphanumerics, mosaics and G3 characters at double width and double size.

**REQ-RND-035 — Underline.**
Underline shall be drawn as one line of foreground colour on the last dot line of the cell, across the whole cell width. In the 12 by 20 cell that is line 19, so it appears on both fields; in the 12 by 10 cell it is line 9 (decision E.47). It shall be drawn for G0 characters, G1 characters of columns 4 and 5, and G2 characters, and never for G3 characters. For a double-height character, only its bottom half shall show the underline.
*Amended 20 October 2026 (decision E.88):* a render option, `underline` in `RenderOptions`, turns the underline off for G0 and G2 characters, for a port that draws its own links over the page; it is on by default, and separated mosaics are unaffected.
*Amended 20 October 2026 (decision E.95):* DRCS characters are not decoded, and the parts of this requirement that concerned them are removed.
*Verification:* Test against golden cell-raster outputs.

**REQ-RND-036 — Invert, flash and conceal when drawing.**
For each cell the renderer shall, in this order (decisions E.47 and E.48):
1. swap the foreground and background colours if the cell is inverted;
2. apply flash: in the off phase, normal and inverted flash show the background colour in place of the foreground, while adjacent-CLUT flash replaces each foreground index c by c exclusive-OR 8;
3. apply conceal: unless reveal is on, a concealed cell shows a space in its background colour.

*Amended 20 October 2026 (decision E.95):* DRCS characters are not decoded, and the parts of this requirement that concerned them are removed.
*Verification:* Test for every combination of invert, each flash mode and conceal.

**REQ-RND-037 — G3 glyph data.**
The G3 characters shall be drawn from glyph data generated by an `xtask` command from the project's own geometry, as decision E.50 describes. The data shall be generated separately for the 12 by 10 and the 12 by 20 cells, at the full 12 half-dot width, and stored as replaceable text data with its provenance recorded, like the other glyph sets (REQ-RND-020). The generated data shall be used for G3 characters at every presentation level. Character rounding (REQ-RND-012) shall not be applied to G3 characters. The generator shall be deterministic, and continuous integration shall check that the stored data matches its output.
*Verification:* Test that regenerating the data gives identical files; golden cell-raster outputs for every G3 position in both cell sizes; the symmetry checks of `research/notes-level25-drcs.md` section 9.3 (column 3 mirrors column 2, and columns 6 and 7 are the complements of columns 2 and 3).

**REQ-RND-038 — Withdrawn: Drawing DRCS characters.** Withdrawn on 20 October 2026: DRCS is not decoded (decision E.95).

**REQ-RND-039 — Drawing the Level 3.5 font styles.**
For a G0 or G2 character with a font style, the renderer shall take the glyph line after rounding and then:
- for bold, combine the line by OR with itself shifted one half-dot to the right;
- for italic, shift the line to the right by the number of lines below it in the cell divided by 4, rounded down, in half-dots (0 to 4 half-dots in the 12 by 20 cell, and 0 to 2 in the 12 by 10 cell).

Bold shall be applied before italic. Dots shifted beyond the twelfth half-dot shall be dropped. Proportional spacing shall be drawn at the fixed pitch (decision E.50). The shifts shall use only integer shifts, with no division (REQ-GEN-016).
*Verification:* Test against golden cell-raster outputs for each style and each cell size.

**REQ-RND-040 — Flash timing at Levels 2.5 and 3.5.**
The library shall provide a function that turns the caller's time (REQ-GEN-005) into the flash unit, 0 to 11, of decision E.48, and a function that says whether a cell with a given flash rate and phase is in its on or off phase in a given unit. The unit shall be computed once per frame, not per cell or per pixel (REQ-GEN-016). At Levels 1 and 1.5, the timing of decision E.12 shall be used.
*Verification:* Test of every rate and phase in every unit, and of incremental and decremental runs.

**REQ-RND-041 — Text and terminal output at Levels 2.5 and 3.5.**
The text renderer (REQ-RND-008) shall write each row composed with its side panels, in the order of REQ-RND-033. It shall convert G3 characters as REQ-CHR-011 describes. The terminal renderer (REQ-RND-009) shall take 24-bit colours from the page's Colour Map. In its 8-colour mode it shall use the nearest of the eight Level 1 colours. Transparent cells shall be written in the screen colour. All output shall pass the sanitising rules of REQ-SEC-010.
*Amended 20 October 2026 (decision E.95):* DRCS characters are not decoded, and the parts of this requirement that concerned them are removed.
*Verification:* Test with snapshot outputs of pages with side panels, G3 and redefined colours.
