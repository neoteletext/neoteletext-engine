# SAA5246A (IVT1.0): technical notes for the software receiver requirements

## Sources and citation keys

| Key | Document | Notes |
|---|---|---|
| **[H]** | `teletext-fonts/manufacturers/philips/datasheets/saa5246a/PHILIPS-SAA5246A-INTEGRATED-VIP-TELETEXT-(IVT1.0)-HANDBOOK.PDF`. Philips *Product specification*, January 1993, 50 pp. | This is the main source. `[H p.N]` means PDF page N, which matches the printed page number. |
| **[F]** | `teletext-fonts/manufacturers/philips/datasheets/saa5246a/SAA5246Afont.PDF.pdf`, 20 pp. | This is an extract of [H] pages 27–46: [F p.1] = [H p.27] … [F p.20] = [H p.46]. It adds nothing to [H]. |
| **[5243]** | `teletext-fonts/manufacturers/philips/datasheets/saa5243/SAA5243_ECCT_Philips.pdf`. SAA5243 series ECCT, March 1991, 34 pp. | Scanned, with no text layer. It was read visually. |
| **[5281]** | `teletext-fonts/manufacturers/philips/datasheets/saa5281/SAA5281_Integrated_VIP_and_Teletext_IVT1.8_Philips.pdf`. IVT1.8*, Preliminary specification, 1996 Nov 04, 48 pp. | |

Notation:
- **INFERRED** means the point follows from the cited material but is not stated in it.
- **UNVERIFIED** means the documents do not cover the point.

**Important scope warning:** despite its filename, [H] is the **product datasheet** and not an applications handbook. It has:
- features
- pinning
- electrical and timing characteristics
- memory map
- register map
- character tables

It contains **no functional or operational prose** about acquisition algorithms, slicing, error handling policy, flash timing or X/26 processing. [5243 p.16] says the same of its parent: "A MORE DETAILED DESCRIPTION OF ECCT OPERATION AND APPLICATION IS AVAILABLE ON REQUEST." Many behavioural questions below are therefore UNVERIFIED.

---

## 1. Architecture and block diagram

- "The SAA5246A is a single-chip teletext decoder IC for decoding 625 line base World System Teletext transmissions. The teletext decoder hardware is based on the Enhanced Computer Controlled ECCT device (SAA5243) with some additional features." [H p.2]
- "The Video Input Processor section of the device uses mixed analog and digital designs in the data slicer and clock phase-locked-loop functions. As a result the number of external components are greatly reduced and no critical or adjustable components are required." [H p.2]
- Block diagram (Fig.1, DIL48) [H p.3] contains the following blocks:
  - CVBS input clamp and sync separator
  - analog-to-digital converter
  - data slicer and clock regenerator
  - teletext acquisition and decoding
  - display memory interface (A0–A12, D0–D7, OE, WE)
  - I²C-bus interface (SDA, SCL)
  - timing chain
  - display clock phase-locked loop
  - crystal oscillator (27 MHz)
  - display outputs: R, G, B, Y, BLAN, COR, RGBREF and ODD/EVEN
- **Page memory is external:** an 8K × 8-bit SRAM is used (application diagram Fig.9) [H p.18]. A0–A12 give 13 address bits (8 KB) [H p.4–5].
- 27 MHz crystal (fundamental or 3rd overtone) [H p.2, p.26].
- The package is DIL48 (SAA5246AP) or QFP64 (SAA5246AGP) [H p.2]. Supply is a single +5 V at 64 mA typical and 128 mA maximum [H p.2].
- Features list [H p.2]:
  - "Complete Teletext decoder"
  - "Both video and scan related synchronization modes are supported"
  - "Digital data slicer and display clock phase-locked loop"
  - "Data capture performance similar to SAA5231 (VIP2)"
  - "Option for up to seven national languages"
  - "Optional storage of packet 24 in the display memory"
  - "Separate text and video signal quality detectors, 625/525 video status and language version all readable via I2C-bus"
  - "Automatic ODD/EVEN output control with override"
  - "Control of display PLL free-run and rolling header via I2C-bus"
  - "VCS to SCS mode for stable 525 line status display"

**Split of work between the chip and the external microcontroller.** INFERRED from the register and memory maps [H p.19–25]. The chip does the following autonomously:
- slicing and acquisition
- matching page requests on up to 4 acquisition circuits
- writing rows into page memory
- the rolling header and rolling time in row 0
- the white/green "looking for page" marker
- storing header control data in row 25
- national-option selection from C12–C14
- storing X/24 in row 24 and displaying it automatically (optional)
- extension-packet storage (optional)
- display generation, including boxing, newsflash/subtitle display control, conceal, double height, cursor and zoom

Everything else is left to the microcontroller over I²C (slave address `0010001` [H p.23]). This covers:
- the user interface and page-number entry
- writing status messages into row 0 columns 0–6, row 24 and row 25 columns 10–23
- interpreting X/26, X/27, X/28 and 8/30
- FLOF link following
- any subpage handling beyond the "do care" matching

---

## 2. Data slicer and acquisition front end

- CVBS input [H p.4]: "A positive-going 1 V (peak-to-peak) input is required, connected via a 100 nF capacitor".
- Input specifications [H p.9]:
  - sync amplitude 0.1 / 0.3 / 0.6 V (min / typ / max)
  - video amplitude 0.7 / 1.0 / 1.4 V p-p
  - source impedance ≤ 250 Ω
  - input capacitance ≤ 10 pF
  - display PLL catching range ±7 % (minimum)
  - CVBS-to-TCS delay −150 to +150 ns
  - change in sync delay between all-black and all-white video of 0–25 ns
- BLACK pin: "video black level storage pin, connected to ground via a 100 nF capacitor" [H p.4]. IREF uses 27 kΩ to ground [H p.4].
- Sync separation: the block diagram has a "CVBS input clamp and sync separator" [H p.3]. Sync outputs go on STTV/LFB, and the TCS waveform is given in Fig.4 [H p.14]. STTV gain relative to video is 0.9–1.1 [H p.11].
- R11B status bits [H p.24]:
  - "VCS SIGNAL QUALITY: Indicates that the video signal quality is good and PLL is phase locked to input video when = 1"
  - "DATA QUALITY: If good Teletext data is detected then = 1"
  - "625/525 SYNC: If the input video is a 525 line signal when = 1"
- The PLL time constant is set by VCR MODE (R0 D1): "Selects short time constant of PLL when logic 1" [H p.23]. FREE RUN PLL (R0 D6): "Will force the PLL to free run in all conditions" [H p.23]. [5281 p.22] adds that the PLL free-runs "at 6 MHz".
- **VBI lines accepted.** R1 D3 DEW/FULL FIELD selects "Field-flyback or full channel mode" [H p.23]. The overbar on DEW in Table 2 [H p.22] means 0 = field-flyback (DEW) and 1 = full field.
  - [H] does not list the actual line numbers.
  - The parent SAA5243 specifies "Field flyback (lines 2 to 22), or full channel (all lines) data acquisition" [5243 p.1].
  - For the SAA5246A, the exact line range is UNVERIFIED (probably inherited).
- Performance: only "Data capture performance similar to SAA5231 (VIP2)" [H p.2]. No eye-height, echo, noise or tolerance figures are given.
- Clock run-in and framing-code detection method, slicing-level method (adaptive or fixed), and framing-code error tolerance are all **UNVERIFIED**, because none is described in [H].
- Teletext bit rate: [5243 p.5] gives the TTC clock from the VIP2 as 6.9375 MHz. [H] does not state a bit rate.

---

## 3. Acquisition

### 3.1 Page memories and chapters

- R4 DISPLAY CHAPTER "determines which of the 8 pages is displayed" [H p.23]. R8 ACTIVE CHAPTER A0–A2 selects the chapter for I²C access [H p.24]. This gives **8 chapters (page memories)**.
- **4 acquisition circuits.** R2 "ACQ CIRCUIT: Selects one of four acquisition circuits" [H p.23]. [5243 p.19]: "There are four groups of data shown in Table 3, one for each acquisition circuit (four simultaneous page requests)."
- R2 D6 BANK SELECT: "Selects which bank of four chapters is being accessed, when EXT. PACKET ENABLE = 0" [H p.23]. The 4 acquisition circuits therefore write into chapters 0–3 or 4–7.
- R1 D4 EXT. PACKET ENABLE: "Allocates 2K bytes memory per chapter" [H p.23]. With it set, 8 KB holds 4 pages, each with an extension page (INFERRED from 8 KB / 2 KB).
  - The 8/30 mapping to "Chapter 4 / 5 / 6 Row 23" [H p.20] suggests the extension areas occupy chapter addresses 4–7 in this mode.
  - INFERRED; the exact address mapping is UNVERIFIED.
- Chapter size is INFERRED as 1024 bytes: rows 0–24 × 40 = 1000 bytes, plus 24 bytes of row 25 (10 control + 14 free, per Fig.10) = 1024. This is consistent with the SAA5243 "1 K byte chapter" [5243 p.2].
- Cursor addressing: R9 row R0–R4 (0–31) and R10 column C0–C5 (0–63) [H p.22, p.24].

### 3.2 Page request mechanism (R2, R3; Table 3 [H p.25])

- R2 selects the acquisition circuit (A0, A1), the bank (A2) and the start column SC0–SC2. R3 then takes the data. "Columns auto-increment on successive I2C-bus transmission bytes" [H p.25].
- R3 is 5 bits per column. PRD4 is the **"Do care"** bit and PRD3–0 are the data:

| Col | Field | PRD3 | PRD2 | PRD1 | PRD0 |
|---|---|---|---|---|---|
| 0 | Magazine | HOLD | MAG2 | MAG1 | MAG0 |
| 1 | Page tens | PT3 | PT2 | PT1 | PT0 |
| 2 | Page units | PU3 | PU2 | PU1 | PU0 |
| 3 | Hours tens | X | X | HT1 | HT0 |
| 4 | Hours units | HU3 | HU2 | HU1 | HU0 |
| 5 | Minutes tens | X | MT2 | MT1 | MT0 |
| 6 | Minutes units | MU3 | MU2 | MU1 | MU0 |

- Quotes from the notes to Table 3 [H p.25]:
  - "When the DO CARE bit is set to logic 1 this means the corresponding digit is to be taken into account for page request. If the DO CARE bit is set to logic 0 the digit is ignored. This allows, for example, 'normal' or 'timed page' selection."
  - "If HOLD is set LOW, the page is held and not updated".
- The subcode is matched as four BCD-like digits: HT 2 bits, HU 4 bits, MT 3 bits, MU 4 bits. It is **not** a 13-bit binary subcode. Wildcarding is per digit only.
- Magazine is 3 bits; magazine 8 is presumably MAG = 000 (INFERRED, standard WST coding).
- Hex page numbers (A–F in tens or units) are not excluded by 4-bit fields. Whether they are accepted is UNVERIFIED.

### 3.3 Where packets are stored

- **Basic page (Fig.10 [H p.19]):**
  - "Rows 0 to 23 form the teletext page; row 24 is the extra row available for software generated status messages and FLOF/FASTEXT prompt information."
  - Fig.10 labels row 22 "PACKET X/22", row 23 "PACKET X/23", and row 24 "PACKET X/24 STORED HERE IF R1D7 = 1".
  - R1D7 is a typo: R1 D7 is VCS TO SCS. The X24 POS bit is **R0 D7** [H p.22–23], and [5281 p.16] prints "R0D7 = 1".
- **Row 0** [H p.19]: "The first seven columns (0 to 6) are free for status messages. The eighth is an alphanumeric white or green control character, written automatically by SAA5246A to give a green rolling header when a page is being looked for. The last eight characters are for rolling time."
  - Fig.10 shows 7 status columns, 1 marker column, "24 characters from page header rolling when display page looked for" (cols 8–31) and "8 characters always rolling (time)" (cols 32–39).
- **Row 25** [H p.19]: "The first 10 bytes of Row 25 contain control data relating to the received page as shown in Table 1. The remaining 14 bytes are free for use by the microprocessor."
- **Extension memory (Fig.11 [H p.21])**, used when EXT. PACKET ENABLE = 1:

| Ext row | Content |
|---|---|
| 0–14 | X/26/0 – X/26/14 |
| 15 | X/28/2 |
| 16–17 | X/27/0 – X/27/1 |
| 18–19 | X/27/4 – X/27/5 |
| 20 | X/24 if R0D7 = 0 |
| 21 | X/25 |
| 22 | X/28/0 |
| 23 | 8/30 |
| 24 | X/28/1 |
| 25 | (unlabelled) |

- Packets **not** in any map, and therefore INFERRED not stored:
  - X/26/15
  - X/27/2–3 and X/27/6–15
  - X/28/3–15
  - X/29 (magazine-level packets)
  - X/31
- Serial versus parallel magazine transmission handling (page-termination rule) is **UNVERIFIED**: [H] does not mention it. C11 (magazine serial) is stored in row 25 col 7 bit D0 [H p.20], so software can read it.
- Erase-page (C4) handling, meaning whether the chip clears the chapter on C4, is **UNVERIFIED**. C4 is stored in row 25 [H p.20].
- CLEAR MEM (R8 D3): "When set to 1, clears the display memory. This bit is automatically reset" [H p.24]. The scope (active chapter or all memory) is UNVERIFIED.
- Power-up [H p.23]: "All memory is cleared to 'space' (00100000) on power-up, except Row 0 Column 7 Chapter 0, which is 'alpha' white (00000111) as the acquisition circuit is enabled but the page is on hold."

### 3.4 Header rolling

- The rolling green header and rolling time work as described above for row 0 [H p.19].
- R0 D4 DISABLE HDR ROLL "Disables green rolling header and time" [H p.23].
- [5281 p.22] is more precise for IVT1.8: "Stops the display update of rolling time and green rolling header during page requests when logic 1. Time updates on page reception only."
- The rule for which headers roll (all magazines or the requested magazine only) is UNVERIFIED.

### 3.5 Error handling

- Parity: R1 D6 7+P/8-BIT selects "7 bits with parity checking or 8-bit mode" [H p.23]. The overbar on "7+P" means 0 = 7-bit plus parity and 1 = 8-bit [H p.22].
  - **What is written to memory when a parity error occurs** (space, old character kept, or error symbol) is **UNVERIFIED**. It is not stated in [H] or [5243].
- Hamming 8/4 on the header: row 25 bytes 0–7 carry a per-byte HAM.ER flag in D4: "Hamming error in corresponding byte" [H p.20].
- Hamming on packet address and row: the behaviour is UNVERIFIED.
- R2 D7 HAM.CHECK: "When logic 1 enables Hamming 8/4 checking of extension packet 27/0, 27/1 and 8/30/x" [H p.23].
  - The result format (corrected nibble or raw byte) is UNVERIFIED.
  - [5281 p.20] states that for 8/30/2 on IVT1.8 "the data is stored after hardware Hamming correction. There are 4 data bits stored in each column address of memory with an additional Hamming error bit". This is the IVT1.8 statement; the SAA5246A equivalent is UNVERIFIED.
- X/26, X/27/4–5, X/28 and X/25 are not covered by HAM.CHECK. They are INFERRED to be stored raw; Hamming 24/18 decoding is left to the microcontroller. The SAA5281 adds a "24 : 18 Hamming checker" [5281 p.2], which is further evidence that the SAA5246A lacks one.
- R11B D1 DATA QUALITY flags good teletext data [H p.24].

### 3.6 Clock and time capture

- The last 8 header characters (cols 32–39) are "always rolling (time)" [H p.19]. This is a raw copy of the header bytes; there is no parsed clock register.

### 3.7 Status flags: row 25, columns 0–9 (Table 1 [H p.20])

| Col | D0 | D1 | D2 | D3 | D4 | D5 |
|---|---|---|---|---|---|---|
| 0 | PU0 | PU1 | PU2 | PU3 | HAM.ER | 0 |
| 1 | PT0 | PT1 | PT2 | PT3 | HAM.ER | 0 |
| 2 | MU0 | MU1 | MU2 | MU3 | HAM.ER | 0 |
| 3 | MT0 | MT1 | MT2 | C4 | HAM.ER | 0 |
| 4 | HU0 | HU1 | HU2 | HU3 | HAM.ER | 0 |
| 5 | HT0 | HT1 | C5 | C6 | HAM.ER | 0 |
| 6 | C7 | C8 | C9 | C10 | HAM.ER | 0 |
| 7 | C11 | C12 | C13 | C14 | HAM.ER | 0 |
| 8 | MAG0 | MAG1 | MAG2 | 0 | FOUND̅ | 0 |
| 9 | 0 | 0 | 0 | 0 | 0 | PBLF |

D6 and D7 are always 0.
- "FOUND: LOW for page has been found"
- "PBLF: page being looked for"

The same format appears in the SAA5243 [5243 p.16] and SAA5281 [5281 p.17].

---

## 4. Register map (Table 2 [H p.22], descriptions [H p.23–24])

General rules:
- "R0 to R10 are WRITE only; R11 is READ/WRITE; R11B is READ only." [H p.22]
- '−' bits must be written 0 [H p.23].
- Power-up: all R0–R10 bits = 0, "except bits D0 and D1 of registers R1, R5 and R6 which are set to logic 1" [H p.23]. This means T1 = T0 = 1, and PON IN = PON OUT = 1 (INFERRED).
- The I²C slave address is 0010001 [H p.23].
- Auto-increment follows the register descriptions: R0→R1→R2→R3 (R3 does not auto-increment itself), R4→R5→R6→R7, and R8→R9→R10→R11/R11B [H p.23–24].
- [H p.22] says page requests are in "Table 4"; the correct reference is Table 3.

**R0 Advanced control:**
- D0 R11̅/R11B SELECT: "Selects reading of R11 or R11B". 0 = R11 per the overbar; [5281 p.22] says "R11 if LOW or R11B if HIGH".
- D1 VCR MODE: short PLL time constant when 1.
- D2 DISABLE ODD/EVEN: "Forces ODD/EVEN output LOW when logic 1".
- D3: unused.
- D4 DISABLE HDR ROLL: disables the green rolling header and time.
- D5 AUTO ODD/EVEN: "When set forces ODD/EVEN low if any TV picture displayed, if DISABLE ODD/EVEN = 0".
- D6 FREE RUN PLL: forces the PLL to free-run.
- D7 X24 POS: "Automatic display of FASTEXT prompt row when logic 1". Per Fig.10 and Fig.11, 1 = X/24 stored in display row 24 and 0 = X/24 stored in extension row 20.

**R1 Mode:**
- D0 T0 and D1 T1: interlace control (Table 4).
- D2 TCS ON / FFB MODE: "Text composite sync or direct sync select".
- D3 DEW̅/FULL FIELD: field-flyback or full-channel acquisition.
- D4 EXT. PACKET ENABLE: "Allocates 2K bytes memory per chapter".
- D5 ACQ̅ ON/OFF: "Acquisition circuits turned off when logic 1".
- D6 7̅+̅P̅/8-BIT.
- D7 VCS TO SCS: "When logic 1 enables display of messages with 60 Hz input signal".

**Table 4 (T1, T0, TCS ON) [H p.25]:**

| TCS ON | T1 | T0 | Result |
|---|---|---|---|
| x | 0 | 0 | interlaced 312.5/312.5 lines |
| x | 0 | 1 | non-interlaced 312/313 lines |
| x | 1 | 0 | non-interlaced 312/312 lines |
| 0 | 1 | 1 | SCS mode, FFB leading edge in first broad pulse ("M7 version onwards") |
| 1 | 1 | 1 | SCS mode, FFB leading edge in second broad pulse ("M7 version onwards") |

Note 1 to Table 4: "Reverts to interlaced mode if a newsflash or subtitle is being displayed."

**R2 Page request address:**
- D0–D2 START COLUMN SC0–SC2.
- D3 TB: "must be logic 0 for normal operation".
- D4, D5 ACQ CIRCUIT A0, A1.
- D6 BANK SELECT A2.
- D7 HAM. CHECK: Hamming check of 27/0, 27/1 and 8/30.

**R3 Page request data:** PRD0–PRD4 (see §3.2). R3 does not auto-increment to another register; the column pointer auto-increments instead.

**R4 Display chapter:** A0–A2 select the displayed page (1 of 8).

**R5 Display control (normal) and R6 Display control (newsflash/subtitle):** each has 4 function pairs, IN and OUT. "These functions have IN and OUT referring to inside and outside the boxing function respectively." [H p.24]
- D0/D1 PON IN/OUT: picture on.
- D2/D3 TEXT IN/OUT: text on.
- D4/D5 COR IN/OUT: contrast reduction on.
- D6/D7 BKGND IN/OUT: background colour on.

R5 is used for normal pages and R6 for newsflash/subtitle pages. The automatic selection by the C5/C6 bits of the displayed page is INFERRED from the register names and the Table 4 note; the mechanism itself is UNVERIFIED.

**R7 Display mode** (does not auto-increment):
- D0 BOX ON 0: boxing allowed on row 0.
- D1 BOX ON 1-23.
- D2 BOX ON 24.
- D3 DOUBLE HEIGHT: "To display double height text". This is a whole-page zoom; see §5.
- D4 BOTTOM HALF: "To select bottom half of page when DOUBLE HEIGHT = 1".
- D5 REVEAL ON: "To reveal concealed text".
- D6 CURSOR ON: "To display cursor".
- D7 STATUS TOP: "Row 25 displayed above or below the main text" (see §5 on the row numbering).

**R8 Active chapter:** A0–A2 select the chapter for I²C access. D3 CLEAR MEM is self-resetting.

**R9 Cursor row:** R0–R4.

**R10 Cursor column:** C0–C5.

**R11 Cursor data:** "Data read from/written to memory via I2C, at location pointed to by R8, R9 and R10. This location automatically increments each time R11 is accessed" [H p.24]. Wrap behaviour at the end of a row is UNVERIFIED.

**R11B Device status (read):**
- D0 VCS SIGNAL QUALITY.
- D1 DATA QUALITY.
- D2–D6 ROM VER R0–R4: "Indicated language/ROM variant".
- D7 625̅/525 SYNC.

---

## 5. Display

- Format: "40 characters by 25 rows. Rows 0 to 23 form the teletext page; row 24 is the extra row" [H p.19].
- Display timing (Fig.5 [H p.15]):
  - active line from 16.67 µs to 56.67 µs (40 µs)
  - field "lines 42 to 291 inclusive (and 355 to 604 inclusive interlaced)"
  - 250 lines / 25 rows = **10 TV lines per character row per field** (INFERRED arithmetic)
  - 40 µs / 40 characters = 1 µs per character (INFERRED)
- **Character cell 12 × 10.**
  - [H] has no text statement of this.
  - Table 14 "SAA5146A basic character matrix" [H p.37] (the "5146A" is a misprint in the source) draws every glyph on a grid that measures 12 × 10 cells. This was checked from a 300 dpi render: in page coordinates the grid is 10 × 12, and the figure is printed rotated 90°.
  - The SAA5243 lists "High quality flicker-free display using a 12 x 10 character matrix" [5243 p.1].
  - Orientation is INFERRED as 12 dots wide × 10 lines high (per field), which is consistent with 10 lines per row above.
- Character rounding / smoothing (interlaced interpolation) is **UNVERIFIED**; it is not mentioned in [H]. Fig.13 [H p.27] shows only these formats:
  - space
  - alphanumerics
  - "alphanumerics or blast-through alphanumerics"
  - contiguous graphics
  - separated graphics
- **Level 1 serial attributes** (control columns 0 and 1, Tables 6–13 [H p.28–35]):
  - 0/0–0/7: alpha black, red, green, yellow, blue, magenta, cyan, white. **Alpha black (0/0) is decoded.**
  - 0/8 flash, 0/9 steady
  - 0/10 end box, 0/11 start box
  - 0/12 normal height, 0/13 double height
  - 0/14 SO, 0/15 SI
  - 1/0–1/7: graphics black to graphics white. **Graphics black (1/0) is decoded.**
  - 1/8 conceal display
  - 1/9 contiguous graphics, 1/10 separated graphics
  - 1/11 ESC (/E /H /T /J) or **TWIST** (/K /L /I /S)
  - 1/12 black background, 1/13 new background
  - 1/14 hold graphics, 1/15 release graphics
- Table footnotes:
  - "(1) These control characters are reserved for compatibility with other data codes": SO, SI and ESC.
  - "(2) These control characters are presumed before each row begins". Per the /I and /J tables [H p.31, p.34], these are:
    - alpha white
    - steady
    - end box
    - normal height
    - contiguous graphics
    - black background
    - release graphics
  - "Control characters shown in Columns 0 and 1 are normally displayed as spaces." [H p.36]
  - "Columns 2a, 3a, 6a and 7a are displayed in graphics mode." [H p.36]
- Set-at versus set-after timing for each attribute and hold-graphics behaviour are UNVERIFIED; the text does not describe them.
- **Flash rate:** UNVERIFIED (not stated).
- **Conceal/reveal:** the 1/8 attribute, plus R7 D5 REVEAL ON [H p.24].
- **Boxing and display modes:**
  - Per-region picture, text, contrast-reduction and background enables for inside and outside boxes (R5, R6) [H p.24].
  - Boxing is enabled per region: row 0, rows 1–23 and row 24 (R7 D0–D2).
  - Text, TV, mix, and box (newsflash/subtitle) modes are all compositions of the PON, TEXT, COR and BKGND bits.
  - COR pin: "programmable output to provide contrast reduction of the TV picture for mixed text and picture displays or when viewing newsflash/subtitle pages" [H p.4].
  - Y pin: foreground-only output [H p.4].
  - BLAN: "dot rate fast blanking output" [H p.4].
- **Page zoom:** R7 DOUBLE HEIGHT with BOTTOM HALF selects the top or bottom half enlarged [H p.24]. This is separate from the 0/13 double-height attribute.
- **Cursor:** R7 D6 CURSOR ON. The position tracks R9/R10 (INFERRED). [5281 p.24] confirms this for IVT1.8 with its new CURSOR FREEZE bit: "cursor position not updated even if active row and column change". Cursor appearance is UNVERIFIED.
- **Screen colour:** no full-screen or border colour register exists in the R0–R11 map [H p.22]. INFERRED: none.
- **Status row placement:** R7 D7 STATUS TOP: "Row 25 displayed above or below the main text" [H p.24].
  - Only 25 display rows exist (250 lines / 10).
  - This bit INFERRED moves the 25th display row (memory row 24, the status/FLOF row) above or below rows 0–23.
  - Memory row 25 is control data and is INFERRED not displayed. [5243 p.17] uses the same "row 25" wording while calling row 24 "this row always free for status" [5243 p.16].
- **Fastext row 24 display:** R0 D7 X24 POS "Automatic display of FASTEXT prompt row when logic 1" [H p.23]. X/24 is stored in display row 24 [H p.19].
- **ODD/EVEN pin:** "a 25 Hz output synchronized to the input CVBS field sync pulses to make a non-interlaced display by adjustment of the vertical deflection currents" [H p.4]. It is gated by R0 D2 and D5. Timing is in Fig.7 [H p.16].
- **525-line (60 Hz):** VCS TO SCS "enables display of messages with 60 Hz input signal" [H p.23]. The chip only acquires 625-line WST [H p.2].

---

## 6. Level 1.5 and packet X/26

- **[H] contains no description of any X/26 processing.** Its only X/26 content is the storage map: "PACKETS X/26/0 - X/26/14" in extension rows 0–14 when EXT. PACKET ENABLE is set [H p.21]. That is 15 packets of 40 bytes each (designation code plus 13 triplets), one per row (INFERRED).
- X/26/15 has no row and is INFERRED not stored.
- **INFERRED conclusion:** on the SAA5246A, Level 1.5 is a **microcontroller software function**. Software reads the X/26 triplets over I²C and writes replacement character codes into the display chapter. The chip has no triplet decoder and no 24/18 Hamming decoder.
- Supporting evidence:
  - SAA5243: "For certain languages, control software processing of the extension packet data may be required for optimum useage of the range of available characters." [5243 p.20]
  - SAA5281 (IVT1.8) adds "Automatic packet 26 extension character processing" and a "24 : 18 Hamming checker" as new features [5281 p.2].
  - R13 DISABLE PKT X/26 on the SAA5281: "Output taken from processing engine written to the display memory when logic 0" [5281 p.24].
- **ROM support for software Level 1.5.** Every character table (Tables 6–13) includes glyph columns **8, 9, 12, 13, 14 and 15 addressed with b8 = 1** [H p.28–35]. These glyphs can only be reached by a display-memory byte with bit 8 set, which software writes or 8-bit mode produces (INFERRED). They hold accented Latin letters in /E /H /T, Cyrillic in /J, Arabic in /K /L, Greek in /I and Thai in /S.
  - Note 5 [H p.36]: "Characters 8/6, 8/7, 9/5, 9/6 and 9/7 are special characters for combining with character 8/5 (E, H and T codes only. Characters 5/12, 5/13, 5/14 and 5/15 are combined with 5/11 (S code only)."
  - Columns 10 and 11 with b8 = 1 are not shown.
  - The b8 row marks columns 2 and 3 as "0 or 1" (all variants), and columns 6 and 7 as "0 or 1" in /K /L. INFERRED meaning: these glyphs are the same with b8 set.
- Which X/26 triplet modes are supported: **UNVERIFIED / none in hardware.** No triplet list, G2 or diacritic handling, or X/26 limits appear in [H].

---

## 7. National option ROM variants

- [H p.27]: "The WST specification allows the selection of national character sets via the page header transmission bits, C12 to C14. The basic 96 character sets differ only in 13 national option characters … The SAA5246A automatically decodes transmission bits C12 to C14."
- The 13 national option positions are 2/3, 2/4, 4/0, 5/11–5/15, 6/0 and 7/11–7/14 [H p.37–40].
- The variant is readable in R11B ROM VER R0–R4 [H p.24].
- The PHCB column in the tables below lists C12 C13 C14.

| Variant | ROM VER | Title [H] | PHCB → language | Default |
|---|---|---|---|---|
| /E | 00000 [p.28] | West European (Table 6, 15) | 000 English, 001 German, 010 Swedish, 011 Italian, 100 French, 101 Spanish [p.38] | English |
| /H | 00001 [p.29] | East European (Table 7, 16) | 000 Polish, 001 German, 010 Swedish, 101 Serbo-Croat, 110 Czechoslovakia, 111 Rumanian [p.39] | German |
| /T | 00010 [p.30] | West European and Turkish (Table 8, 17) | 000 English, 001 German, 110 Turkish, 011 Italian, 100 French, 101 Spanish [p.40] | English |
| /J | 00011 [p.31] | Jugoslav (Table 9, 18) | Serbo-Croat Latin, Hungarian/Swedish, Serbo-Croat, Rumanian, and 100 Serbo-Croatic **Cyrillic** (a full replacement of columns 2–7) [p.41] | Serbo-Croat Latin |
| /K | 00100 [p.32] | French and Arabic (Table 10, 19) | 100 French, 111 Arabic (full alternate set, columns 2–7) [p.42] | French |
| /L | printed **00100** [p.33] | Arabic and Hebrew (Table 11, 20) | 101 Hebrew/English, 111 Arabic [p.43] | Hebrew English |
| /I | 00110 [p.34] | Greek and Turkish (Table 12, 21) | 110 Turkish, 111 Greek [p.44] | Turkish |
| /S | 00111 [p.35] | English and Thai (Table 13, 22–23) | 000 English, 010 Thai; per [p.45–46], "English (000) TWIST not invoked OR Thai (010) and TWIST invoked" selects set A, and the converse selects set B | English |

Notes on the table:
- **/J PHCB codes:** Table 18 [H p.41] prints the PHCB codes inconsistently. The row codes are 010, 011, 011, 011, 011, 011, 011, then 100 for Cyrillic. The individual codes are unreliable, so treat them as UNVERIFIED.
- **/L ROM VER:** 00100 duplicates /K. [5281 p.32–33] repeats 00100 for both /L and /K. The code 00101 is unused in [H], and the SAA5281 uses 00101 for its /R. The true /L code is UNVERIFIED.
- No other suffixes appear in [H].
- **TWIST (1/11 in /K /L /I /S)** is not defined in the text. From Tables 22–23 [H p.45–46], it is INFERRED to toggle between the variant's two sets (an ETSI-style ESC toggle between primary and secondary G0).
- ESC (/E /H /T /J) is "reserved for compatibility" [H p.28].
- "Option for up to seven national languages" [H p.2].

---

## 8. Fastext/FLOF (X/27) and TOP

- **Hardware support:**
  - X/27/0–1 and X/27/4–5 are stored in extension rows 16–19 [H p.21].
  - Optional Hamming 8/4 check of 27/0 and 27/1 (R2 D7) [H p.23].
  - X/24 (the prompt row) is stored in display row 24, with automatic display under R0 D7 X24 POS [H p.19, p.23].
  - Row 24 is "for software generated status messages and FLOF/FASTEXT prompt information" [H p.19].
- **Left to the microcontroller (INFERRED):**
  - decoding link page numbers and subcodes from X/27/0
  - responding to colour keys
  - issuing page requests for linked pages (prefetch into other chapters)
  - the X/27/0 link-control byte and the "display row 24" flag
- **TOP:** the word "TOP" occurs in [H] only as the register bit name "STATUS TOP" [H p.22, p.24]. **TOP (Table of Pages) is not mentioned at all in the SAA5246A document.** It first appears in the successor: "Enhanced mode allows 7 Fastext pages and 8 pages of TOP to be captured" [5281 p.2].

## 9. Packet 8/30

- "Row 23 of the extension page … contains packet 8/30. Packet 8/30 is mapped into the SAA5246A memory as follows:" [H p.20]
  - "8/30/0 and 8/30/1 to Chapter 4 Row 23"
  - "8/30/2 and 8/30/3 to Chapter 5 Row 23"
  - "8/30/4 to 8/30/15 to Chapter 6 Row 23"
- This requires EXT. PACKET ENABLE. Only magazine 8 is stored, which is INFERRED from the "8/30" naming.
- Optional Hamming 8/4 check applies via HAM.CHECK: "8/30/x" [H p.23].
- No hardware interpretation is described for the initial page, the network ID, time/date or the status display. These are INFERRED to be the microcontroller's job.
- Because several designation codes share one row, later packets overwrite earlier ones (INFERRED). For example, 8/30/0 and 8/30/1 share a row, and 8/30/4–15 share another.

---

## 10. Font document [F]

- [F] reproduces [H] pages 27–46. It contains:
  - Fig.13 "Character format" (the space, alphanumeric, blast-through and contiguous/separated graphics cell layouts)
  - Tables 6–13 (full decoding tables for each variant)
  - Notes to Tables 6–13
  - Table 14, the basic 96-character matrix drawn on grids, with "NC = national option character position"
  - Tables 15–23 (national option sets per variant)
- **Organisation:**
  - Code columns 0–1 are control codes, normally displayed as spaces.
  - Columns 2–7 hold the 96-character G0 set.
  - Columns 2a, 3a, 6a and 7a are the block-mosaic (2 × 3 sixel) graphics shown in graphics mode.
  - Blast-through: columns 4 and 5 show alphanumerics while in graphics mode (Fig.13).
  - Columns 8, 9 and 12–15 (b8 = 1) hold extra glyphs specific to each variant.
  - "Character bytes are listed as transmitted from b1 to b7." [H p.27]
- **Matrix:** a 12 × 10 grid (see §5).
- **Repertoire by variant:**
  - /E, /T: Latin with Western accents, plus Turkish in /T.
  - /H: Central European Latin with carons, ogoneks and double acutes.
  - /J: Serbo-Croat Latin and Serbian/Macedonian Cyrillic.
  - /K: French and Arabic, including contextual forms.
  - /L: Hebrew and Arabic.
  - /I: Turkish and Greek.
  - /S: English and Thai, including combining vowel and tone marks in columns 5/12–5/15 and the 8/x and 9/x columns.

## 11. SAA5243 versus SAA5246A versus SAA5281

| Topic | SAA5243 (ECCT) | SAA5246A (IVT1.0) | SAA5281 (IVT1.8) |
|---|---|---|---|
| Front end and memory | Separate SAA5231 VIP2 supplies TTD/TTC (6.9375 MHz), F6 (6 MHz) and VCS; external 8K SRAM; DIL40 [5243 p.1–3] | Integrates the VIP (slicer, clock PLL, sync separator) on a 27 MHz crystal; external 8K SRAM [H p.2–3] | Integrates the VIP; **on-chip 8K × 8 DRAM**; VPS decoding; 13.5 MHz clock output [5281 p.2] |
| Registers | R1–R11; no R0 or R11B | Adds R0 advanced control and R11B device status | Adds R12 (per-channel data-check algorithm, Table 12) and R13 (enhanced mode, meshing, X/26 engine control, VPS, 1.8 mode) [5281 p.21–25] |
| Row 24 | "always free for status" [5243 p.16] | X/24 stored in row 24 or extension memory, with automatic Fastext row display | Same as SAA5246A |
| Hamming checking | — | 8/4 on 27/0, 27/1 and 8/30 | 24/18 checker and automatic packet 26 processing |
| Status and signal detection | — | 625/525 detection, text and video quality flags, VCS-to-SCS, auto ODD/EVEN, PLL free-run, header-roll disable | Same as SAA5246A |
| ROM variants | /E/M2, /H, /K, /L, /T [5243 p.2]; the scan also shows a "SAA5243R/L" table (Estonian, Lettish/Lithuanian, Russian) [5243 p.34] | Eight variants: /E /H /T /J /K /L /I /S | /E /H /T /R /L /K |
| Page requests and storage | 4 acquisition circuits, 8 chapters | Same as SAA5243 | Enhanced mode stores 7 Fastext pages plus TOP; SUBTITLE request bit; column-7 chapter pointer per request |
| Display extras | — | — | Meshing, freeze header, cursor freeze, DV output, line-23 output |
| Compatibility | — | — | "Software and hardware compatible with SAA5246 and SAA5248" [5281 p.2] |

The row 0, row 25 and page-request formats are identical in all three [5243 p.16–19], [H p.19–25], [5281 p.16–25].

## 12. Deviations and additions relative to ETSI, and choices the handbook documents

- **25th display row (row 24)** is used for software status and the X/24 prompt row. The status row can be placed above or below the page (R7 D7) [H p.19, p.24].
- **Row 0 columns 0–6 are reserved for status.** The chip does not write the transmitted header bytes 0–7 there. The page number shown is software's, and column 7 is overwritten with the white/green alpha marker [H p.19].
- **Green rolling header** is shown while searching [H p.19].
- **Subcode matching is by BCD digit with per-digit "do care" masks**, in the old hours/minutes (timed-page) interpretation rather than as an ETSI 13-bit subcode [H p.25].
- **Separate display-control sets** exist for normal and for newsflash/subtitle pages (picture, text, contrast reduction and background, each inside and outside boxes) [H p.24]. The non-interlaced modes revert to interlaced while a newsflash or subtitle is displayed [H p.25].
- **Boxing can be enabled or disabled separately** for row 0, rows 1–23 and row 24 [H p.24].
- **Alpha black (0/0) and graphics black (1/0) are decoded**, which are Level 2.5-style codes, and ESC or TWIST are implemented in ROM [H p.28–35].
- **Extra ROM glyphs sit at b8 = 1 code positions**, a non-ETSI 8-bit extension used for software Level 1.5 [H p.28–35].
- **PHCB combinations not in a variant fall back to a fixed default language** for each variant, e.g. "Other combinations default to English" [H p.38–46].
- **Only a subset of extension packets is stored**, with fixed row slots [H p.21]:
  - X/26/0–14
  - X/27/0, 1, 4, 5
  - X/28/0, 1, 2
  - X/24, X/25
  - 8/30 (4–15 collapsed into one row)
- **Parity-error, Hamming-error, serial/parallel magazine, C4 erase, C7 suppress header, C8 update, C9 interrupted sequence and C10 inhibit display** policies: none is documented in [H]. All are **UNVERIFIED**; C4–C14 are only stored in row 25 [H p.20].

### Documentation errata found

- Fig.10 prints "R1D7" where R0D7 is meant [H p.19]; compare [5281 p.16].
- [H p.22] references "Table 4" for page requests; the correct reference is Table 3.
- Table 14 is titled "SAA5146A" [H p.37].
- The /L ROM version duplicates /K's 00100 [H p.33].
- The /J PHCB codes are printed inconsistently [H p.41].
- Note 6 on [H p.36] references "Table 8" where the national-option tables are meant.
