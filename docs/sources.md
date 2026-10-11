# Sources record

The standards clauses and tables each module is written from (REQ-DOC-005, and the legal basis section of REQUIREMENTS.md). Clause and table numbers are those of ETSI EN 300 706 V1.2.1 unless another document is named. Copies of the standards are kept outside the repository (`~/devel/microcontrollers/teletext/specifications/`).

| Module | Sources |
|---|---|
| `neoteletext-core::consts` | Clauses 5.3, 6.1, 6.2, 7.1 (framing and packet layout), Table 1 (page numbers), clause 9.3.1.3 (control bits), Table 26 (spacing attributes), Table 30 (CLUT 0), clause 9.8.1 (service data); EN 300 472 clause 4.4 (bit order); decisions E.12, E.14, E.19 |
| `neoteletext-core::ecc` | Clause 8.1 (odd parity), 8.2 (Hamming 8/4), 8.3 (Hamming 24/18), 9.6.1 and Figure 13 (page check word) |
| `neoteletext-core::packet` | Clauses 7.1.2 (packet address), 9.3.1 (header), 9.3.2 (rows), 9.4.1 (X/26), 9.4.2 and Tables 3 and 4 (X/28/0 format 1), 9.5 and Tables 12 and 13 (M/29), 9.6.1, Figure 12 and Table 14 (X/27/0), 9.8.1 and Table 18 (8/30) |
| `neoteletext-core::fsm`, `acq` | Clause 7.2.1, Annexes B.2 to B.4 and B.9 (page boundaries and completion); decisions E.6 to E.8, E.11, E.19, E.20 |
| `neoteletext-core::service` | Clause 9.8.1, Table 18; the date digit order verified against live packets (research/answers-teletext-sources.md section 2.1) |
| `neoteletext-core::charset` | Clause 15; Tables 32 and 33 (designation), 35 to 46 (G0 and G2 sets), 47 (G1), 48 (G3); decisions E.4, E.21, E.26 |
| `neoteletext-core::level1` | Clause 12.2 and Table 26; Annexes C.3 and G.3; decisions E.10, E.15 to E.18, E.22, E.23 |
| `neoteletext-core::level15` | Clauses 12.3.1 to 12.3.4, Tables 27 to 29; Annexes A.3, A.4 and D.1.3; decisions E.1 to E.5, E.9 |
| `neoteletext-core::render`, `glyphs` | SAA5050 datasheet (character rounding, mosaic geometry, flash timing) as recorded in decisions E.12 to E.14; glyph data from Bedstead 3.261 (temporary, decision E.25, `third_party/bedstead/PROVENANCE.md`) |
| `neoteletext-core::nav`, `viewer`, `forms` | Annex H (Fastext), clause 11.1; REQUIREMENTS.md Appendices C and E; decisions E.10, E.36 |
| `neoteletext-core::policy` | RFC 1918, RFC 6598, RFC 3927, RFC 4193, RFC 4291 (blocked address ranges); REQUIREMENTS.md area SEC |
| `neoteletext-image::png` | PNG specification (ISO/IEC 15948) sections 5, 9, 11; RFC 1950 (zlib) and RFC 1951 (deflate, fixed Huffman codes section 3.2.6) |
| `neoteletext-image::palette` | Table 30 (CLUT 0) |
| `ttx` iTerm2 output | iTerm2 inline images protocol (OSC 1337 `File=`), as documented by iTerm2; RFC 4648 (base64) |
