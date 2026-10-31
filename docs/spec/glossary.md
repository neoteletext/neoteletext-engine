# Appendix A — Glossary

| Term | Meaning |
|---|---|
| Active position | The row and column at which a Level 1.5 enhancement triplet acts. |
| Blast-through | Display of alphanumeric characters from columns 4 and 5 while in mosaic mode. |
| Chapter | A page memory in the SAA5243 and SAA5246A. |
| Clock run-in | The alternating 16-bit pattern at the start of each teletext line used to synchronise the bit clock. |
| Ducking | Lowering other sounds while one plays, as Audetel's fade byte lowered the programme sound (EN 300 708 Table 19). |
| Designation code | Byte 6 of packets 26 to 31, which distinguishes up to 16 packets of the same number. |
| IDL, independent data line | A teletext packet that carries data independently of pages, addressed by a data channel (EN 300 708 clause 6). The audio profile uses data channel 12, packet 4/31. |
| IMA ADPCM | The Interactive Multimedia Association's adaptive differential PCM: 4 bits a sample, in blocks that each start with their own predictor and step index. |
| DVB | Digital Video Broadcasting, the family of digital television standards that carries teletext in EN 300 472. |
| Fastext, FLOF | The editorial linking scheme using packet X/27/0 and coloured keys ("Full Level One Features", EN 300 706 Annex H). |
| Framing code | The byte 11100100 (transmission order) that identifies a teletext line and aligns bytes. |
| G0, G1, G2, G3 | The primary alphanumeric set, the mosaic set, the supplementary set and the smoothed-mosaic and line-drawing set. |
| Hamming 8/4, Hamming 24/18 | The error-correcting codes of EN 300 706 clauses 8.2 and 8.3. |
| Held mosaic | The mosaic character shown in place of attribute cells while Hold Mosaics is active. |
| Magazine | A group of up to 256 page numbers sharing the first digit of the page number, 1 to 8. |
| Magazine and Row Address Group | The two Hamming-protected bytes giving a packet's magazine and packet number. |
| Mode | The 5-bit field of a triplet that selects its function. |
| National option | One of the thirteen variants of the Latin G0 set's thirteen variable positions. |
| Packet | One teletext line's data: 45 bytes on air, or 42 after the framing code. |
| Page header | Packet X/0: the page address, control bits and row 0 text. |
| Parallel mode, serial mode | The two ways of interleaving magazines, selected by bit C11. |
| PES | Packetised Elementary Stream, the container for teletext in digital television. |
| Sub-code, sub-page | The 13-bit number that distinguishes versions of a page. |
| T42 | A file of 42-byte teletext packets in transmission bit order. |
| Triplet | Three bytes protected by Hamming 24/18, carrying 18 data bits. |
| TTI | The MRG Systems text format for teletext pages. |
| VBI | The vertical blanking interval: the television lines outside the visible picture that carry teletext. |
| Crate | A Rust package: the unit of compilation and of dependency. |
| `no_std` | Rust code that uses only the core language library, with no operating system, so that it runs on bare microcontrollers. |
| Sans-IO | A way of writing protocol code as state machines that are fed bytes and time, and return bytes and events, without performing any input or output themselves. |
| Target triple | The name Rust gives a compilation target, such as `thumbv6m-none-eabi`: processor, vendor and operating system. |
| Miri | The Rust interpreter that detects undefined behaviour, used here to test on 32-bit and big-endian targets. |
| HTTP, TLS | The web's request protocol, and the encryption layer beneath HTTPS and secure WebSockets. |
| WebSocket | A two-way message connection opened from an HTTP request. |
| Server-sent events | A one-way stream of text events from a web server over HTTP. |
| Cookie | A small value a web server asks the client to store and send back, used here for sessions. |
| JSON | JavaScript Object Notation, the text format of the service descriptor. |
| Origin | The scheme, host and port of a web address: the unit of web security (REQ-WEB-015). |
| User agent | The web term for a browser or other client acting for the user. neoteletext is a deliberately minimal one (REQ-WEB-014). |
| Colour Map, CLUT | The 32 colours of Levels 2.5 and 3.5, in four colour lookup tables (CLUTs) of eight entries each (Table 30). A colour is named by its index, 8 × CLUT + entry. |
| DCLUT | A DRCS colour lookup table: four or sixteen Colour Map indices used by multi-colour DRCS characters (Table 8). |
| DRCS, GDRCS | Dynamically Redefinable Characters: characters downloaded as dot patterns on DRCS pages, which this receiver does not decode (decision E.95). A Global DRCS page serves the whole magazine. |
| PTU | Pattern Transfer Unit: 20 bytes of a DRCS page, holding one bit plane of a 12 by 10 character, or a whole 6 by 5 by 4 character (clause 14.1). |
| Object | A reusable sequence of enhancement triplets, Active, Adaptive or Passive, defined locally in packets X/26 or on an object page, and invoked by a page (clause 13). |
| POP, GPOP | Public Object Page and Global Public Object Page: pages that define objects for a set of pages, or for every page of a magazine. |
| MOT | Magazine Organization Table: page mFE of each magazine, linking each page to its object and DRCS pages (clause 10.6). |
| MIP | Magazine Inventory Page: page mFD, an optional list of the page types of a magazine (clause 11.3). |
| Non-spacing attribute | An attribute set by an enhancement triplet. It occupies no character cell, unlike the spacing attributes of Level 1. |
| Origin Modifier | A triplet that offsets the position of the next object invoked. |
| Side panel | Up to 16 extra columns to the left or right of the 40-column page that Levels 2.5 and 3.5 define (clause 12.5). NeoTeletext never shows them: a page is always 40 columns (decision DEC-1636). |
| Presentation level | One of Levels 1, 1.5, 2.5 and 3.5 of EN 300 706 clause 12; a configuration setting of this receiver (REQ-PRF-006). |
| Channel | A folder on a web server that serves teletext pages, identified by its origin and folder path (REQ-WEB-018, decision E.70). What earlier documents call a site. |
| Channel folder | The address of a channel: an `http` or `https` address whose path ends in `/` (REQ-WEB-023). |
| Front door, bundle | A channel's `index.t42`, which may hold any number of pages (REQ-WEB-019). |
| Page file | The file of one page in its channel's folder, named by its number: `140.tti` or `140.t42` (REQ-WEB-019). |
| Contents file | A channel's `contents.txt`: its sections, pages and linked channels, drawn by the receiver as an index (REQ-WEB-025, REQ-BRW-009). |
| Font | Bedstead, the receiver's bundled font, or a BDF file of 12 × 10 cells loaded when the user, the channel or the page names it (area FNT). |
| Font library | A folder with an index, `fonts.txt`, listing fonts with their checksums (REQ-FNT-004). |
| BDF | The Glyph Bitmap Distribution Format, a text format for bitmap fonts; the form of every loaded font (REQ-FNT-002). |
| Alias | An entry of `channel.json` that leads one page number to another channel on every page of the channel (REQ-WEB-020). |
| Discovery | Finding the channel an ordinary web address announces, from a pointer in its answer (REQ-WEB-028, decision E.73). |
| Pointer | A link of type `alternate` to a channel or a page file, with a teletext media type, given in a `Link` header or a `<link>` element (REQ-WEB-029). |
| Tuning screen | The browser's receiver-drawn screen shown before anything from another channel, which the viewer confirms (REQ-NAV-014, decision E.65). Withdrawn with the browser on 18 October 2026. |
| Page catching | Moving a cursor between the links on a page and following one, as some Philips televisions did with page numbers (REQ-NAV-015). |
| Helper process | A process `neottx` starts in web and live mode: a fetcher, which holds only the network, or a decoder, which holds nothing (REQ-SEC-018). |
| Voice sink | What a port supplies to play the audio profile's sounds: it holds sounds and starts, stops and ramps voices on numbered channels (REQ-AUD-018). |
