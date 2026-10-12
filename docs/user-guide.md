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
