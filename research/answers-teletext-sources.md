# Sources of real teletext data, build targets and microcontroller dependencies: answers

Research done on 10 October 2026 (local time; 9 October UTC), in the project's own headful Chrome over the DevTools protocol, with build checks on `oldvga`.

Legend: **[V]** = verified against the cited source today. **[U]** = uncertain or my inference, so check it before relying on it.

Clean-room note: Teletext64U is GPL-3.0. It was read only for facts (feed addresses and data formats). None of its code is used or copied. ZDF's anti-bot check, and how Teletext64U gets past it, were deliberately not used.

---

## 1. Summary: the newest real data

| Source | Newest data | Form | Real broadcaster? |
|---|---|---|---|
| NOS Teletekst, `teletekst-data.nos.nl/page/<n>` | **Live.** Header clock 00:56:59 Dutch time, fetched at 22:57 UTC. [V] | 25 rows of 40 bytes of Level 1 codes; accents converted to ISO 8859-1 | **Yes** |
| Teefax, zxnet WebSocket feed | **Live.** The 8/30 clock read 22:48:41 UTC with the system clock at 22:49:21 UTC. [V] | T42 packet bytes (bytes 6 to 45) for the requested page | No, a hobby service |
| SRG SSR web interface (SRF 1, SRF zwei, SRF info, RTS 1, RTS 2, RSI LA 1, RSI LA 2) | **Live.** SRF 1 page 100 was last modified at 22:31:12 UTC. [V] | Whole pages in EP1 format, base64 inside JSON | **Yes** |
| TVP Poland, off-air transport stream (TSDuck repository) | 10 November 2023 [V] | DVB teletext (EN 300 472) | Yes |
| NPO and RTL Netherlands, off-air transport streams (TSDuck) | 21 June 2020 [V] | DVB teletext | Yes |
| RAI and Mediaset, Italy, terrestrial (TSDuck) | Date not given | DVB teletext | Yes |
| Hot Bird 13.0°E, Mediaset and Euronews (TSDuck) | 11 May 2018 [V] | DVB teletext | Yes |
| UK recoveries from tape (teletext-restoration) | 1978 to 2012 [V] | T42 | Yes |
| zxnet `ch4` feed | A broadcast of **15 July 2007**, replayed in real time (decoded from its 8/30 packet) [V] | T42 packet bytes | Yes, as a replay |

No real broadcaster was found publishing a live **packet stream** on the internet. Live raw packets from a real broadcaster still need a DVB receiver in Europe.

---

## 2. Live web feeds

### 2.1 zxnet WebSocket feeds (REQ-INP-022)

Probed for 60 seconds each on the `ch4` and `teefax` feeds, using the message format of the viewer's own `tv.js` (https://zxnet.co.uk/teletext/viewer/, linked from https://zxnet.co.uk/teletext/). [V]

| Feed | Messages in 60 s | Pages listed | Row packets received |
|---|---|---|---|
| `wss://zxnet.co.uk/ws/ch4` | 10 `header`, 191 `row`, 258 `clock`, 58 `bsdp`, 540 `pageExists` | 540 | Packets 1 to 24 and X/27, page 100 only |
| `wss://zxnet.co.uk/ws/teefax` | 23 `header`, 289 `row`, 288 `clock`, 58 `bsdp`, 247 `pageExists` | 247 | Packets 1 to 24 and X/27, page 100 only |

Findings:
- **Raw bytes, as REQ-INP-022 says.** All 18,320 display-row bytes received (7,280 from `ch4`, 11,040 from `teefax`) had odd parity. [V]
- **`bsdp` is a message type REQ-INP-022 does not list.** It carries the 40 bytes of packet 8/30 (bytes 6 to 45) in base64. The viewer handles it at server API version 1 or above (`tv.js` around line 2267). It arrives about once a second. [V]
  - Decoding the Modified Julian Date and UTC fields (EN 300 706 clause 9.8.1: each digit transmitted plus one) gave MJD 61322, 2026-10-09 22:47:44 for Teefax, and MJD 54296, 2007-07-15 15:21:57 for `ch4`. [V]
  - The `ch4` status message reads "Channel 4 Television". [V]
- **The `ch4` service name is `channel4-capture`**, not `ch4`. Its channel identifier decodes to `["channel4-capture","zxnet.co.uk","/ws/ch4"]` (viewer `settings.js`). The short name `ch4` in REQ-INP-022 is only a label; the reader must send the full identifier. [V]
- **No Level 1.5 enhancement packets (X/26) were seen**, but only page 100 was sampled, so this says nothing about the services as a whole. [V for page 100; U otherwise]
- **Extra page-search slots returned no rows.** Requests on slots 1 to 10 with the flags the viewer uses for linked pages returned no data in 60 seconds. [V] Why is unknown. [U]
- **Walking pages on slot 0 returned nothing.** A second run requested a new page on slot 0 every 3 seconds for 120 seconds. It received headers and `bsdp` messages but no rows and no `clock` messages. The same happened on the NMS Ceefax feed. The likely reason is that 3 seconds is shorter than a page's repeat interval in the service cycle: in the first run, page 100's header came round about every 6 seconds on `ch4` and every 3 seconds on Teefax. [U] Probing stopped there, to avoid loading a hobby server. REQ-INP-022 now requires a walking mode to hold each page until its rows arrive.

### 2.2 SRG SSR (Switzerland)

- **Where the data comes from.** The public page https://www.teletext.ch/SRF1/100 loads https://api.teletext.ch/channels/SRF1/pages/100 itself; the address was seen in the page's own requests. [V]
- **What it returns.** JSON with, for each sub-page, `ep1Info.data.ep1Format`. [V] That object has these fields:
  - `fileType` (254);
  - `txtLevel` (1 on SRF 1 page 100);
  - `languageCode` (13);
  - `attributeFlag` and `attributeLenght` (sic);
  - `header`: the 40-byte header, base64;
  - `content`: 1,228 base64 characters, which is 920 bytes, or 23 rows of 40;
  - `commandRow` and `controlCode`.
- **What else comes with it.** Clickable link rectangles, and plain text for searching. [V]
- **Content is raw.** The decoded bytes include control codes and mosaics (for example 0x07, 0x1D, 0x17). [V]
- **The `txtLevel` field marks each page's level.** A page with a higher level may carry enhancement data in the attribute fields. [U: not checked]
- **Channels.** SRF1, SRF2 and SRFinfo, and the RTS and RSI channels, per the Teletext64U source; the API also lists `/channels`. [V for SRF1; U for the others]
- **Format.** This is EP1 inside JSON, so the EP1 reader (area INP) plus a small JSON wrapper could read it.

### 2.3 NOS Teletekst (Netherlands)

- **The main website renders on the server.** https://nos.nl/teletekst loads no teletext data in the browser; its pages are links such as `/teletekst/101`. [V]
- **The `webplus` view is server-rendered HTML too.** https://teletekst-data.nos.nl/webplus?p=101 (address given by the project owner) is a `<pre>` block with colour classes (`yellow`, `cyan`, `bg-blue`). Its script only handles keys and font size. [V]
- **`/page/<n>` on the same host is the raw form,** checked at the project owner's request. https://teletekst-data.nos.nl/page/101 returned `text/plain; charset=ISO-8859-1` with `cache-control: max-age=5`, 1,073 bytes. [V] It contains:
  - navigation lines first: `pn=p_100-1` and `pn=n_102-1` (previous and next), then `ftl=102-0`, `ftl=601-0`, `ftl=730-0` and `ftl=702-0` (the four Fastext links);
  - then `<pre>`, exactly 1,000 bytes (25 rows of 40, row 0 the header and row 24 the Fastext labels), then `</pre>`;
  - teletext codes 0x00 to 0x7F with control codes and mosaics intact, and no parity bit;
  - a live header clock: `NOS 101 00:56:59`, Dutch local time at 22:57 UTC.
- **Accented letters have been converted.** They arrive as ISO 8859-1 bytes: ö as 0xF6 and ë as 0xEB. They are not the transmitted national option codes or Level 1.5 diacritic triplets. [V]
- **Decision E.28 item 5 was wrong for this address.** It said NOS replaces control codes; that is true of the HTML views but not of `/page/`. E.28 is now amended.

### 2.3a NMS Ceefax (https://nmsceefax.co.uk)

- **It uses the same viewer code as zxnet**: `teletext-viewer/tv.js`, with the channel identifier in the address fragment. [V]
- **Its feed is `wss://internal.nathanmediaservices.co.uk/websockets/ceefax`.** A 30-second probe received 53 headers, 29 `bsdp` messages, 25 clocks and 224 pages listed. [V]
  - It received no rows, because the probe was changing page every 3 seconds (see section 2.1).
  - Its 8/30 clock was live (22:59 UTC).
- **"Pages From Ceefax" (`/live/`) is a Twitch video stream**, so it holds no data. [V]
- **Its pages are also published as TTI files** under `feeds.nmsni.co.uk/svn/ceefax/` (per the Teletext64U source). These are authored pages for a recreation, not broadcasts.

### 2.3b RAI Televideo (Italy)

- **Pages are PNG images.** https://www.servizitelevideo.rai.it/televideo/pub/index.jsp and `pagina.jsp?p=101` show each page as a PNG image (`tt4web/Nazionale/16_9_page-101.png`), with an HTML image map for the links. No text or byte form is loaded. [V]
- **The site loads an Akamai bot-defence script** (`/akam/13/...`). [V]
- **Wikipedia adds little.** Its article (https://en.wikipedia.org/wiki/Televideo_(teletext)) is an unsourced stub: Televideo launched in 1984 and has about 900 pages, with 21 regional editions on Rai 3. [V]
- **`p2js/televideo-web`** (https://github.com/p2js/televideo-web) is a nicer client for the same RAI images. Its README says "the app gets the resources from RAI's own online televideo browser". There is no other data source. [V]
- **The only raw RAI teletext found** is in broadcast recordings (section 3.1: TSDuck `mux1rai.ts`, `mux4rai.ts`).

### 2.3c German web-teletext archives (defgsus)

- **What it is.** `defgsus/teletext-viewer` (https://github.com/defgsus/teletext-viewer) browses archives that GitHub Actions take about three times a day from German broadcasters' websites:
  - `defgsus/teletext-archive-unicode` (2022);
  - `defgsus/teletext-archive-2023`, switched off and replaced by `teletext-archive-2024`. [V]
- **Stations, from January 2022:** 3sat, ARD, NDR, n-tv (which publishes JSON), SR, WDR, ZDF, ZDFinfo and ZDFneo. [V]
- **Format.** Newline-delimited JSON under `docs/snapshots`. Each row is a list of `[colour-code, text]` runs, with mosaics as Unicode sextants. These are converted HTML, not teletext bytes; levels above 1 and hold-mosaic behaviour are lost. [V]
- **Use.** It is the only history of ZDF pages found. It could serve as test content for the text renderer, but not as decoder input. [U]

### 2.4 ZDF (Germany)

- **HTML only.** ZDF's web teletext (https://teletext.zdf.de) is built on the server as plain HTML. There is a "klassisch" (classic) layout. [V]
- **A false lead.** The `data.blob` file the page loads is an icon file (the ICO header 00 00 01 00, 894 bytes), not data. [V]
- **ZDF blocks automated access.** It has added a JavaScript bot check. Teletext64U gets past it in code. That is circumvention, and it is not used here. [V: from the Teletext64U source comments]
- **The broadcast is the only source of raw ZDF data:** Astra 19.2°E or German DVB-T2. No public recording carrying ZDF teletext was found.
  - The TSDuck repository has no German capture. [V]
  - The FFmpeg samples in REQ-TST-022 are Italian, French and English.
- **Why it matters.** German public broadcasters are the main remaining users of Level 2.5, so newer-level test data is most likely there. [U]

### 2.5 Other services read by Teletext64U (from its source)

| Kind | Services and addresses |
|---|---|
| Raw page bytes | NOS (`teletekst-data.nos.nl/page/%s`), SRG (`api.teletext.ch`), Teletekstarchief (`teletekstarchief.nl/archive-json/{page}`: 960 bytes, 40×24, Level 1, base64, original header row kept) |
| TTI page files | NMS Ceefax (`feeds.nmsni.co.uk/svn/ceefax/Worldwide/P%s.tti`), SPARK (`raw.githubusercontent.com/spark-teletext/spark-teletext/master/P%s.tti`), Webfax (`github.com/Webfax-Teletext/...`), BMN Bollentekst and HBN (`feeds.bmn-online.nl/svn/teletext/...`), Teefax (`zxnet.co.uk/git/cf.git`) |
| HTML, scraped back into teletext | ZDF, ARD (`ard-text.de/page_only.php`), ORF (`text.orf.at`), SVT (`texttv.nu`), WDR, NDR, hr and SWR (the `ttxweb` engine), DR, MTVA |
| API with a key | YLE (`external.api.yle.fi/v1/teletext/pages/%s.xml`) |

The HTML services lose information in the round trip and are not raw data. The TTI feeds are authored pages, not broadcasts.

---

## 3. Recorded broadcasts

### 3.1 TSDuck stream repository (https://tsduck.io/streams/)

These are real off-air captures. Only the small published `*.analyze.txt` files were downloaded. A full service is identified by the descriptor "Initial Teletext page" and a high bit rate; "Teletext subtitles" means subtitles only. [V]

| Capture | Date | Full teletext services (bit rate) |
|---|---|---|
| `poland-dttv-warsaw/PL_DVB-T_2023-11-10_MUX3_522MHz.ts` | 2023-11-10 | TVP1 and three other TVP services (150 to 226 kbit/s) |
| `netherlands-dttv/digitenne-482-2020-06-21.ts` | 2020-06-21 | NPO 1, 2, 3 and two more (about 300 kbit/s each) |
| `netherlands-dttv/digitenne-594-2020-06-21.ts` | 2020-06-21 | RTL4 and others (40 to 75 kbit/s) |
| `italy-sardinia-dttv/mux1rai.ts` | not given | RAI 1 to 3 (about 300 kbit/s), plus one at 113 kbit/s |
| `italy-sardinia-dttv/mux4rai.ts` | not given | Three RAI services (about 113 kbit/s) |
| `italy-sardinia-dttv/mux3mediaset.ts` | not given | Mediaset (263 kbit/s) |
| `hotbird-13.0E/hotbird130E-ts6000-2018-05-11.ts` | 2018-05-11 | Mediaset (263 kbit/s) |
| `hotbird-13.0E/hotbird130E-ts9400-2018-05-11.ts` | 2018-05-11 | Euronews (113 kbit/s) |

- **Subtitles only:**
  - every Australian network capture (ABC, SBS, Seven, Nine, Ten);
  - France 3 on Astra (2018);
  - some Polish and Dutch multiplexes.
- **Teletext declared but empty** (0 bit/s): Telesardegna and TCS.
- **Sizes.** The files are 35 to 110 MB. REQ-TST-022 already allows tests to fetch only the first few megabytes with ranged requests.
- **Licence.** Stated on https://tsduck.io/tsduck-license/; not yet read. [U]

### 3.2 teletext-restoration (https://github.com/teletext-restoration/teletext-restoration)

- **Contents.** UK recoveries from tape, 1978 to 2012: BBC, ITV regions, Channel 4, Channel 5, Sky and S4C. [V]
- **Restored pages in git.** Pages manually restored to correct recovery errors are stored in the repository itself as `.t42` files, named `restored/Pmpp-ssss-hhmm-hhmm.t42`. [V]
- **Checksums.** A file suffixed `-maybe` does not exactly match the page checksum broadcast in the recovery; `-nocrc` means no usable checksum was broadcast. Any other restored page therefore matches its broadcast X/27/0 checksum. That makes these pages the right test data for REQ-ECC-007. [V for the naming; U that every unsuffixed page has a checksum]
- **Full recordings** are 7-Zip archives at `https://temp.zxnet.co.uk/teletext-recoveries/<name>.7z`. The maintainers ask users to download only what they need, and to email archive@zxnet.co.uk about bulk use. [V]

### 3.3 Other archives found (not examined)

- **Nigel's Teletext Archive** (https://teletext.nigelreed.net/archives.html): takes and shares T42 files. Its list links the Teletext Archaeologist archive (over 550,000 pages), Alistair Buxton's recoveries and TV Whirl.
- **Computer-Legacy** (https://computer-legacy.com/teletext.html): T42 recoveries.
- **The Teletext Archive** (https://www.teletextarchive.com/): the readable form of the Archaeologist archive.
- **Teletext Rescue** (https://github.com/AssunaYuuki/teletext_rescue): recovers teletext from raw VBI captures, possibly a source of raw sample data for the slicer.
- **CCExtractor TV samples** (https://ccextractor.org/public/general/tvsamples/): includes Russian teletext samples and a mixed Spanish multiplex.
- **TtxFromTS** (https://github.com/orryverducci/TtxFromTS): converts transport streams to TTI or T42, a possible cross-check for our transport stream reader.

---

## 4. Hello-world build across REQ-GEN-015

- **Where.** The probe workspace is `tmp/hello-targets/` (not committed). Builds ran on `oldvga` (Ubuntu 26.04, x86_64) with Rust 1.99.0 and Espressif's toolchain 1.93.0.0 installed by `espup` 0.17.1. [V]
- **What was built.**
  - `hello-std`: a `println!` binary.
  - `hello-bare`: a `no_std` static library with its own panic handler.

| Result | Targets |
|---|---|
| Built, with binary or library produced | thumbv6m, thumbv7em, thumbv8m.main, riscv32imc, riscv32imac, riscv32imafc, aarch64-linux, armv7-linux, riscv64-linux, x86_64-linux, xtensa-esp32, xtensa-esp32s3, wasm32-unknown-unknown, wasm32-wasip1 |
| Compiled only (`cargo check`): no linker or SDK on Linux | aarch64-apple-darwin, x86_64-apple-darwin, x86_64-pc-windows-msvc, xtensa-esp32-espidf, xtensa-esp32s3-espidf |
| Built and run natively on the Mac | aarch64-apple-darwin |

All 19 targets compile. [V] Linking the Apple targets needs a Mac, Windows needs a Windows host or cargo-xwin, and ESP-IDF needs Espressif's ESP-IDF SDK.

---

## 5. Candidate dependencies on microcontrollers (decision E.38)

- **Method.** Each crate was built alone as `no_std` without allocation, with default features off, for `thumbv6m-none-eabi`, `riscv32imc-unknown-none-elf` and `xtensa-esp32-none-elf`. The probe is in `tmp/dep-probe/`.
- **Result.** All eight built on all three targets: [V]
  - `heapless` 0.9.3;
  - `bitflags` 2.13.2;
  - `httparse` 1.10.1;
  - `embedded-io` 0.7.1;
  - `base64` 0.23.1;
  - `sha1` 0.11.0;
  - `serde-json-core` 0.7.0, with `serde`;
  - `embedded-websocket` 0.9.5.
- **The panic and floating-point check failed as an instrument.** It counted undefined symbols in each unlinked library's own object files. Every crate showed 20 floating-point references, including `bitflags`, which generates no code, so the count came from shared objects and measured nothing. [V] The reliable form is the linked-image symbol check now required by REQ-GEN-019.

---

## 6. Consequences for REQUIREMENTS.md (applied on 10 October 2026)

1. **Decision E.28 is partly out of date.**
   - Item 5 says NOS replaces control codes. Teletext64U says NOS serves raw bytes (unchecked, section 2.3).
   - SRG serves raw EP1 pages live (section 2.2). This contradicts "no television channel publishes its live teletext on the open internet as raw data", at least for whole pages.
2. **REQ-INP-022 needs three additions:**
   - the `bsdp` message (packet 8/30 bytes 6 to 45, base64);
   - the `ch4` service name `channel4-capture`;
   - the observation that extra slots and rapid slot-0 page changes returned no rows.
3. **REQ-TST-022 could add:**
   - TVP Poland 2023 and NPO Netherlands 2020 from TSDuck, as the newest broadcast packets;
   - the restored pages of teletext-restoration, for the checksum test of REQ-ECC-007;
   - possibly a recorded SRG EP1 session.
4. **A possible new input.** The SRG and NOS page feeds are candidate live web sources of real broadcaster pages. Whether to support them is recorded as an open question in Appendix D.2, together with where German broadcast data can be obtained.
