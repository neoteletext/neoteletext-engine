# Teletext signal layer and data-slicer front end: cited notes

Purpose: facts for the requirements of a C teletext receiver that slices raw sampled VBI lines in software.
Everything below cites a source. Anything marked **UNVERIFIED** could not be confirmed in the local documents.
Arithmetic that I derived is marked **DERIVED** and shows its working.

## Source key (all paths relative to `/Users/andrewstuart/devel/microcontrollers/teletext/`)

| Key | Document | Page convention used |
|---|---|---|
| EN706 | `specifications/EN_300_706_v1.2.1_Enhanced_Teletext.pdf` (ETSI EN 300 706 V1.2.1, 2003-04) | printed ETSI page number |
| ETS706-97 | `specifications/ETS_300_706_Enhanced_Teletext_1997.pdf` | cross-check only |
| BT653 | `specifications/ITU-R_BT.653-3_Teletext_Systems_1998.pdf` (Rec. ITU-R BT.653-3) | printed page (PDF page in brackets) |
| SAA5231 | `philips-saa5231/SAA5231_Teletext_Video_Processor_VIP2_Philips.pdf` (Product spec, Nov 1986) | printed page |
| SDA5231 | `siemens-sda5231/SDA5231-5_Data_Slicer_for_Teletext_Siemens.pdf` (Preliminary Data; scanned, no text layer) | printed page (PDF page = printed − 4) |
| SAA5246A | `philips-saa5246a/PHILIPS-SAA5246A-INTEGRATED-VIP-TELETEXT-(IVT1.0)-HANDBOOK.PDF` (Jan 1993) | printed page |
| SAA5030 | `databooks/Philips_1979_ICs_for_Digital_Systems_in_Radio_and_TV_part6b.pdf`, SAA5030/SAA5040 sheets | PDF page |
| TPU2735 | `itt-tpu2735/TPU2735_Teletext_Processor_Unit_ITT.pdf` | printed page |
| GEN | `TELETEXT-GENERATION-ANALYSIS.md` (existing analysis written from the generation side) | section |

---

## 1. System B signal parameters (625-line)

### 1.1 Scope
- EN 300 706 applies "CCIR Teletext System B to CCIR 625 Line 50 field Television Systems B, D, G, H, I, K and L" [EN706 §1 Scope, p.10]. It defines **no** 525-line variant (see §7 below).

### 1.2 Coding, bit rate, levels, spectrum
| Parameter | Value | Citation |
|---|---|---|
| Signalling method | Binary Non-Return-to-Zero (NRZ) | EN706 §5.1 p.14; BT653 Table 1a item 1.6, printed p.8 (PDF p.10) |
| Bit rate | **444 × nominal fH (6,9375 Mbit/s ± 25 ppm)** | EN706 §5.3 p.14 |
| Bit rate (ITU wording) | 6.9375 Mbit/s ± 25 × 10⁻⁶ | BT653 Table 1a item 1.4, p.8 |
| "0" level | Black Level ± 2 % | EN706 §5.2 p.14; BT653 Table 1a 1.3 |
| "1" level | 66 ± 6 % of the difference between Black Level and Peak White Level (EN706). BT653 wording: "66% (± 6%) of black-to-white excursion" | EN706 §5.2 p.14; BT653 Table 1a 1.3 p.8 |
| Spectrum / pulse shaping | "Skew symmetrical about 0,5 × bit rate, substantially zero by 5 MHz" (Figs 2 and 3) | EN706 §5.4 p.15; BT653 Table 1a 1.5 ("Spectral shaping is skew symmetrical about 0.5 bit rate") |
| One-bit pulse | Fig. 2 shows the approximate one-bit pulse with the x axis in bit periods "(units of approx. 144 ns)". Fig. 3 shows the spectrum: 0.5 amplitude at the centre of skew symmetry (≈3.47 MHz), the shape "determined by transmission filters", and the plot running from 0 to 7 MHz | EN706 Figs 2 and 3, p.15 |
| Data line length | 360 bits including clock run-in | BT653 Table 1a 1.7 p.8 |
| Packet | "A Teletext packet comprises 360 bits organized as 45 bytes, numbered 1 to 45 … bits numbered 1 to 8 (LSB to MSB) … normally transmitted LSB first" | EN706 §7.1 p.16 |
| Packet per line | "A Teletext data packet occupies one TV line" | EN706 §6 p.15 |
| Data unit length (after run-in) | 43 bytes | BT653 Table 1a item 2.2 |
| ITU caveat | Note (2) to Tables 1a/1b: "Parameters for data positioning, amplitude and shaping may be altered to suit particular transmission requirements." | BT653 Notes to Tables 1a and 1b, PDF p.22 |

- The spec gives **no numeric roll-off factor** (for example "100 % raised cosine") for System B. It gives only skew symmetry about half the bit rate and "substantially zero by 5 MHz". By contrast, BT653 Table 1b gives System C as "raised cosine 100% roll-off" and System D a roll-off factor of 0.6 [BT653 Table 1b 1.5, p.14 (PDF p.16)]. A System B roll-off figure is **UNVERIFIED**.
- **Eye height:** neither EN706 nor BT653 specifies an eye height or an eye-opening figure. **UNVERIFIED** (not in the local documents).
- Cross-check: ETS706-97 has identical wording for levels, bit rate, spectrum, lines and timing (lines 687, 729–739, 808 of its text extraction).

**DERIVED (bit timing):**
- fH = 15 625 Hz → 444 × 15 625 = **6 937 500 bit/s**.
- T_bit = 1/6.9375 MHz = **144.144 ns**, which matches EN706 Fig. 2 ("approx. 144 ns").
- 444 bits × 144.144 ns = 64.000 µs, i.e. exactly one line period.
- Packet duration = 360 × 144.144 ns = **51.892 µs**.
- ±25 ppm over 360 bits = ±0.009 bit, which is negligible within a line.

### 1.3 Clock run-in, framing code, byte order
- Clock run-in: "The bit pattern in transmission order is: 1010101010101010" (16 bits, even parity). It is "used for bit synchronization within a decoder" [EN706 §6.1 p.15].
  - NOTE (important for a slicer): "The clock run-in does not in itself identify a Teletext data line. Under some conditions, the two leading data 'ones' may be absent or reduced in amplitude compared to the subsequent data." [EN706 §6.1 NOTE, p.15]
- Framing code: "The bit pattern in transmission order is: 11100100" (even parity). It is "used for identification of a Teletext data line and for byte synchronization". "Following the detection of the framing code sequence, a decoder should partition the remaining data bits of the packet into 8-bit bytes." [EN706 §6.2 p.15]
  - NOTE: "Other framing code sequences may be used when the data packets are not intended for reception by decoders conforming to the present document." [EN706 §6.2 NOTE, p.15]
  - BT653 wording: "Byte 3 = 11100100" [Table 1a 2.1]. (System A uses 11100111, which a System B receiver should not accept; BT653 Table 1a 2.1.)
  - **DERIVED:** sent LSB-first, 11100100 is the byte value **0x27**. The run-in bytes 1 and 2 are each 0x55 (10101010 LSB-first).
- Bytes 1, 2 and 3 (run-in plus framing) "enable Teletext packets to be identified and synchronization to be established" [EN706 §7.1.1 p.16].
- Packet address (MRAG) = bytes 4 and 5, both Hamming 8/4 [EN706 §7.1.2 p.17]. BT653 also lists "Even parity for bytes 1 to 3; odd parity for bytes 4 to 45" [Table 1a 2.4.1].

### 1.4 Timing of the data relative to line sync
- "The timing reference is measured from the half amplitude point of the leading edge of the line synchronizing pulse to the mid point of the penultimate '1' of the clock run in" [EN706 §6.3 p.16].
- Figure 4 value: **12 +0,4 / −1,0 µs** [EN706 Fig. 4 p.16].
- NOTE: "Nominally, the data is inserted with the point of reference at 12,0 µs after the half-amplitude point … it may be necessary to depart from this to allow for the re-timing of the synchronizing pulses on some networks, particularly as a result of sync reprocessing in some transmitters …" [EN706 §6.3 NOTE, p.16].
- BT653: "Bit 13 is reference plus 12.0 μs (−1.0, +0.4)" [Table 1a 1.2 p.8]. Bit 13 is the penultimate '1' of the run-in (the '1's sit at bits 1, 3, …, 15), so the two documents agree.

**DERIVED (sample-window planning):**
- Bit 1 centre = 12.0 − 12 × 0.144144 = **10.270 µs**.
- Bit 1 leading edge ≈ **10.198 µs**.
- End of bit 360 ≈ 10.198 + 51.892 = **62.090 µs** after the sync half-amplitude point (nominal).
- Allowing −1.0 / +0.4 µs, bit 13's centre lies between 11.0 and 12.4 µs, so bit 1 starts between about 9.2 and 10.6 µs. A software slicer must therefore search for the run-in over a window of at least 1.4 µs (≈10 bit periods).
- Because decoders in the field are known to accept a much wider window (see §5), a wider search window (for example ±2 µs or more) is reasonable. That design choice is not specified by any of the documents.

### 1.5 VBI lines that may carry teletext
- "Subject to availability, lines 6 to 22 and 318 to 335 may be used to carry Teletext data packets" [EN706 §4.1 p.13]. Figure 1 shows the field-blanking interval (25 lines) with line numbers 622…24 (first field) and 310…335 (second field) [EN706 Fig. 1 p.14].
  - **DERIVED:** that gives 17 lines in the first field (6–22) and 18 lines in the second field (318–335).
  - Line numbering is the standard 625-line frame numbering (1–625). Line 318 corresponds to line 5 of the second field (318 − 313 = 5). The spec uses frame line numbers only; the mapping into a per-field index for a capture API is **UNVERIFIED** here and is capture-driver-specific.
- Practical caution: "Some existing Level 1 and 1.5 decoders may not decode Teletext signals on lines 6, 318 and 319. Thus these lines should be used for Level 2.5 or 3.5 enhancement data only, or non-Teletext signals" [EN706 Annex F.4 p.138]. A receiver should therefore **accept** these lines.
- Lines 22 and 335: some decoders do not display the clock bytes of headers carried only on those lines [EN706 Annex F.5 p.138]. A receiver should decode lines 22 and 335 normally.
- Full-field mode (not multiplexed with video): "all lines may be used … except those occurring during the field synchronization and equalizing pulse periods" [EN706 §4.2 p.14; practical notes F.6 p.139]. BT653: "Active part of any TV line subject to availability" [Table 1a 1.1].
- Other signals share these lines: "TV lines 6-22 and 318-335 may be used for test signals, noise measurements, encryption and other information services as well as Teletext" [EN706 Annex P p.153]. SDA5231 notes that the chip "can also be used to process the data line 16 (VPS)" [SDA5231 p.5]. A slicer must therefore reject non-teletext lines.

---

## 2. Receiver tolerance figures (amplitude, ghosting, noise, jitter)

What the specs do and do not say:
- **Transmission tolerances only.** EN706 and BT653 give only transmitter-side limits:
  - levels: 0 at black ±2 %, 1 at 66 ±6 %;
  - bit rate: ±25 ppm;
  - timing: 12 µs +0.4/−1.0.
  Refs: EN706 §5.2, §5.3, §6.3; BT653 Table 1a.
- **No receiver performance limits** (minimum S/N, echo/ghost amplitude and delay, BER, timing jitter tolerance, eye height) appear in EN706 or BT653. Searches for "eye", "ghost", "echo", "jitter" and "noise" found only the Annex P mentions. **UNVERIFIED / not specified.** EN706 refers readers to ETSI TR 101 233 for VBI signal allocation [EN706 ref [7] and F.4], and that document is not held locally.
- Qualitative requirements from EN706 Annex P, p.153:
  - "Teletext decoders should be designed to be insensitive to non-Teletext waveforms."
  - "It is not reasonable to assume that the type of waveform on a given line on any channel will remain constant."
  - Non-teletext signals (for example Videocrypt) "can be interpreted as valid Teletext by some existing decoders", especially "in the presence of noise".
- Real hardware tolerances that can act as guides:
  - **Amplitude range (SAA5231 / SDA5231):**
    - Video input sync-to-white 0.7 / 1.0 / 1.4 V (min / typ / max) at the 1 V setting, or 1.75 / 2.5 / 3.5 V at the 2.5 V setting.
    - Teletext **data amplitude 0.30 / 0.46 / 0.70 V** (1 V setting) or 0.75 / 1.15 / 1.75 V (2.5 V setting).
    - Sync amplitude 0.1 to 1 V.
    - Refs: SAA5231 Characteristics p.5–6; SDA5231 Characteristics p.12.
    - **DERIVED:** 0.46 V ≈ 66 % × 0.7 V, the nominal black-to-white span of a 1 V p-p signal. The accepted data amplitude therefore runs from 0.30/0.46 = 65 % to 0.70/0.46 = 152 % of nominal (about −3.7 dB to +3.6 dB). This is the chip's working range, not a spec requirement.
  - **SAA5246A:** video amplitude 0.7 to 1.4 V p-p, sync 0.1 to 0.6 V [SAA5246A Characteristics p.9].
  - **Ghosting (ITT TPU 2735, digital slicer):** "the input data is processed by a ghost filter which is able to compensate reflections with short delay time of 0 to 0.8 µs for PAL or 0 to 1 µs for NTSC" [TPU2735 §2 Functional Description, p.5]. The ghost compensation can be turned off (NGC) or reset (RGC) through register R-DAC [TPU2735 §7.5 p.21]. This is the only echo figure found in the local documents. It is a product capability, not a spec requirement.
  - **HF loss:** SAA5231 has an "HF loss compensator" (pins 3 and 4) [SAA5231 Fig.1 p.3; pin functions p.10]. In SDA5231 terms, the slicer "compensates the attenuation of the data signal due to mistuning and a non linear frequency response [of] the IF-amplifier" [SDA5231 §a Signal Processing, p.6].
- Noise tolerance, BER and jitter-tolerance figures for SAA5231 and SDA5231: **none stated** in either datasheet. **UNVERIFIED.** SAA5246A claims only "Data capture performance similar to SAA5231 (VIP2)" [SAA5246A Features p.2].

---

## 3. Hardware data-slicer designs

### 3.1 Philips SAA5231 (VIP2), bipolar, 1986
Refs: SAA5231 General Description p.2; Fig. 1 p.3; characteristics p.5–7; pin functions p.10–11; component specs p.9.
- Function: "extracts Teletext Data from the video signal, regenerates Teletext Clock and synchronizes the text display to the television syncs". Features listed: "Adaptive data slicer", "Data clock regenerator", "Adaptive sync separator, horizontal phase detector and 6 MHz VCO forming display phase locked loop" [p.2].
- **Sync separation:** composite video enters pin 27 through a 2.2 µF clamping capacitor into the adaptive sync separator. The black level for the adaptive sync separator is stored on a 68 nF capacitor at pin 26 [pin functions 26–27, p.11]. Sync separator delay is 0.25 / 0.35 / 0.40 µs [p.5].
- **Slicing level (adaptive):**
  - pin 5 "The amplitude for the adaptive data slicer is stored by a 470 pF capacitor";
  - pin 6 "The zero level for the adaptive data slicer is stored by a 22 nF capacitor";
  - pin 8 "A 270 pF capacitor … for timing of the adaptive data slicer" [p.10].
  - Interpretation: the slicer holds a zero (black) reference and a data amplitude, and slices between them. The datasheet does not give the exact threshold formula (**UNVERIFIED** that it is exactly the 50 % point).
- **HF compensation:** pin 3 filters the video for the HF-loss compensator (15 pF). Pin 4 stores the HF amplitude (1 nF) [p.10].
- **Clock regeneration:**
  - pin 11 "A 13,875 MHz crystal, 2 x data rate … applied … to the oscillator and divide-by-two to provide the 6,9375 MHz clock signal";
  - pin 12 is a filter for the 6.9375 MHz clock;
  - pin 9 stores the "output signal from the clock phase detector … by a 100 pF capacitor" [p.10].
  - So the clock is a crystal-stable 6.9375 MHz whose phase is adjusted by a phase detector against the incoming data. The block diagram shows CLOCK PHASE DETECTOR, PHASE SHIFTER and OSCILLATOR ÷2 [Fig.1 p.3].
  - Crystal: 13.875 MHz, adjustment tolerance ±40 × 10⁻⁶, temperature tolerance ±30 × 10⁻⁶ [p.9].
  - TTC (clock) falling edge relative to TTD edges: −20 / 0 / +20 ns [p.7].
- **Burst / timing gating:** a sandcastle input (PL/CBB) from the decoder. Fig. 5 shows edges at 0, 1.5, 4.7, 8.5 and 33.5 µs relative to the line sync [p.11]. The SAA5030 predecessor explains the purpose: CBB "is used to prevent the data slicer being offset by the colour burst" [SAA5030, PDF p.150].
- **Framing code detection:** not done in SAA5231. It outputs only serial data (TTD) and clock (TTC) "for CCT" [pins 14–15, p.10]. Framing detection happens in the downstream decoder (SAA5243 / ECCT). The local SAA5243 PDF has no text layer. Framing tolerance for these chips is **UNVERIFIED**.
- Acquisition window figures: none stated. **UNVERIFIED.**

### 3.2 Siemens SDA 5231-5, bipolar (scanned datasheet; read visually)
- Features [p.5]:
  - "Crystal-stable data clock regeneration for a bit rate of 6.9375 MHz"
  - "Separation and regeneration of teletext information"
  - "Separation of the horizontal and vertical synchronization signals"
  - "Phase-locked coupling of 6-MHz oscillator with comp. video"
  - "Optional adjustment to 1 V or 2.5 V comp. video level"
- **a) Signal Processing** [p.6], verbatim:
  - "The analog signal is processed at the amplitude filter (clamping of the video input signal and separation of the sync. impulse) and by means of the data slicing level. The data slicer compensates the attenuation of the data signal due to mistuning and a non linear frequency response [of] the IF-amplifier."
  - "In case of sync. impulse separation, an adaptive circuit is used for separating the data, i.e. the **clipping level is always in the middle of the sync. impulse and/or the data signal, independent of the signal amplitude**."
  - "Both the clipping level and the control voltage for the AGC (Automatic Gain Control) are **generated while bit synchronization takes place at the beginning of the teletext line and are stored until the teletext line has been fully scanned**."
  - The slice level and AGC are therefore acquired during the clock run-in and held for the rest of the line.
- **b) Synchronization of Data Clock** [p.6]:
  - "Clock synchronization is obtained by means of a phase detector + a phaseshifter. A control voltage proportional to the phase difference between the free running oscillator and the incoming data bits is generated by the phase detector."
  - "**A small time constant is used during bit synchronization, for the rest of the time a 10 times larger constant is used.**" This means fast phase acquisition on the run-in, then a slow track over the data.
- **c) 6 MHz system clock** [p.6]: syncs are shortened to about 2 µs by a monoflop. The 6 MHz VCO locks to the shortened sync. "If the CVBS signal is noisy or during after-hours operation … Data acquisition has to be stopped, and the 6-MHz VCO must oscillate with its nominal frequency", under control of the teletext processor through VCS and the sandcastle PL component.
- Pins [p.9–10]:
  - pin 5 CLIP, "Clipping Level of adapted Data Separation";
  - pin 6 ZERO, "Zero Level";
  - pin 8 TIM, "Time Characteristics of adapted Data Separation";
  - pin 9 CPD, clock phase detector;
  - pin 11 XTAL, "13.875-MHz Crystal Connection: frequency for double teletext data rate";
  - pin 12 FIL, "Filter Connection for 6.9375-MHz data clock";
  - pins 3 and 4, "RF-Level Stabilizer";
  - pin 26 BLACK, "Black Level Input Capacitor for the adapted synchronous pulse separator".
- Block diagram [p.7]: Adaptive Sync Separator → RF-Sensitivity → RF-Level Stabilizer → Teletext Data Separator (gated by CBB) → Output Register. Phase Detector → Phase Shifter ← Oscillator/Divider (13.875 MHz).
- Characteristics [p.12]:
  - CVBS input 0.7 / 1 / 1.4 V (pin 2 grounded) or 1.75 / 2.5 / 3.5 V (pin 2 open);
  - sync 0.1 to 1 V;
  - **teletext data level 0.3 / 0.46 / 0.7 V** (pin 2 grounded) or 0.75 / 1.15 / 1.75 V (open);
  - TTC vs TTD deviation −20 / 0 / +20 ns.
- VCS delay with respect to CVBS sync: typ 0.5 µs [p.13].
- Sandcastle [p.14]: CBB is listed as "**Reset pulse for data separation**" (low 0 to 0.5 V, high 1 to 5.5 V). A PL low time of 100 ms or more makes the oscillator free-wheel.
- Pulse diagram [p.21]: video sync 0 to 4.7 µs. VCS 0.5 to 5.2 µs. Sandcastle levels 0/2/5 V with transitions at 1.5, 8.5 and 33.5 µs.
- Framing code detection, error tolerance, minimum S/N and BER: **not stated** (**UNVERIFIED**). As with SAA5231, framing is handled by the downstream decoder.

### 3.3 Predecessor SAA5030 (Philips, 1978–79): the classic slicer, for context
- "The teletext data is sliced with an automatic data slicer whose **slicing level is the mid-point of two peak detectors** working on the video. Storage capacitors are connected to these pins for the negative and positive peak detectors." [SAA5030 pins 23 and 24, PDF p.152]
- Peak detector attack is 15 V/µs; decay is quoted in mV/µs [SAA5030 characteristics, PDF p.148].
- Clock: "Short pulses are derived from both edges of data … fed, as a current, into the clock coil" (a high-Q tuned circuit at pin 21), then limited [SAA5030 pins 21–22, PDF p.152].
- The CBB part of the sandcastle "is used to prevent the data slicer being offset by the colour burst" [SAA5030 pin 5, PDF p.150].
- The data amplitude needed at pin 16 to enable the "amplitude gate flip-flop" is 0.46 V p-p [PDF p.148]. That is an amplitude gate: no slicing below the threshold.
- SAA5040 (acquisition): "Hamming and parity checks are performed on the incoming data to reduce errors" [PDF p.157].

### 3.4 Algorithmic summary a software slicer can copy (from 3.1–3.3)
1. Clamp the black level / DC restore, taken from back porch or sync [SAA5231 pins 26–27; SDA5231 §a].
2. Ignore the colour burst region when deriving levels (CBB gating) [SAA5030 pin 5; SAA5231 Fig. 5].
3. Take the slice threshold as the midpoint of the data peaks, measured on the clock run-in, and **hold it for the rest of the line** [SDA5231 §a; SAA5030 pins 23–24].
4. Take bit phase from the run-in with a fast loop, then track with a loop about 10× slower [SDA5231 §b]. The frequency is crystal-nominal 6.9375 MHz (twice that is 13.875 MHz) [SAA5231 pin 11; SDA5231 pin 11].
5. Optionally compensate HF loss (IF mistuning) [SAA5231 pins 3–4; SDA5231 §a] and short-delay echoes (0–0.8 µs PAL) [TPU2735 p.5].
6. Apply an amplitude gate: don't slice when the data amplitude is below a minimum [SAA5030 amplitude gate].
Steps 1–6 describe hardware behaviour. How a software implementation maps these steps is a design choice and is not specified by any document.

---

## 4. SAA5246A integrated slicer (brief)
- "Digital data slicer and display clock phase-locked loop reduce peripheral components to a minimum" [SAA5246A Features p.2].
- "Data capture performance similar to SAA5231 (VIP2)" [Features p.2].
- "The Video Input Processor section of the device uses mixed analog and digital designs in the data slicer and clock phase-locked-loop functions. As a result … no critical or adjustable components are required." [Description p.2]
- It decodes "625 line base World System Teletext transmissions" [p.2].
- Single 27 MHz crystal (3rd overtone) [Quick reference p.2; Clock systems p.26].
- CVBS input: "positive-going 1 V (peak-to-peak) … via a 100 nF capacitor". BLACK pin "video black level storage … 100 nF" [pin description p.4].
- Characteristics [p.9]: video 0.7 / 1.0 / 1.4 V p-p; sync 0.1 / 0.3 / 0.6 V; display PLL catching range ±7 % minimum.
- "Separate text and video signal quality detectors, 625/525 video status … readable via I2C-bus" [Features p.2]. R11B status bits:
  - "VCS SIGNAL QUALITY … video signal quality is good and PLL is phase locked";
  - "DATA QUALITY: If good Teletext data is detected then = 1";
  - "625/525 SYNC" [p.24].
- Register R1 [p.23]: ACQ ON/OFF; "7 +P/8-BIT: 7 bits with parity checking or 8-bit mode"; "VCR MODE: Selects short time constant of PLL"; "HAM.CHECK: … enables Hamming 8/4 checking of extension packet 27/0, 27/1 and 8/30/x".
- Page header status "HAM.ER Hamming error in corresponding byte" [p.20].
- No framing-code tolerance or acquisition-window figure is given. **UNVERIFIED.**
- GEN §2 correctly says SAA5246A has a "digital data slicer + display-clock PLL". GEN's claim that "the data clock is regenerated from the clock run-in each line" is consistent with SDA5231 §b but is not stated verbatim in the SAA5246A handbook.
- **DERIVED:** the 27 MHz crystal gives 27 / 6.9375 = 3.89 clocks per bit. Whether the SAA5246A slicer actually samples at 27 MHz is **UNVERIFIED**.

---

## 5. Packet-validity guidance a software slicer should copy

- **Framing code tolerance (normative-adjacent):** "some existing decoders will **accept a single bit error in the framing code** and have a **framing code acceptance window which is considerably wider than the transmission limits** defined in the present document" [EN706 Annex P (informative), p.153].
  - The annex is written to warn broadcasters of non-teletext signals. It still confirms that practical decoders accept a framing code with Hamming distance ≤1 from 0x27, searched over a window wider than 12 +0.4/−1.0 µs. Doing the same raises the risk of falsely accepting non-teletext waveforms, so it should be paired with MRAG Hamming checks.
  - ⚠ GEN §2 says "the framing byte must be bit-exact". That is true for a *generator*, but for a receiver it conflicts with Annex P practice.
- **Framing code identifies the line; run-in does not** [EN706 §6.1 NOTE, §6.2]. The first two run-in '1's may be missing or reduced [EN706 §6.1 NOTE]. Run-in detection and level/phase estimation should therefore not depend on the first 2–3 bits.
- **MRAG Hamming 8/4** [EN706 §7.1.2 p.17; §8.2 p.20–21]:
  - bits 1, 3, 5 and 7 are protection bits and bits 2, 4, 6 and 8 carry data;
  - "Single bit errors can be identified and corrected. Double bit errors can be detected";
  - decoding table: tests A, B and C all correct with D correct → no error, accept; A, B and C all correct with D not correct → error in P4, accept; A, B and C not all correct with D correct → **double error, reject**; A, B and C not all correct with D not correct → single error, correct it if it is in a data bit [EN706 §8.2 table p.21].
  - A software receiver should discard the packet if either MRAG byte (bytes 4 and 5) is uncorrectable, since magazine and row would be unknown. This is a design inference, consistent with SAA5040 "Hamming and parity checks … to reduce errors" [SAA5030/5040 PDF p.157].
- Odd parity for 7-bit data bytes: "if D1 ⊕ … ⊕ D7 ⊕ P = 1, accept data bits" [EN706 §8.1 p.20].
- Hamming 24/18 for triplets (X/26, X/27/4+, X/28, M/29) [EN706 §8.3 p.21].
- BT653 System B error protection: "Odd parity for bytes 4 to 45 / Even parity for bytes 1 to 3"; "8/4 Hamming code for bytes 4 and 5; 8/4 and 24/18 for extension packets numbers 26, 27, 28 and 29"; CRC in bytes 44–45 of designated blocks (packet 27) [BT653 Table 1a 2.4.1–2.4.3].
- Signal-quality gating (hardware practice): SAA5246A exposes "DATA QUALITY" and "VCS SIGNAL QUALITY" [p.24]. SDA5231 stops data acquisition when CVBS is noisy [p.6]. A software equivalent (for example a minimum run-in amplitude or correlation score) is a design choice; no numeric threshold is given in any document.

---

## 6. Raw capture parameters and samples per bit (DERIVED arithmetic)

Formula: **samples/bit = f_s / 6 937 500**. Samples per 64 µs line = f_s / 15 625, and samples/bit = samples-per-line / 444 (GEN §4 uses the same relation).

| Sample clock f_s | Working | samples/bit | samples per 64 µs line |
|---|---|---|---|
| 27.000 MHz (e.g. ITU-R BT.601 2×13.5, SAA5246A crystal) | 27 000 000 / 6 937 500 | **3.8919** | 1728.0 |
| 28.636 36 MHz (8 × fsc NTSC = 8 × 3 579 545.45) | 28 636 363.6 / 6 937 500 | **4.1278** | 1832.73 |
| 35.468 950 MHz (8 × fsc PAL = 8 × 4 433 618.75) | 35 468 950 / 6 937 500 | **5.1126** | 2270.01 |
| 13.5 MHz (BT.601 luma) | 13 500 000 / 6 937 500 | 1.9459 | 864 |
| 17.734 475 MHz (4 × fsc PAL; TPU2735 "17.7 MHz" PAL clock) | 17 734 475 / 6 937 500 | 2.5563 | 1135.0 |
| 14.318 18 MHz (4 × fsc NTSC) | 14 318 180 / 6 937 500 | 2.0639 | 916.36 |

- For 35.468 950 MHz with a 2048-sample VBI capture window: 2048 / 35.46895 MHz = **57.74 µs**, i.e. 2048 / 5.1126 ≈ 400.6 bit periods. That covers the 360-bit packet (51.89 µs) plus about 5.8 µs of margin.
- The "bt8x8 at 35.468950 MHz, 2048 samples per line" capture format itself (Brooktree Bt848/878 VBI, Linux V4L2 / zvbi convention) is **UNVERIFIED**: there is no bt8x8 or V4L2 document in the local sources.
- The TPU2735 uses a 6-bit digital video input (V1 LSB … V6 MSB) [TPU2735 pins, p.6–7] at main clock 17.7 MHz for PAL, 14.4 MHz for NTSC and 20.25 MHz for D2MAC [TPU2735 recommended operating conditions p.12; pin 36 p.9]. This is a historical proof that a digital slicer works at about **2.56 samples/bit with 6-bit samples**, plus ghost cancellation.
- Clock-accuracy effect: ±25 ppm over 360 bits is only 0.035 to 0.046 samples of drift across the table rates (**DERIVED**), so a fixed nominal rate with per-line phase recovery from the run-in is enough. Capture-clock error in real hardware is **UNVERIFIED**.
- GEN §5 edge-jitter table (±0.5 sample / samples-per-bit) applies to generation. For reception, the analogous point is that fewer than about 4 samples/bit calls for interpolation (for example linear or sinc resampling) to find bit centres. This is a design inference.

---

## 7. 525-line / NTSC variants

- **EN 300 706 defines no 525-line variant.** Its scope is 625-line, 50-field systems only [EN706 §1 p.10]. The only line frequency it uses is "nominal fH" with 444 × fH = 6.9375 Mbit/s [§5.3].
- **BT.653 does define System B for 525/60** [BT653 Table 1b, printed p.14 (PDF p.16)]:

| Item | 525/60 System B | 625/50 System B (for comparison) |
|---|---|---|
| 1.2 data positioning | "Bit 13 is reference plus 11.7 μs (± 0.175)" | 12.0 µs (−1.0, +0.4) |
| 1.3 amplitude | "0" black level ± 2 %; "1" **70% (± 6%)** of black-to-white | 66 % |
| 1.4 bit rate | **5.727272 Mbit/s ± 25 × 10⁻⁶** | 6.9375 Mbit/s |
| 1.5 shaping | skew symmetrical about 0.5 bit rate | same |
| 1.6 coding | Binary NRZ | same |
| 1.7 data line incl. run-in | **296 bits** | 360 bits |
| 2.1 framing | Byte 3 = 11100100 (same as 625) | same |
| 2.2 data unit | **35 bytes** | 43 bytes |
| Extension packets | "Packet 27, bytes 7 to 36" etc. | shorter data blocks than 625 |

  - **DERIVED:** 5.727272 MHz ≈ 364 × 15 734.264 Hz (= 8/5 × fsc NTSC). BT653 states "364 × line frequency" explicitly only for System C (and 364 × fH, 8/5 × fsc for System D), not in the System B column.
  - 296 bits = 16 run-in + 8 framing + 34 × 8. With 2 address bytes, that leaves **32 bytes for text** per row versus 40 in 625-line System B.
  - Table 1b does not state which 525-line VBI lines are used. The TPU2735 acquires on NTSC lines 10–21 [TPU2735 §2 p.5]. That is a product fact, not a standard.
- BT653 Table 2 lists System B users including UK, Germany, Australia, NZ, China, the Nordics, Italy, Spain, the Netherlands, South Africa, Singapore, Malaysia, Turkey and Ukraine. The USA and Canada are listed as System C (NABTS), and Japan as System D [BT653 Table 2 p.26]. No country is listed as using 525-line System B.
- Recommendation for requirements: **scope 525-line out** (or make it an optional extension). It needs a different bit rate (5.7273 Mbit/s), timing reference (11.7 µs), "1" level (70 %) and row length (32 text bytes), and EN 300 706 (the presentation-layer spec the receiver implements) does not cover it. The SAA5246A only reports 625/525 status and offers "VCS TO SCS … display of messages with 60 Hz input signal" [p.2, p.23]. It decodes "625 line base" teletext.

---

## Gaps / UNVERIFIED summary
1. No eye-height, roll-off-factor, S/N, echo, BER or jitter-tolerance **requirements** for System B in EN706 or BT653. TR 101 233 (referenced) is not available locally.
2. SAA5231 and SDA5231 give no noise or BER performance, no framing-code logic (done downstream) and no acquisition-window figures. The exact slice-threshold formula for SAA5231 is not stated. SDA5231 says "middle of … the data signal", and SAA5030 says "mid-point of two peak detectors".
3. The SAA5243 ECCT PDF (the downstream framing detector) has no text layer and was not read. The framing-code error tolerance of specific chips is unverified. The only evidence is EN706 Annex P ("some existing decoders will accept a single bit error").
4. The bt8x8 capture format (35.468950 MHz, 2048 samples) is not documented locally.
5. BT653 Table 1b does not list VBI line numbers for 525-line System B.
