# ttx user guide

`ttx` is the command-line receiver of neoteletext (REQ-DOC-002). It decodes World System Teletext (ETSI EN 300 706, Levels 1.0 and 1.5) from recordings, live feeds and web services, and shows pages in the terminal, writes them as text or images, or converts them between formats.

This build targets 64-bit Intel and AMD Linux and macOS. Graphics in the terminal use iTerm2 inline images; every other terminal gets coloured text.

## Building

```sh
cargo build --release -p neoteletext-cli
# the program is target/release/ttx
```

Every part is on by default. A smaller program leaves parts out (decision E.61):

| Feature | What it adds |
|---|---|
| `view` | `ttx view`, the interactive viewer, and image sizing to fit the terminal in `show` |
| `live` | Live feeds and web services (`zxnet:`, `wss://`, `http(s)://`), `check --descriptor`, and the TLS code they need |
| `slice` | `ttx slice` |

```sh
cargo build --release -p neoteletext-cli --no-default-features --features view
```

That program reads files, shows and converts pages, and is about half the size, with no TLS code. Every command stays in the help text. A command whose part is left out exits with status 2 and says the build does not include it. Without `view`, `show` draws images at scale 1 unless `--scale` is given.

The toolchain is pinned in `rust-toolchain.toml`. `scripts/check.sh full` runs every gate: formatting, clippy, the tests, the layer and constant checks, and the build for the bare-metal target.

## Commands

| Command | What it does |
|---|---|
| `ttx list INPUT` | Lists the pages and sub-pages found, with reception counts and error statistics. |
| `ttx show INPUT PAGE` | Shows one page in the terminal and exits. |
| `ttx view INPUT` | Browses pages interactively. |
| `ttx text INPUT PAGE` | Writes one page, or `all`, as UTF-8 text on standard output. |
| `ttx render INPUT PAGE -o FILE` | Writes one page as a PNG or PPM image; with `all`, `-o` names a directory. |
| `ttx convert INPUT --to t42\|tti -o FILE` | Converts a recording or page files to T42 packets or TTI pages. `--page 100` keeps one page. |
| `ttx slice RAW -o OUT.t42` | Slices raw sampled video into T42 packets and prints quality statistics. |
| `ttx services INPUT` | Lists the teletext streams, languages and subtitle pages of a transport stream. |
| `ttx service-info INPUT` | Prints the broadcast service data of packet 8/30: network, time, programme label and status message. |
| `ttx check FILES...` | Checks TTI page files for authoring problems. `--descriptor FILE` resolves link targets against a service descriptor. |
| `ttx font convert INPUT OUTPUT` | Converts a BDF font of another cell size to the 12 × 10 form the browser loads, only where the conversion is exact. See "Fonts" below. |
| `ttx font sets FONT` | Reports, for each of the 24 teletext character sets, whether a 12 × 10 font conforms for it (REQ-FNT-011), how many of its 96 positions it draws, and the first rule it fails; `--library-line NAME` prints the font's `c` line for a library's `fonts.txt`. |
| `ttx contents FOLDER` | Writes a channel's `contents.txt` to standard output from a folder of page files (`NNN.tti`, `NNN.t42`, `index.t42`): a section for each magazine and a dated entry for each page. See [channels.md](channels.md). Exits with 4 when a file was skipped. |

`PAGE` is a page number such as `100`, or a page and sub-code such as `100:0003`. Pages are hexadecimal, as on a television: `1FF` is a valid diagnostic page.

### Exit status

| Status | Meaning |
|---|---|
| 0 | Success. |
| 1 | The requested page was not found. |
| 2 | Invalid usage. |
| 3 | The input could not be read or recognised. |
| 4 | `ttx check` found errors. |

## Inputs

`INPUT` is one of:

- a file;
- `-` for standard input;
- `zxnet:NAME` for a live zxnet feed. The names are `teefax`, `nms-ceefax`, `chunkytext`, `spark`, `ch4` and `test`. A `wss://` channel address also works;
- an `https://` or `http://` address of a web teletext service descriptor (REQUIREMENTS.md area WEB and Appendix C).

The format of a file is detected from its content. `--format` overrides detection:

| `--format` | Format |
|---|---|
| `t42` | 42-byte packets. |
| `t42-reversed` | 42-byte packets with each byte bit-reversed. |
| `t42-framed` | 43-byte packets that start with the framing code. |
| `t42-run-in` | 45-byte packets with clock run-in and framing code. |
| `ts`, `m2ts`, `ts204` | MPEG transport streams with 188, 192 or 204-byte packets (EN 300 472). |
| `pes` | Raw packetised elementary stream. |
| `sliced`, `sliced-be` | Linux sliced VBI records, little or big-endian. |
| `tti` | TTI page files, with the Form Extension (Appendix C) and Link Extension (Appendix E). |
| `ep1` | EP1 page files. |
| `edittf` | edit.tf and zxnet page addresses (the page is in the URL fragment). |
| `dump` | Raw 40 by 25 screen dumps. |

In a transport stream, the first teletext stream is used unless `--pid` names one. `ttx services` lists the choices.

### Live feeds and web services

A live input runs in two processes (REQ-SEC-018). A fetcher process does the networking and TLS; the decoder process never touches the network. Both sandbox themselves on Linux with Landlock and seccomp. On macOS the processes are separated but not sandboxed (Phase 2).

- `--duration SECONDS` sets how long the non-interactive commands collect pages from a live feed before acting. The default is 20 seconds. `show` and `text` stop as soon as the requested page is complete.
- Web services on local-network addresses are refused (REQ-SEC-006). `--allow-local 127.0.0.1` allows one address, for example a test server.
- Plain `http://` works, by the owner's decision E.32. The viewer marks pages that arrived unencrypted.
- `--no-sandbox` turns the operating-system sandbox off, for debugging.
- Offline (REQ-WEB-009): when the network goes, pages already received stay viewable. A page that has not been received or confirmed current for 60 seconds shows "?" in the status area. Asking for a page that was never received shows "Page not available offline". The page on screen is asked for again every 10 seconds, so it is the first one updated when the network returns.

## Outputs

### The terminal

`show` and `view` choose the graphics with `--graphics`:

- `auto` (the default): iTerm2 inline images when `TERM_PROGRAM` is `iTerm.app` or `LC_TERMINAL` is `iTerm2`, otherwise text.
- `iterm2`: the page is rendered at full resolution and sent as a PNG inline image (decision E.37).
- `text`: characters with ANSI colour. Mosaics use the Unicode block mosaic characters; separated mosaics need a font with Unicode 16 separated mosaics, or `--legacy-mosaics` draws them contiguous. `--colours8` uses the eight basic colours instead of 24-bit colour.

`--scale N` sets a whole-number scale factor; 0 fits the terminal. `--aspect` stretches the height by 1.2 for 4:3 proportions (REQ-RND-004). `--reveal` shows concealed text.

### Images

`render` writes PNG (the default, with alpha for transparent cells) or PPM with `--image ppm`. The picture is 480 by 500 pixels at scale 1, or 480 by 250 in the 12-by-10 cell mode of the `saa5246a` profile. A page with side panels at Levels 2.5 and 3.5 is wider: 672 pixels for 56 columns. The palette is the page's 32-entry Colour Map.

### Text

`text` writes the page as UTF-8, one line per row. Mosaics become Unicode block mosaics and national characters their Unicode equivalents. At Levels 2.5 and 3.5 rows include their side panels, G3 smooth mosaics and line-drawing characters become the Unicode characters for them (REQ-CHR-011), and DRCS characters, which have no Unicode equivalent, become U+FFFD.

### Levels 2.5 and 3.5

At the default level, 3.5, `ttx` also keeps the enhancement pages a service sends (object pages, DRCS pages and the Magazine Organization Table) and the magazine packets M/29, and draws each page with them: the page's own Colour Map of 32 colours, screen and row colours, side panels, double width and double size, underline, invert, the additional flash modes, the Level 3.5 font styles, the G3 set, objects and DRCS characters. A fault in any of these removes only what depends on it; the Level 1 page always shows. `--level 1.5` shows a page as a Level 1.5 receiver would, without re-acquiring anything. `cargo xtask level25-stream FILE.t42` writes a test stream with a page for each feature, indexed on page 100.

## The viewer

`ttx view INPUT` opens on the initial page (the service's own, or 100).

| Key | Action |
|---|---|
| `0` to `9` | Enter a page number. |
| `r` `g` `y` `c`, or F1 to F4 | Follow the red, green, yellow and cyan links. They work only while row 24 is shown in full (REQ-SEC-024). |
| `i` | Index link. |
| `h` or Home | Initial page. |
| Up or `+`, Down or `-` | Next and previous page. |
| Right and Left | Next and previous sub-page. |
| Alt+Left or Backspace, Alt+Right | Back and forward through the history. |
| `?` | Reveal concealed text. |
| `H` | Hold the page, so that new receptions do not update it. |
| `m` | Mix: cell backgrounds become transparent, as when text is superimposed on the picture. |
| `z` | Zoom: top half, bottom half, off. |
| `G` | Glyphs: original, Bedstead, both side by side (images only). |
| Tab | Enter the page's form, when it has one. |
| `q`, Esc or Ctrl+C | Quit. |

Inside a form, Tab and Shift+Tab move between fields, the arrow keys move within a field, Space ticks boxes and chooses options, Enter submits, and Esc leaves the form. Before a form is sent, the viewer shows where it is going and asks for confirmation; Enter or F4 confirms, any other key cancels (REQ-SEC-009). A link to another service asks for confirmation too: press the same key again.

Row 0 columns 0 to 7 always belong to the receiver: they show the page number being searched for and the security indicators, which a page cannot forge (REQ-SEC-005).

## Options for the decoder

Every command accepts these options. Each one changes a setting of REQUIREMENTS.md Part E.

| Option | Setting | Default |
|---|---|---|
| `--profile NAME` | The compatibility profile (area PRF). | `en300706` |
| `--region N` | The character set group of Table 32, 0 to 15, used when the page does not give one (decision E.4). | From the transport stream's language, otherwise 0 |
| `--parity keep-good\|space\|marker` | How characters that fail their parity check are shown (decision E.8). | `keep-good` |
| `--level 1\|1.5\|2.5\|3.5` | The presentation level, overriding the profile's (REQ-CLI-007, decision E.40). | The profile's level |
| `--no-rounding` | Turns character rounding off (REQ-RND-012). | Rounding on |
| `--single-start-box` | Accepts a single Start Box code (decision E.17). | Two required |
| `--pages N` | Pages and sub-pages kept in memory. | 4096 |
| `-v`, `-vv` | Diagnostics; `-vv` adds the state machine trace (REQ-FSM-010). | Off |

### Profiles

| Setting | `en300706` | `saa5050` | `saa5246a` |
|---|---|---|---|
| Presentation level (E.40) | 3.5 | 1 | 1.5 |
| X/26/15 processed (E.2) | Yes | Yes | No |
| National option | From C12 to C14 | Fixed: English | From C12 to C14 |
| Black colour codes 0/0 and 1/0 (E.18) | Black | No function | Black |
| Row 24 (E.23) | Annex H flag | Never | Whenever X/24 is present |
| Cell size | 12 by 20 | 12 by 20 | 12 by 10 |
| Rounding | On | On | Off |
| Reveal | Latched | Momentary, 160 ms | Latched |

### Every Part E setting

The library's `neoteletext_core::config::Config` holds every setting below. `ttx` exposes the ones in the table above through options and profiles; the rest keep the default.

| Field | Decision | Default |
|---|---|---|
| `level` | REQ-PRF-006, E.40: Level 1, 1.5, 2.5 or 3.5 | Level 3.5 |
| `process_x26_15` | E.2 | On |
| `strict_row_setting` | E.3: the clause 12.3.2 rule instead of Annex A.3 | Off |
| `ignore_out_of_order_triplets` | REQ-L15-009 | Off |
| `default_region` | E.4 | 0 |
| `fixed_national_index` | REQ-PRF-002 | None |
| `parity_policy` | E.8 | Keep the last good character |
| `rolling_header` | E.11 | On |
| `flash_shown_us`, `flash_hidden_us` | E.12: 0.75 hertz, 3 to 1 | 960 ms, 320 ms |
| `rounding` | REQ-RND-012 | On |
| `double_start_box` | E.17 | On |
| `black_colour_codes` | E.18 | On |
| `page_completion_timeout_us` | E.19 | 100 ms |
| `row24` | E.23 | Annex H |
| `framing_tolerance` | E.24: accept one wrong framing-code bit | On |
| `store_hexadecimal_pages` | REQ-ACQ-013 | Off |
| `stale_after_us` | REQ-ACQ-012 | 60 s |
| `reveal`, `momentary_reveal_us` | REQ-NAV-007 | Latched; 160 ms |
| `step_order` | REQ-NAV-002 | Numerical |
| `cell_mode` | REQ-RND-002, REQ-RND-003 | 12 by 20 |
| `trace` | REQ-FSM-010 | Off |
| `status_on_suppressed_header` | REQ-L1-014 | Off |

## Examples

```sh
ttx list recording.t42
ttx show recording.t42 100
ttx view broadcast.ts --pid 0x46
ttx text zxnet:teefax 100
ttx render recording.t42 all -o pages/
ttx convert broadcast.ts --to t42 -o broadcast.t42
ttx view https://example.org/service.json
ttx check pages/*.tti --descriptor service.json
```

## Glyphs

Pages are drawn in Bedstead 3.261, the one bundled font (REQ-FNT-001, decision E.72), with the SAA5050's character rounding. It covers Latin with every national option, Cyrillic, Greek and Hebrew. It has no Arabic, and lacks six Latin national characters (Ă ă Ğ ğ Ƶ ₺) and two G2 characters (― ™); a character it lacks is composed from a base letter and a mark where that is possible, and otherwise drawn as a hollow box. The browser can load other fonts: 12 × 10 BDF files named by the user, a channel or a page (area FNT).

## Fonts

The browser draws pages in Bedstead unless the user, the channel or the page names another font. A channel names its font in `channel.json` (`"font": {"address": "teletext.bdf", "sha256": "…"}`) and a page with a TTI line `TF,address,sha256`; the page's font wins over the channel's, the channel's over yours, and yours over Bedstead, unless you tick "Always use my font". A character the font lacks comes from your font, then Bedstead. Row 0, the tuning screen and the browser's own pages are always in Bedstead, and the status line names the font in use and who chose it.

The Fonts panel (Menu, Fonts) chooses your font by address, from a local file, or from a font library: a folder with a `fonts.txt` index listing fonts by title, each with its SHA-256 (`f`, the file name or address, the title and the checksum, separated by tabs). It also allows or refuses fonts from other sites, which a channel or page may name only with their checksum, and lists the cached fonts (at most 4 MB, 256 KB each), which you can keep, remove or clear. Fonts are read only in confined helper processes. Every other font is a BDF file of 12 × 10 cells, encoded in Unicode, with at most 1,024 characters (REQ-FNT-002); anything else is refused with the line at fault.

The font browser (Menu, Font browser) is a panel on the left that lists Bedstead and the fonts of every library you have added. Click a font, or move through the list with the up and down arrow keys, and it becomes your font: it loads, the page is drawn again in it, and it is saved. Click the page or press Esc to give the arrow keys back to the page. Your font draws only pages that name no font of their own, so on a page or channel that names one the list changes nothing and the panel says so; tick "Always use my font" in the Fonts panel to see yours there too. The panel stays open between runs until you turn it off (REQ-BRW-010). A library can list the same font twice under different names: once a font has loaded, the browser knows what it draws, and while "Show duplicates" is off it hides a font that draws exactly as one above it in the list. Fonts not yet loaded stay listed, so duplicates disappear as you move through the list, and stay hidden afterwards. "Show sample" replaces the page with every glyph of the chosen font, forty to a row in Unicode order, and below them, where there is room, sample text: "The quick brown fox" in both cases, digits and signs, and Cyrillic, Greek and Hebrew pangrams when the font has most of their letters. A character the font lacks shows as a dim hollow box. A font made from a 5- or 6-dot matrix, such as a dump of an SAA5050 chip, is drawn with the chip's character rounding, as Bedstead is; a font drawn at the finer half-dot grid is drawn exactly as given.

A font library on your own machine, such as a clone of a private repository, can be served with `python3 -m http.server 8642 --bind 127.0.0.1` in its folder and added as `http://127.0.0.1:8642/`, with the browser started as `neoteletext --allow-local 127.0.0.1`: the browser fetches fonts without signing in, so it cannot read a private repository directly.

### Character sets and conformance

A teletext decoder held a glyph at every one of the 96 code positions, 2/0 to 7/F, of each character set it supported. A font *conforms* for a character set when, by the rules of REQ-FNT-011: it is a valid 12 × 10 font; it has a glyph for every character the standard assigns to the set's positions; its space is blank, which also draws the positions the standard leaves empty; and none of its characters is blank. The rules look only at the glyphs, never at the font's name. The 24 sets are G0 Latin and its 13 national options, the three G0 Cyrillic options, G0 Greek, Arabic and Hebrew, and G2 Latin, Cyrillic, Greek and Arabic. A font conforming for a set may also meet the *SAA5050 cell* profile: every glyph leaves the top line and the leftmost dot column blank, as the chip did. `ttx font sets FONT` reports all of this:

```text
G0-Cyrillic-2                      96/96  conforms, SAA5050 cell
G0-Cyrillic-3                      90/96  rule 2: no glyph at 2/6
```

A library's `fonts.txt` may state each font's conformance on a `c` line: `c`, the font's file name or address as its `f` line gives it, and the sets separated by spaces, each followed by `*` for the SAA5050 cell profile. `ttx font sets --library-line NAME FONT` writes that line. The browser uses it to list fonts before loading them, and recomputes conformance from the glyphs once a font has loaded.

`ttx font convert INPUT OUTPUT` makes that form from fonts of other sizes, only where nothing is lost:

| Source cell | What the converter does |
|---|---|
| 6 × 10, 5 × 10 | Makes every pixel two pixels wide; a 5-pixel font sits at the right of the 12-pixel cell, leaving the blank column at the left, as Bedstead does. |
| 6 × 9, 5 × 9 | The same, below one blank line at the top, where the SAA5050 leaves it. |
| 12 × 20 | Keeps every second line, only when each pair of lines is identical. |

Any other cell, a glyph larger than its cell, or a 12 × 20 glyph whose pair of lines differs (a font drawn with character rounding, for instance) is refused, naming the first character that fails. A font whose `CHARSET_REGISTRY` is not ISO10646 needs `--map FILE`: one line for each character, its code as the font's `ENCODING` writes it, a tab, and its Unicode code point:

```
65	U+0041
35	U+00A3
```

`FAMILY_NAME`, `COPYRIGHT` and `NOTICE` are carried over. The converter reads only its two files and never the network, and runs under the sandbox on Linux.

