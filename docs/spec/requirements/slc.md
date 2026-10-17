# F.13 Software data slicer (SLC)

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
