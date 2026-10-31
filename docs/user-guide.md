# neottx user guide

`neottx` is the command-line receiver of neoteletext (REQ-DOC-002). It decodes World System Teletext (ETSI EN 300 706, Levels 1.0 and 1.5) from recordings, live feeds and web services, and shows pages in the terminal, writes them as text or images, or converts them between formats.

This build targets 64-bit Intel and AMD Linux and macOS. Graphics in the terminal use iTerm2 inline images; every other terminal gets coloured text.

## Building

```sh
cargo build --release -p neoteletext-cli
# the program is target/release/neottx
```

Every part below is on by default. A smaller program leaves parts out (decision E.61):

| Feature | What it adds |
|---|---|
| `view` | `neottx view`, the interactive viewer, and image sizing to fit the terminal in `show` |
| `live` | Live feeds and web services (`zxnet:`, `wss://`, `http(s)://`), `check --descriptor`, and the TLS code they need |
| `slice` | `neottx slice` |

```sh
cargo build --release -p neoteletext-cli --no-default-features --features view
```

Two capabilities of Levels 2.5 and 3.5 are off by default (decision E.105) and are named to build them in: `objects` and `additional-flash-functions`:

```sh
cargo build --release -p neoteletext-cli --features objects,additional-flash-functions
```

That program with `--no-default-features --features view` reads files, shows and converts pages, and is about half the size, with no TLS code. Every command stays in the help text. A command whose part is left out exits with status 2 and says the build does not include it. Without `view`, `show` draws images at scale 1 unless `--scale` is given.

The toolchain is pinned in `rust-toolchain.toml`. `scripts/check.sh full` runs every gate: formatting, clippy, the tests, the layer and constant checks, and the build for the bare-metal target.

## Commands

| Command | What it does |
|---|---|
| `neottx list INPUT` | Lists the pages and sub-pages found, with reception counts and error statistics. |
| `neottx show INPUT PAGE` | Shows one page in the terminal and exits. |
| `neottx view INPUT` | Browses pages interactively. |
| `neottx text INPUT PAGE` | Writes one page, or `all`, as UTF-8 text on standard output. |
| `neottx render INPUT PAGE -o FILE` | Writes one page as a PNG or PPM image; with `all`, `-o` names a directory. |
| `neottx convert INPUT --to t42\|tti -o FILE` | Converts a recording or page files to T42 packets or TTI pages. `--page 100` keeps one page. |
| `neottx slice RAW -o OUT.t42` | Slices raw sampled video into T42 packets and prints quality statistics. |
| `neottx services INPUT` | Lists the teletext streams, languages and subtitle pages of a transport stream. |
| `neottx service-info INPUT` | Prints the broadcast service data of packet 8/30: network, time, programme label and status message. |
| `neottx check FILES...` | Checks TTI page files for authoring problems. `--descriptor FILE` resolves link targets against a service descriptor. |
| `neottx font convert INPUT OUTPUT` | Converts a BDF font of another cell size to the 12 × 10 form `neottx render --font` loads, only where the conversion is exact. See "Fonts" below. |
| `neottx font sets FONT` | Reports, for each of the 24 teletext character sets, whether a 12 × 10 font conforms for it (REQ-FNT-011), how many of its 96 positions it draws, and the first rule it fails; `--library-line NAME` prints the font's `c` line for a library's `fonts.txt`. |
| `neottx sound --page PAGE IN.wav OUT.t42` | Writes a sound as the audio profile's packets, with the command that plays it on `PAGE`: IMA ADPCM by default, `--codec pcm` for 16-bit PCM. The WAVE file is 16-bit PCM at 8, 11.025, 16, 22.05, 24, 32, 44.1 or 48 kHz; stereo is mixed to mono. `--channel 0-3`, `--gain 0-100`, `--loop` and `--repeat N` set the rest. Append the output to a channel's T42 stream, and a receiver that plays sound plays it while that page is open. See `docs/spec/design/audio.md`. |
| `neottx qr TEXT --page PAGE OUT.tti` | Writes `TEXT`, such as a qrkeyboard join address, as a QR code in a page's mosaics, from row 2 unless `--row` says otherwise, so a channel can show a code a phone scans. See `neoteletext-common/INPUT.md` section 10. |
| `neottx keyboard WS-ADDRESS` | Makes a phone the keyboard of a channel that takes input: it shows a qrkeyboard QR in the terminal (and with `--qr-page PAGE --qr-output FILE` writes it as a page), pairs with the phone that scans it, and sends what is typed on the phone to the channel's push WebSocket in the input protocol, once the channel has answered its HELLO; it stops if the channel does not answer within 10 seconds. `--pin` pairs with a PIN instead of a key in the QR; `--relay` names another relay. Anyone who can see the code can type. |
| `neottx contents FOLDER` | Writes a channel's `contents.txt` to standard output from a folder of page files (`NNN.tti`, `NNN.t42`, `index.t42`): a section for each magazine and a dated entry for each page. See [channels.md](channels.md). Exits with 4 when a file was skipped. |

`PAGE` is a page number such as `100`, or a page and sub-code such as `100/0003`. Pages are hexadecimal, as on a television: `1FF` is a valid diagnostic page.

### Exit status

| Status | Meaning |
|---|---|
| 0 | Success. |
| 1 | The requested page was not found. |
| 2 | Invalid usage. |
| 3 | The input could not be read or recognised. |
| 4 | `neottx check` found errors. |

## Inputs

`INPUT` is one of:

- a file;
- `-` for standard input;
- `zxnet:NAME` for a live zxnet feed. The names are `teefax`, `nms-ceefax`, `chunkytext`, `spark`, `ch4` and `test`. A `wss://` channel address also works;
- an `https://` or `http://` address of a channel, its folder ending in `/`, or its `channel.json`, such as `https://app.neottx.com/demosite/`. Any address of the channel URL map opens (`neoteletext-common/SERVER.md` section 4): `#302` starts at page 302, `#302/0002` at that sub-page, and `#302?name=value` sends the channel's program those page parameters with that page, and no other. `#contents` and `#wall` start at the channel's first page, as the viewer shows neither. `302.tti` reads as `#302`. Anything else after `#` is refused;
- any other `https://` or `http://` address, of a web teletext service descriptor (REQUIREMENTS.md area WEB);
- a `neoteletext://` link, which opens the `https://` or `http://` address it wraps, as in `neoteletext://https://app.neottx.com/demosite/`. `neottx` registers nothing with the operating system: the browser does that. A fragment after the address, which names a page in the browser, is refused here.

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
| `tti` | TTI page files, with the Link Extension (Appendix E). The lines of the withdrawn Form Extension are ignored, with a diagnostic. |
| `ep1` | EP1 page files. |
| `edittf` | edit.tf and zxnet page addresses (the page is in the URL fragment). |
| `dump` | Raw 40 by 25 screen dumps. |

In a transport stream, the first teletext stream is used unless `--pid` names one. `neottx services` lists the choices.

### Live feeds and web services

A live input runs in two processes (REQ-SEC-018). A fetcher process does the networking and TLS; the decoder process never touches the network. Both sandbox themselves on Linux with Landlock and seccomp. On macOS the processes are separated but not sandboxed (Phase 2).

- `--duration SECONDS` sets how long the non-interactive commands collect pages from a live feed before acting. The default is 20 seconds. `show` and `text` stop as soon as the requested page is complete.
- Web services on local-network addresses are refused (REQ-SEC-006). `--allow-local 127.0.0.1` allows one address, for example a test server.
- `--proxy http://host:port` or `--proxy socks5://host:port` sends every connection through that proxy, TLS still end to end (REQ-WEB-032); with a proxy, `localhost`, names of one label such as `router`, and names under `.local`, `.internal`, `.home.arpa`, `.lan`, `.corp`, `.intranet`, `.private` and `.localdomain` are refused too, since the proxy resolves the rest and only the name can be checked. A proxy that needs a password is not supported.
- Plain `http://` works, by the owner's decision E.32. The viewer marks pages that arrived unencrypted.
- `--no-sandbox` turns the operating-system sandbox off, for debugging.
- Offline (REQ-WEB-009): when the network goes, pages already received stay viewable. A page that has not been received or confirmed current for 60 seconds shows "?" in the status area. Asking for a page that was never received shows "Page not available offline". The page on screen is asked for again every 10 seconds, so it is the first one updated when the network returns.

## Outputs

### The terminal

`show` and `view` choose the graphics with `--graphics`:

- `auto` (the default): iTerm2 inline images when `TERM_PROGRAM` is `iTerm.app` or `LC_TERMINAL` is `iTerm2`, otherwise text.
- `iterm2`: the page is rendered at full resolution and sent as a PNG inline image (decision E.37).
- `text`: characters with ANSI colour. Mosaics use the Unicode block mosaic characters; separated mosaics need a font with Unicode 16 separated mosaics, or `--legacy-mosaics` draws them contiguous. `--colours8` uses the eight basic colours instead of 24-bit colour.

`--scale N` sets a whole-number scale factor; 0 fits the terminal. `--aspect` stretches the width by 16/13 (about 1.23) for the proportions of a 4:3 television, on which each pixel was wider than tall (REQ-RND-004). `--reveal` shows concealed text.

### Images

`render` writes PNG (the default, with alpha for transparent cells) or PPM with `--image ppm`. The picture is 480 by 500 pixels at scale 1, or 480 by 250 in the 12-by-10 cell mode of the `saa5246a` profile. The palette is the page's 32-entry Colour Map.

### Text

`text` writes the page as UTF-8, one line per row. Mosaics become Unicode block mosaics and national characters their Unicode equivalents. At Levels 2.5 and 3.5 G3 smooth mosaics and line-drawing characters become the Unicode characters for them (REQ-CHR-011).

### Levels 2.5 and 3.5

At the default level, 3.5, `neottx` also keeps the enhancement pages a service sends (object pages and the Magazine Organization Table) and the magazine packets M/29, and draws each page with them: the page's own Colour Map of 32 colours (CLUTs 2 and 3 from X/28/0 and M/29/0, and at Level 3.5 CLUTs 0 and 1 from X/28/4 and M/29/4), screen and row colours, the non-spacing attributes (colours, double width and double size, underline, invert and the rest of Table 27) and the G3 set. Objects and the Additional Flash Functions are drawn by a build that names them (above). Side panels are never drawn: a page is always 40 columns (decision DEC-1636). Downloadable characters (DRCS) are not drawn: the Level 1 character beneath shows (decision E.95). Nor are the Level 3.5 font styles, bold, italic and proportional spacing (decision E.96). A fault in any of these removes only what depends on it; the Level 1 page always shows. `--level 1.5` shows a page as a Level 1.5 receiver would, without re-acquiring anything. `cargo xtask level25-stream FILE.t42` writes a test stream with a page for each feature, indexed on page 100.

## The viewer

`neottx view INPUT` opens on the initial page (the service's own, or 100).

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
| `q`, Esc or Ctrl+C | Quit. |

A link to another service asks for confirmation: press the same key again.

On a channel that takes input (its descriptor's `push` says `"input": true`), the viewer greets the channel when its WebSocket opens, and once the channel answers, every key goes to it, as a terminal's keys go to its host: digits, Escape and Ctrl+C included. Printable keys go as text and the rest as key events, with Shift, Control and Alt. The channel decides what they mean; videotex channels take `*nnn#` for a page, `#` for the next and `*#` for the one before. A paste goes to the channel as one paste, and the terminal's focus as the channel's, where the terminal reports them; in a terminal with the Kitty keyboard protocol, releases and repeats go too when the channel asks for them. Ctrl+] leaves the keys to the viewer, telling the channel the focus went, and gives them back. A channel that does not answer within 10 seconds is said not to take input. See `neoteletext-common/INPUT.md`.

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
| `-v`, `-vv` | Diagnostics, and a channel's log lines (`LG`, REQ-INP-026); `-vv` adds the state machine trace (REQ-FSM-010). | Off |

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

The library's `neoteletext_core::config::Config` holds every setting below. `neottx` exposes the ones in the table above through options and profiles; the rest keep the default.

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
neottx list recording.t42
neottx show recording.t42 100
neottx view broadcast.ts --pid 0x46
neottx text zxnet:teefax 100
neottx render recording.t42 all -o pages/
neottx convert broadcast.ts --to t42 -o broadcast.t42
neottx view https://example.org/service.json
neottx view https://app.neottx.com/demosite/
neottx check pages/*.tti --descriptor service.json
neottx sound --page 100 chime.wav chime.t42 && cat pages.t42 chime.t42 > channel.t42
```

## Glyphs

Pages are drawn in Bedstead 3.261, the one bundled font (REQ-FNT-001, decision E.72), with the SAA5050's character rounding. It covers Latin with every national option, Cyrillic, Greek and Hebrew. It has no Arabic, and lacks six Latin national characters (Ă ă Ğ ğ Ƶ ₺) and two G2 characters (― ™); a character it lacks is composed from a base letter and a mark where that is possible, and otherwise drawn as a hollow box. `neottx render --font` draws in another font: a 12 × 10 or 12 × 20 BDF file (area FNT).

## Fonts

Pages are drawn in Bedstead unless another font is named. `neottx render --font FILE` draws a page's text in a loaded font: a BDF file of 12 × 10 or 12 × 20 cells, encoded in Unicode, with at most 1,024 characters (REQ-FNT-002); anything else is refused with the line at fault. A character the font lacks is drawn in Bedstead. Row 0's status area is always drawn in Bedstead. A channel or a page cannot name a font (decision E.111): `channel.json` refuses a `font` member, and the page checker reports a TTI `TF` line as a withdrawn command.

A font library is a folder with a `fonts.txt` index listing fonts by title, each with its SHA-256 (`f`, the file name or address, the title and the checksum, separated by tabs). `third_party/noto/` is such a library. The windowed browser that chooses fonts from libraries is maintained in the separate neoteletext-browser repository.

### Character sets and conformance

A teletext decoder held a glyph at every one of the 96 code positions, 2/0 to 7/F, of each character set it supported. A font *conforms* for a character set when, by the rules of REQ-FNT-011: it is a valid 12 × 10 font; it has a glyph for every character the standard assigns to the set's positions; its space is blank, which also draws the positions the standard leaves empty; and none of its characters is blank. The rules look only at the glyphs, never at the font's name. The 24 sets are G0 Latin and its 13 national options, the three G0 Cyrillic options, G0 Greek, Arabic and Hebrew, and G2 Latin, Cyrillic, Greek and Arabic. A font conforming for a set may also meet the *SAA5050 cell* profile: every glyph leaves the top line and the leftmost dot column blank, as the chip did. `neottx font sets FONT` reports all of this:

```text
G0-Cyrillic-2                      96/96  conforms, SAA5050 cell
G0-Cyrillic-3                      90/96  rule 2: no glyph at 2/6
```

A library's `fonts.txt` may state each font's conformance on a `c` line: `c`, the font's file name or address as its `f` line gives it, and the sets separated by spaces, each followed by `*` for the SAA5050 cell profile. `neottx font sets --library-line NAME FONT` writes that line. A program can use it to list fonts before loading them, and recomputes conformance from the glyphs once a font has loaded.

`neottx font convert INPUT OUTPUT` makes that form from fonts of other sizes, only where nothing is lost:

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

