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
