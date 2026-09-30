# EN 300 706 V1.2.1 (2003-04): Level 1 decoder notes

Source: ETSI EN 300 706 V1.2.1 (2003-04) "Enhanced Teletext specification". Page numbers are the printed page numbers, which match the PDF page numbers for this file. Citations use the form (cl. X.Y, Table N, p. NN).
Conventions used in these notes:
- Bits in a byte are numbered 1..8 = LSB..MSB, and "normally transmitted LSB first" (cl. 7.1, p. 16; cl. 3.1 "bit numbering within bytes", p. 11).
- Character code notation "r/c" in the spec means column/row of a code table, i.e. hex high nibble / low nibble. 1/E = 0x1E. (cl. 3.2 defines "Character code r/c"; in practice the spec writes column/row, e.g. "2/0" = SPACE = 0x20. p. 12.)
- "DERIVED" marks a value I computed from the spec's equations or tables rather than read directly from it. "UNVERIFIED" marks something the spec does not state, or that I could not confirm.

---------------------------------------------------------------------------------------------------

## 1. Signal and transmission basics

| Item | Value | Cite |
|---|---|---|
| Usable VBI lines (multiplexed with CVBS) | lines 6–22 and 318–335 | cl. 4.1, p. 13 |
| Signalling | Binary NRZ | cl. 5.1, p. 14 |
| Levels | 0 = black ±2 %; 1 = 66 ±6 % of black-to-peak-white | cl. 5.2, p. 14 |
| Bit rate | 444 × nominal fH = **6,9375 Mbit/s ±25 ppm** | cl. 5.3, p. 14 |
| Clock run-in | bit pattern in transmission order **1010101010101010** (2 bytes). If read LSB-first as bytes, that is 0x55 0x55 (DERIVED) | cl. 6.1, p. 15 |
| Framing code | bit pattern in transmission order **11100100**. Read LSB-first as a byte, that is **0x27** (DERIVED) | cl. 6.2, p. 15 |
| Timing reference | 12,0 µs nominal from line sync half-amplitude point to mid-point of the penultimate '1' of the clock run-in (Fig. 4 shows 12 +0,4/−1,0 µs) | cl. 6.3, Fig. 4, p. 16 |
| Packet size | "360 bits organized as 45 bytes, numbered 1 to 45". Bytes 1–2 are the clock run-in, byte 3 is the framing code, bytes 4–5 are the packet address, and bytes 6–45 are data (40 bytes). After the framing code there are **42 bytes** (bytes 4–45). | cl. 7.1, 7.1.1, 7.1.3, p. 16–17; Fig. 5 |
| Bit order | "bytes are transmitted least significant bit first" unless stated otherwise | cl. 8, p. 20 |

- Clock run-in note: "The clock run-in does not in itself identify a Teletext data line. Under some conditions, the two leading data 'ones' may be absent or reduced in amplitude" (cl. 6.1 NOTE, p. 15).
- Framing code: "Following the detection of the framing code sequence, a decoder should partition the remaining data bits of the packet into 8-bit bytes." Other framing codes may be used for data not meant for these decoders (cl. 6.2, p. 15).
- Robustness (Annex P, p. 153, informative): signals such as Videocrypt can look like valid Teletext. "some existing decoders will accept a single bit error in the framing code…" "Teletext decoders should be designed to be insensitive to non-Teletext waveforms."
- Lines 6, 318 and 319 may not be decoded by some Level 1/1.5 decoders. Broadcasters should use them only for 2.5/3.5 data (Annex F.4, p. 138).

### Byte coding per packet type
There are four coding methods: odd parity (7+1), Hamming 8/4, Hamming 24/18 (triplets), and 8-bit data (cl. 8, p. 20).

| Packet | Bytes 4–5 | Byte 6 | Bytes 7–13 | Bytes 14–45 | Cite |
|---|---|---|---|---|---|
| X/0 header | Hamming 8/4 (MRAG) | Ham 8/4 (page units) | bytes 6–13 all Ham 8/4 (page, sub-code, control bits) | 32 bytes odd parity | cl. 9.3.1, Fig. 9, p. 25; 9.3.1.4 p. 27 |
| X/1–X/25 (display pages) | Ham 8/4 | bytes 6–45 = 40 bytes odd parity | | | cl. 9.3.2, Fig. 10, p. 27 |
| X/26, X/28, M/29 | Ham 8/4 | Ham 8/4 designation code | bytes 7–45 = 13 Hamming 24/18 triplets | | cl. 9.4, Fig. 11, p. 27 |
| X/27/0–3 (editorial links) | Ham 8/4 | Ham 8/4 DC | bytes 7–43 Ham 8/4; bytes 44–45 = 8-bit CRC | | cl. 9.6.1, Table 14, p. 38 |
| 8/30 fmt 1 | Ham 8/4 | Ham 8/4 DC | 7–12 Ham 8/4; 13–25 8-bit data | 26–45 odd parity | cl. 9.8.1, Table 18, p. 44 |
| 8/30 fmt 2 | Ham 8/4 | Ham 8/4 DC | 7–12 Ham 8/4; 13–25 PDC (EN 300 231) | 26–45 odd parity | cl. 9.8.2, Table 19, p. 46 |

- Non-display pages (TOP, MOT, POP, data broadcasting) may code X/1–X/25 differently. The page coding is given in X/28/0 fmt 1 Table 3 (cl. 9.3.2, p. 27; cl. 9.4.2.1, Table 3, p. 29).

### Odd parity (cl. 8.1, p. 20)
- "bit 8 is the parity bit (P) and bits 1 to 7 carry the data bits." Encoding: P = 1 ⊕ D1 ⊕ … ⊕ D7. Decoding: "if D1 ⊕ D2 ⊕ … ⊕ D7 ⊕ P = 1, accept data bits." Single-bit errors are detected but not corrected.
- The spec does **not** say what to display when a parity check fails. Common practice is to keep the previously stored character or show a space. That choice is UNVERIFIED (not specified; decoder choice).

---------------------------------------------------------------------------------------------------

## 2. Hamming 8/4 (cl. 8.2, p. 20–21)

- "bits 1, 3, 5 and 7 are the protection bits and bits 2, 4, 6 and 8 carry the data. Single bit errors can be identified and corrected. Double bit errors can be detected."
- Transmission order: bit1 P1, bit2 D1, bit3 P2, bit4 D2, bit5 P3, bit6 D3, bit7 P4, bit8 D4.
- In every Hamming 8/4 table in the spec, D1 (byte bit 2) has weight 2^0 and D4 (byte bit 8) has weight 2^3 (e.g. cl. 7.1.2 table, p. 17).
- Encoding:
  - P1 = 1 ⊕ D1 ⊕ D3 ⊕ D4
  - P2 = 1 ⊕ D1 ⊕ D2 ⊕ D4
  - P3 = 1 ⊕ D1 ⊕ D2 ⊕ D3
  - P4 = 1 ⊕ P1 ⊕ D1 ⊕ P2 ⊕ D2 ⊕ P3 ⊕ D3 ⊕ D4
- Decoding uses four odd-parity tests (read from the PDF table, p. 21):
  - A = {P1, D1, D3, D4}
  - B = {D1, P2, D2, D4}
  - C = {D1, D2, P3, D3}
  - D = all 8 bits
- Decision table (p. 21):

| A,B,C | D | Inference | Action |
|---|---|---|---|
| All correct | Correct | No errors | Accept data bits |
| All correct | Not correct | Error in P4 | Accept data bits |
| Not all correct | Correct | Double error | **Reject data bits** |
| Not all correct | Not correct | Single error | "Refer to previous table to identify bit in error. Correct error if in data bit." |

- On an uncorrectable (double) error the spec says only "Reject data bits". It does not say what the decoder should do with the rest of the packet. A sensible policy (UNVERIFIED, decoder choice) is:
  - discard the whole packet if the MRAG (bytes 4–5) fails;
  - for a header, do not start or terminate a page on an unreliable page number.
- **The spec does not list the 16 codewords.** Computed from the equations above (DERIVED; the byte is shown as received with bit 1 = LSB):

| Data | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | A | B | C | D | E | F |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Byte | 15 | 02 | 49 | 5E | 64 | 73 | 38 | 2F | D0 | C7 | 8C | 9B | A1 | B6 | FD | EA |

- Hamming 24/18 is defined in cl. 8.3 (p. 21–22). A Level 1 decoder needs it only if it parses X/28 or M/29, which is not required at Level 1.

---------------------------------------------------------------------------------------------------

## 3. Magazine and Packet Address (MRAG) (cl. 7.1.2, table, p. 17)

| Function | Byte | Data bits (byte bit no.) | Weight | Range |
|---|---|---|---|---|
| Magazine (X or M) | 4 | 2, 4, 6 | 2^0, 2^1, 2^2 | 0–7 |
| Packet number Y | 4 | 8 | 2^0 | 0–31 |
| Packet number Y | 5 | 2, 4, 6, 8 | 2^1, 2^2, 2^3, 2^4 | 0–31 |

- DERIVED: with n4 = Hamming-decoded nibble of byte 4 and n5 = Hamming-decoded nibble of byte 5, magazine = n4 & 7 and Y = (n4 >> 3) | (n5 << 1).
- "magazine number 8: a packet with a magazine value of 0 is referred to as belonging to magazine 8" (cl. 3.1, p. 11). The page number range for M is 1–8 (cl. 3.1 "page number", p. 12).
- For Y = 26 to 31, byte 6 is a Hamming 8/4 "Designation Code" that extends the address (cl. 7.1.2, p. 17).

### Uses of Y (Table 1, p. 23–24; Level ticks read from the PDF)

| Y | Use | Level 1? | Cite |
|---|---|---|---|
| 0 | Page header. "Acts as both a page identifier and a page terminating packet. Decoders should respond to packets X/0 for ALL possible page numbers and sub-codes, including those with hexadecimal elements…" NOTE: "It is not intended that the viewer should be provided with the means to select directly pages with hexadecimal elements in their address." | ✓ | Table 1 |
| 1–23 | Display rows (7 bits + odd parity). Also used by TOP data pages at all levels. | ✓ | Table 1 |
| 24 | Row 24. "Used for navigational purposes in the FLOF Code of Practice." | ✓ | Table 1; cl. 11.1 |
| 25 | "As part of a basic Teletext display page, the packet carries a number of displayable labels relating to the data in the page for key-word search applications." (Also used on non-display enhancement pages.) | ✓ (ticked) | Table 1 |
| 26/0–14 | VCR programming (PDC, EN 300 231): ✓ at all levels. Character overwrite: Level 1.5 only. 26/0–15 full enhancement: 2.5/3.5. | L1 must skip for display | Table 1; cl. 9.4.1 |
| 27/0 | Editorial linking (FLOF) | ✓ | Table 1; cl. 9.6.1 |
| 27/1–3 | Additional editorial links ("Application not currently defined") | note 2 | Table 1 |
| 27/4–7 | Compositional links (2.5/3.5) or data broadcasting | ✗ | Table 1 |
| 28/0 fmt 1 | Page function and page coding: ✓ at all levels ("Can form part of any page at any presentation Level… transmission is not mandatory", note 3). Presentation data (character set designation etc.): 2.5/3.5 only. | partly | Table 1 |
| 28/1 | Character set designation (earlier specs): L1.5 only | ✗ | Table 1 |
| 28/2 | Data broadcasting page key (EN 300 708) | ✗ | Table 1 |
| 28/3, 28/4 | DRCS / CLUT (2.5/3.5) | ✗ | Table 1 |
| 29 (M/29/0, /1, /4) | Magazine-wide enhancement data. M/29/1 is ticked for L1.5 only. | ✗ | Table 1; cl. 9.5 |
| 30 | 8/30 (magazine 8 = value 0): broadcast service data, formats 1 (DC 0–1) and 2 (DC 2–3), ✓ at all levels. 1–3/30 and 5–7/30 "Use not currently defined" (may be independent data). 4/30 proposed for audio description. | ✓ (8/30) | Table 1; cl. 9.8 |
| 31 | 8/31–3/31: independent data services. 4/31–7/31 not defined. "the magazine value represents an additional channel identifier." | not display | Table 1; cl. 9.7 |

- Y = 30 and 31 are "not page- or magazine-related" (cl. 7.4, p. 19). "They can be inserted at any point within the transmission" (cl. 9.7, p. 43). A decoder must not let them terminate or attach to a page.
- Y = 29 "relates to all pages with the included magazine address and not to any one specific page" (cl. 7.3, p. 19).
- "Decoders should ignore bits and bytes which are indicated as being reserved for future use." (cl. 9.2, p. 24)

---------------------------------------------------------------------------------------------------

## 4. Page header packet X/0 (cl. 9.3.1, Fig. 9, p. 25–27)

### Byte map
| Byte | Coding | Content |
|---|---|---|
| 1–2 | – | clock run-in |
| 3 | – | framing code |
| 4–5 | Ham 8/4 | MRAG, Y = 0 |
| 6 | Ham 8/4 | Page units (data bits 2,4,6,8 → 2^0..2^3), range 0–F |
| 7 | Ham 8/4 | Page tens (2^0..2^3), range 0–F |
| 8 | Ham 8/4 | Sub-code S1 (LSB digit), bits 2,4,6,8 → 2^0..2^3, range 0–F |
| 9 | Ham 8/4 | S2: bits 2,4,6 → 2^0..2^2 (range 0–7); **C4** at bit 8 |
| 10 | Ham 8/4 | S3: bits 2,4,6,8 → 2^0..2^3, range 0–F |
| 11 | Ham 8/4 | S4: bits 2,4 → 2^0, 2^1 (range 0–3); **C5** at bit 6; **C6** at bit 8 |
| 12 | Ham 8/4 | **C7** bit 2, **C8** bit 4, **C9** bit 6, **C10** bit 8 |
| 13 | Ham 8/4 | **C11** bit 2, **C12** bit 4, **C13** bit 6, **C14** bit 8 |
| 14–45 | odd parity | 32 display characters or attribute codes |

Cite: cl. 9.3.1.1 table (p. 25), 9.3.1.2 table (p. 26), Table 2 (p. 26), 9.3.1.4 (p. 27).

- Page address definitions (cl. 3.1, p. 12):
  - Page address = "M Pt Pu: S4 S3 S2 S1".
  - Sub-code ranges: S1 0–F, S2 0–7, S3 0–F, S4 0–3, so the sub-code is 13 bits and its maximum is 3F7F.
- DERIVED nibble view, where nN is the decoded data nibble of byte N:
  - S2 = n9 & 7; C4 = n9 >> 3
  - S4 = n11 & 3; C5 = (n11 >> 2) & 1; C6 = n11 >> 3
  - C7..C10 = n12 bits 0..3
  - C11..C14 = n13 bits 0..3
- "The control bits are active on being set to '1'." (cl. 9.3.1.3, p. 26)

### Control bits (Table 2, p. 26), verbatim essentials
| Bit | Name | Meaning |
|---|---|---|
| C4 | Erase Page | "Packets X/1 to X/28 belonging to a previous transmission of the page should be erased from the decoder's memory before packets belonging to the associated page are stored." |
| C5 | Newsflash | "…the associated page is a Newsflash page. All information intended for display on such a page will be boxed and will be displayed inset into the normal video picture." |
| C6 | Subtitle | "…the associated page is a subtitle page. All information intended for display on such a page will be boxed and will be displayed inset into the normal video picture." |
| C7 | Suppress Header | "Data addressed to row 0 is not to be displayed." |
| C8 | Update Indicator | "Data within packets X/1 to X/28 of the associated page has been changed since the previous transmission. The setting of this bit is under editorial control." |
| C9 | Interrupted Sequence | "The associated page is not in numerical order of page sequence, allowing the header to be excluded from a rolling header display to avoid discontinuities." |
| C10 | Inhibit Display | "Data addressed to rows 1 to 24 is not to be displayed." |
| C11 | Magazine Serial | '1' = Serial mode: "the transmission of a page is terminated by the next page header with a different page number." '0' = Parallel mode: "terminated by the next page header with a different page number but the same magazine number." "The same setting shall be used for all page headers in the service." |
| C12–C14 | National Option Character Subset | "Where the decoder is capable of displaying text in more than one language these control bits are used to select G0 character set options (see clause 15.2). The response … may be modified by packets X/28/0 Format 1, X/28/4, M/29/0 and M/29/4." |

- **Inconsistency to note.** Table 2's C11 text says a serial-mode page is terminated by the next header "with a different page number". Clause 7.2.1 (p. 18) and B.3 (p. 125) say a page is terminated by "the next page header having the same magazine address in parallel transmission mode, or any magazine address in serial transmission mode" (B.3: "The second page header may be from any magazine"). A robust decoder should treat any header as ending the current page in serial mode, and any header with the same magazine as ending it in parallel mode. The C11 wording looks like a simplification.
- Annex A.2 (p. 121), rules for display pages:
  - "When the Erase bit C4 is set, that page is expected to be erased completely and a new page written into memory."
  - C8 "will cause a prompt, which may involve automatic redisplay of the page" where the display was cancelled by the user; Newsflash pages are the standard example.
  - "The effect of both of these bits applies to packets 1 to 28 inclusively…"

### Display bytes 14–45
- "Bytes 14 to 45 in page header packets carry 32 character or display control codes, coded 7 data bits plus one bit odd parity. They are normally intended for display. Bytes 38 to 45 are usually coded to represent a real-time clock." (cl. 9.3.1.4, p. 27)
- Table 25 (p. 67), Display Rows and Columns: "Following the 8 control codes (which should not be displayed), 32 character-spaces in the header (row 0), transmitted from left to right." NOTE: "It is usual practice to transmit 'time-of-day' information in the last 8 character spaces of packet X/0."
- DERIVED column mapping: row 0 has 40 columns. Bytes 6–13 occupy columns 0–7 and are not displayed as text. Byte n (14..45) → column n − 6 (8..39). The clock bytes 38–45 → columns 32–39.
- **Where the page number is shown in columns 0–7 is not specified** by EN 300 706. The only reference is E.7 (p. 135): "Some decoders reserve the first eight character cells in the page header row for their own purposes." A decoder-generated page-number display there is a decoder choice (UNVERIFIED as a normative rule).
- E.14 (p. 136): "The last 8 bytes of page header packets are usually coded with a real time clock: hours, minutes seconds numerically in that order. The preferred format is H H separator M M separator S S."
- F.5 (p. 138) refers to "bytes 39 to 45" as the clock. This differs slightly from 9.3.1.4 (38–45). The F.5 point: some decoders will not display the clock if X/0 is sent only on lines 22/335, so broadcasters send an additional M/0 elsewhere at least once per second, with C9 = 0.

### Special page numbers, time filling, hex pages
- cl. 7.2.2 (p. 18): "The Page Address may take any value except: Page Number = FF and Sub-code = 3F7F. The address XFF:3F7F is reserved as a null page address."
- cl. 7.3 (p. 19): headers XFF:0000–XFF:3F7E close the transmission of a page when a magazine has only one displayable page. "These headers may be referred to as 'Time Filling Headers', when they are used to keep the real-time clock field updated in a magazine in parallel transmission mode."
- cl. 3.1 (p. 12), definition of time filling headers: "page header packets with the page number FF which are inserted into the transmission for the sole reason of maintaining a real-time clock display".
- Annex A.1 (p. 120):
  1. "Any page address up to and including hexadecimal FE with any value of sub-code (range 0000 to 3F7F), can be used for a page carrying data and can be specified as a linked page."
  2. "Pages address number FF in any magazine, with any sub-code in the range 0000 to 3F7E may be used for time filling and as a terminator. Pages with these addresses will not carry data for acquisition by a decoder."
  3. "Page header packets with the page address FF, sub-code 3F7F, will not be transmitted. This address will occur as a null link in packets with Y = 27 and 8/30."
- Hex pages: decoders must respond to headers with hex digits (Table 1). Hex pages are not meant to be user-selectable. E.1 (p. 134): object and DRCS pages use hex page numbers "to prevent them being captured as normal display pages".
- Reserved page numbers (A.10, p. 124):
  - 1BE: ACI
  - 1F0: Basic TOP Table
  - mFD: MIP
  - mFE: MOT
  - mFF: "Time filling and terminator"

---------------------------------------------------------------------------------------------------

## 5. Page transmission rules

### Page boundaries
- "Following the page header packet of a page, all subsequent packets with Y = 1 to Y = 28 inclusive, from the same magazine, relate to that page." "The transmission of a given page begins with, and includes, its page header packet. It is terminated by and excludes the next page header packet having the same magazine address in parallel transmission mode, or any magazine address in serial transmission mode." (cl. 7.2.1, p. 18)
- "Packets containing no information do not need to be transmitted." (cl. 7.2.3, p. 18) Pages can be sent row-adaptively (D.1.1 NOTE, p. 131).
- Missing rows: "If any decoder assumes the page is a basic text page, any missing rows in the range X/1 to X/23 should be displayed as if they contained 40 character spaces (code 2/0). Where required, packets X/24 and X/25 should be treated in the same way." (D.1.1, p. 131)

### Serial mode (B.3, p. 125–126), C11 = 1
1. Any X/1–X/28 between two successive headers belong to the page of the first header.
2. They carry the same magazine address as that header.
3. X/1–X/28 from other magazines between those headers are not permitted.
4. The second header may be from any magazine.
5. M/29 from any magazine and packets 30/31 "may be inserted at any point".
- b) "It is preferable for a decoder to use all page headers (providing control bit C9 is set to 0) from all magazines when displaying rolling headers."

### Parallel mode (B.4, p. 126), C11 = 0
1. X/0–X/28 from any magazine may be in any sequence, provided each page follows B.2.
2. "Following a page header with a certain magazine address, only those packets in the range X/1 to X/28 with the same magazine address belong to the page defined by the page header."
3. M/29 and 30/31 may be inserted anywhere.
- b) "It is preferable for a decoder to use page headers (providing control bit C9 set to 0) from only one magazine when displaying rolling headers."
- cl. 7.4 (p. 19): "no particular relationship of VBI line to magazine number may be assumed."

### Packet order within a page (B.2, p. 125)
- Header, then any Y = 27, then Y = 28, "in that order".
- For display pages, Y = 26 are sent before any Y = 1–25, in ascending designation code.
- If transmission is interrupted and restarted, the 27/28/26 packets are re-sent after the header "regardless of the state of the C4 bit".

### Timing
- Page erasure interval, B.1 (p. 125): "a page clearing interval of 20 ms should be maintained in the transmission between the header and any packets of a page that is intended to be interpreted by a Level 1 or 1.5 decoder. This time interval is required by many decoders for erasing the previous page from memory."
- B.9 (p. 127): "the maximum time interval between the transmission of successive packets of the same page shall be 100 ms." NOTE: this "allow[s] software-based decoders to determine when to display the packets received so far if the closing page header is delayed or absent." This supports a 100 ms page-completion timeout in a C decoder.
- F.2 (p. 138): Level 1 pages should be completely re-transmitted, including extension packets, after any interruption (e.g. subtitle insertion).
- F.6 (p. 139), full-field transmission: some decoders do not clear old pages, so broadcasters should avoid row-adaptive transmission and C4 and should send X/0–X/23 (and X/24) for every page.

### Sub-pages and sub-codes (A.1, p. 120)
1. "Pages with no sub-pages associated should be coded Mxx-0000."
2. "Pages intended for display with sub-pages should have sub-pages coded sequentially from Mxx-0001 to Mxx-0009 and then Mxx-0010 to Mxx-0019 and similarly using the decimal values of sub-code nibbles S2 and S1 to Mxx-0079."
3. A sub-page whose content has not changed may be sent with the same sub-code as the preceding one.
- NOTE 1: some decoders need a non-zero sub-code. "A sub-code outside the range 1 to 79 (S2, S1) should be used in order to prevent sub-page collecting decoders indicating that a page is a rotating page when it only has one sub-page."
- Data pages (MOT, POP, …) use sub-code nnXs semantics instead. This is not relevant to Level 1 display.
- E.2 (p. 134), Page Time-Code: sub-code bytes 8, 9, 10, 11 may carry minutes units/tens and hours units/tens in BCD, for example for an "Alarm Clock page".

### Searching, rolling header and clock
- While a page is being searched for, the decoder displays incoming headers ("rolling header"):
  - exclude headers with C9 = 1 (Table 2);
  - serial mode: use all magazines; parallel mode: use one magazine (B.3b, B.4b).
- E.3 and E.7 (p. 134–135): X/26 or object enhancements in row 0 need not be reproduced "for incorrect page headers while it is searching for a page and rolling the Level 1 text from those page headers."
- The clock appears in header bytes 38–45 (see §4).

### "Page not in transmission"
- The only definition in EN 300 706 is Magazine Inventory Page code 00, "Page not in transmission" (cl. 11.3.3, Table 24, p. 64).
- There is no normative Level 1 rule for detecting or announcing a missing page (e.g. a timeout). Any such behaviour is a decoder design choice (UNVERIFIED as a spec requirement). TOP (Annex I.2.3, p. 145) mentions decoder messages such as "Page not included".

### Initial page and service level (D.3, p. 133; H.4.1, p. 143; E.15, p. 137)
- "In the absence of any packet 8/30/0, 8/30/1, 8/30/2 or 8/30/3 … it can be assumed that the page number 100 is being broadcast and is suitable for use as an introductory page." NOTE: a decoder may prefer a user-specified page.
- D.3(1): if no 8/30/1 or 8/30/3 is present, assume the transmission is confined to the VBI.
- E.15: 100 is the "Home" page and can be reassigned via the Initial Teletext Page field of 8/30 format 1.
- E.15 also lists 1DF (EPG), 1E7 (triggers), 1F1–1F4 (TOP AIT/MPT/MPT-EX; actual numbers come from the BTT).

---------------------------------------------------------------------------------------------------

## 6. Packets X/1–X/28, M/29, 8/30 (Level 1 view)

### X/1–X/23, X/24, X/25
- The format is Fig. 10: bytes 6–45 are 40 odd-parity characters, displayed in columns 0–39 left to right (cl. 9.3.2, p. 27; Table 25, p. 67).
- Table 25 (p. 67): "Up to 24 rows, (plus an optional 25th row for navigation purposes), transmitted in packets X/0 to X/24, top to bottom… 40 character spaces (columns) in rows 1 to 24".
- X/24 (display row 24) is shown **only if** the X/27/0 Link Control Byte Display Row 24 flag = 1.
  - Table 14 (p. 38): byte 43 data bit 4, "'0': Data in packets with Y = 24 is not to be displayed. '1': Data in packets with Y = 24 is to be displayed in row 24."
  - H.2.4 (p. 143) and H.4.4 (p. 143): "A packet with Y = 24 may be included in any sequence of data. It shall only be displayed when the Display Row 24 Flag Bit in the packet X/27/0 is set to 1."
  - Consequence (DERIVED): if a page has no X/27/0, X/24 is not displayed under the FLOF Code of Practice.
- X/25 carries key-word search labels (Table 1). Its display rules are not specified further in EN 300 706 (UNVERIFIED how or if to display it; normally not shown).
- Missing rows display as spaces (D.1.1).

### X/26 (Level 1 must tolerate)
- Format: designation code (DC) 0–15, then 13 Hamming 24/18 triplets (cl. 9.4, 9.4.1, p. 27–28).
- A Level 1 decoder ignores X/26 for display. It must still keep them from terminating the page or corrupting rows.
- Broadcasters put Level 1 fall-back characters in positions that X/26 overwrites. Quote: "In countries where Level 1 decoders exist, a suitable fall back character should be transmitted in character locations to be overwritten by data from packets X/26. In countries where there are no Level 1 decoders, an even parity character may be transmitted in these character locations." (Table 25, p. 67)
- Consequence: a Level 1 decoder may see parity errors deliberately placed in rows.

### X/27/0 editorial links (FLOF) (cl. 9.6.1, Fig. 12, Table 14, p. 37–39)
- Byte 6 is the DC (Ham 8/4), 0 for X/27/0. DCs 0000–0011 are editorial links.
- Bytes 7–42 are Ham 8/4, in 6 groups of 6 bytes = links 0–5. Byte 43 is the Link Control Byte (Ham 8/4). Bytes 44–45 are the CRC (8-bit data).
- "Bytes 43, 44 and 45 have this significance for packets X/27/0 only. These bytes are reserved in packets X/27/1, X/27/2 and X/27/3."
- Each link = "the same format as bytes 6 to 11 of a page header packet". Link N occupies bytes 6N+7 … 6N+12:

| Offset | Content |
|---|---|
| +0 | page units |
| +1 | page tens |
| +2 | S1 |
| +3 | S2 (data bits 1–3) + **M1** in data bit 4 (the C4 position) |
| +4 | S3 |
| +5 | S4 (data bits 1–2) + **M2**, **M3** in data bits 3, 4 (the C5, C6 positions) |

- Link sizes: "Relative magazine number: 3 bits; Page number: 8 bits; Page sub-code: 13 bits."
- Relative magazine: "The bits M1, M2, M3 … correspond to the control bits C4, C5 and C6 in the page header packet. They are used here to change the magazine number from that in byte 4 of this packet X/27. Setting any of these bits to '1' complements the corresponding magazine bit."
  - DERIVED: link_mag = (packet_mag XOR (M3<<2 | M2<<1 | M1)) using 3-bit magazine values, with 0 displayed as 8.
- Null values: "When no particular page number is to be specified, the page number FF is transmitted. When no particular page sub-code is to be specified, the page sub-code 3F7F is transmitted. When the page address XFF:3F7F is transmitted, no page is specified."
- Link Control Byte (byte 43):
  - data bits 1, 2, 3: "In the absence of any local Code of Practice, these bits should be set to '1'."
  - data bit 4: Display Row 24 flag (see above).
- CRC (p. 38–39):
  - 16-bit shift register, input = external input ⊕ stages 7, 9, 12, 16; initialised to all zeros.
  - 8 192 clock pulses over bytes 14–37 of X/0, then bytes 6–45 of X/1 … X/25 in ascending address order. That is 24 + 25×40 = 1024 bytes; DERIVED, so all 8 bits of each byte including the parity bit are clocked in.
  - "Any absent packets are considered to contain the character 'space' (2/0) throughout."
  - "For each byte, the bits are applied to the input in the order b8 to b1."
  - Transmission order of the CRC is "bits 9 to 16 followed by bits 1 to 8".
  - H.2.5: decoders "may optionally process the CRC".
- FLOF semantics (Annex H, p. 142–143):
  - Links 0–3 go to four keys coloured **red, green, yellow, cyan**, matching the prompts in X/24.
  - Link 4 (the fifth) is for additional automatic acquisition only.
  - Link 5 (the sixth) is the **Index** key and is not associated with an X/24 prompt.
  - "A decoder intended for use according to this Code of Practice shall process the linked page addresses of a packet X/27/0 to acquire pages automatically."
- FLOF fall-backs (H.4, p. 143):
  - No 8/30: assume multiplexed operation and initial page 100, with the sub-code treated as don't care and no status message.
  - No X/27 and no X/24: the Index key shows the initial page and the coloured keys are disabled.
  - X/27 but no X/24: at least an index link is provided.

### X/28 and M/29 (Level 1 relevance only)
- X/28/0 format 1, triplet 1 bits 1–4 = Page Function and bits 5–7 = Page Coding (Table 3, p. 29). Values:
  - Page Function 0000 = basic Level 1 page (LOP). 0001 = data broadcasting. 0010/0011 = GPOP/POP. 0100/0101 = GDRCS/DRCS. 0110 = MOT. 0111 = MIP. 1000–1011 = TOP BTT/AIT/MPT/MPT-EX. 1100 = trigger page.
  - Page Coding 000 = 7 bits + odd parity. Other values mean 8-bit, Hamming, etc.
  - A Level 1 decoder that parses this can avoid displaying non-text pages. Parsing needs Hamming 24/18 decoding. Level ticks show page function/coding at all levels (Table 1).
- The default G0/G2 designation plus national option (triplet 1 bits 8–14, Table 32) and the second G0 (triplet 1 bits 15–18 + triplet 2 bits 1–3, Table 33) sit in X/28/0 fmt 1, X/28/4, M/29/0 and M/29/4 (Table 4, p. 30; Tables 11 and 13, p. 36–37).
- Priority: page X/28/0 fmt 1 > X/28/4; M/29/0 > M/29/4; page data overrides magazine data (cl. 9.4.2.2, 9.5.1, 9.5.3).
- Table 25 (p. 67) ticks X/28/0 fmt 1 / M/29/0 G0 designation for **Levels 1.5, 2.5, 3.5 only, not Level 1**. X/28/1 / M/29/1 are L1.5 only. So a pure Level 1 decoder selects the national option from **C12–C14 alone** (E.9, p. 135: "Level 1 and 1.5 decoders are likely to select the national option sub-set … by inspecting only the C12, C13 and C14 control bits").
- "Equipment intended for operation with only a single group of character set options may ignore this data." (Table 25)
- Default when absent: Latin G0/G2, i.e. Table 32 value 0000XXX, with C12–C14 selecting the national sub-set (D.2.1(1), p. 133; C.4(1), p. 130).
- M/29 "can be transmitted at any time on any line in the service" (F.3, p. 138). It is not part of any page.

### Packet 8/30 format 1 (cl. 9.8.1, Fig. 15, Table 18, p. 43–44)
- DC (byte 6) = 0000 or 0001.
  - Bit 1: '0' = multiplexed (VBI only, cl. 4.1); '1' = non-multiplexed / full-field (cl. 4.2).
  - Bits 2–4 = 000 select format 1.
- **Initial Teletext Page**, bytes 7–12, all Ham 8/4:

| Byte | Data bits | Content |
|---|---|---|
| 7 | 1–4 | page units (LSB–MSB) |
| 8 | 1–4 | page tens |
| 9 | 1–4 | S1 |
| 10 | 1–3 | S2 |
| 10 | 4 | magazine bit, weight 2^0 |
| 11 | 1–4 | S3 |
| 12 | 1–2 | S4 |
| 12 | 3–4 | magazine bits, weights 2^1, 2^2 |

  - The magazine is **absolute** here, not relative. FF / 3F7F = none specified (note 2).
- **Network Identification**, bytes 13–14, 8-bit data: "The 16 bit NI value is transmitted most significant bit first. Thus the MSB is mapped to byte 13, bit 1 and the LSB to byte 14, bit 8."
  - DERIVED: because bytes are otherwise read LSB-first, the 16 bits must be bit-reversed after reading the two bytes normally.
  - NI codes are allocated in TS 101 231. If no NI is allocated, bytes 13–14 are all 0 (A.9(2), p. 124).
- **Time Offset Code**, byte 15, 8-bit data:
  - bit 1: reserved;
  - bits 2–6: offset in units with weights ½, 1, 2, 4, 8 hours, i.e. a half-hour count;
  - bit 7: polarity, '0' positive, '1' negative ("Negative offsets are west of Greenwich");
  - bit 8: reserved.
  - DERIVED: offset_half_hours = (b15 >> 1) & 0x1F; negative if b15 & 0x40.
- **Modified Julian Date**, bytes 16–18, 8-bit data: "A 5-digit (decimal) number … incrementing daily at midnight UTC. Reference point is 31 January 1982, MJD 45000. **Each digit is incremented by one prior to transmission.** Pairs of 4-bit values are assembled into bytes and the bytes are transmitted least significant bit first." Layout (read from PDF p. 44):
  - byte 16 bits 5–8 reserved; bits 1–4 = 10^4 digit
  - byte 17 bits 5–8 = 10^3; bits 1–4 = 10^2
  - byte 18 bits 5–8 = 10^1; bits 1–4 = 10^0
  - (each nibble LSB–MSB, each digit +1)
- **UTC**, bytes 19–21, 8-bit data: "6-digit number… The transmission relates to the next following second. Each digit is incremented by one prior to transmission."
  - byte 19: bits 5–8 hours tens, bits 1–4 hours units
  - byte 20: minutes tens / units
  - byte 21: seconds tens / units
- Bytes 22–25 reserved.
- **Status Display**, bytes 26–45: 20 odd-parity characters from the default G0 set ("The use of national option characters is not recommended"), e.g. the programme title.
- Transmission rules (B.5, p. 126):
  - format 1 "shall be transmitted within the VBI immediately following the boundary between clock-seconds";
  - up to 4 format 2 packets per second may be interleaved;
  - any two 8/30 packets are at least 200 ms apart.
- E.14 (p. 136): if no true real-time clock is in the headers, 8/30 fmt 1 should carry correct UTC at least once per second.
- A.9(1) (p. 124): when both formats are present, bytes 7–12 and 26–45 shall be identical.

### Packet 8/30 format 2 (cl. 9.8.2, Fig. 16, Table 19, p. 45–46)
- DC = 0010 or 0011: bit 1 = multiplex flag as in format 1; bits 2–4 "When set to 100" select format 2 (bit 2 = 1).
- Bytes 7–12: Initial Teletext Page, identical coding to format 1.
- Bytes 13–25: Programme Identification Data for PDC; "Function and coding is defined in clause 8.2.1 of EN 300 231".
- Bytes 26–45: Status Display (odd parity).

---------------------------------------------------------------------------------------------------

## 7. Level 1 display

### Spacing attributes: Table 26 (cl. 12.2, p. 70–74). Level ticks read from the PDF images.
General rules (p. 70):
- "Set of 32 spacing attributes, four without response at Levels 1 and 1.5, transmitted via packets X/0 to X/25." The four are 0/0, 0/E, 0/F and 1/0.
- "The decoder defaults to specified attributes at the start of each display row."
- "Some attributes have effect immediately ('Set-At'), others at the following character-space ('Set-After')."
- "The action of an attribute persists until the end of a row or until the transmission of a further attribute that modifies its action."
- "Unless operating in 'Hold Mosaics' mode, each character space occupied by a spacing attribute is displayed as a SPACE."

| Code | Name | Set | L1 | L1.5 | 2.5 | 3.5 | Function (essentials) |
|---|---|---|---|---|---|---|---|
| 00 | Alpha Black | After | ✗ | ✗ | ✓ | ✓ | "should be used with caution as it is interpreted by some existing Level 1 and Level 1.5 decoders" |
| 01–07 | Alpha Red, Green, Yellow, Blue, Magenta, Cyan, White | After | ✓ | ✓ | ✓ | ✓ | set foreground colour and "select a G0 set as the current default character set". Persist until the next alpha/mosaic colour code or row start. **07 Alpha White = start-of-row default.** |
| 08 | Flash | After | ✓ | ✓ | ✓ | ✓ | "causes the foreground pixels of the following alphanumeric and mosaics characters to alternate between the foreground and background colours." Cancelled by Steady or new row. |
| 09 | Steady | **At** | ✓ | ✓ | ✓ | ✓ | cancels flash. **Default.** |
| 0A | End Box | After | ✓ | ✓ | ✓ | ✓ | cancels Start Box. **Default.** |
| 0B | Start Box | After | ✓ | ✓ | ✓ | ✓ | "On pages with the C5 or C6 bits set (Newsflash or subtitle), this code defines (on each appropriate row) the start of an area that is to be boxed into the normal video picture. Characters outside this area are not displayed, but changes in display mode, colour, height etc., will affect the boxed area. Cancelled by an End Box code (0/A) or by the start of a new row." NOTE: "Protection against false operation is provided by double transmission of Start Box control characters, with the action taking place between them." |
| 0C | Normal Size | **At** | ✓ | ✓ | ✓ | ✓ | cancels double height/width/size. **Default.** |
| 0D | Double Height | After | ✓ | ✓ | ✓ | ✓ | see below |
| 0E | Double Width | After | ✗ | ✗ | ✓ | ✓ | "interpreted by some existing Level 1 and Level 1.5 decoders"; must not be placed in the last two columns or just before a box edge |
| 0F | Double Size | After | ✗ | ✗ | ✓ | ✓ | NOTE 9: some L1/L1.5 decoders "may interpret it as Double Height" |
| 10 | Mosaics Black | After | ✗ | ✗ | ✓ | ✓ | caution note as for 00 |
| 11–17 | Mosaics Red…White | After | ✓ | ✓ | ✓ | ✓ | set foreground colour and "select the G1 set as the current default character set" |
| 18 | Conceal | **At** | ✓ | ✓ | ✓ | ✓ | "The following characters up to the end of the row, or until a Colour Code attribute (codes 0/0 to 0/7 or 1/0 to 1/7) is encountered, are to be displayed as SPACES until revealed by a decoder or user operation." |
| 19 | Contiguous Mosaic Graphics | **At** | ✓ | ✓ | ✓ | ✓ | "The blocks of a mosaics character adjoin one another." **Default.** |
| 1A | Separated Mosaic Graphics | **At** | ✓ | ✓ | ✓ | ✓ | "Each block of a mosaics character is surrounded by a border of the background colour." |
| 1B | ESC (or Switch) | After | ✓ | ✓ | ✓ | ✓ | "Toggles between the first and second G0 sets defined by packets X/28/0 Format 1, X/28/4, M/29/0 or M/29/4. The default at the start of each row is the default G0 set. This does not affect characters written via packets X/26 or as part of an object." NOTE 13: "May also be used in connection with data for processing." |
| 1C | Black Background | **At** | ✓ | ✓ | ✓ | ✓ | "immediately changes the background colour to the colour at entry 0 of the background CLUT, or to the current Full Row colour if black background colour substitution is in operation. The foreground colour is not affected." **Default.** |
| 1D | New Background | **At** | ✓ | ✓ | ✓ | ✓ | "causes a new background colour to be selected immediately. Any following characters and mosaics may be invisible until a different foreground colour attribute is encountered." Level 1/1.5 row: "The foreground colour currently selected is adopted as the background colour." (2.5/3.5 row: CLUT from the re-mapping bits.) |
| 1E | Hold Mosaics | **At** | ✓ | ✓ | ✓ | ✓ | see below |
| 1F | Release Mosaics | After | ✓ | ✓ | ✓ | ✓ | "cancels the Hold Mosaics mode." **Default.** |

Notes on the table:
- 1B ESC is ticked at **all** levels, including Level 1. A pure Level 1 decoder does not parse X/28 or M/29, so the second G0 set can come only from a local Code of Practice (cl. 15.1, p. 100: two G0 sets "are implied by a local Code of Practice or, with Level 2.5 and 3.5 transmissions, they are defined formally via packets X/28 and M/29").
- Table 33 value 1111111 = "No second G0 set required"; a decoder "may interpret this value to mean that the ESC function should be disabled" (cl. 15.3, p. 102).
- DERIVED: with no second set known, ESC displays as a space and otherwise has no effect.

### Start-of-row defaults
Collected from Table 26 "Start-of-row default condition" markings, plus the colour/set rules:
- Alpha White (G0, white foreground)
- Steady
- End Box (box off)
- Normal Size
- Contiguous Mosaics
- Black Background
- Release Mosaics
- Implicitly:
  - not concealed (Conceal persists only "up to the end of the row");
  - held-mosaic character = SPACE;
  - default G0 set (not the ESC-switched set).
- Explicit confirmation, cl. 12.5.4(1) (p. 91): "The default attributes settings at the start of each Side Panel row are those used at the start of a Level 1 text row, i.e. white foreground (CLUT 0, entry 7), normal size, steady, boxing off, not concealed, contiguous mosaic graphics." At Level 1 the background default is black (1/C).

### Hold Mosaics (1/E), verbatim (p. 74)
"Generally, all spacing attributes are displayed as spaces, implying at least one space between characters or mosaics with different colours in the same row. In mosaics mode, the 'Hold Mosaics' option allows a limited range of attribute changes without intervening spaces. A mosaic character from the G1 set (referred to as the 'Held-Mosaic' character) is displayed in place of the character 'SPACE' corresponding to a control character. Substitution only takes place in mosaics mode when Hold Mosaics mode is in force. At a screen location where substitution is permitted, the 'Held-Mosaic' character inserted is the most recent mosaics character with bit 6 = '1' in its code on that row. The 'Held-Mosaic' character is reset to 'SPACE' at the start of each row, on a change of alphanumeric/mosaics mode or on a change of size. It is not reset by reinforcement of the existing size setting. It is not reset by a change in Hold Mosaics mode. The 'Held-Mosaic' character is always displayed in its original contiguous or separated form regardless of the mode prevailing at the time of substitution."

- DERIVED: bit 6 = weight 0x20. Codes 0x20–0x3F and 0x60–0x7F in mosaic mode are held. Blast-through capitals 0x40–0x5F (bit 6 = 0) are **not** adopted as the held character.
- Worked example, G.3.3 (p. 141), sequence `17 66 1E 39 1A 1F 66` from the start of a row:
  - 17: displayed as black background (a space);
  - 66: contiguous mosaic, becomes the held character;
  - 1E: Hold, so the cell shows held 66;
  - 39: contiguous, becomes held;
  - 1A: the cell shows held 39;
  - 1F: Release is Set-After, so this cell still shows held 39;
  - 66: displayed separated, and becomes held in separated form.
  - "Although the Separated Mosaic Graphics code normally becomes active on the next character, in this case it only takes effect when the next new mosaics character is encountered."
  - Note that the spec itself calls 1/A "Set-At" in Table 26. The example shows that the held character keeps its stored contiguous or separated form.
- G.3.2 (p. 140) separated example: `17 39 1A 39` shows the first 39 contiguous and the second separated.
- DERIVED timing rules for implementers:
  - Set-At codes (09, 0C, 18, 19, 1A, 1C, 1D, 1E) apply to the attribute's own cell. For 1E, the cell of the Hold code itself shows the held mosaic.
  - Set-After codes (colours, 08, 0A, 0B, 0D, 1B, 1F) apply from the next cell. The attribute's own cell is shown with the previous state; for 1F in hold mode, that cell still shows the held character.
  - A change of alpha/mosaic mode caused by a Set-After colour code resets the held character. When the reset takes effect relative to the attribute cell is not spelled out (UNVERIFIED). The common implementation resets on the code's cell after it is drawn.
- NOTE 14: 1/E, 1/F, 1/9 and 1/A "ha[ve] effect only on characters defined at Levels 1 and 1.5".

### Double Height (0/D) (Table 26, p. 71; C.3, p. 129–130)
- "The characters and mosaics following a double height code are stretched into the following row. The origin of a character is the upper character position. The whole of an enlarged character is displayed with the attributes that apply to the origin of the character."
- "When double height (or double size) characters are used on a given row, the row below normal height characters on that row is displayed with the same local background colour and no foreground data. **Any transmitted Level 1 characters and attributes for the lower row are ignored.**"
- NOTE 1: "The broadcaster should not insert double height control characters in rows 23 or 24."
- C.3 rule 1: the transmission "should inhibit the transmission of double height/double size attributes in rows 0, 23 and 24… If a decoder receives an illegal combination, it may choose, for example, to display a partial character, to reduce the size or to ignore the character completely."
- The X/26 row-0 triplet section also says "The use of double height and double size attributes in row 0 is not permitted." (cl. 12.3.3, row address triplet mode 00111, p. 78)
- C.3 rule 2: "Priority is given in scan order… with earlier encountered size attributes taking precedence over later ones." Consequence (DERIVED): the row after a row containing double height is entirely replaced (its own content is ignored). A double-height code on that ignored row therefore has no effect.
- C.3 rule 4: "All attributes applying to the origin of an enlarged character shall also apply to all parts of that character."
- NOTE 2: one size code terminates any other size attribute. The Held-Mosaic is reset "on a change of size" but "not … by reinforcement of the existing size setting".

### Flash
- Table 26, 0/8: foreground pixels alternate between the foreground and background colours.
- The flash rate/period is **not specified** for Level 1 in EN 300 706 (UNVERIFIED; X/26 "Additional Flash Functions" apply to Level 2.5+ only).

### Conceal / Reveal
- Table 26, 1/8. Reveal is "a decoder or user operation". The spec gives no further rule (e.g. reset of reveal on page change is not specified).

### Boxing, Newsflash, Subtitle, "mix"
- Start/End Box apply on C5/C6 pages (Table 26). Only boxed areas are displayed; elsewhere video shows (Table 2 C5/C6 "boxed… inset into the normal video picture").
- Table C.1 (p. 128), for opaque foreground/background (the Level 1 case):
  - Newsflash/Subtitle bit = 0 gives the F/B colour whether boxed or not;
  - bit = 1 and not boxed gives **Video**;
  - bit = 1 and boxed gives the F/B colour.
- G.3.1 (p. 140) example `x 0B 0B 61 0A 0A`: the boxed area covers the second 0B, the 'a', and the first 0A (both Set-After). "changes of colour (foreground and/or background) will affect the boxed characters."
- E.13 (p. 136): Start/End Box "should not be used on level 1/1.5 pages that do not have the Newsflash or Subtitle bits set".
- "Mix" mode: the only mention is G.1 (p. 140), "Existing decoders operating in 'mixed' mode may also display the foreground of non-boxed characters." It is not otherwise defined; a decoder feature (UNVERIFIED semantics).

### Background attributes
- 1/C Black Background: Set-At, background = black (at Level 1 CLUT 0 entry 0).
- 1/D New Background: Set-At, background := current foreground colour.

### Blast-through alphanumerics
- G1 (Table 47 notes, p. 118; Table 25 G1 row, p. 67): "The alphanumeric characters at positions 4/0 to 5/F are taken from the G0 set, applying the same rules to national option positions as described above."
- NOTE 2 of Table 47: "When addressed from the Level 1 page, the characters at the shaded positions in columns 4 and 5 are taken from the national option sub-set currently selected via the C12, C13 and C14 control bits and any 'ESC' commands in force at that time." (Shaded: 4/0, 5/B–5/F.)

### Row 0
- C7 suppresses row 0. The spec does not say whether C10 (Inhibit Display) suppresses row 0; C10 covers rows 1–24 only.
- Row 0 shows header bytes 14–45 in columns 8–39 (see §4). The same Level 1 attribute rules apply (Table 26: attributes are "transmitted via packets X/0 to X/25").
- Double height is not allowed in row 0 (C.3).

### Row 24
- Displayed only with the X/27/0 flag (see §6).
- Double height must not be used in row 24 (Table 26 NOTE 1, C.3).
- E.7(2) (p. 135): "many decoders will display row 24 below rows 0 to 11 when operating in 'Page Expansion' mode."

### Enlargement / reveal / hold keys
- The spec does not define decoder user functions (hold, reveal, double-height page expansion) beyond the E.7 mention. They are decoder choices.

---------------------------------------------------------------------------------------------------

## 8. Character sets for Level 1

### G0 Latin primary set (Table 35, p. 106)
- The set has 96 positions, 2/0–7/F; code = b7..b1 with column = b7b6b5 and row = b4..b1. It is ASCII-like, except:
  - 2/0 = SPACE (NOTE 1);
  - 7/F = a solid block ("occupies an area equivalent to that of any character which does not include a descender. It is thus a rectangle surrounded by the background colour", NOTE 4).
- Shaded (national-option) positions: **2/3, 2/4, 4/0, 5/B, 5/C, 5/D, 5/E, 5/F, 6/0, 7/B, 7/C, 7/D, 7/E** (13 positions).
- The base glyphs shown at those positions (#, ¤, @, [, \, ], ^, _, `, {, |, }, ~) "apply when the set is accessed via packets X/26". "Characters from a national option sub-set from table 36 occupy these positions when the set is addressed directly from a Level 1 page." (NOTE 2, p. 106)
- Table 25 (p. 67): substitution "only occurs when a G0 set character is referenced from packets X/1-X/25 of the Level 1 page. It does not apply when characters are referenced via packets X/26 or objects." X/0 is also Level 1 page data, so the national option applies in the header too (DERIVED from "packets X/0 to X/25" in Table 25).
- 15.6 (p. 105): "the precise appearance of the characters… is a decoder option. The characters shown are based on a matrix of 12 pixels horizontally and 10 pixels vertically."

### Table 36: Latin National Option Sub-sets (p. 107)
Transcribed from a 200 dpi rendering of the PDF. Unicode is my identification of the glyphs.

| Sub-set | 2/3 | 2/4 | 4/0 | 5/B | 5/C | 5/D | 5/E | 5/F | 6/0 | 7/B | 7/C | 7/D | 7/E |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Czech/Slovak | # | ů | č | ť | ž | ý | í | ř | é | á | ě | ú | š |
| English | £ | $ | @ | ← | ½ | → | ↑ | # | ― (horiz. bar) | ¼ | ‖ | ¾ | ÷ |
| Estonian | # | õ | Š | Ä | Ö | Ž | Ü | Õ | š | ä | ö | ž | ü |
| French | é | ï | à | ë | ê | ù | î | # | è | â | ô | û | ç |
| German | # | $ | § | Ä | Ö | Ü | ^ | _ | ° | ä | ö | ü | ß |
| Italian | £ | $ | é | ° | ç | → | ↑ | # | ù | à | ò | è | ì |
| Lettish/Lithuanian | # | $ | Š | ė | ę | Ž | č | ū | š | ą | ų | ž | į |
| Polish | # | ń | ą | Ƶ (Z-stroke) | Ś | Ł | ć | ó | ę | ż | ś | ł | ź |
| Portuguese/Spanish | ç | $ | ¡ | á | é | í | ó | ú | ¿ | ü | ñ | è | à |
| Rumanian | # | ¤ | Ţ | Â | Ş | Ă | Î | ı | ţ | â | ş | ă | î |
| Serbian/Croatian/Slovenian | # | Ë | Č | Ć | Ž | Đ | Š | ë | č | ć | ž | đ | š |
| Swedish/Finnish(/Hungarian) | # | ¤ | É | Ä | Ö | Å | Ü | _ | é | ä | ö | å | ü |
| Turkish | ₺ ("TL" currency) | ğ | İ | Ş | Ö | Ç | Ü | Ğ | ı | ş | ö | ç | ü |

Table 36 notes:
- NOTE 1: "The character TL is the Turkish currency symbol."
- NOTE 2: "In some decoders the Ë at 2/4 of the Serbian/Croatian/Slovenian set has been replaced by either $ or ½."
- NOTE 3: "The Swedish/Finnish sub-set is also used for Hungarian."

Glyph identification caveats: glyphs are identified visually from 12×10 bitmaps. Items to double-check against the ETS 1997 edition or another source:
- Polish 5/B (Z with stroke, Ƶ);
- Polish 7/D (ł);
- English 6/0 (a long horizontal bar, U+2015 or similar);
- English 7/C (double vertical bar ‖).

### C12–C14 selection and Table 32 (cl. 15.2, p. 100–102)
- At Levels 1 and 1.5 "the national option sub-set in use on the page is defined by the C12, C13 and C14 control bits in the page header alone and, in theory, this will result in an ambiguous reference to an entry in table 32" (p. 100).
- Resolving the ambiguity needs the group (triplet 1 bits 11–14) from X/28/M/29, or a local Code of Practice.
- NOTE 1: "bit 10 = C12, bit 9 = C13 and bit 8 = C14". Table columns are listed "C12 C13 C14", left to right.
- DERIVED: sub-set index = C12·4 + C13·2 + C14·1, as written in the table.
  - Implementation caution (UNVERIFIED): check this bit-weighting against real captures or reference decoders. The header bit positions are C12 = byte 13 bit 4, C13 = bit 6, C14 = bit 8.

Table 32 (triplet 1 bits 14 13 12 11 = group; bits 10 9 8 = C12 C13 C14):

| Group bits 14–11 | G0 / G2 | 000 | 001 | 010 | 011 | 100 | 101 | 110 | 111 |
|---|---|---|---|---|---|---|---|---|---|
| 0000 | Latin / Latin | English | German | Swedish/Finnish/Hungarian | Italian | French | Portuguese/Spanish | Czech/Slovak | – |
| 0001 | Latin / Latin | Polish | German | Swe/Fin/Hun | Italian | French | – | Czech/Slovak | – |
| 0010 | Latin / Latin | English | German | Swe/Fin/Hun | Italian | French | Portuguese/Spanish | Turkish | – |
| 0011 | Latin / Latin | – | – | – | – | – | Serbian/Croatian/Slovenian | – | Rumanian |
| 0100 | mixed | Cyrillic-1 / Cyrillic (Serbian/Croatian) | Latin German | Latin Estonian | Latin Lettish/Lithuanian | Cyrillic-2 / Cyrillic (Russian/Bulgarian) | Cyrillic-3 / Cyrillic (Ukrainian) | Latin Czech/Slovak | – |
| 0101 | Reserved | | | | | | | | |
| 0110 | – | – | – | – | – | – | – | Latin/Latin Turkish | Greek/Greek (Greek) |
| 0111 | Reserved | | | | | | | | |
| 1000 | Latin / Arabic (Arabic/Arabic for 111) | English | – | – | – | French | – | – | Arabic (Arabic) |
| 1001 | Reserved | | | | | | | | |
| 1010 | – | – | – | – | – | – | Hebrew / Arabic (Hebrew) | – | Arabic / Arabic (Arabic) |
| other | Reserved | | | | | | | | |

Table 32 notes:
- NOTE 2: "Entries in the table with no character set or national option sub-set specified are reserved."
- NOTE 3: bracketed options have no separate sub-set; "The characters are contained within the appropriate G0 set."
- NOTE 1 also says that for M/29 these bits "should be set to the most common value of C12, C13 and C14 used in that magazine".

### Table 33: Second G0 set (triplet 2 bits 3 2 1, triplet 1 bits 18 17 16 15) (p. 103–104)
The 7-bit value is laid out like Table 32. The high 4 bits are (t2b3, t2b2, t2b1, t1b18) and the low 3 bits are t1b17..15.

| High 4 bits | 000 | 001 | 010 | 011 | 100 | 101 | 110 | 111 |
|---|---|---|---|---|---|---|---|---|
| 0000 | Latin English | Latin German | Latin Swe/Fin/Hun | Latin Italian | Latin French | Latin Port/Span | Latin Czech/Slovak | – |
| 0001 | Latin Polish | Latin German | Latin Swe/Fin/Hun | Latin Italian | Latin French | – | Latin Czech/Slovak | – |
| 0010 | Latin English | Latin German | Latin Swe/Fin/Hun | Latin Italian | Latin French | Latin Port/Span | Latin Turkish | – |
| 0011 | – | – | – | – | – | Latin Serbian/Croatian/Slovenian | – | Latin Rumanian |
| 0100 | Cyrillic-1 (Serbian/Croatian) | Latin German | Latin Estonian | Latin Lettish/Lithuanian | Cyrillic-2 (Russian/Bulgarian) | Cyrillic-3 (Ukrainian) | Latin Czech/Slovak | – |
| 0101 | Reserved | | | | | | | |
| 0110 | – | – | – | – | – | – | Latin Turkish | Greek (Greek) |
| 0111 | Reserved | | | | | | | |
| 1000 | Latin English | – | – | – | Latin French | – | – | Arabic (Arabic) |
| 1001 | Reserved | | | | | | | |
| 1010 | – | – | – | – | – | Hebrew (Hebrew) | – | Arabic (Arabic) |
| 1111111 | "No second G0 set required" | | | | | | | |

Table 33 rules (cl. 15.3, p. 102):
- The second set is reached "following the first ESC character within a display row. Subsequent ESC characters cause the decoder to alternate…"
- "The national option sub-set selected by the C12, C13 and C14 bits is not relevant to the secondary set."
- "This information is unlikely to be interpreted by Level 1 or 1.5 decoders."
- The G2 set is not affected by ESC.

### G1 block mosaic set (Table 47, p. 118; Table 26 1/0–1/7 text, p. 73)
- "Fixed set of 63 block mosaic characters plus SPACE (2/0) and 32 alphanumeric characters… Address range 2/0 to 7/F." (Table 25, p. 67)
- Bit-to-cell mapping (Table 26, p. 73): "For subsequent characters in columns 2, 3, 6 & 7 of the G1 set, bits 1, 2, 3, 4, 5 & 7 respectively set the top left, top right, middle left, middle right, lower left and lower right graphic blocks of each character."

| Bit | Weight | Cell |
|---|---|---|
| b1 | 0x01 | top left |
| b2 | 0x02 | top right |
| b3 | 0x04 | middle left |
| b4 | 0x08 | middle right |
| b5 | 0x10 | bottom left |
| b7 | 0x40 | bottom right |
| b6 | 0x20 | always 1 in mosaic columns 2, 3, 6, 7; 0 in blast-through columns 4, 5 |

- This agrees with the Table 47 drawings, e.g. 2/1 = top-left only, 7/F = all six blocks. Columns 4 and 5 are "(1)", G0 characters (NOTE 1).
- NOTE 4: "The character in position 2/0 is to be interpreted as a mosaics SPACE." NOTE 5: "The dark areas… are displayed in the foreground colour." NOTE 7: "Bit allocations are as for table 35."
- Contiguous vs separated:
  - Contiguous: "The blocks of a mosaics character adjoin one another."
  - Separated: "Each block of a mosaics character is surrounded by a border of the background colour" (Table 26, p. 73).
  - Table 47 NOTE 6 gives example drawings of both forms.
- **The spec does not give pixel geometry** for the 2×3 cell split or the separation gap. With the 12×10 nominal matrix (cl. 15.6), exact row heights (e.g. 3/4/3) and gap widths are decoder choices (UNVERIFIED; not specified). G.2 (p. 140): "The precise appearance of all characters and mosaics … is a decoder option."

---------------------------------------------------------------------------------------------------

## 9. Colours for Level 1

- Table 25, Character Colours row (p. 69), ticked at all levels:
  - "8 background full intensity colours: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White."
  - "7 foreground full intensity colours: Red, Green, Yellow, Blue, Magenta, Cyan, White. Invoked as spacing attributes via codes in packets X/0 to X/25."
  - "Black foreground" is ticked at 2.5/3.5 only.
- DERIVED: at Level 1 a black background is reachable via 1/C, or via 1/D after a colour code. A black foreground is not reachable, because 00/10 have no response.
- Table 30, Colour Map CLUT 0 (cl. 12.4, p. 89), 4-bit R,G,B ("Fixed at Levels 1, 1.5 and 2.5"):

| Entry | Colour | R | G | B |
|---|---|---|---|---|
| 0 | Black | 0 | 0 | 0 |
| 1 | Red | 15 | 0 | 0 |
| 2 | Green | 0 | 15 | 0 |
| 3 | Yellow | 15 | 15 | 0 |
| 4 | Blue | 0 | 0 | 15 |
| 5 | Magenta | 15 | 0 | 15 |
| 6 | Cyan | 0 | 15 | 15 |
| 7 | White | 15 | 15 | 15 |

- NOTE: "The individual R, G, B levels are variable in 16 equally spaced steps. A value of 0 represents zero intensity and a value of 15 (decimal) represents full intensity. The levels are not gamma corrected."
- The colour-code low 3 bits = CLUT entry (bit0 = R, bit1 = G, bit2 = B; DERIVED from the table). The 3 LSBs give the entry: "The entry in the selected CLUT is specified by the 3 LSBs of the code for the spacing colour attribute" (Table 4, p. 30).

---------------------------------------------------------------------------------------------------

## 10. Presentation level definitions (cl. 12, p. 66; cl. 10.1, p. 46–47)

- Level 1: "defines the basic Teletext page, characterized by the use of spacing attributes only and a limited alphanumeric and mosaics repertoire."
- Level 1.5: "decoder responds as Level 1 but the character repertoire is extended via packets X/26."
- Level 2.5: "extends the fixed alphanumeric and mosaic repertoire and increases the number of display colours. Sixteen of the 32 colours are re-definable. The number of character spaces per row can be increased to 56… non-spacing display attributes and a limited number of redefinable characters."
- Level 3.5: more DRCS; bold, italic, proportional spacing; all 32 colours redefinable.
- cl. 10.1 Fig. 17 (p. 46): Level 1/1.5 components are:
  - LOP (X/0–X/25);
  - broadcast service data 8/30 fmt 1 and fmt 2;
  - optional Level 1.5 enhancements (X/26/0–15, X/28/1, M/29/1).
- At Level 1.5 only "a limited number of functions from packets X/26 and X/28/1 are used": G0+diacritics via G2, a few G2/G3 characters, PDC. "Some existing Level 1.5 decoders only respond to packets X/26 with designation codes in the range 0 to 14."
- Level 1 column ticks, Table 25 (p. 67–69):
  - ✓: Display rows/columns; Character bytes (7 bits + parity); G0 set with national options via C12–C14 and ESC toggling; G1 set; Character colours (8 bg / 7 fg).
  - ✗ at L1: side panels; X/26 fall-back handling; G0 designation via X/28/1 (L1.5 only) or X/28/0/M/29/0 (L1.5+); G2 (L1.5+); G3 (L1.5+); bold/italic; DRCS; objects; black foreground; CLUTs; row/screen colours.
- Table 1 Level 1 ticks (packet types) are listed in §3.

---------------------------------------------------------------------------------------------------

## 11. Annex items relevant to Level 1 (summary with cites)

- A.1 (p. 120): page numbering, FF / 3F7F rules, sub-page coding 0001–0079. See §4 and §5.
- A.2 (p. 121): C4 and C8 semantics.
- A.3 (p. 121): X/26 designation codes start at 0 and ascend contiguously. Level 1.5 may ignore X/26/15. (Level 1 just skips.)
- A.9 (p. 124): both 8/30 formats carry identical bytes 7–12 and 26–45; NI = 0 if none allocated.
- A.10 (p. 124): reserved page numbers 1BE, 1F0, mFD, mFE, mFF.
- B.1 (20 ms erase interval), B.2 (packet order), B.3/B.4 (serial/parallel), B.5 (8/30 timing), B.9 (100 ms max gap) (p. 125–127).
- C.1 / Table C.1 (p. 128): boxing vs Newsflash/Subtitle vs video.
- C.3 (p. 129–130): size-attribute rules, priorities, illegal rows 0/23/24 and column 39.
- D.1.1 (p. 131): missing rows = spaces.
- D.1.2 (p. 131): with no X/26, a Level 1.5 decoder assumes all characters come from X/1–X/24.
- D.2.1 (p. 133): defaults are Latin G0/G2 with C12–C14 national option, black screen and row, no re-mapping.
- D.3 (p. 133): page 100 is the introductory page when there is no 8/30; the transmission is VBI-only when there is no 8/30/1 or 8/30/3.
- E.2 (p. 134): page time-code in the sub-code.
- E.3 (p. 134): Level 1.5 X/26 content is typically G3 5/1, 5/B–5/D; G2; G0; G0 plus diacritic. Many decoders do not respond to X/26/15.
- E.7 (p. 135): the first 8 header cells are reserved by some decoders; Page Expansion shows row 24 below rows 0–11.
- E.9 (p. 135): Level 1/1.5 decoders use only C12–C14.
- E.13 (p. 136): Start/End Box only on C5/C6 pages.
- E.14 (p. 136): header clock format HH:MM:SS in the last 8 bytes.
- E.15 (p. 137): 100 is the Home page, plus the other recommended numbers.
- F.2 (p. 138): re-send whole Level 1 pages after an interruption.
- F.4 (p. 138): lines 6/318/319.
- F.5 (p. 138): clock on lines 22/335.
- F.6 (p. 139): full-field transmission cautions (no C4, no row-adaptive).
- G.1 (p. 140): mixed mode.
- G.2 (p. 140): appearance is a decoder option.
- G.3 (p. 140–141): worked examples for box, separated, hold.
- H (p. 142–143): FLOF Code of Practice. See §6.
- O (p. 152): EN 300 706 + EN 300 708 replace EBU SPB 492 (Dec 1992).
- P (p. 153): non-Teletext VBI signals and decoder robustness.

---------------------------------------------------------------------------------------------------

## 12. TOP (Table Of Pages)

- **EN 300 706 mentions TOP but does not fully define it.**
- cl. 11.2.1 (p. 60): "The TOP Code of Practice (see annex I) defines a user-friendly page access method… categorize the pages … according to specific themes and sub-themes… transmitted via a number of special pages."
- cl. 11.2.2 (p. 61) lists the four TOP data page types and their coding only:
  - **BTT**: X/1–X/23 all Hamming 8/4, page number **fixed at 1F0**;
  - **AIT**: per packet, bytes 6–13 and 26–33 Ham 8/4, bytes 14–25 and 34–45 odd parity; page number from the BTT;
  - **MPT** and **MPT-EX**: all Ham 8/4; page numbers from the BTT.
  - Each type uses packets Y = 0–23 only, plus optional X/28/0 fmt 1. Page Function codes are 1000 BTT, 1001 AIT, 1010 MPT, 1011 MPT-EX (Table 3, p. 29).
  - Fig. 24 (p. 61) shows the BTT indexed by page numbers 100–899. **The table contents or semantics (block/group/normal codes, byte layouts) are not given.**
- Annex I (informative, p. 144–145):
  - "The specification for the Table Of Pages (TOP) system can be found in the document: Institut für Rundfunktechnik, ARD/ZDF Technical Guidelines, No. 8 R 5, 2nd Edition (2nd December 1991): 'TOP' System for Teletext'." (Also in Annex R, Bibliography.)
  - It describes the user model: keys "+", red (next group), blue (next block), "−" (back), and a decoder-generated 25th commentary row.
  - "The combination of TOP and parallel transmissions for Level 1 and 1.5 services is not recommended…"
- E.15 (p. 137): TOP AIT/MPT/MPT-EX commonly at 1F1–1F4, assigned via the BTT. A.10: 1F0 reserved for the BTT.
- **EN 300 708 (ETS 300 708, March 1997, specifications/ets_300708e01p.pdf):** a full-text search (pdftotext, 1911 lines) for "TOP", "Table of pages", "BTT" and "navigat" found **no mention of TOP**. EN 300 708 covers data broadcasting only.
- Conclusion: a TOP implementation needs IRT/ARD-ZDF guideline 8 R 5, which is not in the local specification set.

---------------------------------------------------------------------------------------------------

## Gaps / UNVERIFIED summary

1. Parity-error and Hamming double-error handling policy beyond "reject data bits" is not specified.
2. Page-number rendering in header columns 0–7 is not specified; E.7 says only that some decoders reserve them.
3. Flash rate, reveal lifetime, and "mix" mode semantics are not specified.
4. Separated-mosaic pixel geometry and the 2×3 row split are not specified ("decoder option").
5. Exact Set-After timing of held-mosaic reset on a mode change is only implied (G.3.3 example).
6. The bit-significance reading of C12/C13/C14 as a 3-bit index follows the table literally (C12 = MSB) and should be confirmed against real broadcasts or reference decoders.
7. Some national-option glyph identifications (Polish 5/B, 7/D; English 6/0, 7/C) are visual reads of low-resolution bitmaps.
8. The C11 wording in Table 2 conflicts with the page-termination wording in cl. 7.2.1 and B.3 (see §4).
9. X/25 display behaviour is not specified beyond "displayable labels … for key-word search".
10. Clock byte range: 38–45 (cl. 9.3.1.4) vs 39–45 (F.5).
11. The older ETS 300 706 (1997) was not cross-checked in detail.
