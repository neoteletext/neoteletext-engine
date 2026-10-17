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
13. **A windowed browser.** The browser for teletext channels is a prototype in `prototypes/browser/`, outside this specification (decision E.76).
14. **Languages that need more than single-byte sets** (decision E.78): Chinese, Japanese, Korean, and Amharic and Tigrinya in Ge'ez script. Their thousands of characters need two-byte codes and a large glyph store. Chinese has a complete teletext standard, CCST (GB/T 14219-1993), which could be added later as a regional profile; the codes it uses are kept free.
15. **Indic scripts, Burmese, Khmer and Sinhala** (decision E.78): Hindi, Bengali, Marathi, Telugu, Tamil, Gujarati, Kannada, Malayalam, Odia, Punjabi, Burmese, Khmer and Sinhala. The 1990 World System Teletext specification coded twelve Indic scripts and Burmese with X/26 Formats B and C, which EN 300 706 dropped; no broadcast used them. Showing them well needs those withdrawn formats or a text shaper in the receiver.
