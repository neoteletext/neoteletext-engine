# F.15 Command-line program (CLI)

**REQ-CLI-001 — Program and commands.**
A command-line program named `neottx` shall provide these commands:

| Command | Purpose |
|---|---|
| `neottx list` | List the pages and sub-pages found in an input, with counts and error statistics. |
| `neottx show` | Display one page in the terminal, as exact pixels where the terminal supports a graphics protocol and as characters otherwise (decision E.37). |
| `neottx render` | Write one page, or all pages, as image files. |
| `neottx text` | Write one page, or all pages, as UTF-8 text. |
| `neottx convert` | Convert between the input and output formats of area INP, for example from a transport stream to T42, or from TTI to T42. |
| `neottx slice` | Convert raw sampled video to T42 with quality statistics. |
| `neottx services` | List the teletext streams, languages and subtitle pages found in a transport stream. |
| `neottx service-info` | Print the service data from packet 8/30: initial page, network identification, date, time and status message. |
| `neottx check` | Check page files for authoring problems (REQ-CLI-006). |
| `neottx contents` | Write a channel's `contents.txt` from a folder of page files (REQ-CLI-008). |
| `neottx font convert` | Write a font in the 12 × 10 form from a BDF of another size, where the fit is exact (REQ-FNT-010). |
| `neottx discover` | Report the teletext edition a web address points to, and every candidate ignored (REQ-CLI-009). |
*Verification:* Demonstration; test of each command.

**REQ-CLI-002 — Common options.**
Every command shall accept:
- an input file or standard input;
- a live WebSocket feed (REQ-INP-022), given as `zxnet:<short name>` (for example `zxnet:teefax`), as a channel identifier, or as a `wss://` address;
- an input format option, which overrides detection;
- a profile option;
- options for the settings of Part E;
- a verbosity option that can print a trace of every packet with its decoded fields and error status, and the state machine transition trace (REQ-FSM-010).
*Verification:* Test.

**REQ-CLI-003 — Page selection.**
Pages shall be selected by magazine and page number, with an optional sub-code: for example `100`, `100:0003` or `8FF` (the last for diagnostic use). "All" shall be accepted where it makes sense.
*Verification:* Test.

**REQ-CLI-004 — Interactive viewer.**
*Phase:* Phase 2 (whole requirement).
The program should provide an interactive terminal viewer. It should read a live or file input and accept keys for these functions:
- page number entry;
- next and previous page;
- the four Fastext keys, mapped to function keys F1 to F4 and to the letters r, g, y and c;
- Index;
- reveal;
- hold;
- mix;
- zoom;
- sub-page stepping;
- quit.
*Verification:* Demonstration.

**REQ-CLI-005 — Exit status.**
The program shall exit with:
- status 0 on success;
- status 1 when a requested page was not found;
- status 2 on invalid usage;
- status 3 when the input could not be read or recognised.
*Verification:* Test.

**REQ-CLI-006 — The page checker.**
`neottx check` shall read TTI page files and report problems that would make a page fail on some receivers, or behave unsafely. Each problem is reported as an error or a warning, with its file, page and line. It shall report:

*Errors:*
- **Too many links.** More than four coloured links, a repeated key, or two `LK` lines for the same page number.
- **Bad link targets.** Targets naming services missing from a given descriptor, and `LK` or `LT` lines whose target, page number or position is invalid (Appendix E.1).
- **Bad log lines.** An `LG` line the reader refuses, or more than a page may carry (REQ-INP-026).
- **Bad labels.** Labels too long for the prompt, or using characters the page's G0 set cannot show.
- **Bad form fields.** Overlapping fields, fields outside the row, or version 2.0 form syntax.
- **Withdrawn commands.** Use of the withdrawn `FH` command, or of the withdrawn `TF` line (decision E.111).

*Warnings:*
- **Page number not shown.** A link to a page in the same service whose page number does not appear in the page's text (Appendix E.2 rule 5), or an `LK` line that links a page number the page's text does not show (Appendix E.2 rule 7, decision E.64).
- **No other route.** A link to another service with no other route given in the text (Appendix E.2 rule 6).
- **No broadcast fallback.** A form page with no fallback instruction for broadcast viewers (Appendix C.6).
- **Unreliable placeholders.** Form placeholders using national option characters (Appendix C.4).
- **Hidden content.** Content in row 24 of a page with links, which neoteletext will not display.

A warning may be marked as considered with a line `CK,considered,<warning name>` in the page, so that it is not reported again. Other TTI readers ignore the line as an unknown command.

Exit status is 0 when there are no errors, and 4 when there are errors. Warnings alone do not fail.
*Verification:* Test with a set of pages, each containing one problem.

**REQ-CLI-007 — The presentation level option.**
Every command that decodes pages shall accept `--level` followed by `1`, `1.5`, `2.5` or `3.5`, which sets the presentation level of REQ-PRF-006 and overrides the level of the chosen profile. Any other value shall be invalid usage (REQ-CLI-005). The former `--level1-only` option shall be removed; `--level 1` replaces it (decision E.40).
*Verification:* Test of each value, of an invalid value, and that `--level1-only` is rejected.

**REQ-CLI-008 — Writing a channel's contents.**
`neottx contents FOLDER` shall write a `contents.txt` for a channel from the author's own local folder of page files (decision E.70 item 9), to standard output. It shall:
- **read only that folder,** not its subfolders, and only these files in it: `NNN.tti` and `NNN.t42` whose page numbers are decimal, and `index.t42`. It shall use no network and write no file. It reads and decodes those files first, then applies the sandbox of REQ-SEC-017, with no file and no network access, before it takes any title from them, as the other commands do;
- **list every page found,** in page number order, as one `p` entry each. A page in both a page file and `index.t42` is listed once;
- **title each page** with its TTI `DE` line when it has one, otherwise with the first row of rows 1 to 23 that is not blank, converted to printable ASCII and trimmed, otherwise with "Page NNN". Titles longer than 32 characters are cut;
- **date each page** with the modification date, in UTC, of the file it was found in;
- **start each magazine with a section heading,** titled with the title of the magazine's page `M00` when that page exists, and "Magazine M" otherwise;
- **stay within REQ-WEB-025.** Beyond 800 pages, it shall stop listing and say so on standard error. A file that cannot be read, or fails the check for its format (REQ-WEB-027), is skipped with a diagnostic on standard error.

The output never includes `x` entries. An author who lists other channels adds those lines by hand after the generated ones. Exit status is 0 on success, the usage status for a wrong argument, and 4 when any file was skipped or the page limit was reached.
*Verification:* Tests with a folder holding TTI and T42 page files, an `index.t42`, a subfolder, a file with another name, an invalid file, and pages in several magazines; the output shall be exactly the expected file, and shall pass REQ-WEB-025.

**REQ-CLI-009 — Testing a web page's pointer.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**REQ-CLI-010 — Writing a page from text.**
*Phase:* Phase 2 (whole requirement).
`neottx write-text --set SET --page MPP INPUT OUTPUT` shall turn a UTF-8 text file into a TTI page in the named set of REQ-CHR-012, using the writer of `neoteletext-app` (decision E.82). It shall:
- **Refuse what the set cannot write,** naming the character and its line; it never drops or substitutes a character silently. It applies only the substitutions the set documents, such as Thai digits to Western digits.
- **Designate the page:** the header's C12 to C14 and an X/28/0 Format 1 packet carrying the whole designation.
- **Break lines at the row width,** counting cells. A Latin run in a page of another script is switched with ESC, and an ESC takes a cell.
- **Arabic script:** write لا as the lam-alef code, and reorder each broken line into visual order with the Unicode Bidirectional Algorithm, keeping runs of digits and Latin text left to right.
- **Vietnamese, Yoruba and Igbo:** write each letter without its tone in the row, and its tone as an X/26 diacritical-mark triplet. Refuse the page when its triplets would exceed X/26's sixteen packets.
- **Thai and Lao:**
  - lay each line out on two rows, the marks above on the first and the letters on the second;
  - put each mark in its consonant's column, and lower vowel signs on the row below, shared with the next line's marks;
  - add a third row only where a column needs both;
  - write G2 combinations as X/26 triplets with their G0 stand-ins, and ำ as ํ and า.
- **Report** the rows used and the X/26 triplets written.

It never reads from the network and runs under the sandbox of REQ-SEC-017.
*Verification:* Tests for each set that a sample text written and read back gives the same Unicode, apart from documented substitutions; tests of each refusal, of the Thai layout with and without a shared row, of Arabic-script reordering with digits and Latin, and of the X/26 budget.

**REQ-CLI-011 — Writing a sound.**
*Phase:* Phase 2 (whole requirement).
`neottx sound --page MPP [--channel N] [--gain PERCENT] [--codec ima|pcm] [--handle N] [--loop] [--repeat N] [--live [--pace PACKETS]] INPUT.wav OUTPUT.t42` shall write a sound as the audio profile's broadcast packets (REQ-AUD-007): its OBJECT record, its DATA records and a CONTROL record that plays it on the page, on audio service 0. It shall:
- **Read** a RIFF WAVE file of 16-bit linear PCM, mono or stereo, at one of the rates of REQ-AUD-002, of at most 32 MB; stereo is mixed down to mono as (left + right) / 2. It refuses any other file, naming why.
- **Encode** in IMA ADPCM with 512-byte blocks by default, or in PCM with 1 000-byte blocks.
- **Write** the packets as a T42 stream, the whole sequence `--repeat` times, as a carousel would repeat it, to be added to a channel's T42 stream.
- **Send live** with `--live` (REQ-AUD-014): the OBJECT record of a live sound and the play, then each block, IMA ADPCM in 256-byte blocks, one a record, at the packet where a player reading `--pace` packets a second (by default 320, NeoTeletext's pace) reaches the moment the block's last sample was captured. The packets between repeat the announcement and the latest block. The sound is sent `--repeat` times in a row; a rate the pace cannot carry is refused.
- **Report** the samples, the rate, the bytes of sound and the packets written.

It never reads from the network and runs under the sandbox of REQ-SEC-017. A build without the feature `sound` refuses the command (decision E.61).
*Verification:* Tests that PCM written and read back through the receiver is exact and plays on the page and channel asked for, that IMA ADPCM reads back with a signal-to-noise ratio above 25 dB, that a rate outside the profile is refused, that a live sound plays back as the player reaches it and fades two seconds after the last block, that a live sound too fast for the pace is refused, and that a build without `sound` refuses the command.

**REQ-CLI-012 — A QR code as a page.**
*Phase:* Phase 2.
`neottx qr <text> --page <mpp> [--row <r>] [--column <c>] <output>` shall write a TTI page whose rows, from row `r` (2 unless given), show the QR code of `text` in contiguous mosaics, one sixel a module, dark on white with its quiet zone, and shall refuse a code that does not fit rows 1 to 23 and 40 columns.
*Verification:* A test reads the page back with the engine's own reader and refuses a code placed too low; the rendered page was decoded by a QR reader on 20 October 2026 (decision E.94).

**REQ-CLI-013 — A phone as a channel's keyboard.**
*Phase:* Phase 2.
`neottx keyboard <ws address> [--relay <address>] [--pin] [--name <name>] [--qr-page <mpp> --qr-output <file>]` shall create a qrkeyboard instance (REQ-INT-007), show its QR in the terminal with the one line of warning ("Anyone who can see this code can type"), optionally write it as a page (REQ-CLI-012), connect to the channel's push WebSocket, and send each key of the paired phone as a key record (REQ-INT-001), down and up, its text in records of at most 32 bytes, with no modifier.
*Verification:* The records it sends are written by the engine's key-record writer, which refuses anything out of range; pairing is REQ-INT-007's.

*Amended 20 October 2026 (decision E.94):* **REQ-CLI-004's viewer types back.** On a channel whose descriptor declares input, while its WebSocket is open, `neottx view` sends every key to the channel as a key record, down then up (a terminal reports a key once), Control-C included, and tells the channel which page is open; Control-] leaves the keys to the viewer, and again gives them back, as telnet's escape character does.
