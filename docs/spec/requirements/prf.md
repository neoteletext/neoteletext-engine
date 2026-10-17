# F.14 Compatibility profiles (PRF)

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
