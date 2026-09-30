# tellytxt.com — Software Teletext Receiver — Requirements Specification

**Status:** Draft 2 (consolidated) for review
**Date:** 1 October 2026
**Implementation language:** Rust, edition 2024 (REQ-GEN-001), for ARM, RISC-V, Xtensa (ESP32) and AMD64 targets (REQ-GEN-015)
**Presentation levels in scope:** Level 1.0 and Level 1.5 of the World System Teletext (System B) as defined by ETSI EN 300 706
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

**Where the sources are.** The standards and datasheets cited in this document are not kept in the project directory (`~/devel/tellytxt.com`), because they must not be redistributed (see "Legal basis" below). They are in `~/devel/microcontrollers/teletext/`. Every folder path in this document, such as `specifications/`, `philips-saa5050/` or `DATASHEET-INDEX.md`, is relative to that directory.

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

Level 2.5 and Level 3.5 are out of scope for this project. Level 1.0 does not include:
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

**E.2 — Packet X/26 with designation code 15.**
By default, X/26/15 is processed like designation codes 0 to 14. The "SAA5246A" compatibility profile ignores it.
*Reason:* Clause 10.1 includes it in Level 1.5, but the standard also warns that many existing decoders ignore it, and the SAA5246A does not even store it. Processing it is the more correct default. Ignoring it reproduces period hardware.

**E.3 — Which row-address triplets set the active row.**
By default, the receiver follows Annex A.3: every triplet with an address from 40 to 63 sets the active row, whatever its mode. Address 40 means row 24, addresses 41 to 63 mean rows 1 to 23, and mode 00111 with address 63 means row 0. A "strict" option follows clause 12.3.2 instead. Under that option only modes 00001 (Full Row Colour), 00100 (Set Active Position) and 00111 (Address Display Row 0) set the row.
*Reason:* Annex A.3 is normative and describes how the Level 1.5 decoders that broadcasters tested against actually behave. The rule comes from EBU SPB 492 clause 14.6.4, the predecessor of EN 300 706. There, a row-address triplet of any mode sets the row for the character triplets that follow, which is how those decoders were built. Broadcasters were told to keep their data compatible with it; for example, Annex A.5 asks for Full Screen Colour to be sent with address 41. The two behaviours give the same result on correctly formed transmissions, so the choice only matters for malformed data.

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
By default, codes 0/0 (alpha black), 1/0 (mosaic black), 0/E (double width) and 0/F (double size) are displayed as spaces and change no state. The "SAA5246A" profile treats 0/0 and 1/0 as alpha black and mosaic black, as that chip does.
*Reason:* Table 26 gives them no response at Levels 1 and 1.5.

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

**E.23 — Row 24.**
By default, row 24 is displayed only when the page carries packet X/27/0 with the "display row 24" flag set (Annex H). The "SAA5246A" profile shows row 24 whenever the page carries packet X/24, like that chip's automatic Fastext row.

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
5. **Broadcaster page interfaces are not raw data.** For example, the Finnish broadcaster YLE and the Dutch NOS publish pages as JSON with control codes replaced by keywords or HTML. They are not supported.

Live raw teletext from a real channel therefore needs a digital television receiver tuned to a European broadcast that still carries teletext. Its output is the transport stream format of item 1.

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

**E.30 — The web-native design, and what the jellytext project teaches.**
tellytxt.com shall read pages from ordinary web servers as well as from broadcast data. It does so over:
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
2. **Stateless servers, with the session in a versioned cookie.** jellytext's notes record the idea to "store session data in cookie with versioning". A tellytxt.com client treats cookies as opaque and only stores and returns them. The versioning is a server design rule, given as guidance for page servers (REQ-WEB-012).
3. **Asynchronous throughout.** The jellytext notes say the "application can be fully async by using callbacks". Every network operation shall be non-blocking and event-driven. The decoder is fed bytes as they arrive (REQ-INP-001).
4. **Interactive forms drawn as teletext pages.** jellytext's TTI Form System adds form commands after ordinary TTI rows. Decoders that do not know the commands ignore them and still show a readable page. tellytxt.com adopts it, corrected as decision E.31 describes.
5. **The transport belongs to the port.** Network sockets and encryption are supplied by the port through the platform interface (REQ-ARCH-009). This keeps the library free of any particular network stack, and is consistent with REQ-ARCH-010. jellytext planned to embed a network stack and an encryption library on each microcontroller, which is heavy.
6. **One platform at a time.** jellytext decided to "write for 3 platforms at once" (composite video from an ESP32, HDMI from an RP2350, and a video stream to YouTube) shortly before it was suspended. tellytxt.com keeps all platforms outside the library (REQ-ARCH-010), and streaming rendered pages as video is not part of it.
7. **Geometry settled once.** jellytext's notes changed their minds several times about pixel sizes. tellytxt.com settles this in REQ-RND-002 to REQ-RND-004 and REQ-RND-024.

**E.31 — Corrections made in adopting the TTI Form System.**
jellytext's TTI Form System version 2.0 is kept unchanged in `research/jellytext-tti-form-system-spec-v2.md`. Version 3, in Appendix C, is normative for tellytxt.com. It keeps the core ideas:
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
With web-native sources, tellytxt.com is a browser for a teletext web. Like any browser, it fetches, caches, keeps cookies, follows links, submits forms, applies an origin security model, sandboxes and renders. It can stay very small because teletext removes almost everything that makes browsers large:
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
1. **One written profile.** tellytxt.com's web behaviour is defined as one written "minimal user agent" profile (REQ-WEB-014). It lists what is supported and, as firmly, what is permanently excluded.
2. **One security unit.** The origin is the single security unit, defined once (REQ-WEB-015), as browsers do.
3. **A strict URL parser.** URL parsing, a classic source of browser bugs, is done by one small, strict, fuzzed parser (REQ-WEB-016). It prefers rejecting unusual addresses to being compatible with every form the web allows.
4. **History.** Back and forward history (REQ-NAV-010) is the one basic browser function the teletext model lacked.

**E.35 — Cookies follow current practice, not the 2011 standard.**
The cookie specification of 2011, RFC 6265, is out of date. Its replacement, RFC 6265bis, remains an IETF draft; the latest version checked is dated 30 September 2026. The major browsers already implement it. Newer practice has been added on top since:
- the `__Http-` and `__Host-Http-` name prefixes, shipped in Chrome 140 and Firefox 143 in 2025;
- Device Bound Session Credentials, a W3C design generally available in Chrome on Windows from 2026, which binds a session to a key that cannot leave the device, so stolen cookies quickly become useless;
- partitioned cookies ("CHIPS").

Third-party cookies remain enabled by default in Chrome after Google reversed its plan to remove them in April 2025. Safari, Firefox and Brave block them by default.

tellytxt.com adopts:
- **RFC 6265bis in full**, as REQ-WEB-006 lists: size limits, the 400-day cap, rejection of control characters, name prefixes, SameSite with `Lax` as the default, and the rule that plain HTTP cannot touch `Secure` cookies.
- **The new prefixes.**
- **Rules stricter than any browser** wherever its model allows: host-only cookies, and no third-party context at all.
- **Device Bound Session Credentials as a Phase 2 requirement** (REQ-WEB-017), where the port can keep a key that cannot leave the device.

Because RFC 6265bis is still a draft, REQ-WEB-006 shall be checked against the published RFC when it appears, and at each milestone's security review (REQ-SEC-023).

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
| SEC | Security and sandboxing, which overrides every other area (section F.0.3) |

## F.0.1 Delivery phases

The requirements are delivered in two phases.

**Phase 1** is a complete, correct receiver. It covers every "shall" in this part that has no phase note:
- all the input formats of area INP except those listed below;
- Level 1.0 and Level 1.5 decoding;
- the default profile;
- the text, terminal and raster outputs in the 12 by 20 SAA5050 style;
- the basic software data slicer;
- the web-native module (area WEB), including the live WebSocket feed reader (REQ-INP-022);
- the command-line commands;
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
| REQ-CHR-004, REQ-CHR-007 and REQ-RND-010 | Glyphs for Cyrillic, Greek, Arabic and Hebrew, and for the G3 positions not used at Level 1.5. The code tables and Unicode mappings stay in Phase 1, and missing glyphs are drawn as visible placeholders. |
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

Tests for a Phase 2 feature are delivered with that feature.

## F.0.2 Milestones within Phase 1

Phase 1 is large, roughly 100 to 150 person-days, so it is delivered in milestones. Each milestone ends with working, tested software that someone can use. This follows the lesson of the earlier jellytext project, which stalled before anything worked end to end (decision E.30).

| Milestone | Delivers | Main requirements |
|---|---|---|
| **M1 — First light** | Set up the layers of `ARCHITECTURE.md` and their enforcement (REQ-ARCH-011). Read T42 and TTI files. Decode Level 1.0 and Level 1.5 into the cell grid. Write text, terminal and PNG output. The exhaustive error-code tests, the Level 1 and Level 1.5 tests, and the real-broadcast corpus decoding. | ECC, PKT, ACQ (cache mode), CHR (Latin), L1, L15, FSM-003, FSM-004, FSM-006, FSM-007, RND-016 to RND-024, INP-002, INP-013, CLI-001 (`list`, `text`, `render`), TST-001 to TST-022 |
| **M2 — Web** | Fetch TTI and T42 pages over HTTP with caching, push over WebSocket and server-sent events, cookie sessions, and the zxnet live feed. | WEB-001 to WEB-009, WEB-013 to WEB-016, INP-022 |
| **M3 — Broadcast** | Digital television transport streams, Linux sliced data, and the remaining page formats. | INP-003 to INP-011, INP-014 to INP-020, SVC |
| **M4 — Raw video** | The software data slicer and its synthetic signal generator. | SLC, FSM-002, TST-060, TST-061 |
| **M5 — Forms and viewer** | Interactive forms, form input, navigation and the viewer state machine. | WEB-010 to WEB-012, NAV (including history, REQ-NAV-010), FSM-005 |

M1 is expected to take 40 to 50 person-days. The order of M2 to M5 may be changed; each depends only on M1.

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
- **Allow-list only.** The text and terminal renderers (REQ-RND-008 and REQ-RND-009) shall emit only an allow-list of printable characters.
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
The library crates shall not use operating-system facilities of any kind. The library crates are `tellytxt-core`, `tellytxt-formats`, `tellytxt-slicer`, `tellytxt-web` and `tellytxt-image`, except the file-writing functions of the image writers, which take an already-open writer. They shall not use:
- `std::fs`, `std::net`, `std::process` or `std::env`;
- threads;
- the system clock.

All of these come only through the platform interface (REQ-ARCH-009). The rule shall be enforced by clippy's `disallowed-methods` and `disallowed-types` lists and by a check of each crate's dependencies. Only `tellytxt-cli` and port code may use operating-system facilities.
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
`tellytxt-core` and `tellytxt-formats` shall build for `wasm32-unknown-unknown` and `wasm32-wasip1` (REQ-GEN-015). An example host shall run the decoder inside a WebAssembly runtime with limits on memory and execution time. Hosts can then isolate the decoder completely, and C programs can use it without the unsafe C interface.
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

---

Each requirement below gives:
- a statement of what shall be implemented;
- where helpful, the reason and the source;
- the verification method.

---

## F.1 General, language and quality requirements (GEN)

**REQ-GEN-001 — Language and toolchain.**
All code shall be written in Rust, edition 2024.

- **Minimum supported Rust version.** The minimum supported Rust version shall be stated in the workspace manifest. It shall be kept at no more than three stable releases behind the current stable release, and shall be tested in continuous integration. At the time of writing, stable Rust is 1.98, so the minimum shall be no older than 1.95.
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
- the memory required per stored page;
- the flash (code and constant data) size of the core on `thumbv6m-none-eabi`, built with `opt-level = "z"` and link-time optimisation.

Continuous integration shall measure the core's flash size on that target and fail if it exceeds a budget stated in the repository. The budget shall start at 64 kilobytes. The estimate is 15 to 40 kilobytes, of which the character tables are 10 to 20 kilobytes, and the measured figure replaces the estimate.
*Verification:* Test and continuous integration size check.

**REQ-GEN-010 — The Cargo workspace.**
The project shall be a single Cargo workspace with these crates. Each optional crate depends only on crates above it in this table.

| Crate | Contents | Environment |
|---|---|---|
| `tellytxt-core` | Everything listed in REQ-GEN-002 | `no_std`, no allocation |
| `tellytxt-formats` | The streaming readers, writers and page encoder of area INP | `no_std` readers. Writers and file helpers behind an `std` feature. |
| `tellytxt-slicer` | The software data slicer (area SLC) | `no_std`; a `fixed-point` feature (REQ-SLC-015) |
| `tellytxt-web` | The web-native protocols: HTTP, WebSockets, server-sent events, cookies and forms (area WEB), and the live feed reader of REQ-INP-022 | `no_std`, written "sans-IO": it consumes and produces bytes and events, and never performs input or output itself |
| `tellytxt-image` | The pixel-format helpers (REQ-RND-018) and the image writers (REQ-RND-007) | Helpers `no_std`; image writers need `std` |
| `tellytxt-cli` | The command-line program `ttx` (area CLI) | `std` |
| `tellytxt-ffi` | The optional C interface (REQ-GEN-018) | Phase 2 |

A development-tools package (`xtask`) holds project automation that is not part of the library, such as the glyph export, fetching the corpus and checking sizes.

Features shall be additive. Every combination of the `std`, `alloc` and default features of each crate shall build, checked with `cargo hack --feature-powerset`. The mapping of the core modules of REQ-ARCH-001 to Rust modules shall be one module each within `tellytxt-core`.
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
| WebAssembly | `wasm32-unknown-unknown` and `wasm32-wasip1` | Isolated execution inside a WebAssembly runtime, in browsers or other hosts (REQ-SEC-019) | 2 | Build `tellytxt-core` and `tellytxt-formats` |

The rules for these targets are:
- **Which crates build where.** On bare-metal targets, the crates without `std` shall build: `tellytxt-core`, `tellytxt-formats`, `tellytxt-slicer`, `tellytxt-web` and the helpers of `tellytxt-image`. Targets with an operating system shall also build the `std` crates and the command-line program.
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
- **The core.** `tellytxt-core` shall have no dependencies outside the Rust standard distribution, except small `no_std` crates approved in the repository. Examples are `heapless` for fixed-capacity collections and `bitflags`.
- **Other crates.** They may use well-maintained crates where these save real work. Examples:
  - `httparse`, `embedded-websocket` and `serde-json-core` for the sans-IO web protocols;
  - `embedded-io` for the transport traits;
  - `base64`;
  - `png` for image writing;
  - `clap` for the command line;
  - `ratatui` and `crossterm` for the interactive viewer.
- **Licences.** Every dependency shall have a licence compatible with the project's own: MIT, Apache 2.0, BSD, zlib, ISC or CC0. GPL-licensed crates are excluded (REQ-GEN-012). The rule shall be enforced with `cargo-deny`, which shall also check for security advisories.

*Verification:* `cargo-deny` in continuous integration; inspection of the core's dependency list.

**REQ-GEN-018 — Optional C interface.**
*Phase:* Phase 2 (whole requirement).
A separate crate, `tellytxt-ffi`, shall offer the core's interface to ports written in C. Its header shall be generated with `cbindgen`, and it shall be built as a static library. The crate shall be the only one allowed to contain `unsafe` code, and it shall document:
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
- **Allowed modules.** Within `tellytxt-core`, each module may use only the modules listed for it in section 9 of `ARCHITECTURE.md`.
- **Narrow boundaries.** Neighbouring layers communicate only through the boundary types listed in section 5 of `ARCHITECTURE.md`. Types from two or more layers down shall not appear in a layer's public interface.
- **The web stays separate.** `tellytxt-web` shall not depend on the decoding layers (L1 to L4). Pages leave it only as bytes, which enter through L0.
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

Packets 27 with designation codes 1 to 3 shall be stored without interpretation. Designation codes 4 to 15 are Level 2.5 compositional links and shall be ignored.
*Verification:* Test.

**REQ-PKT-006 — Page enhancement packet X/28.**
For packet number 28 with designation code 0 in format 1 (as identified by clause 9.4.2), the decoder shall Hamming 24/18 decode triplets 1 and 2 and extract:
- the page function (bits 1 to 4 of triplet 1);
- the page coding (bits 5 to 7 of triplet 1);
- the 7-bit default G0 and G2 designation (bits 8 to 14 of triplet 1);
- the 7-bit second G0 designation (bits 15 to 18 of triplet 1 and bits 1 to 3 of triplet 2), laid out as Tables 32 and 33 define them.

Packet X/28/1 shall be stored without interpretation (decision E.4). Other designation codes shall be ignored.
*Verification:* Test.

**REQ-PKT-007 — Magazine enhancement packet M/29.**
For packet number 29 with designation code 0 and packet function 0000000, the decoder shall extract the default G0 and G2 designation and the second G0 designation in the same way as for X/28/0. It shall store them for the magazine. Packet M/29/1 shall be stored without interpretation. Other designation codes shall be ignored.
*Verification:* Test.

**REQ-PKT-008 — Packets that do not belong to pages.**
Packets numbered 30 in magazines other than 8, and all packets numbered 31, shall be counted and ignored. Packets numbered 29 and 30 shall never start, end or attach to a page (clauses 7.3, 7.4 and 9.7).
*Verification:* Test that inserts these packets in the middle of a page and checks that the page is unaffected.

**REQ-PKT-009 — Reserved bits.**
Bits and bytes that the standard marks as reserved shall be ignored (clause 9.2).
*Verification:* Inspection.

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
- packets X/28/0 format 1 and X/28/1;
- the time of the first and most recent reception;
- the number of receptions;
- counts of errors.
*Verification:* Inspection; test.

**REQ-ACQ-008 — Merging repeated receptions.**
When a page and sub-page that is already stored is received again without the erase bit, each received byte shall be merged into the store according to the parity-failure policy of decision E.8. A newly received row shall replace the stored row byte by byte. Rows not received in the new transmission shall keep their stored content.
*Verification:* Test with deliberately corrupted retransmissions.

**REQ-ACQ-009 — Magazine data.**
The decoder shall store, for each magazine, the most recently received valid packets M/29/0 and M/29/1.
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
Headers with hexadecimal page digits (A to F) shall be processed for page termination like any other header. Such pages shall not be stored in cache mode unless a diagnostic option enables it, and viewers shall not be able to select them by digit entry (EN 300 706 Table 1 note).
*Verification:* Test.

**REQ-ACQ-014 — Non-text pages.**
If a page carries packet X/28/0 format 1 whose page function is not "basic Level 1 page" (0000), or whose page coding is not "7 bits plus odd parity" (000), the page shall be flagged as not displayable as text and shall not be rendered as a text page.
*Verification:* Test.

**REQ-ACQ-015 — Subtitle and newsflash pages.**
Pages with C5 or C6 set shall be stored and displayed like other pages. Their display shall use the boxed mode of decision E.22. Each reception replaces the previous content (normally with C4 set), so that subtitles update as they are transmitted.
*Verification:* Test with a subtitle stream.

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
*Phase:* Phase 1 for the four positions used at Level 1.5; Phase 2 for glyphs of the other G3 positions.
The G3 set (Table 48) shall be implemented. At minimum, positions 5/1 (horizontal line), 5/B (right arrow), 5/C (left arrow) and 5/D (up arrow), the ones Level 1.5 transmissions use in practice (Annex E.3), shall have glyphs. All other defined positions should have glyphs. The unassigned positions 6/E, 6/F, 7/E and 7/F shall display as a space.
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
Level 1.5 processing shall be switchable on and off in the configuration, and shall be on by default. When it is off, packets X/26 shall be stored but not applied.
*Verification:* Test.

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
Each character written by the Level 1.5 pass shall be placed in the cell grid according to all the rules of decision E.5 (a) to (i).
*Verification:* Test for each rule.

**REQ-L15-009 — Out-of-order triplets.**
*Phase:* Phase 1 for the default behaviour; Phase 2 for the strict option.
By default, a triplet that addresses a position earlier than the previous triplet shall still be processed. A strict option shall ignore it, as Annex A.4 permits.
*Verification:* Test in both settings.

**REQ-L15-010 — Character sets for overwrites.**
The G0 and G2 sets used by the Level 1.5 pass shall be the designated default sets of decision E.4. National options and the ESC attribute shall not affect them.
*Verification:* Test.

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

**REQ-NAV-003 — Fastext keys.**
When the displayed page has a valid packet X/27/0:
- the red, green, yellow and cyan keys shall request the pages given by links 1, 2, 3 and 4;
- the Index key shall request the page given by link 6.

A null link shall leave the key without effect. A link with sub-code 3F7F requests any sub-page; any other sub-code requests that sub-page. When the page has neither packet X/27/0 nor X/24, the coloured keys shall have no effect, and the Index key shall request the initial page (REQ-SVC-004, Annex H.4).
*Verification:* Test.

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
The eight Level 1 colours shall be rendered from colour lookup table 0 of Table 30, with each primary either 0 or full intensity (255 in 8-bit output). The colour values shall be configurable for users who want a different palette.
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

**REQ-RND-010 — Glyph data.**
*Phase:* Phase 1 for the listed Latin, supplementary and G3 coverage; Phase 2 for glyphs of the other sets.
The raster renderer shall use one glyph source in which each character is a 5-dot by 9-line pattern placed in a 6 by 10 cell, the SAA5050 geometry (Part C.3). The source shall cover:
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
The decoder shall mark each screen row as changed whenever its content changes. For a row containing double height, the row below shall be marked too. Rows containing flashing or concealed cells shall be recorded, so that a change of flash phase or reveal state affects only those rows. The renderers shall draw only changed rows unless a full redraw is requested. Drawing shall be decoupled from decoding: the program draws when rows have changed or the flash state changes, not once per packet received.
*Verification:* Test that counts the rows drawn for a flash change and for a single-row update.

**REQ-RND-021 — Reference callers.**
The library shall include these reference users of the row renderer and the platform interface (REQ-ARCH-009). None of them shows pixels on a screen or touches hardware (REQ-ARCH-010):
- the whole-screen convenience loop (REQ-RND-022);
- the image file writers (REQ-RND-007);
- a test harness that calls the row renderer in unusual orders and records the results, used for the independence test of REQ-RND-017 and to test change tracking.

Ports for specific screens, operating systems or devices are written by their users against the platform interface, and are not part of this library.
*Verification:* Test.

**REQ-RND-022 — Whole-screen convenience loop.**
The library shall provide a convenience function that renders all changed rows, or all rows, into a whole-screen indexed framebuffer supplied by the caller, at 8 or 4 bits per pixel. It does this by calling the row renderer for each row at the right offset. In the 12 by 20 mode, a 480 by 500 pixel screen needs 240 kilobytes at 8 bits per pixel, or 120 kilobytes at 4 bits.

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
*Verification:* Test that a flash change and a reveal change alter only the palette, and that the result matches a full redraw.

**REQ-RND-024 — No scaling in the core; aspect correction at the edge.**
The core renderer shall only ever produce the two fixed resolutions of REQ-RND-002 and REQ-RND-003. All scaling shall happen in the port, optionally using the helper below. The library shall provide a helper that scales by whole numbers and optionally stretches the height by 1.2 to give correct 4:3 proportions (REQ-RND-004). It shall use a nearest-pixel method by default and a smoothed method as an option. The porting guide shall warn about the aspect factor explicitly.
*Verification:* Test of the helper; inspection of the porting guide.

**REQ-RND-028 — The original glyph set.**
*Phase:* Must be complete before the first public release. It is not needed for the Phase 1 milestones, which use the temporary glyphs of decision E.25.

An original glyph set shall be drawn for this project, in the 5-dot by 9-line form of REQ-RND-010. It shall cover, in this order of priority:
1. the Latin G0 set with every national option character (Table 36);
2. the Latin G2 set and the diacritical marks;
3. the G3 characters used at Level 1.5;
4. Cyrillic, Greek and Hebrew;
5. the rest of G3;
6. Arabic.

Glyphs shall be stored as text files that can be read and compared, with one glyph per entry, like the temporary export. Each set shall be recorded with its author and licence. The drawing shall be done without tracing the SAA5050 or Bedstead glyphs, although the grid, the rounding rule and the general style are not protected.
*Reason:* Replacing the temporary glyphs is real design work: several hundred characters. It needs its own plan and budget, estimated at 10 to 20 days.
*Verification:* Inspection of coverage and provenance; the release check of REQ-RND-020.

**REQ-RND-020 — Glyph provenance.**
The glyph data used in a release shall be drawn originally for this project, as decision E.25 describes. During development, the Bedstead glyphs in `third_party/bedstead/` may be used as a temporary substitute. The glyph loader shall treat the glyph set as replaceable data, so that the substitution needs no code changes. The provenance of every glyph set shall be recorded in the repository. No glyph may be copied from the contents of a character generator ROM.
*Verification:* Inspection. Before any release, a check shall confirm that no Bedstead-derived glyph remains.

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
- packets X/28/0 format 1.
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
| `ch4` | Channel 4 capture | `wss://zxnet.co.uk/ws/ch4` |
| `test` | Test service | `wss://zxnet.co.uk/ws/test` |

A channel identifier or an address may also be given directly.

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
| `channelSettings`, `secondTick`, `wrongname` | Viewer settings, a timing tick, and a service-name mismatch | `wrongname` means stop reconnecting. |

In the control bit number, C4 is 0x0010, C5 is 0x0020 and so on, up to C11 at 0x0800. The national option bits occupy 0x7000.

**Conversion to packets.** The reader shall rebuild each `header` and `row` message as a complete 42-byte packet in the internal form (REQ-ARCH-002) by Hamming-encoding the packet address from the magazine and packet number, and prepending it to the 40 received bytes. The packets then pass through normal acquisition (REQ-ARCH-004). The decoded fields that the server also sends in `header` messages shall be used only to cross-check the decoder, and any disagreement shall be reported.

**Filtering.** The server sends rows only for the pages requested in `pagesearch` slots, so the feed is a filtered stream, not a whole-service stream. To capture a whole service, the reader shall offer a mode that walks the `pageExists` list, requesting pages in turn through the available slots.

**Tolerance and reconnection.** Unknown message types and extra fields shall be ignored. After a dropped connection the reader shall reconnect after 10 seconds, giving up after 5 failed attempts. It shall treat 5 seconds without a `clock` or `header` message as a dropped connection, as the viewer does.

**Build.** This reader is part of the `tellytxt-web` crate. It uses that crate's sans-IO WebSocket code and the transport supplied by the port (REQ-WEB-001), including the port's encryption. The core library remains free of networking (REQ-GEN-002).
*Reason:* This is the only live source of teletext with raw packet bytes found on the internet. It gives continuous real-time test data (REQ-TST-063).
*Verification:* Test against a recorded session replayed from a file; demonstration against the live servers.

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
- Level 1.5 on;
- X/26/15 processed;
- the row-setting rule of Annex A.3;
- double Start Box;
- black foreground codes ignored;
- row 24 displayed according to Annex H;
- rendering in 12 by 20 cells with rounding.
*Verification:* Test.

**REQ-PRF-002 — The "SAA5050" profile.**
*Phase:* Phase 2 (whole requirement).
A profile shall reproduce a receiver built with the SAA5050 and its companion chips:
- Level 1.0 only, with packets X/26 ignored;
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

---

## F.15 Command-line program (CLI)

**REQ-CLI-001 — Program and commands.**
A command-line program named `ttx` shall provide these commands:

| Command | Purpose |
|---|---|
| `ttx list` | List the pages and sub-pages found in an input, with counts and error statistics. |
| `ttx show` | Display one page in the terminal. |
| `ttx render` | Write one page, or all pages, as image files. |
| `ttx text` | Write one page, or all pages, as UTF-8 text. |
| `ttx convert` | Convert between the input and output formats of area INP, for example from a transport stream to T42, or from TTI to T42. |
| `ttx slice` | Convert raw sampled video to T42 with quality statistics. |
| `ttx services` | List the teletext streams, languages and subtitle pages found in a transport stream. |
| `ttx service-info` | Print the service data from packet 8/30: initial page, network identification, date, time and status message. |
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
- **The raw sampled captures published with vhs-teletext**, for the slicer (REQ-TST-061).

- **A local corpus of recovered broadcasts.** The workspace already holds T42 recoveries made by Dan Farrimond from VHS recordings, in `~/devel/jellytext/dan/`:
  - BBC1, 31 October 1991;
  - TV-am, 20 October 1992;
  - The Children's Channel, January 1993 (several captures);
  - Sky One, 16 February 1993;
  - GMTV, 22 February 1993.

  There are full and "squashed" versions, where squashed means the duplicate sub-pages have been merged. The tests shall read local corpus files from a directory given by the environment variable `TELLYTXT_CORPUS`, and shall also keep a local cached copy of every downloaded corpus file there. The tests then do not depend on outside websites being available. Corpus files shall never be committed, because the page content belongs to the broadcasters.

Expected results for the corpus are the outputs of zvbi and vhs-teletext (REQ-TST-021), plus page counts and spot checks recorded by hand. If a source disappears and no cached copy exists, the test shall report the missing file clearly and skip, not fail. A test run that skipped corpus files shall say so in its summary, so that lost coverage is visible.
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
Every reader, the packet decoder, the slicer, and the HTTP, WebSocket, server-sent event, descriptor, form and address (REQ-WEB-016) parsers of the web module shall have a fuzzing target for `cargo fuzz` (libFuzzer). The targets shall run in continuous integration for a fixed time on each change, and for longer periods on a schedule. Every input that has ever found a fault shall be kept in the repository as a regression test.
*Verification:* Inspection; continuous integration logs.

**REQ-TST-041 — Coverage.**
The core library tests shall reach at least 90 percent line coverage.
*Verification:* Coverage report.

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
- the walk through Level 1.5 triplets (clause 12.3).

This area turns the receiver rules into six explicit state machines, and requires every one of them to tolerate any breach of the broadcaster rules.

The tables below are the normative form of behaviour that areas SLC, ACQ, L1, L15 and NAV describe in prose. If a table and a prose requirement disagree, that is a defect, to be corrected in both.

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
| Displaying or Searching | Fastext, Index, Home, next or previous page | Request the target page (REQ-NAV-002, REQ-NAV-003 and REQ-NAV-008) | Searching |
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
- triplets may address positions out of screen order (REQ-L15-009);
- magazines may be interleaved in any pattern that parallel mode allows.

A page received in any such order shall give the same stored page and the same cell grid as the same packets in standard order. The only exceptions are cases where the standard itself makes order significant: which header starts and ends a page, and which of two overwrites of the same cell comes later (decision E.5 (h)).
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

---

## F.19 Web-native page sources (WEB)

This area makes tellytxt.com a web-native receiver (decision E.30). Pages and packets can be fetched from ordinary web servers, pushed by them, and interacted with through forms. Everything here lives in an optional web module. The core library stays free of networking (REQ-GEN-002), and the network transport itself is supplied by the port (REQ-WEB-001).

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
- optionally, the default character set region (decision E.4).

The page address template uses the placeholders `{m}` (magazine), `{pp}` (page number as two hexadecimal digits), `{mpp}` and `{sub}` (sub-code as four hexadecimal digits), for example `https://example.org/ttx/pages/{mpp}`. The reader shall reject a descriptor with missing required members or unknown placeholders, with a clear diagnostic.
*Reason:* A page number is teletext's address. The template maps it to a web address without making the page files themselves aware of the web.
*Verification:* Test with valid and malformed descriptors.

**REQ-WEB-003 — Fetching pages.**
The web module shall fetch a page by expanding the template for the requested page, and optionally sub-page, and making an HTTP GET request. It shall accept these response types:
- **A TTI page file** (REQ-INP-013), media type `text/x-teletext-tti`. A file may contain several sub-pages.
- **A T42 packet stream** (REQ-INP-002), media type `application/x-teletext-t42`.

These two media types are defined by this project. For interoperability, the module shall also accept `text/plain` and `application/octet-stream`, deciding the format by the detection of REQ-INP-005. The response shall pass through the normal acquisition path, as page files do (REQ-ARCH-004).

The module shall:
- follow at most 5 redirects;
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
The cookie store shall follow current cookie practice (decision E.35): the latest draft of RFC 6265bis, the replacement for RFC 6265, as implemented by current browsers, made stricter wherever tellytxt.com's single-origin model allows. The following rules apply.

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
- **`HttpOnly` and `Partitioned`.** These shall be accepted and stored. tellytxt.com has no scripting and no embedded third-party content, so every cookie is in effect HTTP-only and first-party.

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
Servers built to serve tellytxt.com pages should follow these practices, which the reference test server (REQ-WEB-013) shall demonstrate:
- **Keep no session state on the server.** Hold it in a cookie whose value begins with a format version number, so that the format can change without breaking clients that hold old cookies (decision E.30 item 2).
- **Validate and version.** Issue a form token bound to the session, and check it on every submission. Send `ETag` validators with every page.
- **Push only what changed.** Push only changed pages, and push each page whole, not partial rows.
- **Keep pages simple.** Serve Level 1 and Level 1.5 pages only.

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
| Request headers sent | `Host`, `User-Agent` (fixed, naming tellytxt.com and its version), `Accept` (the two page media types and JSON), `Accept-Encoding: identity`, `Cookie`, `If-None-Match`, `If-Modified-Since`, `Content-Type` and `Content-Length` (form submissions only), `Last-Event-ID` (server-sent events only), and the WebSocket upgrade headers. No other headers are sent, in particular no `Referer`. |
| Response handling | Status codes 200, 204, 301, 302, 303, 304, 307, 308, 404 and other 4xx and 5xx codes as errors. Headers: `Content-Type`, `Content-Length`, `Transfer-Encoding: chunked`, `Location`, `Set-Cookie`, `ETag`, `Last-Modified`, `Cache-Control` (`max-age`, `no-cache`, `no-store`) and `Retry-After`. Other headers are ignored. |
| Connections | One request at a time per connection. Connections may be reused (persistent connections); pipelining is not used. |
| Push | WebSocket (RFC 6455) and server-sent events, as REQ-WEB-005 defines |
| Content | TTI pages, T42 packets and the JSON service descriptor. Nothing else is interpreted. |

*Permanently excluded:*
- HTTP/2, HTTP/3 and pipelining;
- compressed responses (REQ-SEC-011);
- authentication by HTTP headers or client certificates chosen by pages;
- HTML, CSS and any other markup;
- scripting of any kind (REQ-SEC-004);
- images, audio, video and other media;
- downloads to files;
- embedding one page inside another;
- content from a third party loaded into a page;
- plugins;
- `file:`, `data:`, `javascript:` and all other address schemes except those of REQ-WEB-016;
- proxy auto-configuration;
- cross-origin requests other than the permitted form origins of REQ-WEB-008.

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

A service's origin is the origin of its descriptor.
*Verification:* Test with a table of address pairs covering case, default ports, scheme differences and internationalised host names.

**REQ-WEB-016 — The strict address parser.**
All web addresses shall be parsed by one small parser in `tellytxt-web`. This covers addresses in:
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

---

# Part G — Out of Scope

The following are not part of this implementation. Each could be added later as a new set of requirements.

1. **Presentation Levels 2.5 and 3.5.**
   - Non-spacing attributes, colour lookup tables and the 32-colour palette.
   - Side panels, objects, redefinable characters and proportional or styled fonts.
   - The architecture of decision E.1 leaves room for them as further passes over the cell grid.
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
| User agent | The web term for a browser or other client acting for the user. tellytxt.com is a deliberately minimal one (REQ-WEB-014). |

---

# Appendix B — Source documents

**Earlier teletext specifications** (in `~/devel/jellytext/teletextspecifications/`, see Part A.12a): the 1974 and 1976 BBC, IBA and BREMA specifications; "World System Teletext" (1990); EBU SPB 492 (1992); and a United Kingdom 625-line television specification. They are cited by year and PDF page.

**Standards** (in `specifications/`):
- ETSI EN 300 706 V1.2.1 (2003-04), *Enhanced Teletext specification*. This is the primary source for Parts A, B and E.
- ETSI ETS 300 706 (1997), the earlier edition, used for cross-checking.
- ETSI EN 300 472 V1.3.1 and DVB BlueBook A041, *Teletext in DVB bitstreams*.
- ITU-R BT.653-3, *Teletext systems*.
- ETSI ETS 300 708, *Data transmission within Teletext*.

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

On a decoder without form support, this page shows the labels and dotted placeholders. On tellytxt.com it is a working form. The session comes from the cookie, the token from `FT`, and the submission goes to `/contact` on the page's own origin.

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

## D.2 Still open

| Question | Why it matters | How it will be settled |
|---|---|---|
| **What is the tellytxt.com product?** Who writes pages, what the server side is, and who the users are. | This document specifies the receiver and library only. The web-native design implies servers and an authoring workflow that are not specified. | A short product brief, written by the project owner, before milestone M2. |
| Does double-height rounding alternate its neighbour on every line (the datasheet's wording), or repeat each rounded line (all the emulators)? | Visible only on double-height diagonals | Comparison with photographs of a real SAA5050 (REQ-TST-030). E.13 follows the emulators until then. |
| What are G2 diacritical marks 4/9 and 4/C exactly? | Rarely used; the references disagree | A real transmission that uses them (REQ-CHR-008). |
| Is the zxnet live feed protocol stable? | It is undocumented and could change | The tolerant reader of REQ-INP-022, plus the recorded session of REQ-TST-063. The site's operator could also be asked. |
| What noise performance should the slicer reach? | The 20 decibel target of REQ-SLC-013 is an estimate | Measurement in milestone M4 |
| How much flash does the core really need on a Cortex-M0? | The 64 kilobyte budget rests on an estimate | The size check of REQ-GEN-009, from milestone M1 |
| Will RFC 6265bis change before it is published? | REQ-WEB-006 follows the draft of 30 September 2026 | Recheck at each milestone's security review, and when it is published as an RFC (decision E.35). |
| Which Xtensa toolchain versions build correctly? | Espressif's fork 1.94 and 1.95 fail release builds (esp-rs/rust#277) | The pinned toolchain of REQ-GEN-001, re-tested when the issue closes. |
| How does the SAA5246A behave where its datasheet is silent? For example, parity errors, serial mode and erase. | It limits how exactly the SAA5246A profile can be reproduced | Decisions E.6 to E.8 define the behaviour. A real SAA5246A-based television, observed with a generated stream, could settle the rest. |
