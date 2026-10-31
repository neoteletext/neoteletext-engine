# Appendix B — Source documents

**Earlier teletext specifications** (in `~/devel/jellytext/teletextspecifications/`, see Part A.12a): the 1974 and 1976 BBC, IBA and BREMA specifications; "World System Teletext" (1990); EBU SPB 492 (1992); and a United Kingdom 625-line television specification. They are cited by year and PDF page.

**Standards** (in `teletext-standards/`):
- ETSI EN 300 706 V1.2.1 (2003-04), *Enhanced Teletext specification*. This is the primary source for Parts A, B and E.
- ETSI ETS 300 706 (1997), the earlier edition, used for cross-checking.
- ETSI EN 300 472 V1.3.1 and DVB BlueBook A041, *Teletext in DVB bitstreams*.
- ITU-R BT.653-3, *Teletext systems*.
- ETSI ETS 300 708, *Data transmission within Teletext*.
- ETSI EN 300 708 V1.2.1 (2003-04), *Data transmission within Teletext* (`en_300_708_2003.pdf`): independent data lines and low bit-rate audio, clauses 6.4 to 6.8, Tables 16 to 22. The source of area AUD's carriage.

**Audio** (in `teletext-audio/`, area AUD, `design/audio.md` section 1):
- ETSI ETS 300 149 (March 1992), *Videotex Audio syntax* (`standards/ets_300_149_1992.pdf`): consulted, and not followed (decision E.84).
- The Audetel papers, thesis and reports (`audetel/`): Kondoz, Evans and Suddle 1993; Black, Atkinson, Kondoz and Evans 1995; Suddle 1996, chapter 7; *Television*, December 1994; the ITC annual reports of 1991 and 1992.
- The Japanese coded teletext music papers (`japanese-teletext-music/`): Sugimori, Kimata and Araki 1982; Isobe, Sugimori and Araki 1986; Araki and Hanafusa 1987; the MUSCOT features article. Consulted; music as notes is out of scope.
- Not held, and to be obtained: the IMA's 1992 *Recommended Practices for Enhancing Digital Audio Compatibility in Multimedia Systems* (IMA ADPCM), and Microsoft's RIFF WAVE description of format tag 0x0011.

**Unicode documents:**
- Unicode document L2/19-025, "Proposal to add characters from legacy computers and teletext to the UCS" (2019), and its mapping file `TELTXTG3.TXT`. They give the Unicode characters, and the sextant geometry, of the G3 set (decision E.50 and REQ-CHR-011).

**External standards consulted during research** (not held locally):
- ETSI EN 301 775, carriage of vertical blanking interval data in DVB.
- ETSI EN 300 468, DVB service information descriptors.
- The Linux kernel documentation for sliced and raw vertical blanking interval capture.
- The MRG Systems TTI format document.
- SMPTE RDD 8.

**Datasheets:**
- Signetics SAA5050/SAA5055 datasheet (`teletext-fonts/manufacturers/philips/datasheets/saa5050/`).
- Philips 1979 databook, *ICs for Digital Systems in Radio and TV* (`teletext-fonts/manufacturers/philips/datasheets/databooks/`).
- Philips SAA5246A product specification, January 1993 (`teletext-fonts/manufacturers/philips/datasheets/saa5246a/`).
- Philips SAA5243, SAA5281, SAA5231; Siemens SDA 5231; ITT TPU 2735, for the slicer and for lineage.

**Corrections to `catalog/indexes/DATASHEET-INDEX.md`**, found during this work. The page numbers are PDF page numbers in the 1979 databook:

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
- the zxnet online teletext viewer, `https://zxnet.co.uk/teletext/viewer/`, and its WebSocket feeds (REQ-INP-022).

**Community resources used as test oracles:**
- vbit2 and raspi-teletext;
- QTeletextMaker;
- the edit.tf and zxnet editors;
- the Teletext Wiki at teletext.wiki.zxnet.co.uk.

Where these are GPL-licensed, they are used only as external programs and data sources, never copied.
