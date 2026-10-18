# SAA5050 teletext character generator — cited technical notes

Written as input to a requirements document for a C software teletext decoder that uses the SAA5050 as its baseline for Level 1 rendering.

## Sources and citation keys

| Key | Document | Page scheme used in citations |
|---|---|---|
| **[S]** | `teletext-fonts/manufacturers/philips/datasheets/saa5050/SAA5050-55_Teletext_Character_Generator_Philips.pdf`: a Signetics "Linear LSI Products" datasheet, *Teletext Character Generator SAA5050/55*. It is an ABBYY OCR of a scan, 17 PDF pages, and the pages are undated. | `S p.N (5-2xx)`: N is the PDF page index and 5-2xx is the printed page. PDF p.1 is printed 5-255, and so on up to PDF p.17, printed 5-271. |
| **[DB]** | `teletext-fonts/manufacturers/philips/datasheets/databooks/Philips_1979_ICs_for_Digital_Systems_in_Radio_and_TV_part6b.pdf`: the Philips 1979 databook, 395 PDF pages, with most data sheets dated August 1978. | `DB p.N (<part> p.M)`: N is the **PDF page index** and M is the data sheet's own printed page number. |

**Page-number check against catalog/indexes/DATASHEET-INDEX.md.** The index page numbers are PDF indices, and most are right. These ones are not:
- The SAA5040 starts at DB p.156, not p.152. Pages 152–154 are the end of the SAA5030 data sheet.
- The index lists p.141 under SAA5050, but DB p.141 is the SAA5020's application data. It only *mentions* the SAA5050.
- The SAA5050 data sheet itself runs from DB p.180 to p.191.
- The SAB1009B, which is unrelated, starts at DB p.206, not p.208. The teletext chapter therefore ends at p.205.

Detailed page map:
- SAA5020: DB pp.136–144
- SAA5030: DB pp.146–154
- SAA5040: DB pp.156–167
- SAA5041: DB pp.168–179
- SAA5050: DB pp.180–191
- SAA5051: DB p.192, a one-page supplement dated May 1979
- SAA5052: DB pp.194–205

**Labelling.** Every fact below is cited to [S] or [DB].
- **FIGURE-READ** marks a value I measured or interpreted from a drawing rather than read from printed text.
- **UNVERIFIED** marks anything from my general knowledge that is not in these two documents.

---

## 1. Role in the original four-chip system

### Overall data flow (from the sources)

The chain runs in this order:
1. **Composite video** goes into the **SAA5030 VIP**. The SAA5030 slices out the teletext data and regenerates a 6.9375 MHz data clock (the "F7" signals).
2. The data goes to the **SAA5040 TAC**. The TAC selects the requested page and writes it into the page RAM as 7-bit parallel words (D1–D7), using row addresses A0–A4 during DEW.
3. For display, the **SAA5020 TIC** supplies the RAM row addresses (A0–A4, RACK) plus all the display timing: TR6, F1, GLR, DEW, LOSE and CRS.
4. The **page RAM** drives D1–D7 into the **SAA5050**, one character per 1 µs F1 period.
5. The SAA5050 outputs R, G, B, Y and blanking to the TV.

Sources for this flow:
- DB p.136 (SAA5020 p.1): "provides the necessary timing signals to the teletext page memory and to the Character Generator (SAA5050)".
- DB p.146 (SAA5030 p.1).
- DB p.157 (SAA5040 p.2).
- S p.7 (5-261), pins 4–10: "These inputs accept a 7-bit parallel data code from the page memory."
- DB p.185 (SAA5050 p.6), which gives the same wording.

The SAA5050 also takes remote-control commands directly, on its DATA and DLIM pins, from the SAA5010/SAA5012 remote-control decoder (S p.7 (5-261) pins 3 and 11; DB p.185).

The SAA5050 is stated to be "suitable for direct connection to the SAA5010, SAA5012, SAA5020 and SAA5040 Series" (S p.2 (5-256)). DB p.180 lists only SAA5010, SAA5020 and SAA5040.

### SAA5020 Teletext Timing Chain (TIC)
Sources: DB pp.136–144.

**What it is.** A 24-pin MOS N-channel chip that "performs the timing functions for a teletext system" and "maintains the synchronisation between the teletext system and the incoming video signal" (DB p.136, SAA5020 p.1).

**Clock division** (DB p.137, SAA5020 p.2):
- The basic input is 6 MHz (F6) from the SAA5030. It is buffered out as TR6.
- "A divide-by-six counter produces the character rate of 1 MHz. This is followed by a divide-by-64 to produce the line rate and a further divide by 312/313 to derive the field rate."
- "The line rate is also divided by 10 to clock a divide-by-24 counter for the teletext memory row addresses."
- The chip also contains the logic for big-character and transmitted-large-character display. It generates CRS for rounding, and AHS ("after hours sync", a composite sync) for driving the display when there is no incoming video.

**Signals to the SAA5050** (DB pp.141–142, SAA5020 pp.6–7):

| Signal | Description |
|---|---|
| TR6 | 6 MHz dot clock. |
| F1 | 1 MHz. "synchronous with TR6, with a positive going edge occuring at time zero of the line." |
| GLR | "a 1 µs negative going pulse commencing 5 µs from the start of each line". |
| CRS | "high for even fields (0-313 lines) and low for odd fields (314-625 lines)". |
| LOSE | "a positive going pulse of duration 40 µs starting 14.5 µs after the start of the line and occurs on lines 49 to 288 and 362 to 601 only". |
| DEW | "a positive going pulse commencing at the end of line 5 and finishing at end of line 22 and similarly for lines 318 and 335". |

**TLC input** (from the SAA5050): "high for normal character display and must be taken low for large character display."

**BCS and T/B inputs** (from the SAA5040):
- BCS is high for normal characters and low for large characters.
- T/B "must be low for the top half to be displayed, and high for the bottom half".

**Row address outputs A0–A4:**
- They are active during "TV lines 49 to 288 - 362 to 601 inclusive".
- They go high-impedance when HIE is high. "All address outputs are low during line 40."
- The address "is increased every ten lines in small character mode and every twenty lines in large character mode. If any row contains transmitted large characters the address is incremented by two after 20 lines." (DB p.142)

**RACK:** a 1 MHz clock that starts 14.65 µs into the line and ends at 53.15 µs (DB p.142).

### SAA5030 Teletext Video Processor (VIP)
Sources: DB pp.146–154.

**What it is.** A bipolar chip on a 12 V supply. It "extracts data and data clock information from the television composite video signal and feeds this to the Acquisition and Control circuit SAA5040" (DB p.146, SAA5030 p.1).

**Clock and sync:**
- It contains "a 6 MHz crystal controlled phase locked oscillator ... which drives the Timing Chain circuit SAA5020".
- It contains an adaptive sync separator that supplies line and field sync (FLR, FS) to the SAA5020.
- The 6 MHz loop is phase-locked through a "sandcastle" signal built from the SAA5020's PL and CBB outputs (DB p.150, SAA5030 p.5, pin 5).

**Data slicing:**
- The data is sliced at the mid-point of two peak detectors (DB p.152, pins 23–24).
- Data and clock go to the SAA5040 through series capacitors, on pins 19 and 18 (DB p.151).
- The bit rate is 6.9375 Mbit/s, and "odd parity for the 8-bit bytes ensures that there are never more than 14 bit periods between each data transition" (DB p.150, note 2).

**Picture-on (PO) handling.** PO comes from the SAA5040 and selects whether pin 12 outputs the input video or AHS (DB p.151, pins 10–12).

### SAA5040 Teletext Acquisition and Control circuit (TAC)
Sources: DB pp.156–167.

**What it is.** A 28-pin MOS chip that "performs the control, data acquisition and data routing functions". It "selects the required page information and feeds it in parallel form to the teletext page memory". It was designed to "the September 1976 Broadcast Teletext specification published by BBC/IBA/BREMA" (DB p.156, SAA5040 p.1).

**Acquisition section** (DB p.157, SAA5040 p.2):
- Takes F7 Data and F7 Clock (6.9375 MHz) from the SAA5030.
- "Hamming and parity checks are performed on the incoming data to reduce errors. Provision is also made to process the control bits in the page header."

**Control section:**
- Decodes remote-control commands from the SAA5010.
- Can write on-screen status such as the page number and programme name into the page memory.
- Tri-states its memory buses in viewdata mode.

**Memory writes** (DB p.163, SAA5040 p.8, pins 16–22):
- During DEW, received data is written at "867 k bytes per second".
- During TV line 40 (EDIL, "encoded data insertion line"), status information is written at 1 Mbyte/s.
- On a page clear, the data output "is forced to the space code (0100000) during the display period for one field".
- A0–A4 give the 5-bit row address and select "which of 24 rows the teletext data is to be written" (DB p.163).

**Outputs to the SAA5050** (DB p.162, SAA5040 p.7):
- **PO:** "HIGH for TV picture 'ON', LOW for TV picture 'OFF'".
- **DE:** "high for display enabled"; "forced to the low state during the DEW and TV line 40 periods and when a teletext page is cleared".
- **BCS:** "low for double height characters" under user control; forced high on page clear.
- **T/B** (to the SAA5020): high for the bottom half-page.

**User large-character commands.** The SAA5040 command table includes SMALL CHARACTERS, LARGE CHARACTERS TOP HALF PAGE and LARGE CHARACTERS BOTTOM HALF PAGE (DB p.166, SAA5040 p.11).

**SAA5041** (DB p.168, SAA5041 p.1): "similar to the SAA5040 but provides German on-screen displays and has a different set of remote control commands". It "works in conjunction with the SAA5020 Timing Chain and the SAA5050 or SAA5052 Character Generator". The text names the SAA5052, which is the Swedish part, not the German SAA5051.

### The SAA5050 itself
Sources: S p.2 (5-256); DB p.180 (SAA5050 p.1).

"The SAA5050 is a 28 pin device which incorporates a fast access character generator ROM (4.3 k bits), the logic decoding for all the teletext control characters and decoding for some of the remote control functions. The circuit generates 96 alphanumeric and 64 graphic characters. In addition there are 32 control characters which determine the nature of the display."

**Block diagram** (S p.1 (5-255), Fig.1). It shows these blocks:
- input buffer
- ROM
- graphics generator
- parallel-to-serial conversion
- character rounding
- control character detection and store (which drives the TLC pin)
- display control
- colour multiplexer
- timing generator
- flash counter
- power-on reset
- remote control data decoder and store

In the drawing, the GRAPHICS GENERATOR is fed from the input buffer, not from the ROM. This is consistent with "The 64 graphics characters are decoded directly from the character data inputs" (S p.7 (5-261); DB p.185).

---

## 2. Pinout and pin functions

Package: 28-lead DIL, plastic, SOT-117 (S p.1; DB p.180).

The pin list is from S p.2 (5-256) Fig.2 and DB p.181 (SAA5050 p.2). Pin functions are from S pp.7–8 (5-261/262) and DB pp.185–186 (SAA5050 pp.6–7).

In the Signetics pinning diagram, **SI, DATA, GLR, TLC and BCS** are drawn with overbars, meaning they are active-low.

| Pin | Name | Dir | Documented function |
|---|---|---|---|
| 1 | VSS | – | Ground, 0 V. |
| 2 | SI (Superimpose) | I/O, open drain | "dual purpose input/output pin". The output conducts when superimpose mode is selected, "allows contrast reduction of the tv picture in superimpose mode". "If the pin is held LOW, the internal 'tv mode' flip-flop is held in the 'text' state. This is for VDU applications when the remote control is not used." Recommended pull-up is 18 kΩ (S p.6 note 2). |
| 3 | DATA | in | 7-bit serial remote-control data, nominally 32 µs/bit (S p.7). |
| 4–10 | D1–D7 | in | 7-bit parallel character code from the page memory. "The alphanumeric addresses are ROM column addresses, the graphics and control data are decoded internally." |
| 11 | DLIM | in | Remote-control data clock. "The positive-going edge of every second clock pulse is nominally in the centre of each remote control data bit" (S p.7). |
| 12 | GLR (General line reset) | in | From the SAA5020, "required for internal synchronisation of remote control data signals". |
| 13 | DEW (Data entry window) | in | From the SAA5020, "required to reset the internal ROM row address counter prior to the display period. It is also used internally to derive the 'flash' period." |
| 14 | CRS (Character rounding select) | in | From the SAA5020, "required for correct character rounding of displayed characters. (Normal height characters only)." |
| 15 | BCS (Big character select) | in | From the SAA5040, "allows selection of large characters by remote control." |
| 16 | TLC (Transmitted large characters) | out | "This output to the SAA5020 Timing Chain enables double height characters to be displayed as a result of control characters stored in the page memory." |
| 17 | NC | – | Not connected. |
| 18 | VDD | – | +5 V. |
| 19 | TR6 | in | "6 MHz signal from the SAA5020 Timing Chain used as a character dot rate clock." |
| 20 | F1 | in | 1 MHz, "used to latch the 7-bit parallel character data into the input latches. It is also used to synchronise an internal divide-by 6 counter. The F1 signal is internally synchronised with TR6." |
| 21 | Y | out, open drain | Monochrome video, "active in the HIGH state containing character dot information". |
| 22, 23, 24 | B, G, R | out, open drain | "active HIGH and contain both character and background colour information." |
| 25 | BLAN (Blanking) | out, open drain | "active for the duration of a box when Picture On and Display Enable are HIGH. It is also activated permanently for normal teletext display when no tv picture is required (PO LOW)." See Table 3 in §8. |
| 26 | LOSE (Load output shift register enable) | in | From the SAA5020. "resets the internal control character flip-flops prior to the start of each display line. This signal also defines the character display period." |
| 27 | PO (Picture on) | in | From the SAA5040. "When PO is HIGH, only text in boxes is displayed unless in superimpose mode." HIGH means TV picture on. |
| 28 | DE (Display enable) | in | From the SAA5040. HIGH means teletext display enabled; LOW means display cancelled. |

**The SAA5050 has no row address input.** It keeps its own "internal ROM row address counter", which DEW resets before the display period (pin 13). Row selection in the page RAM is done by the SAA5020.

**Implication for software (my inference from pins 13 and 26 plus the DEW/LOSE timing).** Per-line state is reset by LOSE at the start of every display line. The glyph scan-line counter is advanced internally and reset once per field by DEW.

---

## 3. Timing and geometry

**Dot clock, character period and cell size** (S p.2 (5-256); DB p.181, SAA5050 p.2):
- "The character period is 1 µs and the character dot rate is 6 MHz."
- "Each character rectangle is 6 dots wide by 10 TV lines high. One dot space is left between adjacent characters, and there is one line space left between rows. Alphanumeric characters are generated on a 5 x 9 matrix, allowing space for descending characters."
- "Each of the 64 graphic characters is decoded to form a 2 x 3 block arrangement which occupies the complete 6 x 10 dot matrix."

**Clock specifications** (S p.5 (5-259), "Timing characteristics ... For typical display of 40 characters per line. Line rate = 64 µs. Field rate = 20 ms."):
- TR6 is 6 MHz and F1 is 1 MHz, each with a mark/space ratio of 40:60 to 60:40.
- TR6 rising edge to F1 falling edge: 6 to 60 ns.

**Data input timing:**
- [S] gives a set-up time of 80 ns min and a hold time of 100 ns min (S p.5).
- [DB] gives 150 ns set-up and 100 ns hold (DB p.182).
- FIGURE-READ: DB p.187 Fig.2 and S p.9 Fig.4 both place the "Data valid" window, with its set-up and hold times, around the F1 **falling** edge that ends F1's 500 ns high phase. So the SAA5050 latches data on the falling edge of F1.

**Pipeline delay** (S p.5, Fig.4 on S p.9):
- Graphics: tCDG = **2.6 µs** from "character in" to the character at the outputs.
- Alphanumerics: tCDA = **2.767 µs**.
- The difference is 0.167 µs, which is exactly one 6 MHz dot. Fig.4 draws "character 'n' display period for graphics" and "... for alphanumerics" offset by that amount.
- My inference, not stated in the text: in the output video, alphanumeric dots sit one dot period later, relative to graphics, for the same character slot. A pixel-exact emulation may need to model this.

**Display period** (S p.5; Fig.5/6 on S pp.9–10):
- LOSE HIGH time is 40 µs, which is 40 characters.
- Line start to LOSE rising edge: 14.5 µs. LOSE falling edge to line start: 9.5 µs. Line period: 64 µs.
- LOSE rising edge to "Display on": 2.6 µs. LOSE falling edge to "Display off": 2.6 µs.
- GLR LOW time is 1 µs, and it starts 5 µs after line start.

**Rows:**
- 40 characters per row (S p.5 "typical display of 40 characters per line"; LOSE 40 µs).
- **24 rows.** The SAA5020 has a "divide-by-24 counter for the teletext memory row addresses" (DB p.137).
- The display occupies lines 49–288 and 362–601. That is 240 lines per field, which is 24 × 10 (DB p.142).
- The SAA5040 writes into "24 rows" (DB p.163).
- None of the sources mentions a 25th row.
- Row 0 has a special status as the header row. In 'tv' mode, text is allowed "on top row of display only" (S p.14 (5-268), Table 2).

**Lines per row and interlace:**
- Each row is 10 TV lines per field. Fields have 312½ lines (S p.10 (5-264), Fig.7 "for field period of 20 ms, 312½ lines per field").
- The same row data is scanned in both fields. CRS tells the chip which field it is in, and that only affects rounding (see §4).
- The DOCUMENTED INDEX note of a "12 × 20 cell" corresponds to 6 dots × 2 half-dots by 10 lines × 2 fields. This is my arithmetic, not a datasheet statement.

**Field-rate signals** (S p.10 (5-264), Fig.7, "Numbers refer to typical tv line number"):
- DEW is high over lines 6–22 and 318–335.
- CRS changes level at line 313.
- SAA5020 text: CRS is high for lines 0–313 (the "even" field) and low for lines 314–625 (the "odd" field) (DB p.141).

---

## 4. Character rounding (smoothing)

**Complete documented text** (S p.7 (5-261); DB p.185, SAA5050 p.6; DB p.199, SAA5052 p.6, identical):

> "The character rounding function is different for the small and double height characters. In both cases the ROM is accessed twice during the character period of 1 µs. The dot information of two rows is then compared to detect the presence of any diagonal in the character matrix and to determine the positioning of the character rounding half dots.
> For small characters rounding is always referenced in the same direction (i.e. row before in even fields and row after in odd fields as determined by the CRS signal).
> For double height characters rounding is always referenced alternately up and down changing every line using an internally generated signal. (The CRS signal is '0' for the odd field and '1' for the even field of an interlaced tv picture)."

**Description paragraph** (S p.2; DB p.181): "The alphanumeric characters are character rounded, i.e. a half dot is inserted before or after a whole dot in the presence of a diagonal in a character matrix."

What this establishes:
1. **Scope.** Rounding applies to **alphanumeric** characters only. Graphics bypass the ROM ("decoded directly from the character data inputs", S p.7; DB p.185). In the block diagram, the graphics generator path is separate from the ROM and character-rounding blocks (S p.1, Fig.1). Mosaics are therefore not rounded.
2. **Resolution.** The half dot is half of a 6 MHz dot, about 83 ns (12 MHz equivalent). This value is my arithmetic.
3. **Mechanism.** The ROM is read twice per character, once for the current glyph row and once for an adjacent glyph row. The two rows are compared to find diagonals, which decides where half-dots go, "before or after a whole dot".
4. **Normal height.** The comparison row is the *row before* in even fields (CRS = 1) and the *row after* in odd fields (CRS = 0). With interlace, each field shows the 9-row glyph on alternate lines, so the half-dots in one field fill the diagonal "steps" toward the lines of the other field.
5. **Double height.** Each glyph row covers 2 lines per field. The reference direction alternates up and down every line from an internal signal, and CRS is not used. The pin 14 text confirms this: "(Normal height characters only)".
6. **What the datasheets do not give:** the exact boolean rule, meaning which neighbour patterns count as a "diagonal" and whether the half-dot goes on the left or right side.

**UNVERIFIED — general knowledge (emulators such as MAME saa5050.cpp, BeebEm, jsbeeb; not in these sources).** The usual reconstruction is:
- Double each glyph row horizontally to half-dot resolution.
- For the current row A and the adjacent row B, add a half-dot beside a set dot of A where B has a set dot diagonally adjacent (x±1) while the orthogonally adjacent positions do not form a solid block.
- A frequently quoted form works on 12-bit half-dot rows: `out = A | ((A>>1) & B & ~(B>>1)) | ((A<<1) & B & ~(B<<1))`.

This formula must be checked against a reference such as ETS 300 706 or known-good captures before it becomes a requirement. The datasheet supports only the qualitative description above.

---

## 5. Control codes 0x00–0x1F

The code grid is from S p.13 (5-267) Table 1 and DB p.188 (SAA5050 p.9) Table 1:
- Bits b7 b6 b5 select the column and b4 b3 b2 b1 select the row. "Codes may be refered to by their column and row e.g. 2/5 refers to %".
- Column 0 is 0x00–0x0F and column 1 is 0x10–0x1F.
- Table footnotes:
  - "Control characters shown in columns 0 and 1 are normally displayed as spaces."
  - "\* These control characters are reserved for compatability with other data codes."
  - "\*\* These control characters are presumed before each row begins."

| Code | Col/row | Table 1 name | Marker |
|---|---|---|---|
| 0x00 | 0/0 | NUL | \* reserved |
| 0x01 | 0/1 | Alpha(numerics) Red | |
| 0x02 | 0/2 | Alpha Green | |
| 0x03 | 0/3 | Alpha Yellow | |
| 0x04 | 0/4 | Alpha Blue | |
| 0x05 | 0/5 | Alpha Magenta | |
| 0x06 | 0/6 | Alpha Cyan | |
| 0x07 | 0/7 | Alpha White | \*\* default at row start |
| 0x08 | 0/8 | Flash | |
| 0x09 | 0/9 | Steady | \*\* default |
| 0x0A | 0/10 | End Box | \*\* default |
| 0x0B | 0/11 | Start Box | |
| 0x0C | 0/12 | Normal Height | \*\* default |
| 0x0D | 0/13 | Double Height | |
| 0x0E | 0/14 | SO | \* reserved |
| 0x0F | 0/15 | SI (printed "S1") | \* reserved |
| 0x10 | 1/0 | DLE | \* reserved |
| 0x11–0x17 | 1/1–1/7 | Graphics Red, Green, Yellow, Blue, Magenta, Cyan, White | |
| 0x18 | 1/8 | Conceal Display | |
| 0x19 | 1/9 | Contiguous Graphics | \*\* default |
| 0x1A | 1/10 | Separated Graphics | |
| 0x1B | 1/11 | ESC | \* reserved |
| 0x1C | 1/12 | Black Background | \*\* default |
| 0x1D | 1/13 | New Background | |
| 0x1E | 1/14 | Hold Graphics | |
| 0x1F | 1/15 | Release Graphics | \*\* default |

**What the datasheets document about behaviour:**
- Defaults at the start of every row are: alpha white, steady, end box, normal height, contiguous graphics, black background, release graphics (the \*\* codes, Table 1).
- Hardware reset of these is by LOSE: "resets the internal control character flip-flops prior to the start of each display line" (S p.8 pin 26; DB p.186).
- Control characters "are normally displayed as spaces" (Table 1 footnote). The word "normally" leaves room for the Hold Graphics exception, but **the held-mosaic behaviour is not described anywhere in either document.**
- The DB p.191 (SAA5050 p.12) Fig.4 "Character format" example "Alphanumerics and graphics 'space' character 0000010" is drawn as a cell entirely in background colour. The same figure is in S p.12 (5-266) Fig.10.
- Colour codes give 7 foreground colours: red, green, yellow, blue, magenta, cyan and white.
  - Alpha codes select alphanumerics. Graphics codes select mosaics for columns 2, 3, 6 and 7 (Table 1 columns 2a, 3a, 6a, 7a).
  - R, G and B carry "both character and background colour information" (pin 22–24 text).
  - There is no black foreground: 0x00 is NUL and 0x10 is DLE, both reserved.
- Conceal Display: concealed text is shown only in Reveal mode, which is set by remote command (§8). "Reveal mode allows display of text previously concealed by 'conceal display' control characters" (S p.14 (5-268), Table 2 note 3).
- Flash and Steady: 0.75 Hz, 3:1 ON/OFF (§8).

**Set-at versus set-after timing is NOT specified in either datasheet.** Nothing says whether a code takes effect in its own cell or the next one.

**UNVERIFIED — general knowledge of the SAA5050 as used in the BBC Micro, MAME and Mullard application notes, and ETS 300 706 Level 1:**
- Set-at (the code's own cell is affected): Steady, Normal Height, Conceal, Contiguous, Separated, Black Background, New Background, Hold Graphics.
- Set-after (takes effect from the next cell): Alpha and Graphics colour codes, Flash, Start Box, End Box, Double Height, Release Graphics.
- One detail is recalled as SAA5050-specific: a change of height (normal ↔ double) clears the held-graphics character.
- Treat this as a checklist to confirm against ETS 300 706 §12.2, which is in `teletext-standards/`. The datasheets do not provide it.

---

## 6. Hold Graphics

**What the datasheets document:**
- 1/14 (0x1E) is "Hold Graphics".
- 1/15 (0x1F) is "Release Graphics", which is "presumed before each row begins" (Table 1: S p.13; DB p.188).
- Control codes are "normally displayed as spaces" (same footnote).

**Not documented:**
- which character is held
- whether the held character keeps its contiguous/separated state
- what resets it apart from row start

**UNVERIFIED (general knowledge):**
- With Hold on, a control-code cell in graphics mode shows the most recently displayed mosaic character instead of a space. Blast-through capitals (columns 4 and 5) are not mosaics and do not become the held character.
- The held character is reset to space at the start of each row, on a change between alpha and graphics mode, and on a change of size.
- On the SAA5050, the held mosaic's contiguous/separated state is taken from the current mode, not from when it was stored. This is one of the details where later decoders and the Level 1 spec differ.

Verify all of this against ETS 300 706 and emulator references.

---

## 7. Double height

**What the datasheets document:**
- 0/13 (0x0D) is Double Height and 0/12 (0x0C) is Normal Height, the default at row start (Table 1).
- The TLC output, pin 16, "enables double height characters to be displayed as a result of control characters stored in the page memory" (S p.8; DB p.186). In other words, when a row contains Double Height the SAA5050 tells the timing chain.
- On the SAA5020 side, TLC is "high for normal character display and must be taken low for large character display" (DB p.142, SAA5020 p.7, pin 15).
- The SAA5020 then holds the RAM row address for 20 lines and skips a row: "If any row contains transmitted large characters the address is incremented by two after 20 lines." (DB p.142, pins 19–23)
- **Consequence in the original four-chip system (my reading of DB p.142):**
  - Row n is displayed over 20 TV lines per field, the space of rows n and n+1.
  - The memory row n+1 is **never displayed**.
  - The bottom halves are produced by the SAA5050 continuing its own internal glyph-row count through the second 10 lines of the same row n data. The DEW-reset "internal ROM row address counter" is at S p.7 pin 13.
  - Neither document says how normal-height characters on a double-height row are drawn in the bottom half.
- **User-selected large characters (BCS):**
  - From the SAA5040, commands can select LARGE CHARACTERS TOP HALF PAGE or BOTTOM HALF PAGE (DB p.166).
  - BCS goes to both the SAA5020 and the SAA5050. T/B goes to the SAA5020 only.
  - The SAA5020 advances the address "every twenty lines in large character mode" (DB p.142).
  - T/B "must be low for the top half to be displayed, and high for the bottom half" (DB p.142, pin 18).
- **Rounding in double height.** It alternates the reference direction up and down every line and does not use CRS (§4).

**UNVERIFIED (general knowledge):**
- In systems like the BBC Micro, a CRTC drives the rows and does not skip a row. There, the SAA5050 draws the lower row as bottom halves of the double-height characters, and normal-height characters in that lower row show as spaces or background.
- ETS 300 706 Level 1 says the row after a double-height row is ignored.
- For a software decoder, the requirement should follow ETS 300 706. The SAA5020 behaviour (skip row n+1) agrees with it.

---

## 8. Flash, conceal, boxing, backgrounds, mosaic geometry

### Flash
"The circuit generates a 0.75 Hz signal with a 3:1 ON/OFF ratio to provide the flashing character facility." (S p.6 (5-260); DB p.184, SAA5050 p.5)

The flash period is derived from DEW (S p.7 pin 13), so the flash timer counts fields.

My arithmetic: 0.75 Hz is a period of 1.333 s, with 1.0 s on and 0.333 s off. At 50 fields/s that is 66.7 fields per cycle, which is not an integer, so the documented figure is nominal. UNVERIFIED: emulators commonly use 48 fields on and 16 fields off, a 64-field cycle of about 0.78 Hz.

Not documented: whether flash affects only the foreground (whether the background stays visible in the off phase).

### Conceal and Reveal
- 1/8 (0x18) is Conceal Display (Table 1). Table 2 (S p.14 (5-268); DB p.189, SAA5050 p.10) gives these remote commands:
  - "Reveal — Reveals for time-out". It is sent repeatedly while the key is held, and "The SAA5050 reverts to normal 'not Reveal' mode 160 ms after the last Reveal command."
  - "Reveal set — Sets Reveal mode".
  - "Any command apart from reveal set ... Resets Reveal mode."
- Power-on: "When the supply voltage is switched on, the character generator will reset to tv, conceal, and not superimpose modes." (S p.7 (5-261); DB p.184)
- Not documented: how conceal ends within a row. UNVERIFIED: it lasts until a colour code (alpha or graphics), per the Level 1 spec.

### Boxing (Start Box / End Box), PO/DE, Superimpose and Blanking

Codes: 0/11 (0x0B) is Start Box and 0/10 (0x0A) is End Box, the default (Table 1). The datasheets **do not say whether one Start Box or two consecutive ones are required.** UNVERIFIED: the Level 1 spec requires two consecutive Start Box codes.

**Table 3, "Conditions affecting display"** (S p.15 (5-269); DB p.190, SAA5050 p.11). Inputs are PO and DE; control data is Superimpose mode and Box; outputs are Text Display Enabled (the R, G, B, Y outputs) and Blanking.

| Row | PO | DE | Superimpose | Box | Text displayed | Blanking |
|---|---|---|---|---|---|---|
| (a) | 1 | 0 | x | x | 0 | 0 |
| (b) | 0 | 1 | x | x | 1 | 1 |
| (c) | 0 | 0 | x | x | 0 | 1 |
| (d) | 1 | 1 | 0 | 0 | 0 | 0 |
| (e) | 1 | 1 | 1 | 0 | 1 | 0 |
| (f) | 1 | 1 | 1 | 1 | 1 | 1 |
| (g) | 1 | 1 | 0 | 1 | 1 | 1 |

Table 3 notes:
1. "For tv mode (Picture On = '1', Superimpose mode not allowed) rows (a), (d) and (g) of Table 3 refer to display row 0 only. For all other rows text display is disabled and Blanking = '0'."
2. "The R, G, B outputs may contain character and background colour information. The only exception is that background colours are inhibited when Blanking = '0'."
3. "Valid during display period only ... otherwise no character or background information is displayed as blanking is determined by the Picture On."

What Table 3 and its notes mean:
- **Boxed text (newsflash, subtitle).** With the picture on and DE high, text shows only inside boxes (row g), and the blanking output cuts a hole in the TV picture for the box.
- **Superimpose.** Text shows everywhere, but outside boxes it is not blanked (row e). Background colour is suppressed wherever Blanking = 0 (note 2).
- **Superimpose output.** "The Superimpose output is LOW only if Superimpose mode is set and the DE (display enable) input is HIGH." (Table 2 note 5)
- **Software requirement (my reading).** "Mix" or "boxed" rendering must suppress background colour outside boxes, and text outside boxes is hidden unless superimpose is on.

**Remote modes** (Table 2):

| Mode | Behaviour |
|---|---|
| 'tv' (b7 = 0) | "Allows text on top row of display only." Also resets superimpose mode. |
| 'Text' (b7 = 1) | "Allows text throughout display period." |
| Superimpose (1011110) | Sets superimpose mode. |
| Teletext (1011111) | Resets superimpose mode. |
| Viewdata (b7 = b6 = 1) | Resets superimpose mode. |

Table 2 note 6: the SAA5010/SAA5012 transmits, and the SAA5050 requires, the inverse of the codes shown, sent serially in the order b7 b1 b2 b3 b4 b5 b6.

### Background codes
- 1/12 (0x1C) is Black Background, the default. 1/13 (0x1D) is New Background (Table 1).
- The datasheets do not state that New Background takes the current foreground colour. UNVERIFIED: it does, per the Level 1 spec.
- The pin 22–24 text confirms that the RGB outputs carry background colour.

### Mosaic (graphics) geometry
Sources: S p.11 (5-265) Fig.9; DB p.187 (SAA5050 p.8) Fig.3.

**Bit-to-block mapping:**
- The cell is 1 µs wide, which is 6 dots, and "10 TV lines" high.
- It is split into 2 columns and 3 rows of blocks:

  | Row | Left block | Right block |
  |---|---|---|
  | Top | b1 | b2 |
  | Middle | b3 | b4 |
  | Bottom | b5 | b7 |

- "Each cell is illuminated if particular 'bit' (b1, b2, b3, b4, b5, or b7) is a '1'. For graphics characters b6 is always a '1' — See Table 1."
- Graphics codes are columns 2a, 3a, 6a and 7a (0x20–0x3F and 0x60–0x7F in graphics mode).
- Columns 4 and 5 (0x40–0x5F) display as alphanumerics even in graphics mode. This is blast-through (§9).

**Block widths.** 3 dots + 3 dots. FIGURE-READ: the vertical divider is at the midpoint of the 1 µs cell (0.5 µs, which is 3 dots at 6 MHz).

**Block heights.** FIGURE-READ from DB p.187 Fig.3 at 400 dpi:
- The left margin has 10 braces, one per TV line.
- The top block spans 3 of them, the middle block 4 and the bottom block 3. Measured block heights are in the proportion 220 : 315 : 240, about 3 : 4 : 3.
- So the heights are **3, 4 and 3 lines** (top, middle, bottom).
- The DB p.191 Fig.4 contiguous example draws equal thirds, but that figure is schematic and has no dimensions.

**Contiguous graphics:** the blocks fill the whole 6 × 10 cell with no gaps (Fig.4 "Contiguous graphics character 1111111").

**Separated graphics:**
- DB p.191 Fig.4 and S p.12 Fig.10 show each block as a smaller rectangle of display colour, surrounded by background colour on all sides.
- **No dot or line dimensions are given for the gaps.**
- UNVERIFIED (general knowledge): each block loses one dot column (the left column, giving 2 lit + 1 gap per 3-dot half) and its bottom line.

**Bit order in the figure captions.** The Fig.4 captions list code bits "as transmitted from b1 to b7". For example, "0110111" is b1 = 0, b2 = 1, b3 = 1, b4 = 0, b5 = 1, b6 = 1, b7 = 1. That lights the top-right, middle-left, bottom-left and bottom-right blocks, which matches the drawing. Anyone reading these captions must reverse them to get normal hex.

**Other examples in Fig.4 / Fig.10:**
- 1011010 is a small bar, the "−" character.
- 0001001 is an "H", captioned "Alphanumerics or blast-through alphanumerics character". In reversed bit order this is 1001000, which is 0x48 'H', in column 4: blast-through.
- 1111111 is the alphanumeric 7/15 solid block. It is drawn as a display-colour rectangle with a background margin, **not** filling the cell, in contrast to contiguous graphic 7a/15, which fills the whole cell.

---

## 9. Blast-through, character repertoire and national variants

**Blast-through.** Table 1 (S p.13; DB p.188) has graphics sub-columns only for columns 2, 3, 6 and 7 (marked 2a, 3a, 6a, 7a). Columns 4 and 5 have no graphics alternative. The DB p.191 Fig.4 caption names "blast-through alphanumerics". So in graphics mode, codes 0x40–0x5F show the alphanumeric glyph.

**Alphanumeric matrix** (FIGURE-READ from S p.16 (5-270) Fig.11 and S p.17 (5-271) Fig.12). Each glyph is drawn on a grid of **5 columns × 9 rows**, which I counted on the 2/0 blank grid at 300 dpi. This matches the "5 x 9 matrix".
- Capitals occupy the upper 7 rows.
- Descenders (g, p, q, y, j) use the lower rows.
- The 6th dot column and the 10th line of the 6 × 10 cell are the "one dot space" and "one line space".
- **No dot patterns are reproduced in these notes.**

**English SAA5050 repertoire** (DB p.188 Table 1; S p.16 Fig.11):
- 96 alphanumeric positions, 0x20–0x7F.
- 2/0 is space. 2/1–2/15 are `! " £ $ % & ' ( ) * + , - . /`; 2/3 is **£**, not #.
- 3/0–3/15: digits 0–9 and `: ; < = > ?`.
- 4/0 is **@**, then A–O. 5/0–5/10 are P–Z.
- 5/11–5/15: **← ½ → ↑ #**, that is, left arrow, one-half, right arrow, up arrow, hash.
- 6/0 is **—** (a long horizontal dash). 6/1–6/15 are a–o. 7/0–7/10 are p–z.
- 7/11–7/15: **¼ ‖ ¾ ÷ ■**, that is, one-quarter, double vertical bar, three-quarters, divide, solid block.
- 64 mosaics come from the code bits (§8).

**Variants listed** (S p.1 (5-255), Quick reference data):

| Part | Character set |
|---|---|
| 5050 | English |
| 5051 | German |
| 5052 | Swedish |
| 5053 | Italian |
| 5054 | Belgian |
| 5055 | US ASCII |
| 5056 | Hebrew |
| 5057 | Cyrillic |

[S] says the full character sets are "given in Figs. 11 to 18", but **only Figs. 11 (5050) and 12 (5055) are in the 17-page PDF**. [DB] contains tables for 5050, 5051 and 5052 only.

**Code positions that differ from English** (FIGURE-READ; glyph identity only). All other positions match the English set.

| Position | 5050 English (DB p.188) | 5051 German (DB p.192) | 5052 Swedish (DB p.202) | 5055 US ASCII (S p.17) |
|---|---|---|---|---|
| 2/3 (0x23) | £ | # | # | # |
| 2/4 (0x24) | $ | $ | ¤ (currency sign) | $ |
| 3/0 (0x30) | 0 | 0 | 0, printed with a slash (Ø-like)† | 0 |
| 4/0 (0x40) | @ | § | É | @ |
| 5/11 (0x5B) | ← | Ä | Ä | [ |
| 5/12 (0x5C) | ½ | Ö | Ö | \ |
| 5/13 (0x5D) | → | Ü | Å‡ | ] |
| 5/14 (0x5E) | ↑ | ^ | Ü | ^ |
| 5/15 (0x5F) | # | _ (underline) | _ | _ |
| 6/0 (0x60) | — (dash) | ° (degree) | é | ` (grave) |
| 7/11 (0x7B) | ¼ | ä | ä | { |
| 7/12 (0x7C) | ‖ | ö | ö | \| |
| 7/13 (0x7D) | ¾ | ü | å‡ | } |
| 7/14 (0x7E) | ÷ | ß | ü | ~ |
| 7/15 (0x7F) | ■ | ■ | ■ | ■ |

- † The DB p.202 table draws 3/0 with a slash through it. It may just be a slashed-zero drafting style, so verify it before treating it as a real glyph difference.
- ‡ The ring on Å/å shows as a dot in the scan.
- DB p.192: "The SAA5051 is identical to the SAA5050 except for the character set (German)."
- The SAA5052 data sheet (DB pp.194–205) repeats the SAA5050 text word for word, apart from the name and "provides a Swedish Character Set" (DB p.194).
- **5053, 5054, 5056 and 5057 are not documented in either source**, beyond their names.

---

## 10. What the chip does not support, and quirks

**From the datasheets:**
- **Level 1 only**, 1976 specification era. The SAA5040 is designed "in accordance with the September 1976 Broadcast Teletext specification" (DB p.156).
- There is one fixed 96-character G0 set per chip variant. National sets are chosen by fitting a different part, not by page control bits (S p.1 variant list).
  - ESC (1/11) is only "reserved for compatability with other data codes" (Table 1 \*), so there is no G0 set switching.
- **No black foreground.** 0/0 NUL and 1/0 DLE are reserved and display as spaces.
- SO and SI (0/14, 0/15) are reserved, so there is no G1/G2 shift.
- Only two sizes, normal and double height. There are no double-width or double-size codes; 0x0E and 0x0F are SO and SI here.
- 7 foreground colours plus black background. No CLUTs.
- Rounding is applied only to alphanumerics (§4).
- 24 display rows (DB p.137, p.142).
- Double height uses an external address-skip. Memory row n+1 is not displayed (DB p.142).
- **Graphics versus alphanumeric delay.** The pipeline delay differs by one dot: 2.6 µs versus 2.767 µs (S p.5). This is a timing quirk for pixel-exact emulation.
- Flash is 0.75 Hz at 3:1, derived from field timing (DEW).
- Reveal is latched or momentary, with a 160 ms timeout (Table 2).
- Background colour is suppressed outside boxes in superimpose/mix modes (Table 3 note 2).

**Differences between the two sources:**
- Data set-up time: 80 ns in [S], 150 ns in [DB].
- [S] adds SAA5012 compatibility.
- The blanking description differs: [S] says "under the control of the PO and DE inputs and the box control characters", while [DB] says "when a newsflash or subtitle is to be displayed".

**UNVERIFIED — general knowledge, not in these sources:**
- The set-at/set-after assignments are listed in §5.
- Held-graphics details are in §6.
- Hold is cleared by a size change.
- Start Box needs two codes.
- Double height as used in the BBC Micro: the lower row is used for bottom halves (§7).
- Separated-mosaic gap geometry (§8).
- The 48/16-field flash duty (§8).
- These are all recalled from emulator sources and ETS 300 706 knowledge. Confirm them against `teletext-standards/` (ETS 300 706) and, if possible, the MAME `saa5050.cpp` source before writing requirements.

---

## 11. Electrical and interface (low priority)

Sources: S pp.1, 3–4 (5-255, 5-257, 5-258); DB pp.180–183.

**Supply and environment:**
- VDD is 5 V nominal (4.5–5.5 V).
- IDD is 85 mA typical, 160 mA max.
- Operating range −20 to +70 °C.
- NMOS, 28-pin DIL (SOT-117).

**Inputs:**
- D1–D7 and the clocks: VIH ≥ 2.65 V, VIL ≤ 0.6 V.
- Logic inputs: VIH ≥ 2.0 V, VIL ≤ 0.8 V.
- Leakage ≤ 10 µA, capacitance ≤ 7 pF.

**R, G, B, Y and BLAN outputs:**
- Open drain.
- VOL ≤ 0.5 V at 2 mA, 1.0 V at 4 mA, 2.0 V at 6 mA.
- Can be pulled up to 13.2 V.
- Fall time ≤ 30 ns, measured between 9 V and 1 V into 15 pF with a 12 V pull-up.
- Output-to-output skew ≤ 20 ns.
- Short-circuit protected.

**TLC output:** CMOS/TTL-like, VOH ≥ 2.4 V, with rise and fall times ≤ 1 µs.

**SI pin:** open drain, up to 6.5 V, 18 kΩ pull-up recommended.

**Remote-control input timing** (S p.5): DLIM high 6.5–8 µs, low 3.5–60 µs. DATA set-up and hold are 14 µs typical.
