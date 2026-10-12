# neoteletext — Software Teletext Receiver — Requirements Specification

**Status:** Draft 2 (consolidated) for review
**Date:** 1 October 2026
**Implementation language:** Rust, edition 2024 (REQ-GEN-001), for ARM, RISC-V, Xtensa (ESP32) and AMD64 targets (REQ-GEN-015)
**Presentation levels in scope:** Levels 1.0, 1.5, 2.5 and 3.5 of the World System Teletext (System B) as defined by ETSI EN 300 706 (Levels 2.5 and 3.5 added on 10 October 2026, decision E.40)
**Baseline reference hardware:** Philips SAA5050 (rendering) and Philips SAA5246A (receiver behaviour)

---

## How to read this document

This document has seven parts:

- **Part A** is a thorough walkthrough of Presentation Level 1.0 as defined by ETSI EN 300 706 V1.2.1. It covers the transmission layer, the page layer and the display layer.
- **Part B** is a thorough walkthrough of Presentation Level 1.5: what it adds to Level 1.0 and exactly how it does so.
- **Part C** walks through the Philips SAA5050 Teletext Character Generator, the baseline for how a Level 1 page looks.
- **Part D** walks through the Philips SAA5246A Integrated Video Input Processor and Teletext decoder (IVT1.0), the baseline for how a receiver behaves.
- **Part E** is a decision register. For each place where the standard is silent, where the standard contradicts itself, or where the reference chips differ from the standard, it records the decision this implementation will follow and why.
- **Part F** holds the numbered requirements. **Security (area SEC, section F.0.3) overrides every other requirement** (decision E.32).
  Everything that is to be implemented appears there as a separately numbered requirement. Section F.0.1 divides the requirements into two delivery phases: Phase 1 is a complete, correct receiver, and Phase 2 holds the optional features and nice-to-haves.
- **Part G** lists what is deliberately out of scope.
- **Appendix A** is a glossary, and **Appendix B** lists the source documents.
- The companion document `ARCHITECTURE.md` describes how the code is layered. REQ-ARCH-011 makes its rules mandatory.
- **Appendix C** is the TTI Form Extension, version 3, which is normative.
- **Appendix E** is the TTI Link Extension, which is normative.
- **Appendix D** records the open questions: which were answered by research, with the evidence, and which remain open.

Parts A to E are explanatory and not normative. Only Part F is normative, and a requirement in Part F wins over anything in Parts A to E.

### Requirement wording

Each requirement has an identifier of the form `REQ-<AREA>-<NUMBER>`, for example `REQ-L1-012`. The area codes are listed at the start of Part F. Requirement identifiers are never reused. A requirement that is withdrawn keeps its number and is marked as withdrawn. Numbers within an area are not always consecutive; gaps are deliberate and leave room for related requirements to be added later.

- **shall** marks a mandatory requirement.
- **should** marks a recommendation. It is expected to be implemented unless there is a documented reason not to.
- **may** marks a permitted option.

Each requirement carries a **verification method**, which is how compliance will be demonstrated. The methods are:

- **Test**: an automated test in the project's test suite.
- **Inspection**: a review of the source code or the documentation.
- **Demonstration**: running the program and observing the result.

### Citation conventions

**Where the sources are.** The standards and datasheets cited in this document are not kept in the project directory (`~/devel/neoteletext`), because they must not be redistributed (see "Legal basis" below). They are in `~/devel/microcontrollers/teletext/`. Every folder path in this document, such as `specifications/`, `philips-saa5050/` or `DATASHEET-INDEX.md`, is relative to that directory.

The research notes behind Parts A to E are in `research/` in the project directory, with a citation for each fact.

Citations take the form "EN 300 706 clause 9.3.1" or "EN 300 706 Table 26". Printed page numbers of EN 300 706 V1.2.1 match the page numbers of the PDF file in `specifications/`.

- Citations of the SAA5050 refer either to the Signetics datasheet in `philips-saa5050/` or to the Philips 1979 databook in `databooks/`. For the databook, the number given is the PDF page number.
- Citations of the SAA5246A refer to the Philips product specification of January 1993 in `philips-saa5246a/`.

Character codes are written as column and row, as the standard does. "1/E" means column 1, row 14, which is hexadecimal 0x1E. Hexadecimal is also used where it is clearer.

### Terminology

The standard names things with abbreviations, such as the Magazine and Row Address Group and the Fastext linking scheme called "Full Level One Features". This document spells each term out in full at its first use in each part. The Glossary in Appendix A lists every abbreviation that still appears.

### Legal basis of the implementation

The implementation will be written from the published standards (EN 300 706, EN 300 472 and ITU-R BT.653) and from behaviour described in published datasheets. The following restrictions apply:

- No manufacturer's code will be copied or disassembled. This includes the mask ROM contents of any microcontroller.
- Character glyph bitmaps will not be copied from any character generator ROM. The glyphs will be drawn originally or taken from a public-domain recreation (see REQ-RND-020).
- No code will be copied from GPL-licensed teletext decoders. Such decoders may be used as external test oracles (see Part F, area TST).
- The standards and datasheets themselves will not be redistributed in the project repository.

---

# Part A — Walkthrough of Presentation Level 1.0 (EN 300 706)

## A.1 What a teletext signal is

Teletext is a digital data service carried inside an analogue 625-line, 50-field television signal. It uses lines in the vertical blanking interval, the part of each field that is not displayed.

- EN 300 706 clause 4.1 allows teletext on lines 6 to 22 of the first field and lines 318 to 335 of the second field. That is 17 lines plus 18 lines, or 35 lines per frame.
- A service may also run in "full field" mode, in which almost every line of the picture carries teletext and there is no picture (clause 4.2).
- Each television line that carries teletext carries exactly one packet (clause 6).

The data is binary non-return-to-zero. A "0" is transmitted at black level, within plus or minus 2 percent. A "1" is transmitted at 66 percent, plus or minus 6 percent, of the difference between black level and peak white (clause 5.2).

- **Bit rate.** The bit rate is 444 times the nominal line frequency, which is 6.9375 megabits per second, with a tolerance of plus or minus 25 parts per million (clause 5.3). One bit therefore lasts about 144.144 nanoseconds, and exactly 444 bits fit into one 64-microsecond line period.
- **Spectrum.** The spectrum is shaped to be skew-symmetrical about half the bit rate and is substantially zero by 5 megahertz (clause 5.4).
- **Timing.** The start of the data is referenced to the line synchronising pulse. The midpoint of the second-to-last "1" of the clock run-in lies 12.0 microseconds after the half-amplitude point of the leading edge of the line sync pulse. The tolerance is plus 0.4 microseconds and minus 1.0 microseconds (clause 6.3 and Figure 4).

## A.2 The structure of a packet on the line

A packet is 360 bits long, organised as 45 bytes numbered 1 to 45. Bits within a byte are numbered 1 (least significant) to 8 (most significant). Unless the standard says otherwise, **bytes are transmitted least significant bit first** (clauses 7.1 and 8).

| Bytes | Content |
|---|---|
| 1 and 2 | **Clock run-in.** In transmission order the pattern is 1010101010101010 (clause 6.1). Read least significant bit first as bytes, it is 0x55 0x55. It exists so that the receiver can lock its bit clock. The standard warns that the run-in does not by itself identify a teletext line, and that its first two "1" bits may be missing or reduced in amplitude. |
| 3 | **Framing code.** In transmission order the pattern is 11100100 (clause 6.2), which read least significant bit first is **0x27**. This is what identifies a teletext line and establishes byte alignment. After it has been found, the remaining bits are grouped into 8-bit bytes. |
| 4 and 5 | **Magazine and Row Address Group.** Both bytes are protected by Hamming 8/4 coding (clause 7.1.2). |
| 6 to 45 | **40 bytes of payload.** Their coding depends on the packet type. |

A receiver that works from already-extracted packets, such as a file in the common "T42" format, sees only bytes 4 to 45: 42 bytes per packet.

## A.3 The error-protection codes

Three codes are used at Levels 1.0 and 1.5 (clause 8).

**Odd parity (clause 8.1).** Displayable characters are 7 data bits plus a parity bit in bit 8, chosen so that the total number of "1" bits in the byte is odd. The receiver accepts the character if the eight bits contain an odd number of ones. A single-bit error is detected but cannot be corrected. The standard does not say what a receiver must display when a character fails its parity check; that is a receiver design choice (see decision E.8).

**Hamming 8/4 (clause 8.2).** This code protects addresses and control information. The byte carries four data bits in bit positions 2, 4, 6 and 8, and four protection bits in positions 1, 3, 5 and 7. In transmission order the bits are P1, D1, P2, D2, P3, D3, P4, D4.

The encoding equations are:
- P1 = 1 ⊕ D1 ⊕ D3 ⊕ D4
- P2 = 1 ⊕ D1 ⊕ D2 ⊕ D4
- P3 = 1 ⊕ D1 ⊕ D2 ⊕ D3
- P4 = 1 ⊕ P1 ⊕ D1 ⊕ P2 ⊕ D2 ⊕ P3 ⊕ D3 ⊕ D4

The decoder runs four odd-parity tests:
- Test A covers P1, D1, D3 and D4.
- Test B covers D1, P2, D2 and D4.
- Test C covers D1, D2, P3 and D3.
- Test D covers all eight bits.

The results are interpreted as follows:
- If tests A, B and C all pass and test D passes, there is no error.
- If tests A, B and C all pass but test D fails, the error is in P4, and the data is accepted.
- If any of tests A, B and C fails but test D passes, there is a double error, and the data must be rejected.
- If any of tests A, B and C fails and test D also fails, there is a single error. It can be located and corrected.

Data bit D1 carries the weight 1 and D4 carries the weight 8. The standard does not list the sixteen valid code bytes, but they follow from the equations. For data values 0 to 15 in order, they are hexadecimal 15, 02, 49, 5E, 64, 73, 38, 2F, D0, C7, 8C, 9B, A1, B6, FD and EA.

**Hamming 24/18 (clause 8.3)** protects groups of three bytes called triplets. It is needed from Level 1.5 upward and is described in Part B.

## A.4 Magazines, rows and the packet address

Teletext pages are grouped into eight **magazines**, numbered 1 to 8. A magazine value of 0 in the packet address means magazine 8 (clause 3.1). Each packet carries a 3-bit magazine number and a 5-bit **packet number** (also called the row number), in the two Hamming-protected bytes 4 and 5:

- **Byte 4** carries the magazine in its three low data bits, with weights 1, 2 and 4.
- **Byte 4** also carries the least significant bit of the packet number in its highest data bit.
- **Byte 5** carries the remaining four bits of the packet number.

If the decoded 4-bit value of byte 4 is called n4 and that of byte 5 is called n5, then:
- the magazine is n4 AND 7;
- the packet number is (n4 shifted right by 3) OR (n5 shifted left by 1).

The packet number says what the packet is (EN 300 706 Table 1):

| Packet number | Name and purpose | Level 1.0 relevance |
|---|---|---|
| 0 | **Page header.** Identifies a page and terminates the previous page in the magazine. Carries 32 displayable characters that form row 0. | Essential |
| 1 to 23 | **Display rows** 1 to 23 of the page, 40 characters each. | Essential |
| 24 | **Row 24.** Carries the prompts for the four coloured navigation keys under the Fastext scheme ("Full Level One Features"). | Displayed only when enabled (see A.9) |
| 25 | Displayable labels for key-word search. | Not displayed |
| 26 | Page enhancement data. At Level 1.5 this carries replacement characters. It also carries recording-control data (Programme Delivery Control). | Must be tolerated and skipped at Level 1.0 |
| 27 | **Editorial links.** Designation code 0 carries the six Fastext links. | Used for navigation |
| 28 | Page-related enhancement data: page function, character set designation. | Optional |
| 29 | Magazine-related enhancement data. | Optional |
| 30 | **Broadcast service data** (magazine 8 only): the initial page, network identification, date and time, and a status message. | Used for service information |
| 31 | Independent data services. | Ignored |

Packets 30 and 31 are not related to any page. They can appear at any point in the transmission and must never end, or attach to, a page (clauses 7.4 and 9.7). Packet 29 applies to a whole magazine, not to a page (clause 7.3).

## A.5 The page header (packet X/0)

The page header is the most important packet, because it identifies a page, tells the receiver how to treat it, and ends the reception of the previous page. Its byte layout is (clause 9.3.1 and Figure 9):

| Byte | Coding | Content |
|---|---|---|
| 4 to 5 | Hamming 8/4 | Magazine and packet number (packet number 0) |
| 6 | Hamming 8/4 | Page number units digit (0 to F) |
| 7 | Hamming 8/4 | Page number tens digit (0 to F) |
| 8 | Hamming 8/4 | Sub-code digit S1 (4 bits) |
| 9 | Hamming 8/4 | Sub-code digit S2 (3 bits) and control bit C4 |
| 10 | Hamming 8/4 | Sub-code digit S3 (4 bits) |
| 11 | Hamming 8/4 | Sub-code digit S4 (2 bits) and control bits C5 and C6 |
| 12 | Hamming 8/4 | Control bits C7, C8, C9 and C10 |
| 13 | Hamming 8/4 | Control bits C11, C12, C13 and C14 |
| 14 to 45 | Odd parity | 32 displayable characters or spacing attributes |

**Page number.** The full page address is written M Pt Pu : S4 S3 S2 S1, where M is the magazine, Pt the tens digit and Pu the units digit. So "page 100" is magazine 1, tens 0, units 0.

- Both page digits may take hexadecimal values. The receiver must process headers for all page numbers, including hexadecimal ones, because they terminate pages even though viewers are not meant to select them (Table 1).
- Page number FF is special. Headers with page number FF are "time filling headers". They carry no page. They keep the clock updated and terminate the previous page when a magazine has only one displayable page (clause 7.3 and Annex A.1).
- The address XFF:3F7F is the null page address and is never transmitted as a header.

**Sub-code.** The sub-code is 13 bits long with a maximum value of 3F7F. It distinguishes sub-pages of a rotating page (Annex A.1):
- A page without sub-pages should be coded 0000.
- Rotating sub-pages are numbered 0001 to 0079, using S2 and S1 as decimal digits.
- The sub-code digits can also be used as an hours-and-minutes time code for "timed pages", such as an alarm-clock page (Annex E.2).

**Control bits** (Table 2). Each bit is active when set to 1:

- **C4, Erase Page.** Rows belonging to an earlier transmission of this page must be erased before the new rows are stored.
- **C5, Newsflash.** The page is a newsflash. Its content is boxed and inset into the television picture.
- **C6, Subtitle.** The page is a subtitle page, boxed and inset into the picture in the same way.
- **C7, Suppress Header.** Row 0 must not be displayed.
- **C8, Update Indicator.** The page content has changed since its last transmission. This is under editorial control and may prompt the receiver to redisplay the page.
- **C9, Interrupted Sequence.** The page is out of numerical sequence, so its header should be left out of the rolling header display.
- **C10, Inhibit Display.** Rows 1 to 24 must not be displayed.
- **C11, Magazine Serial.** When 1, the service is in serial mode. When 0, it is in parallel mode. The whole service uses the same setting.
- **C12, C13 and C14, National Option Character Subset.** These select which national variant of the Latin character set applies to the page (see A.11).

**Displayable bytes.** Bytes 14 to 45 are 32 characters displayed in columns 8 to 39 of row 0. Columns 0 to 7 of row 0 correspond to the eight Hamming-coded address and control bytes and are not displayed as text. The standard does not say what to show in columns 0 to 7. Annex E.7 notes that decoders reserve them for their own purposes, typically the page number (see decision E.10).

The last eight characters of the header, in columns 32 to 39, normally carry a real-time clock in the form HH:MM:SS (clause 9.3.1.4 and Annex E.14).

## A.6 How a page is transmitted and received

A page is transmitted as its header followed by its rows. Clause 7.2.1 defines which packets belong to a page: "Following the page header packet of a page, all subsequent packets with Y = 1 to Y = 28 inclusive, from the same magazine, relate to that page." It also defines where a page ends: "The transmission of a given page begins with, and includes, its page header packet. It is terminated by and excludes the next page header packet having the same magazine address in parallel transmission mode, or any magazine address in serial transmission mode."

- **Serial mode** (C11 = 1; Annex B.3). One magazine is transmitted at a time. A page's rows all follow its header, and the next header of **any** magazine ends it.
- **Parallel mode** (C11 = 0; Annex B.4). The magazines are interleaved. A page's rows are identified by magazine number, and only the next header of the **same** magazine ends it.

The wording given for C11 in Table 2 is simpler and slightly inconsistent with this. Decision E.6 records that clause 7.2.1 is followed.

The standard also sets the following rules:

- **Omitted packets.** Packets that carry nothing do not have to be sent (clause 7.2.3). A row may simply be absent. A missing row on a basic text page is displayed as 40 spaces (Annex D.1.1).
- **Packet order.** Within a page, packet 27 comes first after the header, then packet 28, then packet 26 in ascending designation code, and then the display rows (Annex B.2).
- **Erase delay.** The broadcaster must leave at least 20 milliseconds between a header and the rest of a Level 1 page, so that older decoders have time to erase their memory (Annex B.1).
- **Maximum gap.** The gap between successive packets of the same page must not exceed 100 milliseconds (Annex B.9). The note to that rule states the purpose explicitly: it lets "software-based decoders" decide when to display what they have received if the closing header is late or missing.

**The rolling header.** While a receiver is waiting for a requested page, it traditionally shows the text of each incoming header in row 0, so that the viewer sees the page numbers counting and the clock running.

- The standard asks that headers with C9 set are left out of the rolling display.
- In serial mode, headers from all magazines should be used (Annex B.3).
- In parallel mode, headers from only one magazine should be used (Annex B.4).

**Initial page.** If the service sends no broadcast service data packet, the receiver may assume page 100 is the introductory page (Annex D.3). Page 100 is the conventional "home" page (Annex E.15).

## A.7 Display rows (packets X/1 to X/25)

Packets 1 to 25 on a display page carry 40 characters each, in bytes 6 to 45. Each character has 7 data bits plus odd parity, and they are displayed in columns 0 to 39 from left to right (clause 9.3.2 and Table 25).

- Row 0 comes from the header.
- Rows 1 to 23 are the main page.
- Row 24 is an optional navigation row.

Every byte is either a displayable character (codes 2/0 to 7/F) or a **spacing attribute** (codes 0/0 to 1/F). A spacing attribute occupies a character cell. It is normally displayed as a space, and it changes the way the following characters on the same row are displayed.

In countries where some decoders are Level 1.0 only, broadcasters place a reasonable "fall back" character at each position that a Level 1.5 decoder will overwrite. In countries with no Level 1.0 decoders, they may put a byte with deliberately wrong parity there (Table 25). A Level 1.0 receiver can therefore expect some parity errors that are intentional.

## A.8 Spacing attributes and the Level 1 display model

The Level 1 display model is deliberately simple. Each row is processed from left to right, starting from a fixed set of default conditions. Each spacing attribute changes one element of the current state, and the state lasts until another attribute changes it or the row ends (Table 26 general notes). No attribute carries over from one row to the next.

**Set-at and set-after.** Some attributes take effect in their own cell ("Set-At"). The others take effect from the next cell ("Set-After"). Unless hold mosaics is active (see below), the cell holding an attribute displays as a space. That space is drawn with whatever state applies to that cell, so a set-at background change colours its own cell, but a set-after colour change does not.

**Start-of-row defaults.** At the start of every row the state is:
- alphanumeric mode, white foreground
- steady (not flashing)
- box off
- normal size
- contiguous mosaics
- black background
- release mosaics (hold off)
- not concealed
- the held mosaic character set to space
- the default G0 character set in use (not the alternative set selected by ESC)

These defaults come from Table 26 and clause 12.5.4.

**The attributes** (EN 300 706 Table 26). "L1" means the code has a defined effect at Level 1.0 and Level 1.5.

| Code | Name | Timing | L1 | Effect |
|---|---|---|---|---|
| 0/0 | Alpha Black | After | No | Level 2.5 only. Some older decoders act on it, but at Level 1 it has no defined effect. |
| 0/1 to 0/7 | Alpha Red, Green, Yellow, Blue, Magenta, Cyan, White | After | Yes | Sets the foreground colour and selects alphanumeric mode (the G0 character set). |
| 0/8 | Flash | After | Yes | Foreground pixels of the following characters alternate between the foreground and background colours. |
| 0/9 | Steady | At | Yes | Cancels flash. Default. |
| 0/A | End Box | After | Yes | Ends a boxed area. Default. |
| 0/B | Start Box | After | Yes | Starts a boxed area. Only meaningful on newsflash and subtitle pages. |
| 0/C | Normal Size | At | Yes | Cancels double height. Default. |
| 0/D | Double Height | After | Yes | Following characters are stretched down into the next row. |
| 0/E | Double Width | After | No | Level 2.5 only. |
| 0/F | Double Size | After | No | Level 2.5 only. Some Level 1 decoders treat it as double height. |
| 1/0 | Mosaic Black | After | No | Level 2.5 only. |
| 1/1 to 1/7 | Mosaic Red, Green, Yellow, Blue, Magenta, Cyan, White | After | Yes | Sets the foreground colour and selects mosaic mode (the G1 set). |
| 1/8 | Conceal | At | Yes | Following characters are displayed as spaces until revealed. Cancelled by any colour attribute or by the end of the row. |
| 1/9 | Contiguous Mosaics | At | Yes | Mosaic blocks touch each other. Default. |
| 1/A | Separated Mosaics | At | Yes | Each mosaic block is surrounded by a border of background colour. |
| 1/B | ESC (Switch) | After | Yes | Toggles between the first and second G0 sets, when a second set has been designated. |
| 1/C | Black Background | At | Yes | Background becomes black. Default. |
| 1/D | New Background | At | Yes | The current foreground colour becomes the background colour. |
| 1/E | Hold Mosaics | At | Yes | Attribute cells in mosaic mode show the held mosaic instead of a space. |
| 1/F | Release Mosaics | After | Yes | Cancels hold mosaics. Default. |

**Colours.** Level 1 has eight colours, numbered 0 to 7: black, red, green, yellow, blue, magenta, cyan and white.

- Bit 0 of the colour number is red, bit 1 is green and bit 2 is blue (Table 30, colour lookup table 0).
- Each primary is either fully on or fully off.
- There is no way to select a black foreground at Level 1.
- The background can be black, or any other colour by way of the New Background attribute.

**Mosaics.** In mosaic mode, codes in columns 2, 3, 6 and 7 are drawn as a 2-by-3 arrangement of blocks. The code bits map to blocks as follows (Table 26 note and Table 47):

| Block | Code bit | Value |
|---|---|---|
| Top left | bit 1 | 0x01 |
| Top right | bit 2 | 0x02 |
| Middle left | bit 3 | 0x04 |
| Middle right | bit 4 | 0x08 |
| Bottom left | bit 5 | 0x10 |
| Bottom right | bit 7 | 0x40 |

- Bit 6 (value 0x20) is always 1 for mosaic codes.
- Codes in columns 4 and 5 (0x40 to 0x5F) are **not** mosaics, even in mosaic mode. They display as the normal alphanumeric characters, a behaviour called "blast-through". The national option substitutions (see A.11) apply to blast-through characters too.
- Code 2/0 in mosaic mode is a mosaic space.

**Hold mosaics.** Normally every attribute takes up a cell that shows a space, so a change of colour inside a mosaic drawing leaves a visible gap. Hold Mosaics removes the gap. Table 26 states the rule exactly:

- While hold is active and the row is in mosaic mode, the cell of each attribute shows the "held mosaic" character instead of a space.
- The held mosaic is the most recent mosaic character on that row whose code has bit 6 set. Blast-through capitals do not qualify, because their bit 6 is clear.
- The held mosaic is reset to space at the start of each row, on any change between alphanumeric and mosaic mode, and on any change of size.
- It is not reset by repeating the current size, and not by switching hold on or off.
- The held mosaic is always displayed in the contiguous or separated form it had when it was stored, whatever the current mode.

Annex G.3.3 works through the sequence 17 66 1E 39 1A 1F 66, which shows these rules in action.

**Double height.** After a Double Height attribute, the following characters are twice the normal height. Their top half is drawn in the current row and their bottom half in the row below.

- The whole enlarged character takes the attributes that apply at its top half.
- If any character in a row is double height, the next row's transmitted content is **ignored entirely**.
- In that next row:
  - below each double-height character, its bottom half is drawn;
  - below each normal-height character, the cell shows that character's background colour with no foreground.

  (Table 26 and Annex C.3.)
- Broadcasters must not use double height in rows 0, 23 or 24. If one appears there, the decoder may do what it considers reasonable (Annex C.3).

**Boxing, newsflash and subtitles.** On a page with the newsflash (C5) or subtitle (C6) bit set, only the characters inside boxed areas are displayed, and the television picture shows everywhere else (Table 2, Table 26 and Annex C Table C.1).

- A boxed area starts after a Start Box attribute and ends after an End Box attribute or at the end of the row.
- The standard recommends transmitting Start Box twice in succession as protection against false triggering. Its worked example, the sequence x 0B 0B 61 0A 0A in Annex G.3.1, shows the boxed area covering the second 0B, the letter "a" and the first 0A. That is exactly what a single set-after Start Box and a set-after End Box produce.
- Start Box should not be used on ordinary pages (Annex E.13).

**Conceal and reveal.** Concealed characters display as spaces until the viewer asks to reveal them. The standard leaves the reveal mechanism to the receiver.

**Flash.** The standard does not specify a flash rate for Level 1. It is a receiver choice (decision E.12).

**Mix mode.** The standard mentions "mixed" mode, with text over the picture, only in passing (Annex G.1). It is a receiver feature.

## A.9 Row 24 and the Fastext navigation scheme

Row 24 (packet X/24) is displayed only when the page also has an editorial link packet X/27/0 whose Link Control Byte has its "display row 24" flag set (Table 14 and Annex H). If a page has no X/27/0, its row 24 is not displayed.

Packet X/27/0 carries six links (clause 9.6.1 and Table 14):

- **Links 1 to 4** go with the red, green, yellow and cyan keys. Their prompts are the coloured words in row 24.
- **Link 5** is used only to acquire an extra page automatically in the background.
- **Link 6** is the **Index** key.

**Link layout.** Each link takes six Hamming 8/4 bytes: page units, page tens, S1, S2, S3 and S4.

- The spare bits in the S2 and S4 bytes, in the same positions as C4, C5 and C6 of the header, carry three magazine bits called M1, M2 and M3.
- These bits are **relative**. Each one that is set inverts the corresponding bit of the magazine number of the X/27 packet itself. So the linked magazine is the packet's magazine exclusive-ORed with (M3, M2, M1).

**Null values.**
- Page number FF means "no particular page".
- Sub-code 3F7F means "no particular sub-page".
- The combination FF:3F7F means "no link".

**The Link Control Byte** is byte 43. Its data bit 4 is the "display row 24" flag.

**The checksum.** Bytes 44 and 45 carry a 16-bit cyclic redundancy check computed over the whole page. Checking it is optional (Annex H.2.5).

If a page has neither X/27 nor X/24, the receiver should disable the coloured keys and make the Index key go to the initial page (Annex H.4).

## A.10 Broadcast service data (packet 8/30)

Packet 30 in magazine 8 carries information about the service as a whole. Its designation code (byte 6) says which format it is. Format 1 (designation codes 0 and 1) is the one a receiver uses (clause 9.8.1 and Table 18):

- **Bytes 7 to 12, Hamming 8/4: the initial teletext page.**
  - Encoded like a Fastext link, but the magazine bits are absolute, not relative. They are split between bytes 10 and 12.
  - This is the page the service wants viewers to start on. FF:3F7F means none is specified.
- **Bytes 13 and 14, 8-bit: a 16-bit Network Identification code.**
  - This value is sent most significant bit first, which is unusual.
- **Byte 15, 8-bit: the local time offset from Coordinated Universal Time.**
  - Bits 2 to 6 give the offset in half hours.
  - Bit 7 gives the sign; 1 means west of Greenwich, so the offset is negative.
- **Bytes 16 to 18, 8-bit: the Modified Julian Date.**
  - This is a five-digit day number, where day 45000 is 31 January 1982.
  - Each decimal digit is transmitted increased by one.
- **Bytes 19 to 21, 8-bit: the time as Coordinated Universal Time.**
  - Hours, minutes and seconds as two digits each, again with each digit increased by one.
  - The time given is that of the **next** second boundary.
- **Bytes 26 to 45, odd parity: a 20-character status message,** such as the programme title.

Format 2 (designation codes 2 and 3) carries Programme Delivery Control information for video recorders, which is defined in a separate standard, EN 300 231. Format 1 is transmitted immediately after each second boundary (Annex B.5).

## A.11 Character sets at Level 1

The basic alphanumeric set is the **Latin G0 primary set** of 96 characters, codes 2/0 to 7/F (Table 35). It is similar to ASCII except for two points:

- Position 7/F is a solid block.
- Thirteen positions are **national option positions**: 2/3, 2/4, 4/0, 5/B, 5/C, 5/D, 5/E, 5/F, 6/0, 7/B, 7/C, 7/D and 7/E.

When a Level 1 page refers to a national option position, the character shown depends on which national option subset is in force. Table 36 defines subsets for:
- Czech and Slovak
- English
- Estonian
- French
- German
- Italian
- Lettish and Lithuanian
- Polish
- Portuguese and Spanish
- Rumanian
- Serbian, Croatian and Slovenian
- Swedish, Finnish and Hungarian
- Turkish

For example, in the English subset:
- 2/3 is the pound sign
- 5/B, 5/D and 5/E are the left, right and up arrows
- 5/C is one-half
- 7/B is one-quarter
- 7/C is a double vertical bar
- 7/D is three-quarters
- 7/E is the division sign

In the German subset:
- 4/0 is the section sign
- 5/B, 5/C and 5/D are Ä, Ö and Ü
- 7/B, 7/C and 7/D are ä, ö and ü
- 7/E is ß

**How the subset is chosen.** The subset is chosen by the three header bits C12, C13 and C14. Three bits give only eight values, but there are thirteen subsets, so the same three-bit value means different languages in different regions.

Table 32 resolves this with a 7-bit code:
- The high four bits give a "group", in effect a region.
- The low three bits are C12, C13 and C14.
- C12 is the most significant of the three, according to Table 32 note 1, which assigns C12 to bit 10, C13 to bit 9 and C14 to bit 8.

In the default Western European group 0000, the values are:

| C12 C13 C14 | Subset |
|---|---|
| 000 | English |
| 001 | German |
| 010 | Swedish, Finnish and Hungarian |
| 011 | Italian |
| 100 | French |
| 101 | Portuguese and Spanish |
| 110 | Czech and Slovak |

The group can be signalled in packet X/28/0 format 1 or packet M/29/0. A Level 1.0 decoder does not read those packets. It uses C12 to C14 alone together with a regional default, which the standard calls a "local Code of Practice" (Annex E.9).

**The G1 mosaic set** (Table 47) has 64 mosaic characters, including the mosaic space, plus the 32 blast-through alphanumerics.

The standard gives no exact pixel geometry for characters or mosaics. It says the precise appearance "is a decoder option" and that its drawings use a matrix of 12 pixels horizontally by 10 pixels vertically (clause 15.6 and Annex G.2).

## A.12 Presentation levels and what Level 1.0 excludes

Clause 12 defines the levels:
- **Level 1** is "the basic Teletext page, characterized by the use of spacing attributes only and a limited alphanumeric and mosaics repertoire".
- **Level 1.5** is where the "decoder responds as Level 1 but the character repertoire is extended via packets X/26".
- **Level 2.5 and Level 3.5** add non-spacing attributes, colour tables, objects, redefinable characters, side panels and more.

Level 2.5 and Level 3.5 were out of scope when this walkthrough was written. They were brought into scope on 10 October 2026 (decision E.40), and are specified in areas L25, OBJ and DRC of Part F. Level 1.0 does not include:
- the black foreground codes;
- double width and double size;
- the supplementary character sets G2 and G3;
- any use of packet X/26 for display.

## A.12a How the specification evolved

EN 300 706 is the last of a line of teletext specifications. The earlier ones are held in `~/devel/jellytext/teletextspecifications/`:

| Year | Document | What it contributed |
|---|---|---|
| 1974 | First joint BBC, IBA and BREMA teletext specification | The original service. Its control codes differ entirely from later ones (for example, alphanumeric colours were at 1/9 to 1/F). It already split mosaic cells 3:4:3. |
| 1976 | BBC, IBA and BREMA "Broadcast Teletext Specification" | The Level 1 display model as it is still used, including hold graphics and the English character set. The SAA5040 was designed to this edition. Header bits C12 to C14 were not yet allocated. |
| 1990 | "World System Teletext" (Comité consultatif international pour la radio (CCIR) System B) | National option selection by C12 to C14, and enhancement packets |
| 1992 | EBU SPB 492 | The document EN 300 706 replaced. It defines the character set codes of packets X/28/1 and M/29/1, and the row-address rule behind Annex A.3. |
| 1997 | ETS 300 706 | The first ETSI edition |
| 2003 | EN 300 706 V1.2.1 | The current edition, and the primary source of this document |

The main points where the older documents clarify EN 300 706 are recorded in decisions E.3, E.4 and E.16.

## A.13 The Table Of Pages navigation scheme

The "Table Of Pages" scheme, used mainly in Germany, Austria and Switzerland, is only outlined in EN 300 706:

- Clause 11.2 names its four page types:
  - the Basic Top Table, always on page 1F0;
  - the Additional Information Table;
  - the Multi-Page Table;
  - the Multi-Page Extension Table.
- It gives their packet coding, but not their contents.
- Annex I refers to a separate document for the full specification: Institut für Rundfunktechnik, ARD/ZDF Technical Guideline 8 R 5 (1991).

That document is not in the project's source set, so the Table Of Pages scheme is out of scope for now (see Part G).

---

# Part B — Walkthrough of Presentation Level 1.5 (EN 300 706)

## B.1 Why Level 1.5 exists

The Latin G0 set with national options gives each page 96 alphanumeric characters. Many languages need more than that. Level 1.5 "has evolved in the field due to the language requirements in certain countries" (EN 300 706 introduction) and was documented formally for the first time in EN 300 706.

A Level 1.5 decoder responds exactly as a Level 1.0 decoder does, except that it also reads packets X/26 attached to the page. Each X/26 packet carries instructions of the form "at row r, column c, display character k from character set S instead of what the Level 1 page has there". It is a character-overwrite mechanism and nothing more (clauses 10.1, 12 and 15.1, and Table 1).

The characters that can be written are:
- a character from the **G2 supplementary set**, which contains currency signs, fractions, typographic marks and additional letters;
- a **G0 character combined with a diacritical mark**, such as é, č, ő or ł;
- a plain **G0 character** without national option substitution;
- a small number of **G3 line-drawing characters**.

Clause 15.1 describes typical use: "a few characters from the G2 supplementary set of characters, plus a few G0 characters with diacritical marks."

## B.2 Packets used at Level 1.5

Clause 10.1 lists the Level 1.5 packets. Figure 17 and Table 1 confirm them.

- **X/26/0 to X/26/15.** The character overwrites. The same packet type also carries Programme Delivery Control data, which a Level 1.5 display decoder must skip.
  - Table 1 marks only designation codes 0 to 14 as Level 1.5 character overwrites.
  - Clause 10.1 notes that "some existing Level 1.5 decoders only respond to packets X/26 with designation codes in the range 0 to 14". The SAA5246A (Part D) stores only designation codes 0 to 14.
- **X/28/1 and M/29/1.** Character set designation "for compatibility with earlier specifications only". The meaning of the 7-bit character set codes in these packets is defined in EBU SPB 492 (1992), Figure 14. For example, code 6 is the first Latin set, 8 is block mosaics, 12 is Cyrillic and 16 is Greek. EN 300 706 Table 25 says equipment built for a single group of character sets "may ignore this data".
- **X/28/0 format 1 and M/29/0.** Table 1 marks their presentation data as Level 2.5 only. Table 25, however, ticks their default character set designation function at Level 1.5, 2.5 and 3.5. The designation they give is "also the default G0 and G2 sets to be addressed by packet 26 triplets as part of Level 1.5" (clause 15.2). Decision E.4 records how this contradiction is handled.
- **8/30 formats 1 and 2.** As at Level 1.0.

## B.3 Hamming 24/18 and the triplet

Packet X/26 is made up of:
- byte 6, a Hamming 8/4 **designation code** from 0 to 15;
- bytes 7 to 45, thirteen **triplets** of three bytes each. Triplet number n (1 to 13) occupies bytes 3n+4, 3n+5 and 3n+6.

Each triplet is protected by the Hamming 24/18 code (clause 8.3). Of the 24 bits, bits 1, 2, 4, 8, 16 and 24 are protection bits. The other 18 bits carry data.

**Bit layout.** Numbering bits in transmission order, and so least significant bit first in each byte, the layout is:

| Byte | Bit positions and meaning |
|---|---|
| First | 1 P1, 2 P2, 3 D1, 4 P3, 5 D2, 6 D3, 7 D4, 8 P4 |
| Second | 9 to 15 are D5 to D11, 16 is P5 |
| Third | 17 to 23 are D12 to D18, 24 is P6 |

**Encoding equations** (clause 8.3), all using odd parity:

- P1 = 1 ⊕ D1 ⊕ D2 ⊕ D4 ⊕ D5 ⊕ D7 ⊕ D9 ⊕ D11 ⊕ D12 ⊕ D14 ⊕ D16 ⊕ D18
- P2 = 1 ⊕ D1 ⊕ D3 ⊕ D4 ⊕ D6 ⊕ D7 ⊕ D10 ⊕ D11 ⊕ D13 ⊕ D14 ⊕ D17 ⊕ D18
- P3 = 1 ⊕ D2 ⊕ D3 ⊕ D4 ⊕ D8 ⊕ D9 ⊕ D10 ⊕ D11 ⊕ D15 ⊕ D16 ⊕ D17 ⊕ D18
- P4 = 1 ⊕ D5 ⊕ D6 ⊕ D7 ⊕ D8 ⊕ D9 ⊕ D10 ⊕ D11
- P5 = 1 ⊕ D12 ⊕ D13 ⊕ D14 ⊕ D15 ⊕ D16 ⊕ D17 ⊕ D18
- P6 = 1 ⊕ every other bit of the triplet

**Decoding** uses six odd-parity tests. Bit positions are numbered 1 to 24 in transmission order:

- Test A covers every position whose binary value has bit 0 set: 1, 3, 5, 7 and so on.
- Test B covers positions with bit 1 set.
- Test C covers positions with bit 2 set.
- Test D covers positions with bit 3 set.
- Test E covers positions with bit 4 set.
- Test F covers all 24 bits.

The results are interpreted in the same way as for Hamming 8/4:

| Tests A to E | Test F | Meaning | Action |
|---|---|---|---|
| All pass | Passes | No error | Accept the data |
| All pass | Fails | Error in P6 | Accept the data |
| Any fails | Passes | Double error | Reject the triplet |
| Any fails | Fails | Single error | Correct the bit and accept |

For a single error, the position of the wrong bit is the binary number formed from the failed tests: 16 times E, plus 8 times D, plus 4 times C, plus 2 times B, plus 1 times A. The standard does not say what to do with a rejected triplet, or with an error position greater than 24, which can only come from three or more errors. Decision E.9 covers both.

**The 18 data bits.** They are read as a number with D1 as the least significant bit and are divided into three fields (clause 12.3.1):

- **Address**: 6 bits, D1 to D6. A value from 0 to 39 is a **column address**. A value from 40 to 63 is a **row address**.
- **Mode**: 5 bits, D7 to D11. It says what the triplet does.
- **Data**: 7 bits, D12 to D18. Typically a character code.

## B.4 The active position

X/26 processing keeps an **active position**, a row and a column, that says where the next character goes (clause 12.3.2). It starts at row 0, column 0 at the beginning of each page's enhancement data.

**Row address triplets** (address 40 to 63) set the row:
- Address 40 means row 24.
- Addresses 41 to 63 mean rows 1 to 23.
- Row 0 can only be reached with the special mode "Address Display Row 0" (mode 00111) and address 63. That triplet sets both row and column to 0.

**Column address triplets** (address 0 to 39) set the column to the address value and leave the row unchanged. Then, depending on the mode, they place a character at that position.

**Which row triplets set the row.** The standard contradicts itself here:

- Clause 12.3.2 says the row is set only by the Full Row Colour, Set Active Position and Address Display Row 0 triplets. It adds that Level 2.5 and Level 3.5 decoders should not take the row from any other row triplet.
- Annex A.3, which is normative, describes what existing Level 1.5 decoders actually do: "The most recent value of the address bits in the range 40 to 63 will be treated by existing Level 1.5 decoders as setting the row co-ordinate of the Active Position, regardless of the mode value of the triplet."
- Annex E.3 repeats the Annex A.3 point.
- Annex A.5 even recommends that broadcasters send the Full Screen Colour triplet with address 41, which is row 1, precisely so that it does no harm on Level 1.5 decoders that behave this way.

Decision E.3 records how this is handled.

## B.5 The triplet modes a Level 1.5 decoder acts on

Tables 27, 28 and 29 of EN 300 706 define 32 row modes and 32 column modes. The level tick marks in those tables show which ones apply at Level 1.5.

**Row address group** (address 40 to 63):

| Mode | Function | Level 1.5 action |
|---|---|---|
| 00100 | Set Active Position | Sets the active row. At Level 1.5 only the row part is ticked. The column in the data field is a Level 2.5 feature. |
| 00111 | Address Display Row 0 | With address 63, sets the active position to row 0, column 0. Later column triplets should then address only columns 8 to 39. |
| 01000 to 01101 | Programme Delivery Control data | Not display data. Skipped for display. |
| 11111 | Termination Marker | With address 63, marks the end of the page's enhancement data. The broadcaster fills any unused triplets after it with copies of it. |
| All others | Level 2.5 and 3.5 functions, or reserved | Ignored for display. Under Annex A.3 behaviour they still set the row. |

**Column address group** (address 0 to 39):

| Mode | Function | Level 1.5 action |
|---|---|---|
| 00010 | Character from the G3 set, Level 1.5 form | Places the G3 character given by the data field. Annex E.3 notes that in practice only G3 positions 5/1 (horizontal line), 5/B (right arrow), 5/C (left arrow) and 5/D (up arrow) are used at Level 1.5. |
| 01111 | Character from the G2 set | Places the G2 character given by the data field. |
| 10000 | G0 character without diacritical mark | Places the G0 character given by the data field. As a special case, data value 2/A (binary 0101010) displays "@" instead of "*". |
| 10001 to 11111 | G0 character with diacritical mark | The low four bits of the mode select a diacritical mark from column 4 of the G2 set. That mark is combined with the G0 character given by the data field. |
| 00110 | Programme Delivery Control data | Skipped. The active position is not changed. |
| 01011 | Character from the G3 set, Level 2.5 form | Must be **ignored** by a Level 1.5 decoder (Table 29 note 3). |
| All others | Level 2.5 and 3.5 functions, or reserved | Ignored for display. |

Data values below 0x20 are reserved for the character modes, but the column must still be set to the address value (Table 29).

**The sixteen diacritical marks.** Column 4 of the G2 set, positions 4/0 to 4/F, holds these marks in order:
1. none
2. grave
3. acute
4. circumflex
5. tilde
6. macron
7. breve
8. dot above
9. diaeresis (umlaut)
10. a low dot, at position 4/9
11. ring above
12. cedilla
13. a low line, at position 4/C
14. double acute
15. ogonek
16. caron

The identity of positions 4/9 and 4/C is read from the drawing in Table 37, because the standard gives no names. The standard does not say how the mark and the base letter are drawn together. That is a decoder option (Annex G.2), and Annex E.10 says composition should be limited to the Latin sets.

**No national option substitution.** G0 characters written through X/26 never have national option substitution applied (Table 25 and Table 35 note 2). At the thirteen national option positions they take the base glyphs shown in Table 35: #, ¤, @, [, \, ], ^, _, `, {, |, } and ~. The ESC attribute on the Level 1 page does not affect X/26 characters either (Table 26, code 1/B).

## B.6 The G2 and G3 sets

**The Latin G2 supplementary set** (Table 37) holds 95 characters plus space:

| Column | Contents |
|---|---|
| 2 | Inverted exclamation mark, cent, pound, dollar, yen, hash, section sign, currency sign, single and double quotation marks, guillemets, and four arrows |
| 3 | Degree, plus-minus, superscript 2 and 3, multiplication sign, micro sign, pilcrow, middle dot, division sign, closing quotation marks, guillemets, one-quarter, one-half, three-quarters and inverted question mark |
| 4 | The sixteen diacritical marks |
| 5 | Horizontal bar, superscript 1, registered, copyright, trade mark, musical note, European currency sign, per mille, alpha, and the eighths fractions |
| 6 and 7 | Additional letters and symbols: Ω, Æ, Đ, ª, Ħ, Ĳ, Ŀ, Ł, Ø, Œ, º, Þ, Ŧ, Ŋ and ŉ, with their lower-case partners, and ĸ, ð, ı, ß, and a solid block at 7/F |

Positions 5/9, 5/A, 5/B and 6/5 are not defined and should not be transmitted. Cyrillic, Greek and Arabic have their own G2 sets (Tables 41, 43 and 45). Hebrew pages use the Arabic G2 set.

**The G3 set** (Table 48) holds smoothed mosaic and line-drawing characters. Only the four positions listed under mode 00010 in B.5 matter in practice at Level 1.5.

## B.7 Other character sets

For completeness:

- EN 300 706 defines non-Latin G0 sets: three Cyrillic options (Tables 38 to 40), Greek (Table 42), Arabic (Table 44) and Hebrew (Table 46).
- Table 32 selects them in the non-Western groups. For example, group 0100 with C12 C13 C14 = 100 selects Cyrillic option 2 for Russian and Bulgarian.
- These sets have no national option positions.

## B.8 Ordering, termination and compatibility rules

- **Packet order.** Packets X/26 of a page are sent before its display rows, in ascending designation code order starting from 0 (Annex B.2 and Annex A.3).
- **Gaps.** A gap in the sequence of designation codes before the Termination Marker "may be interpreted as an error condition", and the decoder's response is not defined (Annex D.1.3).
- **Triplet order.** Triplets are in screen order, left to right and top to bottom. A decoder may ignore a triplet that addresses a position earlier than the one before it (Annex A.4).
- **One character per position.** There should be only one character-writing triplet per screen position that a Level 1.5 decoder could act on (Annex A.3).
- **Termination.** A Termination Marker ends the local enhancement data. After it may come object definitions, which a Level 1.5 decoder ignores (Annex A.3).
- **Row 0 during a search.** X/26 characters in row 0 must appear when the page is displayed in full. They need not appear in the rolling header while searching (Annex E.3).
- **Pages without X/26.** If a page has no X/26 packets, a Level 1.5 decoder shows the Level 1 page unchanged (Annex D.1.2).

## B.9 How an overwrite interacts with the Level 1 page

The standard defines the overwrite only as "overwriting the character defined for this location on the Level 1 page" (Table 1). It is silent on most interactions:

- whether a spacing attribute at the overwritten cell still takes effect;
- whether an overwritten character in a concealed area is concealed;
- how it behaves in mosaic mode;
- whether it changes the held mosaic;
- whether it can appear on the lower row of a double-height row.

The only guidance comes from the description of objects in clause 13.3, which says enhancement data takes the Level 1 attributes that apply at its position. Table 26 notes 3 and 6 add that "some existing Level 1.5 decoders will not display local X/26 enhancement data" on the lower row of a double-height row.

Decision E.5 records the interpretation this project adopts.

---

# Part C — Walkthrough of the Philips SAA5050 Teletext Character Generator

## C.1 Why the SAA5050 is the rendering baseline

The SAA5050 was the character generator in the original Mullard and Philips teletext chipset of 1976 to 1979. It was also used in the BBC Microcomputer's "Mode 7" display, the Philips P2000 and many other machines. More people have seen teletext as drawn by this chip than by any other part, which is why it is the reference for how a Level 1 page should look.

The following sources are available locally:
- the Signetics datasheet in `philips-saa5050/`;
- the Philips 1979 databook in `databooks/`. The SAA5050 data sheet itself is on PDF pages 180 to 191, and the one-page SAA5051 supplement is on page 192.

The page numbers for this chipset in `DATASHEET-INDEX.md` are partly wrong. They are corrected in Appendix B.

## C.2 Its place in the original four-chip receiver

The original receiver used four chips and a page memory, working together (databook pages 136 to 191):

1. **SAA5030 Video Input Processor.** Separates the synchronising pulses from the composite video signal. It slices the teletext data at the midpoint between two peak detectors and recovers a 6.9375 megahertz data clock. It also runs a crystal-controlled 6 megahertz oscillator locked to the picture, which becomes the display dot clock.
2. **SAA5040 Teletext Acquisition and Control circuit.**
   - Designed to "the September 1976 Broadcast Teletext specification published by BBC/IBA/BREMA" (databook page 156).
   - Checks Hamming and parity, selects the requested page, and writes its rows into the page memory as 7-bit characters.
   - Accepts remote-control commands and can write status information into the memory.
3. **SAA5020 Timing Chain.** Divides the 6 megahertz clock into:
   - the 1 megahertz character rate;
   - the line and field rates;
   - the memory row addresses for 24 rows of 10 television lines each.

   It also produces the timing signals the SAA5050 needs.
4. **SAA5050 Character Generator.** Reads one 7-bit character from the page memory every microsecond. It decodes the control characters and produces red, green, blue, luminance and blanking outputs for the television.

## C.3 Display geometry and timing

The datasheet states the geometry directly: "The character period is 1 µs and the character dot rate is 6 MHz. Each character rectangle is 6 dots wide by 10 TV lines high. One dot space is left between adjacent characters, and there is one line space left between rows. Alphanumeric characters are generated on a 5 x 9 matrix, allowing space for descending characters."

- **Rows and columns.** The display has 40 characters per row and 24 rows. Each row is 10 television lines per field, and the display occupies lines 49 to 288 and 362 to 601. No 25th row is supported.
- **Interlace.** The same row data is shown in both fields of the interlaced picture. The chip is told which field is being drawn by the "character rounding select" input, and uses it only for character rounding.
- **Effective resolution.** With the half-dot rounding described in C.4 and both interlaced fields, the effective resolution of a character cell is 12 half-dots wide by 20 lines high.
- **Pipeline delay.** Alphanumeric dots emerge from the chip one dot period (about 167 nanoseconds) later than mosaic dots for the same character: 2.767 microseconds against 2.6 microseconds. This is a detail only a pixel-exact emulation would model.

## C.4 Character rounding

The SAA5050's characteristic smooth look comes from "character rounding". The datasheet describes it as follows: "a half dot is inserted before or after a whole dot in the presence of a diagonal in a character matrix."

- The ROM is read twice in each character period, once for the current glyph row and once for a neighbouring row. The two rows are compared to find diagonals.
- **Normal height.** The neighbour is the row before in one field and the row after in the other field.
- **Double height.** The reference direction alternates up and down on every television line.
- **Scope.** Rounding applies only to alphanumeric characters. Mosaic characters are generated directly from the code bits and are never rounded.

The datasheet does not give the exact rule for detecting a diagonal. It has since been reconstructed and cross-checked by others:
- the MAME emulator's SAA5050 device;
- the BBC Micro emulators jsbeeb and beebjit;
- the Bedstead font;
- an analysis of a decapsulated SAA5050 published on the stardot forum.

All of them agree. Decision E.13 records the rule, and REQ-RND-012 makes it a testable requirement.

## C.5 Control characters as the SAA5050 implements them

The SAA5050 control code table (datasheet Table 1) matches the Level 1 table in Part A.8, with these differences:

- **Reserved codes.** Codes 0/0, 0/E, 0/F, 1/0 and 1/B are "reserved for compatibility with other data codes" and display as spaces. The SAA5050 therefore has no black foreground, no double width or double size, and no ESC character-set switching.
- **Defaults.** The codes presumed at the start of every row are alpha white, steady, end box, normal height, contiguous graphics, black background and release graphics. They are reset in hardware at the start of every display line.
- **Display of control characters.** Control characters "are normally displayed as spaces". The word "normally" allows for the hold graphics exception, but the datasheet does not describe hold graphics beyond naming the codes.
- **Timing.** The datasheet does not say which codes are set-at and which are set-after. That comes from EN 300 706 Table 26 (Part A.8).

## C.6 Mosaics

- **Cell layout.** A mosaic character fills the whole 6-dot by 10-line cell, divided into 2 columns and 3 rows of blocks.
- **Block sizes.** Measured from the datasheet drawing (databook page 187, Figure 3), the blocks are 3 dots wide and, from top to bottom, 3, 4 and 3 lines high.
- **Bit mapping.** Code bits 1, 2, 3, 4, 5 and 7 light the top-left, top-right, middle-left, middle-right, bottom-left and bottom-right blocks respectively. Bit 6 is always 1 for mosaic codes.
- **Separated mosaics.** Each block is drawn as a smaller rectangle surrounded by background colour. The datasheet gives no dimensions for the gap. The emulators and the decapsulated-chip analysis agree that the leftmost dot column of each block and the bottom line of each block are blanked (decision E.14).
- **Blast-through.** Codes in columns 4 and 5 display as alphanumerics even in graphics mode.
- **Solid block.** The alphanumeric solid block at 7/F is drawn as a rectangle with a background margin. In contrast, the mosaic 7/F fills the entire cell.

## C.7 Double height in the four-chip system

When a row contains a Double Height control code, the SAA5050 signals the timing chain through its "transmitted large characters" output:

- The timing chain then displays the memory row for 20 television lines instead of 10.
- It skips the following memory row altogether: "If any row contains transmitted large characters the address is incremented by two after 20 lines" (databook page 142).
- The SAA5050 continues its own glyph-row counter through the second ten lines to produce the bottom halves.

This matches the EN 300 706 rule that the transmitted content of the row below a double-height row is ignored.

The chip also supports a viewer-selected "large characters" mode, a page zoom, that shows the top or bottom half of the page at double height.

## C.8 Flash, conceal, boxing and display modes

- **Flash.** "The circuit generates a 0.75 Hz signal with a 3:1 ON/OFF ratio to provide the flashing character facility." The real chip counts fields with a 6-bit counter: flashing characters are visible for 48 fields and hidden for 16. At 50 fields per second that is 960 milliseconds visible and 320 milliseconds hidden, a cycle of 1.28 seconds (0.78 hertz). The datasheet rounds this to 0.75 hertz. The MAME, jsbeeb and beebjit emulators all use 48 and 16.
- **Conceal and reveal.** Concealed characters are shown only in reveal mode. There are two ways to reveal:
  - "Reveal set" latches reveal mode on. Any other command clears it.
  - A momentary "Reveal" command expires 160 milliseconds after the last one was received.

  At power-on the chip starts in television mode, conceal mode and not superimposed.
- **Boxing and superimposition** (datasheet Table 3):
  - With the picture on and teletext enabled but not superimposed, only text inside boxes is displayed. The blanking output cuts a hole in the picture for the box.
  - In superimpose mode, text outside boxes is displayed over the picture without blanking, and **background colour is suppressed** wherever blanking is not active.
  - In "tv" mode, text is allowed on the top row only, which is how the rolling header appears over the picture.

## C.9 National character sets

Each SAA5050 variant has one fixed character set in its ROM:

| Part | Character set |
|---|---|
| SAA5050 | English |
| SAA5051 | German |
| SAA5052 | Swedish |
| SAA5053 | Italian |
| SAA5054 | Belgian |
| SAA5055 | United States ASCII |
| SAA5056 | Hebrew |
| SAA5057 | Cyrillic |

The chip does not act on the national option bits C12 to C14; a manufacturer chose a language by fitting the matching variant.

The English set differs from the EN 300 706 English national option subset only in how glyphs are drawn. The code assignments are the same: £ at 2/3, arrows and ½ at 5/B to 5/E, # at 5/F, a long dash at 6/0, and ¼, ‖, ¾ and ÷ at 7/B to 7/E.

## C.10 What the SAA5050 does not do

The SAA5050 is a Level 1 device of the 1976 era. It has:
- one character set;
- no black foreground;
- no double width or double size;
- no ESC switching;
- no supplementary character sets, so no Level 1.5;
- 24 rows only;
- seven foreground colours and a black or coloured background.

---

# Part D — Walkthrough of the Philips SAA5246A Integrated Teletext Decoder

## D.1 Why the SAA5246A is the receiver-behaviour baseline

The SAA5246A (January 1993) belongs to Philips's "Computer Controlled Teletext" family:
- it is based directly on the SAA5243 "Enhanced Computer Controlled Teletext" device;
- it is succeeded by the SAA5281.

That family was fitted to a very large share of European television sets in the late 1980s and the 1990s. It is the best available model of how a real receiver acquires pages, stores them, handles row 24 and Fastext, and shares work between the chip and the television's microcontroller.

The document in `philips-saa5246a/` is the product datasheet, not an applications handbook. It gives the register map, memory map and character tables, but almost no description of how the chip behaves internally. Wherever it is silent, this document says so, and the matching decision is recorded in Part E.

## D.2 Architecture

The SAA5246A is "a single-chip teletext decoder IC for decoding 625 line base World System Teletext transmissions". It integrates:
- a composite video input clamp and synchronising pulse separator;
- an analogue-to-digital converter;
- a digital data slicer and clock regenerator, with "data capture performance similar to SAA5231";
- teletext acquisition and decoding;
- a display timing chain;
- a display clock phase-locked loop;
- a 27 megahertz crystal oscillator.

The page memory is an external 8 kilobyte static memory. A microcontroller controls the chip over the two-wire I²C bus.

**The chip does the following by itself:**
- slicing and acquisition;
- matching up to four page requests;
- writing received rows into memory;
- the rolling header and rolling clock;
- a "looking for page" marker;
- storing each received page's header control bits;
- selecting the national option subset from C12 to C14;
- optionally storing and showing row 24;
- optionally storing extension packets;
- generating the display.

**The microcontroller does everything else:**
- the user interface and page number entry;
- status messages;
- interpreting packets X/26, X/27, X/28 and 8/30;
- following Fastext links;
- any sub-page management beyond simple digit matching.

## D.3 Acquisition

**Page memory.** The SAA5246A has **eight page memories**, called chapters, and **four acquisition circuits**.

- Each acquisition circuit is given a page request by the microcontroller and fills one chapter.
- The request is written as seven digit fields: magazine, page tens, page units, then four sub-code digits called hours tens, hours units, minutes tens and minutes units.
- Each field has a **"do care" bit**. When it is 0, that digit is ignored during matching. This is how the chip supports "any sub-page" requests and timed-page requests.
- The magazine field also carries a **hold** bit that freezes the page so it is not updated.

**Extension packets.** In extension packet mode each chapter takes 2 kilobytes, so four pages can each have an extension area. The extension area stores:
- packets X/26/0 to X/26/14 (designation code 15 is not stored);
- X/27/0 and X/27/1;
- X/27/4 and X/27/5;
- X/28/0, X/28/1 and X/28/2;
- X/24 (when it is not placed in display row 24);
- X/25;
- one row for 8/30.

Magazine-related packets (packet 29) are not stored.

**Row 25: per-page control data.** Each chapter has a "row 25" holding control data for the received page:
- the page number, sub-code and control bits C4 to C14;
- a Hamming error flag for each header byte;
- the magazine;
- a "page found" flag;
- a "page being looked for" flag.

**Row 0 layout.**
- Columns 0 to 6 are free for the microcontroller's status messages, typically the page number the viewer has typed.
- Column 7 is written automatically with an alphanumeric control character. It is white normally and **green while the page is being looked for**, so that the rolling header appears in green during a search.
- Columns 8 to 31 carry the rolling header text.
- Columns 32 to 39 carry the clock, which always keeps rolling.

**Errors and memory.**
- Parity checking can be switched off for 8-bit data. The datasheet does not say what is written to memory when a character fails parity.
- The chip can optionally check the Hamming 8/4 code of packets X/27/0, X/27/1 and 8/30.
- At power-on, all memory is cleared to spaces.

## D.4 Display

- **Layout.** The display is 40 characters by 25 rows. Rows 0 to 23 are the teletext page. Row 24 is "the extra row available for software generated status messages and FLOF/FASTEXT prompt information".
- **Automatic Fastext row.** When the "X24 position" bit is set, the chip stores packet X/24 into display row 24 and shows it automatically as the Fastext prompt row.
- **Status row position.** A register bit places this 25th display row above or below the main page.
- **Character cell.** The cell is **12 dots wide by 10 lines high**, at 1 microsecond per character and 10 lines per row per field. The character tables are drawn on a 12-by-10 grid, and the parent SAA5243 advertises a "high quality flicker-free display using a 12 x 10 character matrix".
- **Rounding.** The datasheet does not mention character rounding.

**Attributes.** Every Level 1 spacing attribute is supported, as are two additions:
- **Black foreground codes.** Alpha black (0/0) and graphics black (1/0) are decoded, even though they are Level 2.5 codes in EN 300 706.
- **ESC or TWIST.** In variants with two character sets, the code 1/B (ESC, called "TWIST" in some variants) switches between the two sets.

**Display control.** There are two sets of display controls:
- one set for normal pages;
- one set for newsflash and subtitle pages.

Each set has separate switches for inside and outside boxed areas, covering picture on, text on, contrast reduction and background colour on. Text mode, television mode, mixed mode and boxed mode are all combinations of these switches. Boxing can also be enabled separately for row 0, rows 1 to 23 and row 24.

**Viewer functions.**
- Reveal is a register bit.
- Page zoom shows the top or bottom half of the page at double height.
- A cursor can be shown.
- The datasheet does not state the flash rate.

## D.5 National option ROM variants

The chip "automatically decodes transmission bits C12 to C14" to choose among the national subsets built into its ROM. Eight variants exist:

| Variant | Region | Languages selected by C12, C13, C14 | Default |
|---|---|---|---|
| /E | West European | 000 English, 001 German, 010 Swedish, 011 Italian, 100 French, 101 Spanish | English |
| /H | East European | 000 Polish, 001 German, 010 Swedish, 101 Serbo-Croat, 110 Czech, 111 Rumanian | German |
| /T | West European and Turkish | West European languages, plus 110 Turkish | English |
| /J | Yugoslav | Serbo-Croat Latin, Hungarian and Swedish, Rumanian, plus 100 Serbo-Croat Cyrillic | Serbo-Croat Latin |
| /K | French and Arabic | 100 French, 111 Arabic | French |
| /L | Arabic and Hebrew | 101 Hebrew with English, 111 Arabic | Hebrew and English |
| /I | Greek and Turkish | 110 Turkish, 111 Greek | Turkish |
| /S | English and Thai | 000 English, 010 Thai | English |

Combinations that a variant does not support fall back to that variant's default language.

These assignments agree with EN 300 706 Table 32 for the groups each variant serves. For example, the /E variant is group 0000 and the /H variant is group 0001.

## D.6 Level 1.5 on the SAA5246A

The SAA5246A has **no hardware that decodes packet X/26**. It only stores the packets. The ROM contains extra glyphs at code positions that can only be reached with the eighth bit set:
- accented Latin letters in the Western variants;
- Cyrillic, Greek, Arabic and Thai characters in the others.

Level 1.5 was therefore a microcontroller software function. The television's software read the stored X/26 triplets, decoded the Hamming 24/18 code, and wrote the appropriate extended character codes into the page memory.

Two facts confirm this:
- The successor SAA5281 lists "automatic packet 26 extension character processing" and a "24 : 18 Hamming checker" as **new** features.
- The parent SAA5243 datasheet says: "For certain languages, control software processing of the extension packet data may be required."

This is a useful precedent for this project. Level 1.5 as a software pass over a stored Level 1 page, run after acquisition, is exactly how the reference receiver did it.

## D.7 Fastext, Table Of Pages and 8/30 on the SAA5246A

**Fastext.** The chip stores X/27/0 and can show row 24 automatically. Decoding the six links, responding to the coloured keys and pre-fetching linked pages into spare chapters is left to the microcontroller.

**Table Of Pages.** It is not mentioned anywhere in the SAA5246A document. It first appears in the successor SAA5281, which can "capture 7 Fastext pages and 8 pages of TOP".

**Packet 8/30.** Packets 8/30 are stored raw in extension memory, and their interpretation is left to the microcontroller.

## D.8 What the SAA5246A leaves undocumented

The datasheet gives no information about any of the following:
- the slicing method or framing-code tolerance;
- which lines are accepted (its parent accepts lines 2 to 22);
- what is stored on a parity error;
- how serial and parallel magazine modes end pages;
- whether the erase bit clears memory;
- how C7 to C10 are acted on;
- the flash rate.

Part E records what this project does in each case.

---

# Part E — Decision Register

Each entry records a place where the standard is silent, contradicts itself, or differs from the reference chips. It states the decision taken and why. Requirements in Part F refer to these decisions by number. Where a decision has a reasonable alternative, the alternative is offered as an option in a compatibility profile (see area PRF in Part F).

**E.1 — Scope of presentation levels and how Level 1.5 is layered.**
The receiver implements Level 1.0 and Level 1.5 only. Level 1.5 is implemented as a separate pass. It takes the fully decoded Level 1 page, meaning the grid of cells with their attributes, and applies the packet X/26 character overwrites to it. The raw received page is never modified.
*Reason:* This is exactly how the reference SAA5246A receiver worked, with Level 1.5 done in microcontroller software over the stored page. It also keeps the Level 1 decoder simple and testable on its own. And it lets Level 2.5 be added later as a further pass without redesign.
*Amended 10 October 2026:* The receiver now implements Levels 1.0, 1.5, 2.5 and 3.5, chosen by the presentation level setting (decision E.40). The pass described above is the Level 1.5 pass and is unchanged. At Levels 2.5 and 3.5 the enhancement pass of decision E.42 runs in its place. That pass changes attributes as well as glyphs: the Level 1 spacing attributes and the non-spacing attributes of each affected row are resolved together, as Annex C.2 describes (decision E.47). The raw received page is still never modified.

**E.2 — Packet X/26 with designation code 15.**
By default, X/26/15 is processed like designation codes 0 to 14. The "SAA5246A" compatibility profile ignores it.
*Reason:* Clause 10.1 includes it in Level 1.5, but the standard also warns that many existing decoders ignore it, and the SAA5246A does not even store it. Processing it is the more correct default. Ignoring it reproduces period hardware.

**E.3 — Which row-address triplets set the active row.**
By default, the receiver follows Annex A.3: every triplet with an address from 40 to 63 sets the active row, whatever its mode. Address 40 means row 24, addresses 41 to 63 mean rows 1 to 23, and mode 00111 with address 63 means row 0. A "strict" option follows clause 12.3.2 instead. Under that option only modes 00001 (Full Row Colour), 00100 (Set Active Position) and 00111 (Address Display Row 0) set the row.
*Reason:* Annex A.3 is normative and describes how the Level 1.5 decoders that broadcasters tested against actually behave. The rule comes from EBU SPB 492 clause 14.6.4, the predecessor of EN 300 706. There, a row-address triplet of any mode sets the row for the character triplets that follow, which is how those decoders were built. Broadcasters were told to keep their data compatible with it; for example, Annex A.5 asks for Full Screen Colour to be sent with address 41. The two behaviours give the same result on correctly formed transmissions, so the choice only matters for malformed data.
*Amended 10 October 2026:* At Levels 2.5 and 3.5 the strict rule of clause 12.3.2 always applies, whatever this setting says (decision E.46). Object Invocation and Object Definition triplets carry object numbers in their address field, so the Annex A.3 rule cannot be used there. The Annex A.3 rule remains the default at Level 1.5 only.

**E.4 — Default character set designation at Level 1.5.**
The national option subset is always taken from the header bits C12, C13 and C14, following Annex E.9. The "group", which is bits 11 to 14 of the Table 32 value, is resolved in this order:
1. from packet X/28/0 format 1 of the page, if present and correctly decoded;
2. otherwise, from the latest packet M/29/0 of the page's magazine;
3. otherwise, from a configured default region, which is itself 0000 (Western European) if not configured.

Packets X/28/1 and M/29/1 are parsed only for diagnostic output and do not affect character sets.

The designation found this way also sets:
- the default G0 and G2 sets used by X/26 characters (clause 15.2);
- the second G0 set used by the ESC attribute (Table 33).

*Reason:*
- Table 25 permits X/28/0 and M/29/0 designation at Level 1.5.
- Using the group from them fixes the "ambiguous reference" that clause 15.2 describes, at no cost when they are absent.
- The codes carried by X/28/1 and M/29/1 are defined in EBU SPB 492 (1992), Figure 14, which is now available locally. They are still used only for diagnostics, for three reasons:
  - The newer X/28/0 and M/29/0 designation covers the same need.
  - One of the Latin codes (70) is contradicted by its own table in SPB 492.
  - EN 300 706 reuses the second triplet of X/28/1 for colour tables of redefinable characters.

  Only the first triplet of X/28/1 and M/29/1 shall be read. Table 25 allows single-region equipment to ignore this data.

*Amended 10 October 2026:* At Levels 2.5 and 3.5 the national option is taken from bits 8 to 10 of the highest-priority X/28 or M/29 designation when one is present (Annex E.9), and from C12 to C14 otherwise. The order of priority is that of Annex C.4 rule 1: X/28/0 format 1, X/28/4 (Level 3.5 only), M/29/0, M/29/4 (Level 3.5 only) (decision E.44). The first triplet of X/28/1 and M/29/1 remains for diagnostics only. Their other triplets carry the DRCS colour tables at Level 3.5 (decision E.54). The rule above is unchanged at Level 1.5.

**E.5 — How a Level 1.5 overwrite interacts with the Level 1 page.**
The standard is silent on the details (Part B.9). The receiver adopts the following rules:

- (a) All Level 1 attribute processing is done first, from the raw Level 1 bytes, exactly as if no X/26 existed. A spacing attribute in a cell that is later overwritten still has its full effect on the row, at its set-at or set-after timing.
- (b) The overwrite replaces only the glyph shown in the cell. The character keeps every display property that the Level 1 pass computed for that cell: foreground colour, background colour, flash, conceal, box membership and size. This follows the principle in clause 13.3.
- (c) The overwriting character is always drawn as an alphanumeric-style glyph from the character set the triplet names, even if the cell is in mosaic mode. Separated and contiguous mosaic modes do not apply to it.
- (d) Overwrites never change the held mosaic character. The held mosaic is computed from the Level 1 bytes only. If an overwrite lands on a cell that would show a held mosaic, the overwrite wins, because local enhancement data has higher priority than the Level 1 page (clause 13.2).
- (e) An overwrite at the top half of a double-height character is drawn double height, and its bottom half appears in the row below.
- (f) An overwrite addressed to a cell in the lower row of a double-height row is not displayed.
- (g) An overwrite is displayed whether or not the Level 1 byte at that cell passed its parity check. Broadcasters may deliberately send bad parity there (Table 25).
- (h) If two overwrites address the same cell, the later one in transmission order wins.
- (i) Overwrites in row 0 are applied when the page is displayed, but not to headers shown in the rolling header during a search (Annex E.3).
- (j) Overwrites addressed to row 0, columns 0 to 7, are always discarded, because that area belongs to the receiver (REQ-SEC-005).

*Reason:* Rules (a), (b) and (e) follow the only relevant statements in the standard. The others are the simplest choices consistent with it. Rule (f) matches the behaviour the standard itself attributes to existing Level 1.5 decoders (Table 26 notes 3 and 6).
*Amended 10 October 2026:* The rules above are unchanged at Level 1.5. At Levels 2.5 and 3.5 these rules change:
- (a) and (b): the Level 1 spacing attributes and the non-spacing attributes are resolved together, row by row, under Annex C.2, so enhancement data changes attributes as well as glyphs (decisions E.42 and E.47).
- (c): a G1 character written by X/26 is a mosaic, and G0 and G2 characters can be underlined (decision E.50).
- (f): enhancement characters written to the lower row of a double-height row are displayed, in the cells not covered by lower halves (Table 26 codes 0/D and 0/F; decision E.15, as amended).
- (h): the later of two writes to the same cell wins only within one display priority; between priorities, clause 13.2 decides (decision E.51).

Rules (d), (e), (g), (i) and (j) apply unchanged at every level.

**E.6 — Where a page ends.**
The receiver follows clause 7.2.1 and Annexes B.3 and B.4:
- In serial mode (C11 = 1), the next header of any magazine ends the current page.
- In parallel mode (C11 = 0), only the next header of the same magazine ends it.

The simplified wording in Table 2 is not used.
*Reason:* Clause 7.2.1 is the precise definition, and two annexes agree with it.

**E.7 — Uncorrectable Hamming 8/4 errors.**
- If either packet address byte (byte 4 or 5) is uncorrectable, the whole packet is discarded, because its magazine and row are unknown.
- If a header's magazine is known but any of bytes 6 to 13 is uncorrectable, the header is used to **end** the current page, as the termination rules require. It does **not** start the reception of a new page, and its displayable bytes are not used for the rolling header.
- In a Fastext link, an uncorrectable byte makes that single link invalid (treated as "no link").
- In packet 8/30, an uncorrectable byte makes the field it belongs to invalid.

*Reason:* The standard says only "reject data bits". These rules never act on an address that might be wrong, and never lose a page ending.

**E.8 — Characters that fail their parity check.**
Every stored character carries a flag saying whether it passed its parity check. By default:
- A new character that fails its check does not replace a character that passed its check in an earlier reception of the same page and sub-page, unless the page was erased in between (C4 set, or a new sub-code).
- A cell that has never held a character that passed its check is displayed as a space.

The two alternative policies are "always show a space" and "show a visible error marker", the latter for diagnostics.
*Reason:* The standard is silent. Keeping the last good character is how a receiver builds a clean page out of several noisy retransmissions, which is common on weak signals and on recovered video recordings.

**E.9 — Uncorrectable Hamming 24/18 errors.**
A triplet that fails its Hamming 24/18 check is ignored completely. It neither moves the active position nor writes anything. A single-error position greater than 24 is treated as uncorrectable. An uncorrectable designation code makes the whole X/26 packet unusable.
*Reason:* The standard is silent. Ignoring a single triplet loses at most one character, which is the least harmful result.

**E.10 — Columns 0 to 7 of row 0.**
Following the SAA5246A:
- Columns 0 to 6 are a status area written by the receiver. By default it shows the page number being requested or displayed, for example "P100".
- Column 7 is an alphanumeric colour attribute generated by the receiver. It is white when the displayed page has been found, and green while the requested page is being searched for.

The header's own bytes 6 to 13 are never displayed. The status area content is configurable.
*Reason:* The standard leaves these columns to the decoder (Annex E.7). The SAA5246A convention is the most widely seen one.

**E.11 — Rolling header and clock.**
While a requested page has not yet been received, row 0 columns 8 to 31 show the displayable text of incoming headers:
- In serial mode, from every magazine.
- In parallel mode, from the requested magazine only.
- Headers with C9 set, and headers rejected under decision E.7, are excluded.

Columns 32 to 39, the clock, are updated from every valid header of any magazine, whether or not the page has been found. This matches the SAA5246A's "always rolling" clock. Rolling can be disabled.
*Reason:* This follows Annexes B.3 and B.4 and the SAA5246A.

**E.12 — Flash timing.**
Flashing characters are shown for 48 fields and hidden for 16, as the real SAA5050 does. That is 960 milliseconds shown and 320 milliseconds hidden at 50 fields per second, which the datasheet describes as 0.75 hertz with a 3 to 1 ratio (Part C.8). When time is supplied in milliseconds rather than fields, those durations are used. The timing is configurable. In the hidden phase the character's foreground pixels are drawn in the background colour.
*Reason:* The standard gives no rate. The SAA5050 is the rendering baseline.
*Amended 10 October 2026:* Unchanged at Levels 1 and 1.5. At Levels 2.5 and 3.5, Table 29 defines a slow rate of 1 hertz and a fast rate of 2 hertz, and flash follows decision E.48, which keeps the 3 to 1 ratio of this decision within a one-second cycle.

**E.13 — Character rounding rule.**
The following rule is used for the 12-by-20 rendering mode. The SAA5050 datasheet describes rounding only in words (Part C.4). This exact rule is the one used by the MAME emulator (its SAA5050 device, source lines 433 to 436), and jsbeeb uses the same expression. beebjit, Bedstead and the published analysis of a decapsulated SAA5050 are equivalent to it. It is therefore confirmed, not assumed.

Each 5-dot glyph row is first doubled to 10 half-dots. Each glyph row then produces two output lines:
- **The upper line** of the pair takes the row above as its neighbour B.
- **The lower line** takes the row below as its neighbour B.
- **Interlaced output:** these are the first field's and the second field's lines respectively.

For the row being drawn (A) and its neighbour (B), a half-dot is added wherever all three of these hold:
- A has a set dot;
- B has a set dot diagonally adjacent to it;
- the dot orthogonally between them in B is clear.

In bit-mask form, working on half-dot rows:

    A | ((A >> 1) & B & ~(B >> 1)) | ((A << 1) & B & ~(B << 1))

Further rules:
- **No rounding across the cell edge.** A neighbour row outside the 9-row glyph counts as empty.
- **Scope.** Rounding applies to alphanumeric glyphs, including blast-through capitals in mosaic mode, and never to mosaics.
- **Double height.** Each line of the rounded 12-by-20 glyph is repeated twice, which is what all four emulators do.

The datasheet's wording ("referenced alternately up and down changing every line") could also be read as alternating the neighbour on every output line in double height. That difference is only visible on double-height diagonals. It shall be checked against photographs of a real SAA5050 (REQ-TST-030), and the rule changed if they disagree.

**E.14 — Mosaic geometry.**
In a cell of W half-dots by H lines, the geometry is as follows.

*Contiguous mosaics:*
- The two block columns are each W/2 wide.
- The three block rows are 3/10, 4/10 and 3/10 of H, taken from the SAA5050 drawing. That is 6, 8 and 6 lines in the 20-line mode, and 3, 4 and 3 lines in the 10-line mode.

*Separated mosaics:*
- Each block loses its leftmost dot column and its bottom line, drawn in background colour.
- In the 6-dot by 10-line SAA5050 cell, that is dot columns 0 and 3 and lines 2, 6 and 9.
- In the 12-by-20 cell, it is half-dot columns 0, 1, 6 and 7 and lines 4, 5, 12, 13, 18 and 19.
- The 3/4/3 block heights and 3/3 block widths are also confirmed.
- This agrees with the MAME, jsbeeb and beebjit emulators and with the decapsulated-chip analysis (Part C.6).

**E.15 — The row below a double-height row.**
The transmitted content of that row is ignored entirely.
- Each cell below a double-height character shows the character's bottom half.
- Each cell below a normal-height character shows that character's background colour with no foreground.
- Double height in rows 0, 23 and 24 is treated as normal height.

*Reason:* Table 26 and Annex C.3. The SAA5050 system behaves the same way. Annex C.3 lets the decoder choose for illegal rows, and normal height is the safest choice.
*Amended 10 October 2026:* Unchanged at Levels 1 and 1.5. At Levels 2.5 and 3.5 (Table 26 codes 0/D and 0/F, Table 29 mode 01100 and Annex C.3):
- enhancement characters written to the lower row, by local X/26 data or by objects, are displayed in the cells not covered by lower halves;
- a row whose only double height or double size comes from non-spacing attributes does not suppress the row below;
- double width and double size follow Annex C.3 (REQ-L25-025 and REQ-L25-026).

Double height in rows 0, 23 and 24 is still treated as normal height.

**E.16 — Hold mosaics details.**
The receiver implements the Table 26 text exactly, including the rule that "The 'Held-Mosaic' character is always displayed in its original contiguous or separated form regardless of the mode prevailing at the time of substitution."

When a set-after attribute changes alphanumeric or mosaic mode, or changes size:
- the attribute's own cell is displayed with the state before the change, so it can still show the held mosaic;
- the held mosaic is reset to space immediately after that cell.

When a set-at attribute changes size (Normal Size), the reset takes effect before its own cell is displayed.
*Reason:* This is consistent with the worked example in Annex G.3.3 and with set-at and set-after timing. The decapsulated-chip analysis confirms that the real SAA5050 also shows the held mosaic in its stored contiguous or separated form, and that Hold Mosaics is set-at and Release Mosaics set-after. The 1976 Broadcast Teletext Specification (clause 3.1.7) already defined the held character the same way: the most recent character with bit 6 set, kept in its original form, reset by a change between alphanumeric and graphics or by a change of height.

**E.17 — Start Box.**
A box starts only when two Start Box attributes appear in consecutive cells, and it starts at the second of them. End Box ends a box from the next cell. An option allows a single Start Box to start a box from the next cell.
*Reason:*
- The standard says that "protection against false operation is provided by double transmission of Start Box control characters, with the action taking place between them".
- The widely used decoders zvbi and vhs-teletext both require the pair.
- On noisy signals, a single corrupted byte could otherwise make part of a subtitle or newsflash page appear.
- The worked example in Annex G.3.1 (`x 0B 0B 61 0A 0A`, with the second 0B, the "a" and the first 0A boxed) gives the same result under either rule.

**E.18 — Codes with no Level 1 function.**
By default, codes 0/0 and 1/0 are alpha black and mosaic black: set-after attributes like the other colour codes, which select alphanumeric or mosaic mode and a black foreground. Codes 0/E (double width) and 0/F (double size) are displayed as spaces and change no state. The "SAA5050" profile displays 0/0 and 1/0 as spaces with no effect, as that chip does.
*Reason:* Table 26 gives these codes no response at Levels 1 and 1.5, but current services transmit Level 1 pages that depend on them. A comparison with zvbi on off-air recordings (REQ-TST-021) found them on Italian RAI pages from 2021 (page 517 writes "SETTIMANA DAL 24/01 AL 30/01" after alpha black, in a row that starts in mosaic mode; pages 509 to 513 draw mosaics after mosaic black) and on a Dutch regional service from 2020 (page 100). Treating them as having no function turned that text into mosaics and those mosaics into letters. zvbi and the SAA5246A decode them, and decision E.38 gives recent broadcasts priority.
*Amended 10 October 2026:* previously the default ignored 0/0 and 1/0, and only the "SAA5246A" profile decoded them.

**E.19 — When a page is complete.**
The receiver shows rows as they arrive, as hardware decoders do. A page is complete when one of these happens:
- its terminating header is received (decision E.6);
- 100 milliseconds pass with no packet of the page.

Completion raises an event.
*Reason:* Annex B.9 allows a software decoder to use this 100 millisecond limit.

**E.20 — Sub-pages.**
The page store keeps each sub-page separately, keyed by its full sub-code. A request can name:
- a specific sub-code;
- "any sub-page", which shows the most recently received one;
- a digit-masked pattern, where each of the four sub-code digits may be marked "don't care", as the SAA5246A allows.

The viewer can step through the stored sub-pages of a page.

**E.21 — Bit order of C12, C13 and C14.**
The national option index is 4×C12 + 2×C13 + C14, as the Table 32 column heading and its note 1 give. C12 is data bit 2 of the header byte 13, C13 is bit 3 and C14 is bit 4, so the decoded 4-bit value of that byte must be bit-reversed within those three bits to form the index.

**Confirmed by experiment on 1 October 2026.** Page headers were decoded from two off-air digital television recordings in the FFmpeg sample archive (REQ-TST-022), where the two possible orders predict different languages:

| Page | Header bits (C12, C13, C14) | This rule gives | The reversed order would give |
|---|---|---|---|
| France 2, subtitle page 888 | 1, 0, 0 | index 4, **French** (correct) | index 1, German |
| An Italian channel whose descriptor lists Italian subtitles, subtitle page 777 | 0, 1, 1 | index 3, **Italian** (correct) | index 6, Czech and Slovak |

The reversed reading gives the wrong language in both cases, so it can be ruled out.

Because this ordering is easy to get wrong, it is pinned down by tests (REQ-TST-012), using these same recordings. In the page-file format "TTI", the page status field puts C12 in bit 7, C13 in bit 8 and C14 in bit 9, following the most widely used implementation.

**E.22 — Newsflash and subtitle pages.**
When a displayed page has C5 or C6 set, the receiver automatically switches to boxed display: only boxed areas are drawn, and the rest of the screen is "picture".
- When rendering to an image with no video underneath, "picture" is rendered as transparent pixels (alpha zero), or as a configurable fill colour if the output format has no transparency.
- When rendering on top of supplied video, the video shows through.

*Amended 10 October 2026:* At Levels 2.5 and 3.5, the boxing bit of an X/26 Display Attributes triplet also starts a boxed area on these pages. On other pages it starts a window, inside which the full row colour becomes transparent. Which of the foreground or background colour, the row or screen colour, and the picture each pixel shows follows Table C.1 (decision E.47 and REQ-L25-029).

**E.23 — Row 24.**
By default, row 24 is displayed only when the page carries packet X/27/0 with the "display row 24" flag set (Annex H). Pages whose links come from the TTI Link Extension have their row 24 drawn by the receiver instead (decision E.36, REQ-NAV-011). The "SAA5246A" profile shows row 24 whenever the page carries packet X/24, like that chip's automatic Fastext row.

**E.24 — Framing code tolerance in the software data slicer.**
By default, a framing code that differs from 0x27 in one bit is accepted, but only if the two packet address bytes that follow both decode without an uncorrectable error. A "strict" option requires an exact framing code.
*Reason:* Annex P records that existing decoders accept a one-bit framing error. Requiring valid Hamming in the address bytes guards against the false detections the same annex warns about.

**E.25 — Glyph source.**
**During development**, the glyphs are temporarily taken from the "Bedstead" font by Ben Harris (version 3.261). They are exported to `third_party/bedstead/bedstead-glyphs.txt`, and their provenance is recorded in `third_party/bedstead/PROVENANCE.md`.

- **What Bedstead is.** Bedstead's own work is dedicated to the public domain under CC0 1.0. However, many of its glyphs reproduce the SAA5050 typeface designs, taken from the datasheet code tables.
- **Why that is permitted.** Copyright in those designs still exists. Using them as a typeface is permitted in the United Kingdom by section 55 of the Copyright, Designs and Patents Act 1988, because the SAA5050 was first marketed more than 25 years ago.
- **Debian's view.** Debian classes Bedstead as "non-free" for exactly this reason.

**For release**, the glyphs shall be drawn originally for this project, and Bedstead shall be removed. The provenance of every glyph set is recorded. No bitmap is copied from any character generator ROM.
*Reason:* Legal basis (see "How to read this document"). A complete, well-tested glyph set lets development proceed now; original glyphs remove the remaining legal doubt before release.
*Amended 12 October 2026 by decision E.72:* Bedstead is the bundled font for release too, and the original glyphs are removed. The owner accepts the legal position described above.

**E.26 — Composition of diacritical marks.**
When rendering pixels:
- If the glyph set contains a precomposed glyph for the base letter plus mark, that glyph is used.
- Otherwise, the mark's glyph is overlaid on the base glyph.

When producing text, the base letter and the Unicode combining character for the mark are emitted, normalised to the precomposed form (Unicode Normalization Form C) where one exists.

**E.27 — Scope exclusions decided here.** The following are out of scope (see Part G):
- 525-line teletext (ITU-R BT.653 System B for 525 lines);
- the Table Of Pages navigation scheme;
- Programme Delivery Control decoding;
- Level 2.5 and 3.5.

*Amended 10 October 2026:* Levels 2.5 and 3.5 are no longer excluded; the owner brought them into Phase 1 (decision E.40). Only a layout algorithm for proportional spacing remains excluded, because the standard defines none (decision E.50).

**E.28 — Where real teletext data comes from, and what the receiver must read.**
A search on 1 October 2026 found that no television channel publishes its live teletext on the open internet as raw data. Real teletext data is available only in the following forms, and the input requirements are built around them.

1. **Digital television transport streams.** European broadcasters that still run teletext carry it inside their digital broadcasts as DVB teletext (EN 300 472). Recordings of these broadcasts are the most common form of real, raw teletext data.
   - The FFmpeg project's public sample archive contains off-air recordings with teletext tracks from Italian, French and English-language channels. They include a file that uses the 192-byte packet variant.
   - Some recordings list several languages in one teletext descriptor.
   - Requirements REQ-INP-006 to REQ-INP-008 cover this format.
2. **Broadcasters' own internet streams carry no teletext.** The live web stream of tagesschau24 (ARD) contained only video and audio when examined. The Das Erste and ZDF streams refused access from outside their countries. A receiver cannot rely on broadcasters' web streams for teletext.
3. **T42 archives recovered from recordings.** The Teletext Archaeologist archive holds over 2000 recovered service snapshots. REQ-INP-002 covers this format.
4. **A live feed of services over WebSockets.** The zxnet online teletext viewer receives live services from servers over the WebSocket protocol:
   - hobby services, including Teefax and a recreation of Ceefax;
   - a replay of a capture of Channel 4 teletext.

   Each message carries the 40 data bytes of a packet exactly as transmitted, with parity and Hamming coding intact, but the messages are wrapped in a request-and-response protocol of JSON messages. This protocol is not formally documented. It was established by reading the viewer's public JavaScript and by connecting to the servers, so it may change without notice. REQ-INP-022 covers it and requires the reader to be tolerant of unknown messages.
5. **Most broadcaster page interfaces are not raw data.** Most publish rendered pages:
   - ZDF, ARD, ORF, SVT, WDR, NDR and others publish HTML;
   - RAI Televideo publishes PNG images;
   - the Finnish broadcaster YLE publishes XML behind an API key.

   ZDF's site also runs a bot check, so it must not be read automatically. These interfaces are not supported.

Live raw teletext from a real channel therefore needs a digital television receiver tuned to a European broadcast that still carries teletext. Its output is the transport stream format of item 1.

*Amended on 10 October 2026* (research record: `research/answers-teletext-sources.md`). Two broadcasters do publish live pages with their teletext bytes intact, although as whole pages rather than packet streams:
- **SRG SSR** (SRF, RTS and RSI): the public site `teletext.ch` loads each page from `api.teletext.ch` as JSON. The page is held in EP1 format: a header, 23 rows of 40 bytes, and a per-page teletext level field.
- **NOS**: `teletekst-data.nos.nl/page/<page>` returns navigation lines followed by 25 rows of 40 bytes of Level 1 codes. NOS has already converted accented letters to ISO 8859-1, so they are not transmitted codes.

Item 5's earlier statement about NOS was wrong for this address. No broadcaster was found publishing a live packet stream. The newest real broadcast packets found are off-air recordings of TVP (Poland, November 2023) and NPO (the Netherlands, June 2020). The zxnet feed named `ch4` is a real-time replay of a Channel 4 broadcast of 15 July 2007.

**E.29 — How pixels are produced, and what earlier designs teach.**
Producing pixels portably is a solved problem, and the rendering design copies the solutions.

- **The SAA5050 itself.** It never held a picture in memory. For each television line it took a character code and the line number within the character cell, and produced one row of dots plus a foreground and background colour, in step with the scanning beam. The natural unit of teletext drawing is therefore **one cell, one scanline**.
- **zvbi.** Its page-drawing function writes a page region into memory supplied by the caller, in a pixel format and row stride the caller chooses. The caller owns memory and pixel format; the library only writes pixels.
- **Embedded graphics libraries.** They show how to fit small memories:
  - LVGL draws into a small partial buffer and hands each finished area to one port callback.
  - u8g2 draws a screen in horizontal strips.
  - The Rust `embedded-graphics` crate draws to anything that implements one drawing trait, which hundreds of display drivers do.
- **Doom**, the best-known example of software that runs on everything:
  - **A small platform layer.** It renders into a fixed-size 8-bit indexed framebuffer, and a port implements only a handful of platform functions. The "doomgeneric" project reduced these to six callbacks: initialise, draw frame, get time, sleep, get key and set window title.
  - **Colour at the edge.** The engine never handles real colours; the palette is applied only at output.
  - **Palette effects.** Screen-wide effects, such as the red damage flash, are done by changing the palette, not by redrawing.
  - **Scaling at the edge.** The engine always renders at one resolution and ports scale it. Ports that forgot Doom's pixels are 1.2 times taller than wide showed squashed pictures, which is the same 1.2 aspect factor as teletext (REQ-RND-004).
  - **Integer arithmetic only**, and byte order handled explicitly.
  - **Display decoupled from simulation**, with deterministic replay of recorded input.

The decisions taken from this are:

1. The core never produces real colours. It produces **palette indices**, and colour conversion happens only at the edge, in optional helpers or in the port (REQ-RND-018 and REQ-RND-023).
2. All teletext-specific drawing lives in one pure function that draws one line of one cell (REQ-RND-016). Every renderer and every port uses it, which makes the output identical everywhere.
3. **Once a page is decoded, every text row can be rendered on its own.** Every teletext feature that crosses rows is resolved while the page is decoded into the cell grid, before any pixels are drawn:
   - Double height suppresses the row below and places bottom halves there.
   - Level 1.5 overwrites address any position.

   Everything that remains is local to one row, or to one cell within it:
   - attributes, hold mosaics, conceal and boxes all reset at each row;
   - character rounding looks only at glyph lines within one cell;
   - flash and reveal depend only on time;
   - the row 0 status area is row-local;
   - page zoom is a re-mapping of rows, handled like double height.

   The only cross-row effect left is change tracking: a change to double height in one row also changes the row below (REQ-RND-019).

   So the library's whole pixel output is **"render text row r (or screen line y) into this buffer"**. The port asks for pixels; the library never pushes them. This is the same model as zvbi's page drawing (REQ-RND-017). A port's whole job is defined by a platform checklist in the style of doomgeneric (REQ-ARCH-009).
4. **Buffering belongs to the port.** Whether the port keeps a whole-screen framebuffer as Doom does, a ring of two or three row buffers, or a single row buffer, and when it renders relative to its hardware, is the port's decision. A port with a framebuffer just renders each changed row at the right offset in its own memory. The library offers a convenience loop for this (REQ-RND-022), but does not require it. An optional precomputed glyph line cache (REQ-RND-026) reduces drawing cost for ports that redraw often.
5. Flash and reveal are done by **changing the palette**, not by redrawing (REQ-RND-023).
6. The core never scales. Scaling and aspect correction happen in the output stage, with a helper so ports get the 1.2 aspect factor right (REQ-RND-024).
7. Decoding and drawing are decoupled. Drawing happens only when a row has changed or the flash state changes (REQ-RND-019).
8. **The library stops at the row renderer.** It writes pixels into buffers supplied by the target port's hardware layer, and does nothing else with them. It does not put pixels on a screen, generate video signals, keep to display timing, or contain code for any specific hardware. Those are the port's responsibility, reached only through the platform interface (REQ-ARCH-009 and REQ-ARCH-010).

*Amended 10 October 2026:* At Levels 2.5 and 3.5 (decisions E.42, E.49 and E.55):
- **Item 3.** Objects, row colours, side panels and the row extension of the Font Style triplet are further features that cross rows. They are all resolved while the grid is built, so rows stay independent. A row is 40 cells plus the side-panel widths. The patterns and colour tables of the DRCS characters a page uses travel with the grid, so a row still renders from the grid and the viewer state alone.
- **Item 5.** Flash and reveal are resolved at render time in the resolved colour mode, instead of by palette changes. The change tracker limits the redrawing to rows that flash or conceal. The palette technique remains for Levels 1 and 1.5.

**E.30 — The web-native design, and what the jellytext project teaches.**
neoteletext shall read pages from ordinary web servers as well as from broadcast data. It does so over:
- HTTP;
- WebSockets;
- server push;
- cookies.

An earlier project in the same workspace, jellytext (August 2025, suspended), sketched this architecture:

```
web server → proxy (HTTP, sessions, cookies, TTI cache, TTI-to-T42 encoder) → packet stream → decoder → renderer
```

The decisions taken from it are listed below.

1. **Teletext's own formats all the way to the decoder.** A web source delivers TTI pages or T42 packets, and nothing else. Web pages enter the decoder exactly as page files and broadcast packets do (REQ-ARCH-004), so there is one decoder and one set of tests. All web-specific complexity stays in the web module (area WEB).
2. **Stateless servers, with the session in a versioned cookie.** jellytext's notes record the idea to "store session data in cookie with versioning". A neoteletext client treats cookies as opaque and only stores and returns them. The versioning is a server design rule, given as guidance for page servers (REQ-WEB-012).
3. **Asynchronous throughout.** The jellytext notes say the "application can be fully async by using callbacks". Every network operation shall be non-blocking and event-driven. The decoder is fed bytes as they arrive (REQ-INP-001).
4. **Interactive forms drawn as teletext pages.** jellytext's TTI Form System adds form commands after ordinary TTI rows. Decoders that do not know the commands ignore them and still show a readable page. neoteletext adopts it, corrected as decision E.31 describes.
5. **The transport belongs to the port.** Network sockets and encryption are supplied by the port through the platform interface (REQ-ARCH-009). This keeps the library free of any particular network stack, and is consistent with REQ-ARCH-010. jellytext planned to embed a network stack and an encryption library on each microcontroller, which is heavy.
6. **One platform at a time.** jellytext decided to "write for 3 platforms at once" (composite video from an ESP32, HDMI from an RP2350, and a video stream to YouTube) shortly before it was suspended. neoteletext keeps all platforms outside the library (REQ-ARCH-010), and streaming rendered pages as video is not part of it.
7. **Geometry settled once.** jellytext's notes changed their minds several times about pixel sizes. neoteletext settles this in REQ-RND-002 to REQ-RND-004 and REQ-RND-024.

**E.31 — Corrections made in adopting the TTI Form System.**
jellytext's TTI Form System version 2.0 is kept unchanged in `research/jellytext-tti-form-system-spec-v2.md`. Version 3, in Appendix C, is normative for neoteletext. It keeps the core ideas:
- the form is drawn as ordinary TTI rows;
- field definitions follow the row they belong to;
- unknown commands are ignored by older decoders;
- submission is by HTTP, followed by a success or error page.

It makes these corrections:

1. **Field positions are stated, not guessed.** Version 2.0 found fields by scanning rows for runs of underscores and brackets. That fails on real teletext for two reasons.
   - The underscore (5/F) and square brackets (5/B and 5/D) are **national option positions**. With the English option, which is the SAA5050 default, 5/F is drawn as "#" and 5/B and 5/D as arrows. So `_____` appears as `#####` and `[ ]` as `← →`. The Greek set likewise replaces "<" and ">" with guillemets.
   - A run of underscores inside ordinary text is indistinguishable from a field.

   In version 3, every field definition states its column and width. The drawn placeholder characters are purely visual, and the parser never relies on them. Authoring tools may still find fields by pattern and write the columns in.
2. **No credentials in pages.** Version 2.0's `FH` command could put headers such as `Authorization: Bearer …` into page content that anyone can download. Version 3 withdraws `FH`. Authentication comes only from the session cookie, which the server sets and the client returns.
3. **Protection against forged submissions.** A new `FT` command carries a server-issued form token, bound to the session. The client returns it with the submission (REQ-WEB-010).
4. **An input method.** Version 2.0 assumed a keyboard. A teletext viewer may only have a remote control: digits, four colour keys and a few others. Version 3 defines how fields are edited from both a keyboard and a remote control (REQ-NAV-009).
5. **Characters the page can show.** A value entered in a field is drawn in the page's own G0 character set. Characters the page cannot show are refused at entry.
6. **Where a form may submit to.** A form may submit only to the same origin (scheme, host and port) as the page that contains it, unless the service descriptor lists other permitted origins (REQ-WEB-008).

**E.32 — Security overrides every other requirement; plain HTTP is allowed.**
Security is the overriding requirement of this project. Every requirement in area SEC takes precedence over any requirement it conflicts with. Any such conflict is resolved in favour of security and recorded as a decision in this part.

The project owner decided on 1 October 2026 that **plain HTTP is allowed in full**. Plain HTTP may be used for reading pages, and also for cookies, sessions and form submissions. This is the only place where a security recommendation was deliberately not followed, so the residual risk is recorded here:

- **What an attacker on the network path can do.** They can read and change pages, session cookies and submitted form values on plain-HTTP services.
- **How the risk is reduced:**
  - the receiver-owned security indicator (REQ-SEC-005);
  - a warning and confirmation before every form submission over plain HTTP (REQ-SEC-008);
  - `Secure` cookies never sent over plain HTTP;
  - no downgrade of a service first reached over HTTPS (REQ-SEC-007).
- **What remains.** The user is told, but not prevented. Services that need protection should use HTTPS.

**E.33 — How the software is sandboxed.**
Sandboxing is layered, and each layer assumes the one inside it might fail:

1. **No authority in the library.** The library crates have no access to files, network, processes, environment or clock (REQ-SEC-016). Even a logic bug in the decoder cannot open a file or a socket, because the code to do so does not exist in it.
2. **No executable content.** Pages, forms, descriptors and pushed messages are data. Nothing received is ever run, interpreted as a script or loaded as code (REQ-SEC-004).
3. **Memory safety** inside the process: `forbid(unsafe_code)`, freedom from panics, Miri and fuzzing (REQ-GEN-006 and REQ-GEN-008).
4. **Bounded resources:**
   - fixed memory;
   - bounded work per packet and per message;
   - a fair share of processing time for each input source (REQ-SEC-012).
5. **An operating-system sandbox** for the command-line program and hosted ports. They restrict themselves after opening their inputs and outputs (REQ-SEC-017).
6. **Privilege separation for web mode.** A fetcher process holds the network rights, and a decoder process holds none. They are joined only by a stream of T42 or TTI bytes, which every other part of the design already uses (REQ-SEC-018).
7. **WebAssembly as a portable sandbox** for any host, including C programs, as an alternative to the unsafe C interface (REQ-SEC-019).
8. **Hardware isolation** on microcontrollers with a memory protection unit, as guidance for ports (REQ-SEC-021).
9. **Sandboxed test tools.** Third-party tools that parse untrusted corpus files run with no network access and read-only inputs (REQ-SEC-022).

**E.34 — A deliberately minimal browser.**
With web-native sources, neoteletext is a browser for a teletext web. Like any browser, it fetches, caches, keeps cookies, follows links, submits forms, applies an origin security model, sandboxes and renders. It can stay very small because teletext removes almost everything that makes browsers large:
- no layout engine (the page is a fixed 40 by 25 grid);
- no scripting (REQ-SEC-004);
- no font shaping;
- no images or media;
- pages of about one kilobyte.

Three earlier systems teach how to keep it that way:

- **Viewdata, Prestel and Minitel** were teletext's interactive relatives. They had character-cell pages, navigation and forms over a network on cheap terminals. They proved the product shape at national scale without anything resembling a modern browser.
- **Gemini**, a modern minimal alternative to the web, writes down what it refuses (no scripting, no cookies, no complex markup) so that features cannot creep back in.
- **WAP and WML**, the early mobile web, show the opposite. It grew scripting and complex page decks, became half a browser, and failed under its own complexity.

The decisions taken are:
1. **One written profile.** neoteletext's web behaviour is defined as one written "minimal user agent" profile (REQ-WEB-014). It lists what is supported and, as firmly, what is permanently excluded.
2. **One security unit.** The origin is the single security unit, defined once (REQ-WEB-015), as browsers do.
3. **A strict URL parser.** URL parsing, a classic source of browser bugs, is done by one small, strict, fuzzed parser (REQ-WEB-016). It prefers rejecting unusual addresses to being compatible with every form the web allows.
4. **History.** Back and forward history (REQ-NAV-010) is the one basic browser function the teletext model lacked.

*Amended 12 October 2026 by decision E.73:* the profile handles the `Link` header, and scans the first 8,192 bytes of an HTML answer for pointers, during discovery only (REQ-WEB-028, REQ-WEB-029).

**E.35 — Cookies follow current practice, not the 2011 standard.**
The cookie specification of 2011, RFC 6265, is out of date. Its replacement, RFC 6265bis, remains an IETF draft; the latest version checked is dated 30 September 2026. The major browsers already implement it. Newer practice has been added on top since:
- the `__Http-` and `__Host-Http-` name prefixes, shipped in Chrome 140 and Firefox 143 in 2025;
- Device Bound Session Credentials, a W3C design generally available in Chrome on Windows from 2026, which binds a session to a key that cannot leave the device, so stolen cookies quickly become useless;
- partitioned cookies ("CHIPS").

Third-party cookies remain enabled by default in Chrome after Google reversed its plan to remove them in April 2025. Safari, Firefox and Brave block them by default.

neoteletext adopts:
- **RFC 6265bis in full**, as REQ-WEB-006 lists: size limits, the 400-day cap, rejection of control characters, name prefixes, SameSite with `Lax` as the default, and the rule that plain HTTP cannot touch `Secure` cookies.
- **The new prefixes.**
- **Rules stricter than any browser** wherever its model allows: host-only cookies, and no third-party context at all.
- **Device Bound Session Credentials as a Phase 2 requirement** (REQ-WEB-017), where the port can keep a key that cannot leave the device.

Because RFC 6265bis is still a draft, REQ-WEB-006 shall be checked against the published RFC when it appears, and at each milestone's security review (REQ-SEC-023).

**E.36 — Links are Fastext: four coloured links per page, drawn by the receiver.**
neoteletext's only hyperlinks are the Fastext coloured keys.
- **Up to four links.** Each page may carry at most four links, one per coloured key: red, green, yellow and cyan.
- **Index.** The standard Index key is available as well. It goes to the service's index page, and is not a content link.
- **Typing a page number** remains ordinary teletext navigation, not a hyperlink.

No other link mechanism exists. The alternatives that were considered and rejected are:
- links marked inside the page text;
- link markers drawn by the page, such as `>>`, `[]` or `LINK<>`;
- whole-row links;
- numbered link hints;
- further sets of links beyond four.

*Reason:*
- **Familiar.** Fastext is the link model teletext viewers already know.
- **Remote-friendly.** It needs only the coloured keys, with no cursor and no mode.
- **Compatible.** It is the link model a real Fastext television of the late 1980s supports, so a page broadcast through the page encoder keeps working links on old sets (REQ-INP-019).
- **A useful limit.** Four links keep pages simple.
- **Trustworthy:**
  - **Markers.** Any marker a page draws, a malicious page can draw where there is no link. Bracket characters are also national option positions, which show as arrows or accented letters in some languages (decision E.31).
  - **Prompts.** Instead, **the receiver draws row 24 itself** from the link data, as Table Of Pages decoders drew their own prompt row. The page supplies only targets and short labels. Each prompt is therefore always shown whole, in a fixed style the page cannot alter, and links that leave the service are always marked as such.
  - **Destinations.** The first use of a link to another service needs confirmation.

Pages received from broadcasts keep their transmitted row 24 (decision E.23). Their links can only lead to pages in the same service, so the destination is always fully determined by the packet X/27/0.

*Amended 12 October 2026 by decision E.64:* links may also lead to addresses, and a page may link page numbers (`LK`) and runs of text (`LT`) as well as its coloured keys. The four coloured keys, the receiver-drawn row 24 and the ban on page-drawn link markers are unchanged. In the browser, a link to another service goes through the tuning screen (decision E.65) instead of the confirmation in row 24 of REQ-NAV-012.

**E.37 — Terminal output: exact pixels where the terminal can show them, characters everywhere else.**
A terminal can show a teletext page in two ways:
- **As characters**, drawn by the terminal's own font with colour escape sequences (REQ-RND-009).
- **As pixels**, rendered by the library and sent through a terminal graphics protocol (REQ-RND-029).

Character output cannot be exact:
- the shapes depend on the user's font, so SAA5050 rounding (decision E.13) is lost;
- the separated mosaic characters (REQ-CHR-005) were added only in Unicode 16 and are missing from most fonts;
- few terminals support the double-height line escape sequences;
- many terminals ignore the blink attribute.

The pixel output of the row renderer (REQ-RND-017) is exact by construction, and the same on every port (REQ-RND-015).

The receiver therefore supports both, as separate output adapters (`ARCHITECTURE.md` layer L5). When the terminal offers one, a graphics protocol is used; otherwise the characters are used. The user can always force either. Three protocols are supported, in this order of preference:

1. **The kitty graphics protocol**, supported by kitty, Ghostty, WezTerm and Konsole. It accepts compressed images, places them by terminal cell, and keeps transmitted images so that the terminal can switch between them. Both flash phases can therefore be sent once and swapped.
2. **Sixel**, supported by xterm, foot, WezTerm, Windows Terminal, iTerm2 and mlterm. It has the widest support. Its colours are numbered registers, which match the palette indices of REQ-RND-023 directly.
3. **The iTerm2 inline image protocol**, supported by iTerm2, WezTerm and Visual Studio Code. It is a fallback for terminals with neither of the others.

The alternative of shipping a teletext font for the terminal to use was rejected:
- it looks right only where the user has installed it;
- it still cannot show double height or exact rounding;
- glyphs in the Private Use Area would tie the text output to that one font.

*Reason:*
- **No new capability in the core.** Both forms are callers of the existing outputs: the cell grid for characters, and the row renderer for pixels. The core does not change, which confirms the boundary of decision E.29.
- **Each form keeps its strength.** The pixel form is the faithful one. The character form works everywhere: over a serial line, in a terminal multiplexer, and as text that can be copied and searched.

**E.38 — Dependencies are judged by the microcontroller, not by the desktop.**
Running on microcontrollers is a critical requirement (REQ-GEN-002 and REQ-GEN-015). Every crate that runs there, which means every crate except the command-line program, the image file writers, `xtask` and the C interface, is therefore held to the rules of the smallest target, and so is everything it depends on.

The project's own safeguards stop at its own source:
- the panic lints (REQ-GEN-006);
- the bans on floating point and atomic operations (REQ-GEN-016);
- fuzzing (REQ-TST-040).

A dependency can contain any of what they forbid, and each one adds to the 64 kilobyte flash budget of REQ-GEN-009.

Eight candidate crates were tested on 10 October 2026. Each was built alone as `no_std` without allocation for `thumbv6m-none-eabi`, `riscv32imc-unknown-none-elf` and `xtensa-esp32-none-elf`. All eight built:
- `heapless` 0.9.3;
- `bitflags` 2.13.2;
- `httparse` 1.10.1;
- `embedded-io` 0.7.1;
- `base64` 0.23.1;
- `sha1` 0.11.0;
- `serde-json-core` 0.7.0, with `serde`;
- `embedded-websocket` 0.9.5.

Building proves little, though. A first attempt to detect panic and floating-point code by reading the symbols of the unlinked libraries gave the same result for every crate, including one with no code, so it measured nothing. The reliable test is the linked image, with unused code removed (REQ-GEN-019).

The decisions are:
- **The core, the format readers and the slicer have no dependencies.** The fixed-capacity collections and bit flags the core needs are a few hundred lines in the Foundation layer, written and tested under the project's own lints.
- **The web module uses only `embedded-io` and `httparse`.**
  - `embedded-io` is a set of traits with no code, and it is how a port connects its own network stack (REQ-WEB-001).
  - `httparse` is mature, small and has no dependencies of its own.
- **The project writes its own WebSocket framing, SHA-1, base64 and descriptor reader.**
  - `embedded-websocket` brings in five further crates, including copies of the other three.
  - `serde-json-core` brings serde's generated code into the flash image to read one small document (REQ-WEB-002).
  - Each replacement is short, bounded, and testable against the published vectors of its standard: RFC 6455 for WebSocket, RFC 3174 for SHA-1, RFC 4648 for base64 and RFC 8259 for JSON.
- **Crates that never run on a microcontroller are judged normally.** These are crates such as `clap`, `png`, `ratatui` and `crossterm`.

*Reason:* A dependency is code the project ships to a microcontroller but cannot lint, fuzz or budget. On the smallest targets the cost of writing a small, well-specified piece is lower than the cost of trusting it.

*Amended 11 October 2026:* `httparse` is no longer used. `embedded-io` is now the only crate from outside the project that a microcontroller crate depends on.
- **Why.** The symbol check of the linked image (REQ-GEN-019) found two panic paths that `httparse` 1.10.1 brings into every web build: an `unreachable!()` in its SWAR scanner and a range index of a header value. No panic-free alternative exists. `ntex-httparse` inherits the same code, `reqwless` and `edge-http` are built on `httparse`, and `thhp` is unmaintained and full of unsafe code. This is the case the reason above describes.
- **What replaces it.** On the owner's decision, the web module parses response heads with its own parser (`http::head`, RFC 9112 sections 2 to 5). It links no panic path, and is tested, fuzzed and compared with `httparse` on generated heads.
- **Behaviour.** It accepts and rejects what `httparse` did, with one exception: a version `HTTP/x.y` other than 1.0 and 1.1 is now reported as an unsupported version rather than as a malformed head. Where RFC 9112 leaves the recipient a choice, it keeps the choices `httparse` made:
  - a bare LF ends a line (RFC 9112 section 2.2);
  - empty lines before the status line are skipped (section 2.2);
  - the reason phrase may be missing together with the space before it (section 4).

  Obsolete line folding is rejected (section 5.2, REQ-SEC-011).

**E.39 — Details of the web protocols that the requirements leave open.**
Decided while implementing `neoteletext-web` on 10 October 2026:

1. **Descriptor members** (REQ-WEB-002). The JSON members are:
   - `name`;
   - `initial_page`, a string such as `"100"`;
   - `page_template`;
   - `push`, an object with `address` and `method` (`"websocket"` or `"sse"`);
   - `form_origins`;
   - `region`, 0 to 15;
   - `services`, a list of objects with `name`, `display_name` and `descriptor`.

   Unknown or repeated members make the descriptor invalid. Addresses may be relative to the descriptor's own address. Template placeholders may appear only in the path and query, never in the host.
2. **Subscriptions** (REQ-WEB-005). Over WebSocket the client sends `subscribe all` or `subscribe 100 1A0`. For server-sent events the push address gets the query parameter `subscribe=all` or `subscribe=100,1A0`.
3. **`{sub}` without a sub-page** expands to `0000`.
4. **Server-sent event connections send `Accept: text/event-stream`.** REQ-WEB-014 is amended to permit it.
5. **The server-sent events `retry` field is ignored**, because REQ-WEB-005 fixes the reconnection delays.
6. **A response body ended by the connection closing is accepted**, as RFC 9112 section 6.3 allows.
7. **Retries.** A GET is retried once if a kept-alive connection closes before answering. A POST is never retried.
8. **`Expires` cookies.** Until the port supplies calendar time, a cookie with only `Expires` is kept as a session cookie, and session cookies are never saved (REQ-SEC-013).
9. **IPv6 hosts are compared exactly as written**, so `[::1]` and `[0:0:0:0:0:0:0:1]` are different origins. This is the stricter reading of REQ-WEB-015.

*Reason:* Each is the simplest choice consistent with the requirements and the relevant RFCs, and REQ-DOC-004 requires behaviour the requirements leave open to be recorded here.

**E.40 — Presentation Levels 2.5 and 3.5 are in scope, and the presentation level is a setting.**
On 10 October 2026 the project owner asked for complete Presentation Levels 2.5 and 3.5, with comprehensive tests, as Phase 1 work. The receiver therefore implements Levels 1.0, 1.5, 2.5 and 3.5.

- **One setting.** The presentation level is a configuration value with four values: Level 1, Level 1.5, Level 2.5 and Level 3.5. It replaces the earlier on and off switch for Level 1.5 (REQ-L15-001 and REQ-PRF-006).
- **Profile defaults.** The default profile ("EN 300 706") presents at Level 3.5. The "SAA5050" profile presents at Level 1.0, and the "SAA5246A" profile at Level 1.5, as those chips did.
- **Command line.** `ttx --level 1|1.5|2.5|3.5` overrides the level of the profile. The former `--level1-only` option is removed in favour of `--level 1` (REQ-CLI-007).
- **What each level reads.** At Level 1 the page is decoded from its Level 1 bytes alone. At Level 1.5 the Level 1.5 overwrite pass runs (decision E.1). At Levels 2.5 and 3.5 the enhancement processor of decision E.42 runs instead of the Level 1.5 pass. The features that Tables 1 and 25 tick for Level 3.5 only are used only when the level is 3.5: packet X/27/5, the DRCS colour tables of X/28/1 and M/29/1, packets X/28/4 and M/29/4, the Font Style triplet, DRCS modes 1 to 3, and Modified G0 and G2 designation beyond two pairs.

*Reason:* EN 300 706 clause 12 defines the levels as cumulative, so one ordered setting describes a receiver exactly. Tables 1, 25, 28 and 29 say which packets and triplets each level acts on (`research/notes-level25-enhancement.md` sections 1.2 to 1.4). Presenting at the highest level by default shows broadcasts as their authors intended, while the chip profiles keep their period behaviour.

**E.41 — Principles for reading Levels 2.5 and 3.5.**
The clauses for Levels 2.5 and 3.5 leave many details open. The research notes list them: `research/notes-level25-enhancement.md` section 12, `research/notes-level25-objects.md` section 14 and `research/notes-level25-drcs.md` section 15. Each is settled by one of decisions E.42 to E.56, and each of those decisions follows these principles:

1. **Normative text beats informative text, and tables beat prose.** Where a normative clause and an informative annex disagree, the normative clause is followed. Where a table and the prose that describes it disagree, the table is followed, as decisions E.16 and E.21 already do.
2. **Where the standard is silent, follow zvbi, or else the most literal reading.** zvbi 0.2.44 is the test oracle (REQ-TST-023), so its observable behaviour is followed where it has one. Otherwise the most literal reading of the standard is followed. Where zvbi contradicts the standard, the standard is followed. The known case is Table 30, CLUT 1 entries 5 and 6 (decision E.43).
3. **The receiver's status area is never written.** Row 0, columns 0 to 7, belong to the receiver at every level (REQ-SEC-005). Nothing from packets X/26, objects, side panels or row colours may write or colour them.
4. **Faults are contained at the smallest unit.** An uncorrectable or reserved triplet is skipped. An object that cannot be resolved is skipped. A missing enhancement page means that the page is shown without that enhancement. The Level 1 page is always shown.
5. **The limits of Annex A.8 are fixed resource bounds.** Beyond them, further invocations are ignored. Enhancement processing uses no allocation.
6. **Show at once, redraw when complete.** Rows are drawn as soon as the Level 1 page allows, and redrawn when enhancement data arrives or completes. The receiver never waits for enhancement pages, which may take up to 20 seconds to arrive (Annex B.6).

*Reason:* These are the owner's rulings of 10 October 2026. Principle 3 applies REQ-SEC-005, which overrides every other requirement (decision E.32). Principle 4 extends the approach that decision E.9 takes for Level 1.5. Principle 6 answers a question that the standard leaves open (`research/notes-level25-objects.md` section 14 item 30).

**E.42 — How Levels 2.5 and 3.5 are layered, and what a cell holds.**
- **A further pass over the Level 1 grid.** Enhancement processing is a module of the presentation layer that runs after the Level 1 grid has been built. It processes, in this order:
  1. the page's local enhancement data, from its packets X/26;
  2. the objects that the local enhancement data invokes, whether Active, Adaptive or Passive, and whether local, public (from a Public Object Page) or global (from a Global Public Object Page);
  3. when the page has no local enhancement data that displays anything, the default objects of decision E.53.

  It writes both characters and attributes, and resolves contention between attributes as Annex C.2 describes (decision E.47).
- **Read-only access to other pages.** Object pages, DRCS pages and the Magazine Organization Table are read from the page store through a read-only lookup. The enhancement processor never decodes packets itself: it applies the packet layer's decoders to the stored bytes, and it never changes what is stored.
- **What a cell gains.** Each cell of the grid gains:
  - a size made of a vertical part (normal, top half or bottom half) and a horizontal part (normal, left half or right half);
  - underline;
  - invert;
  - a flash mode, with its rate and phase (decision E.48);
  - the Level 3.5 font styles: bold, italic and proportional;
  - glyph variants for G3 characters and for DRCS characters, the latter named by table (normal or global), sub-table and character;
  - its colours as Colour Map indices 0 to 31 (decision E.43).

  The colours of multi-colour DRCS characters are resolved at render time, from the DRCS colour lookup tables that travel with the grid (decision E.54).
- **Side panels** are held beside the 40-column grid, as decision E.49 describes.

*Reason:* Decision E.1 chose a separate pass so that Level 2.5 could be added later without redesign. Clause 13.3 describes local enhancement data as "a one-time usable Active Object", so one walk serves the local data and every object (`research/notes-level25-objects.md` section 2.3). Leaving the stored bytes untouched keeps acquisition and presentation apart (`ARCHITECTURE.md` section 8, rule 5).

**E.43 — The Colour Map and colour indices.**
- **One colour number everywhere.** Every colour in the grid is a 5-bit Colour Map index from 0 to 31, equal to 8 × CLUT + entry, where CLUT is the colour lookup table. This is how every colour field of Tables 28 and 29 is coded, and it is the numbering of Figure 28. The Level 1 colours are CLUT 0 entries 0 to 7, so existing values keep their meaning. Index 8, which is CLUT 1 entry 0, is transparent, and that is already the renderer's transparent index (REQ-RND-023).
- **The map.** The Colour Map has 32 entries of 12-bit colour, four bits for each primary. Its defaults are those of Table 30, verified on the page image of page 89. In particular, CLUT 1 entry 5 is 7, 0, 7 (half magenta) and CLUT 1 entry 6 is 0, 7, 7 (half cyan). zvbi 0.2.44 reports different values for those two entries, and the standard is followed (decision E.41, principle 2).
- **Which CLUTs can change.**
  - CLUTs 2 and 3 are redefined by X/28/0 format 1 or by M/29/0, at Levels 2.5 and 3.5.
  - CLUTs 0 and 1 are redefined by X/28/4 or by M/29/4, at Level 3.5 only. Table 10 is followed where clause 9.4.7 says the reverse.
  - At Level 3.5, 31 entries can be redefined. CLUT 1 entry 0 always stays transparent, and its twelve bits in X/28/4 and M/29/4 are ignored.
- **Bit significance.** In every multi-bit colour field the lowest-numbered bit is the least significant. This includes the default screen colour and default row colour of Table 4, triplet 13.
- **Default black.** "Black" as a default screen or row colour means Colour Map entry 0 (Table 25), which is black unless it is redefined at Level 3.5.
- **Re-mapping and substitution.** The Colour Table Re-mapping of Table 4 (triplet 13, bits 16 to 18) applies to the Level 1 spacing colour attributes, and also to the start-of-row default colours (white foreground and black background), unless Black Background Colour Substitution replaces the background.
- **Numbering errors in the standard.** Figure 28 numbers the CLUTs 1 to 4; the numbering 0 to 3 of Table 30 is used. Where the text of a row of Table 8 contradicts the row's heading, the heading is followed.

*Reason:* EN 300 706 Tables 4, 8, 10, 25, 28, 29 and 30, and Figure 28 (`research/notes-level25-enhancement.md` sections 3.2 and 7.1 to 7.4, and ambiguities A-13, A-14 and A-16 to A-20; `research/notes-level25-drcs.md` section 13.2 for the zvbi values). One index space means that the grid, the renderer and the DRCS colour tables (decision E.54) all use the same colour numbers.

**E.44 — Page-level presentation data: format, precedence and errors.**
- **Format 1 for display.** Packet X/28/0 is read as format 1 on every page that is displayed. Format 2 (Table 7) is never used for display.
- **Precedence.** Side panels, the default screen and row colours, Black Background Colour Substitution, Colour Table Re-mapping and the colour tables are taken from the page before the magazine, and from the magazine before the defaults. The page's own X/28/0 format 1 and X/28/4 therefore beat the magazine's M/29/0 and M/29/4. Within the page, X/28/0 beats X/28/4, and within the magazine M/29/0 beats M/29/4, for every field except the colour tables, where each packet defines different CLUTs.
- **Character set designation** follows Annex C.4 rule 1: X/28/0 format 1, then X/28/4 (Level 3.5 only), then M/29/0, then M/29/4 (Level 3.5 only), then the defaults (decision E.4, as amended).
- **DRCS colour tables** come from the page's X/28/1, then from the magazine's M/29/1, then from the defaults of Annex D.2.2 (decision E.54).
- **Errors.** An X/28 or M/29 packet in which any triplet of a field group is uncorrectable is treated as absent for that field group. It does not take precedence, and the next source in order applies. The field groups are:
  - the page function and page coding;
  - the default G0 and G2 designation and the second G0 designation;
  - the side-panel fields;
  - the sixteen colour words;
  - the default screen and row colours, substitution and re-mapping;
  - each DRCS colour table.

  Inside local enhancement data and objects, an uncorrectable triplet is skipped (decision E.9).
- **Mode identification in X/28/3.** The mode of each PTU is read as decision E.54 describes.

*Reason:* Clauses 9.4.2 to 9.5.3, Tables 4 and 7 to 13, Annex C.4 and Table 1 note 1 (`research/notes-level25-enhancement.md` sections 7.2 and 9.1 to 9.10, and ambiguities A-15 and A-21 to A-23). The standard states that the page overrides the magazine for each pair of packets. Extending that to the pair X/28/4 and M/29/0 is the reading that lets the page's own data win, as it does everywhere else. Treating a damaged field group as absent stops a corrupted packet from overriding good magazine data.

**E.45 — Screen colour, row colours and borders.**
- **Screen colour.** Highest first: a Full Screen Colour triplet; the page's default screen colour; the magazine's default screen colour; Colour Map entry 0 (Annex A.5).
- **Row colour.** Highest first:
  1. Full Row Colour triplets, applied in transmission order;
  2. the screen colour, but only when an explicit Full Screen Colour triplet set it;
  3. the page's default row colour;
  4. the magazine's default row colour;
  5. Colour Map entry 0.

  A default screen colour from X/28 or M/29 does not replace the default row colour.
- **Full Row Colour.** Commands apply in transmission order. "This row only" (S1 S0 = 00) changes only its own row. "Down to the bottom" (S1 S0 = 11) colours its row and every row below it. An earlier "down to the bottom" fill continues below a later "this row only" row.
- **Reserved row selections.** Address Display Row 0 and Full Row Colour with S1 S0 = 01 or 10 set the active position but set no colour.
- **Borders.** The screen areas above row 0, below the last row, to the left and right of the rows, and outside the side panels all use the screen colour.
- **Row 0.** Row colours never colour row 0, columns 0 to 7, of the main page (decision E.41, principle 3). The side-panel area of row 0 shows the screen colour (decision E.49).

*Reason:* Annex A.5 and Table 4 (`research/notes-level25-enhancement.md` sections 4.1 to 4.4 and 7.6, and ambiguities A-1 to A-3, A-25 and A-30). Reading the phrase "the screen colour already invoked" of Annex A.5 as the Full Screen Colour triplet is the only reading under which the default row colour field of Table 4 can ever take effect.

**E.46 — The active position, triplet order and damaged triplets at Levels 2.5 and 3.5.**
- **The strict row rule.** At Levels 2.5 and 3.5 only Full Row Colour (mode 00001), Set Active Position (mode 00100) and Address Display Row 0 (mode 00111 with address 63) set the active row, as clause 12.3.2 requires. The Annex A.3 rule of decision E.3 stays the default at Level 1.5.
- **Set Active Position with a column of 40 or more** sets the row only, and leaves the column unchanged.
- **The column** is set by every column triplet except the reserved modes and the Programme Delivery Control mode 00110. This includes character modes whose data is below 2/0 (clause 12.3.4).
- **Out-of-order triplets.** A triplet that addresses a position earlier than the previous triplet from the same source is applied at the position it addresses. Annex A.4 permits ignoring it. zvbi instead writes the character at the current position, so comparisons with zvbi use only streams that keep the order of Annex A.4 (REQ-TST-023).
- **Termination.** Any triplet with address 63 and mode 11111 ends the data it is in. Its type bits are only hints. An Object Definition triplet met in the local enhancement data also ends the local enhancement data.
- **Origin Modifier.** An Origin Modifier that is not immediately followed by an object invocation is discarded.
- **Damaged triplets.** An uncorrectable triplet is skipped, and does not move the active position (decision E.9).

*Reason:* Clause 12.3.2, Annexes A.3 and A.4, and Table 28 (`research/notes-level25-enhancement.md` sections 2.3, 4.3, 4.6 and 4.11, and ambiguities A-9, A-23 and A-26; `research/notes-level25-objects.md` section 14 items 13, 25, 26 and 31). Object Invocation and Object Definition triplets carry object numbers in their address field, so the Annex A.3 rule would move the row at every invocation. It cannot be used at these levels.

**E.47 — Attribute contention and the display attributes.**
- **Attribute types.** Foreground colour, background colour and flash are attribute types, as Annex A.7.1 lists them. Each bit of the Display Attributes triplet (double height, double width, boxing or window, conceal, invert, and underline or separated mosaics) is an attribute type of its own.
- **Contention.** When two attributes of one type contend for a cell, the latest wins, judged as Annex C.2 says: first by the cell where each takes effect, then by the cell where each is placed, then by the order of transmission.
- **A Level 1 attribute and an X/26 attribute at the same cell.** When a set-at Level 1 spacing attribute and an X/26 attribute of the same type are placed at the same cell, the X/26 attribute wins. This follows the priority of clause 13.2. Read literally, rule 3 of Annex C.2 would always let the Level 1 attribute win, because packets X/26 are sent before the rows (Annex B.2), and that would make enhancement attributes on attribute cells useless.
- **A 0 bit cancels.** A Display Attributes triplet sets all six functions at once. A bit at 0 cancels its function from that cell onward, including a function set by a Level 1 spacing attribute.
- **Conceal.** A Level 1 colour code ends only a Level 1 conceal. A non-spacing conceal lasts until a Display Attributes triplet clears it or the row ends.
- **Boxing.** At Levels 2.5 and 3.5, a Level 1 Start Box on a page that is neither a newsflash nor a subtitle page does not open a window; boxing keeps its Level 1 meaning (decision E.17). Windows come only from the boxing bit of an X/26 Display Attributes triplet, and from boxed pages (decision E.22).
- **Underline** is drawn on the last dot line of the cell, one line thick, on both fields in the 12 by 20 cell. The asymmetry of Table 26 notes 11 and 12 is followed: a non-spacing Underline or Separated attribute affects Level 1 characters, but the Level 1 codes 1/9 and 1/A do not affect enhancement characters. Underline is not drawn on G3 or DRCS characters, for which Table 29 leaves the response undefined.
- **Invert** swaps the cell's foreground and background colours. It is applied before flash and conceal, which then act on the colours displayed.

*Reason:* Annexes A.7.1, C.2 and E.13, clause 13.2, Table 26 notes 11 and 12, and Table 29 mode 01100 (`research/notes-level25-enhancement.md` sections 5.8 and 6.1 to 6.6, and ambiguities A-6 to A-8, A-28 and A-29). Applying invert before flash is the literal reading of Table 29: invert swaps the colours, and flash "affects the foreground colour only".

**E.48 — Flash at Levels 2.5 and 3.5.**
- **The cycle.** One flash cycle lasts one second, and is divided into twelve units of one twelfth of a second, numbered 0 to 11.
  - **Slow flash (1 hertz)** is on during units 0 to 8 and off during units 9 to 11. That is the 3 to 1 ratio of decision E.12.
  - **Fast flash (2 hertz)** treats each half-second of six units alike. With phase p, which is 1, 2 or 3, the character is off during units 2(p − 1) and 2p − 1 of each half-second, and on otherwise.
  - **Incremental flash** gives successive characters, left to right from the triplet's column, the phases 1, 2, 3, 1, 2 and so on. **Decremental flash** gives them the phases 1, 3, 2, 1, 3 and so on.
- **Modes.**
  - Normal flash shows the background colour in the off phase.
  - Inverted flash swaps the on and off phases.
  - Adjacent-CLUT flash shows, in the off phase, the colour whose index is the cell's foreground index exclusive-ORed with 8. That is the same entry in the neighbouring CLUT: CLUT 0 with CLUT 1, and CLUT 2 with CLUT 3.
- **Level 1 Flash.** At Levels 2.5 and 3.5 the Level 1 Flash attribute 0/8 means slow flash in normal mode. At Levels 1 and 1.5, decision E.12 is unchanged.
- **Duration.** An Additional Flash Functions attribute lasts to the end of the row, or until another flash attribute: the Level 1 codes 0/8 and 0/9, or another Additional Flash Functions triplet.
- **Multi-colour DRCS characters.** In the off phase of normal or inverted flash, a multi-colour DRCS character shows a space in the background colour (clause 14.3.2). Adjacent-CLUT flash maps the index of each of its pixels by exclusive-OR with 8.

*Reason:* Table 29 mode 00111 gives the two rates and names the modes, but gives no duty cycle and no meaning for the phases (`research/notes-level25-enhancement.md` section 5.7, and ambiguities A-4 and A-5; `research/notes-level25-drcs.md` section 6.3 and ambiguity A-9). Twelve units divide both rates and the three phases evenly, and the slow rate keeps the ratio that decision E.12 measured on the SAA5050.

**E.49 — Side panels.**
- **Storage.** Side panels are held beside the 40-column grid, in a panel area addressed by columns 40 to 71 and rows 1 to 24 (clause 12.5.3). The right panel occupies columns 40 to 40 + right width − 1, and the left panel columns 72 − left width to 71.
- **Widths.** The widths follow from the left and right enable bits and from the number n in bits 7 to 10 of triplet 2 of Table 4:

  | Left panel | Right panel | Left width | Right width |
  |---|---|---|---|
  | Off | Off | 0 | 0 |
  | On | Off | n, or 16 when n is 0 | 0 |
  | Off | On | 0 | 16 − n, or 16 when n is 0 |
  | On | On | n | 16 − n, so that n = 0 gives a left width of 0 and a right width of 16 |

- **Status flag.** At Level 2.5 side panels are shown only when the Side Panel Status Flag is 1. At Level 3.5 they are shown whenever they are enabled.
- **Sources.** Side-panel data is taken as decision E.44 describes. When no X/28 or M/29 packet gives it, the fallback bits of the page's POP link in the Magazine Organization Table apply (decision E.53). With no POP link, there are no side panels.
- **Display width.** The display is 40 columns plus the left and right widths, and so not always 56. The renderer composes the left panel, the main page and the right panel, in that order.
- **Content.** Side-panel content comes only from objects placed by Origin Modifiers into columns 40 to 71, in rows 1 to 24.
  - Each panel row starts from the start-of-row defaults of clause 12.5.4 rule 1, with the full row colour as its background and a space in every cell.
  - Attributes do not pass between a panel and the main page, in either direction (rule 2).
  - Writes outside the enabled panel columns, to row 0, below row 24 or beyond column 71 are discarded.
  - An object element that would cross between the main page and a panel is discarded.
- **Row 0** has no transmitted panel content. Its panel area shows the screen colour.

*Reason:* Clause 12.5, and Tables 4 and 21 (`research/notes-level25-enhancement.md` section 8, and ambiguities A-10, A-11 and A-30; `research/notes-level25-objects.md` section 2.9, and section 14 items 12 and 38). The width table is the literal reading of "16 minus this value" and "0 means 16 columns".

**E.50 — Character repertoire, font styles and G3 drawing.**
- **National option at Levels 2.5 and 3.5.** The national option comes from bits 8 to 10 of the highest-priority X/28 or M/29 designation when one is present (Annex E.9), and from the header bits C12 to C14 otherwise (decision E.4, as amended).
- **Modified G0 and G2 designation.** At Level 2.5, designations beyond two G0 and G2 pairs on a page, counting the default pair, are ignored. At Level 3.5 there is no limit.
- **G1 characters written by triplets.** A G1 character written by an X/26 triplet is a mosaic, drawn contiguous or separated according to the non-spacing Underline or Separated attribute. Its codes in columns 4 and 5 are the characters of the current G0 set, without national option (Table 47 note 3).
- **G3 and hold.** A G3 character written by X/26 does not change the held mosaic, and Hold Mosaics never replaces a character written by X/26 (decision E.5 (d)).
- **G3 drawing.** The smoothed mosaics and line-drawing characters of Table 48 are drawn from the project's own geometry: the sextant geometry of Unicode document L2/19-025, as polygons on halves and thirds of the cell.
  - A generator in `xtask` rasterises them into replaceable glyph data for the 12 by 10 and 12 by 20 cells, in the same way as the Bedstead glyphs are exported.
  - Points at one third and two thirds of the cell height are rounded down to whole lines.
  - Light strokes are 2 half-dots wide and heavy strokes 4.
  - Position 2/F, "alternate dots", is a single-dot checkerboard on the 12-dot grid, as the words of Table 48 say, and not the 2 by 1 pattern of its figure.
- **Undefined codes.** G3 positions 6/E, 6/F, 7/E and 7/F, the undefined Latin G2 positions, and invalid DRCS data show a space, and a diagnostic is traced.
- **Font styles (Level 3.5).**
  - **Bold** is the glyph combined by OR with itself shifted one half-dot to the right.
  - **Italic** shifts each glyph line to the right by the number of lines below it in the cell divided by 4, in whole half-dots, rounded down.
  - **Proportional spacing** is recorded in the cell but drawn at the fixed pitch. The standard defines no spacing algorithm, Annex G.2 leaves appearance to the decoder, and zvbi only sets a flag.
  - Styles apply to G0 and G2 characters only (Table 25), never to G1, G3 or DRCS characters.
  - A Font Style triplet acts from its column to the end of its row. Its row count R extends the style, from the same column to the end of the row, over each of the next R rows. A later Font Style triplet replaces the style from its own position onward.

*Reason:* Clause 15, Tables 25, 29, 34, 47 and 48, and Annexes E.9 and G.2 (`research/notes-level25-drcs.md` sections 9 and 10, and ambiguities A-12 to A-16; `research/notes-level25-enhancement.md` sections 5.3 to 5.11). The geometry agreed with the standard's figure on 95 to 100 percent of pixels for every polygon glyph (`research/notes-level25-drcs.md` section 9.3). Drawing it from the project's own geometry keeps glyph provenance clean (decision E.25).

**E.51 — Objects: resolution, nesting and limits.**
- **Pointers.** A pointer in a Public Object Page or Global Public Object Page whose value is not a valid triplet address means that the object is absent. That covers the values 507 to 510, pointers into the pointer packets, and pointers to a triplet that is not an Object Definition. The value 0x1FF means "not used". Definitions in packets 1 and 2 of an object page cannot be reached by any pointer, and are ignored.
- **Checks.** An invocation whose definition has a different type, or whose usage bits exclude the presentation level, is skipped. The object number bits N8 to N0 of the definition are not compared with those of the invocation.
- **Local invocations** of triplet numbers 13 to 15, or of designation codes that were not received, are skipped.
- **Objects inside public objects** refer to the displayed page's POP and GPOP links.
- **Nesting.** The local enhancement data may invoke objects of every type. Local enhancement data and Active objects may invoke Adaptive and Passive objects; Adaptive objects may invoke Passive objects; Passive objects invoke nothing. Any other invocation is skipped. These rules bound the depth at three below the local enhancement data.
- **Coordinates.** Every coordinate inside an object is relative to its origin, including Address Display Row 0 and row address 40.
- **Writes out of range.** An element that would write outside rows 0 to 24 or columns 0 to 71 is discarded, and the object continues. So are writes to row 0, columns 0 to 7 (decision E.41, principle 3), and the side-panel writes that decision E.49 discards.
- **Adaptive objects.** Their attribute effects on a row end at the rightmost cell that any triplet of the object addresses on that row. The object is scanned before it runs, to find those cells.
- **Passive objects.** Each invocation starts from the default attributes of clause 13.5. Transparency inside them is CLUT 1 entry 0, as everywhere else.
- **Priority.** Characters follow clause 13.2: Passive objects over Adaptive objects, over local enhancement data, over Active objects, over the Level 1 page. Ties between attributes follow Annex C.2, with the order of transmission last.
- **Restricted triplets.** Full Screen Colour, Full Row Colour and Address Display Row 0 inside Adaptive or Passive objects are ignored.
- **Damage.** An uncorrectable triplet inside an object is skipped. In a pointer table, an uncorrectable triplet makes both of its pointers unused. When packets of an object page are missing, only the objects that need them are dropped.
- **Definitions after the marker "no Local Object definitions follow"** are still used if they are invoked.
- **Limits.** Annex A.8 is enforced as a fixed bound: at most 127 invocations of each object type per page, at most 15 objects open at once, and at most 15 objects opened in succession. Further invocations are ignored.

*Reason:* Clauses 10.5.1 and 13.1 to 13.8, Table 28, and Annexes A.4, A.7, A.8 and C.4 (`research/notes-level25-objects.md` sections 2 to 5, and section 14 items 6, 7, 9 to 17, 25 to 29, 32, 35, 36, 42 and 45). Each choice confines a fault to the object that has it (decision E.41, principle 4). The standard does not require the object number check, so it is not made.

**E.52 — Links to object and DRCS pages: the Magazine Organization Table and packets X/27/4 and X/27/5.**
- **Precedence.** The links in the page's own X/27/4, and in X/27/5 at Level 3.5, take precedence over the Magazine Organization Table, whatever the X/27/4,5 flag of the table's link says.
- **X/27/4 links 0 to 3** are taken by position, as Table 16 fixes them: GPOP, POP, GDRCS and DRCS. Their function bits are still checked, and a link whose bits do not match its position is ignored. A link that duplicates one already accepted is ignored. At Level 3.5, X/27/4 links 4 and 5 and X/27/5 links 0 and 1 are taken from their function and validity bits.
- **Format 1 marker.** A link is accepted only when bit 11 of its first triplet is 1 and bit 1 of its second triplet is 0, as Table 15 requires. The link function and the page validity are two bits each, as Table 15 shows, whatever the bit count of clause 9.6.2 says.
- **No link.** Page validity 00, all sixteen sub-page flags at 0, or page number FF means no link.
- **Sub-page flags** of X/27/4 are used only as hints for acquisition. Invocations name the sub-page directly by its S1 value.
- **Object pages are identified by the links that name them.** X/28/0 is not required on an object page.
- **Magazine of a table link.** The magazine of a link in the Magazine Organization Table is absolute, with 0 meaning magazine 8. This reading is checked against zvbi (REQ-TST-023), and any difference is recorded here.
- **Global flags.** The global object flag and the global DRCS flag of a page's entry are used only as hints for acquisition. A reference to the GPOP or GDRCS is resolved through the magazine's GPOP or GDRCS link whatever the flag says, as decision E.53 does for default objects.
- **Level 3.5 links.** A Level 3.5 decoder uses packets X/22 to X/24 of the table. When X/22 or X/24 is missing, it uses X/19 to X/21 and processes the page at Level 2.5 (clause 10.6.1). When only X/23 is missing, POP links 4 to 7 are taken from the Level 2.5 links in X/20.
- **Page xFF.** The table's byte pair for page xFF is ignored.
- **Sub-page counts.** A link's sub-page count n means that sub-pages 0 to n are assumed, and missing ones are absent. Object sub-pages beyond the advertised count are used if they are stored.
- **Enhancement pages are not text.** Object pages, DRCS pages and the Magazine Organization Table never receive Level 1 national option or display semantics, and their bits C12 and C13 mean "fragment" and "partial page" (Annex B.6). A Magazine Organization Table whose sub-page S1 is not 0 is ignored.

*Reason:* Clauses 9.6.2, 10.5.1.4, 10.5.2.2 and 10.6, Tables 15, 16 and 20 to 23, and Annexes A.1 and B.6 (`research/notes-level25-objects.md` sections 7 and 9, and section 14 items 1 to 5, 8, 22 to 24, 34, 40, 41 and 43).

**E.53 — Default objects.**
- **Suppression.** Default objects are suppressed only by packets X/26 that carry display triplets. A page whose X/26 packets hold only Programme Delivery Control triplets keeps its default objects.
- **Magazine-wide default objects.** The default objects of the GPOP link apply to every page of the magazine that has no local display enhancement, whatever the page's global object flag says.
- **Order.** GPOP default object 1, GPOP default object 2, POP default object 1, POP default object 2, as Annex G.5 recommends. Where two of the same type write the same cell, the latest invoked wins (Annex A.7.1).
- **Origin.** Each default object is placed with its origin at row 0, column 0 (clause 13.8).
- **Fallback flags.** The fallback bits for side panels and Black Background Colour Substitution are read from the page's POP link. With no POP link they are clear, which means no side panels and no substitution.

*Reason:* Clause 13.8, Table 21, and Annexes E.11, E.12 and G.5 (`research/notes-level25-objects.md` sections 7.5, 7.6 and 8, and section 14 items 18 to 21 and 33).

**E.54 — DRCS pages, modes and colours.**
- **Usable when complete.** A DRCS sub-page is usable once every PTU that a referenced character needs has been received (Annex B). Later partial receptions replace the PTUs they carry.
- **Mode identification.** Packet X/28/3 gives the mode of each PTU. The mode of PTU n is at positions 4n to 4n + 3 of the bit stream formed by triplets 2 to 12, least significant bit first. Without X/28/3, every PTU is mode 0 (Annex D.1.7). A Level 2.5 decoder reads X/28/3, and treats any PTU not marked mode 0 (12 by 10 by 1) as unusable.
- **Mode 3 (6 by 5 by 4).** The most significant of the six bits of each D-byte is the left pixel, as in mode 0. Each pixel is drawn as 2 by 2 dots in the 12 by 10 cell, and 2 by 4 in the 12 by 20 cell.
- **Colour tables.** A Level 3.5 decoder takes the DRCS colour lookup tables from M/29/1 when the page has no X/28/1, and from the defaults of Annex D.2.2 when neither is present. The row of Table 8 headed "DCLUT16 for Normal" is for normal DRCS, as its heading says.
- **Limits not enforced.** The Level 2.5 limits of two sub-tables and 24 characters, and the Level 3.5 limit of 384 PTUs, bind the broadcaster. They are not enforced beyond the store's own bound.
- **Attributes on multi-colour characters.** Flash follows decision E.48. Invert has no effect. Conceal shows a space in the background colour. Boxing follows the cell. Font styles do not apply (decision E.50).
- **Enlarged characters** are drawn by exact pixel replication, with no smoothing.
- **Missing and invalid data.** A cell whose DRCS data is missing shows a space in its background colour. Invalid DRCS data shows a space, and a diagnostic is traced. Invalid data means a character whose first PTU is marked 1110 or 1111, a character that runs past PTU 47, or a D-byte outside 4/0 to 7/F or failing its parity check.
- **The DRCS Mode triplet.** One whose level bits exclude the presentation level is ignored. The reserved level value 00 is treated as both levels. The sub-table it chooses persists to the end of the local enhancement data or of the object.
- **Typographical errors.** The "7/E and 7/E" of Table 25 is read as 7/E and 7/F, and Table 10 is followed for the colour tables of X/28/4.

*Reason:* Clause 14, Tables 8, 9, 28, 29 and 31, and Annexes B, D.1.7 and D.2.2 (`research/notes-level25-drcs.md` sections 2 to 8, and ambiguities A-1 to A-11 and A-16 to A-21).

**E.55 — The resolved colour mode of the renderer.**
- **Two colour modes.**
  - **The palette mode** of REQ-RND-023, with the indices 16 + 8f + b and 80 + 8f + b, stays as the mode for Levels 1 and 1.5. Ports with hardware palettes keep the benefit that flash and reveal need only a new palette.
  - **The resolved mode** resolves flash and conceal at render time, from a flash phase and the reveal setting, and gives Colour Map indices 0 to 31 directly.
- **When each is used.** The resolved mode is used at Levels 2.5 and 3.5. The palette mode is refused for any grid that uses a colour outside CLUT 0, or any flash mode of Level 2.5.
- **Change tracking.** The change tracker reports which rows hold flashing or concealed cells, so that a port redraws only those rows when the flash phase or the reveal setting changes.

*Reason:* With 32 colours and four flash modes, the palette encoding would need far more entries than a hardware palette holds. Resolving at render time keeps the pull model of decision E.29, and costs only the redrawing of the rows that flash or conceal.

**E.56 — Storing enhancement pages and magazine data.**
- **Stored as received.** Object pages, DRCS pages and the Magazine Organization Table are kept in the page store. Each reception of such a page replaces the stored bytes of the packets it carries, with no merge by parity (decision E.8 does not apply). This applies to the rows of:
  - a page whose X/28/0 page function is not "basic Level 1 page";
  - page xFE, the Magazine Organization Table;
  - a page that a stored Magazine Organization Table link, or a stored X/27/4 or X/27/5 link, names as an object or DRCS page (decision E.52).
- **Hexadecimal pages.** At Levels 2.5 and 3.5, pages with hexadecimal numbers are stored, because enhancement pages usually have them (Annex E.1). Navigation still skips them, unless the diagnostic setting of REQ-ACQ-013 is on.
- **Replacement.** Stored object pages are replaced by new receptions. Nothing is discarded when the Magazine Organization Table changes, beyond the store's normal eviction.
- **Magazine data.** Packets M/29/0, M/29/1 and M/29/4 are stored for each magazine. Each new reception replaces the stored packet. All stored M/29 packets are discarded when the service changes, meaning a new network identification in packet 8/30 or a new input. There is no time-out.

*Reason:* Annexes A.1, B.6, E.1 and E.8, and clause 10.6 (`research/notes-level25-objects.md` sections 6 and 11, and section 14 item 39; `research/notes-level25-enhancement.md` ambiguity A-24). Object pages carry Hamming 24/18 triplets, whose bytes do not follow the odd parity rule, so a merge by parity would keep stale bytes or reject good ones.

**E.57 — Points of the enhancement processor, objects and DRCS that decisions E.40 to E.56 leave open.**
Recorded on 11 October 2026, when the processor was written (REQ-DOC-004). Each point is commented where the code applies it.
- **Damaged triplets and the Origin Modifier.** A damaged triplet met while an Origin Modifier waits for its invocation discards the offsets.
- **DRCS invocation and data below 2/0.** The rule that a character triplet whose data is below 2/0 writes nothing does not apply to the DRCS Character Invocation, because global DRCS characters 0 to 31 have data below 2/0.
- **Counting invocations.** Every permitted invocation counts toward the limit of 127 per object type, whether or not its object is found, so that failed lookups also bound the work (REQ-OBJ-020).
- **Objects open at once (Annex A.8).** Counted at the invocation point against the open areas of all earlier objects. An object's area runs from its invocation to the last position any of its moving triplets reaches. The count of successive opens restarts at any character or attribute actually placed.
- **Tables without Level 3.5 links.** At Level 3.5, a stored Magazine Organization Table that lacks packet X/22 or X/24 makes the page fall back to Level 2.5 links, whatever X/27/4 says (clause 10.6.1).
- **Links role by role.** X/27/4 and X/27/5 take precedence over the table one role at a time; a role they do not give comes from the table.
- **Default objects** are found through the table's own GPOP and POP pages (clause 13.8). A table link to page FF gives no fallback bits and no default objects. A display triplet, which suppresses the default objects, is any character or attribute column triplet, screen or row colour, Address Display Row 0 or invocation met before the first Termination Marker or Object Definition (with decision E.53).
- **An object's area.** An object writes only into the area of its origin column: the main page or one side panel. Elements that fall outside it are discarded, which covers an object that would cross from the page into a panel.
- **Box codes at Levels 2.5 and 3.5.** On a page with neither C5 nor C6 set, both Level 1 box codes are ignored. On a boxed page, one boxed state is shared with the window bit of the Display Attributes triplet, and the later of the two wins.
- **Side-panel rows** start with the re-mapped white foreground (decision E.14) and the row colour as their background.
- **Overlapping Adaptive objects** keep one merged attribute record per cell, owned by the latest object to place there.
- **Passive objects** apply their attributes in transmission order, as the walk meets them; this is scan order for streams that keep the order of Annex A.4. A Passive attribute at a discarded position is dropped.
- **Incremental and decremental flash** count phases by column from the triplet's column, spaces included.
- **Which rows are rebuilt.** Row 0 only when C7 is clear, always keeping columns 0 to 7. Row 24 only when it is shown and not drawn by the receiver. A page with C10 set, or with an X/28/0 page function other than basic text, keeps its Level 1 rows and gets no side panels.
- **Enhancement pages shown as text (REQ-OBJ-041).** A page is taken as an enhancement page, and its rows 1 to 24 are blanked, when its number is xFE or its stored copy carries an X/28 page function of GPOP, POP, GDRCS, DRCS or MOT. A page named only by a link is still shown, so that a wrong link cannot hide a display page.
- **DRCS faults (REQ-DRC-009).** An uncorrectable X/28/3 mode code counts as missing data, not invalid data. An invalid character shows as a space and is counted. A missing character keeps its DRCS reference with no pattern, and is drawn empty. Patterns beyond the grid's capacity count as missing.
- **X/26/15** is always processed at Levels 2.5 and 3.5; the profile option of decision E.2 concerns Level 1.5 only.
- **Lower-row fill cells** of double height take the background of the cell above and keep its box membership; flash and invert are cleared.
- **Reporting.** `enhance::apply` returns an `EnhancementReport` that counts each kind of fault and the triplets read. The caller adds the fault counts to the decoder's statistics after building (REQ-L25-051, REQ-ARCH-007). A build is the page, its reception count and the level asked for, and the build recorded last is not counted again. A viewer builds the shown page on every redraw, and counting each redraw would make the counts measure how long the page stayed on screen; counting once per reception matches the acquisition counts, which count a damaged packet each time it is received. The command-line `list` command builds every page once to show the counts. With tracing on, the walk reports each transition of its page-level run as a trace entry of the enhancement walk machine (REQ-FSM-010); the runs for each row repeat the same transitions, so they are not traced. A transition that met a fault names the fault as its action. The Level 1.5 walk reports its transitions in the same way.

*Reason:* Clauses 12.3, 13 and 14, Annexes A and C, and the research notes' ambiguity lists (`research/notes-level25-enhancement.md`, `research/notes-level25-objects.md`, `research/notes-level25-drcs.md`). Each choice keeps the work bounded, keeps faults contained to the cell or object concerned (decision E.41), or takes the reading that never hides displayed text.

**E.58 — Points of rendering at Levels 2.5 and 3.5 that decisions E.47 to E.55 leave open.**
Recorded on 11 October 2026 (REQ-DOC-004).
- **Choosing the colour mode.** `RenderOptions::from_config` keeps the palette mode at every level, so Level 1 output is unchanged; `RenderOptions::for_level` chooses the resolved mode at Levels 2.5 and 3.5, and the command-line program uses it. The palette mode refuses a grid it cannot draw (`RenderError::NeedsResolvedMode`), and the 4-bit format refuses indices above 15 in the resolved mode.
- **Underline** is drawn on the last line of the cell, for G0 and G2 characters only, and on double height only on the bottom half, enlarged to two lines. It is not drawn on empty cells.
- **Italic** uses the position of the normal-height line, so a double-height italic character is the normal one enlarged.
- **Transparent pixels** of multi-colour DRCS characters show the plane below (the row colour or the picture, Table C.1), not the cell's background. A transparent colour in row 0, columns 0 to 7, shows the picture, never the row colour.
- **Mix mode** makes gaps the picture; a dot hidden by flash or conceal therefore also shows the picture.
- **Side panels.** Panel widths are clamped to 16 columns each. A grid with no panel storage but panels enabled shows the screen colour there. Borders are the caller's to fill, in the colour `border_index` gives.
- **Still images** (`ttx show` and `ttx render`) are drawn at flash unit 0, in the "on" phase of every rate.
- **G3 glyphs.** A pixel is set when its centre, nudged upwards by an infinitesimal amount, lies inside the shape, which makes mirrors and complements exact. 2/E and 3/E are drawn at half-dots 1–2 and 9–10; 4/8 to 4/B are straight diagonals; diagonal strokes are two half-dots wide; the checkerboard alternates per row of the 12×10 matrix. The generated glyphs are used at every level, so G3 characters at Level 1.5 also change from the temporary glyph source.
- **Eight-colour terminals** show each primary when it is above half intensity.

*Reason:* Table C.1, clauses 12.3 and 15, and the rendering requirements REQ-RND-030 to REQ-RND-041. Keeping the palette mode as the default of `from_config` keeps the golden rasters of Levels 1 and 1.5 unchanged (REQ-TST-030).

**E.59 — Points of TTI files and the authoring builder that REQ-INP-023 to REQ-INP-025 leave open.**
Recorded on 11 October 2026 (REQ-DOC-004).
- **The page function in TTI files.** A function carried by an `OL,28` line (X/28/0, X/28/3 or X/28/4) wins over a `PF` line, and the lowest designation code wins among them, whatever the order of the lines, so that a page read, written and read again is unchanged.
- **Building X/28/0 from `RE` and `PF`.** A function other than 0 gives the Table 6 form. A basic page with `RE`, or with a coding other than 0, gives the Table 4 form with every other field at its default, including the colours of Table 30. `PF,0,0` alone gives no packet, so that it does not override a magazine's M/29/0.
- **The Table 6 marker** "11111111100" of bits 8 to 18 is read bit 8 first.
- **Page codings 1, 4 and 5** are sent as parity text.
- **Page mFF** carries the magazine packets: it is sent as a time-filling header after its M/29 packets, and converting a stream to TTI puts each magazine's stored M/29 packets back on it.
- **Builder defaults.** Pages are sent with C4 set. Enhancement sub-codes follow Annex A.1, with the last packet sent in S4 and S3. DRCS pages pad unused PTUs with spaces and send X/28/3 unless told not to. A page with no local data sends no packet X/26 (clause 13.8: any X/26 cancels the default objects). Table entries for both levels repeat the Level 2.5 links in X/22 to X/24.
- **Converting an enhancement page with no X/28** writes rows that all pass parity as parity text; otherwise the page is taken to be a POP with coding 2.

*Reason:* Clauses 9.4, 10 and 13.8, Annexes A.1, B.2 and B.7, and the conventions of existing TTI files (vbit2 read as documentation only, decision E.31).

**E.60 — zvbi as the oracle at Levels 2.5 and 3.5.**
zvbi is the oracle of REQ-TST-023, compared cell by cell by `tests/oracles/run_cells.sh`. Tracing every difference on the authored stream with a debug build of zvbi 0.2.44 and 0.2.45 showed these zvbi faults, which `tests/oracles/zvbi-known-differences.txt` names page by page:
- it does not store a sub-page when the next header in the magazine has the same page number (the test harness puts a time-filling header between them, which changes nothing for a decoder that follows clause 7.1.2);
- it reads the page number of an X/27/4 or X/27/5 link one bit too high (EN 300 706 Table 15), so no such link is followed;
- it starts a Local Object at its definition triplet and stops there, so Local Objects never draw;
- it drops all of a page's enhancement at its first fault, such as one invalid DRCS character, where this receiver contains the fault (decision E.41);
- it applies a Display Attributes size only to characters that enhancement data writes;
- it flashes for the reserved flash rates;
- it never invokes the default objects of the table's global link;
- it lets M/29/0 beat X/28/4 (against decision E.44);
- it takes page 1F0 to be the Basic TOP Table, so the authored stream puts its GPOP elsewhere;
- its default colours differ from Table 30 at CLUT 1 entries 5 and 6 (decision E.40), so colours are compared as indices.

Where zvbi follows a different reading from decisions E.45 (the full row colour behind substituted black backgrounds) and E.53 (X/26 with no displayed data), the page is listed with that reason. Authored pages 119 and 124 exercise DRCS and sizes without these faults, so zvbi can check them.

*Reason:* REQ-TST-023 requires an independent oracle; recording its faults keeps the comparison strict everywhere else, since a page not in the list must agree in every compared cell.

**E.61 — Compile-time features: a build contains only the parts it uses.**
Recorded on 11 October 2026 (REQ-DOC-004), on the owner's instruction that the code be organised so that only the parts a build needs are compiled in.

The linker removes code nothing calls, but not code reachable through a choice made at run time. Measured on 11 October 2026 with a linked image built with `opt-level = "z"` and link-time optimisation (`x86_64-unknown-none`), whose roots are the functions a port calls:
- a decoder that composes its pages through the configured level carries the Level 2.5 and 3.5 processor, about 27 kilobytes of code, even when it is configured for Level 1.5, which needs 1.7;
- format detection reaches every reader: about 40 kilobytes of code in a port that reads only T42, which needs 1;
- the renderer adds 6 kilobytes of code and the Bedstead glyphs 33 kilobytes of data;
- forms add 8 kilobytes of code and 6 of data.

Only compile-time features can remove these. The rules:
1. **Three tools.** Crates separate different dependencies or environments. Cargo features separate the optional areas within a crate. The configuration chooses behaviour among what is compiled.
2. **Features are additive and coarse**: one per area of Part F, never one per function. A feature never changes what another feature does.
3. **Data types and acquisition are never behind a feature.** Level 1.5 reads the character set designations of X/28/0 and M/29/0 (decision E.4), so acquisition stays whole at every level. Keeping types unconditional keeps the features additive. Only processing is optional.
4. **A configured level that is not compiled in** falls back to the highest compiled level below it (`PresentationLevel::available`).
5. **Each crate's default is every feature**, so that a plain dependency works. Library crates depend on one another with default features off and name what they use, so that Cargo's feature unification never brings in an area a port did not ask for.
6. **The gate builds and tests a fixed list of builds** from `xtask`, and measures the linked test image of REQ-GEN-009 and REQ-GEN-019.

The features of `neoteletext-core`, all on by default:

| Feature | Contents |
|---|---|
| `level15` | The Level 1.5 pass (area L15) |
| `level25` | The enhancement processor for Levels 2.5 and 3.5 (areas L25, OBJ and DRC) |
| `render` | The row renderer and the generated G3 glyphs (area RND) |
| `original` | The project's own glyph set (REQ-RND-028, decision E.62); removed by decision E.72, when `bedstead` becomes a default feature and part of the modern-classic profile |
| `bedstead` | The temporary Bedstead glyph set (decision E.25); not a default since decision E.62, kept for comparison |
| `viewer` | The viewer state machine, navigation and history (area NAV) |
| `forms` | The forms model (Appendix C) |

`compose_level`, which applies the level's pass, moves from the viewer to its own module `compose`, and `Zoom` from the renderer to the Level 1 grid, because a port without the viewer or the renderer still composes pages.

The features of `neoteletext-formats`, all on by default except `std`. T42 is always compiled. Detection recognises only the formats compiled in, and a format asked for by name that is not compiled in gives the diagnostic `FormatNotCompiled`:

| Feature | Contents |
|---|---|
| `transport` | Transport stream and PES readers (REQ-INP-006 to 009) |
| `sliced` | Linux sliced VBI (REQ-INP-010) |
| `pages` | The page encoder (REQ-INP-019) |
| `tti`, `ep1`, `edittf`, `dump` | One page-file reader each; each needs `pages` |
| `check` | The page checker (REQ-CLI-006); needs `tti` |
| `std` | The writers over `std::io`; the TTI writer and the authoring builder also need `tti` |

Detection reading T42 only is 4.3 kilobytes of code, down from 46.8.

The features of `neoteletext-web`, all on by default. A request a build cannot serve gives an event or failure saying so: `Event::PushNotCompiledIn`, `FormRefused::NotCompiledIn`, `CookieRejected::NotCompiledIn`. The zxnet feed is not a feature: nothing in the user agent reaches it, so the linker already leaves it out.

| Feature | Contents | Code |
|---|---|---|
| `push` | WebSocket and server-sent events (REQ-WEB-005) | 12.3 kilobytes |
| `cookies` | The cookie store's use by the agent (REQ-WEB-006) | 7.6 kilobytes |
| `forms` | Form submission and its response (REQ-WEB-010, REQ-WEB-011) | 4.0 kilobytes |

The application layer of `ttx` is a library crate of its own, `neoteletext-app`: sessions, inputs, both processes of live mode, and sandboxing, which a windowed viewer will share. Its features are `live` (the web module, TLS and the fetcher process) and `slice` (the slicer). `ttx`'s features are `view` (the interactive viewer and its terminal library), `live` and `slice`. Every command stays defined, so the help text never changes, and a command whose area is left out exits with the usage status and says so. A `ttx` without `live` contains no TLS code and is about half the size. The fetcher stays a second run of the same program: a separate fetcher binary would leave TLS out of the decoder process too, but is deferred for its packaging cost, since the decoder's sandbox already keeps it off the network.

*Reason:* The microcontroller targets (REQ-GEN-015) and the flash budget of REQ-GEN-009.

**E.62 — How the original glyphs are drawn and accepted.**
Recorded on 11 October 2026 (REQ-DOC-004), on the owner's instruction to write the checks first and then start the glyphs of REQ-RND-028.

**What is drawn.** The required characters are generated, not chosen: every code point the character set tables produce for every G0 set, every national option and every G2 set, codes 2/0 to 7/F, plus the spacing forms of the diacritical marks that decision E.26 overlays. `cargo xtask glyph-inventory` writes them to `crates/neoteletext-core/src/glyphs/required.txt`. Block mosaics are computed (REQ-CHR-005), G3 is generated from geometry (REQ-RND-037), and DRCS characters come from the service, so none of them is drawn.

**How it is drawn.** The glyphs are kept in `crates/neoteletext-core/src/glyphs/original.txt`, one glyph per entry, drawn as nine rows of five characters. An entry is one of three kinds:
- drawn;
- a copy of another entry, for letters that share a shape across scripts, such as Latin A and Cyrillic А;
- a base letter with a spacing mark overlaid, as decision E.26 composes at run time.

The rules are written in words in `docs/glyph-style-guide.md`, from first principles: the grid, cap height, x-height, descenders, stroke, marks. They are never written from the SAA5050 or Bedstead glyphs.

**Who draws.** The designer works from a design pack: the style guide, the required list and the file format. The designer never sees the SAA5050 datasheets, the Bedstead files, the generated Bedstead table, golden rasters or rendered samples.

**How the glyphs are accepted.** `cargo xtask glyph-check` checks:
1. **Format:** every entry parses, and no code point appears twice.
2. **Well-formedness:** no ink in the descender rows without the `descender` tag, no isolated dot without the `dots` tag, and no empty glyph except a space.
3. **Confusables:** no two characters of one set have the same glyph, and the listed confusable pairs differ by at least two dots.
4. **Independence:** every drawn glyph is compared with the Bedstead glyph of the same code point, the stand-in for the SAA5050. An exact match, or a difference of one dot, fails unless `crates/neoteletext-core/src/glyphs/original-audit.txt` gives the reason. Shapes such as a full stop or a hyphen have only one sensible form.
5. **Coverage:** reported always; required to be complete with `--complete`, which becomes part of the release check of REQ-RND-020.

Recognition by people fluent in each script, and proofs of real pages, are reviewed by hand.

*Amended 11 October 2026:* all 530 required characters are drawn (427 drawn, 45 copies, 58 overlays), `glyph-check --complete` passes, and the original set is the core's default; `bedstead` is no longer a default feature. The full gate runs `glyph-check --complete`, which also fails if the defaults include Bedstead. The review by readers of each script has not been done (IMPLEMENTATION-LOG.md row 54).

*Reason:* REQ-RND-020 and REQ-RND-028. Writing the checks before the glyphs gives the designer feedback while drawing, and the independence check makes the clean room something the gate measures rather than a promise.
*Superseded on 12 October 2026 by decision E.72:* Bedstead is the bundled font, and the original glyph set and its checks are removed.

**Decisions E.63 to E.70: the web-native browser.**
These record the design of `docs/web-native-concept.md` (its decisions 1 to 14, settled on 11 October 2026: decisions 1 to 5 are the owner's, and 6 to 14 were recommended and accepted by the owner), the owner's decisions on the browser in `docs/desktop-browser.md` section 15, and the decisions taken while building the macOS browser on 12 October 2026, when the owner asked for the build to be completed without questions. The build's decisions are marked *(build)* below. They are listed with their reasons in `docs/macos-build-decisions.md` (D1 to D27) and await the owner's review (issue #162). The behaviour was built before these entries were written, against REQ-DOC-004; they were recorded on 12 October 2026 to restore the rule.

**E.63 — Web-native sites: what a site is, where its descriptor lives, and what its files may hold.**
1. **A site is a web origin that serves teletext** (REQ-WEB-015). *(build, D7)* The browser treats every origin, that is scheme, host and port, as its own site. The concept named the registrable domain; the origin is stricter, so more moves show the tuning screen (decision E.65), and it needs no list of public suffixes. This should be revisited if sites spread over several subdomains of one domain become common.
2. **The descriptor lives at `/.well-known/teletext.json`** (concept decision 13). A file at the site root, such as `index.json`, could collide with the site's own files. Typing the well-known address names the site itself.
3. **A site needs no descriptor.** *(build, D8)* When the well-known address answers 404 or 410, the site gets defaults, built as a descriptor so that REQ-WEB-002's checks apply to them: its name is the host, its initial page is 100, its pages are `/{mpp}.t42`, and its front door is `/index.t42`. Any other failure, such as a server error or an invalid descriptor, is reported and not replaced by the defaults, so that a broken site never silently changes meaning.
4. **Page files and a front door both work, with no setting** (concept decision 6). A typed number is looked for in the store, if the page is there and confirmed current (REQ-WEB-004), then in the page's own file, and is otherwise "not found".
   - *(build, D10)* A page's own file fills only pages that have been asked for, and enhancement pages, whose numbers are not decimal and which a page file may carry for its objects (item 5). Any other page in it is dropped by its header, so a page file cannot overwrite the rest of the site. The front door, pushed messages, form responses and local files fill every page they hold. `ttx`'s web mode follows the same rule, because it shares the code.
   - *(build, D9)* The front door is bounded by the page response limit of REQ-WEB-008, 256 kilobytes by default. A bundled site larger than that publishes page files instead.
   - *(build, D17)* A file or front door that does not hold the page asked for opens at its own first page, unless the viewer has already chosen a page.
5. **Enhancement dependencies are found both ways** (concept decision 7). A page's own file may carry the object and DRCS pages it uses. Otherwise, *(build, D26)* when a page is shown at Level 2.5 or 3.5, the decoder asks once for its magazine's Organization Table (page xFE), then for the object and DRCS pages that the table and the page name. Pages already stored are not asked for again, and these requests do not change which page is kept fresh.
6. **The descriptor gains aliases and a refresh interval** (concept decisions 3 and 13). A features list and a search page were considered and left out: nothing would read the features list, and a search page is an ordinary page that a site links to.
   - An **alias** maps one page number, never a range, to a target that is `name:mpp` or an address. *(build, D24)* A relative address is refused, an address in a descriptor fetched over `https` must itself be `https`, and a descriptor has at most 16 aliases.
   - **`refresh`** is a whole number of seconds. *(build, D24)* It is accepted from 5 to 86 400, and is used only when the service has no push.
7. **Sub-pages in addresses are not needed** (concept decision 5). An address leads to a page's file and shows its newest sub-page.

*Reason:* `docs/web-native-concept.md` section 2. A simple site is then a folder of files on any static host, and nothing needs declaring. The page-file rule keeps one page's file from rewriting another page of the site.
*Amended 12 October 2026 by decision E.70:* items 1, 2 and 3 are replaced. A site is now a channel, which is a folder; its descriptor is `channel.json` in that folder, not `/.well-known/teletext.json`; and its files have fixed names in the folder, with no page address template. Items 4 to 7 stand, applied to the channel's folder: the front door is the channel's `index.t42`, which every channel may have, with or without `channel.json`.

**E.64 — Links may lead to addresses; page numbers and runs of text may be links.**
This amends decision E.36 and Appendix E.
1. **Addresses in pages** (concept decision 1, the owner's). A Fastext key, a page number or a run of text may link to an address as well as to a named service, and no setting forbids it. A site that publishes other people's content removes their links before publishing, as on the ordinary web.
2. **One rule tells targets apart:** a target containing `://` is an address. *(build, D23)* An address must be `http` or `https`, printable ASCII without spaces or commas, and at most 256 bytes; anything else makes the link invalid. `weather:300`, which has no `//`, is always a named service.
3. **Two new TTI lines.** `LK,mpp,target` links a page number shown on the page, and `LT,row,column,length,target` links a run of text on rows 1 to 23. *(build, D23)* A page has at most eight of each. The TTI writer keeps all three link lines.
4. **Which link a typed number follows** (concept decision 9): the current page's own link for that number, otherwise the descriptor's alias for it, otherwise the site's own page with that number. The most specific link wins, so what the viewer sees on the page is what typing the number does. *(build, D23)* The decoder checks the page's own link; the browser's interface checks the alias when the decoder asks for the page.
5. **The page checker warns** when a page links a number that it does not show.
6. **Every link to another site goes through the tuning screen** (decision E.65). An address written in a page is marked there as not declared by the site.

What stays from decision E.36: the four coloured keys and Index remain the Fastext model, row 24 is drawn by the receiver from the link data, and pages never draw link markers. Text links look exactly as the author drew them until the viewer asks to see them (decision E.66).

*Reason:* `docs/web-native-concept.md` sections 1, 3 and 5. Sites without a descriptor can link elsewhere only by writing addresses, and the tuning screen, not a ban, is the safeguard.

**E.65 — Leaving a site: the tuning screen, and row 0 on every page.**
1. **The browser owns all of row 0 on web sites** (concept decision 2, the owner's). Header text in a web page file is ignored. *(build, D13)* In the browser, row 0 is drawn by the interface on every page, whatever its source: the top 20 pixel lines of every frame a decoder sends are replaced across their whole width.
   - **On web sites,** row 0 shows the receiver's status area, the site's name and the clock. The clock stays while the name fits in columns 8 to 31, and gives way when it does not (concept decision 10). A name too long for the row is shortened from the left and marked with `<`, so the end of the host, which names the site, always shows.
   - **For files and live feeds,** row 0 shows the status area and the broadcast's own header text, without the browser's clock.
   - **A decoder's reported page digits** are accepted only as three hexadecimal digits or `-`; anything else is shown as `---`.
2. **Row 0 shows what the keys are doing** (concept decision 12): `P` and the page digits while browsing, the digits so far while one is being typed, `FORM` while a form has the keys, `LINK` in page catching, and `GO?` on the tuning screen. A page over plain HTTP shows the red marker and `!` of REQ-SEC-005.
3. **The tuning screen** (concept decision 8). Before any page from another site is shown, the browser clears rows 1 to 24 and shows the destination's host in double height, the full address, whether the connection is encrypted, and how the viewer came to be going there, with row 0 showing `GO?` and the destination. It stays inside the page area, with row 0 above it (the owner's instruction of 11 October 2026). Only the receiver accepts the confirmation, so a fake screen drawn by a page leads nowhere.
4. **When it is shown** (concept decision 4, the owner's). *(build, D12)* It is shown for a typed address, including in a new tab; a bookmark or history entry; a restored tab; every link to another site; and every redirect to another site. It is skipped only when going back or forward to a site confirmed earlier in the session. A link to a site already confirmed still asks: the concept leaves this open, and asking is the safer reading.
5. **Redirects to another site are not followed.** *(build, D11)* In the browser the web module reports a redirect to another origin instead of following it, and the browser shows the tuning screen for the new destination. An upgrade from `http` to `https` on the same host is not another site. `ttx` keeps following redirects as REQ-WEB-003 says, and form submissions keep their existing rule, under which a redirect to another origin makes the submission cross-site for cookies.
6. **Only the interface confirms.** *(build, D14)* A form submission's confirmation is a bar outside the page that the viewer answers. A confirmation request from a decoder is dropped, never passed on.
7. **The receiver's own pages** *(build, D15)*, which are the start page, the keys page, the tuning screen, error pages and the loading page, go through the same Level 1 decoding as broadcast pages but are built in the interface and never pass through a decoder.

International domain names are shown in their ASCII form. Flagging names that imitate other domains (concept section 6) is not built; Appendix D.2 keeps it open.

*Reason:* `docs/web-native-concept.md` sections 4 and 6. A 40 by 25 page can draw anything in rows 1 to 24, including a fake tuning screen; row 0, which no page can write, is what makes the real one recognisable on every port. The rule for viewers is short: the site is whatever row 0 says.

**E.66 — Page catching and showing links.**
Concept decision 11 settled the model: the selected link has its colours swapped, its destination appears in row 0, a key underlines every link, and the pointer highlights links. *(build, D25)* The details:
- **Turning it on.** Enter turns page catching on at the first link in reading order. Links are the page numbers printed on rows 1 to 23 that lead somewhere, and the runs of `LT` lines.
- **Moving.** Right and Down move forward; Left and Up move back. Enter follows the selected link.
- **Turning it off.** Escape or Delete turns it off; any other key turns it off and then does what it does.
- **Row 0** shows `LINK` and the destination as the decoder reports it, written only after the receiver's `LINK`, so it can never be mistaken for the tuning screen's `GO?`.
- **Pages with side panels** are not caught, because their text columns are not the grid's.
- **The pointer.** A page number under the pointer names its destination in the window's status line, and a click follows it, as it follows a Fastext prompt or an `LT` run.
- **Show links.** `l` underlines every link with the receiver's own underline, and `l` again stops. The concept names no key; `l` was chosen as the letter for "links".

*Reason:* `docs/web-native-concept.md` section 5 item 2. The cursor stops only on real links, so text a page draws to look like a selected link can never be selected.

**E.67 — The browser's keys.**
The keys follow concept decision 14: Page Up and Page Down are Back and Forward, End is Index, Insert is Hold, Delete is Cancel, every function has a plain letter key, `K` shows the keys, and Escape never quits. *(build, D18)* The windowed browser differs as follows:
- **`q` does not quit;** Command-Q does, as in every macOS program, so a stray key cannot close the browser. `ttx` keeps `q`.
- **Command-Left and Command-Right** go back and forward, as do Command-[ and Command-].
- **Command-Shift-C** copies the page's text.
- **`G`** switches the glyph set (decision E.68).
- **`.` for sub-page entry** is not built.

*Reason:* `docs/web-native-concept.md` section 7, and the conventions of macOS programs.

**E.68 — The browser program.**
1. **One Rust program on egui for all five platforms** (the owner's decision of 11 October 2026, `docs/desktop-browser.md` section 15 decisions 2 and 9): Windows, macOS, Linux, Android and iOS. Slint is not used.
2. **All browser behaviour is in a crate with no operating-system access** (design decision 11, built in *(build, D5)*): `neoteletext-browser` holds address parsing, sites, history, the tuning screen's rules and page, row 0, the receiver's pages and the formats of saved files, and `xtask check-io` treats it as a library crate. `neoteletext-gui` holds the window, the tabs' processes and storage, and its program is called `neoteletext`.
3. **Processes.** *(build, D1)* Each tab has its own fetcher process and decoder process. One decoder per site, shared by tabs, is later work (design decision 1); the messages are the same either way. The interface routes every frame between them and reads all of it as untrusted.
4. **The decoder's loop** *(build, D6)* is a new module, `neoteletext_app::decoder`, ported from `ttx view`'s. Moving `ttx view` onto it is later work; the shared pieces were moved into `neoteletext-app` so that they are not duplicated.
5. **Files.** *(build, D16)* Files open from a typed path, a `file://` address, or by dropping them on the window. T42 files are read up to 256 megabytes and anything else as TTI up to 256 kilobytes. The other formats `ttx` reads are not opened by the browser yet, and there is no file dialog.
6. **Fonts and glyphs.** *(build, D19)* The window's own text uses the system's fonts, read at start; egui's bundled fonts are left out because their licences are not on REQ-GEN-017's list. Pages are always drawn with the teletext glyph sets, and the viewer can choose the project's own glyphs, Bedstead, or both side by side.
7. **Menus are inside the window** *(build, D20)*, not in the macOS menu bar, which would need another library.
8. **Saved data** (design decision 10, built in *(build, D21)*). Bookmarks, history, open tabs and settings are kept in the platform's application data directory, written only by the interface process, each written whole by renaming a temporary file. History keeps 30 days. Confirmed sites are kept for the session only, and restored tabs reopen at their tuning screens.
9. **Cookies** *(build, D22)* live in each tab's fetcher, in memory only. Leaving a site in a tab ends that tab's session with it, and saving cookies between runs stays off (REQ-SEC-013).
10. **Screen readers** *(build, D27)* are given the page's text as the page area's label, besides egui's own description of the window.
11. **Libraries** *(build)*. `eframe` and `egui` 0.36.2, with the features `wgpu`, `accesskit`, `x11` and `wayland` and without default fonts; `chrono` 0.4.45 for the local time of row 0's clock; and, for tests only, `egui_kittest` 0.36.2 and `image` 0.25.10. All are MIT or Apache-2.0. egui brings in two crates licensed BSL-1.0, `clipboard-win` and `error-code`, used only on Windows; `deny.toml` allows that licence for those two crates only. The owner is to confirm this exception (issue #145).

The design's other decisions (section 15: decisions 3, 4, 6, 7, 8, 12, 13 and 15 to 18) were recommendations still awaiting the owner (issue #150), and Appendix D.2 lists them.

*Reason:* `docs/desktop-browser.md`. Keeping every browser decision in a crate with no operating-system access lets one set of tests cover all five platforms, and keeps the shells thin.

**E.69 — Confining the browser's helper processes.**
1. **The browser runs privilege-separated web mode** (REQ-SEC-018) for every tab, with the decoder holding no rights and the fetcher holding only the network.
2. **On macOS the helpers are confined by Seatbelt profiles applied with `/usr/bin/sandbox-exec`.** *(build, D2)* Design decision 6 recommended XPC services, which need an app bundle with nested services and Objective-C or `unsafe` code, and only the C interface crate may contain `unsafe` code (REQ-SEC-015). `sandbox-exec` needs neither, works for both the bare program and the bundle, and confines the helper before its first instruction. The profiles deny everything, then allow only what was measured to be needed on macOS 15.6:
   - **the decoder:** running itself, reading the system libraries, its own file and the root directory, and reading `sysctl` values; no files, no network and no other programs;
   - **the fetcher:** the same, plus outgoing connections, name resolution through the system's resolver, and the resolver's files.

   `sandbox-exec` is deprecated by Apple but present and working on macOS 15.6. If it is removed, XPC services are the replacement.
3. **On Linux** each helper applies its own Landlock and seccomp sandbox, as `ttx` does (REQ-SEC-017).
4. **The interface process itself is not sandboxed.** *(build, D3)* The App Sandbox needs a signed bundle with entitlements, which is part of the release work and needs membership of the Apple Developer Program (design decision 8).
5. **The WebAssembly wall around the decoder** (design decision 12) **is not built.** *(build, D4)* On macOS the decoder already runs in a process with no file or network access. The interpreter is the only wall available on iOS; it is tracked in issue #160.

*Reason:* Security overrides every other requirement (decision E.32), and the helpers parse untrusted input. A test on every run proves the confinement, by trying to read a file, connect and start a program under each profile (REQ-SEC-026).

**E.70 — A channel is a folder.**
Decided by the owner on 12 October 2026, who asked for solutions that are opinionated and easy to understand and to build. The details below were written to that instruction. This decision replaces items 1, 2 and 3 of decision E.63 and refines its items 4 and 6, and the browser's "site" becomes the "channel" throughout. The reasoning, the alternatives considered and the measurements behind it are in `docs/channels.md`.

1. **A channel is a folder on a web server.** Its address is an `http` or `https` address whose path ends in `/`, such as `https://example.org/` or `https://user.github.io/ceefax/`. There is no other kind of web-native site. One server may hold any number of channels, in different folders. A folder inside a channel is a different channel, and inherits nothing from the folder that contains it.
2. **A channel is identified by its origin and its folder.** Two addresses are in the same channel only when they have the same origin (REQ-WEB-015) and the same folder path, compared exactly, letter case included, since paths on the web are case-sensitive. The identity decides the tuning screen, the name in row 0, cookies, history, bookmarks, the confirmed channels of a session, where forms may submit, and which redirects are followed. The origin stays the unit for the protections that concern the server as a whole: the plain-HTTP marker, no downgrade, and the local-network checks.
3. **Folder paths are restricted, so that two spellings can never name one folder, or one spelling two.** Each segment of a channel's folder path is 1 to 64 characters from the unreserved set of RFC 3986 section 2.3 (ASCII letters, digits, `-`, `.`, `_` and `~`), and is not `.` or `..` or made only of dots. Percent-encoding is not allowed in a folder path at all. A folder has at most 8 segments and its path at most 256 characters. An address that breaks these rules is refused, never repaired. Hosting services' own naming fits inside this set: GitHub user and repository names, and `~user` folders.
4. **Every file of a channel has a fixed name, in the channel's folder.** There is no page address template for channels, and no setting for file names.

   | File | Holds | Required |
   |---|---|---|
   | `NNN.tti` or `NNN.t42` | The page `NNN`, written as three hexadecimal digits with `A` to `F` in capitals, such as `100.tti` or `1FE.t42` | Only the initial page |
   | `index.t42` | Any number of pages at once: a bundle, a capture or an archive | No |
   | `channel.json` | The channel's settings | No |
   | `contents.txt` | The channel's list of pages and sections | No |

5. **Opening a channel is three requests, in order.** First `channel.json`: a valid file gives the settings, 404 or 410 means the defaults, and any other answer is a failure that stops the channel opening. Then `index.t42`: its pages are stored, 404 or 410 means there is no bundle, and any other failure is reported and the channel opens without it. Then the initial page, from the store or from its own file. The defaults are: initial page 100, no push, no linked channels, no aliases, no refresh, the default character set region, and the name shown being the channel's host and folder.
6. **A page is found by one fixed rule.** A page in the store and confirmed current is shown from the store (REQ-WEB-004). Otherwise the browser asks for `NNN.tti`, and on 404 or 410 for `NNN.t42`, and on 404 or 410 again the page is "not found". The browser tries first the extension that last succeeded in that channel, which changes only the number of requests, never the result. A page's own file fills only pages that have been asked for, and enhancement pages (decision E.63 item 4).
7. **`channel.json` is the service descriptor of REQ-WEB-002, with three differences.** Every member is optional. `page_template` is not allowed in it, and a file that has one is refused, because a channel's files have fixed names. A linked channel is given by its folder's address in a member `channel`. The descriptor is no longer looked for at `/.well-known/teletext.json`: RFC 8615 reserves `.well-known` for the root of a server, which is what kept a site from living in a folder.
8. **`contents.txt` is the only map of a channel.** It is a list, one entry per line, of three kinds: a section heading, a page of the channel, and another channel. The receiver draws it as a teletext index. Nothing else is used to learn a channel's structure. The browser never lists folders, never probes page numbers and never crawls. A channel without `contents.txt` has a contents view listing the pages the tab has shown on the channel.
9. **The author's tool writes the contents.** `ttx contents FOLDER` reads the author's own folder of page files and writes a `contents.txt` for it. The only crawling is done by the owner, of their own files, before publishing.
10. **Static and dynamic servers are the same to the browser.** "Folder" is a rule about addresses, not about files on disk. A server program may generate any channel's files on request, and any number of channels from a database. The browser sees only addresses and responses.
11. **Media types are generic in practice, so the content decides.** No teletext media type is registered with IANA, and none is in `mime-db` or the common `mime.types` files. Measured on 12 October 2026, raw.githubusercontent.com serves `.tti` as `text/plain; charset=utf-8` and `.t42` as `application/octet-stream`, jsDelivr serves both as `application/octet-stream`, and NMS Ceefax's feed serves `.tti` as `text/plain`. The extensions are also used by other formats: `.tti` by Template Toolkit and `.t42` by PostScript Type 42 fonts. A channel's files are therefore accepted with this project's own types and with these generic types, and each file must then pass the check for the format its name says. Any other type, including `text/html`, is refused, which also catches hosts that answer a missing file with an HTML page and status 200.
12. **The channel is a boundary for everything a site was.**
    - **Cookies** are kept per channel: a cookie set by a response from inside the folder belongs to that channel, a `Path` outside the folder is narrowed to the folder, and the channel's cookies are sent only with requests inside the folder, with its push connection, and with its form submissions.
    - **Forms** may submit only to an address inside the channel's folder, or on an origin that `channel.json` lists in `form_origins`.
    - **The push connection** named in `channel.json` may be anywhere on the channel's origin, because servers usually serve push from a fixed path.
    - **Redirects** of the channel's own requests are followed only within the folder. A redirect from `http` to `https` keeps the channel when the host and folder are the same and both ports are the schemes' defaults. Any other redirect is another channel, and shows the tuning screen.
    - **Links** to a page file in the same folder stay in the channel. Every other address is another channel.
13. **Row 0 names the channel by its host and folder,** such as `user.github.io/ceefax`, never by the name the channel gives itself. A name too long for the row keeps the last segment of the folder and as much of the end of the host as fits.
14. **Descriptor services keep working.** A descriptor given by an address whose file name is not `channel.json` is a service of REQ-WEB-002, with its page address template. Its channel identity is the folder that holds the descriptor, and its contents are the `contents.txt` in that folder. `ttx` keeps opening services this way. New channels are not told about templates.
15. **What is given up, deliberately.**
    - Collections whose files have other names, such as SPARK's `P100.tti`, are not served as they are; they are converted once into a channel folder, or read through a descriptor service (item 14).
    - A broadcaster with several channels on one server shows the tuning screen between them, since each is a channel.
    - A typed address without a trailing slash whose last segment contains a dot is taken as a file name, and is refused unless it is one of the channel's file names. A folder whose name contains a dot must be typed with its trailing slash.
    - There is no bundle in TTI form: `index.t42` is the only bundle. An author of TTI pages publishes page files.

*Reason:* The site as an origin could not hold a channel in a folder, which is how the easiest free hosts publish (a GitHub Pages project site is `user.github.io/project/`), and it made strangers on one shared host one site. A channel as a folder works on every static host with no configuration. Fixed names, one request per question, no inference and failing closed keep it simple to learn, to build, and to make safe.

*Amended 12 October 2026 by decision E.73:* a folder whose four requests all show that it holds no channel leads to discovery (REQ-WEB-028), instead of a failure.

**E.71 — Capability features within Levels 2.5 and 3.5, and the modern-classic profile.**
Recorded on 12 October 2026, on the owner's instruction to add teletext feature flags so that capabilities can be compiled out, with this "modern-classic" profile: keep the 40 by 24 grid, the classic eight-colour palette by default, contiguous and separated mosaics, no proportional spacing, non-spacing colour and character attributes, the additional international character sets, monochrome downloadable characters, reusable page objects and bold characters; leave out the 32-colour palette, italic characters, side panels, multicolour downloadable characters and animation. This amends rule 2 of decision E.61: within the enhancement area, there is one feature for each capability the owner named, not one for the whole area.

1. **Eight features of `neoteletext-core`, on by default.** Each needs `level25`, and the decoding and the drawing of its capability are both behind it, so a build without it contains neither:

   | Feature | Capability | When it is compiled out |
   |---|---|---|
   | `objects` | Active, Adaptive and Passive objects, and default objects (area OBJ) | Object invocations and default objects are ignored; a page's own enhancement data still applies |
   | `drcs` | Dynamically redefinable characters in mode 0, 12 by 10 by 1 (area DRC) | Every DRCS character is missing: a space in the background colour, counted as missing (REQ-DRC-007) |
   | `drcs-multicolour` | DRCS modes 1, 2 and 3 (clause 14.2); needs `drcs` | Those characters are missing, as they are below Level 3.5 |
   | `bold` | Bold characters (Table 28, Font Style) | The style is not applied |
   | `italic` | Italic characters (Table 28, Font Style) | The style is not applied |
   | `side-panels` | Side panels (clause 12.5) | There are no side panels; a page is always 40 columns, and anything placed in a panel is outside the page, as for a panel that is not enabled |
   | `colours32` | The Colour Map's CLUTs 2 and 3, and every redefinition of a CLUT by X/28/0, X/28/4, M/29/0 or M/29/4 (clause 12.4) | No CLUT is redefined, and CLUTs 2 and 3 are copies of CLUTs 0 and 1, so a page shows at most the sixteen default colours of CLUTs 0 and 1, and the eight classic colours stay the default |
   | `animation` | Additional Flash Functions: the flash modes, the fast rates and phases, incremental and decremental flash (Table 29) | The triplet is ignored; the Level 1 flash, slow and normal (decision E.48), is unchanged |

2. **The profile is a feature too.** `modern-classic` turns on `level15`, `level25`, `render`, `original`, `viewer`, `forms`, `objects`, `drcs` and `bold`. A port uses it with the default features off.
3. **What is not a feature.**
   - The 40 by 24 grid, the eight-colour default and the mosaics are Level 1, which every build has.
   - Non-spacing attributes are the enhancement area itself, the `level25` feature.
   - Proportional spacing is never laid out (Part G item 1, decision E.50).
   - The additional international character sets: their glyphs are one generated table of every set (decision E.62), and their code tables are small, so compiling them out needs the glyph generator to write a table per script. That is later work.
4. **Gate builds.** `cargo xtask features` adds the core with Levels 1.5 and 2.5 and every capability off, and with `modern-classic`. The tests of a capability need its feature; a test file checks that each capability compiled out is ignored as item 1 says.

5. **Measured on 12 October 2026** with `cargo xtask size`, on the linked Cortex-M0 image (`thumbv6m-none-eabi`, `opt-level = "z"`, link-time optimisation) of a receiver with Level 1.5, the renderer and the viewer: 65,758 bytes with every capability of Levels 2.5 and 3.5, and 62,894 bytes with the modern-classic profile's, 2,864 bytes or about 4 percent less. Most of the enhancement processor is the non-spacing attributes and the walk, which the profile keeps.

*Reason:* Teletext's distinctive look comes from its constraints: a fixed 40 by 24 grid, eight saturated colours, block mosaics, attributes set within the text, and no proportional spacing or pictures. The profile keeps the capabilities that preserve those constraints, and those that make pages more usable without changing them: non-spacing attributes, more languages, monochrome custom characters in the grid, reusable objects and bold for legibility. It switches off, without removing, the capabilities that would dilute the style: a redefinable palette of 4,096 colours, italics, a page wider than 40 columns, per-pixel colour characters that tile into bitmap images, and animation. Each remains implemented and tested, and a port that needs one compiles it in. One feature per named capability keeps each one switchable alone, while the profile names the recommended choice once. `docs/capabilities.md` explains the mechanism.
**E.72 — Fonts: Bedstead as the one bundled font, and every other font a 12 × 10 BDF loaded when it is named.**
Decided by the owner on 12 October 2026. This supersedes decision E.62 and amends decisions E.25 and E.34 (REQ-WEB-014 now permits one kind of third-party request: a font named with its checksum). `docs/font-security.md` is the security analysis of this decision.

1. **One bundled font: Bedstead 3.261.** It is the receiver's own font and the default for every page. It keeps the 5-dot by 9-line form and the SAA5050 character rounding (REQ-RND-010, REQ-RND-012). The project's original glyph set of decision E.62, its drawing rules and its checks are removed; they remain in the git history up to commit 9dc932d. The glyph switch between sets is removed.
   - *Licence:* CC0 1.0 for the Bedstead authors' work, and section 55 of the United Kingdom Copyright, Designs and Patents Act 1988 for the SAA5050 designs it reproduces, which permits making and using typefaces from them because the SAA5050 was marketed in 1982 or earlier. Debian files Bedstead as non-free because uses other than as a typeface might infringe (`third_party/bedstead/debian-copyright.txt`). The owner accepts that position.
   - *Gaps:* Bedstead has no Arabic, and lacks seven Latin national characters and two G2 characters. Where a character can be composed from a base letter and a spacing mark (decision E.26), it is; otherwise it is drawn as the visible placeholder of REQ-CHR-004, until a loaded font supplies it.
2. **Every other font has one form: a BDF file of 12 × 10 cells.** 12 × 10 is the cell of the standard's own downloadable characters (EN 300 706 clause 14.1). A font is drawn exactly as given, as monochrome DRCS characters are, without character rounding: each line twice in the 12 × 20 cell, and enlarged for double height, width and size. The profile is REQ-FNT-002; anything else is refused.
3. **Fonts are loaded when they are named, and only then,** from any `http` or `https` address: by the user, by the channel, or by the page. The repository ships only Bedstead. The owner's collection of fonts is kept in a separate repository and published as font libraries.
4. **Who chooses.** The page's font, then the channel's, then the user's chosen font, then Bedstead; and a user setting makes the user's font win over every channel and page. Row 0, the tuning screen, the contents view and every page the receiver draws itself always use Bedstead. Characters missing from the font in use come from the user's font, then from Bedstead, then the placeholder.
5. **Font libraries.** A folder with a fixed-name index, `fonts.txt`, lists any number of fonts, each with its SHA-256, so fonts can be found without knowing their names.
6. **The cache.** Fonts are stored by content, named by their SHA-256; fonts named by a channel or a page are kept apart per channel; at most 4 MB in all and 256 KB per font, the least recently used removed first; checked again whenever loaded; written only by the interface process.
7. **The converter.** `ttx font convert` turns a BDF of another size into the 12 × 10 form, offline, only where the fit is exact and reversible: doubling every pixel's width, adding the blank top line where the SAA5050 leaves it, or halving a 12 × 20 font whose line pairs are identical. Anything else is refused.
8. **Not fonts:** mosaics and G3 characters are drawn from geometry whatever the font (REQ-CHR-005, REQ-RND-037), and DRCS characters come from the page.
9. **Features.** The core's `original` feature is removed; `bedstead` becomes a default feature and part of the modern-classic profile (decision E.71). The fonts of items 2 to 7 are the browser's, and `ttx`'s converter.
10. **Phases.** Item 1 and the converter are Phase 1. Loading, choosing and caching fonts follow the browser: Phase 1 on macOS and Linux, Phase 2 on Windows, Android and iOS.

*Reason:* Bedstead is the most complete openly licensed teletext font and the only one of the 317 collected text fonts with a licence the project can ship that also fits the receiver. One small, exact font form keeps the reader that faces the network simple, keeps every font in the grid and look of teletext (decision E.71), and lets the 151 collected fonts that fit be used without the project distributing them.


**E.73 — Discovery: an ordinary web address leads to a channel by pointing to it.**
Decided by the owner on 12 October 2026, who asked whether teletext pages could be carried inside ordinary web responses, in the HTTP headers or in the HTML `<head>`, and then asked for this design. This amends decision E.34 (REQ-WEB-014 handles one more response header, `Link`, and scans the start of an HTML response for one element) and decision E.70 item 5 (a folder that holds no channel leads to discovery instead of a failure). `docs/web-discovery.md` is the design and `docs/web-discovery-security.md` its security analysis.

1. **Web addresses point; they never carry pages.** A web page announces a teletext edition with a pointer to a channel, or to a page of one, and the pages still arrive as a channel's files (REQ-WEB-019), through every check they pass today (REQ-WEB-027). These were considered and refused:
   - *Pages in an HTTP header.* A page is about 1 kilobyte as T42 and 1 to 3 kilobytes as TTI, more with sub-pages; proxies and content delivery networks commonly limit all of a response's headers to 4 to 8 kilobytes, static hosts such as GitHub Pages cannot set a header per file, and every ordinary visitor would download the page without using it.
   - *Pages embedded in the HTML,* in a data block such as `<script type="text/x-teletext-tti">`. It would need an HTML reader that faces the network, the page would belong to no channel and so have no place for its cookies, forms and links, and its two copies, the HTML and the teletext, would drift apart.
   - *`HEAD` requests.* A `HEAD` response has only headers, so it can carry only a `Link` header, which a `GET` brings in the same round trip. Servers often answer `HEAD` differently from `GET`, and hosts that cannot set headers can only point from the HTML, which `HEAD` never returns.
   - *A DNS TXT record,* which the transport of REQ-WEB-001 cannot look up, and *a file under `/.well-known/`,* which belongs to the root of a server (decision E.70 item 7) and would add a second mechanism for the same thing.
2. **Discovery runs only after the viewer has confirmed the address,** on the tuning screen as today (REQ-NAV-014), never before, and never for pre-fetching (REQ-SEC-013). It runs for:
   - a folder address that holds no channel: `channel.json` answers 404 or 410, or with a redirect that leaves the folder, or with an HTML page; and the bundle and the asked-for page are missing in the same way. The four requests of E.70 item 5 are made first, unchanged;
   - an address that is not a channel's: a file name that is not one of a channel's (`story.html`), or an address with a query.

   It never runs for a channel's own file names, which are explicit, and never for the address a pointer leads to: there is one step, never a chain.
3. **One request, with the redirects it needs.** The browser asks for the address with `GET`, `Accept: text/x-teletext-tti, application/x-teletext-t42, text/html;q=0.5`, no cookies and no `Referer`, and stores no cookie from the answer. It follows at most 5 redirects, to any origin, each checked for the local network and for downgrade. It reads the headers and at most the first 8,192 bytes of the body, then closes the connection.
4. **Three ways to point, tried in this order:**
   1. *Content negotiation through a redirect.* When the final address is a page file of a channel and the answer is a page in one of the accepted media types, that is the page. A dynamic server that sees the teletext types first in `Accept` answers `303 See Other` with the page file's address. A teletext answer at any other address is refused: pages live in channels.
   2. *The `Link` header* of RFC 8288, with `rel="alternate"` and one of the two teletext media types as its `type`.
   3. *A `<link>` element* with the same `rel` and `type`, within the first 8,192 bytes of an HTML answer. HTML already requires `<meta charset>` within its first 1,024 bytes for the same reason: a reader can decide from a fixed prefix. Measured on 12 October 2026, the head of the BBC News home page is 198,533 bytes long and the Guardian's 741,595, with its alternate link at byte 741,499; a pointer is therefore looked for only where the publisher has been told to put it, not wherever a head may end.

   The first pointer that passes REQ-WEB-030 wins; the others are ignored.
5. **What a pointer may lead to:** a channel's folder or one of its files, as the address bar takes them (REQ-WEB-023, REQ-BRW-002), and nothing else. A pointer from an `https` answer to an `http` target is refused (no downgrade, REQ-SEC-007); a target in the local network is refused unless the configuration allows it (REQ-SEC-006). An address holding `&`, a byte outside printable ASCII, or anything the strict parser refuses is refused rather than decoded, so no HTML character references are ever read. A relative address is resolved against the final address of the answer, and refused if a `<base>` element comes before it.
6. **The scan is not an HTML parser.** It reads bytes, skips comments and the contents of `script`, `style`, `noscript`, `template` and `title`, reads the attributes of `link` elements, and stops at `<body`, `</head>`, the 8,192-byte limit, or the first byte it cannot read exactly. It runs in the tab's confined fetcher, and the interface receives only the address it found, which it parses again. Nothing of the HTML is shown, stored or used for anything else.
7. **The tuning screen names what was found.** When the target is on the same origin as the address the viewer confirmed, and in its folder or one inside it, it opens at once: the viewer has already agreed to go there. Otherwise the tuning screen is shown again, giving the target and the address that announced it, with the reason "announced by". A redirect to another origin during discovery, such as `example.org` to `www.example.org`, therefore costs the viewer a second key press; that is deliberate.
8. **The author's tool.** `ttx discover ADDRESS` makes the same request and reports the pointer it finds, every candidate it ignored and why, and where in the answer each was, so a publisher can test a page.
9. **Phases.** As the browser: Phase 1 on macOS and Linux, Phase 2 on Windows, Android and iOS. `ttx discover` is Phase 1.

*Reason:* The niche format's main obstacle is being found. One line in a web page's head, which every host can serve, makes an ordinary address lead to its teletext edition, per site or per story, while pages stay files in channels and every rule of decision E.70 is unchanged. Pointing, rather than carrying, costs a small, bounded scanner instead of an HTML reader, and keeps the web page and the teletext page each in one place. `rel="alternate"` is how web pages already announce their feeds and translations.

---

# Part F — Requirements

## F.0 Requirement areas

| Area code | Area |
|---|---|
| GEN | General, language and quality requirements |
| ARCH | Architecture and programming interface |
| ECC | Error-protection codes |
| PKT | Packet decoding |
| ACQ | Page acquisition and the page store |
| SVC | Broadcast service data (packet 8/30) |
| CHR | Character sets |
| L1 | Level 1.0 presentation |
| L15 | Level 1.5 presentation |
| L25 | Level 2.5 and 3.5 enhancement: attributes, colours, side panels and characters (section F.9a) |
| OBJ | Objects, object pages and the links to enhancement pages (section F.9b) |
| DRC | Dynamically redefinable characters (section F.9c) |
| NAV | Navigation and viewer functions |
| RND | Rendering and output |
| INP | Input formats |
| SLC | Software data slicer for raw video samples |
| PRF | Compatibility profiles |
| CLI | Command-line program |
| TST | Testing and verification |
| DOC | Documentation |
| FSM | State machines |
| WEB | Web-native page sources |
| BRW | The browser (section F.20) |
| FNT | Fonts (section F.21) |
| SEC | Security and sandboxing, which overrides every other area (section F.0.3) |

## F.0.1 Delivery phases

The requirements are delivered in two phases.

**Phase 1** is a complete, correct receiver. It covers every "shall" in this part that has no phase note:
- all the input formats of area INP except those listed below;
- Level 1.0 and Level 1.5 decoding;
- Level 2.5 and Level 3.5 decoding and rendering, brought into Phase 1 at the owner's request on 10 October 2026 (decision E.40);
- the default profile;
- the text, terminal and raster outputs in the 12 by 20 SAA5050 style;
- the basic software data slicer;
- the web-native module (area WEB), including the live WebSocket feed reader (REQ-INP-022);
- the command-line commands;
- the browser (area BRW, and REQ-NAV-013 to REQ-NAV-015, REQ-SEC-025, REQ-SEC-026, REQ-WEB-018 to REQ-WEB-020, REQ-WEB-022 to REQ-WEB-030 and REQ-TST-081) on macOS and Linux, added on 12 October 2026 (decisions E.68 and E.70);
- writing a channel's contents (REQ-CLI-008);
- the full test suite for all of these.

**Phase 2** holds the optional features and nice-to-haves. A requirement, or part of one, belongs to Phase 2 when it carries a *Phase:* note saying so. Phase 1 requirements carry no phase note. Phase 1 must not depend on anything in Phase 2.

| Requirement | What moves to Phase 2 |
|---|---|
| REQ-ACQ-006 | Request mode (SAA5246A-style acquisition slots). Phase 1 uses cache mode only. |
| REQ-NAV-004 | Automatic pre-fetching of linked Fastext pages. It depends on request mode. |
| REQ-PRF-002 | The "SAA5050" compatibility profile. |
| REQ-PRF-003 | The "SAA5246A" compatibility profile. |
| REQ-RND-003 | The 12 by 10 SAA5246A-style raster mode. It belongs to that profile. |
| REQ-PRF-004 | The alternative options: strict row setting, single Start Box, exact framing code, and ignoring out-of-order triplets. |
| REQ-L15-004 and REQ-L15-009 | The strict row-setting mode and the strict option for out-of-order triplets. The default behaviours stay in Phase 1. |
| REQ-L1-020 and REQ-NAV-007 | Page zoom, and momentary (SAA5050-style) reveal. Normal, mix and boxed modes and latched reveal stay in Phase 1. |
| REQ-CHR-004 and REQ-RND-010 | Glyphs for Cyrillic, Greek, Arabic and Hebrew. The code tables and Unicode mappings stay in Phase 1, and missing glyphs are drawn as visible placeholders. The glyphs of every G3 position moved to Phase 1 on 10 October 2026 (REQ-CHR-007 and REQ-RND-037). |
| REQ-RND-009 | Terminal double-height line escape sequences. |
| REQ-INP-016 | The writer for edit.tf and zxnet page addresses. The reader stays in Phase 1. |
| REQ-INP-017 | Detection of repeated double-height rows in BBC Microcomputer dumps. |
| REQ-INP-021 | The other formats: zvbi sliced files, TTA, ivtv, SMPTE RDD 8 and ST 2031, FAB and Cebra. |
| REQ-SLC-006 | The slow phase-tracking loop. The Phase 1 slicer uses a fixed bit period with phase taken from the run-in. |
| REQ-SLC-011 | High-frequency loss compensation and echo cancellation. |
| REQ-CLI-004 | The interactive terminal viewer. |
| REQ-SLC-015 | The fixed-point form of the slicer. |
| REQ-WEB-005 | The long-polling fallback. WebSocket and server-sent event push stay in Phase 1. |
| REQ-GEN-018 | The optional C interface crate. |
| REQ-WEB-017 | Device-bound sessions. |
| REQ-SEC-017 and REQ-SEC-018 | Self-sandboxing and privilege separation on Windows, OpenBSD and macOS. Linux stays in Phase 1. |
| REQ-SEC-019 | The example WebAssembly host. The WebAssembly builds stay in Phase 1. |
| REQ-SEC-021 | Guidance on hardware isolation for microcontroller ports. |
| REQ-RND-026 | The glyph line cache. |
| REQ-FNT-002 to REQ-FNT-009 | Loaded fonts on Windows, Android and iOS. macOS and Linux stay in Phase 1. |
| Area BRW, REQ-NAV-013 to REQ-NAV-015, REQ-SEC-025, REQ-SEC-026, REQ-WEB-018 to REQ-WEB-020, REQ-WEB-022 to REQ-WEB-030 and REQ-TST-081 | The browser on Windows, Android and iOS. macOS and Linux stay in Phase 1. |

Tests for a Phase 2 feature are delivered with that feature.

## F.0.2 Milestones within Phase 1

Phase 1 is large, roughly 100 to 150 person-days, so it is delivered in milestones. Each milestone ends with working, tested software that someone can use. This follows the lesson of the earlier jellytext project, which stalled before anything worked end to end (decision E.30).

| Milestone | Delivers | Main requirements |
|---|---|---|
| **M1 — First light** | Set up the layers of `ARCHITECTURE.md` and their enforcement (REQ-ARCH-011). Read T42 and TTI files. Decode Level 1.0 and Level 1.5 into the cell grid. Write text, terminal and PNG output, and show pages in the terminal as exact pixels. The exhaustive error-code tests, the Level 1 and Level 1.5 tests, and the real-broadcast corpus decoding. | ECC, PKT, ACQ (cache mode), CHR (Latin), L1, L15, FSM-003, FSM-004, FSM-006, FSM-007, RND-001, RND-002, RND-004 to RND-024 (the raster, text and terminal renderers, the glyphs, rounding and the image writers), RND-029, INP-002, INP-013, CLI-001 (`list`, `text`, `render`, `show`), TST-001 to TST-022 |
| **M2 — Web** | Fetch TTI and T42 pages over HTTP with caching, push over WebSocket and server-sent events, cookie sessions, and the zxnet live feed. | WEB-001 to WEB-009, WEB-013 to WEB-016, INP-022 |
| **M3 — Broadcast** | Digital television transport streams, Linux sliced data, and the remaining page formats. | INP-003 to INP-011, INP-014 to INP-020, SVC |
| **M4 — Raw video** | The software data slicer and its synthetic signal generator. | SLC, FSM-002, TST-060, TST-061 |
| **M5 — Forms, links and viewer** | Interactive forms, form input, links (Appendix E), navigation and the viewer state machine. | WEB-010 to WEB-012, NAV (including history, REQ-NAV-010), FSM-005 |
| **M6 — Levels 2.5 and 3.5** | Decode and render Presentation Levels 2.5 and 3.5 completely: the packets of both levels, storage of enhancement pages, the Colour Map, non-spacing attributes, row and screen colours, side panels, objects and default objects, DRCS, G3 glyphs, font styles and flash, with the presentation level setting and its option. The tests include the worked examples of the standard, authored streams with golden rasters, zvbi as oracle at both levels, real data, and fuzzing of every new parser. | PKT-010 to PKT-018, ACQ-016 to ACQ-022, L25, OBJ, DRC, CHR-007, CHR-011, RND-030 to RND-041, PRF-006, CLI-007, INP-023 to INP-025, FSM-011, FSM-012, TST-015 to TST-019, TST-023 to TST-025, TST-042 |
| **M7 — Browser** | The windowed browser on macOS and Linux: tabs, the address bar, channels in folders with and without `channel.json`, contents files and the contents view, links to addresses, aliases, the tuning screen, row 0 drawn by the receiver, page catching, files, saved data, and helper processes confined on both systems, tested end to end against local channels; and `ttx contents`. The browser was built on 12 October 2026 ahead of these requirements (decisions E.63 to E.69); channels (decision E.70) were specified, then built, the same day. | BRW, NAV-013 to NAV-015, SEC-025, SEC-026, WEB-018 to WEB-027, CLI-008, TST-081 |

M1 is expected to take 40 to 50 person-days. The order of M2 to M5 may be changed; each depends only on M1. M6 depends on M1, and on M2 for the captures from the live feed that REQ-TST-025 uses. M7 depends on M2 and M5.

Where Part E mentions a Phase 2 feature (the SAA5246A profile, or a strict option), the default behaviour it describes is the Phase 1 behaviour.


Every milestone ends with the security review of REQ-SEC-023.

---

## F.0.3 Security and sandboxing (SEC) — this area overrides all others

Every requirement in this area takes precedence over any other requirement in this document (decision E.32). The requirements are numbered and verified like all others, and are tested in every milestone.

**REQ-SEC-001 — Security takes precedence.**
Where any requirement, decision or profile conflicts with a requirement of this area, this area shall prevail. The conflict shall be recorded as a decision in Part E, and the other requirement amended to remove it. No option, profile or configuration may switch off a requirement of this area. The only exceptions are the explicitly configurable points named in this area, such as the list of permitted local-network services in REQ-SEC-006.
*Verification:* Inspection at each milestone's security review (REQ-SEC-023).

**REQ-SEC-002 — The threat model.**
The repository shall contain a threat model in `THREAT-MODEL.md`. It shall list:
- **Assets.** These include:
  - the host device and its resources;
  - the user's session cookies and form data;
  - the user's trust in what is displayed;
  - the user's local network;
  - the user's terminal;
  - the integrity of the build and release;
  - the user's privacy.
- **Attackers.** These include:
  - anyone able to supply input, such as a file, a stream, a web service or a live feed;
  - anyone on the network path;
  - the software supply chain.
- **Trust boundaries**, and the requirement of this area that protects each asset from each attacker.

It shall be reviewed and updated at the end of every milestone, and whenever a new input source or output is added.
*Verification:* Inspection.

**REQ-SEC-003 — All input is untrusted.**
Everything the software receives shall be treated as hostile, except the configuration supplied by the user or port. This covers:
- every packet, file, stream and sample buffer;
- every web response and pushed message;
- every service descriptor and form;
- every live feed;
- every cookie.

The robustness requirements (REQ-GEN-008) and the fuzzing requirement (REQ-TST-040) apply to all of these without exception. No source is trusted more because of where it comes from, including the zxnet live feed (REQ-INP-022).
*Verification:* Inspection of the threat model; the fuzzing targets.

**REQ-SEC-004 — No executable content, ever.**
Pages, forms, descriptors, pushed messages and every other received content are data only. Nothing received shall ever be executed, interpreted as a script or used to load code or plugins. Forms (Appendix C) are declarative, and shall remain so. No future requirement may add scripting, macros or loadable code driven by content, unless this requirement is first changed by an explicit, recorded decision of the project owner.
*Verification:* Inspection.

**REQ-SEC-005 — The receiver-owned status area cannot be forged.**
Row 0, columns 0 to 7, belong to the receiver alone. Nothing a page transmits can write there:
- neither the header's own bytes;
- nor Level 1.5 overwrites (a triplet addressing those columns shall be discarded);
- nor any other data.

Every security indicator shall be shown only there:
- a marker for an unencrypted connection;
- the origin of a form while it has focus;
- the warning before submitting over plain HTTP.

While a security indicator is active, it shall be shown even on pages with bit C7 (Suppress Header) set.
*Reason:* Every warning in this area depends on the user being able to believe the status area. A page that could draw there could draw a fake "secure" marker.
*Verification:* Test that pages writing to row 0 columns 0 to 7 by every route leave the status area unchanged.

**REQ-SEC-006 — Protecting the local network.**
The web module shall refuse to connect to these addresses:
- **Loopback:** 127.0.0.0/8 and ::1.
- **Private:** 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 and fc00::/7.
- **Link-local:** 169.254.0.0/16 and fe80::/10.
- **Shared address space:** 100.64.0.0/10.
- **Unspecified and multicast addresses.**
- **The IPv4-mapped IPv6 forms of all of these.**

The check shall be applied:
- to the address the connection actually reaches, which the transport reports after name resolution (REQ-WEB-001);
- again after every redirect.

This covers descriptors, pages, push connections and form submissions. Local-network services can be allowed only by naming them explicitly in the configuration.
*Reason:* A web-native client on a home network is a classic target for requests forged to reach routers and other devices behind it ("server-side request forgery").
*Verification:* Test against a list of blocked addresses and encodings, including redirects and names that resolve to blocked addresses.

**REQ-SEC-007 — No downgrade.**
Once a service has been reached over HTTPS, every later connection for that service shall use encryption, for the rest of the session and for as long as its cookies last. This covers pages, push, redirects and form submissions. A redirect from HTTPS to plain HTTP shall be refused. A descriptor fetched over HTTPS shall not name plain `http://` or `ws://` addresses; if it does, the descriptor shall be rejected.
*Verification:* Test.

**REQ-SEC-008 — Plain HTTP, as decided.**
Plain HTTP is allowed for reading, cookies and forms (decision E.32). The following safeguards are mandatory:
- **Indicator.** An unencrypted connection shall be shown by the security indicator of REQ-SEC-005 whenever the page on screen came from plain HTTP.
- **Warning before submission.** Before every form submission over plain HTTP, the viewer shall see a warning that the entered values will travel unencrypted, and must confirm it.
- **`Secure` cookies.** Cookies marked `Secure` shall never be sent over plain HTTP.

*Verification:* Test.

**REQ-SEC-009 — Protecting against deceptive forms.**
- **Show where the form goes.** While a form has focus, the origin it submits to shall be shown by the security indicator.
- **Confirm the first submission.** The first submission to any origin shall require explicit confirmation showing that origin.
- **No password fields.** Forms shall offer no field type for passwords or secrets (Appendix C has none).
- **Restricted destinations.** Forms may submit only to the origins of REQ-WEB-008.

*Reason:* Any page can draw "enter your password". The user must always be able to see where their input will go.
*Verification:* Test.

**REQ-SEC-010 — Safe text and terminal output.**
- **Allow-list only.** The text and terminal renderers (REQ-RND-008 and REQ-RND-009) shall emit only an allow-list of printable characters. The terminal graphics renderer (REQ-RND-029) writes no page text at all, only image data it encodes itself.
- **Stripped characters.** Every C0 and C1 control character and DEL shall be removed, as shall every Unicode bidirectional control (U+061C, U+200E, U+200F, U+202A to U+202E and U+2066 to U+2069). Mapped characters from the Hebrew and Arabic sets are laid out by the renderer, not by embedded controls.
- **Own escape sequences only.** The terminal renderer shall emit no escape sequences except those it generates itself, from a fixed list.
- **Safe file names.** File names derived from received content, such as a service name or page title, shall be reduced to a safe character set and shall never contain path separators.

*Verification:* Test with a corpus of injection attempts through every character set.

**REQ-SEC-011 — Strict HTTP handling.**
- **No compression.** The web module shall ask for uncompressed responses (`Accept-Encoding: identity`) and reject compressed ones, so that no response can expand beyond the size limits.
- **Unambiguous framing.** It shall reject responses whose framing is ambiguous:
  - both `Content-Length` and chunked encoding;
  - conflicting `Content-Length` values;
  - invalid chunk sizes;
  - obsolete header folding.
- **Header limits.** It shall limit the number and total size of headers.
- **Time limits.** Every connection and request shall have time limits.

*Verification:* Test with a corpus of malformed and ambiguous responses; fuzzing.

**REQ-SEC-012 — Bounded work and fair shares.**
The work done for each packet shall have a fixed upper bound. The work done for each network message shall be at most proportional to its size, within its size limit. No input may cause work that grows faster, such as quadratic searches of the page store. When several sources feed one decoder, each shall get a fair share of processing, so that a hostile source cannot starve the others.
*Verification:* Tests with worst-case inputs that measure operation counts; the performance tests of REQ-TST-050.

**REQ-SEC-013 — Cookies and privacy.**
- **Not saved by default.** The cookie store shall not be saved between runs unless the port enables it. When it is saved, expired cookies and session cookies (those with no expiry) shall never be written. The porting guide shall tell ports to store the saved cookies as secrets.
- **Wipeable.** The store shall have a function that erases it.
- **Private requests.** The web module shall send no `Referer` header, and shall accept and send cookies only as REQ-WEB-006 allows. Pre-fetching (REQ-NAV-004) shall fetch only from the service of the page being viewed.

*Verification:* Test.

**REQ-SEC-014 — Supply chain.**
- **Locked dependencies.** `Cargo.lock` shall be committed.
- **Checked dependencies.** Dependencies shall be checked by `cargo-deny` on every change, and for new security advisories on a daily schedule (REQ-GEN-017).
- **Pinned CI actions.** Continuous integration actions shall be pinned to exact commit hashes.
- **Verified toolchains.** Toolchains, including Espressif's fork, shall be verified by checksum.
- **Signed releases.** Each release shall publish a software bill of materials in CycloneDX form, and shall be tagged and signed.
- **Reproducible builds.** Release builds should be reproducible, and the check shall be automated.

*Verification:* Inspection; continuous integration.

**REQ-SEC-015 — Unsafe code confined.**
Only the optional C interface crate (REQ-GEN-018) may contain `unsafe` code. It shall be kept as small as possible. It shall be tested under the address sanitiser and fuzzed through its C entry points.
*Verification:* Inspection; tests.

**REQ-SEC-016 — No ambient authority in the library.**
The library crates shall not use operating-system facilities of any kind. The library crates are `neoteletext-core`, `neoteletext-formats`, `neoteletext-slicer`, `neoteletext-web` and `neoteletext-image`, except the file-writing functions of the image writers, which take an already-open writer. They shall not use:
- `std::fs`, `std::net`, `std::process` or `std::env`;
- threads;
- the system clock.

All of these come only through the platform interface (REQ-ARCH-009). The rule shall be enforced by clippy's `disallowed-methods` and `disallowed-types` lists and by a check of each crate's dependencies. Only `neoteletext-cli` and port code may use operating-system facilities.
*Verification:* Continuous integration check that fails if a forbidden item appears.

**REQ-SEC-017 — The command-line program sandboxes itself.**
After opening its inputs and outputs, `ttx` shall restrict itself so that it cannot open other files or start other programs, and cannot use the network except in web mode.

| System | Mechanism | Phase |
|---|---|---|
| Linux | Landlock for file access, and a seccomp system-call filter | 1 |
| Windows | An AppContainer or restricted token, and a job object | 2 |
| OpenBSD | `pledge` and `unveil` | 2 |
| macOS | Whatever the operating system allows a command-line program, which is weaker; the documentation shall say exactly what is and is not enforced | 2 |

If the sandbox cannot be applied, `ttx` shall refuse to run unless told to continue without it, and shall say why.
*Phase:* Phase 1 on Linux; Phase 2 on the other systems.
*Verification:* Tests on each system that try, after start-up, to open another file, start a program and open a network connection, and expect all three to fail.

**REQ-SEC-018 — Privilege-separated web mode.**
In web mode on hosted systems, `ttx` shall run as two processes:
- **A fetcher.** It holds the network rights and runs the web module's transport and TLS.
- **A decoder and renderer.** It holds no rights beyond its output.

They shall be joined only by a pipe carrying TTI pages or T42 packets and a small, fixed set of framed control messages, such as page requests and form submissions. Each process validates everything it receives from the other. The decoder process shall run under the sandbox of REQ-SEC-017 with no network and no file access.
*Phase:* Phase 1 on Linux; Phase 2 on the other systems.
*Verification:* Test that the decoder process cannot open network connections or files; fuzzing of the control messages.

**REQ-SEC-019 — WebAssembly isolation.**
`neoteletext-core` and `neoteletext-formats` shall build for `wasm32-unknown-unknown` and `wasm32-wasip1` (REQ-GEN-015). An example host shall run the decoder inside a WebAssembly runtime with limits on memory and execution time. Hosts can then isolate the decoder completely, and C programs can use it without the unsafe C interface.
*Phase:* Phase 1 for the builds; Phase 2 for the example host.
*Verification:* Continuous integration builds; test of the example host with a hostile input that must hit its limits cleanly.

**REQ-SEC-020 — Secure defaults.**
Every configurable security-related setting shall default to the safer choice. Examples:
- no local-network access;
- cookie store not saved;
- the security indicator on;
- confirmation of form submission on.

The documentation shall list each such setting and the risk of changing it.
*Verification:* Inspection; a test of the default configuration.

**REQ-SEC-021 — Guidance on hardware isolation for microcontroller ports.**
The porting guide shall explain how a microcontroller port can use a memory protection unit, where the chip has one (the Arm Cortex-M MPU, or RISC-V physical memory protection), to separate the network stack from the decoder. The library cannot enforce this (REQ-ARCH-010).
*Phase:* Phase 2 (whole requirement).
*Verification:* Inspection.

**REQ-SEC-022 — Sandboxed test tools.**
Third-party programs used as test oracles or for fuzzing shall run in containers with no network access and read-only inputs. These are zvbi, vhs-teletext, vbit2 and QTeletextMaker (REQ-TST-001). Downloaded corpus files shall be verified by checksum before anything reads them (REQ-TST-022).
*Verification:* Inspection of the continuous integration configuration.

**REQ-SEC-023 — Security process.**
- **Reporting.** The repository shall contain `SECURITY.md`, saying how to report a vulnerability privately and how reports are handled.
- **Reviews.** Each milestone shall end with a security review against the threat model, recorded in the repository.
- **Release gate.** A release requires:
  - every requirement of this area passing its tests;
  - no open security advisories against any dependency;
  - clean fuzzing for a stated minimum time;
  - the software bill of materials published.
- **Regression tests.** Every security fault found shall get a regression test.

*Verification:* Inspection.


**REQ-SEC-024 — Links are always shown whole and never disguised.**
A link may be followed only while the receiver is showing its complete prompt in row 24. That prompt is drawn by the receiver for TTI Link Extension pages (REQ-NAV-011), or is the transmitted row 24 for broadcast pages. A page cannot:
- hide, conceal, flash, overlap or partly draw a link prompt;
- disguise a link to another service as a local one;
- give a key a link it does not show.

On pages where row 24 is not displayed, the coloured keys shall do nothing. That includes newsflash and subtitle pages shown boxed, and page zoom of the top half when row 24 is off screen. The only link mechanisms are the coloured keys (decision E.36) and the page-number and text links declared by `LK` and `LT` lines (decision E.64). Text on the page that looks like a link, but is not declared as one, is only text: it cannot be selected, and nothing shows a destination for it.
*Amended 12 October 2026* (decision E.64): page-number and text links.
*Verification:* Test with hostile pages that try each of these.

**REQ-SEC-025 — The browser draws row 0 on every page.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
In the browser, all of row 0 belongs to the receiver on every page, not only columns 0 to 7 (decision E.65 items 1 and 2). The interface shall replace row 0 of every frame a decoder sends, across its whole width, with the row it draws itself:
- columns 0 to 6, the status area of REQ-SEC-005, showing the activity: the page digits, `FORM`, `LINK` or `GO?`, and the red marker for plain HTTP;
- on channels, the channel's host and folder from the location the interface holds, never from the page or from `channel.json` (decision E.70 item 13). The name is the host, then the folder path without its last `/`, such as `user.github.io/ceefax`; the root folder adds nothing. The clock is shown in columns 32 to 39 while the name fits in columns 8 to 31. A name too long for columns 8 to 39 is shortened: the folder's last segment is kept whole, with its own start cut and marked `<` if it alone is longer than 20 characters; segments between the host and the last are replaced by `/.../`; and the host is cut from the left, marked `<`, so that its end shows;
- for files and live feeds, the broadcast's own header text.

Page digits reported by a decoder shall be shown only when they are three hexadecimal digits or `-`, and as `---` otherwise. The pages the receiver draws itself (the start page, the keys page, the tuning screen, error and loading pages) shall be built in the interface process and never pass through a decoder.
*Reason:* Row 0 is how the viewer knows which site they are on, and what the keys are doing. A decoder handling a hostile page must not be able to write it.
*Verification:* Tests in which a decoder sends a forged row 0 and a hostile site draws a fake header and a fake tuning screen; the row shown must be the receiver's.

**REQ-SEC-026 — The browser's helper processes.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
The browser shall run each tab's network access and decoding in separate helper processes, as REQ-SEC-018 describes for `ttx`: a fetcher, which holds only the network, and a decoder, which holds nothing (decision E.69). The interface process shall route every message between them, read every message as untrusted, and check each against the strict, bounded framing of the pipe. It shall drop any confirmation request a decoder sends: only the interface asks the viewer to confirm, outside the page (decision E.65 item 6).

The helpers shall be confined before they read any input:
- **on macOS,** by Seatbelt profiles applied with `/usr/bin/sandbox-exec` when the helper starts. The decoder's profile allows no files, no network and no other programs; the fetcher's allows outgoing connections and name resolution only;
- **on Linux,** by the Landlock and seccomp sandbox of REQ-SEC-017, applied by each helper.

If a helper cannot be confined, it shall not run, unless the browser was started with `--no-sandbox`, which exists for debugging only.
*Verification:* A test on every run that, under each helper's confinement, tries to read a file, open a network connection and start a program, and expects exactly the failures above, and that the same attempts succeed unconfined, so the test measures something.

---

Each requirement below gives:
- a statement of what shall be implemented;
- where helpful, the reason and the source;
- the verification method.

---

## F.1 General, language and quality requirements (GEN)

**REQ-GEN-001 — Language and toolchain.**
All code shall be written in Rust, edition 2024.

- **Minimum supported Rust version.** The minimum supported Rust version shall be stated in the workspace manifest. It shall be kept at no more than three stable releases behind the current stable release, and shall be tested in continuous integration. On 10 October 2026, stable Rust is 1.99.0 (released on 1 October 2026), so the minimum shall be no older than 1.96.
- **Stable Rust only.** The library shall use only stable Rust features. The one exception is the Xtensa targets (REQ-GEN-015), which need Espressif's fork of the toolchain and `build-std` (below).
- **Pinning the Xtensa toolchain.** The Xtensa toolchain version shall be pinned in the repository and changed only deliberately. Espressif's fork versions 1.94 and 1.95 fail to compile release builds for Xtensa, an open issue (esp-rs/rust#277) as of 1 October 2026. Version 1.93 is known to work.

*Reason:* A single language and one pinned toolchain per target family keep builds reproducible across four instruction-set architectures.
*Verification:* Inspection; continuous integration builds with the stated minimum version and with the current stable version.

**REQ-GEN-002 — A `no_std` core with no allocation.**
The core crate (REQ-GEN-010) shall declare `#![no_std]`. It shall not use the `alloc` crate, file or console input and output, any clock, or any operating-system facility. The core comprises:
- the teletext constants;
- the error-protection codes;
- packet decoding;
- character tables;
- the state machines;
- acquisition and the page store;
- service data;
- Level 1.0 and Level 1.5 decoding;
- Level 2.5 and 3.5 decoding: the Colour Map, the enhancement processor, objects and DRCS (areas L25, OBJ and DRC);
- navigation;
- the cell grid, the cell raster and the row renderer.

The core shall build and run on every target of REQ-GEN-015, including microcontrollers with no operating system.
*Reason:* A teletext decoder fitted into small integrated circuits, so its software equivalent should fit into small microcontrollers.
*Verification:* Inspection; continuous integration builds of the core for every bare-metal target of REQ-GEN-015.

**REQ-GEN-003 — Caller-provided memory.**
The core shall keep all of its state in values whose storage the caller provides. Every configurable capacity shall be a const generic parameter or a caller-supplied slice. Examples are the number of stored pages, the number of sub-pages per page and the size of the cookie store in the web module. The size in bytes of every state type, for each documented configuration, shall be reported by a test and published in the documentation (REQ-GEN-009).
*Verification:* Inspection; a test that runs a decoder whose entire state is in a `static` value created at compile time.

**REQ-GEN-004 — No hidden global state.**
The core shall have no `static mut` items, no mutable statics behind interior mutability, and no lazily initialised globals. Several decoder instances shall be able to run independently in one program. Decoder state types shall be `Send` so that a port may move a decoder between threads or tasks. They need not be `Sync`, because a decoder is used from one thread at a time.
*Verification:* Inspection; a test that runs two decoders on different inputs and compares each result with a single-decoder run.

**REQ-GEN-005 — Determinism and caller-supplied time.**
The core shall produce the same output for the same input and the same configuration. Wherever timing matters, the caller shall supply the current time as a monotonic count of microseconds. This covers the page-completion timeout, flash phase, the reveal timeout and packet time stamps.
*Verification:* Test that runs identical input twice and compares the results.

**REQ-GEN-006 — Lint-clean and free of unsafe code.**
- **Formatting and lints.** The code shall be formatted with `rustfmt`, and shall build without warnings under `clippy`, including the `clippy::pedantic` group, with any exceptions listed and justified in the workspace configuration.
- **No unsafe code.** Every crate except the optional C interface crate (REQ-GEN-018) shall declare `#![forbid(unsafe_code)]`.
- **No panicking constructs in the core.** The core shall also deny these lints, so that no panic can be written by accident (REQ-GEN-008):
  - `clippy::indexing_slicing`;
  - `clippy::unwrap_used`;
  - `clippy::expect_used`;
  - `clippy::panic`;
  - `clippy::arithmetic_side_effects`.
- **Checked under Miri.** The core's tests shall also run under Miri, the Rust interpreter that detects undefined behaviour and out-of-bounds access, on a 64-bit, a 32-bit (`i686-unknown-linux-gnu`) and a big-endian (`s390x-unknown-linux-gnu`) target.

*Verification:* Continuous integration.

**REQ-GEN-007 — Portable bit handling.**
- **Unsigned types for bits.** Bit manipulation shall use unsigned integer types.
- **Explicit byte order.** Multi-byte values from files and streams shall be assembled explicitly in the byte order the format specifies, using `from_le_bytes`, `from_be_bytes` or shifts, never by reinterpreting memory.
- **No endianness assumption.** Every target in REQ-GEN-015 happens to be little-endian, including the ESP32's Xtensa cores, but the code shall not rely on that. The Miri run on a big-endian target (REQ-GEN-006) checks it.

*Verification:* Inspection; the big-endian Miri run.

**REQ-GEN-008 — Robustness against any input, without panics.**
No input shall cause any of the following:
- a panic;
- an out-of-bounds access;
- an arithmetic overflow;
- an unbounded loop;
- unbounded memory use.

This covers every packet, file, stream, sample buffer and network message, however malformed, truncated or hostile. Malformed input shall be reported through the error types and events of the library, and skipped. Builds for bare-metal targets shall use `panic = "abort"`.
*Verification:* Fuzz testing (REQ-TST-040), the lints of REQ-GEN-006 and Miri.

**REQ-GEN-009 — Documented resource use.**
The documentation shall state:
- the memory required by a decoder instance in its minimum configuration, which is one requested page with Level 1.5 enabled;
- the memory required in the default configuration of the command-line program;
- the memory required for presentation at Level 3.5, including the store for enhancement pages (REQ-ACQ-019), the side-panel area and the DRCS patterns that travel with the grid (REQ-L25-006);
- the memory required per stored page;
- the flash (code and constant data) size of the core on `thumbv6m-none-eabi`, built with `opt-level = "z"` and link-time optimisation.

Continuous integration shall measure the core's flash size on that target and fail if it exceeds a budget stated in the repository. The budget shall start at 64 kilobytes. The estimate is 15 to 40 kilobytes, of which the character tables are 10 to 20 kilobytes, and the measured figure replaces the estimate. Measured on 11 October 2026 by `cargo xtask size` (decision E.61): 26,198 bytes for the core with no optional area, 45,154 bytes for a Level 1.5 receiver with the renderer and the viewer, the build the budget applies to, 65,786 bytes at Level 3.5, and 111,294 bytes with every area and the temporary Bedstead glyphs. With the project's own glyphs (decision E.62) the full core is 86,706 bytes.
*Verification:* Test and continuous integration size check.

**REQ-GEN-010 — The Cargo workspace.**
The project shall be a single Cargo workspace with these crates. Each optional crate depends only on crates above it in this table.

| Crate | Contents | Environment |
|---|---|---|
| `neoteletext-core` | Everything listed in REQ-GEN-002 | `no_std`, no allocation |
| `neoteletext-formats` | The streaming readers, writers and page encoder of area INP | `no_std` readers. Writers and file helpers behind an `std` feature. |
| `neoteletext-slicer` | The software data slicer (area SLC) | `no_std`; a `fixed-point` feature (REQ-SLC-015) |
| `neoteletext-web` | The web-native protocols: HTTP, WebSockets, server-sent events, cookies and forms (area WEB), and the live feed reader of REQ-INP-022 | `no_std`, written "sans-IO": it consumes and produces bytes and events, and never performs input or output itself |
| `neoteletext-image` | The pixel-format helpers (REQ-RND-018) and the image writers (REQ-RND-007) | Helpers `no_std`; image writers need `std` |
| `neoteletext-app` | The application layer shared by `ttx` and a windowed viewer: sessions, inputs, the two processes of live mode, sandboxing (decision E.61) | `std` |
| `neoteletext-cli` | The command-line program `ttx` (area CLI) | `std` |
| `neoteletext-ffi` | The optional C interface (REQ-GEN-018) | Phase 2 |

A development-tools package (`xtask`) holds project automation that is not part of the library, such as the glyph export, fetching the corpus and checking sizes.

Features shall be additive. Each crate's optional areas are features, as decision E.61 lists. `cargo xtask features` builds, lints and tests each library and application crate with no optional area and with each area alone, and the workspace tests run with every feature. The mapping of the core modules of REQ-ARCH-001 to Rust modules shall be one module each within `neoteletext-core`.
*Verification:* Continuous integration.

**REQ-GEN-011 — Public interface conventions.**
- **Conventions.** The public interface shall follow the Rust API Guidelines.
- **Errors.** Errors are enumerations implementing `core::error::Error`, never strings.
- **Configuration.** Configuration is a plain struct with a `Default` implementation and named profile constructors (area PRF).
- **Documentation.** Every public item is documented.
- **Stability.** Crates follow semantic versioning, checked with `cargo-semver-checks` before each release.
- **Identifier prefixes.** The prefix `ttx_` is kept only for identifiers visible outside Rust: the C interface and the form field names `ttx_token` and `ttx_action` of Appendix C.

*Verification:* Inspection; `cargo-semver-checks` in the release process.

**REQ-GEN-012 — Provenance and licensing.**
The repository shall contain a licence file for the project. It shall also contain a provenance record listing:
- the source of every table transcribed from a standard, identified by clause and table number;
- the source and licence of every glyph set.

No code from GPL-licensed teletext software shall be included.
*Verification:* Inspection.

**REQ-GEN-013 — Throughput.**
- The packet-level decoder, which covers everything from 42-byte packets to stored pages including Level 1.5, shall process at least 17,500 packets per second on a single core of a current desktop computer. That is twenty times the maximum broadcast rate of 35 packets per frame at 25 frames per second.
- The raw-sample slicer shall process at least ten times the real-time rate of one stream of 35 lines per frame at 25 frames per second on a single core of the same computer.
*Verification:* Performance test (REQ-TST-050).

**REQ-GEN-014 — Every teletext constant is defined once.**
Every teletext value shall be defined in exactly one place, a constants module of the core, and read by name everywhere else. This covers every timing, level, code value, bit pattern, packet length, line allocation and tolerance. Where a value is needed in more than one form, both forms shall be defined side by side in that module with the conversion stated. The main example is the framing code, which is 0x27 in transmission order (T42) and 0xE4 in the bit order of digital television. No other module may restate, round, bit-reverse or re-derive a teletext value privately.
*Reason:* This is requirement R1 of the tellyboy project in the same workspace. Its recorded history shows that every teletext defect not caused by signal levels was a number existing in two places. In one case the framing code lived as 0xE4 in one file and 0x27 in another, both correct in their own bit order, with nothing to compare them.
*Verification:* Inspection; a test that searches the source outside the constants module for literal teletext values such as 0x27, 0xE4, 6937500 and 0x55.

**REQ-GEN-015 — Target platforms.**
The library shall support these targets, grouped by the four instruction-set families. "Build" means that continuous integration compiles the listed crates for the target; "test" means that it also runs the test suite there.

| Family | Target triple | Examples | Rust support tier | Continuous integration |
|---|---|---|---|---|
| ARM | `thumbv6m-none-eabi` | Cortex-M0 and M0+ | 2 | Build core, formats, slicer and web. Size check (REQ-GEN-009). |
| ARM | `thumbv7em-none-eabihf` | Cortex-M4F and M7F | 2 | Build; run the core tests under QEMU with semihosting |
| ARM | `thumbv8m.main-none-eabihf` | Cortex-M33 | 2 | Build |
| ARM | `aarch64-unknown-linux-gnu` | 64-bit Arm Linux boards | 1 | Build and test (native runner or emulated with `cross`) |
| ARM | `armv7-unknown-linux-gnueabihf` | 32-bit Arm Linux boards | 2 | Build and test with `cross` |
| ARM | `aarch64-apple-darwin` | Development on Apple computers | 1 | Build and test |
| RISC-V | `riscv32imc-unknown-none-elf` | ESP32-C2 and ESP32-C3 | 2 | Build |
| RISC-V | `riscv32imac-unknown-none-elf` | ESP32-C6, ESP32-H2 and other RV32IMAC parts | 2 | Build; run the core tests under QEMU |
| RISC-V | `riscv32imafc-unknown-none-elf` | ESP32-P4 | 2 | Build |
| RISC-V | `riscv64gc-unknown-linux-gnu` | 64-bit RISC-V Linux boards | 2 | Build and test with `cross` |
| Xtensa | `xtensa-esp32-none-elf` | ESP32 | 3 | Build with the pinned Espressif toolchain and `build-std = ["core"]` (REQ-GEN-001) |
| Xtensa | `xtensa-esp32s3-none-elf` | ESP32-S3 | 3 | Build, as above |
| Xtensa | `xtensa-esp32-espidf` and `xtensa-esp32s3-espidf` | ESP32 and ESP32-S3 with the ESP-IDF operating system, where `std` is available | 3 | Build, as above |
| AMD64 | `x86_64-unknown-linux-gnu` | Linux personal computers and servers | 1 | Build and test, including fuzzing and Miri |
| AMD64 | `x86_64-pc-windows-msvc` | Windows | 1 | Build and test |
| AMD64 | `x86_64-apple-darwin` | Intel Apple computers | 2 | Build and test |
| WebAssembly | `wasm32-unknown-unknown` and `wasm32-wasip1` | Isolated execution inside a WebAssembly runtime, in browsers or other hosts (REQ-SEC-019) | 2 | Build `neoteletext-core` and `neoteletext-formats` |

The rules for these targets are:
- **Which crates build where.** On bare-metal targets, the crates without `std` shall build: `neoteletext-core`, `neoteletext-formats`, `neoteletext-slicer`, `neoteletext-web` and the helpers of `neoteletext-image`. Targets with an operating system shall also build the `std` crates and the command-line program.
- **Where tests run.** Tests of logic run on the host. Cross-target tests prove that the code builds and behaves the same on each instruction-set family.
- **Xtensa testing.** Running tests on Xtensa, in Espressif's QEMU fork or the Wokwi simulator, may be added later. Xtensa is build-only in Phase 1.
- **Scope.** None of these targets implies any hardware support in the library (REQ-ARCH-010). The examples only indicate the class of processor.

*Verification:* Continuous integration matrix.

**REQ-GEN-016 — Rules for code that must run on small processors.**
Code in the `no_std` crates shall follow these rules, because some targets lack hardware for them.

| Rule | Why | Where it applies |
|---|---|---|
| **No floating point** | The Cortex-M0, M3 and RV32IMC have no floating-point unit, and the ESP32 emulates double precision in software. | The core, the web module and the formats. The slicer may use floating point, with a fixed-point form as REQ-SLC-015 requires. |
| **No atomic operations** | The Cortex-M0 and RV32IMC have no compare-and-swap. | Everywhere; the library needs no concurrency of its own. |
| **No division or 64-bit arithmetic in per-packet and per-pixel paths** | The Cortex-M0 has no hardware divide, and 64-bit operations are slow on 32-bit processors. | Per-packet and per-pixel code. Time values may be 64-bit, but arithmetic on them stays out of inner loops. |
| **No unaligned access** | The Cortex-M0 faults on it. | Everywhere; safe Rust already guarantees this. |
| **No formatting machinery on hot paths** | It adds several kilobytes to a small device. | The core shall not use `core::fmt` except in `Debug` implementations and behind an optional feature, so a small build can leave it out. |
| **No deep recursion or large stack buffers** | Small devices have small stacks. | Everywhere. The largest stack frame of the core shall be documented. |

*Verification:* Inspection; the size check of REQ-GEN-009; the builds of REQ-GEN-015.

**REQ-GEN-017 — Dependency policy.**
Dependencies are judged by the microcontroller, as decision E.38 describes.

- **Microcontroller crates.** These are the crates that run on bare-metal targets (REQ-GEN-015): `neoteletext-core`, `neoteletext-formats` without its `std` feature, `neoteletext-slicer`, `neoteletext-web` and the helpers of `neoteletext-image`.
- **Crates with no dependencies.** `neoteletext-core`, `neoteletext-formats` and `neoteletext-slicer` shall have no dependencies outside the Rust distribution's `core` library. Fixed-capacity collections and bit flags shall be written in the Foundation layer.
- **The web module.** `neoteletext-web` may depend only on `embedded-io`, with default features off. WebSocket framing, SHA-1, base64, the descriptor reader and the HTTP response-head parser shall be written in the project. Each shall be tested against the published test vectors of its standard, where the standard publishes them.
  *Amended 11 October 2026:* `httparse` was previously permitted as well. It was removed because of its linked panic paths (decision E.38, as amended).
- **Rules for any new dependency.** A new dependency of a microcontroller crate, direct or indirect, needs a decision recorded in Part E. It shall:
  - build as `no_std` without allocation for `thumbv6m-none-eabi`, `riscv32imc-unknown-none-elf` and `xtensa-esp32-none-elf`;
  - use no compare-and-swap atomic operations and no floating point;
  - have no build script and no native code;
  - have its flash cost measured by the size check of REQ-GEN-009;
  - pass the symbol check of REQ-GEN-019.
- **Other crates.** The command-line program, the image file writers, `xtask` and the C interface never run on a microcontroller. They may use well-maintained crates where these save real work, for example `png` for image writing, `clap` for the command line, and `ratatui` and `crossterm` for the interactive viewer.
- **Licences.** Every dependency shall have a licence compatible with the project's own: MIT, Apache 2.0, BSD, zlib, ISC or CC0. GPL-licensed crates are excluded (REQ-GEN-012). The rule shall be enforced with `cargo-deny`, which shall also check for security advisories.
- **Enforcing the list.** The permitted dependency list of each microcontroller crate shall be recorded in the repository. An `xtask` check shall compare it with the resolved dependency graph, and shall fail on any difference.

*Verification:* `cargo-deny` and the `xtask` dependency check in continuous integration; the symbol check of REQ-GEN-019.

**REQ-GEN-019 — Symbol check of the linked microcontroller image.**
Continuous integration shall link a test image for `thumbv6m-none-eabi`. The image shall contain the microcontroller crates of REQ-GEN-017 and call every public entry point, so that no code is removed as unused. The image shall be built with `opt-level = "z"`, link-time optimisation and `panic = "abort"`.

The check shall fail if the linked image contains:
- any software floating-point routine, such as `__aeabi_fadd` or `__aeabi_dmul`, outside the slicer's floating-point feature (REQ-SLC-015);
- any compare-and-swap atomic routine;
- any reachable panic other than the image's own panic handler.

The size check of REQ-GEN-009 shall measure the same image.
*Reason:* The lints of REQ-GEN-006 and REQ-GEN-016 cover only the project's own source. Only the linked image shows what dependencies and the compiler actually brought in (decision E.38).
*Verification:* Continuous integration, including a test that a deliberately added floating-point operation makes the check fail.

**REQ-GEN-020 — Capability features and the modern-classic profile.**
`neoteletext-core` shall offer the eight capability features of decision E.71, `objects`, `drcs`, `drcs-multicolour`, `bold`, `italic`, `side-panels`, `colours32` and `animation`, each on by default and each needing `level25`. A build without one of them shall contain neither the decoding nor the drawing of that capability, and shall treat its page data as decision E.71 item 1 says, never as an error. The feature `modern-classic` shall turn on `level15`, `level25`, `render`, `original`, `viewer`, `forms`, `objects`, `drcs` and `bold`, and nothing else.
*Verification:* `cargo xtask features` builds, lints and tests the core with `level25` alone and with `modern-classic`; a test checks each capability compiled out; `cargo xtask size` reports the image of the profile beside the full one.

**REQ-GEN-018 — Optional C interface.**
*Phase:* Phase 2 (whole requirement).
A separate crate, `neoteletext-ffi`, shall offer the core's interface to ports written in C. Its header shall be generated with `cbindgen`, and it shall be built as a static library. The crate shall be the only one allowed to contain `unsafe` code, and it shall document:
- that a panic aborts, because panics cannot unwind across the C boundary;
- the requirement for a panic handler on bare-metal targets;
- matching the floating-point calling convention of the C port.

Its public identifiers shall use the prefix `ttx_`.
*Verification:* Test that builds and runs a small C program against the static library on the host.

---

## F.2 Architecture and programming interface (ARCH)

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

---

## F.3 Error-protection codes (ECC)

**REQ-ECC-001 — Odd parity check.**
A function shall take a byte and report whether it has odd parity, meaning an odd number of "1" bits among its eight bits. It shall also report the 7-bit character value, which is the byte with bit 7 cleared (EN 300 706 clause 8.1).
*Verification:* Test over all 256 byte values.

**REQ-ECC-002 — Hamming 8/4 decoding.**
A function shall decode a Hamming 8/4 byte exactly as clause 8.2 specifies. The data bits are in positions 2, 4, 6 and 8, with bit 2 carrying the weight 1. The function shall return:
- the 4-bit value;
- a status of "no error", "corrected", or "uncorrectable".

A single error in a protection bit, including P4, shall be reported as "corrected", and the data shall be returned. A double error shall be reported as "uncorrectable". The function shall be table-driven: a 256-entry table generated from the encoding equations, not typed in by hand.
*Verification:* Exhaustive test (REQ-TST-002).

**REQ-ECC-003 — Hamming 8/4 encoding.**
A function shall encode a 4-bit value into a Hamming 8/4 byte using the equations of clause 8.2. It shall produce the sixteen codes 15, 02, 49, 5E, 64, 73, 38, 2F, D0, C7, 8C, 9B, A1, B6, FD and EA (hexadecimal) for the values 0 to 15.
*Verification:* Test.

**REQ-ECC-004 — Hamming 24/18 decoding.**
A function shall decode a three-byte triplet as clause 8.3 specifies:
- apply the six odd-parity tests A to F;
- accept the data when tests A to E all pass;
- correct a single error at the position 16E + 8D + 4C + 2B + A when tests A to E do not all pass and test F fails;
- report "uncorrectable" when tests A to E do not all pass and test F passes;
- report "uncorrectable" when the computed error position is greater than 24.

It shall return:
- the 18 data bits as a number, with data bit D1 as the least significant bit;
- the status.
*Verification:* Exhaustive test (REQ-TST-003).

**REQ-ECC-005 — Hamming 24/18 encoding.**
A function shall encode an 18-bit value into a triplet using the encoding equations of clause 8.3.
*Verification:* Test that every 18-bit value decodes back to itself.

**REQ-ECC-006 — Bit reversal.**
A function shall reverse the order of the bits within a byte. It is used when converting between broadcast order and the bit order of the Digital Video Broadcasting standard (area INP).
*Verification:* Test over all 256 byte values.

**REQ-ECC-007 — Page checksum.**
A function shall compute the 16-bit cyclic redundancy check of a page as clause 9.6.1 defines it for packet X/27/0:
- a 16-bit shift register initialised to zero, with the input combined with stages 7, 9, 12 and 16;
- the input bytes are 14 to 37 of the header followed by bytes 6 to 45 of packets X/1 to X/25 in order;
- absent packets are treated as spaces throughout;
- each byte is applied from bit 8 down to bit 1.

The decoder may use it to check received pages, and the encoder shall use it to generate packet X/27/0.
*Verification:* Test against a checksum from a broadcast page captured with a known-correct X/27/0.

---

## F.4 Packet decoding (PKT)

**REQ-PKT-001 — Packet address.**
The decoder shall Hamming-decode bytes 4 and 5 of each packet and compute:
- the magazine as the low three bits of the first value;
- the packet number as the top bit of the first value plus the second value shifted left by one.

Magazine value 0 shall be reported as magazine 8. If either byte is uncorrectable, the packet shall be discarded and counted (decision E.7).
*Verification:* Test with all 256 combinations of valid address values, and with injected errors.

**REQ-PKT-002 — Page header.**
For packet number 0, the decoder shall decode bytes 6 to 13 as Hamming 8/4 and extract:
- the page units and tens digits;
- the four sub-code digits (S1 with 4 bits, S2 with 3, S3 with 4, S4 with 2);
- control bits C4 to C14 at the exact bit positions of clause 9.3.1;
- the 32 display bytes 14 to 45 with their parity status.

A validity flag shall be kept separately for the page number, the sub-code and the control bits, so that decision E.7 can be applied.
*Verification:* Test against headers built by the encoder, and against captured headers.

**REQ-PKT-003 — Display rows.**
For packet numbers 1 to 25, the decoder shall extract the 40 bytes 6 to 45 as 7-bit characters, each with its parity status.
*Verification:* Test.

**REQ-PKT-004 — Enhancement packets.**
For packet number 26, the decoder shall:
- Hamming 8/4 decode byte 6 as the designation code;
- Hamming 24/18 decode the thirteen triplets in bytes 7 to 45;
- split each valid triplet into its 6-bit address, 5-bit mode and 7-bit data fields (clause 12.3.1);
- mark each triplet valid or invalid.
*Verification:* Test.

**REQ-PKT-005 — Editorial link packets.**
For packet number 27 with designation code 0, the decoder shall extract the six links. For each link it shall:
- decode the page number and sub-code;
- compute the linked magazine by exclusive-OR of the packet's own magazine with the relative magazine bits M1, M2 and M3 (clause 9.6.1);
- mark the link as null when the page number is FF and the sub-code is 3F7F.

It shall also extract:
- the Link Control Byte, including the "display row 24" flag in data bit 4 of byte 43;
- the two checksum bytes.

Packets 27 with designation codes 1 to 3 shall be stored without interpretation. Designation codes 4 and 5 are the compositional links of Levels 2.5 and 3.5, decoded as REQ-PKT-011 describes. Designation codes 6 to 15 shall be ignored.
*Amended 10 October 2026:* designation codes 4 and 5 were previously ignored with the rest (decision E.40).
*Verification:* Test.

**REQ-PKT-006 — Page enhancement packet X/28.**
For packet number 28 with designation code 0 in format 1 (as identified by clause 9.4.2), the decoder shall Hamming 24/18 decode triplets 1 and 2 and extract:
- the page function (bits 1 to 4 of triplet 1);
- the page coding (bits 5 to 7 of triplet 1);
- the 7-bit default G0 and G2 designation (bits 8 to 14 of triplet 1);
- the 7-bit second G0 designation (bits 15 to 18 of triplet 1 and bits 1 to 3 of triplet 2), laid out as Tables 32 and 33 define them.

The remaining fields of X/28/0 format 1 are decoded as REQ-PKT-012 describes. Packets X/28/1, X/28/3 and X/28/4 are decoded as REQ-PKT-013 to REQ-PKT-015 describe; the first triplet of X/28/1 is used for diagnostics only (decision E.4). Other designation codes shall be ignored.
*Amended 10 October 2026:* X/28/1 was previously stored without interpretation (decision E.40).
*Verification:* Test.

**REQ-PKT-007 — Magazine enhancement packet M/29.**
For packet number 29 with designation code 0 and packet function 0000000, the decoder shall extract the default G0 and G2 designation and the second G0 designation in the same way as for X/28/0. It shall store them for the magazine. The remaining fields, and packets M/29/1 and M/29/4, are decoded as REQ-PKT-016 describes. Other designation codes shall be ignored.
*Amended 10 October 2026:* M/29/1 was previously stored without interpretation (decision E.40).
*Verification:* Test.

**REQ-PKT-008 — Packets that do not belong to pages.**
Packets numbered 30 in magazines other than 8, and all packets numbered 31, shall be counted and ignored. Packets numbered 29 and 30 shall never start, end or attach to a page (clauses 7.3, 7.4 and 9.7).
*Verification:* Test that inserts these packets in the middle of a page and checks that the page is unaffected.

**REQ-PKT-009 — Reserved bits.**
Bits and bytes that the standard marks as reserved shall be ignored (clause 9.2).
*Verification:* Inspection.

**REQ-PKT-010 — Triplet functions at Levels 2.5 and 3.5.**
For every valid triplet of a packet X/26, and of packets X/1 to X/25 of an object page, the decoder shall provide the triplet's function as a typed value. The value shall cover every row mode of Table 28 and every column mode of Table 29, with the fields those tables define. In particular:
- for Object Invocation (row modes 10001 to 10011): the source (local, POP or GPOP, with S1 S0 = 00 marked illegal), the type, and the object number N8 to N0, split as Table 28 defines it for local and for public objects;
- for Object Definition (row modes 10101 to 10111): the usage level bits, the type and the object number;
- for the Origin Modifier: the row offset (the address minus 40) and the column offset (the data);
- for DRCS Mode: normal or global, the level bits and the sub-table;
- for the Termination Marker: the type in data bits 12 to 14;
- for every colour field: the 5-bit Colour Map index, with data bits D6 and D5 kept so that reserved values can be recognised.

Reserved modes shall be reported as reserved. The Level 1.5 pass, the enhancement processor and the object resolver shall all use this one decoding.
*Verification:* Test with one triplet of every mode, built by the encoder, including the worked encodings of `research/notes-level25-objects.md` section 13.2 and `research/notes-level25-drcs.md` section 11.

**REQ-PKT-011 — Compositional link packets X/27/4 and X/27/5.**
For packet number 27 with designation code 4 or 5, the decoder shall Hamming 24/18 decode triplets 1 to 12, and extract the six links of two triplets each that Table 15 defines. For each link it shall extract:
- the link function and the page validity, two bits each;
- the page units and tens;
- the magazine, computed by exclusive-OR of the packet's own magazine with the relative magazine bits, as for X/27/0;
- the sixteen sub-page flags;
- the two Format 1 marker bits: bit 11 of the first triplet and bit 1 of the second.

A link with an uncorrectable triplet shall be marked invalid. The roles of the links are applied by REQ-OBJ-030 (decision E.52).
*Verification:* Test with links built by the encoder, including the worked link of `research/notes-level25-objects.md` section 13.2, and with injected errors.

**REQ-PKT-012 — Packet X/28/0 format 1 in full.**
For packet X/28/0 format 1 (Table 4), the decoder shall extract, in addition to the fields of REQ-PKT-006:
- the left and right side-panel enable bits, the Side Panel Status Flag and the number of left-panel columns (triplet 2, bits 4 to 10);
- the sixteen 12-bit colour words for CLUTs 2 and 3, from the 192-bit stream formed by triplet 2 bits 11 to 18, triplets 3 to 12, and triplet 13 bits 1 to 4. Each word is red, green and blue, four bits each, with each group least significant bit first;
- the default screen colour (triplet 13, bits 5 to 9) and the default row colour (bits 10 to 14);
- Black Background Colour Substitution (triplet 13, bit 15);
- Colour Table Re-mapping (triplet 13, bits 16 to 18).

Each field group of decision E.44 shall carry its own validity flag.
*Verification:* Test with packets built by the encoder covering every class of field value, and with an error injected into each field group in turn.

**REQ-PKT-013 — Packet X/28/1: DRCS colour lookup tables.**
For packet X/28/1 (Table 8), the decoder shall extract the four DRCS colour lookup tables, read as 5-bit entries, least significant bit first, from triplet 2 bit 1 to triplet 13 bit 2:
- four entries for global 12 by 10 by 2 characters;
- four entries for normal 12 by 10 by 2 characters;
- sixteen entries for global 12 by 10 by 4 and 6 by 5 by 4 characters;
- sixteen entries for normal 12 by 10 by 4 and 6 by 5 by 4 characters.

Triplet 1 shall be kept for diagnostics only (decision E.4). Each table shall carry its own validity flag.
*Verification:* Test with the default tables of `research/notes-level25-drcs.md` section 6.2 and with the example values of Figure 28.

**REQ-PKT-014 — Packet X/28/3: DRCS mode identification.**
For packet X/28/3 (Table 9), the decoder shall extract the page function and page coding of triplet 1 and the 48 four-bit mode identification codes, one for each PTU, read as decision E.54 describes. Triplet 12 bits 13 to 18, and triplet 13, are reserved.
*Verification:* Test with the worked encoding of `research/notes-level25-drcs.md` section 4.

**REQ-PKT-015 — Packet X/28/4.**
For packet X/28/4 (Table 10), the decoder shall extract the same fields as REQ-PKT-012, except that the sixteen colour words are CLUT 0 entries 0 to 7 followed by CLUT 1 entries 0 to 7. The word for CLUT 1 entry 0 shall be ignored.
*Verification:* Test.

**REQ-PKT-016 — Magazine packets M/29/0, M/29/1 and M/29/4.**
For packet number 29, the decoder shall extract:
- with designation code 0 and packet function 0000000, every field of REQ-PKT-012 except the page function and page coding (Table 11);
- with designation code 1, the DRCS colour lookup tables, as REQ-PKT-013 describes (Table 12);
- with designation code 4 and packet function 0000000, the fields of REQ-PKT-015 (Table 13).

A packet with any other packet function shall be ignored.
*Verification:* Test.

**REQ-PKT-017 — Packets of object pages.**
For a stored packet X/1 to X/25 of an object page, the decoder shall Hamming 8/4 decode byte 6 and Hamming 24/18 decode the thirteen triplets. For packets 1 to 4, bit 1 of byte 6 shall say whether the packet carries pointer data (clause 10.5.1.2). From a pointer packet, the decoder shall extract the 24 nine-bit pointers of the triplets numbered 1 to 12, counting from 0 as clause 10.5.1.2 does: bits 1 to 9 hold the pointer to the even-numbered object, and bits 10 to 18 the pointer to the odd-numbered one. Triplet 0 is reserved. An uncorrectable pointer triplet shall make both of its pointers unused (decision E.51).

The decoder shall also convert an absolute triplet number from 0 to 506 into a packet and a triplet: number t below 299 is triplet t mod 13 of packet 3 + t / 13, and any higher number is triplet (t − 299) mod 13 of packet X/26 with designation code (t − 299) / 13.
*Verification:* Test with pointer tables built by the encoder, including the worked pointer of `research/notes-level25-objects.md` section 13.2, and with the extreme triplet numbers 0, 298, 299 and 506.

**REQ-PKT-018 — Packets of the Magazine Organization Table.**
For packets of page xFE, every byte of which is Hamming 8/4 coded, the decoder shall extract:
- from packets X/1 to X/14, the byte pair of any page number, at the position Figure 20 gives;
- from packets X/19 and X/20 (Level 2.5) and X/22 and X/23 (Level 3.5), the GPOP link and POP links 1 to 7 of Tables 20 and 21, including the fallback bits and the two default objects of each link;
- from packets X/21 (Level 2.5) and X/24 (Level 3.5), the GDRCS link and DRCS links 1 to 7 of Tables 22 and 23, and the totals of enhancement pages (clause 10.6.6).

An uncorrectable byte shall make the link or byte pair it belongs to invalid (decision E.7).
*Verification:* Test with the positions of `research/notes-level25-objects.md` sections 7.2 and 13.3, and the default object of section 13.4.

---
## F.5 Page acquisition and the page store (ACQ)

**REQ-ACQ-001 — Tracking the page in progress for each magazine.**
The decoder shall keep, for each of the eight magazines, a record of which page (if any) is currently being received in that magazine. It shall also record whether the service is in serial or parallel mode, taken from bit C11 of the most recent valid header.
*Verification:* Test.

**REQ-ACQ-002 — Starting a page.**
A header whose magazine, page number, sub-code and control bits are all valid (decision E.7) shall start the reception of that page in its magazine. Packets numbered 1 to 28 that follow shall be attached to the page as clause 7.2.1 defines:
- in parallel mode, packets of the same magazine;
- in serial mode, packets of the same magazine, received before the next header of any magazine.

In serial mode, packets numbered 1 to 28 from any other magazine shall be discarded and counted, because Annex B.3 does not permit them.
*Verification:* Test with interleaved serial and parallel streams.

**REQ-ACQ-003 — Ending a page.**
The reception of a page shall end:
- in serial mode, when any header of any magazine arrives;
- in parallel mode, when a header of the same magazine arrives (decision E.6).

The following headers end a page but do not start a displayable page:
- headers with invalid page number or control bytes;
- time-filling headers with page number FF.

In addition, a page shall be marked complete when 100 milliseconds pass without a packet for it (decision E.19). The timeout shall be configurable.
*Verification:* Test.

**REQ-ACQ-004 — Erase bit.**
When a header has bit C4 set, the stored rows 1 to 28 of that page and sub-code shall be cleared before any new rows are stored. This includes its enhancement and link packets.
*Verification:* Test.

**REQ-ACQ-005 — Page store in cache mode.**
The page store shall support a "cache" mode that stores every displayable page and sub-page received, up to a configured capacity. When the store is full, the least recently received page shall be replaced first. The capacity shall be set when the decoder is created, and the store shall use only the memory the caller provides.
*Verification:* Test that fills the store beyond its capacity.

**REQ-ACQ-006 — Page store in request mode.**
*Phase:* Phase 2 (whole requirement). Phase 1 uses cache mode only (REQ-ACQ-005), with a small capacity where memory is tight.
The page store shall also support a "request" mode like the SAA5246A. A configured number of page requests (four by default) is active at any time, and only pages matching a request are stored. Each request shall name:
- the magazine and page number;
- optionally, a sub-code pattern in which each of the four sub-code digits may be marked "don't care" (decision E.20).
*Verification:* Test.

**REQ-ACQ-007 — What is stored for a page.**
For each stored page and sub-page, the store shall keep:
- the header's page address and control bits;
- the header's 32 display bytes with parity flags;
- rows 1 to 25 with parity flags, and which rows have been received;
- up to sixteen packets X/26 with their designation codes;
- packet X/27/0 as decoded links;
- packets X/27/1 to X/27/3 raw;
- packets X/28/0 format 1, X/28/1, X/28/3 and X/28/4;
- packets X/27/4 and X/27/5;
- the time of the first and most recent reception;
- the number of receptions;
- counts of errors.
*Verification:* Inspection; test.

**REQ-ACQ-008 — Merging repeated receptions.**
When a page and sub-page that is already stored is received again without the erase bit, each received byte shall be merged into the store according to the parity-failure policy of decision E.8. A newly received row shall replace the stored row byte by byte. Rows not received in the new transmission shall keep their stored content.
*Verification:* Test with deliberately corrupted retransmissions.

**REQ-ACQ-009 — Magazine data.**
The decoder shall store, for each magazine, the most recently received valid packets M/29/0 and M/29/1. At Levels 2.5 and 3.5 it shall also store M/29/4, and shall discard them all when the service changes (REQ-ACQ-018).
*Verification:* Test.

**REQ-ACQ-010 — Update indicator.**
When a page with bit C8 set is received, or when the content of a displayed page changes, an event shall be raised so that a viewer program can redisplay or prompt.
*Verification:* Test.

**REQ-ACQ-011 — Rolling header and clock.**
The decoder shall provide the rolling header text and the clock text as decision E.11 defines. They are to be displayed while a requested page is being searched for.
*Verification:* Test with serial and parallel streams, including headers with C9 set.

**REQ-ACQ-012 — Search status.**
For the page the viewer has requested, the decoder shall report one of these states:
- "searching";
- "found";
- "found but not in the most recent cycle", meaning it is stored but has not been received for a configurable time.

The row 0 status column uses this state (decision E.10).
*Verification:* Test.

**REQ-ACQ-013 — Pages with hexadecimal numbers.**
Headers with hexadecimal page digits (A to F) shall be processed for page termination like any other header. Such pages shall not be stored in cache mode unless a diagnostic option enables it, and viewers shall not be able to select them by digit entry (EN 300 706 Table 1 note). At Levels 2.5 and 3.5 they are stored, because enhancement pages usually have hexadecimal numbers (REQ-ACQ-017).
*Verification:* Test.

**REQ-ACQ-014 — Non-text pages.**
If a page carries packet X/28/0 format 1 whose page function is not "basic Level 1 page" (0000), or whose page coding is not "7 bits plus odd parity" (000), the page shall be flagged as not displayable as text and shall not be rendered as a text page.
*Verification:* Test.

**REQ-ACQ-015 — Subtitle and newsflash pages.**
Pages with C5 or C6 set shall be stored and displayed like other pages. Their display shall use the boxed mode of decision E.22. Each reception replaces the previous content (normally with C4 set), so that subtitles update as they are transmitted.
*Verification:* Test with a subtitle stream.

**REQ-ACQ-016 — Enhancement pages are stored as received.**
At Levels 2.5 and 3.5, the store shall keep object pages, DRCS pages and the Magazine Organization Table (decision E.56). The rows of these pages shall be stored as received, so that each reception replaces the stored bytes of the packets it carries, with no merge by parity (REQ-ACQ-008 and decision E.8 do not apply):
- a page whose X/28/0 page function is not "basic Level 1 page";
- page xFE of each magazine;
- a page that a stored link of a Magazine Organization Table, or of a packet X/27/4 or X/27/5, names as an object or DRCS page.

A page that becomes an enhancement page only after it was first stored shall be stored as received from its next reception on. Each packet's presence shall be recorded.
*Verification:* Test with repeated receptions containing Hamming-coded bytes, and with a page that is named by a link after it was first stored.

**REQ-ACQ-017 — Hexadecimal pages at Levels 2.5 and 3.5.**
At Levels 2.5 and 3.5, pages with hexadecimal page numbers shall be stored, because enhancement pages usually have them (Annex E.1; decision E.56). Viewers shall still be unable to select them by digit entry, and navigation shall still skip them, unless the diagnostic setting of REQ-ACQ-013 is on.
*Verification:* Test at each presentation level.

**REQ-ACQ-018 — Magazine data at Levels 2.5 and 3.5.**
The decoder shall store, for each magazine, the most recent valid packets M/29/0, M/29/1 and M/29/4. Each new reception shall replace the stored packet of the same designation code. Every stored M/29 packet shall be discarded when the service changes, that is when packet 8/30 gives a new network identification, or when a new input is started. There shall be no time-out (decision E.56).
*Verification:* Test with a change of network identification and with a change of input.

**REQ-ACQ-019 — Capacity for enhancement pages.**
Enhancement pages shall be kept in a part of the store whose capacity, in packets, is set by the caller (REQ-GEN-003). The documented default shall be the totals of Annex E.8: 500 packets at Level 2.5 and 1200 at Level 3.5, over all magazines. When that part of the store is full, the least recently received enhancement sub-page shall be replaced first. Nothing shall be discarded when a Magazine Organization Table changes, beyond this eviction (decision E.56). The memory used shall be stated as REQ-GEN-009 requires.
*Verification:* Test that fills the enhancement store beyond its capacity.

**REQ-ACQ-020 — The read-only enhancement lookup.**
The page store shall offer a read-only lookup that the presentation layer uses to reach enhancement data (decision E.42). Given a magazine, a page number and an S1 sub-page number, it shall return the stored packets of that object or DRCS sub-page, with the record of which packets were received. Given a magazine, it shall return that magazine's Magazine Organization Table and its stored M/29 packets. The lookup shall have no side effects. Its cost shall not grow with the number of stored pages faster than a bounded search (REQ-SEC-012).
*Verification:* Inspection; test.

**REQ-ACQ-021 — Updates of enhancement pages.**
For each stored enhancement sub-page, the store shall apply the page status of Annex B.6 and Table B.1:
- a header with C4 set on a full transmission shall erase the stored packets before new ones are stored;
- a partial transmission (C13 set) and a continued fragment (C12 set) shall replace only the packets they carry;
- a change of the update indicator S2 shall be reported, so that decoded data taken from the sub-page is not reused.

The sub-code's last-packet number (S4 and S3) shall be recorded, so that completeness can be judged (REQ-DRC-009).
*Verification:* Test for every row of Table B.1.

**REQ-ACQ-022 — Redrawing when enhancement data arrives.**
When an enhancement page, a Magazine Organization Table, or a magazine packet M/29 that the displayed page uses is stored or changed, the decoder shall raise an event, so that the page's grid is built again and its changed rows are drawn again (decision E.41, principle 6). The Level 1 page shall be shown at once, without waiting for enhancement data.
*Verification:* Test that a page is shown before its object page arrives, and is redrawn when it arrives.

---

## F.6 Broadcast service data (SVC)

**REQ-SVC-001 — Packet 8/30 format 1.**
For packet number 30 in magazine 8 with designation code 0 or 1, the decoder shall extract the following (clause 9.8.1 and Table 18):

| Field | Source and handling |
|---|---|
| Multiplexed or full-field flag | Designation code bit 1 |
| Initial page | Bytes 7 to 12, Hamming 8/4, with an **absolute** magazine in byte 10 data bit 4 and byte 12 data bits 3 and 4 |
| Network identification | Bytes 13 and 14, a 16-bit value transmitted most significant bit first, reassembled accordingly |
| Time offset | Byte 15. Half-hours in bits 2 to 6; negative when bit 7 is set |
| Modified Julian Date | Bytes 16 to 18, five digits, each reduced by one after reception |
| Coordinated Universal Time | Bytes 19 to 21, six digits, each reduced by one |
| Status message | Bytes 26 to 45, 20 characters with parity |

*Verification:* Test against packets built by the encoder and against a captured packet.

**REQ-SVC-002 — Date and time.**
The decoder shall convert the Modified Julian Date and the time to a calendar date and time of day, where day 45000 is 31 January 1982. It shall report:
- the time as Coordinated Universal Time;
- the local time given by the offset.

The time refers to the next second boundary after the packet (clause 9.8.1), and the report shall say so. Out-of-range digits, such as an hour above 23, shall make the date or time invalid.
*Verification:* Test.

**REQ-SVC-003 — Packet 8/30 format 2.**
For designation codes 2 and 3, the decoder shall extract:
- the initial page and the status message, coded as in format 1;
- bytes 13 to 25 (the Programme Delivery Control data), stored raw without interpretation (Part G).
*Verification:* Test.

**REQ-SVC-004 — Use of the initial page.**
The initial page from packet 8/30 shall be used:
- as the page shown when the decoder starts;
- as the target of the Index key when a page has no index link.

When no 8/30 packet has been received, page 100 shall be used (Annex D.3).
*Verification:* Test.

**REQ-SVC-005 — Service name.**
The 20-character status message shall be available to the caller as text, decoded with the default G0 set without national option substitution.
*Verification:* Test.

---

## F.7 Character sets (CHR)

**REQ-CHR-001 — Internal character representation.**
Inside the decoder, each displayable character shall be represented by a character set identifier and a 7-bit code, and not by a Unicode value. The character set identifiers are:
- G0 Latin with a given national option;
- G0 Latin without national option;
- G0 Cyrillic option 1, 2 or 3;
- G0 Greek, Arabic or Hebrew;
- G1 mosaic;
- G2 Latin, Cyrillic, Greek or Arabic;
- G3.

A G0 character combined with a diacritical mark shall carry the mark's number as well. Conversion to Unicode shall happen only at the output stage.
*Reason:* Some teletext characters, such as mosaics and some G3 shapes, have no exact Unicode equivalent. Keeping the teletext identity lets each renderer choose its own representation.
*Verification:* Inspection.

**REQ-CHR-002 — Latin G0 with national options.**
The Latin G0 primary set (Table 35) shall be implemented with all 13 national option subsets of Table 36, for every one of the 13 national option positions:
- Czech and Slovak
- English
- Estonian
- French
- German
- Italian
- Lettish and Lithuanian
- Polish
- Portuguese and Spanish
- Rumanian
- Serbian, Croatian and Slovenian
- Swedish, Finnish and Hungarian
- Turkish

The base forms shown in Table 35 (#, ¤, @, [, \, ], ^, _, `, {, |, } and ~) shall be used whenever a Latin G0 character is referred to by packet X/26.

Four national option glyphs are drawn too small in the standard to identify with certainty. They were identified by the reference mapping in zvbi and by the Bedstead font:

| Position | Character | Unicode |
|---|---|---|
| Polish 5/B | Z with stroke | U+01B5 |
| Polish 7/D | l with stroke | U+0142 |
| English 6/0 | The SAA5050 long dash | U+2014, the em dash, which is also what Bedstead uses |
| English 7/C | Double vertical bar | U+2016 |

The Turkish currency sign at 2/3 is mapped to the Turkish lira sign, U+20BA. zvbi uses a private-use code, U+E800, from before Unicode had the sign.
*Verification:* Test against the transcribed tables and against the reference mapping (REQ-TST-011).

**REQ-CHR-003 — Selecting the character set of a page.**
The G0 and G2 sets and the national option in force on a page shall be found from Table 32 using the 7-bit value built as decision E.4 describes. The national option index shall be 4×C12 + 2×C13 + C14 (decision E.21).

If the resulting Table 32 entry is reserved:
1. the same three national option bits shall first be tried in group 0000;
2. if that entry is also reserved, English shall be used.

The fallback shall be counted as a diagnostic.
*Verification:* Test covering every defined entry of Table 32.

**REQ-CHR-004 — Other G0 sets.**
*Phase:* Phase 1 for the code tables and Unicode mappings; Phase 2 for the glyphs of these sets.
The following sets shall be implemented as code tables with their Unicode mappings (Tables 38 to 46):
- Cyrillic G0 options 1 (Serbian and Croatian), 2 (Russian and Bulgarian) and 3 (Ukrainian);
- Greek G0;
- Arabic G0;
- Hebrew G0.

Glyphs for these sets should be provided for the raster renderer; where a glyph is missing, the renderer shall draw a visible placeholder rather than a wrong character.
*Verification:* Test of the mappings; inspection of glyph coverage.

**REQ-CHR-005 — The G1 mosaic set.**
In mosaic mode, codes 2/0 to 3/F and 6/0 to 7/F shall be treated as block mosaics using the bit-to-block mapping of Part A.8, and codes 4/0 to 5/F as blast-through G0 characters with national option substitution applied (Table 47 note 2). For text output, mosaics shall map to Unicode as follows. Take n = (code AND 0x1F) OR ((code AND 0x40) shifted right by 1), the 6-bit block pattern, from 0 to 63.

**Contiguous mosaics:**

| n | Unicode |
|---|---|
| 0 | Space |
| 21 | U+258C (left half block) |
| 42 | U+2590 (right half block) |
| 63 | U+2588 (full block) |
| Any other value | The sextant characters U+1FB00 to U+1FB3B, in increasing order of n, skipping 21 and 42 |

**Separated mosaics** map to U+1CE50 + n (U+1CE51 to U+1CE8F), from the Symbols for Legacy Computing Supplement added in Unicode 16. Output that must work with older fonts may use the contiguous forms instead.

These ranges were checked against the Bedstead font's tables.
*Verification:* Test of all 64 mosaic codes.

**REQ-CHR-006 — The G2 sets.**
The following sets shall be implemented with their Unicode mappings:
- the Latin G2 supplementary set (Table 37), with all 95 characters;
- the Cyrillic G2 set (Table 41);
- the Greek G2 set (Table 43);
- the Arabic G2 set (Table 45).

Positions that the standard says are undefined or should not be transmitted (in the Latin set, 5/9, 5/A, 5/B and 6/5) shall display as a space.
*Verification:* Test.

**REQ-CHR-007 — The G3 set.**
The G3 set (Table 48) shall be implemented. Every defined position shall have a glyph, drawn as REQ-RND-037 describes, including positions 5/1 (horizontal line), 5/B (right arrow), 5/C (left arrow) and 5/D (up arrow), the ones Level 1.5 transmissions use in practice (Annex E.3). The unassigned positions 6/E, 6/F, 7/E and 7/F shall display as a space.
*Amended 10 October 2026:* glyphs for the G3 positions not used at Level 1.5 were Phase 2. They are Phase 1 now that Levels 2.5 and 3.5 are (decision E.40), and are drawn from the project's own geometry (decision E.50).
*Verification:* Test; inspection of glyph coverage.

**REQ-CHR-008 — Diacritical marks.**
The sixteen entries of column 4 of the G2 set shall be implemented as diacritical marks for the "G0 character with diacritical mark" triplet modes:

| Position | Mark |
|---|---|
| 4/0 | None |
| 4/1 | Grave |
| 4/2 | Acute |
| 4/3 | Circumflex |
| 4/4 | Tilde |
| 4/5 | Macron |
| 4/6 | Breve |
| 4/7 | Dot above |
| 4/8 | Diaeresis |
| 4/9 | Dot below (uncertain; see note) |
| 4/A | Ring above |
| 4/B | Cedilla |
| 4/C | Low line (uncertain; see note) |
| 4/D | Double acute |
| 4/E | Ogonek |
| 4/F | Caron |

*Note:* The glyphs at 4/9 and 4/C are drawn in Table 37 as a low dot and a low bar, but the standard does not name them. zvbi maps them to U+002E and U+02CD, and vhs-teletext to U+02CC and U+005F. This project treats them as the combining dot below (U+0323) and the combining low line (U+0332). They are rarely transmitted. The mapping shall be revisited if a real transmission is found that uses them.

Each shall have a Unicode combining character mapping. Composition shall follow decision E.26. Composition shall be attempted only for Latin G0 base characters (Annex E.10). For other G0 sets, the base character shall be shown without the mark, and the omission shall be counted as a diagnostic.
*Verification:* Test that every mark on every Latin letter produces the expected Unicode normalised form.

**REQ-CHR-009 — The second G0 set and the ESC attribute.**
When a second G0 set has been designated through Table 33 (decision E.4), the ESC attribute shall toggle the current row between the default and second G0 sets. When no second set has been designated, or the designation is 1111111 ("no second G0 set required"), ESC shall have no effect other than occupying its cell. The national option bits do not apply to the second set (clause 15.3).
*Verification:* Test.

**REQ-CHR-010 — Unicode conversion.**
A function shall convert any internal character to a Unicode scalar value or to a short sequence of scalar values. For characters with no Unicode equivalent, it shall return a documented private-use or fallback value.
*Verification:* Test (REQ-TST-011).

**REQ-CHR-011 — Unicode for the G3 set and for DRCS characters.**
Each defined G3 position shall convert to the Unicode character that Unicode document L2/19-025 and its mapping file `TELTXTG3.TXT` assign to it, as `research/notes-level25-drcs.md` section 9.3 lists them. Most are in the block "Symbols for Legacy Computing" from U+1FB3C to U+1FB6F, and the rest are box-drawing, geometric-shape and arrow characters. Position 5/F, which shows the background colour, shall convert to a space. The unassigned positions shall convert to a space. A DRCS character has no Unicode equivalent and shall convert to the documented fallback value of REQ-CHR-010.
*Verification:* Test of every G3 position against the mapping file.

---

## F.8 Level 1.0 presentation (L1)

**REQ-L1-001 — The cell grid.**
Decoding a page for display shall produce a grid of 25 rows (0 to 24) by 40 columns. Each cell shall record:
- the character in the representation of REQ-CHR-001, or a mosaic pattern with its contiguous or separated form;
- the foreground colour;
- the background colour;
- whether it flashes;
- whether it is concealed;
- whether it lies inside a box;
- its size role: normal, top half of double height, or bottom half of double height;
- flags recording whether it shows a held mosaic, whether it came from a Level 1.5 overwrite, and whether its source byte failed parity.

At Levels 2.5 and 3.5, each cell records more, and the grid also holds a side-panel area, as REQ-L25-006 describes.
*Verification:* Inspection; test.

**REQ-L1-002 — Row processing.**
Each row shall be processed from column 0 to column 39, starting from the default state of Part A.8:
- alphanumeric mode with white foreground;
- steady;
- box off;
- normal size;
- contiguous mosaics;
- black background;
- hold off;
- not concealed;
- held mosaic set to space;
- default G0 set.

No state shall carry over from one row to the next.
*Verification:* Test.

**REQ-L1-003 — Spacing attributes.**
Every attribute of Table 26 that has a Level 1 function shall be implemented with the timing Table 26 gives:
- **set-at** (takes effect in its own cell): 0/9, 0/C, 1/8, 1/9, 1/A, 1/C, 1/D and 1/E;
- **set-after** (takes effect from the next cell): 0/1 to 0/7, 0/8, 0/A, 0/B, 0/D, 1/1 to 1/7, 1/B and 1/F.
*Verification:* Test for every attribute (REQ-TST-013).

**REQ-L1-004 — Attribute cells.**
A cell holding a spacing attribute shall display as a space drawn with the state in force for that cell, except where hold mosaics substitutes the held mosaic (REQ-L1-007).
*Verification:* Test.

**REQ-L1-005 — Colours and modes.**
- Codes 0/1 to 0/7 shall set the foreground colour to colours 1 to 7 and select alphanumeric mode.
- Codes 1/1 to 1/7 shall set the foreground colour and select mosaic mode.

Colour number n has red if bit 0 is set, green if bit 1 is set and blue if bit 2 is set.
*Verification:* Test.

**REQ-L1-006 — Mosaic display.**
In mosaic mode, the G1 set shall be used as REQ-CHR-005 describes. The contiguous or separated form in force at the cell shall be recorded with the mosaic pattern.
*Verification:* Test.

**REQ-L1-007 — Hold mosaics.**
Hold mosaics shall be implemented as Table 26 states, with the timing of decision E.16. The rules are:
- **Substitution.** While hold is active and the row is in mosaic mode, each attribute cell shows the held mosaic.
- **What is held.** The held mosaic is the most recent mosaic character on the row whose code has bit 6 set. Blast-through characters are never held.
- **Resets.** The held mosaic is reset to space at the start of the row, on any change between alphanumeric and mosaic mode, and on any change of size. Repeating the current size does not reset it, and turning hold on or off does not reset it.
- **Form.** The held mosaic keeps the contiguous or separated form it had when it was stored.
*Verification:* Test including the worked example of Annex G.3.3 (REQ-TST-013).

**REQ-L1-008 — Double height.**
Double height shall be implemented as decision E.15 describes:
- Characters after a Double Height attribute have their top half in the current row and their bottom half in the row below.
- The whole character takes the attributes in force at its top half.
- When any cell of a row is double height, the transmitted content of the next row is ignored. That row shows bottom halves below double-height characters, and background colour without foreground below normal-height characters.
- Double height in rows 0, 23 and 24 shall be treated as normal height.
*Verification:* Test.

**REQ-L1-009 — Flash.**
Cells after a Flash attribute, until a Steady attribute or the end of the row, shall be marked as flashing. The displayed foreground of a flashing cell shall depend on the flash phase supplied by the caller or derived from caller-supplied time (decision E.12). In the hidden phase, foreground pixels are drawn in the background colour.
*Verification:* Test.

**REQ-L1-010 — Conceal and reveal.**
Cells after a Conceal attribute, until the next alphanumeric or mosaic colour attribute or the end of the row, shall be marked as concealed. Concealed cells display as spaces, keeping their background colour, unless the viewer's reveal setting is on.
*Verification:* Test.

**REQ-L1-011 — Boxes.**
Start Box and End Box shall mark cells as inside or outside a box, with set-after timing, following decision E.17. Box membership shall affect display only on pages with bit C5 or C6 set (decision E.22), or when a viewer display mode makes use of it.
*Verification:* Test including the worked example of Annex G.3.1.

**REQ-L1-012 — Background attributes.**
- Black Background shall set the background to black from its own cell onward.
- New Background shall set the background to the current foreground colour from its own cell onward.
*Verification:* Test.

**REQ-L1-013 — ESC.**
ESC shall toggle the G0 set as REQ-CHR-009 describes, taking effect from the next cell.
*Verification:* Test.

**REQ-L1-014 — Row 0.**
Row 0 shall be built as follows:
- Columns 0 to 6 come from the receiver status area (decision E.10).
- Column 7 is the receiver-generated colour attribute: white when found, green when searching.
- Columns 8 to 39 come from header bytes 14 to 45, either of the displayed page or, while searching, of the rolling header.

Row 0 shall then be decoded like any other row, starting from the default state at column 0. Row 0 shall not be displayed when the displayed page has bit C7 (Suppress Header) set. The exceptions are the receiver status area, if the configuration asks for it, and any active security indicator, which is always shown (REQ-SEC-005).
*Verification:* Test.

**REQ-L1-015 — Row 24.**
Row 24 shall be displayed according to decision E.23. When it is not displayed, it shall be left blank, so that it is available for receiver status messages.
*Verification:* Test.

**REQ-L1-016 — Inhibit display.**
When the displayed page has bit C10 set, rows 1 to 24 shall not be displayed. Row 0 is unaffected.
*Verification:* Test.

**REQ-L1-017 — Missing rows.**
A row that has not been received shall be displayed as 40 spaces (Annex D.1.1).
*Verification:* Test.

**REQ-L1-018 — Characters that failed parity.**
Characters that failed their parity check shall be displayed according to the policy of decision E.8.
*Verification:* Test.

**REQ-L1-019 — Codes with no Level 1 function.**
Codes 0/0, 0/E, 0/F and 1/0 shall be handled as decision E.18 describes, depending on the active profile.
*Verification:* Test.

**REQ-L1-020 — Viewer display modes.**
*Phase:* Phase 1 for normal, mix and boxed modes; Phase 2 for page zoom.
The display function shall support the following viewer modes:
- **Normal.** Text and backgrounds everywhere.
- **Mix.** Foreground everywhere and background transparent everywhere, so the text is shown over the picture.
- **Boxed.** Only boxed areas visible. This mode is automatic on newsflash and subtitle pages.
- **Page zoom.** The top half (rows 0 to 11) or the bottom half (rows 12 to 23) enlarged to double height, as the SAA5050 and SAA5246A offer.
*Verification:* Test; demonstration.

---

## F.9 Level 1.5 presentation (L15)

**REQ-L15-001 — Enabling Level 1.5.**
The Level 1.5 pass shall run when the presentation level setting (REQ-PRF-006) is Level 1.5. At Level 1, packets X/26 shall be stored but not applied. At Levels 2.5 and 3.5, the enhancement processor of area L25 shall run in place of this pass.
*Amended 10 October 2026:* this requirement was an on and off switch for Level 1.5, on by default. The presentation level setting replaces it (decision E.40).
*Verification:* Test at each presentation level.

**REQ-L15-002 — Processing order.**
The Level 1.5 pass shall process:
- the stored packets X/26 of the page in ascending designation code order (0 to 15, or 0 to 14 under decision E.2);
- within each packet, triplets 1 to 13 in order.

A missing designation code shall not stop processing; the next present packet is processed (Annex D.1.3 leaves this to the decoder). The first valid Termination Marker (row address 63, mode 11111) shall end processing.
*Verification:* Test.

**REQ-L15-003 — Starting position.**
The active position shall be row 0, column 0 when processing starts (clause 12.3.2).
*Verification:* Test.

**REQ-L15-004 — Setting the row.**
*Phase:* Phase 1 for the default rule; Phase 2 for the strict mode (see REQ-PRF-004).
Row address triplets shall set the active row according to decision E.3:
- by default, every row triplet (address 40 to 63) except the Termination Marker sets the row, with address 40 meaning row 24 and 41 to 63 meaning rows 1 to 23;
- in strict mode, only modes 00001, 00100 and 00111 set the row.

Mode 00111 with address 63 shall set row 0 and column 0 in both modes.
*Verification:* Test for each mode in both settings.

**REQ-L15-005 — Setting the column.**
Column address triplets shall set the active column to their address value for every mode except:
- the reserved column modes 00100, 00101 and 01010;
- mode 00110 (Programme Delivery Control).

These modes leave the active position unchanged (clause 12.3.4).
*Verification:* Test.

**REQ-L15-006 — Character-writing modes.**
At the active position, the following column modes shall write a character:

| Mode | Character written |
|---|---|
| 00010 | The G3 character given by the data field |
| 01111 | The G2 character from the designated G2 set |
| 10000 | The G0 character from the designated G0 set, without national option substitution. Data value 2/A writes "@". |
| 10001 to 11111 | The G0 character combined with the diacritical mark numbered by the low four bits of the mode |

Data values below 2/0 shall write nothing, but still set the column.
*Verification:* Test for every mode and for data values below 2/0.

**REQ-L15-007 — Modes ignored at Level 1.5.**
All other triplet modes shall be ignored for display, apart from their effect on the active position given by REQ-L15-004 and REQ-L15-005. This includes:
- mode 01011, the Level 2.5 G3 form, which Table 29 note 3 says Level 1.5 decoders should ignore;
- the Programme Delivery Control modes.
*Verification:* Test.

**REQ-L15-008 — Overwrite rules.**
Each character written by the Level 1.5 pass shall be placed in the cell grid according to all the rules of decision E.5 (a) to (j).
*Verification:* Test for each rule.

**REQ-L15-009 — Out-of-order triplets.**
*Phase:* Phase 1 for the default behaviour; Phase 2 for the strict option.
By default, a triplet that addresses a position earlier than the previous triplet shall still be processed. A strict option shall ignore it, as Annex A.4 permits.
*Verification:* Test in both settings.

**REQ-L15-010 — Character sets for overwrites.**
The G0 and G2 sets used by the Level 1.5 pass shall be the designated default sets of decision E.4. National options and the ESC attribute shall not affect them.
*Verification:* Test.

---
## F.9a Level 2.5 and 3.5 enhancement (L25)

This area specifies the enhancement processor of decision E.42: how the local enhancement data of a page, and the objects it invokes, change the characters, attributes, colours and side panels of the Level 1 grid. Objects themselves are specified in area OBJ (section F.9b), and redefinable characters in area DRC (section F.9c). Decisions E.40 to E.56 apply throughout. Clause and table numbers refer to EN 300 706 V1.2.1.

**REQ-L25-001 — When the enhancement processor runs.**
The enhancement processor shall run when the presentation level (REQ-PRF-006) is 2.5 or 3.5, and only then. It shall run after the Level 1 grid of the page has been built (area L1), in place of the Level 1.5 pass of area L15. At Level 2.5 it shall ignore every packet, triplet and field that Tables 1, 25, 28 and 29 mark for Level 3.5 only (decision E.40). These are packet X/27/5, the DRCS colour tables of X/28/1 and M/29/1, packets X/28/4 and M/29/4, the Font Style triplet, DRCS modes 1 to 3, and Modified G0 and G2 designations beyond the Level 2.5 limit.
*Verification:* Test that one stream containing every item marked for Level 3.5 only gives the expected grid at Level 2.5 and at Level 3.5.

**REQ-L25-002 — Processing order.**
The enhancement processor shall process, in this order:
1. the local enhancement data: the page's stored packets X/26 in ascending designation code order, 0 to 15, skipping missing codes, and within each packet its triplets 1 to 13 in order, up to the first Termination Marker or Object Definition triplet (decision E.46);
2. each object, at the point where the local enhancement data or another object invokes it (area OBJ);
3. when the page has no packet X/26 that carries a display triplet, the default objects (REQ-OBJ-040).

The walk shall follow the state machine of REQ-FSM-011. The attributes and characters it produces shall be resolved row by row, as REQ-FSM-012 describes.
*Verification:* Test; the transition tests of REQ-FSM-009.

**REQ-L25-003 — The active row.**
The active position shall start at row 0, column 0 at the start of the local enhancement data and whenever a default object is placed (clause 12.3.2). At Levels 2.5 and 3.5 only these row triplets shall move it (decision E.46):
- Full Row Colour (mode 00001) shall set the row from its address and the column to 0.
- Set Active Position (mode 00100) shall set the row from its address. It shall set the column from its data when the data is below 40, and leave the column unchanged when the data is 40 or more.
- Address Display Row 0 (mode 00111) with address 63 shall set row 0, column 0. With addresses 40 to 62 it shall leave the position unchanged.

A row address of 40 means row 24, and addresses 41 to 63 mean rows 1 to 23. No other row triplet shall move the active position.
*Verification:* Test for every row mode, including Object Invocation triplets whose address field would set a row under the rule of Annex A.3.

**REQ-L25-004 — The active column.**
Every column triplet shall set the active column to its address, except those of the reserved modes 00100, 00101 and 01010 and of the Programme Delivery Control mode 00110, which shall leave the position unchanged (clause 12.3.4). A character mode whose data is below 2/0 shall set the column and write nothing.
*Verification:* Test for every column mode.

**REQ-L25-005 — Damaged and out-of-order triplets.**
An uncorrectable triplet shall be skipped without moving the active position. A triplet that addresses a position earlier than the previous triplet from the same source shall be applied at the position it addresses. An Origin Modifier that is not immediately followed by an object invocation shall be discarded (decision E.46).
*Verification:* Test.

**REQ-L25-006 — What a cell records at Levels 2.5 and 3.5.**
At Levels 2.5 and 3.5, each cell of the grid shall record, in addition to the contents of REQ-L1-001 (decision E.42):
- its foreground and background colours as Colour Map indices 0 to 31 (decision E.43);
- its size, as a vertical part (normal, top half or bottom half) and a horizontal part (normal, left half or right half);
- underline, and whether its mosaics are separated;
- invert;
- the flash mode, rate and phase of decision E.48;
- the window or box membership of REQ-L25-029;
- the Level 3.5 font styles: bold, italic and proportional;
- for a G3 character, its code; for a DRCS character, the table (normal or global), the sub-table and the character number;
- which source wrote its character: the Level 1 page, the local enhancement data, or an Active, Adaptive or Passive object.

The grid shall also hold the page's Colour Map (REQ-L25-030), its screen colour and row colours (REQ-L25-034), its side-panel area and widths (REQ-L25-040), and the DRCS patterns and colour tables that its DRCS cells use (REQ-DRC-007). All of this shall be held in memory the caller provides (REQ-GEN-003).
*Verification:* Inspection; test.

**REQ-L25-010 — Characters written at Levels 2.5 and 3.5.**
The following column modes shall write a character at the active position, with the display priority of the source that writes it (REQ-OBJ-012):

| Mode | Character written |
|---|---|
| 00001 | A G1 character (REQ-L25-011) |
| 00010 and 01011 | A G3 character |
| 01001 | A G0 character from the current G0 set |
| 01101 | A DRCS character (REQ-DRC-006) |
| 01111 | A G2 character from the current G2 set |
| 10000 to 11111 | A G0 character combined with the diacritical mark numbered by the low four bits of the mode. Mode 10000 with data 2/A writes "@". |

National option substitution and the ESC attribute shall never apply to these characters (Table 25; clause 15.4). A G3 character shall not change the held mosaic, and Hold Mosaics shall never replace a character written by enhancement data (decision E.50). The undefined positions named in decision E.50 shall show a space, and a diagnostic shall be traced.
*Verification:* Test for every mode, every character set and every undefined position.

**REQ-L25-011 — G1 characters written by triplets.**
A G1 character written by mode 00001 with a code in columns 2, 3, 6 or 7 shall be drawn as a block mosaic. It shall be contiguous or separated according to the non-spacing Underline or Separated attribute in force at its cell, and not according to the Level 1 codes 1/9 and 1/A (Table 26 notes 11 and 12). A code in columns 4 or 5 shall be the character of the current G0 set at that position, without national option substitution (Table 47 note 3).
*Verification:* Test.

**REQ-L25-012 — Character set designation.**
The default G0 and G2 sets for characters written by enhancement data, and the second G0 set used by ESC on the Level 1 page, shall be taken from the highest-priority designation of Annex C.4 rule 1 (decision E.44). The national option shall be chosen as decision E.4, as amended, describes.

A Modified G0 and G2 Character Set Designation triplet (mode 01000) shall select the pair of sets that Table 34 gives for its data. Reserved values shall be ignored. Its effect shall last:
- in local enhancement data and Active objects, to the end of the row or to the next such triplet, after which the default sets apply again;
- in Adaptive and Passive objects, to the last character cell of the object or to the next such triplet.

It shall never affect the Level 1 page. At Level 2.5, a designation that would make more than two G0 and G2 pairs on the page, counting the default pair, shall be ignored. At Level 3.5 there is no limit (decision E.50).
*Verification:* Test, including a third pair at both levels.

**REQ-L25-020 — Attribute types and contention.**
Foreground colour, background colour, flash, and each of the six display attributes (double height, double width, boxing or window, conceal, invert, and underline or separated mosaics) shall each be a separate attribute type (decision E.47). Where two attributes of one type apply to a cell, the latest shall win under Annex C.2: first by the cell where each takes effect, then by the cell where each is placed, then by the order of transmission. A non-spacing attribute placed at the same cell as a set-at Level 1 attribute of the same type shall win. Every non-spacing attribute shall take effect at the cell where it is placed.
*Verification:* Test with the three worked examples of Annex C.2 and with the same-cell case (REQ-TST-016).

**REQ-L25-021 — Foreground Colour and Background Colour triplets.**
Column modes 00000 (Foreground Colour) and 00011 (Background Colour) shall set their colour to the Colour Map index in data bits D4 to D0. The colour shall apply from the cell where the triplet is placed to the end of the row, unless a later spacing or non-spacing attribute of the same type overrides it. Data with D6 or D5 set is reserved, and the triplet shall then only set the column. Neither triplet shall change alphanumeric or mosaic mode.
*Verification:* Test.

**REQ-L25-022 — Level 1 colour attributes at Levels 2.5 and 3.5.**
At Levels 2.5 and 3.5, the Level 1 spacing colour attributes shall work as Table 26 defines them for these levels:
- 0/0 Alpha Black and 1/0 Mosaic Black shall set a black foreground: entry 0 of the foreground CLUT.
- 0/1 to 0/7 and 1/1 to 1/7 shall set entry n of the foreground CLUT that Colour Table Re-mapping selects.
- 1/C Black Background shall set entry 0 of the background CLUT, or the row's full row colour when Black Background Colour Substitution is on.
- 1/D New Background shall set a background from the background CLUT, with the entry number of the foreground colour currently selected by the Level 1 page.

The start-of-row default white foreground and black background shall be re-mapped in the same way, except that substitution replaces the default black background when it is on (decision E.43). A black background obtained by 0/0 or 1/0 followed by 1/D shall not be substituted. A background colour set by a triplet shall never be substituted.
*Verification:* Test for each of the eight re-mapping values, with substitution on and off.

**REQ-L25-023 — The Display Attributes triplet.**
Column mode 01100 shall set, at its cell, the six display attributes from its data bits: D0 double height, D1 boxing or window, D2 conceal, D4 invert, D5 underline or separated mosaics, and D6 double width, with D0 and D6 together meaning double size. D3 is reserved and shall be ignored. A bit at 1 shall turn its function on and a bit at 0 shall turn it off, from that cell to the end of the row or until changed again. A 0 bit shall also turn off a function that a Level 1 spacing attribute turned on (decision E.47). In particular:
- **Conceal** set by the triplet shall last until a Display Attributes triplet clears it or the row ends. A Level 1 colour code shall not end it.
- **Underline or separated mosaics** set by the triplet shall underline G0 characters, G1 characters of columns 4 and 5, and G2 characters, and shall draw G1 mosaics separated. The Level 1 code 1/9 shall not cancel it. Clearing it by a triplet shall also clear a Separated Mosaics set by the Level 1 code 1/A.
- **Invert** shall swap the displayed foreground and background colours. While it is in force, a Level 1 change of foreground alters the displayed background, and a change of background alters the displayed foreground.
- **Boxing or window** shall follow REQ-L25-029.
*Verification:* Test for each bit, set and cleared, against each Level 1 attribute of the same type.

**REQ-L25-025 — Size at Levels 2.5 and 3.5.**
Double height, double width and double size shall be available as the spacing attributes 0/D, 0/E and 0/F and as non-spacing attributes (REQ-L25-023). Any size attribute shall end any other (Table 26 notes 2, 5 and 8). Sizes shall be resolved by the rules of Annex C.3:
1. Double height and double size shall not take effect in rows 0, 23 and 24, and double width and double size shall not take effect in column 39 or in the last column of a side panel. At those places the character shall be drawn without the doubling that cannot be shown.
2. Earlier size attributes in scan order, left to right and top to bottom, shall take priority over later ones.
3. Where an enlarged character conflicts with an earlier one, the conflicting doubling shall be dropped. A double-size character shall be reduced to normal size only if neither double width nor double height alone would fit.
4. All attributes at the origin of an enlarged character shall apply to all its parts.
5. Attributes at positions hidden by an enlarged character shall take effect at the first position after it that is not hidden, unless they are superseded.

Double width and double size shall never spread from the main page into a side panel, or back (clause 12.5.4 rule 3). Each cell shall record its vertical and horizontal parts (REQ-L25-006).
*Verification:* Test for each rule, including the conflicts of rule 3.

**REQ-L25-026 — The row below a double-height row.**
When a row contains spacing double height or double size, alone or together with non-spacing double height, the row below shall show the lower halves under the enlarged characters. Elsewhere it shall show the local background colour of the cell above, with no foreground, and its own Level 1 characters and attributes shall be ignored (Table 26 codes 0/D and 0/F).

Enhancement characters that local enhancement data or objects write to that lower row, in cells not covered by lower halves, shall be displayed. For local enhancement data, Active objects and Adaptive objects, their attributes shall be the start-of-row defaults, modified only by earlier enhancement data on that row. Cells where no enhancement character is written shall be unchanged.

When a row's only double height or double size comes from non-spacing attributes, the row below shall not be suppressed: everything outside the lower halves keeps its normal effect (decisions E.5 and E.15, as amended).
*Verification:* Test for spacing, non-spacing and mixed double height, with enhancement characters on the lower row.

**REQ-L25-027 — Additional Flash Functions.**
Column mode 00111 shall set, at its cell, the flash mode from data bits D1 and D0 (steady, normal, inverted phase, or adjacent CLUT) and the rate and phase from D4 to D2 (slow; fast with phase 1, 2 or 3; fast incremental; or fast decremental), with the timing of decision E.48. Data with D6 or D5 set, and the rate values 110 and 111, are reserved, and the triplet shall then only set the column.

The flash attribute shall last to the end of the row, or to the next flash attribute: Level 1 Flash or Steady, or another Additional Flash Functions triplet. For incremental and decremental flash, the phases shall be assigned to successive characters from the triplet's column rightwards. At Levels 2.5 and 3.5 the Level 1 Flash attribute shall mean slow flash in normal mode. Flash shall act on the foreground colour only.
*Verification:* Test for every mode and rate, and for the phases given to a run of characters.

**REQ-L25-028 — Font Style (Level 3.5).**
At Level 3.5, column mode 01110 shall set proportional spacing (D0), bold (D1) and italics (D2) from its column to the end of the row, each bit enabling its style when 1 and cancelling it when 0. The row count in D6 to D4 shall extend the same styles over the following 0 to 7 rows, from the same column to the end of each row, stopping at row 24. A later Font Style triplet shall replace the styles from its own position onward. D3 is reserved. The styles shall apply to G0 and G2 characters only (decision E.50). At Level 2.5 the triplet shall only set the column.
*Verification:* Test against the zvbi observation recorded in `research/notes-level25-drcs.md` section 13.2, where a row count of 2 reached the next two rows only, and with overlapping row extensions.

**REQ-L25-029 — Boxing, windows and the display planes.**
At Levels 2.5 and 3.5, the boxing or window bit of a Display Attributes triplet shall start a boxed area on a page with C5 or C6 set, and a window on any other page (Table 29 mode 01100, D1). Inside a window the full row colour becomes transparent, so that the picture shows where the foreground or background colour is also transparent. A Level 1 Start Box on a page with neither C5 nor C6 set shall not open a window (decision E.47).

The source of each pixel shall follow Table C.1 of Annex C.1: the foreground or background colour, the full screen or row colour, or the picture. The picture is rendered as transparent pixels or the fill colour (REQ-RND-006).
*Verification:* Test of all rows of Table C.1 (REQ-TST-016).

**REQ-L25-030 — The Colour Map of a page.**
For each page shown at Level 2.5 or 3.5, the enhancement processor shall build the page's Colour Map of 32 entries (decision E.43):
- every entry starts with its Table 30 default;
- CLUTs 2 and 3 are replaced by the colour words of the page's X/28/0 format 1, or, when that is absent or its colour words are invalid, by those of the magazine's M/29/0;
- at Level 3.5 only, CLUTs 0 and 1 are replaced by the colour words of the page's X/28/4, or, when that is absent or invalid, by those of the magazine's M/29/4. CLUT 1 entry 0 always stays transparent.

The Colour Map shall travel with the grid to the renderer (REQ-RND-030).
*Verification:* Test for each source and each order of precedence, including the Table 30 values of CLUT 1 entries 5 and 6.

**REQ-L25-031 — Precedence of page-level presentation data.**
Side panels, the default screen and row colours, Black Background Colour Substitution and Colour Table Re-mapping shall each be taken from the first valid source in this order (decision E.44):
1. the page's X/28/0 format 1;
2. the page's X/28/4, at Level 3.5;
3. the magazine's M/29/0;
4. the magazine's M/29/4, at Level 3.5;
5. for side panels and substitution only, the fallback bits of the page's POP link in the Magazine Organization Table (REQ-OBJ-031);
6. the defaults of Annex D.2.1.

A source is valid for a field group when the triplets of that group were decoded without uncorrectable errors (REQ-PKT-012). Packet X/28/0 shall be read as format 1 on every page that is displayed.
*Verification:* Test for each source, with each field group damaged in turn.

**REQ-L25-032 — Full Screen Colour.**
Row mode 00000 with data bits D6 and D5 at 0 shall set the screen colour of the whole page to the Colour Map index in D4 to D0. Other data values are reserved, and the triplet shall then be ignored. The triplet shall not move the active position. It shall be acted on only in local enhancement data and in Active objects (decision E.51).
*Verification:* Test.

**REQ-L25-033 — Full Row Colour and Address Display Row 0.**
Row mode 00001 shall set the active position (REQ-L25-003). When its bits S1 and S0 are 00 it shall set the row colour of the addressed row. When they are 11 it shall set the row colour of the addressed row and of every row below it, down to row 23, or to row 24 when row 24 is displayed. Row mode 00111 with address 63 shall do the same for row 0. With S1 and S0 at 01 or 10, either triplet shall set the position and no colour.

The commands shall apply in transmission order, so that a "this row only" command changes only its own row, and an earlier "down to the bottom" fill continues below it (decision E.45). Both triplets shall be acted on only in local enhancement data and in Active objects.
*Verification:* Test with sequences of both kinds of command.

**REQ-L25-034 — Screen colour, row colours and borders.**
The screen colour and each row's colour shall be resolved in the orders of decision E.45. The screen colour shall fill the areas above row 0, below the last displayed row, to the left and right of the rows, and outside the side panels. A row's colour shall show through the row's cells as Table C.1 describes (REQ-L25-029), and shall be the default background of the row's side-panel cells (REQ-L25-041). No row colour shall be drawn in row 0, columns 0 to 7 (REQ-L25-050).
*Verification:* Test against the precedence lists of Annex A.5 (REQ-TST-016).

**REQ-L25-040 — Enabling side panels.**
The left and right side panels and their widths shall be found from the source that REQ-L25-031 chooses, using the width table of decision E.49. At Level 2.5, side panels shall be shown only when the Side Panel Status Flag is 1. At Level 3.5 they shall be shown whenever they are enabled. When the source is the fallback of the Magazine Organization Table, its bits shall select no panel, a 16-column left panel, a 16-column right panel, or two 8-column panels (Table 21). The display width shall be 40 columns plus the widths of the two panels.
*Verification:* Test of every row of the width table, and of each fallback value.

**REQ-L25-041 — Side-panel content.**
Side-panel cells shall be addressed as columns 40 to 71 of rows 1 to 24: the right panel from column 40, and the left panel up to column 71 (clause 12.5.3). The following rules shall hold (clause 12.5.4):
- Each panel row starts from the Level 1 start-of-row defaults, with the full row colour as its background, and every cell is a space until it is written (rule 1).
- Attributes shall not pass from a left panel into the main page, nor from the main page into a right panel (rule 2).
- The page's header bits C5, C6, C7 and C10 govern the panels too (rule 5).
- Content comes only from objects placed by Origin Modifiers.
- Writes to panel columns that are not enabled, to row 0, below row 24 or beyond column 71 shall be discarded, as shall an object element that would cross between the main page and a panel, or between the two panels (decision E.49).
- In row 0, the panel area shows the screen colour.
*Verification:* Test with objects writing to every boundary, including the three column layouts given in clause 12.5.3.

**REQ-L25-050 — The receiver's status area at every level.**
No character, attribute, row colour, screen colour or side-panel content from local enhancement data, objects or default objects shall be written to, or drawn in, row 0, columns 0 to 7 (REQ-SEC-005; decision E.41). A write addressed there shall be discarded, and the rest of the data it came from shall still be processed.
*Verification:* Test that pages writing to that area by every Level 2.5 and 3.5 route leave it unchanged. The routes include Address Display Row 0 with a row colour, objects with Origin Modifiers, side panels and default objects.

**REQ-L25-051 — Faults are contained.**
A missing, damaged or unusable enhancement source shall remove only the enhancement that depends on it (decision E.41): a skipped triplet, object or DRCS character, or a page shown without its object or DRCS page. The Level 1 page shall always be shown. Each such fault shall be counted in the decoder's statistics (REQ-ARCH-007) and traced.
*Verification:* Test with each kind of missing or damaged source.

**REQ-L25-052 — Reserved and Programme Delivery Control triplets.**
Reserved row and column modes, the Programme Delivery Control modes (row modes 01000 to 01101 and column mode 00110), and reserved data values shall have no display effect. They shall move the active position only as REQ-L25-003 and REQ-L25-004 say (clause 9.2; Part G).
*Verification:* Test.

---

## F.9b Objects (OBJ)

This area specifies objects (clause 13), the pages that carry them (clause 10.5), and the links that lead to those pages: the Magazine Organization Table (clause 10.6) and packets X/27/4 and X/27/5 (clause 9.6.2). Decisions E.51 to E.53 apply. Objects are processed by the walk of REQ-FSM-011.

**REQ-OBJ-001 — Object invocation.**
An Object Invocation triplet (row modes 10001 Active, 10010 Adaptive and 10011 Passive) shall invoke the object it names, from the source its bits S1 and S0 give: 01 local, 10 the page's Public Object Page (POP), or 11 the page's Global Public Object Page (GPOP). Source 00 is illegal, and the triplet shall be skipped. The invocation shall run the object's triplets once, from the origin of REQ-OBJ-002, subject to the nesting rules of REQ-OBJ-008 and the limits of REQ-OBJ-020 (clauses 13.6 and 13.7).
*Verification:* Test.

**REQ-OBJ-002 — The origin and the Origin Modifier.**
An object's origin shall be the active position at its invocation plus the offsets of an Origin Modifier (row mode 10000) that immediately precedes the invocation. The row offset is the modifier's address minus 40, and the column offset is its data, 0 to 71. Inside the object the active position shall start at row 0, column 0, relative to the origin. Every coordinate in the object shall be relative to the origin, including Address Display Row 0 and row address 40 (decision E.51). When the object ends, the active position shall return to its value at the invocation, and the Origin Modifier shall be cancelled. An Origin Modifier followed by any triplet other than an invocation shall be discarded (decision E.46). A column offset above 71 places every element beyond column 71, where REQ-OBJ-014 discards it.
*Verification:* Test, including the worked Origin Modifier of `research/notes-level25-objects.md` section 13.2.

**REQ-OBJ-003 — Finding a local object.**
For a local invocation, bits N7 to N4 of the object number shall give the designation code of the packet X/26 that holds the definition, and bits N3 to N0 the triplet within it, numbered 0 to 12 (Table 28). Bit N8 shall be 0. An invocation shall be skipped when N8 is 1, when the triplet number is 13 to 15, or when the packet with that designation code was not received (decision E.51). The triplet found shall then be checked as REQ-OBJ-005 describes.
*Verification:* Test, including each case that is skipped.

**REQ-OBJ-004 — Finding a public or global object.**
For a POP or GPOP invocation, the definition shall be found as clause 10.5.1.2 and Table 28 describe:
1. the object page is the POP or GPOP that the page's links name (REQ-OBJ-030);
2. the sub-page is the one whose S1 equals N3 to N0 (REQ-OBJ-032);
3. the pointer packet is packet 1 + (N8 N7), and its byte 6 shall say that it carries pointer data;
4. the pointer triplet, numbered from 0, is 1 + 3 × (N6 N5) + k, where k is 0 for Active, 1 for Adaptive and 2 for Passive objects;
5. the pointer is bits 1 to 9 of that triplet when N4 is 0, and bits 10 to 18 when N4 is 1;
6. the pointer value is an absolute triplet number, converted to a packet and triplet as REQ-PKT-017 describes.

The object shall be absent when the pointer is 0x1FF (not used), when its value is 507 to 510, when it points into a pointer packet or into packets 1 or 2, or when the pointer packet is missing or does not carry pointer data (decision E.51).
*Verification:* Test with the worked pointer of `research/notes-level25-objects.md` section 13.2, and with each case that makes the object absent.

**REQ-OBJ-005 — Checking the definition.**
The triplet found shall be an Object Definition (row modes 10101 to 10111) of the same type as the invocation, whose usage bits include the presentation level: 01 for Level 2.5, 10 for Level 3.5, and 11 for both. Otherwise the invocation shall be skipped. The object number bits of the definition shall not be compared with those of the invocation (decision E.51).
*Verification:* Test with a definition of each other type, and with each usage value at both levels.

**REQ-OBJ-006 — The body of an object.**
An object's body shall be the triplets that follow its definition, up to the next Object Definition or Termination Marker triplet (clause 13.1). Triplet 12 of a packet is followed by triplet 0 of the next packet, and packet 25 is followed by packet X/26 with designation code 0. A body shall be at most 507 triplets, and shall never continue onto another sub-page (clause 10.5.1.1).

Before an object runs, its body shall be scanned to find where it ends and which cells it addresses on each row (REQ-OBJ-010 and REQ-OBJ-020). When the scan meets a packet that has not been received before the end of the body, the object shall be absent; other objects on the same page are unaffected (decision E.51). Uncorrectable triplets in the body shall be skipped.
*Verification:* Test with bodies that cross packets, that reach packet X/26, and that meet a missing packet.

**REQ-OBJ-008 — Which objects may invoke which.**
Invocations shall be permitted only as this table shows (clauses 13.3 to 13.6; decision E.51). Any other invocation shall be skipped.

| Invoker | May invoke |
|---|---|
| Local enhancement data | Active, Adaptive and Passive objects, from local, POP or GPOP sources |
| Active object | Adaptive and Passive objects |
| Adaptive object | Passive objects |
| Passive object | Nothing |
| The default-object mechanism (REQ-OBJ-040) | POP and GPOP objects of any type, never local ones |

A local object shall be invoked only by the page's own local enhancement data, or by another local object, and never by a public or global object (clause 13.6). An object invoked from inside a public or global object shall be found through the displayed page's POP and GPOP links.
*Verification:* Test for every pair of invoker and type, and for each source.

**REQ-OBJ-009 — Active objects.**
An Active object's triplets shall act as if they were local enhancement data at the active position (clause 13.3). The attributes at its starting position shall be those of the Level 1 page, as already modified there by local enhancement data and earlier Active objects. The attributes it sets shall act to the end of the row, on every character, whether the character comes from the object or from the Level 1 page, until a later attribute of the same type supersedes them.
*Verification:* Test with the Active case of the clause 13.9 example (REQ-TST-016).

**REQ-OBJ-010 — Adaptive objects.**
An Adaptive object shall start with the underlying attributes at its position: those of the Level 1 page after local enhancement data and Active objects (clause 13.4). On each row, the attribute changes it makes shall affect both its own characters and the Level 1 characters between the first and the rightmost cell that any of its triplets addresses on that row (decision E.51). For an attribute type that the object has changed, later changes made by the Level 1 page inside that range shall be ignored. To the right of the rightmost addressed cell, the underlying attributes shall apply again. A character set designation made by the object shall last to the last character cell of the object.
*Verification:* Test with the Adaptive case of the clause 13.9 example (REQ-TST-016).

**REQ-OBJ-011 — Passive objects.**
Each invocation of a Passive object shall start from these attributes (clause 13.5): white foreground (CLUT 0 entry 7), black background (CLUT 0 entry 0), normal size, steady, boxing off, not concealed and contiguous mosaics. Its attributes shall persist across changes of row and of active position until the object changes them or ends. They shall apply only to the cells where the object writes a character, and the underlying attributes shall not affect those cells. Cells that the object does not write shall show the underlying page unchanged. Transparency inside a Passive object is CLUT 1 entry 0, as everywhere else (decision E.51).
*Verification:* Test with the Passive case of the clause 13.9 example (REQ-TST-016), and with two invocations of one object.

**REQ-OBJ-012 — Display priority.**
Where several sources write a character to one cell, the character shown shall be chosen by the priority of clause 13.2: Passive objects, then Adaptive objects, then the local enhancement data, then Active objects, then the Level 1 page. Between two objects of the same type, the latest invoked shall win (Annex A.7.1); this includes default objects (decision E.53). Ties between attributes shall be resolved by Annex C.2, with the order in which the walk of REQ-FSM-011 meets them as the order of transmission. The displayed attributes of a cell shall be those of the plane whose character it shows, as REQ-FSM-012 describes. Transmissions that break the overlap and interleave rules of Annex A.7 shall be displayed by these same rules.
*Verification:* Test with overlapping objects of each pair of types.

**REQ-OBJ-013 — Triplets restricted to local data and Active objects.**
Full Screen Colour, Full Row Colour and Address Display Row 0 triplets inside Adaptive or Passive objects shall be ignored entirely (decision E.51). Inside an object whose origin lies in a side panel, Full Screen Colour and Full Row Colour shall be ignored (clause 12.5.3).
*Verification:* Test.

**REQ-OBJ-014 — Where objects may write.**
An element of an object, meaning a character or an attribute, that would be placed outside rows 0 to 24 or columns 0 to 71 shall be discarded, and the object shall continue (decision E.51). Elements placed in row 0, columns 0 to 7 (REQ-L25-050), and elements that decision E.49 discards in or across the side panels, shall be discarded in the same way.
*Verification:* Test with objects placed at every edge.

**REQ-OBJ-020 — Limits on objects.**
The limits of Annex A.8 shall be enforced as fixed bounds (decision E.51):
- at most 127 invocations of each object type per page, side panels and default objects included;
- at most 15 objects open at once, where an object is open from the active position at its invocation up to and including the last position it addresses, in scan order (Annex A.8, Figure A.1);
- at most 15 objects opened in succession without a displayable character or attribute being written in between.

Invocations beyond a limit shall be ignored and counted. The work done to build a page's grid shall be bounded by these limits (REQ-SEC-012), and enhancement processing shall use no allocation.
*Verification:* Test with pages that reach and exceed each limit; the bounded-work test of REQ-TST-019.

**REQ-OBJ-030 — The enhancement pages a page uses.**
The GPOP, POP, Global DRCS (GDRCS) and DRCS pages that a page uses, with their sub-pages, shall be taken from the page's own X/27/4, and at Level 3.5 also X/27/5, when those are present, and from the Magazine Organization Table otherwise (Annex D.1.4). The links of X/27/4 and X/27/5 shall take precedence whatever the table's X/27/4,5 flag says, and shall be read as decision E.52 describes:
- X/27/4 links 0 to 3 are, by position, the GPOP, POP, GDRCS and DRCS links. A link whose function bits do not match its position shall be ignored.
- At Level 3.5, X/27/4 links 4 and 5 and X/27/5 links 0 and 1 take their role from their function bits.
- A link shall be used only when its Format 1 marker bits are as Table 15 requires, and its page validity includes the presentation level.
- Page validity 00, all sixteen sub-page flags at 0, or page number FF shall mean no link.
- A link that duplicates one already accepted shall be ignored.
- The sub-page flags shall be used only as hints for acquisition.
*Verification:* Test with the worked link of `research/notes-level25-objects.md` section 13.2, and with each case that makes a link unusable.

**REQ-OBJ-031 — The Magazine Organization Table.**
For a page P of magazine m, the decoder shall use the stored Magazine Organization Table of m, page mFE, sub-page S1 = 0. A table on another sub-page shall be ignored (decision E.52). From P's byte pair (REQ-PKT-018) it shall take the global object flag, the POP link number, the global DRCS flag and the DRCS link number. The links shall be read:
- at Level 2.5, from X/19 to X/21;
- at Level 3.5, from X/22 to X/24. When X/22 or X/24 is missing, the Level 2.5 links shall be used, and the page shall be processed at Level 2.5 (clause 10.6.1). When only X/23 is missing, POP links 4 to 7 shall be taken from X/20.

The magazine of a link is absolute, with 0 meaning magazine 8. A link to page FF means "not used". The byte pair for page xFF shall be ignored. The global flags shall be used only as hints for acquisition, and a GPOP or GDRCS reference shall be resolved through the magazine's link whatever the flag says. The fallback bits of P's POP link shall supply side panels and Black Background Colour Substitution when REQ-L25-031 reaches them; with no POP link, they shall be clear (decision E.53).
*Verification:* Test with the table positions of `research/notes-level25-objects.md` sections 7.2 and 13.3, at both levels and with each Level 3.5 packet missing.

**REQ-OBJ-032 — Sub-pages of enhancement pages.**
An invocation shall name the sub-page of its object page by S1 directly. A link's sub-page count n shall mean that sub-pages 0 to n are assumed; missing ones are absent. A sub-page beyond the advertised count shall be used if it is stored (decision E.52).
*Verification:* Test.

**REQ-OBJ-040 — Default objects.**
When a page has no packet X/26 that carries a display triplet, the decoder shall place the default objects that the Magazine Organization Table gives (clause 13.8; decision E.53):
- up to two from the GPOP link of the page's magazine, which apply to every such page of the magazine whatever the page's global object flag says;
- up to two from the page's POP link.

They shall be placed in the order GPOP 1, GPOP 2, POP 1, POP 2, each with its origin at row 0, column 0, and the active position shall be row 0, column 0 before and after each. Each default object number shall be resolved as REQ-OBJ-004 describes, with N8 = 0, so that only the first 16 objects of each type on each sub-page can be reached. A page whose X/26 packets carry only Programme Delivery Control triplets shall keep its default objects.
*Verification:* Test with the worked default object of `research/notes-level25-objects.md` section 13.4, and with pages that have and lack local display data.

**REQ-OBJ-041 — Enhancement pages are never shown as text.**
Object pages, DRCS pages and the Magazine Organization Table shall never receive Level 1 national option or display semantics, and shall not be rendered as text pages (REQ-ACQ-014). On these pages, bit C12 means a continued fragment and bit C13 a partial page (Annex B.6, Table B.1).
*Verification:* Test.

---

## F.9c Dynamically redefinable characters (DRC)

This area specifies Dynamically Redefinable Characters (DRCS, clause 14): the pages that carry them, the four modes, their colours and how they are drawn. Decision E.54 applies.

**REQ-DRC-001 — DRCS pages.**
A page shall be treated as a DRCS page when its X/28/0 or X/28/3 page function is 0100 (Global DRCS) or 0101 (normal DRCS), or when a GDRCS or DRCS link names it (REQ-OBJ-030 and REQ-OBJ-031). Each sub-page, identified by S1, is one sub-table of 48 Pattern Transfer Units (PTUs) (clause 14.1). Packets X/1 to X/24 each carry two PTUs: PTU 2k is bytes 6 to 25 of packet X/(k + 1), and PTU 2k + 1 is bytes 26 to 45 (Figure 27). Packet X/25 is not used.
*Verification:* Test with the layout of Figure 27 (REQ-TST-016).

**REQ-DRC-002 — D-bytes.**
Each PTU byte shall be checked for odd parity. A valid byte, called a D-byte, has a code from 4/0 to 7/F, and its six least significant bits are six dots, the most significant of them the leftmost (clause 14.2.1). D-byte i of a PTU, numbered 0 to 19, covers row i / 2 of the 12 by 10 matrix, and columns 0 to 5 when i is even or 6 to 11 when i is odd. A byte that fails its parity check, or whose code lies outside 4/0 to 7/F, shall make every character using that PTU invalid (decision E.54).
*Verification:* Test with the worked PTUs of `research/notes-level25-drcs.md` section 3.5.

**REQ-DRC-003 — Mode identification.**
The mode of each PTU shall be taken from the page's X/28/3 (REQ-PKT-014), in the order decision E.54 gives. Without X/28/3 every PTU is mode 0 (Annex D.1.7). The first PTU of a character carries its mode. Each further PTU of a mode 1 or mode 2 character shall be marked 1110, and a PTU marked 1111 has no data (Table 9). A character shall be invalid when its first PTU is marked 1110, 1111 or a reserved value, when a further PTU is not marked 1110, or when it would run past PTU 47. At Level 2.5, a PTU not marked mode 0 shall be unusable.
*Verification:* Test with the worked encoding of `research/notes-level25-drcs.md` section 4, and with each invalid case.

**REQ-DRC-004 — Pixel values of each mode.**
The pixels of a DRCS character shall be decoded according to its mode (Table 31; clauses 14.2.1 to 14.2.4):

| Mode | Format | PTUs | Pixel value |
|---|---|---|---|
| 0 | 12 by 10 by 1 | 1 | One bit: 1 for foreground, 0 for background |
| 1 | 12 by 10 by 2 | 2 | Plane 0 from the first PTU plus 2 × plane 1 from the second, indexing a four-entry DRCS colour table |
| 2 | 12 by 10 by 4 | 4 | Planes 0 to 3 from the four PTUs, the first the least significant, indexing a sixteen-entry DRCS colour table |
| 3 | 6 by 5 by 4 | 1 | D-byte 4r + k is plane k of row r, with six pixels each, the most significant bit leftmost; the four planes index a sixteen-entry DRCS colour table |

The global or normal colour table is used according to whether the character comes from the global or the normal table (REQ-DRC-007).
*Verification:* Test for each mode with characters built by the encoder.

**REQ-DRC-005 — The DRCS Mode triplet.**
Row mode 11000 shall choose the sub-table, T3 to T0, of the normal table when D6 is 1 and of the global table when D6 is 0, for later DRCS characters. It shall not move the active position. When its level bits L1 and L0 exclude the presentation level, the triplet shall be ignored; the reserved value 00 shall count as both levels (decision E.54). The chosen sub-table shall persist to the end of the local enhancement data or of the object in which the triplet occurs. It shall not pass into an invoked object, nor back from one (Table 28 note; Annex C.4 rule 3). At the start of the local enhancement data and of each object, the normal and global sub-tables shall both be 0.
*Verification:* Test with the worked DRCS Mode triplets of `research/notes-level25-drcs.md` section 5.1, and with objects that do and do not choose their own sub-tables.

**REQ-DRC-006 — The DRCS Character Invocation triplet.**
Column mode 01101 shall set the column and write a DRCS character: from the normal table when D6 is 1 and from the global table when it is 0, in the sub-table currently chosen, with the character number in D5 to D0. A number from 48 to 63 shall make the character invalid. The cell shall record the table, the sub-table and the character (REQ-L25-006). The DRCS page shall be the one the displayed page's GDRCS or DRCS link names (REQ-OBJ-030), also for characters inside objects (Annex E.6; Table 23 note 2).
*Verification:* Test with the worked invocations of `research/notes-level25-drcs.md` section 5.2.

**REQ-DRC-007 — DRCS colour lookup tables.**
At Level 3.5, the four DRCS colour lookup tables shall be taken from the page's X/28/1, or, when it is absent or invalid, from the magazine's M/29/1, or otherwise from the defaults of Annex D.2.2: entries 0 to 3 of the Colour Map in both four-entry tables, and entries 0 to 15 in both sixteen-entry tables (decision E.54). Each entry is a Colour Map index (Figure 28). The tables, and the patterns of every DRCS character that the page's cells use, shall travel with the grid (REQ-L25-006), so that the renderer needs nothing else. The capacity for those patterns shall be set by the caller (REQ-GEN-003); a character beyond it shall be treated as missing, and counted.
*Verification:* Test with the Figure 28 example values (REQ-TST-016), the defaults and each precedence case.

**REQ-DRC-008 — Colours and attributes of DRCS characters.**
A mode 0 character shall take the foreground and background colours at its cell, and every attribute shall act on it as on any other character (clause 14.2.1). For modes 1 to 3, each pixel's colour shall be its colour table entry, and an entry of CLUT 1 entry 0 shall be transparent (Annex C.1). For those modes (decision E.54):
- flash shall follow decision E.48;
- invert shall have no effect;
- conceal shall show a space in the background colour;
- boxing shall follow the cell.

Underline shall not be drawn on any DRCS character, and font styles shall not apply to them. Enlarged characters shall be drawn by exact pixel replication (REQ-RND-038).
*Verification:* Test for each attribute and each mode.

**REQ-DRC-009 — Missing, unusable and invalid DRCS data.**
A DRCS sub-page shall be usable once every PTU that a referenced character needs has been received; later partial receptions replace the PTUs they carry (decision E.54). A cell whose DRCS data is missing or not yet usable shall show a space in its background colour. A cell whose DRCS data is invalid shall show a space, and a diagnostic shall be traced (decision E.50).
*Verification:* Test with missing packets, partial receptions and each invalid case of REQ-DRC-002 and REQ-DRC-003.

**REQ-DRC-010 — Limits on DRCS use.**
The Level 2.5 limits of two sub-tables and 24 characters per page, and the Level 3.5 limit of 384 PTUs, shall not be enforced; they bind the broadcaster (decision E.54). The only limits shall be the capacity of the enhancement page store (REQ-ACQ-019) and the pattern capacity of REQ-DRC-007.
*Verification:* Test with a page that uses more than the limits of each level.

---
## F.10 Navigation and viewer functions (NAV)

**REQ-NAV-001 — Page number entry.**
The viewer shall be able to request a page by entering three digits. The first digit is the magazine, 1 to 8, and the other two are 0 to 9.
- While digits are being entered, the partial number shall be shown in the row 0 status area.
- Digits that cannot form a valid page number shall be rejected.
- Hexadecimal page numbers cannot be entered.
*Verification:* Test; demonstration.

**REQ-NAV-002 — Next and previous page.**
The viewer shall be able to step to the next or previous page number. Two behaviours shall be available, selected in the configuration:
- in numerical order;
- to the next or previous page actually present in the store.

Page 899 wraps to 100.
*Verification:* Test.

**REQ-NAV-003 — Links: the four coloured keys and Index.**
A page has at most four links, one for each coloured key, plus the Index key (decision E.36). The links come from either of two sources:
- **Pages from broadcast or T42 sources:** packet X/27/0. Links 1 to 4 are the red, green, yellow and cyan keys, and link 6 is the Index key.
- **Pages from TTI files and web services:** the link lines of the TTI Link Extension (Appendix E). The page encoder also turns these into packet X/27/0 (REQ-INP-019).

Pressing a coloured key requests that key's target. A null or absent link leaves the key without effect. A target with sub-code 3F7F requests any sub-page; any other sub-code requests that sub-page. When the page has no links at all, the coloured keys have no effect, and the Index key requests the initial page (REQ-SVC-004, Annex H.4). Link 5 of packet X/27/0 is used only for pre-fetching (REQ-NAV-004), never as a link the viewer can follow.
*Verification:* Test for each key, each source and each null case.

**REQ-NAV-011 — The receiver draws the link row.**
For pages whose links come from the TTI Link Extension, the receiver shall draw row 24 itself, from the link data. Any row 24 the page transmits shall be ignored. The row is divided into four equal prompts of 10 columns each, in the order red, green, yellow and cyan. Each prompt consists of:
- an alphanumeric colour attribute in the key's colour;
- then the link's label, of at most 9 characters.

A key with no link shows blank columns. A link to a page in another service shall begin its label with the receiver's mark for leaving the service: the right guillemet "»" from the G2 set. Its label then has at most 8 characters of the author's text.

The row shall be drawn in the receiver's fixed style:
- a black background;
- the key colour for text;
- no flash, conceal, double height or other attribute from the page.

Because the receiver draws the row, every link prompt is always shown whole (REQ-SEC-024).
*Verification:* Test that page attempts to alter row 24 have no effect, and that the drawn row matches the link data exactly.

**REQ-NAV-012 — Following a link to another service.**
When a coloured key's link leads to a different service (REQ-WEB-015), and that origin has not been confirmed before in this session, the link shall not be followed at once. Instead:
- the receiver-owned status area shows that the receiver is asking (REQ-SEC-005);
- the receiver redraws row 24 to show the destination service's name as the descriptor gives it, and its origin, with the instruction to press the same key again to go.

Pressing the same key again follows the link and marks the origin as confirmed. Any other key cancels. Links within the same service, and the Index key, are followed at once.

This requirement applies to `ttx view` and to ports without a browser interface. In the browser (area BRW), every link to another channel goes through the tuning screen of REQ-NAV-014 instead (decision E.65).
*Verification:* Test of confirmation, cancellation, and the already-confirmed case.

**REQ-NAV-004 — Pre-fetching linked pages.**
*Phase:* Phase 2 (whole requirement), because it depends on request mode (REQ-ACQ-006).
In request mode, when a page with Fastext links is displayed, the decoder should automatically place requests for all six linked pages, including link 5, in the spare acquisition slots. This lets the coloured keys respond at once, as the Fastext scheme intends (Annex H). On web services, pre-fetching shall fetch only from the service of the page being viewed (REQ-SEC-013).
*Verification:* Test.

**REQ-NAV-005 — Sub-pages.**
The viewer shall be able to:
- show whichever sub-page of the current page arrived most recently (rotating display, the default);
- step forward and backward through the stored sub-pages;
- enter a sub-page number directly.

The status area shall show the current sub-page number and the number of sub-pages stored.
*Verification:* Test; demonstration.

**REQ-NAV-006 — Hold.**
The viewer shall be able to hold the displayed page, so that it is not updated by new receptions until hold is released.
*Verification:* Test.

**REQ-NAV-007 — Reveal, mix and zoom.**
*Phase:* Phase 1 for latched reveal, mix and reveal clearing; Phase 2 for momentary reveal and page zoom.
The viewer shall be able to toggle reveal, toggle mix mode, and cycle page zoom through three states: top half, bottom half and off (REQ-L1-020).

Reveal shall work in either of two configurable styles:
- **Latched**, which is the default.
- **Momentary**, with a timeout of 160 milliseconds after the last request, like the SAA5050.

Reveal shall be cleared when a different page is selected.
*Verification:* Test.

**REQ-NAV-008 — Home.**
A Home function shall request the initial page (REQ-SVC-004).
*Verification:* Test.

**REQ-NAV-009 — Entering values into forms.**
On a page with an interactive form (REQ-WEB-010 and Appendix C), the viewer shall be able to move between fields, edit them and submit. Both input devices shall be supported.

**With a keyboard:**
- the Tab key and Shift-Tab move between fields;
- typing enters characters;
- the arrow keys move within a field and step through SELECT and RADIO choices;
- the space bar toggles a checkbox;
- the Enter key presses the focused button.

**With a remote control only** (digits, the four colour keys and a few others):

| Key | Action |
|---|---|
| Red | Previous field |
| Green | Next field |
| Yellow | Change the focused field. It steps SELECT and RADIO choices, toggles a checkbox, and switches TEXT entry between letters and digits. |
| Cyan | Presses the focused button, or submits the form from any field |
| Index | Leaves the form without submitting |

In letter mode, TEXT entry uses the digit keys with repeated presses, as on a telephone keypad ("2" gives A, B, C and so on). In digit mode, the digit keys enter digits directly.

While a form has focus, the digit keys shall not change the page. The status area shall show which mode is active.
*Verification:* Test of each key in both input methods; demonstration.


**REQ-NAV-010 — Back and forward history.**
The viewer shall keep a history of the pages it has displayed, like a browser. Each entry records:
- the service;
- the page number;
- the sub-page;
- whether the page came from a form response.

**Moving through history:**
- **Back** returns to the previous page. **Forward** undoes a Back.
- Selecting a new page by any means clears the forward history.
- A form submission's response page is recorded in the history. Back from it returns to the form, with the values already entered (REQ-WEB-011).

**Keys:** On a keyboard, Back and Forward are the Backspace key and Alt with the left and right arrows. On a remote control, which has no standard back key, they are two configurable keys. The default is to hold the Index key for Back.

**Other rules:**
- The history holds a configurable number of entries, 32 by default, in caller-provided memory (REQ-GEN-003).
- Returning to a page shows the stored copy at once, and checks it is current as REQ-WEB-007 describes.
- History is never saved between runs unless the port enables it, and it can be wiped, like the cookie store (REQ-SEC-013).

*Verification:* Test of back, forward, clearing and the form case.

**REQ-NAV-013 — Links to addresses, page numbers and runs of text.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
A link target may be a page of the current service, a page of a named service, or an address (Appendix E.1, decision E.64). The browser shall follow:
- **coloured keys and Index** to any of these targets;
- **page-number links** (`LK`): typing that number, or choosing the number printed on the page, while the page is shown;
- **text links** (`LT`): choosing the linked run of text.

A typed number shall follow the current page's own link for it, otherwise the channel's alias for it (REQ-WEB-020), otherwise the current channel's page with that number. A link to a page of the current channel is followed at once. A link to another channel, whether by a named service or an address, shall go through the tuning screen (REQ-NAV-014); an address in the current channel's own folder is a page of the current channel (REQ-WEB-026). A link to an ordinary web address goes through the tuning screen like any other, and then to discovery (REQ-WEB-028). `ttx view` keeps REQ-NAV-012 for named services, and does not yet follow addresses, page-number links or text links.
*Verification:* Test of each link source and target form, and of the order of page link, alias and channel page.

**REQ-NAV-014 — The tuning screen.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
Before the browser shows anything from another channel (REQ-WEB-018), it shall clear rows 1 to 24 and show the tuning screen (decision E.65 item 3). Another folder on the same origin is another channel. The screen shall show:
- the destination's host, in double height, shortened from the left so that its end always shows, and the destination's folder on the row below it;
- the channel's folder address, followed by the page number when there is one, wrapped over up to four rows, with a mark when it is cut. Which of a page's two files answers is not known until it is fetched, so the page is given by its number;
- whether the connection is encrypted; a plain HTTP destination is stated in red;
- why the browser is going there: a typed address, a choice from bookmarks or history, a restored tab, a redirect and the channel that sent it, a link and the channel it is on, or a pointer and the web address that announced it (REQ-WEB-028), with the address the viewer asked for when it differs. For a link, the name the channel's `channel.json` gives the destination, or that the address was written in the page and not declared. An ordinary web address, before discovery, is shown as such, with the note that the browser will look for its teletext edition;
- the keys: Enter to go, and Escape or Delete to stay.

Row 0 shall show `GO?` and the destination. The screen shall be shown for a typed address, a bookmark or history entry, a restored tab, a link to another channel, and a redirect to another channel. It shall be skipped only when going back or forward to a channel confirmed earlier in the same session, and when discovery leads to a channel on the same origin as the address the viewer has just confirmed, in that address's folder or a folder inside it (decision E.73 item 7). For an address that is not a folder, its folder is its path up to and including the last `/`. Confirmed channels shall not be remembered between runs. Only the viewer's own key press confirms; nothing a page or a decoder sends can.
*Verification:* Test of each reason for the screen, of the skipped case, and that a page drawing a copy of the screen cannot confirm anything (the hostile test channel of REQ-TST-081).

**REQ-NAV-015 — Page catching and showing links.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The viewer shall offer page catching (decision E.66):
- Enter, when nothing else uses it, turns page catching on at the first link in reading order. The links are the page numbers on rows 1 to 23 that lead to a page, and the runs of `LT` lines.
- Right and Down move to the next link, Left and Up to the previous one, and Enter follows the selected link.
- Escape or Delete turns page catching off. Any other key turns it off and then does what it does.
- The selected link is shown with its foreground and background colours swapped, drawn by the receiver. Row 0 shows `LINK` and the selected link's destination.
- Pages with side panels are not caught.

A show-links key, `l`, shall underline every link on the page with the receiver's own underline until it is pressed again. With a pointer, the destination of a page number under the pointer shall be shown outside the page, and a click shall follow it.
*Verification:* Test of turning on, moving, following and turning off; test that only real links can be selected.

---

## F.11 Rendering and output (RND)

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

At Levels 2.5 and 3.5 the cell raster shall also do, and nothing outside it shall repeat: double width and double size (REQ-RND-034), underline (REQ-RND-035), G3 drawing (REQ-RND-037), DRCS drawing (REQ-RND-038) and the font styles (REQ-RND-039). For a multi-colour DRCS character it returns a colour index for each dot (REQ-RND-038), and in the resolved colour mode its indices are Colour Map indices (REQ-RND-031).
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
*Verification:* Test against golden cell-raster outputs for alphanumerics, mosaics, G3 and DRCS characters at double width and double size.

**REQ-RND-035 — Underline.**
Underline shall be drawn as one line of foreground colour on the last dot line of the cell, across the whole cell width. In the 12 by 20 cell that is line 19, so it appears on both fields; in the 12 by 10 cell it is line 9 (decision E.47). It shall be drawn for G0 characters, G1 characters of columns 4 and 5, and G2 characters, and never for G3 or DRCS characters. For a double-height character, only its bottom half shall show the underline.
*Verification:* Test against golden cell-raster outputs.

**REQ-RND-036 — Invert, flash and conceal when drawing.**
For each cell the renderer shall, in this order (decisions E.47 and E.48):
1. swap the foreground and background colours if the cell is inverted, except for multi-colour DRCS characters;
2. apply flash: in the off phase, normal and inverted flash show the background colour in place of the foreground (and a space in the background colour for multi-colour DRCS characters), while adjacent-CLUT flash replaces each foreground index c, or each DRCS pixel index c, by c exclusive-OR 8;
3. apply conceal: unless reveal is on, a concealed cell shows a space in its background colour.
*Verification:* Test for every combination of invert, each flash mode and conceal.

**REQ-RND-037 — G3 glyph data.**
The G3 characters shall be drawn from glyph data generated by an `xtask` command from the project's own geometry, as decision E.50 describes. The data shall be generated separately for the 12 by 10 and the 12 by 20 cells, at the full 12 half-dot width, and stored as replaceable text data with its provenance recorded, like the other glyph sets (REQ-RND-020). The generated data shall be used for G3 characters at every presentation level. Character rounding (REQ-RND-012) shall not be applied to G3 characters. The generator shall be deterministic, and continuous integration shall check that the stored data matches its output.
*Verification:* Test that regenerating the data gives identical files; golden cell-raster outputs for every G3 position in both cell sizes; the symmetry checks of `research/notes-level25-drcs.md` section 9.3 (column 3 mirrors column 2, and columns 6 and 7 are the complements of columns 2 and 3).

**REQ-RND-038 — Drawing DRCS characters.**
A DRCS character shall be drawn from the pattern carried with the grid (REQ-DRC-007). In the 12 by 10 cell, each of its ten rows shall be one line; in the 12 by 20 cell, each row shall be drawn on two lines, the same on both fields. A mode 3 pixel shall cover 2 by 2 dots in the 12 by 10 cell and 2 by 4 in the 12 by 20 cell. Enlarged characters shall be drawn by exact pixel replication, with no smoothing and no rounding (decision E.54). For a multi-colour DRCS character, the cell raster (REQ-RND-016) shall return a Colour Map index for each of the 12 dots of the line instead of a single foreground index.
*Verification:* Test against golden cell-raster outputs for each mode, size and cell height.

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
The text renderer (REQ-RND-008) shall write each row composed with its side panels, in the order of REQ-RND-033. It shall convert G3 characters as REQ-CHR-011 describes, and write DRCS characters as the fallback value of REQ-CHR-010. The terminal renderer (REQ-RND-009) shall take 24-bit colours from the page's Colour Map. In its 8-colour mode it shall use the nearest of the eight Level 1 colours. Transparent cells shall be written in the screen colour. All output shall pass the sanitising rules of REQ-SEC-010.
*Verification:* Test with snapshot outputs of pages with side panels, G3, DRCS and redefined colours.

---

## F.12 Input formats (INP)

**REQ-INP-001 — Reader interface.**
Every input format shall be read by a streaming reader. The reader takes bytes as they arrive, in pieces of any size, and produces packets with metadata (REQ-ARCH-002 and REQ-ARCH-003) or pages. Its memory use shall be bounded and independent of file size. Readers shall work on regular files, pipes and standard input.
*Verification:* Test that feeds each format one byte at a time and in large blocks, and compares the results.

**REQ-INP-002 — T42 packet streams.**
The reader shall accept the "T42" format:
- a headerless sequence of 42-byte packets;
- each packet consists of bytes 4 to 45 of the broadcast packet, in transmission bit order, with parity and Hamming coding intact.

This is the format produced and consumed by vhs-teletext, vbit2 and raspi-teletext.
*Verification:* Test with files from those tools.

**REQ-INP-003 — Packet streams with run-in and framing code.**
The reader shall accept two further variants:
- 43-byte packets that begin with the framing code 0x27;
- 45-byte packets that begin with the clock run-in 0x55 0x55 and framing code 0x27. This is the "raw" output of vbit2.

The leading bytes shall be checked and removed.
*Verification:* Test.

**REQ-INP-004 — Bit-reversed packet streams.**
The reader shall accept 42-byte packet streams in which every byte is bit-reversed. This is the output of vbit2's "reverse" option, and the byte order of Digital Video Broadcasting.
*Verification:* Test.

**REQ-INP-005 — Format detection.**
When the format is not specified, it shall be detected in this order:
1. **Text or byte signatures.** Page files begin with text commands such as `PN,` or `DE,`. Transport streams have the byte 0x47 every 188, 192 or 204 bytes. Elementary streams begin with 00 00 01 BD. EP1 files begin with FE 01.
2. **Headerless binary streams.** For each packet length and bit order in REQ-INP-002 to REQ-INP-004, decode the first several hundred packets and choose the variant in which the packet address bytes decode without uncorrectable errors most often.

The detected format shall be reported. An explicit format option shall override detection.
*Verification:* Test with every supported format, including files with corrupted starts.

**REQ-INP-006 — MPEG transport streams.**
The reader shall accept MPEG-2 transport streams with:
- 188-byte packets;
- 192-byte packets, which have a 4-byte prefix before each 188-byte packet ("M2TS");
- 204-byte packets, which have 16 bytes of error-correction data after each packet.

It shall find the sync byte 0x47 and regain synchronisation after corruption. It shall extract the packet identifier, the payload-unit-start flag, the adaptation-field control and the continuity counter.
- It shall accept packets that carry both an adaptation field and payload, even though EN 300 472 clause 4.1 permits only one or the other.
- It shall discard packets with the transport-error flag set.
- It shall report continuity-counter breaks.

The 192-byte variant is not only theoretical: off-air recordings in the public test corpus (REQ-TST-022) use it.
*Verification:* Test with generated streams and with the off-air recordings of the real-broadcast corpus (REQ-TST-022).

**REQ-INP-007 — Finding teletext in a transport stream.**
The reader shall decode the Program Association Table (packet identifier 0) and each Program Map Table. It shall find the elementary streams whose stream type is 0x06 and which carry one of these:
- a teletext descriptor (tag 0x56);
- a Vertical Blanking Interval teletext descriptor (tag 0x46);
- a Vertical Blanking Interval data descriptor (tag 0x45) listing teletext.

For each teletext descriptor entry it shall report:
- the three-letter language code;
- the teletext type (initial page, subtitle page, additional information page, programme schedule page, or subtitle page for hearing-impaired people);
- the magazine (0 meaning 8);
- the page number (two hexadecimal digits).

A single descriptor may list several entries with different languages and types for the same stream, as the off-air recordings of REQ-TST-022 do. Every entry shall be reported. When an entry identifies a subtitle page, the reader shall offer that page as the default page for subtitle display.

The user shall be able to select a stream by packet identifier or by programme. Without a selection, the first teletext stream shall be used and the choice reported.
*Verification:* Test, including recordings with several descriptor entries.

**REQ-INP-008 — Teletext in elementary stream packets.**
The reader shall decode packetised elementary stream packets with stream identifier 0xBD, skip the header using its length field, and read:
- the data identifier, accepting values 0x10 to 0x1F and 0x99 to 0x9B, and rejecting others with a diagnostic;
- the data units that follow.

For data units with identifier 0x02 (teletext), 0x03 (teletext subtitles) or 0xC0 (inverted teletext), it shall:
- read the field parity and line offset into the packet metadata, where line offset 0 means unknown;
- check the framing code: 0xE4 in this byte order for teletext, and 0x1B for inverted teletext;
- bit-reverse each of the following 42 bytes to obtain the packet in the internal form.

All other data units, including stuffing (0xFF), shall be skipped using their length field, whatever their length. The presentation time stamp, where present, shall be converted to the packet time stamp.
*Reason:* EN 300 472 clause 4.4 inserts bits "in the same order as they would appear in the VBI", which makes every byte the bit-reverse of the internal form.
*Verification:* Test with streams generated by vbit2 and with captured broadcast streams.

**REQ-INP-009 — Raw elementary stream files.**
The reader shall accept a file that is a plain sequence of the packetised elementary stream packets of REQ-INP-008, without a transport stream around them.
*Verification:* Test.

**REQ-INP-010 — Linux sliced data.**
The reader shall accept the Linux sliced-data record format: 64-byte records made up of four 32-bit fields (service identifier, field, line and reserved) followed by 48 bytes of data. Records with service identifier 0x0001 (teletext system B) shall give a packet from the first 42 data bytes, with field and line placed in the metadata. Records with identifier 0 shall be ignored. The 32-bit fields shall be read in the byte order of the computer that wrote the data, which is configurable and defaults to little-endian.
*Verification:* Test.

**REQ-INP-011 — Raw sampled video files.**
The reader shall accept headerless files of raw sampled lines from the blanking interval and pass each line to the software data slicer (area SLC). The file geometry shall be configurable:
- sample rate;
- samples per line;
- sample size (8-bit, or 16-bit little-endian);
- lines per field;
- which of those lines to use;
- the line number of the first line of each field.

Presets shall be provided matching the vhs-teletext tool:

| Preset | Sample rate | Samples per line | Sample size | Lines per field | Lines used |
|---|---|---|---|---|---|
| bt8x8 (default) | 35,468,950 Hz | 2048 | 8-bit | 16 | All 16 (7 to 22 and 320 to 335) |
| cx88 | 35,468,950 Hz | 2048 | 8-bit | 18 | 1 to 16 |
| saa7131 | 27,000,000 Hz | 1440 | 8-bit | 16 | |
| tbc | 17,730,000 Hz | 1135 | 16-bit | 313 | 6 to 21 |
| tbc-vbi | 17,730,000 Hz | 1135 | 16-bit | 16 | |

The last four bytes of each bt8x8 frame may hold a frame sequence number written by the Linux driver, and shall not be allowed to cause errors.
*Verification:* Test with the vhs-teletext sample captures.

**REQ-INP-012 — Withdrawn (live capture on Linux).**
*Withdrawn on 1 October 2026.* Capture devices are hardware supplied by the port through the data source of REQ-ARCH-009. The file formats such devices produce remain supported (REQ-INP-010 and REQ-INP-011). The number is kept and will not be reused.

**REQ-INP-013 — TTI page files.**
The reader shall accept TTI page files (MRG Systems format, with the extensions used by vbit2):
- Lines may end with carriage return and line feed, or line feed alone.
- Commands are recognised by their first two letters.

The commands shall be handled as follows:

| Command | Handling |
|---|---|
| `PN,mppss` | Page number and sub-page. A new `PN` line starts a new sub-page of a carousel. |
| `SC,hhhh` | Sub-code in hexadecimal. |
| `PS,hhhh` | Page status. The bits map to control bits as decision E.21 describes: 0x4000 is C4, 0x0001 C5, 0x0002 C6, 0x0004 C7, 0x0008 C8, 0x0010 C9, 0x0020 C10 and 0x0040 C11. Bits 7, 8 and 9 are C12, C13 and C14. |
| `OL,n,text` | Output line for rows 0 to 25. |
| `OL,26`, `OL,27`, `OL,28`, `OL,29` | Enhancement, link and magazine packets. The first character's low four bits are the designation code. Each following group of three characters carries 18 bits, six bits per character with the least significant character first. For link packets with designation codes 0 to 3, the characters carry four-bit values. |
| `FL,` | Six three-digit hexadecimal Fastext links, where 0 and 8FF mean no link. |
| `LN,key,target,label` | A link of the TTI Link Extension (Appendix E), which takes precedence over `FL`. |
| `RE,h` | Region, which sets the character set group. |
| `PF,f,c` | Page function and page coding. |
| `DE,text` and `DS,text` | Description, kept as metadata. |
| `CT` | Cycle time, ignored. |
| Unknown commands | Ignored with a diagnostic. |

In `OL` text, a control code may appear in any of these forms:
- as the byte plus 0x80;
- as the escape byte 0x1B followed by the code plus 0x40;
- in the case of double height, as 0x10, which shall be read as 0x0D, as the MRG format specifies.

The page shall be converted to packets (REQ-ARCH-004).
*Verification:* Test with files from vbit2, QTeletextMaker and edit.tf.

**REQ-INP-014 — TTI writer.**
A writer shall produce TTI files from stored pages. It shall write control codes in the escape form, so that the files are plain text, and write enhancement packets in the form described in REQ-INP-013. Reading a written file shall reproduce the same page.
*Verification:* Round-trip test (REQ-TST-020).

**REQ-INP-015 — EP1 page files.**
The reader shall accept EP1 page files:
- a 6-byte header (FE 01, language code, enhancement flag, two-byte little-endian offset);
- when the flag is 0xCA, an optional enhancement block of up to sixteen 40-byte packets, each holding a designation code and thirteen triplets stored as unprotected address, mode and data bytes, with the terminator stored as address 0x7F;
- 24 rows of 40 bytes;
- a further 40-byte row;
- a 2-byte terminator.

A file without enhancements is 1008 bytes long.
*Verification:* Test with files exported by the zxnet and edit.tf editors.

**REQ-INP-016 — edit.tf and zxnet page addresses.**
*Phase:* Phase 1 for the reader; Phase 2 for the writer.
The reader shall accept page data in the web address form used by the edit.tf and zxnet editors:
- a hexadecimal metadata digit, where the low three bits select a character set;
- a colon;
- a base64url string (alphabet A–Z, a–z, 0–9, "-" and "_", no padding).

The string carries 25 rows by 40 columns of 7-bit codes. The code for row r and column c occupies bits 280r + 7c to 280r + 7c + 6, most significant bit first, giving 1167 characters in all. The older 1120-character (24-row) form shall also be accepted. Optional `key=value` fields shall be read where defined:
- `PN` page number;
- `SC` sub-code;
- `PS` page status;
- `X270` Fastext links;
- `X26` enhancement triplets.

Unknown fields shall be ignored. A matching writer shall be provided.
*Verification:* Test with addresses produced by both editors.

**REQ-INP-017 — Raw screen dumps.**
*Phase:* Phase 1 for reading dumps; Phase 2 for detecting repeated double-height rows.
The reader shall accept raw page dumps of 1000 bytes (25 rows by 40 columns) or 960 bytes (24 rows), masking each byte to 7 bits. This includes the BBC Microcomputer form, in which every byte has its top bit set. Because that computer needs the row below a double-height row to repeat it, an option shall detect such repeated rows and treat them as the ignored lower row.
*Verification:* Test.

**REQ-INP-018 — T42 writer.**
A writer shall produce T42 packet streams. Sources are received packets (pass-through, optionally filtered by magazine and page) and packets encoded from pages. When encoding pages, it shall follow the packet order of Annex B.2:
1. header;
2. packets 27;
3. packets 28;
4. packets 26 in ascending designation code;
5. rows.
*Verification:* Round-trip test.

**REQ-INP-019 — The page encoder.**
The page encoder shall produce correctly coded packets for:
- headers, with all control bits;
- rows with odd parity;
- packets X/26 with Hamming 24/18;
- packets X/27/0 with correct relative magazine bits and a checksum computed by REQ-ECC-007;
- for pages with TTI Link Extension links, packet X/27/0 and a row 24 (packet X/24) generated as Appendix E.2 describes;
- packets X/28/0 format 1.

The packets of Levels 2.5 and 3.5 are encoded as REQ-INP-024 describes.
*Verification:* Test that decodes encoded packets back to the original page.

**REQ-INP-020 — Reporting input errors.**
Every reader shall report malformed input with the byte offset at which it was found. It shall continue with the next recognisable unit where one can be found.
*Verification:* Test with corrupted files.

**REQ-INP-021 — Other formats.**
*Phase:* Phase 2 (whole requirement).
The following should be implemented:
- the zvbi "old sliced" capture format;
- the TTA format (a 2048-entry index followed by T42 data).

The following may be implemented later:
- the video-recorder embedded sliced format of the Linux ivtv driver;
- the SMPTE RDD 8 ("OP-47") and SMPTE ST 2031 serial digital interface formats;
- the FAB and Cebra page formats.

They are listed in Part G until they are scheduled.
*Verification:* Test, when implemented.

**REQ-INP-022 — The live WebSocket feed used by the zxnet teletext viewer.**
An optional network module shall read the live teletext feeds served to the zxnet online teletext viewer (decision E.28).

**Connecting.** Each feed ("channel") is identified by a channel identifier: base64 text encoding a JSON array of three strings, the service name, the host name and the path. The reader shall connect to the secure WebSocket address formed from the host and path. For example, the Teefax channel identifier decodes to `["teefax","zxnet.co.uk","/ws/teefax"]`, giving the address `wss://zxnet.co.uk/ws/teefax`.

The feeds known on 1 October 2026 shall be available by short name:

| Short name | Service | Address |
|---|---|---|
| `teefax` | Teefax | `wss://zxnet.co.uk/ws/teefax` |
| `nms-ceefax` | Ceefax recreation | `wss://internal.nathanmediaservices.co.uk/websockets/ceefax` |
| `chunkytext` | ChunkyText | `wss://zxnet.co.uk/ws/chunkytext` |
| `spark` | SPARK | `wss://zxnet.co.uk/ws/spark` |
| `ch4` | Channel 4 capture: a real-time replay of a broadcast of 15 July 2007 | `wss://zxnet.co.uk/ws/ch4` |
| `test` | Test service | `wss://zxnet.co.uk/ws/test` |

A channel identifier or an address may also be given directly. The short name is only a label for the user. The service name sent to the server is the first string of the channel identifier, and it can differ from the short name: the `ch4` identifier carries the service name `channel4-capture`. Sending a different name draws a `wrongname` message.

**Messages sent by the client.** Client messages are text frames holding comma-separated values, which is how the viewer's JavaScript sends its arrays. After connecting, the reader shall send:
- `service` followed by the channel identifier;
- `ttx` followed by `true`, which turns teletext on;
- one `pagesearch` message for each page wanted. A `pagesearch` message carries, in order:
  - a slot number;
  - the magazine;
  - the page number;
  - the sub-code, where 16255 (hexadecimal 3F7F) means any sub-page;
  - three true or false flags.

  The viewer uses slot 0 for the displayed page and further slots, up to at least slot 10, for linked pages.
- `keepalive` every 5 seconds.

When the server sends `apiver`, the reader shall reply with `apiver` and the protocol version it understands, which is 1 at the time of writing.

**Messages received.** Server messages are JSON arrays whose first element names the message:

| Message | Contents | Handling |
|---|---|---|
| `header` | slot, the 40 bytes of header packet bytes 6 to 45 in base64 (raw, including the Hamming-coded page address and control bytes), a page-matched flag, the control bits as a number, the magazine, the sub-code and the page number | See the conversion rules below. |
| `row` | slot, magazine, packet number, the 40 raw bytes of packet bytes 6 to 45 in base64 | Parity and Hamming coding are intact, as verified by connecting on 1 October 2026. |
| `clock` | Header clock characters | |
| `pageExists` | A magazine and page number present in the service | Used to build the service's page list. |
| `initialpage` | The initial page and the status message | |
| `bsdp` | The 40 bytes of packet 8/30, bytes 6 to 45, in base64, sent about once a second (server protocol version 1 and later) | Rebuilt as a packet 8/30 in magazine 8 and decoded as service data (area SVC). Its date and time show whether a feed is live or a replay. |
| `channelSettings`, `secondTick`, `wrongname` | Viewer settings, a timing tick, and a service-name mismatch | `wrongname` means stop reconnecting. |

In the control bit number, C4 is 0x0010, C5 is 0x0020 and so on, up to C11 at 0x0800. The national option bits occupy 0x7000.

**Conversion to packets.** The reader shall rebuild each `header` and `row` message as a complete 42-byte packet in the internal form (REQ-ARCH-002) by Hamming-encoding the packet address from the magazine and packet number, and prepending it to the 40 received bytes. The packets then pass through normal acquisition (REQ-ARCH-004). The decoded fields that the server also sends in `header` messages shall be used only to cross-check the decoder, and any disagreement shall be reported.

**Filtering.** The server sends rows only for the pages requested in `pagesearch` slots, so the feed is a filtered stream, not a whole-service stream. To capture a whole service, the reader shall offer a mode that walks the `pageExists` list, requesting pages in turn through the available slots.

When tested on 10 October 2026, only slot 0 returned rows. A page then needed to be held for longer than one page cycle: requests that changed page every 3 seconds received headers but no rows. The walking mode shall therefore hold each page until its rows have arrived, or until a timeout. It shall keep to the pace of a person using the viewer, because these are hobby servers.

**Tolerance and reconnection.** Unknown message types and extra fields shall be ignored. After a dropped connection the reader shall reconnect after 10 seconds, giving up after 5 failed attempts. It shall treat 5 seconds without a `clock` or `header` message as a dropped connection, as the viewer does.

**Build.** This reader is part of the `neoteletext-web` crate. It uses that crate's sans-IO WebSocket code and the transport supplied by the port (REQ-WEB-001), including the port's encryption. The core library remains free of networking (REQ-GEN-002).
*Reason:* This is the only live source of teletext with raw packet bytes found on the internet. It gives continuous real-time test data (REQ-TST-063).
*Verification:* Test against a recorded session replayed from a file; demonstration against the live servers.

**REQ-INP-023 — Enhancement data and enhancement pages in TTI files.**
The TTI reader (REQ-INP-013) and writer (REQ-INP-014) shall carry every packet of Levels 2.5 and 3.5, in the form of the vbit2 extensions (`research/notes-formats.md` section 5.2):
- `OL,26`: up to sixteen lines per page, each with its own designation code;
- `OL,27`: designation codes 4 and 5 hold thirteen triplets, written as for X/26;
- `OL,28`: designation codes 0, 1, 3 and 4 hold thirteen triplets. A line with designation code 0, 3 or 4 also sets the page function and page coding, overriding an earlier `PF` line;
- `OL,29` on a page numbered mFF: the magazine packets M/29/0, M/29/1 and M/29/4 of magazine m, which are encoded as magazine packets and not as part of the page;
- `PF,f,c`: the page function and page coding (Table 3).

The rows of enhancement pages shall be read and written according to their page coding:
- coding 0, used by DRCS pages: 40 characters with parity added on encoding, as for display rows;
- coding 2, used by object pages: the first character's low four bits give byte 6, and the following 39 characters hold thirteen triplets, as for X/26;
- coding 3, used by the Magazine Organization Table: 40 four-bit values, each in the low four bits of a character from 4/0 to 4/F.

Reading a written file shall reproduce the same packets.
*Verification:* Round-trip test (REQ-TST-020) with object pages, DRCS pages, a Magazine Organization Table and magazine packets; test with files written by vbit2 where available.

**REQ-INP-024 — Encoding the packets of Levels 2.5 and 3.5.**
The page encoder (REQ-INP-019) shall also produce correctly coded packets for:
- packets X/26 with every designation code from 0 to 15;
- packets X/27/4 and X/27/5;
- packets X/28/0 format 1 with every field, X/28/1, X/28/3 and X/28/4;
- the magazine packets M/29/0, M/29/1 and M/29/4;
- object pages, with byte 6 of each packet Hamming 8/4 coded and thirteen Hamming 24/18 triplets;
- the Magazine Organization Table, with every byte Hamming 8/4 coded;
- DRCS pages, with each D-byte given odd parity.

When it writes a stream, it shall follow the packet order of Annex B.2, and the order of Annex B.7 for a set of pages: the Magazine Organization Table, then the DRCS pages, then the object pages, then the Level 1 pages.
*Verification:* Test that decodes encoded packets back to the original fields, and that zvbi accepts the encoded streams (REQ-TST-023).

**REQ-INP-025 — Authoring test pages for Levels 2.5 and 3.5.**
The formats crate shall provide, behind its `std` feature, a builder for authoring Level 2.5 and 3.5 test pages and streams by function rather than by bit pattern. It shall be able to:
- write every row and column triplet of Tables 28 and 29 by name, with its fields;
- build local objects, and POP and GPOP pages, with their pointer tables computed from the objects they define;
- build a Magazine Organization Table, with each byte pair placed at the position of its page number, and with object links, DRCS links and default objects;
- build DRCS characters of every mode from pixel patterns, with the mode identification of X/28/3 computed;
- set every field of X/28/0 format 1, X/28/1, X/28/4 and the M/29 packets;
- write the result as TTI files and as T42 streams, through REQ-INP-023 and REQ-INP-024.

The builder is the source of the authored test streams of REQ-TST-024. Because the builder and the decoder are written from the same reading of the standard, the builder's output shall also be checked against zvbi (REQ-TST-023) and against the worked encodings of the research notes.
*Verification:* Test that the builder reproduces the worked encodings of `research/notes-level25-objects.md` section 13.2 and `research/notes-level25-drcs.md` sections 3.5, 4, 5 and 11.

---

## F.13 Software data slicer (SLC)

**REQ-SLC-001 — Input model.**
The slicer shall take one sampled television line at a time. Each line comes with:
- its samples;
- the sample rate;
- the sample format;
- the line and field number, where known;
- optionally, the offset of the first sample from the leading edge of the line synchronising pulse.

It shall output either no packet or one 42-byte packet with quality metadata.
*Verification:* Inspection; test.

**REQ-SLC-002 — Sample rates.**
The slicer shall work at any sample rate of at least 13.5 megahertz, which is about 1.95 samples per bit. It shall be tested at least at:
- 17.73 megahertz (2.56 samples per bit);
- 27 megahertz (3.89 samples per bit);
- 28.636 megahertz (4.13 samples per bit);
- 35.469 megahertz (5.11 samples per bit).
*Verification:* Test with synthetic signals at each rate (REQ-TST-060).

**REQ-SLC-003 — Black level.**
The slicer shall estimate the black level of each line from samples outside the data area, excluding the synchronising pulse and colour burst. It shall measure signal levels relative to it. This does in software what the black-level clamps of the SAA5231 and SDA5231 did.
*Verification:* Test with offset and drifting black levels.

**REQ-SLC-004 — Finding the clock run-in.**
- **Search.** The slicer shall search for the clock run-in over a configurable window, by default the whole line. The nominal position is 12.0 microseconds (minus 1.0, plus 0.4) from the synchronising edge, but capture offsets are often unknown.
- **What to match.** The search shall match the alternating pattern of bits 3 to 16 and shall not rely on the first two bits, which the standard warns may be reduced or absent (clause 6.1).
- **Estimates.** From the run-in it shall estimate the "0" and "1" levels, the slicing threshold midway between them, and the bit phase to a fraction of a sample.
- **Holding.** The threshold shall be held for the rest of the line, as the SDA5231 does.
*Verification:* Test with synthetic signals across the full timing tolerance.

**REQ-SLC-005 — Amplitude and quality gate.**
The slicer shall reject a line when:
- the run-in amplitude is below a configurable minimum;
- the match to the run-in pattern is below a configurable quality threshold.

The minimum amplitude shall default to 50 percent of the nominal data amplitude. The documented working range of the SAA5231 is 65 to 150 percent.
*Verification:* Test.

**REQ-SLC-006 — Bit clock.**
*Phase:* Phase 1 for the fixed bit period with phase taken from the run-in; Phase 2 for the slow phase-tracking loop.
The bit period shall be the sample rate divided by 6,937,500. The phase shall be set from the run-in. The slicer shall optionally follow slow timing drift across the line with a phase-tracking loop about ten times slower than the initial acquisition, as the SDA5231 does. This copes with the timing errors of recordings played back from tape.
*Verification:* Test with synthetic signals that have a bit rate error of up to 0.5 percent and timing jitter.

**REQ-SLC-007 — Sampling at bit centres.**
The value of each bit shall be decided at the estimated centre of the bit. The slicer shall interpolate between samples when there are fewer than four samples per bit.
*Verification:* Test at 17.73 and 27 megahertz.

**REQ-SLC-008 — Framing code.**
After the run-in, the slicer shall look for the framing code 11100100 in transmission order within plus or minus two bit periods of its expected position, with tolerance following decision E.24. It shall report whether a correction was needed.
*Verification:* Test with framing codes carrying single-bit errors.

**REQ-SLC-009 — Assembling bytes.**
After the framing code, the slicer shall assemble 336 bits into 42 bytes, taking the first bit of each byte as bit 0 (clause 7.1).
*Verification:* Test.

**REQ-SLC-010 — Rejecting other signals.**
Lines that carry other signals shall be rejected by the combination of the run-in match, the framing code check and the packet address Hamming check. Examples are the video programming signal on line 16, wide-screen signalling on line 23, test signals and scrambling data (Annex P).
*Verification:* Test with synthetic non-teletext lines and with real captures.

**REQ-SLC-011 — Equalisation.**
*Phase:* Phase 2 (whole requirement).
The slicer should offer optional compensation for high-frequency loss and for short echoes. These correspond to the high-frequency loss compensator of the SAA5231 and the echo ("ghost") filter of the TPU2735, which handled echoes of up to 0.8 microseconds on 625-line signals.
*Verification:* Test with synthetic filtered and echoed signals, when implemented.

**REQ-SLC-012 — Quality measures.**
For each line the slicer shall report:
- the measured data amplitude;
- the run-in match quality;
- the estimated timing position;
- whether the framing code was corrected.

These measures play the same role as the SAA5246A "data quality" status bit.
*Verification:* Test.

**REQ-SLC-013 — Noise performance.**
With synthetic lines generated to EN 300 706 within all transmission tolerances, the slicer shall meet two targets:
- With no added noise, it shall decode 100 percent of packets without error.
- With added white Gaussian noise at a ratio of 20 decibels between the data amplitude and the root-mean-square noise, it shall decode at least 99 percent of packet addresses correctly.

The noise figure is an initial target, to be revisited after measurement.
*Verification:* Test (REQ-TST-060).

**REQ-SLC-014 — Real recordings.**
On the sample captures published with vhs-teletext, the slicer shall recover at least as many packets with valid addresses as that tool's own conventional (non-machine-learning) slicer.
*Verification:* Comparative test (REQ-TST-061).

**REQ-SLC-015 — Fixed-point arithmetic option.**
*Phase:* Phase 2 (whole requirement).
The slicer shall be buildable in a form that uses integer and fixed-point arithmetic only, with no floating point, so that it runs efficiently on processors without floating-point hardware. Both forms shall meet REQ-SLC-013. Any difference in the packets they recover from the test signals shall be reported.
*Verification:* Test of both forms against the synthetic signals of REQ-TST-060.

---
## F.14 Compatibility profiles (PRF)

**REQ-PRF-001 — The default profile ("EN 300 706").**
The default profile shall follow EN 300 706 and every default choice in Part E:
- presentation at Level 3.5 (REQ-PRF-006);
- X/26/15 processed;
- the row-setting rule of Annex A.3;
- double Start Box;
- alpha black and mosaic black decoded (decision E.18);
- row 24 displayed according to Annex H;
- rendering in 12 by 20 cells with rounding.
*Verification:* Test.

**REQ-PRF-002 — The "SAA5050" profile.**
*Phase:* Phase 2 (whole requirement).
A profile shall reproduce a receiver built with the SAA5050 and its companion chips:
- Level 1.0 only, with packets X/26 ignored;
- codes 0/0 and 1/0 displayed as spaces with no effect (decision E.18);
- 24 rows, with row 24 never displayed;
- a single fixed national character set chosen in the configuration, English by default, with C12 to C14 ignored, as on that chip;
- 12 by 20 cells with rounding;
- flash at 0.75 hertz with a 3 to 1 ratio;
- momentary reveal with a 160 millisecond timeout.
*Verification:* Test.

**REQ-PRF-003 — The "SAA5246A" profile.**
*Phase:* Phase 2 (whole requirement).
A profile shall reproduce a receiver built with the SAA5246A and television software that performs Level 1.5:
- Level 1.5 on, with X/26/15 ignored;
- alpha black and mosaic black decoded;
- row 24 displayed whenever packet X/24 is present;
- the status area and green search marker of decision E.10;
- 12 by 10 cells without rounding.

A variant setting (/E, /H, /T, /J, /K, /L, /I or /S) shall select the character set group of Part D.5. National option combinations that the variant does not support shall fall back to that variant's default language.
*Verification:* Test.

**REQ-PRF-004 — Alternative and strict options.**
*Phase:* Phase 2 (whole requirement).
Independent of the profile, these alternative behaviours shall be available:
- the clause 12.3.2 row-setting rule (decision E.3);
- single Start Box (decision E.17);
- exact framing code (decision E.24);
- ignoring out-of-order triplets (REQ-L15-009).
*Verification:* Test.

**REQ-PRF-005 — Overriding a profile.**
After a profile has been applied, every individual setting shall remain changeable.
*Verification:* Test.

**REQ-PRF-006 — The presentation level setting.**
The configuration shall have a presentation level setting with the values Level 1, Level 1.5, Level 2.5 and Level 3.5 (decision E.40). It shall replace the on and off switch for Level 1.5. The default profile shall present at Level 3.5, the "SAA5050" profile at Level 1 and the "SAA5246A" profile at Level 1.5. The setting shall choose the processing as follows:
- at Level 1, packets X/26 are stored but not applied;
- at Level 1.5, the Level 1.5 pass of area L15 runs;
- at Levels 2.5 and 3.5, the enhancement processor of area L25 runs, with the features marked for Level 3.5 only used at Level 3.5 (REQ-L25-001).

Changing the setting shall not require pages to be acquired again (REQ-ARCH-008).
*Verification:* Test that the same stream gives the expected grid at each level, and that each profile selects its level.

---

## F.15 Command-line program (CLI)

**REQ-CLI-001 — Program and commands.**
A command-line program named `ttx` shall provide these commands:

| Command | Purpose |
|---|---|
| `ttx list` | List the pages and sub-pages found in an input, with counts and error statistics. |
| `ttx show` | Display one page in the terminal, as exact pixels where the terminal supports a graphics protocol and as characters otherwise (decision E.37). |
| `ttx render` | Write one page, or all pages, as image files. |
| `ttx text` | Write one page, or all pages, as UTF-8 text. |
| `ttx convert` | Convert between the input and output formats of area INP, for example from a transport stream to T42, or from TTI to T42. |
| `ttx slice` | Convert raw sampled video to T42 with quality statistics. |
| `ttx services` | List the teletext streams, languages and subtitle pages found in a transport stream. |
| `ttx service-info` | Print the service data from packet 8/30: initial page, network identification, date, time and status message. |
| `ttx check` | Check page files for authoring problems (REQ-CLI-006). |
| `ttx contents` | Write a channel's `contents.txt` from a folder of page files (REQ-CLI-008). |
| `ttx font convert` | Write a font in the 12 × 10 form from a BDF of another size, where the fit is exact (REQ-FNT-010). |
| `ttx discover` | Report the teletext edition a web address points to, and every candidate ignored (REQ-CLI-009). |
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
`ttx check` shall read TTI page files and report problems that would make a page fail on some receivers, or behave unsafely. Each problem is reported as an error or a warning, with its file, page and line. It shall report:

*Errors:*
- **Too many links.** More than four coloured links, a repeated key, or two `LK` lines for the same page number.
- **Bad link targets.** Targets naming services missing from a given descriptor, and `LK` or `LT` lines whose target, page number or position is invalid (Appendix E.1).
- **Bad font lines.** A `TF` line that is malformed or repeated (REQ-FNT-007).
- **Bad labels.** Labels too long for the prompt, or using characters the page's G0 set cannot show.
- **Bad form fields.** Overlapping fields, fields outside the row, or version 2.0 form syntax.
- **Withdrawn commands.** Use of the withdrawn `FH` command.

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
`ttx contents FOLDER` shall write a `contents.txt` for a channel from the author's own local folder of page files (decision E.70 item 9), to standard output. It shall:
- **read only that folder,** not its subfolders, and only these files in it: `NNN.tti` and `NNN.t42` whose page numbers are decimal, and `index.t42`. It shall use no network and write no file. It reads and decodes those files first, then applies the sandbox of REQ-SEC-017, with no file and no network access, before it takes any title from them, as the other commands do;
- **list every page found,** in page number order, as one `p` entry each. A page in both a page file and `index.t42` is listed once;
- **title each page** with its TTI `DE` line when it has one, otherwise with the first row of rows 1 to 23 that is not blank, converted to printable ASCII and trimmed, otherwise with "Page NNN". Titles longer than 32 characters are cut;
- **date each page** with the modification date, in UTC, of the file it was found in;
- **start each magazine with a section heading,** titled with the title of the magazine's page `M00` when that page exists, and "Magazine M" otherwise;
- **stay within REQ-WEB-025.** Beyond 800 pages, it shall stop listing and say so on standard error. A file that cannot be read, or fails the check for its format (REQ-WEB-027), is skipped with a diagnostic on standard error.

The output never includes `x` entries. An author who lists other channels adds those lines by hand after the generated ones. Exit status is 0 on success, the usage status for a wrong argument, and 4 when any file was skipped or the page limit was reached.
*Verification:* Tests with a folder holding TTI and T42 page files, an `index.t42`, a subfolder, a file with another name, an invalid file, and pages in several magazines; the output shall be exactly the expected file, and shall pass REQ-WEB-025.

**REQ-CLI-009 — Testing a web page's pointer.**
`ttx discover ADDRESS` shall make the discovery request of REQ-WEB-028 for an address, and print to standard output:
- the address asked for, every redirect, and the final address;
- the result: the target found, how it was found, and the channel it leads to; or that nothing was found;
- every candidate that was not used, with where it was (the header field, or the byte offset in the body) and why: wrong `rel` or `type`, an `anchor` parameter, past the 8,192-byte limit or after `<body` or `</head>`, inside a comment or a script, or a target refused by REQ-WEB-030 with the rule;
- whether the body held a `<link rel="alternate">` for a teletext type beyond the first 8,192 bytes, which the browser would not see, found by reading on to at most 1 megabyte for this report only.

It shall not open the channel, and shall show nothing of the HTML beyond the candidates' attribute and parameter values. Every value taken from the answer, including addresses, header values and attribute values, shall be printed by the allow-list of REQ-SEC-010: printable ASCII as it is, and every other byte as `\xHH`, cut to 200 characters with a mark. It needs only the network: it applies the sandbox of REQ-SEC-017 with network access and no file access before it connects, and runs the scan in the same confined process. Exit status (REQ-CLI-005) is 0 when a target was found, 1 when none was, 2 for a wrong argument, and 3 when the address could not be reached or its answer could not be read.
*Reason:* A publisher must be able to see why the browser does or does not find their pointer.
*Verification:* Tests against the local test server for each kind of candidate.

---

## F.16 Testing and verification (TST)

**REQ-TST-001 — Test framework.**
Tests shall be written in Rust and run with `cargo test`:
- unit tests inside each crate;
- integration tests in each crate's `tests` directory;
- property-based tests with `proptest` where a property can be stated, such as encode-then-decode round trips;
- snapshot tests with `insta` for text and terminal output.

Tests of the `no_std` crates shall run on the host, and, where REQ-GEN-015 says so, on emulated targets. External tools used as reference oracles are:
- zvbi;
- vhs-teletext;
- vbit2;
- QTeletextMaker.

These tools shall be invoked as separate programs at test time. Their source code and test files shall not be copied into the repository.
*Verification:* Inspection.

**REQ-TST-002 — Exhaustive Hamming 8/4 test.**
A test shall check all 256 byte values against the decoding rules of clause 8.2. It shall confirm that:
- each of the 16 valid codes decodes to its value;
- every single-bit error of every valid code is corrected;
- every double-bit error is reported as uncorrectable.
*Verification:* Test.

**REQ-TST-003 — Exhaustive Hamming 24/18 test.**
A test shall confirm that:
- every one of the 262,144 18-bit values encodes and decodes back to itself;
- all 24 single-bit errors of each are corrected;
- a large random sample of double-bit errors is reported as uncorrectable.
*Verification:* Test.

**REQ-TST-010 — Packet and header vectors.**
Tests shall decode headers, rows, links and service data packets built by the encoder and taken from real captures, and check every field.
*Verification:* Test.

**REQ-TST-011 — Character mapping against a reference.**
A test shall compare the Unicode mapping of every character set and national option with the reference output published with zvbi (its `unicode-out-ref.txt`), running zvbi's own program at test time where possible. Every difference shall be explained in the test source or fixed.
*Verification:* Test.

**REQ-TST-012 — Order of the national option bits.**
Tests shall pin down the ordering of decision E.21 using:
- captured broadcast pages whose language is known, such as a German page showing Ä, Ö and Ü at 5/B to 5/D;
- the result of a TTI file written by vbit2 with a known page status value.
*Verification:* Test.

**REQ-TST-013 — Level 1 attribute tests.**
Tests shall cover every spacing attribute, including set-at and set-after timing, and every example in Annex G:
- the boxing example of G.3.1;
- the separated mosaics example of G.3.2;
- the hold mosaics example of G.3.3.

They shall also cover these combinations:
- hold with size changes;
- hold with mode changes;
- double height with its ignored lower row;
- conceal ended by a colour attribute;
- New Background after a colour change;
- blast-through with national options.
*Verification:* Test.

**REQ-TST-014 — Level 1.5 tests.**
Tests shall cover:
- every triplet mode in both row-setting settings;
- the "@" special case;
- every diacritical mark;
- data values below 2/0;
- missing designation codes;
- the Termination Marker and the triplets after it;
- X/26/15 under each profile;
- each rule of decision E.5.
*Verification:* Test.

**REQ-TST-015 — Level 2.5 and 3.5 enhancement tests.**
Tests shall cover, at both Level 2.5 and Level 3.5:
- every row mode of Table 28 and every column mode of Table 29, with each class of data value, including reserved values;
- every rule of decisions E.43 to E.50, each in a test of its own;
- every requirement of area L25.

Each test page shall be built with the authoring builder of REQ-INP-025, so that the test states the page by function.
*Verification:* Test; the traceability table of REQ-DOC-003 shall link each decision rule to its test.

**REQ-TST-016 — The worked examples of the standard.**
Tests shall reproduce every worked example and truth table of EN 300 706 that concerns Levels 2.5 and 3.5:
- the three contention examples of Annex C.2, which give red, blue and yellow;
- the object example of clause 13.9, for each of the Active, Adaptive and Passive types, with the word colours of its notes 1 to 8. The example's "pink" shall be CLUT 2 entry 0 (red 15, green 0, blue 5), because the standard does not give its index;
- the PTU layout of Figure 27, and the DRCS colour table values of Figure 28;
- all rows of Table C.1;
- the precedence lists for screen and row colour of Annex A.5;
- the three side-panel column layouts of clause 12.5.3;
- the open area of Figure A.1;
- the default values of Table 30.
*Verification:* Test.

**REQ-TST-017 — Object tests.**
Tests shall cover every requirement of area OBJ and every rule of decisions E.51 to E.53, including:
- every source and type of invocation, and every permitted and forbidden pair of invoker and type;
- every case in which a pointer, a definition or a link is unusable;
- each limit of Annex A.8, reached and exceeded;
- the reading of the Magazine Organization Table at both levels, with each Level 3.5 packet missing in turn;
- default objects, with and without local display data, and the order of placement;
- objects placed at every edge of the page and of the side panels.
*Verification:* Test.

**REQ-TST-018 — DRCS tests.**
Tests shall cover every requirement of area DRC and every rule of decision E.54, including:
- every mode, in both cell sizes and every size;
- the order of the mode identification bits;
- the DRCS colour tables from each source;
- each flash mode, invert and conceal on multi-colour characters;
- missing, partial and invalid data;
- the level bits and the scope of the DRCS Mode triplet.
*Verification:* Test.

**REQ-TST-019 — Bounded work and memory of enhancement processing.**
Tests shall build worst-case pages: every object type invoked up to the limits of Annex A.8, nesting to the greatest depth allowed, objects of 507 triplets, all sixteen X/26 packets and full side panels. They shall check that building the grid stays within a stated bound on operation counts (REQ-SEC-012), uses only the memory the caller provided, and never panics. The performance tests (REQ-TST-050) shall report the time to build such a grid at Level 3.5.
*Verification:* Test.

**REQ-TST-020 — Round-trip tests.**
For each page file format, a test shall read a set of pages, encode them to packets, decode the packets, write the pages back in the same format, and confirm that the result matches the original.
*Verification:* Test.

**REQ-TST-021 — Comparison with other decoders.**
A test shall decode the same inputs with this project and with zvbi and vhs-teletext, and compare the results. The inputs are the T42 and transport stream files of the real-broadcast corpus (REQ-TST-022). The comparison covers:
- the set of pages found;
- their text content, row by row.

Differences shall be listed and explained.
*Verification:* Test.

**REQ-TST-022 — Real-broadcast test corpus.**
The tests shall use a corpus of real broadcast teletext. It shall be downloaded at test time and not stored in the repository, and each file shall be pinned by its checksum. The corpus consists of:

- **Off-air digital television recordings** from the FFmpeg sample archive, at `https://samples.ffmpeg.org/ffmpeg-bugs/trac/ticket2086/`:

  | File | Size | Teletext track |
  |---|---|---|
  | `teletextsubtitles.ts` | about 96 megabytes | Teletext track whose descriptor lists Italian and English entries |
  | `France2.m2ts` | about 109 megabytes | French teletext in 192-byte packets |
  | `dreambox-2.ts` | about 98 megabytes | English teletext |
  | `RBT_20100801_1835.ts` | about 47 megabytes | English teletext |

  Each was confirmed on 1 October 2026 to contain a DVB teletext track. The same folder's `Dreambox-1.ts` and `IRIB602MHz.ts` should be checked and added if they carry teletext. Tests may use only the first few megabytes of each file, fetched with ranged requests, to keep test runs short.
- **Recovered service snapshots in T42 form** from the Teletext Archaeologist archive (`https://archive.teletextarchaeologist.org`). The selection shall cover:
  - several broadcasters;
  - several decades;
  - both clean and noisy recoveries.
- **Recent off-air recordings** from the TSDuck stream repository (`https://tsduck.io/streams/`). Each carries full teletext services, not only subtitles, as its published analysis file shows. They are the newest real broadcast packets found (research record: `research/answers-teletext-sources.md`).

  | File | Recorded | Teletext services |
  |---|---|---|
  | `poland-dttv-warsaw/PL_DVB-T_2023-11-10_MUX3_522MHz.ts` | 10 November 2023 | TVP1 and three other TVP services |
  | `netherlands-dttv/digitenne-482-2020-06-21.ts` | 21 June 2020 | NOS Teletekst on NPO 1, 2 and 3, and two more |
  | `italy-sardinia-dttv/mux1rai.ts` | not stated | RAI Televideo |
  | `hotbird-13.0E/hotbird130E-ts6000-2018-05-11.ts` | 11 May 2018 | Mediaset |

  The same rules on ranged requests apply. The repository's licence shall be checked and recorded before the files are used.
- **Restored pages with verified checksums** from the teletext-restoration repository (`https://github.com/teletext-restoration/teletext-restoration`). The restored single-page T42 files whose names carry neither `-maybe` nor `-nocrc` match the page checksum that was broadcast with them. They shall be used for the page checksum test of REQ-ECC-007. The full recordings behind them may be fetched only one at a time, as the maintainers ask.
- **The raw sampled captures published with vhs-teletext**, for the slicer (REQ-TST-061).

- **A local corpus of recovered broadcasts.** The workspace already holds T42 recoveries made by Dan Farrimond from VHS recordings, in `~/devel/jellytext/dan/`:
  - BBC1, 31 October 1991;
  - TV-am, 20 October 1992;
  - The Children's Channel, January 1993 (several captures);
  - Sky One, 16 February 1993;
  - GMTV, 22 February 1993.

  There are full and "squashed" versions, where squashed means the duplicate sub-pages have been merged. The tests shall read local corpus files from a directory given by the environment variable `NEOTELETEXT_CORPUS`, and shall also keep a local cached copy of every downloaded corpus file there. The tests then do not depend on outside websites being available. Corpus files shall never be committed, because the page content belongs to the broadcasters.

Expected results for the corpus are the outputs of zvbi and vhs-teletext (REQ-TST-021), plus page counts and spot checks recorded by hand. If a source disappears and no cached copy exists, the test shall report the missing file clearly and skip, not fail. A test run that skipped corpus files shall say so in its summary, so that lost coverage is visible.
*Verification:* Test.

**REQ-TST-023 — zvbi as the oracle at Levels 2.5 and 3.5.**
A test shall decode the same streams with this project and with zvbi 0.2.44 at its Level 2.5 and Level 3.5 settings, run as a separate program (REQ-TST-001 and REQ-SEC-022), and compare, cell by cell:
- the character, as Unicode, with zvbi's private value 0xEF00 plus the code mapped to G3 positions;
- the foreground and background colours, as Colour Map indices and not as colours, because zvbi's values for CLUT 1 entries 5 and 6 differ from Table 30 (decision E.43);
- the size;
- the underline, flash, conceal, bold, italic and proportional flags;
- the screen colour index.

The streams shall be the authored streams of REQ-TST-024 and the real data of REQ-TST-025, limited to streams whose triplets keep the order of Annex A.4 (decision E.46). The test shall also check the reading of the magazine of a Magazine Organization Table link (decision E.52), and record the result in that decision. DRCS cells shall be excluded until zvbi can be made to load a DRCS page; why it would not load the synthetic page of `research/notes-level25-drcs.md` section 13.2 shall be investigated, and the finding recorded in Appendix D. Every difference shall be explained in the test source or fixed.
*Verification:* Test.

**REQ-TST-024 — Authored Level 2.5 and 3.5 test streams with golden rasters.**
A set of test streams shall be authored with the builder of REQ-INP-025. Between them they shall use every triplet mode, every object source and type, Public and Global Object Pages, the Magazine Organization Table, DRCS pages of every mode, and every variant of X/28 and M/29. Each stream's pages shall be kept as TTI source in the repository, with golden checksums of their rendered rasters in both cell sizes, under the format version rule of REQ-TST-030.
*Verification:* Test.

**REQ-TST-025 — Real Level 2.5 and 3.5 data.**
The local corpus (REQ-TST-022) shall gain real pages that carry Level 2.5 and 3.5 data:
- captures from the Teefax and NMS Ceefax feeds of REQ-INP-022, recorded through the feed reader into `$NEOTELETEXT_CORPUS`, of pages that carry packets X/26 or X/28, with any object, DRCS and Magazine Organization Table pages they use;
- the Polish TVP pages with packets X/26 in the recording already listed in REQ-TST-022.

These files shall never be committed. The tests shall decode them at Levels 2.5 and 3.5, compare them with zvbi (REQ-TST-023), and skip clearly when a file is absent.
*Verification:* Test.

**REQ-TST-030 — Reference images.**
Golden raster images shall be kept for:
- the Level 1.5 example pages published with QTeletextMaker (for example `Level1p5-Defcon.tti`, `Level1p5-KLF.tti` and `Level1p5-PleaseHammer.tti`), downloaded at test time;
- the edit.tf engineering test card;
- test pages written for this project that exercise every attribute and mosaic form.

Golden images and golden cell-raster outputs shall carry a format version number. A change to rendering that alters pixels shall raise the version and regenerate the golden files in the same change, with the reason recorded, so that stale golden files are never mistaken for regressions. This is a lesson of Doom's recorded demos, which silently went out of step between program versions.

The SAA5050-style rendering, including rounding and separated mosaic geometry (decisions E.13 and E.14), shall be compared by eye against reference images of real SAA5050 output from a documented emulator or photographs. The findings shall be recorded.
*Verification:* Test and inspection.

**REQ-TST-040 — Fuzz testing.**
Every reader, the packet decoder, the slicer, and the HTTP, WebSocket, server-sent event, descriptor, form, address (REQ-WEB-016) and pointer (REQ-WEB-029) parsers of the web module shall have a fuzzing target for `cargo fuzz` (libFuzzer). The targets shall run in continuous integration for a fixed time on each change, and for longer periods on a schedule. Every input that has ever found a fault shall be kept in the repository as a regression test.
*Verification:* Inspection; continuous integration logs.

**REQ-TST-041 — Coverage.**
The core library tests shall reach at least 90 percent line coverage.
*Verification:* Coverage report.

**REQ-TST-042 — Fuzz targets for the data of Levels 2.5 and 3.5.**
Each new parser and processor of Levels 2.5 and 3.5 shall have a fuzzing target under REQ-TST-040:
- packets X/27/4 and X/27/5, X/28/0 to X/28/4, and M/29/0, M/29/1 and M/29/4;
- the packets of object pages, including pointer tables;
- the packets of the Magazine Organization Table;
- DRCS pages;
- the enhancement lines of TTI files (REQ-INP-023);
- the whole grid build at Level 3.5, from a displayed page and a set of linked object pages, DRCS pages and a Magazine Organization Table, all built from the fuzzer's input.

The last target shall also check that the work stays within the bound of REQ-TST-019, and that row 0, columns 0 to 7, are never written (REQ-L25-050).
*Verification:* Inspection; continuous integration logs.

**REQ-TST-050 — Performance tests.**
Tests shall measure the throughput requirements of REQ-GEN-013 and report the results.

They shall also report hardware-independent drawing costs on the build machine, with and without the glyph line cache of REQ-RND-026:
- cell-line draws per second;
- the average number of machine instructions per cell-line draw, where the platform's performance counters allow it.

These figures let each port estimate its own budget. Redrawing every row of every field, for example, takes 500,000 cell-line draws per second.
*Verification:* Test.

**REQ-TST-060 — Synthetic signal tests.**
A test signal generator shall produce sampled teletext lines to EN 300 706 at each sample rate of REQ-SLC-002, with controllable:
- level;
- black-level offset;
- timing;
- bit-rate error;
- high-frequency loss;
- echo;
- noise.

It shall be used to verify REQ-SLC-002 to REQ-SLC-013.
*Verification:* Test.

**REQ-TST-061 — Real capture tests.**
The slicer shall be tested on the sample captures published with vhs-teletext, downloaded at test time, and compared with that tool as REQ-SLC-014 describes. The packets it recovers shall then be decoded in full, and the resulting pages compared with those vhs-teletext produces from the same capture.
*Verification:* Test.

**REQ-TST-063 — Live feed soak test.**
A scheduled test, run outside the normal test suite, shall connect to each live feed of REQ-INP-022 for at least one hour and walk the whole page list. It shall check:
- that no crash, memory growth or unbounded delay occurs;
- that every page in the feed's page list is eventually received;
- that the cross-check of REQ-INP-022 between the decoder's header fields and the server's decoded fields finds no disagreement.

A session shall also be recorded to a file so that it can be replayed in the normal test suite.
*Verification:* Test on a schedule; recorded results.

**REQ-TST-070 — Withdrawn (hardware demonstration).**
*Withdrawn on 1 October 2026.* It depended on specific hardware (a Raspberry Pi inserter and a television), which is outside the library (REQ-ARCH-010). The number is kept and will not be reused.

**REQ-TST-080 — Continuous integration.**
Continuous integration shall run on every change:
- the target matrix of REQ-GEN-015;
- the minimum-version and current-stable builds (REQ-GEN-001);
- `rustfmt` and `clippy` (REQ-GEN-006);
- the feature combinations (REQ-GEN-010);
- the Miri runs (REQ-GEN-006);
- `cargo-deny` (REQ-GEN-017);
- the size budget (REQ-GEN-009);
- the fuzzing runs (REQ-TST-040).
*Verification:* Inspection.

**REQ-TST-081 — End-to-end tests of the browser against local channels.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
The browser shall be tested end to end, with its real helper processes, against local test channels served on 127.0.0.1. The server is test code (`tests/web-server/sites.py`), and generates every file on request, which also shows that a dynamic server and a static one look the same to the browser (decision E.70 item 10). It shall serve:
- **a news channel** at the root of its own port, with `channel.json`, `contents.txt`, TTI pages, aliases, `LN`, `LK` and `LT` links to other channels, and a form;
- **a weather channel** without `channel.json`, with `index.t42` and T42 page files;
- **two channels in two folders of one port,** `/one/` and `/two/`, each with its own pages and cookies, and links and a redirect from one to the other;
- **a channel served with the generic media types** `text/plain` and `application/octet-stream`, and one that answers a missing file with an HTML page and status 200;
- **a redirecting channel,** whose `channel.json` redirects to another channel;
- **a hostile channel,** whose pages try to forge row 0 and the tuning screen, and one of whose responses never ends;
- **addresses with path tricks:** `%2F`, `..`, doubled slashes, and `/ab/` against `/a/`;
- **web pages for discovery** (REQ-WEB-028 to REQ-WEB-030): a page with a `Link` header; a page with a `<link>` early in its head; a page whose only `<link>` is past 8,192 bytes, and one inside a comment and a script; a root folder that answers every file with an HTML page and status 200; a redirect to another port before the pointer; a program that answers `303 See Other` to a page file when `Accept` puts teletext first, and one that answers teletext at a web address; pointers that are relative, relative after `<base>`, `&amp;`-encoded, to `http` from `https`, to 127.0.0.1, and to an address that is not a channel; and a page whose pointer leads to a missing channel.

The tests shall drive the browser's tabs through the same code the window uses, without a display. They shall cover typed addresses in every form of REQ-BRW-002, history, links, aliases, page catching, the contents view, redirects, forms, cookies kept apart between two channels on one origin, files, the hostile channel, the path tricks, the media types, discovery and its tuning screens, and the helpers' confinement (REQ-SEC-026). Window tests shall render the whole window without a display and check what it shows.
*Amended 12 October 2026* (decision E.70): channels in folders.
*Verification:* The tests run in the gate.

---

## F.17 Documentation (DOC)

**REQ-DOC-001 — Programming interface reference.**
Every public function, type and constant shall be documented in its header. The documentation shall cover:
- ownership of memory;
- valid ranges;
- error returns;
- thread-safety.
*Verification:* Inspection.

**REQ-DOC-002 — User guide.**
A user guide shall describe the command-line program, every input and output format, the profiles and every configurable setting of Part E.
*Verification:* Inspection.

**REQ-DOC-003 — Traceability.**
A traceability table shall link every requirement in this document to the tests that verify it. Requirements verified by inspection or demonstration shall be linked to the record of that check.
*Verification:* Inspection.

**REQ-DOC-004 — Keeping the decision register current.**
Any change of behaviour where the standard is silent or ambiguous shall be recorded as a new or amended entry in Part E before it is implemented.
*Verification:* Inspection.

**REQ-DOC-005 — Sources record.**
The repository shall contain a record of the standards clauses and tables used for each module, as the legal basis section requires.
*Verification:* Inspection.

---

## F.18 State machines (FSM)

EN 300 706 contains two different kinds of ordering rule.

**Rules for the broadcaster.** Most of the ordering rules tell the broadcaster how to transmit. Examples are:
- the packet order within a page (Annex B.2);
- the order of triplets (Annex A.4);
- the 20 millisecond gap after a header (Annex B.1);
- the limit of 100 milliseconds between packets of a page (Annex B.9).

Real transmissions and recordings do not always obey them. A receiver that depended on them would reject good data.

**Rules for the receiver.** A smaller set of rules binds the receiver:
- what starts and ends a page (clause 7.2.1);
- the erase bit;
- the page-completion timeout (decision E.19);
- the left-to-right processing of attributes in a row, with set-at and set-after timing (Table 26);
- the walk through Level 1.5 triplets (clause 12.3);
- at Levels 2.5 and 3.5, the walk through local enhancement data and objects (clauses 12.3 and 13), and the resolution of contending attributes (Annex C.2).

This area turns the receiver rules into explicit state machines: the six of REQ-FSM-002 to REQ-FSM-007, and the two of Levels 2.5 and 3.5 in REQ-FSM-011 and REQ-FSM-012. It requires every one of them to tolerate any breach of the broadcaster rules.

The tables below are the normative form of behaviour that areas SLC, ACQ, L1, L15, L25, OBJ and NAV describe in prose. If a table and a prose requirement disagree, that is a defect, to be corrected in both.

**REQ-FSM-001 — How the machines are implemented.**
Each state machine shall be implemented as an explicit enumeration of states and a single transition function. The transition function takes the current state and one event, and returns the next state together with the actions to perform. In Rust this is an `enum` of states and a `match` over the state and the event, with no wildcard arms, so that the compiler checks that every combination is handled. No state that affects a transition may be kept anywhere except in the machine's own state record. Every transition in the tables of this area shall exist in the code, and there shall be no transitions in the code that are not in the tables. Each machine shall be usable and testable on its own, without the rest of the decoder.
*Verification:* Inspection; the transition tests of REQ-FSM-009.

**REQ-FSM-002 — The line slicer machine.**
One run of this machine processes one sampled line (area SLC). It shall follow this table:

| State | Event | Action | Next state |
|---|---|---|---|
| Start | Line received | Estimate black level (REQ-SLC-003) | Searching for run-in |
| Searching for run-in | Run-in pattern found within the search window (REQ-SLC-004) | Record its position | Measuring |
| Searching for run-in | Window exhausted | Count "no signal" | Rejected |
| Measuring | Levels, threshold and bit phase estimated | None | Gate check |
| Gate check | Amplitude and match quality at or above the minimums (REQ-SLC-005) | Hold the threshold for the rest of the line | Looking for framing code |
| Gate check | Below either minimum | Count "below gate" | Rejected |
| Looking for framing code | Exact framing code found within plus or minus 2 bit periods | None | Collecting bytes |
| Looking for framing code | Framing code with one wrong bit found, and single-bit tolerance enabled (decision E.24) | Mark "framing corrected" | Collecting bytes |
| Looking for framing code | Not found | Count "no framing code" | Rejected |
| Collecting bytes | 336 bits read at bit centres (REQ-SLC-007 and REQ-SLC-009) | Assemble 42 bytes | Checking address |
| Checking address | Both address bytes decode without uncorrectable error | Output packet with quality data (REQ-SLC-012) | Accepted |
| Checking address | Either address byte uncorrectable | Count "bad address". This check is what makes accepting a corrected framing code safe (decision E.24). | Rejected |

*Verification:* The transition tests of REQ-FSM-009, using synthetic lines from REQ-TST-060.

**REQ-FSM-003 — The magazine acquisition machine.**
There shall be one instance of this machine for each of the eight magazines (area ACQ). Header events shall be delivered as follows:
- **Serial mode:** every header is delivered to all eight instances, because in serial mode any header ends the page in progress (clause 7.2.1 and decision E.6).
- **Parallel mode:** a header is delivered only to the instance for its own magazine.

The mode is taken from bit C11 of each header as it arrives. Packets 29, 30 and 31 never reach this machine; they go straight to the magazine and service handlers (REQ-PKT-008).

The states are:
- **Idle:** no page is being received in this magazine.
- **Receiving P:** page P is being received and stored.
- **Absorbing:** a page that is not to be stored is being transmitted, such as a hexadecimal page or, in request mode, a page nobody asked for. Its rows must not be attached to anything else.

| State | Event | Action | Next state |
|---|---|---|---|
| Idle | Valid header for a storable page P in this magazine | Start P. If C4 is set, erase P's stored rows 1 to 28 (REQ-ACQ-004). | Receiving P |
| Idle | Valid header for a page not to be stored, in this magazine | None | Absorbing |
| Idle | Row packet 1 to 28 of this magazine | Discard; count "row without header" | Idle |
| Idle | Any other header | None | Idle |
| Receiving P | Row packet 1 to 28 of this magazine | Store in P (REQ-ACQ-007 and REQ-ACQ-008); mark the row changed | Receiving P |
| Receiving P | Row packet 1 to 28 of another magazine, in serial mode | Discard; count (Annex B.3) | Receiving P |
| Receiving P | Valid header for a storable page Q in this magazine | End P and raise "page completed"; start Q, erasing it if C4 is set | Receiving Q |
| Receiving P | Valid header for a page not to be stored, in this magazine | End P and raise "page completed" | Absorbing |
| Receiving P | Header of another magazine, in serial mode | End P and raise "page completed" | Idle |
| Receiving P | Time-filling header (page FF) in this magazine, or, in serial mode, in any magazine | End P and raise "page completed" | Idle |
| Receiving P | Header of this magazine with a valid address but uncorrectable page or control bytes (decision E.7) | End P and raise "page completed" | Idle |
| Receiving P | No packet for P for the completion timeout, 100 milliseconds by default (decision E.19) | Raise "page completed" | Idle |
| Absorbing | Row packet 1 to 28 of this magazine | Discard silently; this is not an error | Absorbing |
| Absorbing | Any header that would end a page, following the same rules as in Receiving P | As for Receiving P, but with no "page completed" event | The same next state as for Receiving P |
| Absorbing | Completion timeout | None | Idle |

Row packets are stored whatever order they arrive in. The machine shall not require packets 26, 27 and 28 to come before the rows, nor designation codes to ascend, nor rows to ascend (REQ-FSM-008).
*Verification:* The transition tests of REQ-FSM-009.

**REQ-FSM-004 — The page lifecycle machine.**
There shall be one instance of this machine for each stored page and sub-page (areas ACQ and NAV). It is driven by the events of the acquisition machine and by time. It shall follow this table:

| State | Event | Action | Next state |
|---|---|---|---|
| Empty | Reception started without C4 | Raise "page started" | Partial |
| Empty | Reception started with C4 | Raise "page started" | Partial |
| Partial | Row stored | Raise "page updated" for that row | Partial |
| Partial | Reception ended | Raise "page completed" | Complete |
| Complete | Reception started without C4 | Merge the new rows under decision E.8; raise "page started" | Partial |
| Complete | Reception started with C4 | Erase rows 1 to 28; raise "page erased" | Partial |
| Complete | Header with C8 received, or content changed | Raise "update indicator" (REQ-ACQ-010) | Unchanged |
| Complete | Not received for the configured "recently received" period | Raise "page stale" | Stale |
| Stale | Reception started, with or without C4 | As for Complete | Partial |
| Any | Evicted from the cache (REQ-ACQ-005) | Release storage | Empty |

*Verification:* The transition tests of REQ-FSM-009.

**REQ-FSM-005 — The viewer machine.**
There shall be one instance of this machine for each viewer (area NAV). Reveal, mix and zoom are part of the viewer state that the renderer reads (REQ-L1-020 and REQ-NAV-007). They do not change the machine's state. The machine shall follow this table:

| State | Event | Action | Next state |
|---|---|---|---|
| Starting | Decoder started | Request the initial page (REQ-SVC-004) | Searching |
| Any state except Holding | First digit 1 to 8 | Show it in the status area | Entering 1 digit |
| Entering 1 digit | Digit 0 to 9 | Show it | Entering 2 digits |
| Entering 2 digits | Digit 0 to 9 | Request the entered page | Searching |
| Entering 1 or 2 digits | Digit that cannot make a valid page | Reject; keep the digits entered so far | Unchanged |
| Searching | Requested page present in the store | Show it; set the row 0 marker to white (decision E.10) | Displaying |
| Searching | Header received | Update the rolling header and clock (decision E.11) | Searching |
| Displaying | Page updated, or flash or reveal changed | Mark the affected rows changed (REQ-RND-019) | Displaying |
| Displaying | Page stale | Show "not recently received" in the status area | Displaying |
| Displaying | Hold key | Stop applying updates | Holding |
| Holding | Hold key | Apply updates received meanwhile | Displaying |
| Displaying or Searching | Coloured key with a link in the same service, Index, Home, next or previous page | Request the target page (REQ-NAV-002, REQ-NAV-003 and REQ-NAV-008) | Searching |
| Displaying | Coloured key whose link leads to an unconfirmed other service | Show the confirmation in the status area and row 24 (REQ-NAV-012) | Confirming link |
| Confirming link | The same key again | Mark the origin confirmed; request the target | Searching |
| Confirming link | Any other key | Restore row 24 | Displaying |
| Displaying | Next or previous sub-page | Show the neighbouring stored sub-page (REQ-NAV-005) | Displaying |
| Displaying or Searching | Back or Forward, with an entry in that direction | Move in the history and request that page (REQ-NAV-010) | Searching |
| Displaying or Searching | Back or Forward, with no entry in that direction | None | Unchanged |

When a request is made and the page is already in the store, the machine passes through Searching straight to Displaying, and the found page is shown at once.
*Verification:* The transition tests of REQ-FSM-009.

**REQ-FSM-006 — The row attribute machine: the per-cell step.**
The Level 1 decoding of each row (area L1) shall be implemented as a state record, updated one cell at a time from column 0 to column 39. The record holds:
- foreground colour;
- alphanumeric or mosaic mode;
- flash;
- conceal;
- box;
- size;
- contiguous or separated;
- hold;
- the held mosaic character and its form;
- which G0 set is in use.

It starts from the defaults of REQ-L1-002. For each cell, the machine shall perform these steps in this order:
1. **Set-at attributes.** If the byte is a set-at attribute, apply it. The set-at attributes are:
   - Steady;
   - Normal Size, which resets the held mosaic if the size changes;
   - Conceal;
   - Contiguous;
   - Separated;
   - Black Background;
   - New Background;
   - Hold Mosaics.
2. **Choose what the cell shows.**
   - An alphanumeric character: from the current G0 set, with national option substitution.
   - A mosaic character: from the G1 set, or the G0 character for blast-through codes.
   - An attribute: a space, or the held mosaic when hold is on and the row is in mosaic mode.
3. **Record the cell** with the current colours, flash, conceal, box and size (REQ-L1-001).
4. **Update the held mosaic.** If the byte was a mosaic character with bit 6 set, it becomes the held mosaic, stored with its contiguous or separated form.
5. **Set-after attributes.** If the byte is a set-after attribute, apply it. The set-after attributes are:
   - the alphanumeric and mosaic colours. A change of mode resets the held mosaic, and any colour ends conceal.
   - Flash;
   - End Box;
   - Start Box. Under decision E.17 it takes effect only when the next cell also holds Start Box, so this step needs to look one byte ahead;
   - Double Height, which resets the held mosaic if the size changes and marks the row as containing double height;
   - ESC;
   - Release Mosaics.

The handling of the row after a double-height row (REQ-L1-008) happens before this machine runs: that row is not processed from its own bytes at all.
*Reason:* The attributes have too many combinations to list as named states. The order of the five steps is exactly what Table 26 and decision E.16 define, and it is where attribute bugs are usually found.
*Verification:* The transition tests of REQ-FSM-009, and the attribute tests of REQ-TST-013.

**REQ-FSM-007 — The Level 1.5 triplet walk.**
One run of this machine applies a page's enhancement packets to its cell grid (area L15). It shall follow this table:

| State | Event | Action | Next state |
|---|---|---|---|
| Start | Walk begins | Active position is row 0, column 0 (REQ-L15-003) | Walking |
| Walking | Next stored X/26 packet, in designation-code order. Missing codes are skipped. X/26/15 is skipped when the profile says so (decision E.2). | Take its triplets in order | Walking |
| Walking | Triplet that failed its Hamming 24/18 check | Ignore (decision E.9) | Walking |
| Walking | Row triplet with address 63 and mode 11111 (Termination Marker) | Stop | Terminated |
| Walking | Row triplet with address 63 and mode 00111 | Active position becomes row 0, column 0 | Walking |
| Walking | Any other row triplet that sets the row under decision E.3 | Row becomes 24 for address 40, or the address minus 40 otherwise; column becomes 0 | Walking |
| Walking | Other row triplet | None | Walking |
| Walking | Column triplet with mode 00100, 00101, 01010 or 00110 | None; the active position is unchanged (REQ-L15-005) | Walking |
| Walking | Column triplet with a character mode (00010, 01111, or 10000 to 11111) | Column becomes the address. If the data is 0x20 or above, write the character under decision E.5 (REQ-L15-006 and REQ-L15-008). | Walking |
| Walking | Any other column triplet | Column becomes the address | Walking |
| Walking | No more packets | None | Terminated |

*Verification:* The transition tests of REQ-FSM-009, and the Level 1.5 tests of REQ-TST-014.

**REQ-FSM-008 — Tolerance of the broadcaster's ordering rules.**
None of the machines shall depend on the ordering rules that EN 300 706 places on the broadcaster. In particular:
- packets 26, 27 and 28 may arrive after, or among, the rows of their page;
- designation codes may arrive in any order, or with gaps;
- rows may arrive in any order;
- the gap after a header may be shorter than 20 milliseconds;
- triplets may address positions out of screen order (REQ-L15-009 and REQ-L25-005);
- magazines may be interleaved in any pattern that parallel mode allows;
- the Magazine Organization Table, object pages and DRCS pages may arrive before or after the pages that use them, and in any order among themselves (Annex B.7 asks for an order the receiver does not rely on).

A page received in any such order shall give the same stored page and the same cell grid as the same packets in standard order, once all of them have arrived (REQ-ACQ-022). The only exceptions are cases where the standard itself makes order significant: which header starts and ends a page, which of two overwrites of the same cell comes later (decision E.5 (h)), and which of two contending attributes comes later in transmission (Annex C.2 rule 3).
*Verification:* The reordering tests of REQ-FSM-009.

**REQ-FSM-009 — Tests for every transition.**
The test suite shall contain at least one test for every row of every table in this area, and for each of the five steps of REQ-FSM-006. Each test shall check both the next state and the actions performed.

In addition, reordering tests shall take pages from the encoder and from the real-broadcast corpus (REQ-TST-022) and feed them to the decoder in each non-standard order listed in REQ-FSM-008. The resulting pages and cell grids shall be checked to be identical to those from standard order.
*Verification:* Test; the traceability table of REQ-DOC-003 shall link each table row to its test.

**REQ-FSM-010 — Transition trace.**
Each machine shall be able to report every transition it makes, as a trace entry containing:
- the machine and instance, for example "magazine 3";
- the time;
- the state before;
- the event;
- the actions;
- the state after.

Tracing shall be switched on and off in the configuration. When it is off, it shall cost nothing beyond a single test per transition. The command-line program's verbosity option (REQ-CLI-002) shall be able to print the trace.
*Reason:* A trace of transitions is the most effective way to find out why a real recording decodes wrongly.
*Verification:* Test that a known input produces the expected trace.

**REQ-FSM-011 — The Level 2.5 and 3.5 enhancement walk.**
One run of this machine walks a page's local enhancement data, the objects it invokes and the default objects, and produces the character and attribute events that REQ-FSM-012 resolves into the grid (areas L25 and OBJ). The Level 1.5 walk of REQ-FSM-007 is unchanged and is used at Level 1.5 only.

The machine's state record holds the current source, which is the local enhancement data, an object or a default object, and a stack of walk records, one for each invoking source. A walk record holds:
- the active position, relative to the source's origin;
- the origin;
- any pending Origin Modifier;
- the chosen DRCS sub-tables;
- the source's attribute plane and display priority.

The stack is at most three records deep, because of the nesting rules of REQ-OBJ-008, so no recursion is needed (REQ-GEN-016). The machine shall follow this table:

| State | Event | Action | Next state |
|---|---|---|---|
| Start | Walk begins and the page has a packet X/26 with a display triplet | The source is the local enhancement data. Active position row 0, column 0; DRCS sub-tables 0; no Origin Modifier (REQ-L25-003 and REQ-DRC-005). | Walking |
| Start | Walk begins and the page has no packet X/26 with a display triplet | None | Placing default objects |
| Walking | Next triplet of the source: for the local enhancement data, from the next stored X/26 in designation-code order, skipping missing codes; for an object, the next triplet of its body (REQ-OBJ-006) | Take it | Walking |
| Walking | Triplet that failed its Hamming 24/18 check | Skip it (decision E.46) | Walking |
| Walking | Termination Marker (address 63, mode 11111), or the end of the source's data | End the source | Returning |
| Walking | Object Definition triplet | End the source (decision E.46) | Returning |
| Walking | Full Row Colour, Set Active Position, or Address Display Row 0 with address 63 | Move the active position (REQ-L25-003). For Full Row Colour and Address Display Row 0, emit a row colour event (REQ-L25-033), unless the source is an Adaptive or Passive object or lies in a side panel (REQ-OBJ-013) | Walking |
| Walking | Full Screen Colour | Emit a screen colour event (REQ-L25-032), unless REQ-OBJ-013 forbids it | Walking |
| Walking | DRCS Mode | Choose the sub-table if the level bits allow (REQ-DRC-005) | Walking |
| Walking | Origin Modifier | Record the offsets (REQ-OBJ-002) | Modifier pending |
| Modifier pending | Object Invocation | Handle it as an invocation from Walking, with the recorded offsets | As for an invocation from Walking |
| Modifier pending | Any other triplet | Discard the offsets (decision E.46), then handle the triplet as in Walking | As for that triplet in Walking |
| Walking | Object Invocation that REQ-OBJ-008 permits, within the limits of REQ-OBJ-020, whose definition is found and accepted (REQ-OBJ-003 to REQ-OBJ-006) | Push the walk record. The new source is the object: origin as REQ-OBJ-002 gives; active position row 0, column 0; DRCS sub-tables 0; the attribute plane of its type (REQ-OBJ-009 to REQ-OBJ-011) | Walking |
| Walking | Any other Object Invocation | Skip it and count it | Walking |
| Walking | Any other row triplet: a reserved mode, or a Programme Delivery Control mode | None (REQ-L25-052) | Walking |
| Walking | Column triplet of a reserved mode, or of mode 00110 | None; the active position is unchanged (REQ-L25-004) | Walking |
| Walking | Column triplet of a character mode (REQ-L25-010) | Column becomes the address. If the data is 2/0 or above, emit a character event at the origin plus the active position, with the source's priority, unless REQ-OBJ-014 or REQ-L25-050 discards it | Walking |
| Walking | Column triplet of an attribute mode: 00000, 00011, 00111, 01000, 01100 or 01110 | Column becomes the address. Emit an attribute event at the origin plus the active position, unless REQ-OBJ-014 or REQ-L25-050 discards it, or the level ignores the mode (REQ-L25-001) | Walking |
| Returning | The ended source is an object | Pop the walk record: restore the invoker's active position, origin and DRCS sub-tables; cancel the Origin Modifier (REQ-OBJ-002) | Walking |
| Returning | The ended source is the local enhancement data | None | Finished |
| Placing default objects | Next default object of REQ-OBJ-040, in order, that is found and accepted | The source is the default object, with origin and active position row 0, column 0 | Walking |
| Returning | The ended source is a default object | Active position row 0, column 0 | Placing default objects |
| Placing default objects | No more default objects | None | Finished |

*Verification:* The transition tests of REQ-FSM-009, and the tests of REQ-TST-015 and REQ-TST-017.

**REQ-FSM-012 — Row resolution at Levels 2.5 and 3.5: the per-cell step.**
At Levels 2.5 and 3.5, each row shall be resolved from its Level 1 bytes and from the events of REQ-FSM-011 that fall on it. This extends the row attribute machine of REQ-FSM-006 (decision E.47).

The state record of REQ-FSM-006 is kept for the underlying plane, which is the Level 1 page modified by the local enhancement data and Active objects. It gains these fields:
- the colours as Colour Map indices;
- underline or separated mosaics, set by a non-spacing attribute;
- invert;
- the flash mode, rate and phase;
- conceal, with whether it was set by a non-spacing attribute;
- window;
- the font styles, including those extended from rows above by REQ-L25-028;
- the horizontal part of the size;
- the current G0 and G2 designation of the enhancement data.

Each Adaptive object on the row has an overlay record, holding the attribute types it has changed. Each Passive object has its own record, starting from the defaults of REQ-OBJ-011.

The row starts from the start-of-row defaults, re-mapped as REQ-L25-022 describes. For each cell, from column 0 to column 39, and then separately for the cells of the right panel and of the left panel, which start from the defaults of REQ-L25-041 and have no Level 1 bytes, the machine shall perform these steps in this order:
1. **Set-after attributes from the previous cell.** Apply the set-after attributes recorded by step 7 of the previous cell, as REQ-FSM-006 step 5 does.
2. **Set-at Level 1 attributes.** If the Level 1 byte is a set-at attribute, apply it, as REQ-FSM-006 step 1 does.
3. **Non-spacing attributes of the underlying plane.** Apply each attribute event of the local enhancement data and of Active objects placed at this cell, in the order of the walk. These win against step 2 (REQ-L25-020).
4. **Adaptive overlays.** For each Adaptive object whose range on this row, from its first to its rightmost addressed cell, includes this cell: start its overlay, at its first cell, from the underlying record; apply its attribute events at this cell; and, for each attribute type it has changed, use its value in place of the underlying one (REQ-OBJ-010). Outside its range the overlay has no effect.
5. **Passive records.** Apply the attribute events of each Passive object placed at this cell to that object's own record (REQ-OBJ-011).
6. **Choose what the cell shows.** Take the character of the highest display priority at this cell (REQ-OBJ-012): Passive, then Adaptive, then local enhancement data, then Active, then the Level 1 character chosen as REQ-FSM-006 step 2 does. Take the attributes from the Passive object's record when a Passive object supplied the character. Otherwise take them from the Adaptive overlay when the cell is in an Adaptive object's range, whether the character is the object's or the Level 1 page's; where two ranges overlap, the later invoked object's overlay applies. Otherwise take them from the underlying record. Apply the size rules of REQ-L25-025.
7. **Record the cell and finish.** Record the cell (REQ-L25-006). Update the held mosaic from Level 1 characters only, as REQ-FSM-006 step 4 does (decision E.50). Record the set-after attributes of the Level 1 byte for step 1 of the next cell.

The row after a row with spacing double height or double size is handled as REQ-L25-026 describes before this machine runs: its Level 1 bytes are not used, and only enhancement events are resolved on it.
*Reason:* Annex C.2 orders contending attributes by where they take effect, then where they are placed, then transmission order. Steps 1 to 3 apply exactly that order to the underlying plane. Steps 4 to 6 add the planes of clause 13.2, which reproduce the worked example of clause 13.9.
*Verification:* The transition tests of REQ-FSM-009, and the worked examples of REQ-TST-016.

---

## F.19 Web-native page sources (WEB)

This area makes neoteletext a web-native receiver (decision E.30). Pages and packets can be fetched from ordinary web servers, pushed by them, and interacted with through forms. Everything here lives in an optional web module. The core library stays free of networking (REQ-GEN-002), and the network transport itself is supplied by the port (REQ-WEB-001).

**REQ-WEB-001 — The network transport is supplied by the port.**
The web module shall contain the protocol logic for:
- HTTP/1.1;
- WebSockets;
- server-sent events;
- cookies;
- forms.

It shall not open sockets or perform encryption itself. It shall be written "sans-IO":
- it is a set of state machines;
- bytes received, the current time and user actions are fed in;
- bytes to send, timers and events come out.

The same code then works with a blocking port, an asynchronous port, a desktop program using the standard library, or a bare-metal network stack. The port shall supply, through an optional part of the platform interface (REQ-ARCH-009), a byte-stream transport with these operations:
- connect to a host and port, with or without Transport Layer Security (TLS);
- send;
- receive, without blocking, reporting data as it arrives;
- close.

Certificate checking is the port's responsibility and shall be on by default. Every operation shall be non-blocking and report its results as events (decision E.30 item 3).
*Verification:* Test with an in-memory transport that replays recorded exchanges; inspection that the module has no socket or cryptography code.

**REQ-WEB-002 — The service descriptor.**
A web teletext service shall be identified by the address of a small descriptor document. The descriptor is a JSON object with these members:
- the service name;
- the initial page;
- a page address template;
- optionally, a push address and the push method (WebSocket or server-sent events);
- optionally, a list of other origins that forms may submit to;
- optionally, the default character set region (decision E.4);
- optionally, a list of other services that links may lead to (Appendix E). Each has a short name, a display name and the address of its own descriptor;
- optionally, `aliases`: an object whose members map a page number to a target, either `name:mpp` or an address (decisions E.63 and E.64). An alias is one page number, never a range. A relative address shall be refused, an address in a descriptor fetched over `https` shall be `https`, and there shall be at most 16 aliases;
- optionally, `refresh`: a whole number of seconds from 5 to 86 400, the interval at which the page on screen is asked for again when the service has no push (decision E.63).

A channel's descriptor is the file `channel.json` in the channel's folder, with the differences REQ-WEB-024 lists (decision E.70). A descriptor may also be given by any other address, as a service with a page address template (REQ-WEB-018).

The page address template uses the placeholders `{m}` (magazine), `{pp}` (page number as two hexadecimal digits), `{mpp}` and `{sub}` (sub-code as four hexadecimal digits), for example `https://example.org/ttx/pages/{mpp}`. The reader shall reject a descriptor with missing required members or unknown placeholders, with a clear diagnostic.
*Reason:* A page number is teletext's address. The template maps it to a web address without making the page files themselves aware of the web.
*Amended 12 October 2026* (decisions E.63 and E.70): `aliases`, `refresh`, and `channel.json` as a channel's descriptor.
*Verification:* Test with valid and malformed descriptors, including aliases and refresh intervals out of range.

**REQ-WEB-003 — Fetching pages.**
The web module shall fetch a page by expanding the template for the requested page, and optionally sub-page, and making an HTTP GET request. It shall accept these response types:
- **A TTI page file** (REQ-INP-013), media type `text/x-teletext-tti`. A file may contain several sub-pages.
- **A T42 packet stream** (REQ-INP-002), media type `application/x-teletext-t42`.

These two media types are defined by this project. For interoperability, the module shall also accept `text/plain` and `application/octet-stream`, deciding the format by the detection of REQ-INP-005. A channel's files are accepted and checked as REQ-WEB-027 says. The response shall pass through the normal acquisition path, as page files do (REQ-ARCH-004).

The module shall:
- follow at most 5 redirects. When the port asks for redirects to stay within the channel, a redirect that leaves the channel shall instead be reported, not followed (REQ-WEB-021);
- treat 404 as "page not in this service";
- report other failures as events without disturbing pages already stored.
*Verification:* Test against the local test server of REQ-WEB-013.

**REQ-WEB-004 — Caching and conditional requests.**
The web module shall honour the caching headers of each response. Before fetching a page again, it shall make a conditional request (`If-None-Match` or `If-Modified-Since`) whenever it holds a validator (`ETag` or `Last-Modified`). A response of 304 (not modified) shall mark the stored page as current without changing it.

Pages fetched from the web are held in the normal page store (REQ-ACQ-005), so they remain viewable when the network is lost (REQ-WEB-009). The validators are stored with each page. The number of fetches per second shall be limited by a configurable rate, so that the next-page and pre-fetching functions (REQ-NAV-002 and REQ-NAV-004) cannot flood a server.
*Verification:* Test that unchanged pages cost one conditional request and no body.

**REQ-WEB-005 — Server push.**
When the descriptor names a push address, the web module shall hold a push connection and apply what it receives as it arrives. Two push methods shall be supported:

- **WebSocket** (RFC 6455).
  - A binary message carries T42 packets, as whole 42-byte packets.
  - A text message carries a TTI page.
  - The client sends a text message `subscribe` followed by page numbers, to limit the push to the pages it wants. `subscribe all` asks for the whole service.
- **Server-sent events** (the `text/event-stream` format).
  - An event named `page` carries a TTI page in its data lines.
  - An event named `packets` carries T42 packets encoded in base64.
  - Page numbers to subscribe to are given as a query parameter.

Pushed content enters the normal acquisition path. After a lost connection, the module shall reconnect with increasing delays, starting at 1 second and doubling to at most 60 seconds. For server-sent events it shall resume with the `Last-Event-ID` header. A **long-polling fallback** (a repeated conditional GET that the server holds open until a page changes) should be available for servers that support neither push method.

The zxnet live feed reader (REQ-INP-022) is a separate, fixed protocol and is not changed by this requirement.
*Phase:* Phase 1 for WebSocket and server-sent events; Phase 2 for the long-polling fallback.
*Verification:* Test against the local test server, including dropped connections and resumption.

**REQ-WEB-006 — Cookies and sessions.**
The cookie store shall follow current cookie practice (decision E.35): the latest draft of RFC 6265bis, the replacement for RFC 6265, as implemented by current browsers, made stricter wherever neoteletext's single-origin model allows. The following rules apply.

*Accepting a cookie (processing `Set-Cookie`):*
- **Malformed or empty.** A `Set-Cookie` header containing a control character (other than horizontal tab) shall be ignored entirely. So shall a cookie whose name and value are both empty.
- **Size.** A cookie whose name and value together exceed 4096 bytes shall be ignored. An attribute whose value exceeds 1024 bytes shall be ignored.
- **Host-only.** The `Domain` attribute is ignored, so every cookie is host-only (REQ-WEB-015). This is stricter than the draft, and means no list of public suffixes is needed.
- **Lifetime.** `Max-Age` takes precedence over `Expires`. Any lifetime longer than 400 days shall be reduced to 400 days.
- **Secure cookies over plain HTTP.** A cookie with the `Secure` attribute shall be ignored if it arrives over a plain HTTP connection. A cookie arriving over plain HTTP shall not replace or delete a `Secure` cookie of the same name, even though plain HTTP is allowed (decision E.32).
- **Name prefixes.** Checked without regard to letter case:
  - `__Secure-` requires the `Secure` attribute and an HTTPS connection.
  - `__Host-` also requires `Path=/` and no `Domain` attribute.
  - `__Http-` requires `Secure` and `HttpOnly`.
  - `__Host-Http-` requires all the `__Host-` and `__Http-` conditions together.

  A cookie that breaks the rule for its prefix shall be ignored.
- **SameSite.** The attribute shall be stored as `Strict`, `Lax`, `None` or, when absent or unrecognised, the default. The default shall be enforced as `Lax`.
- **`HttpOnly` and `Partitioned`.** These shall be accepted and stored. neoteletext has no scripting and no embedded third-party content, so every cookie is in effect HTTP-only and first-party.

*Sending cookies:*
- **Which cookies.** Cookies shall be sent only to the host that set them, on paths matching their `Path`, and `Secure` cookies only over encrypted connections.
- **Which requests.** They shall be sent with every request that matches, including WebSocket upgrades and server-sent event connections, over both plain and encrypted connections (decision E.32).
- **Cross-origin form submissions.** A form submission to a different origin is a cross-site request. Only `SameSite=None` cookies that are also `Secure` shall be sent with it.
- **Navigation between services.** A top-level GET to a different service, made by the viewer following a link, shall carry the destination's `Lax`, default and `None` cookies, but not its `Strict` cookies.

*The store:*
- **Opaque values.** Cookie values are opaque: the client never interprets session contents.
- **Limits.** The store shall hold at most a configurable number of cookies per host, 50 by default, and in total, 100 by default, in caller-provided memory (REQ-GEN-003). The least recently used cookies beyond these limits are discarded, expired cookies first.
- **Persistence.** The store may be saved between runs only as REQ-SEC-013 allows. It can be wiped at any time.

*Verification:* A test for every rule above. Where the draft has a published test suite (the web-platform-tests cookie tests), the applicable cases shall be adapted and run.

**REQ-WEB-007 — Navigation on web services.**
The following shall all resolve to pages of the same service, fetched through REQ-WEB-003:
- page number entry;
- next and previous page;
- sub-pages;
- Fastext links, from `FL` lines in TTI or from packet X/27/0;
- Index;
- Home.

Pages already in the store shall be shown at once, with a conditional request made in the background to check they are current.
*Verification:* Test.

**REQ-WEB-008 — Security baseline.**
The requirements of area SEC apply to the web module in full and take precedence over this requirement. In particular:
- local-network protection (REQ-SEC-006);
- no downgrade (REQ-SEC-007);
- plain HTTP safeguards (REQ-SEC-008);
- deceptive forms (REQ-SEC-009);
- strict HTTP handling (REQ-SEC-011).

In addition, the web module shall:
- **allow both plain and encrypted connections.** It shall support both plain HTTP and HTTPS, and both plain and secure WebSockets (`ws://` and `wss://`). Neither requires any special configuration. The scheme of the service descriptor's addresses decides which is used.
  - Plain HTTP must be allowed. Many small devices that might display teletext cannot run TLS, and much teletext content is public in any case.
  - A connection made without encryption shall be shown in the status area (for example, a marker in row 0 columns 0 to 6) and reported as an event. The viewer can then see that the page and any form data travel unprotected.
  - The port may still refuse to offer TLS. The module shall then work over plain connections only, and report an HTTPS address as unreachable.
- **limit sizes.** It shall enforce configurable size limits, with documented defaults:
  - one page response, 256 kilobytes by default;
  - one push message, 64 kilobytes;
  - the service descriptor, 16 kilobytes;
  - the headers of one response, 16 kilobytes.

  Anything larger shall be abandoned with a diagnostic.
- **treat everything received as untrusted input.** Page files, packets, descriptors and headers all pass the robustness requirement REQ-GEN-008, including fuzzing (REQ-TST-040).
- **restrict form submission.** A form may submit only to the origin of the page containing it, or to an origin listed in the service descriptor (decision E.31 item 6). A form on a page fetched over plain HTTP may submit over plain HTTP. Before a form is submitted over plain HTTP, the status area shall show a warning that the entered values will be sent unencrypted.
- **ignore headers in page content.** The withdrawn `FH` form command shall be ignored with a diagnostic, never acted on (decision E.31 item 2).
*Verification:* Test for each rule; fuzzing of the HTTP, WebSocket, server-sent event and descriptor parsers.

**REQ-WEB-009 — Working offline.**
When the network is unavailable:
- Pages already in the store shall remain viewable.
- A page that has not been confirmed current for longer than a configurable period shall be marked stale (REQ-FSM-004), and the row 0 status area shall say so.
- Requests for pages not in the store shall show a "not available offline" status, not a blank screen.

When the network returns, stale pages that are being viewed shall be checked first.
*Verification:* Test with the transport switched off and on.

**REQ-WEB-010 — Forms.**
The web module shall implement the TTI Form Extension version 3 of Appendix C in full. This covers:
- field definitions with explicit columns;
- the field types;
- options;
- the form token;
- the submission target;
- success and error pages;
- client-side validation;
- submission;
- the response handling rules.

A decoder without the web module shall still display form pages correctly as ordinary pages, because it ignores the form commands (REQ-INP-013).
*Verification:* Test of each command and field type; test that the same page displays identically with the form commands stripped.

**REQ-WEB-011 — Form submission and the response.**
On submission, the web module shall:
1. validate every field against its definition (Appendix C.5). If any field fails, it shall not submit, but move to the first failing field and show why in the status area.
2. send the form as `application/x-www-form-urlencoded` data:
   - by POST or GET, as the `FS` command says;
   - to the `FS` address;
   - with the session cookies (REQ-WEB-006);
   - with the form token (`FT`) as the field `ttx_token`;
   - with the name of the submit button pressed as the field `ttx_action`.
3. handle the response:
   - If it is a TTI page (REQ-WEB-003), display that page.
   - Otherwise, a 2xx status leads to the success page of the `FP` command.
   - A 4xx or 5xx status, or a failure of the network, leads to the error page. Values already entered shall be kept, so the viewer can return and try again.

Only one submission of the same form may be in progress at a time.
*Verification:* Test against the local test server for success, validation failure, server error, network failure and a response carrying a TTI page.

**REQ-WEB-012 — Guidance for page servers.**
Servers built to serve neoteletext pages should follow these practices, which the reference test server (REQ-WEB-013) shall demonstrate:
- **Keep no session state on the server.** Hold it in a cookie whose value begins with a format version number, so that the format can change without breaking clients that hold old cookies (decision E.30 item 2).
- **Validate and version.** Issue a form token bound to the session, and check it on every submission. Send `ETag` validators with every page.
- **Push only what changed.** Push only changed pages, and push each page whole, not partial rows.
- **Keep pages simple.** Serve Level 1 and Level 1.5 pages only.
- **Publish channels as folders** (decision E.70). Serve each channel's files under their fixed names in its folder (REQ-WEB-019), whether they are files on disk or generated on request. Answer 404 for a file that does not exist, never a page with status 200. Send one of the media types of REQ-WEB-027. Keep each channel's pages, forms and redirects inside its folder, and put any variation in the folder path or the session cookie, never in a query string, which the browser never sends for a channel's files.
- **Point to it from the web** (decision E.73). Put `<link rel="alternate" type="text/x-teletext-tti" href="...">` in the first 8,192 bytes of each web page that has a teletext edition, directly after `<meta charset>`, naming the channel's folder or the page's file; or send the same as a `Link` header. A program may instead answer a request whose `Accept` puts the teletext types first with `303 See Other` to the page's file, sending `Vary: Accept` so that caches keep the two answers apart. Test with `ttx discover` (REQ-CLI-009).

*Verification:* Inspection of the reference server.

**REQ-WEB-013 — Test server and tests.**
The test suite shall include a small local test server, which may be written in any language because it is test code, not part of the library. It shall serve:
- a service descriptor;
- TTI and T42 pages, with validators;
- WebSocket and server-sent event push;
- cookies and sessions;
- forms with tokens.

It shall be able to inject faults:
- slow responses;
- dropped connections;
- oversized and malformed responses;
- wrong media types;
- redirect loops;
- expired and forged tokens.

The tests of this area shall run against it, and against the in-memory transport of REQ-WEB-001 for exact replay.
*Verification:* Test.


**REQ-WEB-014 — The minimal user agent profile.**
The web module shall implement exactly the following profile, and nothing beyond it, unless this requirement is changed by a recorded decision (decision E.34).

*Supported:*

| Feature | What is supported |
|---|---|
| Protocol | HTTP/1.1 over plain or TLS connections supplied by the port |
| Methods | GET, and POST for form submission only |
| Request headers sent | `Host`, `User-Agent` (fixed, naming neoteletext and its version), `Accept` (the two page media types and JSON, or `text/event-stream` for server-sent events, decision E.39, or for discovery the value REQ-WEB-028 gives), `Accept-Encoding: identity`, `Cookie`, `If-None-Match`, `If-Modified-Since`, `Content-Type` and `Content-Length` (form submissions only), `Last-Event-ID` (server-sent events only), and the WebSocket upgrade headers. No other headers are sent, in particular no `Referer`. |
| Response handling | Status codes 200, 204, 301, 302, 303, 304, 307, 308, 404 and other 4xx and 5xx codes as errors. Headers: `Content-Type`, `Content-Length`, `Transfer-Encoding: chunked`, `Location`, `Set-Cookie`, `ETag`, `Last-Modified`, `Cache-Control` (`max-age`, `no-cache`, `no-store`), `Retry-After`, and, for discovery only, `Link` (REQ-WEB-029). Other headers are ignored. |
| Connections | One request at a time per connection. Connections may be reused (persistent connections); pipelining is not used. |
| Push | WebSocket (RFC 6455) and server-sent events, as REQ-WEB-005 defines |
| Content | TTI pages, T42 packets and the JSON service descriptor; and, for discovery only, the pointers of REQ-WEB-029 in the first 8,192 bytes of an HTML answer. Nothing else is interpreted. |

*Permanently excluded:*
- HTTP/2, HTTP/3 and pipelining;
- compressed responses (REQ-SEC-011);
- authentication by HTTP headers or client certificates chosen by pages;
- HTML, CSS and any other markup, except the scan for pointers in the first 8,192 bytes of an HTML answer during discovery (REQ-WEB-029), which reads nothing else and shows nothing;
- scripting of any kind (REQ-SEC-004);
- images, audio, video and other media;
- downloads to files;
- embedding one page inside another;
- content from a third party loaded into a page, except a font named by the channel or the page at another address, which is permitted only with its SHA-256, without cookies or `Referer`, and only while the viewer allows fonts from other sites (REQ-FNT-003; `docs/font-security.md` section 3.4);
- plugins;
- `file:`, `data:`, `javascript:` and all other address schemes except those of REQ-WEB-016;
- proxy auto-configuration;
- cross-origin requests other than the permitted form origins of REQ-WEB-008, the fonts of REQ-FNT-003, and the redirects followed during discovery (REQ-WEB-028).

Any response or message outside the profile shall be rejected with a diagnostic, never partly interpreted.
*Verification:* Inspection; tests that each excluded feature is rejected; a test that every request sent contains only the permitted headers.

**REQ-WEB-015 — The origin.**
An origin is the combination of scheme, host and port of an address, after the address has been normalised by the parser of REQ-WEB-016:
- the host is compared in lower case;
- a missing port is the scheme's default port (80 for `http` and `ws`, 443 for `https` and `wss`).

Two addresses have the same origin only if all three parts are equal. The one exception is that `ws` and `wss` addresses count as the same origin as the `http` or `https` address with the same host and port. Every security rule that depends on "the same service" or "the same origin" shall use this definition and no other:
- which cookies are sent. Cookies are host-only: a cookie is sent only to the exact host that set it, and the cookie `Domain` attribute is ignored (REQ-WEB-006). This is stricter than RFC 6265bis requires, and is chosen because security overrides (REQ-SEC-001).
- where a form may submit to (REQ-WEB-008 and REQ-SEC-009);
- the no-downgrade rule (REQ-SEC-007), which applies to the host whatever the scheme;
- pre-fetching (REQ-SEC-013);
- the security indicator (REQ-SEC-005).

A service's origin is the origin of its descriptor. In the browser, a channel (REQ-WEB-018) is narrower than an origin: the rules REQ-WEB-026 lists use the channel, and the rest use the origin.
*Verification:* Test with a table of address pairs covering case, default ports, scheme differences and internationalised host names.

**REQ-WEB-016 — The strict address parser.**
All web addresses shall be parsed by one small parser in `neoteletext-web`. This covers addresses in:
- descriptors;
- page address templates;
- redirects;
- push addresses;
- form targets;
- the configuration.

It shall accept only a strict subset of URL syntax:
- **Schemes:** `http`, `https`, `ws` and `wss`, in any letter case.
- **Hosts:**
  - A DNS name of ASCII letters, digits, hyphens and dots, with no empty labels and a total length of at most 253 characters. Internationalised names must already be in their ASCII (Punycode) form.
  - An IPv4 address in dotted decimal only.
  - An IPv6 address in brackets.
- **Port:** an optional decimal port from 1 to 65535.
- **Path and query:** an optional path and query of printable ASCII, with percent-encoding.

It shall reject:
- user names and passwords in the address;
- fragments;
- white space;
- control characters;
- backslashes;
- IPv4 addresses written in octal, hexadecimal or shortened forms;
- more than 2048 characters in total;
- anything else it cannot parse exactly.

Relative addresses shall be resolved against their base address by the rules of RFC 3986, and the result parsed again. The parser shall be `no_std`, shall not allocate, and shall be a fuzzing target (REQ-TST-040). The address checks of REQ-SEC-006 shall be applied to its output.
*Reason:* Different parsers disagreeing about what an address means is a classic source of browser security flaws. One strict parser, used everywhere, removes that class of fault. Rejecting unusual addresses costs nothing for a teletext service.
*Verification:* Test with a corpus of valid, invalid and deceptive addresses, including known cases where parsers disagree; fuzzing.


**REQ-WEB-017 — Device-bound sessions.**
*Phase:* Phase 2 (whole requirement).
Where the port can keep a signing key that cannot leave the device, the web module should support Device Bound Session Credentials (decision E.35). Examples are a secure element, a trusted platform module or a microcontroller's protected key storage. The port supplies the key through an optional part of the platform interface (REQ-ARCH-009).

The session is then bound to that device. The server issues short-lived cookies only after the client proves it holds the key, so cookies copied from the device, or read on a plain HTTP connection, soon become useless. Ports without such storage shall simply not offer it, and sessions then work as REQ-WEB-006 describes. The implementation shall follow the W3C specification as it stands when the work is scheduled.
*Verification:* Test against a test-server implementation of the protocol.

**REQ-WEB-018 — Channels: a channel is a folder.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
The web module shall be able to open a channel given only the address of its folder, and the browser shall open every web-native site this way (decision E.70). A channel is a folder on a web server: its address is an `http` or `https` address whose path ends in `/`, written in the canonical form of REQ-WEB-023.

**Identity.** A channel is identified by its origin (REQ-WEB-015) together with its folder path, compared exactly, including letter case. Every rule that depends on "the same channel" shall use this identity and no other: the tuning screen (REQ-NAV-014), the name in row 0 (REQ-SEC-025), cookies, forms, push, redirects and links (REQ-WEB-026), history and bookmarks (REQ-BRW-003, REQ-BRW-006), and the channels confirmed in a session. The rules that concern the server as a whole keep using the origin: the plain-HTTP marker (REQ-SEC-005), no downgrade (REQ-SEC-007) and the local-network checks (REQ-SEC-006). A folder inside a channel's folder is a different channel, and inherits nothing from it.

**Opening a channel.** The web module shall make these requests in this order, each relative to the channel's folder, and no others:
1. **`channel.json`.**
   - When it answers 200 with a valid file (REQ-WEB-024), the channel takes its settings.
   - When it answers 404 or 410, the channel takes the defaults: initial page 100, no push address, no linked channels, no aliases, no refresh interval, no other form origins, and the configured default character set region.
   - Any other answer, including a server error, a refused media type (REQ-WEB-027) and an invalid file, shall be reported as a failure, and the channel shall not open. The defaults shall never replace a file that exists but cannot be used.
   - In the browser, an answer of a redirect that leaves the folder, or of status 200 with the media type `text/html`, is a failure only when the folder turns out to hold a channel. When every request of this list answers 404, 410, a redirect that leaves the folder, or status 200 with `text/html`, the folder holds no channel, and the browser runs discovery instead (REQ-WEB-028, decision E.73). The requests after `channel.json` are therefore still made in that case.
2. **`index.t42`.**
   - When it answers 200 with whole T42 packets, every page it holds shall be stored (REQ-WEB-019).
   - When it answers 404 or 410, the channel has no bundle.
   - Any other failure shall be reported, and the channel shall open without the bundle.
3. **The initial page,** from the store when the bundle held it, otherwise from its own file (REQ-WEB-019).

`contents.txt` is not asked for when a channel opens. It is asked for when the viewer first opens the contents view (REQ-BRW-009).

**Descriptor services.** A service descriptor given by an address whose file name is not `channel.json` is a service of REQ-WEB-002, with its page address template. Its channel identity is its origin and the folder that holds the descriptor, its contents are the `contents.txt` in that folder, and every rule of this requirement other than "Opening a channel" applies to it.
*Reason:* A channel in a folder works on every static host, including a GitHub Pages project site at `user.github.io/project/`, and keeps strangers on one shared host apart. Three fixed requests answer every question about a channel, with nothing inferred.
*Amended 12 October 2026* (decision E.70): this requirement was "Sites", which made the origin the site and looked for the descriptor at `/.well-known/teletext.json`. *Amended 12 October 2026* (decision E.73): a folder that holds no channel leads to discovery.
*Verification:* Tests against the local test server with a channel at the root and two channels in different folders of one origin; with and without `channel.json` and `index.t42`; with a `channel.json` that answers a server error, one that is invalid, and one served as `text/html`; and that a channel never asks for any other file when it opens.

**REQ-WEB-019 — The files of a channel.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
A channel's files shall have these fixed names, in the channel's folder, and no others (decision E.70 item 4):

| File | Holds |
|---|---|
| `NNN.tti` | Page `NNN` as a TTI file (REQ-INP-013), with its sub-pages |
| `NNN.t42` | Page `NNN` as T42 packets (REQ-INP-002), with its sub-pages |
| `index.t42` | Any number of pages, as T42 packets |
| `channel.json` | The channel's settings (REQ-WEB-024) |
| `contents.txt` | The channel's contents (REQ-WEB-025) |

`NNN` is the page number as three hexadecimal digits, the magazine 1 to 8 first, with the digits `A` to `F` in capitals: `100.tti`, `1FE.t42`. The web module shall ask for page files only in this form.

**Finding a page.** A page shall be looked for:
1. in the store, when it is there and confirmed current (REQ-WEB-004);
2. otherwise in `NNN.tti`; when that answers 404 or 410, in `NNN.t42`;
3. otherwise it is "not found", and is reported so (REQ-WEB-003).

Within a channel, the extension that last answered 200 shall be tried first, and the other second. This changes only the number of requests, never which file is used when both exist: when the first answers 200, the second is not asked for. Conditional requests to keep a page current (REQ-WEB-004) shall go to the file that answered.

**What each file may fill.**
- **A page's own file** shall fill only pages that have been asked for in this channel, and enhancement pages, whose page numbers are not decimal and which a page may carry for its objects and DRCS characters (decision E.63 item 5). Any other page in it shall be dropped, by its header, with a diagnostic.
- **`index.t42`,** a pushed message, a form response and a local file shall fill every page they hold.
- `index.t42` is bounded by the page response limit of REQ-WEB-008, 256 kilobytes by default. A channel larger than that publishes page files.

When the bundle or a file does not hold the page asked for, and the viewer has not chosen a page, the first page it holds shall be shown.
*Reason:* A page's file cannot overwrite the rest of its channel, while a whole capture published as `index.t42` is a working channel. Fixed names mean one layout to learn and to build.
*Amended 12 October 2026* (decision E.70): fixed file names in the channel's folder replace the page address template and the front door at the root.
*Verification:* Tests that a page is found as `.tti`, as `.t42`, and not at all; that the order of the two requests changes after a success but the result does not; that a page file holding other pages fills only the pages asked for and enhancement pages; and that `index.t42` fills every page.

**REQ-WEB-020 — Aliases and refresh.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
In the browser, when the channel's `channel.json` has aliases (REQ-WEB-024), a typed number that has no link on the current page shall follow the alias for that number, if there is one (decision E.64 item 4). An alias to another channel goes through the tuning screen (REQ-NAV-014). When the descriptor has a refresh interval and the service has no push, the page on screen shall be asked for again at that interval, with a conditional request (REQ-WEB-004).
*Verification:* Test that an alias leads to its target and that a page's own link wins over it; test the refresh interval against the local test server.

**REQ-WEB-021 — Redirects within the channel.**
The web module shall offer a setting under which a redirect that leaves the channel is not followed, but reported as an event naming the new address (decisions E.65 item 5 and E.70 item 12). A redirect stays within the channel when the new address has the same origin and lies inside the channel's folder. A redirect from `http` to `https` also stays within the channel when the host and the folder are the same and both ports are the schemes' default ports; a redirect to any other port does not. The browser shall use this setting for every request of a channel except form submissions, which keep the rule of REQ-WEB-006; `ttx` follows redirects as REQ-WEB-003 says. The limit of five redirects applies in both cases.
*Reason:* Otherwise a redirect would take the viewer to another channel without the tuning screen.
*Amended 12 October 2026* (decision E.70): the boundary is the channel's folder, not the origin.
*Verification:* Tests that a redirect to another origin and a redirect to another folder of the same origin are reported and not followed; that a redirect to a file inside the folder and an upgrade to `https` on the same host and folder are followed; and that an upgrade to another port is reported.

**REQ-WEB-022 — Enhancement dependencies.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
When the browser shows a page at Presentation Level 2.5 or 3.5 and its enhancement pages are not in the store, it shall ask once for its magazine's Organization Table (page xFE), then for the object and DRCS pages that the table and the page name (decision E.63 item 5). Pages already stored shall not be asked for again. These requests shall not change which page is kept current. While they load, the page is shown with what has arrived, and is drawn again as they arrive. A dependency that cannot be fetched removes only what depends on it (decision E.41).
*Verification:* Test against the local test server that a Level 2.5 page fetches its table and object pages once.

**REQ-WEB-023 — Channel addresses.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
A channel's address shall be checked and written in one canonical form, so that two spellings never name one channel and one spelling never names two (decision E.70 item 3). After the strict parser of REQ-WEB-016:
- the scheme shall be `http` or `https`; the host is compared and written in lower case, because host names are not case-sensitive;
- the port is written only when it is not the scheme's default, and an explicit default port names the same channel;
- the address shall have no query and no fragment;
- the folder path shall begin and end with `/`. Each segment between the slashes shall be 1 to 64 characters from the unreserved set of RFC 3986 section 2.3: the ASCII letters, the digits, `-`, `.`, `_` and `~`. No segment may be `.`, `..` or made only of dots. Percent-encoding shall not appear in a folder path. There shall be at most 8 segments, and the whole path shall be at most 256 characters.

An address whose folder path breaks any of these rules shall be refused with a diagnostic that names the rule, never corrected. Letter case in the host and an explicit default port are not ambiguous, and are normalised. The root folder `/` is a channel like any other.

A channel's file addresses are its folder's address followed by one of the file names of REQ-WEB-019. An address whose path is a folder followed by one of those names belongs to that folder's channel. Any other address names no channel.
*Reason:* Path tricks such as `%2F`, `..` and doubled slashes are the classic way to make one address look like another. Refusing them outright costs nothing, because the allowed set holds the names hosting services use.
*Verification:* A table of addresses, valid and invalid, including `%2F`, `%2E`, `..`, `.`, doubled slashes, upper-case hosts, default and explicit ports, a query, a fragment, nine segments, a segment of 65 characters, and `/ab/` against `/a/`.

**REQ-WEB-024 — `channel.json`.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
`channel.json` is the service descriptor of REQ-WEB-002, read by the same strict, bounded reader with the same 16-kilobyte limit, with these differences (decision E.70 item 7):
- **Every member is optional.** An absent member takes the default of REQ-WEB-018.
- **`page_template` is not allowed.** A `channel.json` that has it shall be refused with a diagnostic, because a channel's files have fixed names.
- **`initial_page`** is a page number of three decimal digits, the magazine 1 to 8 first.
- **`name`,** when present, is the channel's own name for itself. It may be shown as the tab's title, but never in row 0 (REQ-SEC-025) or on the tuning screen, which name a channel only by its host and folder, and by the name the linking channel declares.
- **`services`,** the linked channels, gives each one a short name, a display name, and either `channel`, the address of the linked channel's folder in the form of REQ-WEB-023, or `descriptor`, the address of a descriptor service (REQ-WEB-018). An entry with both, or neither, shall be refused.
- **`font`,** an object with `address` and `sha256`, names the channel's font (REQ-FNT-006).
- **`aliases`, `refresh`, `push` and `form_origins`** are as REQ-WEB-002 defines them. A push address shall be on the channel's origin, in any folder.

Unknown members shall be refused, as REQ-WEB-002 requires of every descriptor.
*Verification:* Tests of an empty object, of each member alone, of `page_template`, of a linked channel given each way and both ways, of a push address on another origin, and of an unknown member.

**REQ-WEB-025 — `contents.txt`.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
`contents.txt` lists a channel's sections, pages and linked channels, in the order they shall be shown (decision E.70 item 8). It shall be read by a strict, bounded reader:
- **Size.** At most 64 kilobytes and 1000 lines. A larger file shall be refused whole, with a diagnostic.
- **Characters.** Lines end with LF or CR LF. Fields are separated by one horizontal tab. A byte outside printable ASCII, other than the tab and the line ends, makes its line invalid.
- **Ignored lines.** Empty lines, and lines whose first character is `#`, are comments.
- **Entries.** The first field names the kind of entry:

  | Kind | Fields after the kind | Meaning |
  |---|---|---|
  | `s` | title | A section heading |
  | `p` | page, title, and optionally a date | A page of this channel |
  | `x` | target, title | Another channel or a page of one |

  - **The page** of a `p` entry is three decimal digits, the magazine 1 to 8 first: a page a viewer can type. Enhancement pages are not listed.
  - **The date** of a `p` entry, when present, is when the page last changed, as `YYYY-MM-DD` (ISO 8601).
  - **The target** of an `x` entry is `name:mpp`, a linked channel named in `channel.json`, or an address: a channel's folder, or a page file of a channel, as REQ-WEB-023 defines them.
  - **A title** is 1 to 32 characters. A longer title shall be cut to 32.
- **Faults.** A line of an unknown kind shall be ignored, so that later versions may add kinds. A line of a known kind with the wrong number of fields, an invalid page, date or target, or an empty title shall be ignored. The second and later `p` entries for one page shall be ignored. The number of ignored lines shall be reported, and shown in the contents view's status line.
- **At most 800 `p` entries** are kept; later ones shall be ignored and counted.

Nothing in `contents.txt` is checked against the channel's files: a listed page that does not exist is "not found" when chosen, as a typed number would be.
*Reason:* One small text format that a person can write by hand, a server can generate, and the author's tool can write (REQ-CLI-008), is the whole of a channel's map.
*Verification:* Tests of each kind of entry, of every fault listed, of comments and both line ends, of the size and count limits, and of a duplicate page.

**REQ-WEB-026 — The channel is the boundary.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
In the browser, the channel shall be the boundary for these rules (decision E.70 item 12). Each is stricter than the origin-based rule of the same name, which still applies.
- **Cookies.** Each channel has its own cookie store, under the rules of REQ-WEB-006. A cookie set by a response to a request inside the channel's folder belongs to that channel. A `Path` attribute outside the folder, including the default path, shall be narrowed to the folder; a `Path` inside it is kept. A channel's cookies shall be sent only with requests inside its folder, with its push connection, and with its form submissions, and never with another channel's requests, even on the same origin.
- **Forms.** A form shall submit only to an address inside the channel's folder, or to an origin that `channel.json` lists in `form_origins` (REQ-WEB-008). A form whose target is elsewhere shall be refused before anything is sent, with the target shown.
- **Push.** The push connection named in `channel.json` shall be on the channel's origin, and may be in any folder.
- **Redirects.** As REQ-WEB-021 says.
- **Links.** An address that is a page file in the channel's own folder is a page of the channel, followed at once. Every other address leads to another channel, through the tuning screen (REQ-NAV-014).
*Reason:* On a shared host, another folder is often another person. The channel must not share a session, a form target or a link's trust with its neighbours.
*Verification:* Tests with two channels on one origin: their cookies are kept apart, including a cookie set with `Path=/`; a form targeting the other channel's folder is refused; a link and a redirect into the other folder show the tuning screen.

**REQ-WEB-027 — Media types and content checks of a channel's files.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
No teletext media type is registered, and common hosts send generic types (decision E.70 item 11). A channel's files shall therefore be accepted with these media types, whatever their parameters, such as `charset`:

| File | Accepted media types |
|---|---|
| `NNN.tti` | `text/x-teletext-tti`, `text/plain`, `application/octet-stream` |
| `NNN.t42`, `index.t42` | `application/x-teletext-t42`, `application/octet-stream`, `text/plain` |
| `channel.json` | `application/json`, `text/plain`, `application/octet-stream` |
| `contents.txt` | `text/plain`, `application/octet-stream` |

Any other media type, including `text/html`, shall be refused, with a diagnostic naming the file and the type. A response with no media type shall be refused. These rules apply to responses with status 200. A 404 or 410 means the file does not exist, whatever the type and body of the response, since servers commonly answer it with an HTML page.

Each file shall then pass the check for the format its name says, and is refused otherwise, never read as another format:
- **a TTI file** shall be a TTI page file with at least one `PN` line, read by REQ-INP-013;
- **a T42 file** shall be a whole number of 42-byte packets, every one with a valid Hamming 8/4 magazine and row address (EN 300 706 clause 8.2);
- **`channel.json`** shall pass REQ-WEB-024;
- **`contents.txt`** shall pass REQ-WEB-025.

A refused page file is reported as a failure of that page, not as "not found", and the other extension is not then tried.
*Reason:* A file's name and type cannot be trusted alone: `.tti` is also Template Toolkit's extension, `.t42` is also PostScript Type 42 fonts', and some hosts answer a missing file with an HTML page and status 200.
*Verification:* Tests serving each file with each accepted type and with `text/html`, with no type, with a Template Toolkit file named `100.tti`, with a Type 42 font named `100.t42`, and with an HTML "not found" page answered with status 200.

**REQ-WEB-028 — Discovery: from a web address to a channel.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
The web module shall be able to find the teletext edition that an ordinary web address announces, and the browser shall use it, as decision E.73 sets out. The design is `docs/web-discovery.md`.

**When it runs.** Discovery shall run only after the viewer has confirmed the address on the tuning screen (REQ-NAV-014), and only for:
- **a folder that holds no channel.** The browser first opens the folder as a channel (REQ-WEB-018). When every one of its requests (`channel.json`, `index.t42`, and the asked-for page as `.tti` and as `.t42`) answers 404, 410, a redirect that leaves the channel's folder, or status 200 with the media type `text/html`, the folder holds no channel, and discovery runs on the address as the viewer gave it, before any trailing `/` was added. A refused media type is decided from the headers, and the body of such an answer shall not be read;
- **an address that names no channel** (REQ-WEB-023): one whose last segment is a file name that is not one of a channel's, or one with a query.

It shall never run for an address that is a channel's file (REQ-WEB-019), for pre-fetching (REQ-SEC-013), for an address a pointer led to, or because of anything a page or a decoder sends.

**The request.** The web module shall make one `GET` request for the address, with these headers and no others: `Host`, the fixed `User-Agent`, `Accept: text/x-teletext-tti, application/x-teletext-t42, text/html;q=0.5`, and `Accept-Encoding: identity`. It shall send no cookie and no `Referer`, and shall ignore every `Set-Cookie` in the answers. It shall follow at most 5 redirects, to any origin, applying the local-network checks (REQ-SEC-006) to every address reached and refusing a redirect from `https` to `http` (REQ-SEC-007). The time limits of REQ-SEC-011 apply.

**What it reads.** From the final answer it shall read the headers, within the limit of REQ-WEB-008, and, when the answer is status 200 with the media type `text/html`, at most the first 8,192 bytes of the body after any chunked encoding is removed. It shall then close the connection without reading further, and the connection shall not be reused. A `Content-Encoding` other than `identity` ends discovery with nothing found.

**The order of the answer.** The first of these that gives a target passing REQ-WEB-030 is the result:
1. **A page.** When the final address is a page file of a channel (`NNN.tti` or `NNN.t42`, REQ-WEB-023), and the answer is status 200 with a media type that REQ-WEB-027 accepts for that file, the result is that page of that channel. The body read for discovery is not used; the channel then opens and fetches the page as usual. A teletext media type at any other address shall be refused with a diagnostic saying that pages must be served from a channel's page files.
2. **The `Link` header pointers** of the final answer, in the order they appear (REQ-WEB-029).
3. **The `<link>` element pointers** in the first 8,192 bytes of the body, in the order they appear (REQ-WEB-029).

When none gives a target, the result is "no teletext edition found", shown with the address asked for and, where it differs, the final address.

**The result.** Discovery shall return to the browser the target address, the final address that announced it, and how it was found. The browser then goes to the target as REQ-NAV-014 says. A target that is not a channel, or whose channel cannot be opened, shall be reported as a failure; discovery shall never run on it.
*Reason:* Decision E.73. The four requests of decision E.70 item 5 are kept first, so a channel at a folder address behaves exactly as before.
*Verification:* Tests against the local test server (REQ-TST-081) for each case of "When it runs" and each of the three results; that no request carries a cookie or `Referer`; that a redirect to a local address, and from `https` to `http`, is refused; that no more than 8,192 bytes of a body are read, measured at the transport; and that a target that fails to open does not lead to a second discovery.

**REQ-WEB-029 — Pointers: the `Link` header and the `<link>` element.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
A pointer is a link relation of type `alternate` whose media type is `text/x-teletext-tti` or `application/x-teletext-t42`, compared without regard to letter case, and with any parameters ignored. Both readers below shall be `no_std`, shall not allocate, shall be fuzzing targets (REQ-TST-040), and shall refuse what they cannot read exactly rather than guess.

**The `Link` header** (RFC 8288 section 3). Every `Link` header field of the final answer shall be read, in order. Each link value is `<` target `>` followed by parameters, separated by `;`, with link values separated by `,` outside quoted strings. A parameter value is a token or a quoted string. A link value is a pointer when:
- its `rel` parameter, split at spaces, contains `alternate`;
- its `type` parameter is one of the two media types;
- it has no `anchor` parameter, since that would make it describe some other resource.

A link value that is malformed is ignored, and the reader continues with the next value of the field; a field that cannot be split into values is ignored whole. At most 16 link values shall be examined in all.

**The `<link>` element.** The first 8,192 bytes of an HTML body shall be scanned as bytes, comparing names without regard to ASCII letter case. The scanner shall:
- **stop** at a byte-order mark for UTF-16, at a zero byte, at `<body` followed by white space, `/` or `>`, at `</head`, and at the end of the 8,192 bytes, including in the middle of a construct;
- **skip** comments from `<!--` to the next `-->`, `<!` and `<?` constructs to the next `>`, end tags to the next `>`, and the contents of the elements `script`, `style`, `noscript`, `template`, `textarea`, `title` and `xmp`, up to their end tag, as the HTML standard's raw text and escapable raw text elements are read;
- **read the attributes of every start tag**, so that a `>` inside a quoted value does not end it: a name runs to `=`, white space, `/` or `>`; a value is double-quoted, single-quoted, or unquoted up to white space or `>`. Of two attributes with the same name, the first is used, as HTML does;
- **note a `<base>` element** with an `href` attribute;
- **take a `<link>` element** as a pointer when its `rel` attribute, split at white space, contains `alternate`, its `type` attribute is one of the two media types, and its `href` attribute is present. Leading and trailing white space is removed from the `href` value.

At most 16 `<link>` elements with `rel` containing `alternate` shall be examined. Character references are never decoded: an `href` that holds `&` is not a valid target (REQ-WEB-030). Nothing else in the HTML is read, kept or shown.
*Reason:* A small, exact reader of two constructs, bounded by the 8,192-byte prefix, is enough to find a pointer that the publisher has put where decision E.73 says, and is far smaller than an HTML parser.
*Verification:* Tests of every rule above: pointers in headers and in the body, in both letter cases, with parameters, with `anchor`, malformed values, quoted commas; `<link>` elements inside comments, scripts, styles and titles (not taken), after `<body` or `</head>` (not taken), split by the 8,192-byte limit (not taken), with `>` in a quoted value, with duplicate attributes, unquoted values, and a UTF-16 body; the 17th candidate; and fuzzing.

**REQ-WEB-030 — The target of a pointer.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
A pointer's target shall be checked before it is used, and refused, with a diagnostic naming the reason, when any check fails:
1. **Characters.** The target holds only printable ASCII, and no `&`.
2. **Resolution.** An absolute target is used as it is. A relative target from a `Link` header, or from a `<link>` element with no `<base>` element before it, is resolved against the final address of the answer by RFC 3986 section 5. A relative target from a `<link>` element after a `<base>` element is refused, and the publisher is told to write an absolute address.
3. **The strict parser.** The result passes REQ-WEB-016, and its scheme is `http` or `https`.
4. **A channel.** The result is a channel's folder, or one of a channel's files, as REQ-WEB-023 and the address bar (REQ-BRW-002) take them. A folder without its trailing `/` is refused, so that a target is never guessed.
5. **No downgrade.** When the answer that gave the pointer came over `https`, the target is `https` (REQ-SEC-007).
6. **The local network.** The target's host is checked by REQ-SEC-006 when it is connected to, as every address is; a target that is a literal address in a blocked range is refused at once.

The browser shall receive the target from the fetcher as an address, and shall parse and check it again by rules 3 to 5 before acting on it.
*Reason:* A pointer is input from an arbitrary web server. Refusing every target that is not plainly a channel keeps discovery from leading anywhere a typed address could not.
*Verification:* A table of targets, valid and invalid, covering each rule, including `&amp;` in a target, a relative target with and without `<base>`, a folder without its trailing `/`, an `https` answer pointing to `http`, a target at 127.0.0.1 and at 192.168.1.1, a `javascript:` target, and a target that is a file name of no channel.

---

## F.20 The browser (BRW)

This area is the windowed browser, `neoteletext`, designed in `docs/desktop-browser.md`, `docs/web-native-concept.md` and `docs/channels.md`, and recorded in decisions E.63 to E.70. Its security requirements are REQ-SEC-025 and REQ-SEC-026, and its navigation requirements REQ-NAV-013 to REQ-NAV-015.

Every requirement in this area carries the same phase: Phase 1 on macOS and Linux, and Phase 2 on Windows, Android and iOS (section F.0.1).

**REQ-BRW-001 — One program, with its behaviour in a platform-free crate.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser shall be one Rust program built on egui for every platform (decision E.68 item 1). Every decision about what the browser shows and does (addresses, channels, history, the tuning screen, row 0, the receiver's own pages, the contents view and the formats of saved files) shall be made in `neoteletext-browser`, a library crate with no operating-system access (REQ-SEC-016). `neoteletext-gui` holds only the window, the helper processes and storage. The program shall also be its own helpers: started with a hidden command, it runs as a tab's fetcher or decoder (REQ-SEC-026).
*Verification:* Inspection; `xtask check-io` classes `neoteletext-browser` as a library crate.

**REQ-BRW-002 — Tabs and the address bar.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser window shall hold any number of tabs. Each tab has its own location, history, helper processes and cookies. The window shall have a tab strip; a toolbar with back, forward, reload and home buttons, the connection's security, the address bar and buttons for the four coloured keys; a menu with bookmarks, history, copying and settings; and a status line.

The address bar shall accept:
- **a channel's folder:** `https://host/`, `https://host/ceefax/`, or a bare `host` or `host/ceefax/`, which mean `https`. The channel opens at its initial page (REQ-WEB-018);
- **a folder without its trailing slash:** `https://host/ceefax` is the folder `ceefax/` when the last segment contains no `.`. A last segment with a `.` is a file name, and must be one of the names below;
- **a channel's file:** `https://host/ceefax/140.tti` or `140.t42` is that page of that channel; `index.t42` and `channel.json` are the channel at its initial page; `contents.txt` is the channel's contents view (REQ-BRW-009);
- **a channel and a page number:** `host/ceefax 140`, a folder address, a space and three digits;
- **a descriptor service:** `https://host/path/name.json`, for any file name other than `channel.json`, is the service that descriptor names (REQ-WEB-018);
- `zxnet:name`: a known zxnet live feed (REQ-INP-022);
- `file:///path` or `/path`: a local file (REQ-BRW-005);
- **an ordinary web address:** any other `http` or `https` address, such as `https://host/news/story.html` or one with a query, whose teletext edition is found by discovery (REQ-WEB-028). A folder address that holds no channel is also given to discovery.

Every address shall pass the strict parser of REQ-WEB-016; channel addresses shall also pass the channel rules of REQ-WEB-023, and only `http` and `https` lead to channels or discovery. A page number must be three digits of a real page. While a page is shown, the address bar shows the address of its page file, in the extension that answered, which typed back leads to the same page.
*Amended 12 October 2026* (decision E.73): ordinary web addresses lead to discovery, where any other file name was refused.
*Verification:* Test of each accepted form and of rejected ones; a window test.

**REQ-BRW-003 — Each tab's history.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
Each tab shall keep a history of the pages it has shown, across channels, of at most 100 entries, the oldest dropped first. Each entry records the location, the page and a title from the page's first line of text. A new page after going back drops the entries ahead of it. Going back or forward to another channel shows the tuning screen unless that channel was confirmed earlier in the session (REQ-NAV-014).
*Verification:* Test of back, forward, the dropped entries and the case of another channel.

**REQ-BRW-004 — Keys.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser's keys shall follow decision E.67:

| Function | Letter | Other keys |
|---|---|---|
| Page number | `0` to `9` | |
| Coloured keys | `r`, `g`, `y`, `c` | F1 to F4 |
| Index | `i` | End |
| Initial page | `h` | Home |
| Next and previous page | `+`, `-` | Up, Down |
| Sub-pages | | Right, Left |
| Back and forward | Backspace, `f` | Page Up, Page Down; Command-Left, Command-Right; Command-[, Command-] |
| Hold | `H` | Insert |
| Reveal, mix, zoom | `?`, `m`, `z` | `*`, `/` |
| Reload | `u` | F5, Command-R |
| Page catching, show links | Enter, `l` | |
| Into the page's form | Tab | |
| Confirm | Enter | F4 |
| Cancel | Escape | Delete |
| Address bar | | Command-L, F6 |
| New tab, close tab, next tab | | Command-T, Command-W, Control-Tab |
| Bookmark | | Command-D |
| Copy the page's text | | Command-Shift-C |
| Glyph set | `G` | |
| The keys page | `K` | |
| The channel's contents | `C` | |
| Quit | | Command-Q |

Escape never quits. `K` shows a page, drawn by the receiver, that lists the letter for each function first, each line fitting the 40 columns and using no character that differs between national options. While a form has the keys, typing is text, and F1 to F4 remain the coloured keys.
*Verification:* A test that every line of the keys page fits; window tests of the main keys.

**REQ-BRW-005 — Files.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser shall open a local file given in the address bar or dropped on the window (decision E.68 item 5). A file whose name ends in `.t42` shall be read as T42 packets, up to 256 megabytes; any other file as TTI, up to 256 kilobytes. Every page a file holds shall be stored, and the file opens at the page asked for or, failing that, its first page (REQ-WEB-019). A file is its own channel for the tuning screen.
*Verification:* Test of opening each format by path; test of the first-page rule.

**REQ-BRW-006 — Saved data.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser shall keep bookmarks, history, the open tabs and its settings between runs, in the platform's application data directory: Application Support on macOS, `$XDG_DATA_HOME` or `~/.local/share` on Linux, `%APPDATA%` on Windows (decision E.68 item 8). Only the interface process shall write them. Each file shall be written whole, by writing a temporary file and renaming it.
- History shall keep 30 days.
- Confirmed channels shall not be saved.
- Restored tabs shall reopen at their tuning screens.
- Cookies shall not be saved (REQ-SEC-013).

Reading shall be strict and bounded, because the files could have been changed by anything: at most 4 megabytes per file and 10 000 records, and every stored address read back through the address bar's parser, so a stored address can never mean more than a typed one.
*Verification:* Tests of writing and reading back, of the history limit, and of malformed files; a window test that a bookmark is kept and listed on a new tab's start page.

**REQ-BRW-007 — Fonts and the window's own text.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
Pages shall be drawn with teletext fonts only: Bedstead, and the fonts of area FNT. The settings shall let the user choose a font, by address, from a font library (REQ-FNT-004) or from a local file; set "always use my font" (REQ-FNT-005); allow or refuse fonts from other sites (REQ-FNT-003), allowed by default; and see, pin and remove cached fonts (REQ-FNT-009). These settings apply to every tab and are saved. The window's own text shall use the platform's fonts, read at start, and no bundled font whose licence is not on REQ-GEN-017's list.
*Amended 12 October 2026* (decision E.72): the switch between the original glyphs and Bedstead is replaced by the fonts of area FNT.
*Verification:* Window tests of choosing a font and of the override.

**REQ-BRW-008 — Accessibility.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The window's controls shall be described to the platform's screen reader. The page area shall be labelled with the page's text, as `ttx text` gives it. The page's text shall be copyable. The rest of `docs/web-native-concept.md` section 11 (navigation row by row and link by link, and link descriptions) is not yet required.
*Verification:* Inspection with the platform's screen reader.

**REQ-BRW-009 — The contents view.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The browser shall show a channel's contents (REQ-WEB-025) as teletext screens drawn by the receiver (decision E.70 item 8).
- **Opening it.** The `C` key, the menu's "Contents" item, and the address of the channel's `contents.txt` open the contents view of the current channel. It replaces the page area until the viewer leaves it.
- **Fetching.** `contents.txt` shall be asked for once in each visit to the channel, the first time the view is opened; opening the view again in the same visit shows the same contents. It is fetched by the tab's fetcher and read by the interface; it never passes through a decoder, and its screens are built in the interface process like the receiver's other pages (REQ-SEC-025).
- **Row 0** shows `LIST` in the status area, then the channel's host and folder and the clock, as REQ-SEC-025 says.
- **Rows 1 and 2** show "Contents" in double height, with the screen number and the number of screens, such as `1/3`, at the right.
- **Rows 4 to 23** show twenty entries per screen, in the file's order:
  - **a section heading** in yellow, from column 1. A heading that would fall on row 23 starts the next screen instead, so that a heading is never separated from its first entry;
  - **a page** in white: column 1 holds `*` when the entry's date is later than the viewer's previous visit to this channel recorded in the history, and a space otherwise; its title starts at column 2; dots fill the row up to column 35; and the page number is in columns 37 to 39;
  - **another channel** in cyan: column 1 holds the receiver's mark for leaving, the right guillemet `»` from the G2 set; the title starts at column 2; and the page number, when the target names one, is in columns 37 to 39.
- **Row 24** is drawn by the receiver: "Enter go   Left Right more   Esc back".
- **Keys.** Up and Down move a selection, drawn with swapped colours as in page catching (REQ-NAV-015); Enter follows the selected entry; Left and Right show the previous and next screen; digits type a page number as on any page; a pointer click follows the entry under it; and Escape or Delete returns to the page shown before.
- **Following an entry.** A page entry is found as any page of the channel (REQ-WEB-019). An entry for another channel goes through the tuning screen (REQ-NAV-014), as a link on this channel, marked as declared when its target is a `name:mpp` that `channel.json` names.
- **Without a contents file.** When `contents.txt` answers 404 or 410, the view shall list instead the pages of this channel the tab has shown, in page number order, each titled as its history entry is (REQ-BRW-003), cut to 32 characters, with row 3 saying "No contents file: pages received so far". When `contents.txt` fails in any other way, row 3 shall say that the contents could not be used and why, and the same list follows.
- **Faults.** The number of lines `contents.txt` had that were ignored (REQ-WEB-025) is shown in the window's status line, never on the page.
- **Text.** Titles are drawn by the receiver with the characters every national option shows the same; other characters are drawn as `?`.
*Verification:* Tests of the screens drawn from a contents file with each kind of entry, a section on the last row, more than one screen and a dated entry; of each key; of the list drawn without a contents file; and a window test.

---

## F.21 Fonts (FNT)

This area is the receiver's fonts (decision E.72): the bundled Bedstead font, and fonts in one 12 × 10 form loaded when the user, the channel or the page names them. The security analysis is `docs/font-security.md`. Mosaics, G3 characters and DRCS characters are not fonts and are not affected.

**REQ-FNT-001 — The bundled font.**
The receiver shall carry one font, Bedstead 3.261, in the 5-dot by 9-line form of REQ-RND-010, drawn with the character rounding of REQ-RND-012. It is the default font of every page and the only font of the receiver's own drawing: row 0, the tuning screen, the contents view, the start, keys, error and loading pages, and any confirmation. Its provenance and licence (CC0 1.0 and section 55 of the United Kingdom Copyright, Designs and Patents Act 1988) shall be recorded in `third_party/bedstead/PROVENANCE.md`, with the SHA-256 of its export, which the gate checks. A character it lacks, and that cannot be composed from a base letter and a spacing mark (decision E.26), shall be drawn as the placeholder of REQ-CHR-004.
*Verification:* Inspection of provenance; the gate's check of the export's SHA-256; a coverage test listing every required character Bedstead lacks.

**REQ-FNT-002 — The font form.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
A font other than the bundled one shall be a file in the Glyph Bitmap Distribution Format (BDF) version 2.1 or 2.2, accepted only when all of these hold:
- the file runs from `STARTFONT` to `ENDFONT`; `COMMENT` lines may appear anywhere;
- `FONTBOUNDINGBOX 12 10 0 0`;
- the properties include `CHARSET_REGISTRY "ISO10646"`, `CHARSET_ENCODING "1"` and `SPACING "C"` or `SPACING "M"`, and at most 64 properties in all;
- `CHARS` gives the number of characters, at most 1,024, and exactly that many follow;
- every character has `ENCODING` a Unicode scalar value, never −1 and never repeated; `DWIDTH 12 0`; `BBX 12 10 0 0`; and a `BITMAP` of exactly ten rows of four hexadecimal digits whose last four bits are zero;
- the file is at most 256 kilobytes, with lines of at most 512 bytes.

Any other keyword, a malformed number, or any rule broken refuses the whole file, with the line number; nothing of a refused file is used. The font's name is its `FAMILY_NAME` property, and its `COPYRIGHT` and `NOTICE` properties are kept with it.
*Verification:* Tests of a valid font and of each rule broken; a fuzz target for the reader (REQ-TST-040).

**REQ-FNT-003 — Where fonts come from.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
A font may be loaded from any `http` or `https` address, or from a local file the user chooses, and only when the user, the channel (REQ-FNT-006) or the page (REQ-FNT-007) names it. Nothing else shall cause a font to be fetched. A font shall be fetched by a confined fetcher process (REQ-SEC-026) under the network rules of a channel's files: the local-network protection of REQ-SEC-006, the plain-HTTP marker of REQ-SEC-005, and no downgrade (REQ-SEC-007). A font at an address other than the channel's own origin, named by the channel or the page, is fetched only with its SHA-256, without cookies or `Referer`, and only while the viewer allows fonts from other sites (REQ-WEB-014).

Fonts and library indexes shall be read only in a confined process: the tab's fetcher, or a font helper under the same confinement. The interface process shall never parse a font. It receives the font's bytes, whose SHA-256 it computes itself, and a glyph table of at most 1,024 entries, each a code point and ten 12-bit rows, whose shape it checks (`docs/font-security.md` section 3.1). When a font is named with a SHA-256, a file with any other checksum shall be refused.
*Verification:* Tests that a font named by each of the three is fetched, that nothing else fetches one, and that a wrong checksum is refused.

**REQ-FNT-004 — Font libraries.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
A font library is a folder, in the form of a channel's folder (REQ-WEB-023), holding an index named `fonts.txt`. The index has one entry per line, fields separated by one tab, and comments and empty lines as `contents.txt` has (REQ-WEB-025):

| Kind | Fields after the kind |
|---|---|
| `f` | the font: a file name in the library's folder or an address; its title, 1 to 32 characters; its SHA-256 as 64 hexadecimal digits |

The index is at most 64 kilobytes and 1,000 lines; a line of an unknown kind is ignored, a malformed line is ignored and counted. The user adds a library by its address, and the browser lists its fonts by title for the user to choose; a font is fetched only when chosen.
*Verification:* Tests of a library with fonts in its folder and elsewhere, of each fault, and that listing a library fetches no font.

**REQ-FNT-005 — Which font draws a page.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
The font drawing a page's rows 1 to 24 shall be the first of: the page's font (REQ-FNT-007), the channel's font (REQ-FNT-006), the user's chosen font, and Bedstead. When the user has set "always use my font", the user's chosen font, or Bedstead if none is chosen, shall be used whatever the channel or page names. Every character the font in use lacks shall come from the user's chosen font, then from Bedstead, then the placeholder (REQ-CHR-004). The receiver's own drawing shall always use Bedstead (REQ-FNT-001). The window's status line shall name the font drawing the page, and whether the page, the channel or the user chose it.
*Verification:* Tests of each step of the order, of the user's override, of the fallback for a missing character, and that row 0 and the tuning screen are drawn in Bedstead whatever font a page names.

**REQ-FNT-006 — A channel's font.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
`channel.json` (REQ-WEB-024) may have a member `font`, an object with `address`, a file name in the channel's folder or an address, and `sha256`, 64 hexadecimal digits; both are required, so a font at another address always carries its checksum (REQ-WEB-014). The font is fetched when the channel opens, after its settings and bundle, and the channel's pages are drawn in Bedstead until it arrives (REQ-FNT-009).
*Verification:* Tests of a channel with a font in its folder, one with a font elsewhere, and one whose font fails its checksum.

**REQ-FNT-007 — A page's font.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
A TTI page may name its font with a line `TF,address,sha256`, applying to the page or sub-page of the most recent `PN` line: the address of a font file, absolute or relative to the page's own address, and its SHA-256. Other TTI readers ignore the line as an unknown command. A page has at most one `TF` line; a second, a malformed line or a checksum that is not 64 hexadecimal digits makes the page's font null, with a diagnostic, and the page is still shown. A page from T42 packets or a broadcast cannot name a font. The TTI writer keeps the line, and the page checker (REQ-CLI-006) reports a malformed one as an error.
*Verification:* Reader, writer and checker tests; tests that a page's font draws that page only.

**REQ-FNT-008 — Drawing a loaded font.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
A loaded font's character shall be drawn as its 12 × 10 bitmap with each line repeated in the 12 × 20 cell, without character rounding, and enlarged for double height, double width and double size as a mode 0 DRCS character is (REQ-RND-038). Foreground and background colours, flash, conceal, boxing, underline and the other attributes apply as to any character. The glyphs shall reach a decoder process as a bounded message of the pipe, at most one font per message, checked again on arrival.
*Verification:* Golden rasters of a test font at every size; tests that a font message beyond its bounds is refused.

**REQ-FNT-009 — The font cache.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS.
Fonts shall be kept in the profile directory (REQ-BRW-006), written only by the interface process, each written whole through a temporary file and renamed:
- **By content.** Each font is stored once as its original bytes in a file named by its SHA-256 alone, never by anything taken from its address.
- **Kept apart.** Fonts named by a channel or a page are kept per channel, and used only for that channel; fonts the user chose and the fonts of libraries the user added are shared.
- **Bounded.** At most 4 megabytes in all and 256 kilobytes per font. When a new font would exceed the total, the least recently used fonts are removed first, except those the user has pinned.
- **Fresh.** A font named with a SHA-256 is used from the cache with no request. A font the user named by address alone is checked again at most once a day with a conditional request (REQ-WEB-004); the cached copy stays in use when the check fails.
- **Checked on every load.** Whenever a font is loaded from the cache, the interface recomputes its SHA-256, and a confined process reads it again against REQ-FNT-002; a file that fails is deleted and fetched again.
- **Visible.** The settings list every cached font with its name, address, licence notice and size, and let the user pin a font, remove one, or clear all.

While a font is being fetched, the page is drawn in the font that would apply without it, then drawn again when it arrives. A font that cannot be fetched leaves that font in place, with a message in the status line. Cached fonts work offline.
*Verification:* Tests of reuse without a request, of keeping channels apart, of eviction and pinning, of a file changed on disk, and of the daily check.

**REQ-FNT-010 — The font converter.**
`ttx font convert INPUT OUTPUT` shall write a font in the form of REQ-FNT-002 from a BDF of another cell size, offline, only where the conversion is exact and reversible:
- a 5 × 10 or 6 × 10 cell: every pixel made two pixels wide, a 5-pixel cell placed in the left of the 6;
- a 5 × 9 or 6 × 9 cell: the same, with one blank line added at the top, where the SAA5050 leaves it;
- a 12 × 20 cell in which every pair of lines, 1 and 2, 3 and 4 and so on, is identical: every second line kept.

Any other cell, any glyph larger than its cell, or any 12 × 20 glyph whose line pairs differ shall be refused, naming the first character that fails. When the input's `CHARSET_REGISTRY` is not ISO10646, the converter shall take the Unicode code point of each `ENCODING` from a mapping file given with `--map`, one `code<TAB>U+XXXX` line each, and refuse a character the map lacks. `FAMILY_NAME`, `COPYRIGHT` and `NOTICE` are carried over. The converter never reads from the network and runs under the sandbox of REQ-SEC-017.
*Verification:* Tests of each exact conversion and its reversal, of each refusal, and that the output passes REQ-FNT-002.

---

# Part G — Out of Scope

The following are not part of this implementation. Each could be added later as a new set of requirements.

1. **A layout algorithm for proportional spacing.** Presentation Levels 2.5 and 3.5 were brought into scope on 10 October 2026 (decision E.40). The one part of them that stays out is a layout for the proportional spacing of Level 3.5. The standard defines no spacing algorithm, so proportional spacing is recorded in each cell and drawn at the fixed pitch (decision E.50).
2. **The Table Of Pages navigation scheme.** Its full specification (Institut für Rundfunktechnik guideline 8 R 5) is not available. The page types can be recognised by page number (1F0) and by page function in packet X/28, and such pages shall not be displayed as text (REQ-ACQ-014).
3. **Programme Delivery Control decoding** (EN 300 231). The data is stored raw (REQ-SVC-003), and its triplets are skipped for display (REQ-L15-007).
4. **525-line teletext.** EN 300 706 does not cover it. ITU-R BT.653 defines a 525-line System B with a different bit rate, timing, level and row length, and no country is listed as using it.
5. **Data broadcasting** (EN 300 708) and independent data services (packet 31).
6. **The page checksum as a reception requirement.** It is computed (REQ-ECC-007) but not required for display.
7. **Packet X/25 display** (key-word search labels). It is stored but not displayed.
8. **Serial digital interface carriage** (SMPTE RDD 8, also known as OP-47, and SMPTE ST 2031). **FAB, Cebra and ivtv formats.** These are listed as later options in REQ-INP-021.
9. **Viewdata.** Its screen format is related, but it is a different service.
10. **Display, video and device hardware.** This covers putting pixels on a screen, display drivers, video signal generation, scan timing, capture devices and any specific target hardware (for example the RP2040 or RP2350). These belong to the port's hardware layer, which the library reaches only through the platform interface (REQ-ARCH-009 and REQ-ARCH-010).
11. **Teletext generation as a product.** The page encoder exists for testing and conversion. A broadcast inserter, with carousel timing, magazine scheduling and the 20 millisecond erase interval, is not in scope.
12. **Interpretation of the Magazine Inventory Page** (page mFD, clause 11.3). It is optional, and the Magazine Organization Table and packets X/27/4 and X/27/5 are enough to find every object and DRCS page (`research/notes-level25-objects.md` section 10). It is stored like other enhancement pages, and not shown as text. Appendix D.2 asks the owner to confirm this.

---

# Appendix A — Glossary

| Term | Meaning |
|---|---|
| Active position | The row and column at which a Level 1.5 enhancement triplet acts. |
| Blast-through | Display of alphanumeric characters from columns 4 and 5 while in mosaic mode. |
| Chapter | A page memory in the SAA5243 and SAA5246A. |
| Clock run-in | The alternating 16-bit pattern at the start of each teletext line used to synchronise the bit clock. |
| Designation code | Byte 6 of packets 26 to 31, which distinguishes up to 16 packets of the same number. |
| DVB | Digital Video Broadcasting, the family of digital television standards that carries teletext in EN 300 472. |
| Fastext, FLOF | The editorial linking scheme using packet X/27/0 and coloured keys ("Full Level One Features", EN 300 706 Annex H). |
| Framing code | The byte 11100100 (transmission order) that identifies a teletext line and aligns bytes. |
| G0, G1, G2, G3 | The primary alphanumeric set, the mosaic set, the supplementary set and the smoothed-mosaic and line-drawing set. |
| Hamming 8/4, Hamming 24/18 | The error-correcting codes of EN 300 706 clauses 8.2 and 8.3. |
| Held mosaic | The mosaic character shown in place of attribute cells while Hold Mosaics is active. |
| Magazine | A group of up to 256 page numbers sharing the first digit of the page number, 1 to 8. |
| Magazine and Row Address Group | The two Hamming-protected bytes giving a packet's magazine and packet number. |
| Mode | The 5-bit field of a triplet that selects its function. |
| National option | One of the thirteen variants of the Latin G0 set's thirteen variable positions. |
| Packet | One teletext line's data: 45 bytes on air, or 42 after the framing code. |
| Page header | Packet X/0: the page address, control bits and row 0 text. |
| Parallel mode, serial mode | The two ways of interleaving magazines, selected by bit C11. |
| PES | Packetised Elementary Stream, the container for teletext in digital television. |
| Sub-code, sub-page | The 13-bit number that distinguishes versions of a page. |
| T42 | A file of 42-byte teletext packets in transmission bit order. |
| Triplet | Three bytes protected by Hamming 24/18, carrying 18 data bits. |
| TTI | The MRG Systems text format for teletext pages. |
| VBI | The vertical blanking interval: the television lines outside the visible picture that carry teletext. |
| Crate | A Rust package: the unit of compilation and of dependency. |
| `no_std` | Rust code that uses only the core language library, with no operating system, so that it runs on bare microcontrollers. |
| Sans-IO | A way of writing protocol code as state machines that are fed bytes and time, and return bytes and events, without performing any input or output themselves. |
| Target triple | The name Rust gives a compilation target, such as `thumbv6m-none-eabi`: processor, vendor and operating system. |
| Miri | The Rust interpreter that detects undefined behaviour, used here to test on 32-bit and big-endian targets. |
| HTTP, TLS | The web's request protocol, and the encryption layer beneath HTTPS and secure WebSockets. |
| WebSocket | A two-way message connection opened from an HTTP request. |
| Server-sent events | A one-way stream of text events from a web server over HTTP. |
| Cookie | A small value a web server asks the client to store and send back, used here for sessions. |
| JSON | JavaScript Object Notation, the text format of the service descriptor. |
| Origin | The scheme, host and port of a web address: the unit of web security (REQ-WEB-015). |
| User agent | The web term for a browser or other client acting for the user. neoteletext is a deliberately minimal one (REQ-WEB-014). |
| Colour Map, CLUT | The 32 colours of Levels 2.5 and 3.5, in four colour lookup tables (CLUTs) of eight entries each (Table 30). A colour is named by its index, 8 × CLUT + entry. |
| DCLUT | A DRCS colour lookup table: four or sixteen Colour Map indices used by multi-colour DRCS characters (Table 8). |
| DRCS, GDRCS | Dynamically Redefinable Characters: characters downloaded as dot patterns on DRCS pages. A Global DRCS page serves the whole magazine. |
| PTU | Pattern Transfer Unit: 20 bytes of a DRCS page, holding one bit plane of a 12 by 10 character, or a whole 6 by 5 by 4 character (clause 14.1). |
| Object | A reusable sequence of enhancement triplets, Active, Adaptive or Passive, defined locally in packets X/26 or on an object page, and invoked by a page (clause 13). |
| POP, GPOP | Public Object Page and Global Public Object Page: pages that define objects for a set of pages, or for every page of a magazine. |
| MOT | Magazine Organization Table: page mFE of each magazine, linking each page to its object and DRCS pages (clause 10.6). |
| MIP | Magazine Inventory Page: page mFD, an optional list of the page types of a magazine (clause 11.3). |
| Non-spacing attribute | An attribute set by an enhancement triplet. It occupies no character cell, unlike the spacing attributes of Level 1. |
| Origin Modifier | A triplet that offsets the position of the next object invoked, which is how objects reach the side panels. |
| Side panel | Up to 16 extra columns to the left or right of the 40-column page at Levels 2.5 and 3.5 (clause 12.5). |
| Presentation level | One of Levels 1, 1.5, 2.5 and 3.5 of EN 300 706 clause 12; a configuration setting of this receiver (REQ-PRF-006). |
| Channel | A folder on a web server that serves teletext pages, identified by its origin and folder path (REQ-WEB-018, decision E.70). What earlier documents call a site. |
| Channel folder | The address of a channel: an `http` or `https` address whose path ends in `/` (REQ-WEB-023). |
| Front door, bundle | A channel's `index.t42`, which may hold any number of pages (REQ-WEB-019). |
| Page file | The file of one page in its channel's folder, named by its number: `140.tti` or `140.t42` (REQ-WEB-019). |
| Contents file | A channel's `contents.txt`: its sections, pages and linked channels, drawn by the receiver as an index (REQ-WEB-025, REQ-BRW-009). |
| Font | Bedstead, the receiver's bundled font, or a BDF file of 12 × 10 cells loaded when the user, the channel or the page names it (area FNT). |
| Font library | A folder with an index, `fonts.txt`, listing fonts with their checksums (REQ-FNT-004). |
| BDF | The Glyph Bitmap Distribution Format, a text format for bitmap fonts; the form of every loaded font (REQ-FNT-002). |
| Alias | An entry of `channel.json` that leads one page number to another channel on every page of the channel (REQ-WEB-020). |
| Discovery | Finding the channel an ordinary web address announces, from a pointer in its answer (REQ-WEB-028, decision E.73). |
| Pointer | A link of type `alternate` to a channel or a page file, with a teletext media type, given in a `Link` header or a `<link>` element (REQ-WEB-029). |
| Tuning screen | The receiver-drawn screen shown before anything from another channel, which the viewer confirms (REQ-NAV-014, decision E.65). |
| Page catching | Moving a cursor between the links on a page and following one, as some Philips televisions did with page numbers (REQ-NAV-015). |
| Helper process | A process the browser starts for a tab: a fetcher, which holds only the network, or a decoder, which holds nothing (REQ-SEC-026). |

---

# Appendix B — Source documents

**Earlier teletext specifications** (in `~/devel/jellytext/teletextspecifications/`, see Part A.12a): the 1974 and 1976 BBC, IBA and BREMA specifications; "World System Teletext" (1990); EBU SPB 492 (1992); and a United Kingdom 625-line television specification. They are cited by year and PDF page.

**Standards** (in `specifications/`):
- ETSI EN 300 706 V1.2.1 (2003-04), *Enhanced Teletext specification*. This is the primary source for Parts A, B and E.
- ETSI ETS 300 706 (1997), the earlier edition, used for cross-checking.
- ETSI EN 300 472 V1.3.1 and DVB BlueBook A041, *Teletext in DVB bitstreams*.
- ITU-R BT.653-3, *Teletext systems*.
- ETSI ETS 300 708, *Data transmission within Teletext*.

**Unicode documents:**
- Unicode document L2/19-025, "Proposal to add characters from legacy computers and teletext to the UCS" (2019), and its mapping file `TELTXTG3.TXT`. They give the Unicode characters, and the sextant geometry, of the G3 set (decision E.50 and REQ-CHR-011).

**External standards consulted during research** (not held locally):
- ETSI EN 301 775, carriage of vertical blanking interval data in DVB.
- ETSI EN 300 468, DVB service information descriptors.
- The Linux kernel documentation for sliced and raw vertical blanking interval capture.
- The MRG Systems TTI format document.
- SMPTE RDD 8.

**Datasheets:**
- Signetics SAA5050/SAA5055 datasheet (`philips-saa5050/`).
- Philips 1979 databook, *ICs for Digital Systems in Radio and TV* (`databooks/`).
- Philips SAA5246A product specification, January 1993 (`philips-saa5246a/`).
- Philips SAA5243, SAA5281, SAA5231; Siemens SDA 5231; ITT TPU 2735, for the slicer and for lineage.

**Corrections to `DATASHEET-INDEX.md`**, found during this work. The page numbers are PDF page numbers in the 1979 databook:

| Part | Index says | Correct |
|---|---|---|
| SAA5020 | | Pages 136 to 144 |
| SAA5030 | | Pages 146 to 154 |
| SAA5040 | Page 152 | Pages 156 to 167 |
| SAA5041 | | Pages 168 to 179 |
| SAA5050 | Includes page 141 | Pages 180 to 191. Page 141 belongs to the SAA5020. |
| SAA5051 | | Page 192 |
| SAA5052 | | Pages 194 to 205 |
| SAB1009B | Page 208 | Starts at page 206 |

**Sources of real teletext data** (decision E.28 and REQ-TST-022; checked 1 October 2026):
- the FFmpeg sample archive, `https://samples.ffmpeg.org/ffmpeg-bugs/trac/ticket2086/`, for off-air DVB transport streams with teletext;
- the Teletext Archaeologist archive, `https://archive.teletextarchaeologist.org`, for recovered services in T42 form;
- the zxnet online teletext viewer, `https://zxnet.co.uk/teletext/viewer/`, and its WebSocket feeds (REQ-INP-022);
- the vhs-teletext sample captures, for raw sampled video.

**Community resources used as test oracles:**
- zvbi;
- vhs-teletext;
- vbit2 and raspi-teletext;
- QTeletextMaker;
- the edit.tf and zxnet editors;
- the Teletext Wiki at teletext.wiki.zxnet.co.uk.

Where these are GPL-licensed, they are used only as external programs and data sources, never copied.

---

# Appendix C — TTI Form Extension, version 3 (normative)

This appendix defines interactive forms on teletext pages. It is version 3 of the TTI Form System written for the jellytext project. Version 2.0 is kept in `research/jellytext-tti-form-system-spec-v2.md`, and the changes are explained in decision E.31. Where this appendix and version 2.0 differ, this appendix applies.

## C.1 Principles

1. **Visual first.** A form is drawn exactly as the viewer will see it, using ordinary TTI `OL` rows.
2. **Stated positions.** Each field definition states the row, column and width it occupies. The drawn placeholder characters are only decoration.
3. **Definitions next to rows.** A field definition follows the `OL` row it belongs to.
4. **Graceful degradation.** Decoders that do not understand the form commands ignore them and show a readable, non-interactive page.
5. **Ordinary web submission.** Forms submit by HTTP to an address on the page's own origin, and the viewer is then taken to a success or error page.

## C.2 Commands

All commands are TTI lines of the form `XX,arguments`, where XX is two letters. An unknown command is ignored.

| Command | Syntax | Meaning |
|---|---|---|
| Field definition | `FD,type,name,column,width,parameters…` | Defines a field on the row of the most recent `OL` line. `column` is 0 to 39. `width` is the number of columns the field occupies, and the field must fit within the row. Parameters depend on the type (C.3). |
| Option | `OPTION,value,display_text,selected` | One choice of the most recent SELECT or RADIO field. `selected` is `Y` or empty. |
| Form submission | `FS,method,address` | The method is `POST` or `GET`. The address is absolute, or relative to the page's own address. One per page. |
| Form pages | `FP,success_page,error_page` | Page numbers in `mpp` form, shown after submission (REQ-WEB-011). |
| Form token | `FT,token` | An opaque token issued by the server for this page and session, returned with the submission as `ttx_token`. |
| Header | `FH,…` | **Withdrawn.** Ignored with a diagnostic (decision E.31 item 2). |

A field definition applies to the row of the most recent `OL` line, even if other form commands come in between.

For a multi-row field (TEXTAREA), the field starts on that row and continues down the following rows at the same column and width.

Fields on the same page shall not overlap. A page with overlapping fields shall be displayed but not made interactive, and a diagnostic shall be given.

## C.3 Field types

| Type | Parameters after `column,width` | Notes |
|---|---|---|
| TEXT | `maxlen,required,pattern,default` | `pattern` is a simple pattern: `9` for a digit, `A` for a letter, `*` for any character. |
| EMAIL | `maxlen,required,default` | Must contain one "@" with text on either side. |
| TEL | `maxlen,required,pattern,default` | Digits, spaces and "+" only, unless a pattern says otherwise. |
| NUMBER | `min,max,step,required,default` | Whole numbers only. |
| DATE | `min_date,max_date,required,default` | Year-month-day. |
| TIME | `min_time,max_time,required,default` | Hours and minutes. |
| SELECT | `required` | Choices from the following `OPTION` lines, shown one at a time within the field. |
| RADIO | `required` | One choice from the following `OPTION` lines. `OPTION` display text is not drawn by the decoder, because the page row already shows the labels. `column` is the first choice's marker; each further choice's marker position follows as a `column` given in its `OPTION` line after `selected`. |
| CHECKBOX | `value,required,checked` | One marker cell. `checked` is `Y` or empty. |
| TEXTAREA | `rows,maxlen,required,default` | Occupies `rows` rows from the field's row down. |
| SUBMIT | `label,action` | The whole field area is the button. `action` is sent as `ttx_action`. |
| RESET | `label` | Restores all fields to their defaults. |

`required` is `Y` or `N`.

## C.4 Drawing and characters

- **Placeholders are decoration.** The author draws each field's placeholder in the `OL` row.
- **Recommended placeholder characters.** Because the parser never uses them, placeholders may be any characters. Authors should still use characters that look the same in every character set the service uses, such as the full stop for text fields and "(" "*" ")" for markers. They should not use the underscore or square brackets, which are national option positions (decision E.31 item 1).
- **How the decoder draws a field.** An interactive decoder draws the field's current value over the placeholder:
  - in the attributes in force at those cells;
  - highlighted for the field that has focus;
  - with a cursor where text is being entered.

  A value longer than the field scrolls within it.
- **What can be entered.** Characters shall be those the page's G0 set can display. Other characters are refused at entry (decision E.31 item 5).

## C.5 Validation

Before submission, the decoder shall check:
- that required fields are filled;
- lengths against `maxlen`;
- patterns;
- number ranges and steps;
- date and time ranges;
- the EMAIL and TEL rules.

The server remains the authority, and shall check everything again.

## C.6 Degradation and compatibility

- A decoder without form support shows the `OL` rows and ignores every other command in this appendix.
- A version 2.0 form, which has no column or width, shall be shown as an ordinary page and not made interactive, with a diagnostic that it needs converting.
- A conversion tool should convert version 2.0 forms to version 3 by finding the placeholders and writing their columns in. It should warn where a placeholder uses national option characters.
- **Broadcast.** When a form page is encoded for broadcast (REQ-INP-019), the form commands are not transmitted, because a broadcast television has no way to send anything back. A form page should therefore say in its own rows how a viewer without neoteletext can respond, for example by a telephone number or a web address. The page checker (REQ-CLI-006) shall warn when a form page has no such text. It detects this only approximately, and the warning can be marked as considered (REQ-CLI-006).

## C.7 Example

```
PN,40001
DE,Contact form
PS,8000
FT,5f2c9a17e0
OL,1,CONTACT US
OL,3,Name:  ...............................
FD,TEXT,name,7,31,50,Y,,
OL,4,Email: ...............................
FD,EMAIL,email,7,31,100,Y,
OL,6,Urgency: (*) Low  ( ) Medium  ( ) High
FD,RADIO,urgency,10,1,Y
OPTION,low,Low,Y
OPTION,medium,Medium,,19
OPTION,high,High,,31
OL,8,  < SEND MESSAGE >
FD,SUBMIT,send,2,16,Send message,send_contact
FS,POST,/contact
FP,40101,40102
```

On a decoder without form support, this page shows the labels and dotted placeholders. On neoteletext it is a working form. The session comes from the cookie, the token from `FT`, and the submission goes to `/contact` on the page's own origin.

---

# Appendix D — Open questions and research record

This appendix lists the questions raised while writing this document. For each, it says whether research has answered it and where the evidence is. The research reports are in `research/`.

## D.1 Answered by research (1 October 2026)

| Question | Answer | Evidence | Recorded in |
|---|---|---|---|
| Which way round are the national option bits C12, C13 and C14? | Index = 4×C12 + 2×C13 + C14 | Decoding the headers of French and Italian broadcast recordings, where only this order gives the right language. zvbi uses the same order. SPB 492 makes C12 the most significant bit. | E.21, REQ-TST-012 |
| What is the exact SAA5050 character rounding rule? | The candidate rule is exactly the MAME emulator's, and jsbeeb, beebjit, Bedstead and the decapsulated-chip analysis are equivalent. No rounding across the cell edge. | `research/answers-saa5050-emulators.md` | E.13 |
| What are the separated mosaic dimensions? | Leftmost dot column and bottom line of each block blanked. Blocks 3/4/3 lines by 3/3 dots. | As above | E.14 |
| What is the real flash timing? | 48 fields shown, 16 hidden (0.78 hertz, 3 to 1) | As above | E.12 |
| Does a held mosaic keep its stored separated or contiguous form? | Yes, on the real chip and in the 1976 specification | As above; `research/answers-historic-specifications.md` | E.16 |
| Is one Start Box enough? | zvbi and vhs-teletext require two, as the standard's protection note intends | `research/answers-saa5050-emulators.md` | E.17 (default changed to two) |
| What do the X/28/1 and M/29/1 character set codes mean? | EBU SPB 492 Figure 14 | `research/answers-historic-specifications.md` | E.4 |
| Where does Annex A.3's row-address rule come from? | EBU SPB 492 clause 14.6.4 | As above | E.3 |
| Which Unicode characters represent mosaics? | Sextants U+1FB00 to U+1FB3B, plus the half and full blocks; separated sextants U+1CE51 to U+1CE8F | `research/answers-saa5050-emulators.md`; Bedstead | REQ-CHR-005 |
| What are four hard-to-read national option glyphs? | Ƶ, ł, the em dash and ‖ | zvbi and Bedstead | REQ-CHR-002 |
| Which Rust targets, tiers and toolchains apply? | See REQ-GEN-015. Xtensa needs Espressif's pinned fork. | `research/answers-rust-targets.md` | REQ-GEN-001, REQ-GEN-015 and REQ-GEN-016 |

## D.1a Settled for Levels 2.5 and 3.5 (10 October 2026)

The research notes `research/notes-level25-enhancement.md`, `research/notes-level25-objects.md` and `research/notes-level25-drcs.md` list the places where the standard is silent or contradicts itself at these levels. The owner's rulings of 10 October 2026 settle them in decisions E.40 to E.56. The questions that matter most are these:

| Question | Answer | Evidence | Recorded in |
|---|---|---|---|
| What are the default colours of CLUT 1 entries 5 and 6? | Half magenta (7, 0, 7) and half cyan (0, 7, 7), as Table 30 gives. zvbi 0.2.44 differs, so comparisons with it use colour indices. | The page image of Table 30, page 89; a zvbi run on `oldvga` | E.43, REQ-TST-023 |
| Which rows does the row count of a Font Style triplet reach? | The next R rows, from the same column to the end of each row | A zvbi run on `oldvga` | E.50, REQ-L25-028 |
| How does zvbi report G3 characters? | As the private value 0xEF00 plus the code | A zvbi run on `oldvga` | REQ-TST-023 |
| Does the strict row rule of clause 12.3.2 apply at Levels 2.5 and 3.5? | Yes. Object triplets carry object numbers in their address field. | Clause 12.3.2 and Table 28 | E.3 (amended), E.46 |
| Which row colour applies when only default colours are sent? | The default row colour; the default screen colour does not replace it | Annex A.5 and Table 4 | E.45 |
| How wide are the side panels, and is the display always 56 columns? | As the table of decision E.49, and 40 columns plus the panel widths | Clause 12.5 and Table 4 | E.49 |
| What do the flash phases and rates mean? | A one-second cycle of twelve units | Table 29 gives only the rates | E.48 |
| How are the G3 characters drawn? | From the sextant geometry of Unicode document L2/19-025, which matched the standard's figure on 95 to 100 percent of pixels | `research/notes-level25-drcs.md` section 9.3 | E.50, REQ-RND-037 |
| Which page links decide the object and DRCS pages? | X/27/4 and X/27/5 before the Magazine Organization Table, whatever its flag says | Clauses 10.5.1.4 and 10.5.2.2 | E.52 |
| What do objects do with damaged, missing or out-of-range data? | The fault is confined to the smallest unit, and the Level 1 page always shows | The owner's principle | E.41, E.51 |

## D.2 Still open

| Question | Why it matters | How it will be settled |
|---|---|---|
| **What is the neoteletext product?** Who writes pages, what the server side is, and who the users are. | This document specifies the receiver and library only. The web-native design implies servers and an authoring workflow that are not specified. | A short product brief, written by the project owner, before milestone M2. |
| Should the receiver read the live page feeds of SRG SSR (EP1 inside JSON) and NOS (Level 1 rows)? | They are the only live sources of a real broadcaster's teletext bytes found (decision E.28, amended 10 October 2026). They are whole pages, not packets, and the NOS form has already converted accented letters. | A decision in Part E, including the broadcasters' terms of use, before any reader is written. |
| Where can raw teletext from German broadcasters, ZDF in particular, be obtained? | German public broadcasters are the main remaining users of Level 2.5, so their data best tests what lies beyond Level 1.5. This matters more now that Levels 2.5 and 3.5 are in scope (decision E.40). Their websites publish only HTML, and ZDF's runs a bot check that must not be circumvented. Archives of the web pages (defgsus `teletext-archive-unicode` and its successors, from 2022) hold Unicode text, not teletext bytes. | A recording from a DVB receiver in Europe tuned to Astra 19.2°E or German DVB-T2. |
| Does double-height rounding alternate its neighbour on every line (the datasheet's wording), or repeat each rounded line (all the emulators)? | Visible only on double-height diagonals | Comparison with photographs of a real SAA5050 (REQ-TST-030). E.13 follows the emulators until then. |
| What are G2 diacritical marks 4/9 and 4/C exactly? | Rarely used; the references disagree | A real transmission that uses them (REQ-CHR-008). |
| Is the zxnet live feed protocol stable? | It is undocumented and could change | The tolerant reader of REQ-INP-022, plus the recorded session of REQ-TST-063. The site's operator could also be asked. |
| What noise performance should the slicer reach? | The 20 decibel target of REQ-SLC-013 is an estimate | Measurement in milestone M4 |
| How much flash does the core really need on a Cortex-M0? | The 64 kilobyte budget rests on an estimate | The size check of REQ-GEN-009, from milestone M1 |
| Will RFC 6265bis change before it is published? | REQ-WEB-006 follows the draft of 30 September 2026 | Recheck at each milestone's security review, and when it is published as an RFC (decision E.35). |
| Which Xtensa toolchain versions build correctly? | Espressif's fork 1.94 and 1.95 fail release builds (esp-rs/rust#277) | The pinned toolchain of REQ-GEN-001, re-tested when the issue closes. |
| How does the SAA5246A behave where its datasheet is silent? For example, parity errors, serial mode and erase. | It limits how exactly the SAA5246A profile can be reproduced | Decisions E.6 to E.8 define the behaviour. A real SAA5246A-based television, observed with a generated stream, could settle the rest. |
| Why does zvbi not load a synthetic DRCS page, and why does it then drop every X/26 enhancement of the page? | Until this is understood, zvbi cannot be the oracle for DRCS cells (REQ-TST-023) | The investigation that REQ-TST-023 requires, or a real capture with DRCS from the corpus |
| Is the magazine of a Magazine Organization Table link absolute or relative? | Decision E.52 reads it as absolute; a wrong reading sends every link to the wrong magazine | The zvbi comparison of REQ-TST-023, and real Magazine Organization Tables (REQ-TST-025) |
| Is the bit order of the mode identification codes of X/28/3, and the pixel order of DRCS mode 3, as decision E.54 reads them? | No oracle has confirmed either | A real DRCS page of Level 3.5, or zvbi once it loads DRCS pages |
| Should an invocation be skipped when the object number bits of its definition differ from its own? | Decision E.51 does not compare them, because the standard does not require it. Comparing would reject a misplaced pointer. | An owner decision |
| Should the Magazine Inventory Page be interpreted? | It could guide which enhancement pages to keep when the store is full (REQ-ACQ-019). Part G item 12 leaves it uninterpreted. | An owner decision |
| Does Level 3.5 fit the flash budget of REQ-GEN-009 on a Cortex-M0? | The enhancement processor, the G3 glyph data and the larger grid all add to the 64 kilobyte budget | The size check of REQ-GEN-009, from milestone M6 |
| Are the decisions taken while building the macOS browser right? | Decisions E.63 to E.69 include choices marked *(build)*, taken on 12 October 2026 without the owner: among them `sandbox-exec` instead of XPC services, the WebAssembly wall deferred, `q` not quitting, and the BSL-1.0 licence exception. | The owner's review (issue #162); each change is recorded as an amendment in Part E. |
| The browser design's remaining decisions | `docs/desktop-browser.md` section 15 decisions 3, 4, 6, 7, 8, 12, 13 and 15 to 18 are recommendations: libraries, the input router, process isolation on macOS, Windows test machines, signing accounts, the WebAssembly wall, iOS as one process, the interface to the phone shells, minimum system versions, an outside audit, and the phone tools. | The owner's decisions (issue #150), recorded in Part E. |
| Should a TTI bundle (`index.tti`) be allowed beside `index.t42`? | Decision E.70 keeps one bundle format to keep one request; authors of TTI pages must publish page files. | Experience with TTI-authored channels. |
| Should declared sister channels on one origin skip the tuning screen? | Decision E.70 shows it between every two channels; a broadcaster with several channels on one server would see it often. | A decision in Part E, if a broadcaster asks. |
| Should the tuning screen flag host names that imitate others? | The concept (section 6) says names that imitate other domains are flagged; nothing does this yet. It needs a source of confusable characters and of well-known names. | A decision in Part E before it is built. |
| Do the Teefax and NMS Ceefax feeds carry packets X/26 and X/28, objects or DRCS pages? | REQ-TST-025 relies on them for real data of these levels, and the feeds send only requested pages (REQ-INP-022) | Captures made for REQ-TST-025 |

---

# Appendix E — TTI Link Extension (normative)

This appendix defines how a TTI page declares its links (decisions E.36 and E.64). It was amended on 12 October 2026 to allow addresses as targets, and to add the `LK` and `LT` lines.

## E.1 The link command

```
LN,key,target,label
```

- **`key`** is `R`, `G`, `Y` or `C` for the red, green, yellow and cyan keys, or `I` for Index. Each key may appear at most once on a page or sub-page.
- **`target`** has one of these forms:
  - `mpp` or `mpp:ssss`: a page, and optionally a sub-page, in the same service, in hexadecimal;
  - `name:mpp` or `name:mpp:ssss`: a page in another service named in the service descriptor (REQ-WEB-002). Names are letters, digits and hyphens;
  - an address: a channel's folder, or a page file in a channel's folder, such as `https://weather.example/300.t42` (decisions E.64 and E.70).

  A target containing `://` is an address; any other target is a page or a named service. An address shall use `http` or `https`, shall be printable ASCII with no spaces or commas, and shall be at most 256 bytes. A target that breaks these rules makes the link invalid. An address that is a page file in the page's own channel folder is a page of the current channel; any other address leads to another channel (REQ-WEB-026).
- **`label`** is the prompt text. It is at most 9 characters for a link in the same service and 8 for a link to another service, using characters the page's G0 set can show. It is ignored for `I`.

`LN` lines apply to the page or sub-page of the most recent `PN` line.

### E.1.1 Page-number links

```
LK,mpp,target
```

Links the page number `mpp`, which the page shows, to `target`, in any of the forms above. While the page is shown, typing that number, or choosing the number printed on the page, follows the link (REQ-NAV-013). A page has at most eight `LK` lines; a ninth is ignored with a diagnostic. Two `LK` lines for the same number, or an invalid number or target, are reported by the page checker as errors (REQ-CLI-006).

### E.1.2 Text links

```
LT,row,column,length,target
```

Links a run of `length` characters starting at `column` on `row` to `target`. `row` is 1 to 23 and the run must lie within the 40 columns; anything else makes the line invalid. A page has at most eight `LT` lines; a ninth is ignored with a diagnostic. The page draws the text itself; the receiver shows it as a link only in page catching or when the viewer asks to see links (REQ-NAV-015).

`LK` and `LT` lines apply to the page or sub-page of the most recent `PN` line, and are kept by the TTI writer.

## E.2 Rules

1. **At most four.** A page has at most four coloured links. A fifth coloured `LN` line, or a repeated key, makes the page's links invalid. The page is still displayed, with no links, and a diagnostic is given.
2. **Unknown services.** A target naming a service not in the descriptor makes that link null.
3. **Older decoders.** Older TTI readers ignore `LN` lines. A page that wants links on such readers may also carry an ordinary `FL` line. When both are present, `LN` wins in neoteletext.
4. **Encoding for broadcast.** When the page encoder turns the page into packets (REQ-INP-019):
   - it writes packet X/27/0 from the links to the same service, with links to other services and to addresses written as null;
   - it writes a row 24 (packet X/24) drawn as REQ-NAV-011 describes, with the "display row 24" flag set. The one difference is that a link to another service is written in white rather than its key's colour. The "»" mark still shows, but the prompt does not look like a key that works: a broadcast television cannot follow it.

   A Fastext television therefore shows the same prompts and follows the same-service links.
5. **Reachable without Fastext.** Televisions made before Fastext (before about 1987) can only follow a link by having the viewer type its page number. Every link to a page in the same service should therefore have its three-digit page number visible somewhere in the page's own rows, for example "Sport 140". The page checker (REQ-CLI-006) shall warn when one is not.
6. **Honest about what cannot work.** A link to another service, or a destination outside teletext, cannot be followed on a broadcast-only television. Authors should give such destinations in a form a viewer can act on without the link: the channel or service name, a short address, or a QR code drawn in mosaic graphics. The page checker shall warn when a page's only route to another service is its link.
7. **Linked numbers are shown.** A page-number link changes what typing that number does on that page only, so the number should appear in the page's text, where the viewer can see it. The page checker shall warn when an `LK` line links a number the page does not show.
8. **No packet form.** Addresses, `LK` lines and `LT` lines cannot be carried in packets. The page encoder writes a coloured link to an address as null, with its prompt in white (rule 4), and leaves `LK` and `LT` lines out.

## E.3 Example

```
PN,10000
OL,1,NEWS INDEX
OL,3,Headlines        ...
LN,R,101,Headlines
LN,G,140,Sport
LN,Y,weather:400,Weather
LN,C,190,TV guide
LN,I,100,
```

Row 24 then shows:
- red "Headlines";
- green "Sport";
- yellow "»Weather", because it leaves this service;
- cyan "TV guide".

On a broadcast Fastext television, the yellow prompt appears in white and its key does nothing, because that link leaves the service. The page checker would also warn unless the page's text gives the page numbers 101, 140 and 190, and some other way to reach the weather service.
