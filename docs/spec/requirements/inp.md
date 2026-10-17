# F.12 Input formats (INP)

**REQ-INP-001 — Reader interface.**
Every input format shall be read by a streaming reader. The reader takes bytes as they arrive, in pieces of any size, and produces packets with metadata (REQ-ARCH-002 and REQ-ARCH-003) or pages. Its memory use shall be bounded and independent of file size. Readers shall work on regular files, pipes and standard input.
*Verification:* Test that feeds each format one byte at a time and in large blocks, and compares the results.

**REQ-INP-002 — T42 packet streams.**
The reader shall accept the "T42" format:
- a headerless sequence of 42-byte packets;
- each packet consists of bytes 4 to 45 of the broadcast packet, in transmission bit order, with parity and Hamming coding intact.

This is the format produced and consumed by vhs-teletext, vbit2 and raspi-teletext.
*Verification:* Test with files from those tools.

**REQ-INP-003 — Packet streams with run-in and framing code.**
The reader shall accept two further variants:
- 43-byte packets that begin with the framing code 0x27;
- 45-byte packets that begin with the clock run-in 0x55 0x55 and framing code 0x27. This is the "raw" output of vbit2.

The leading bytes shall be checked and removed.
*Verification:* Test.

**REQ-INP-004 — Bit-reversed packet streams.**
The reader shall accept 42-byte packet streams in which every byte is bit-reversed. This is the output of vbit2's "reverse" option, and the byte order of Digital Video Broadcasting.
*Verification:* Test.

**REQ-INP-005 — Format detection.**
When the format is not specified, it shall be detected in this order:
1. **Text or byte signatures.** Page files begin with text commands such as `PN,` or `DE,`. Transport streams have the byte 0x47 every 188, 192 or 204 bytes. Elementary streams begin with 00 00 01 BD. EP1 files begin with FE 01.
2. **Headerless binary streams.** For each packet length and bit order in REQ-INP-002 to REQ-INP-004, decode the first several hundred packets and choose the variant in which the packet address bytes decode without uncorrectable errors most often.

The detected format shall be reported. An explicit format option shall override detection.
*Verification:* Test with every supported format, including files with corrupted starts.

**REQ-INP-006 — MPEG transport streams.**
The reader shall accept MPEG-2 transport streams with:
- 188-byte packets;
- 192-byte packets, which have a 4-byte prefix before each 188-byte packet ("M2TS");
- 204-byte packets, which have 16 bytes of error-correction data after each packet.

It shall find the sync byte 0x47 and regain synchronisation after corruption. It shall extract the packet identifier, the payload-unit-start flag, the adaptation-field control and the continuity counter.
- It shall accept packets that carry both an adaptation field and payload, even though EN 300 472 clause 4.1 permits only one or the other.
- It shall discard packets with the transport-error flag set.
- It shall report continuity-counter breaks.

The 192-byte variant is not only theoretical: off-air recordings in the public test corpus (REQ-TST-022) use it.
*Verification:* Test with generated streams and with the off-air recordings of the real-broadcast corpus (REQ-TST-022).

**REQ-INP-007 — Finding teletext in a transport stream.**
The reader shall decode the Program Association Table (packet identifier 0) and each Program Map Table. It shall find the elementary streams whose stream type is 0x06 and which carry one of these:
- a teletext descriptor (tag 0x56);
- a Vertical Blanking Interval teletext descriptor (tag 0x46);
- a Vertical Blanking Interval data descriptor (tag 0x45) listing teletext.

For each teletext descriptor entry it shall report:
- the three-letter language code;
- the teletext type (initial page, subtitle page, additional information page, programme schedule page, or subtitle page for hearing-impaired people);
- the magazine (0 meaning 8);
- the page number (two hexadecimal digits).

A single descriptor may list several entries with different languages and types for the same stream, as the off-air recordings of REQ-TST-022 do. Every entry shall be reported. When an entry identifies a subtitle page, the reader shall offer that page as the default page for subtitle display.

The user shall be able to select a stream by packet identifier or by programme. Without a selection, the first teletext stream shall be used and the choice reported.
*Verification:* Test, including recordings with several descriptor entries.

**REQ-INP-008 — Teletext in elementary stream packets.**
The reader shall decode packetised elementary stream packets with stream identifier 0xBD, skip the header using its length field, and read:
- the data identifier, accepting values 0x10 to 0x1F and 0x99 to 0x9B, and rejecting others with a diagnostic;
- the data units that follow.

For data units with identifier 0x02 (teletext), 0x03 (teletext subtitles) or 0xC0 (inverted teletext), it shall:
- read the field parity and line offset into the packet metadata, where line offset 0 means unknown;
- check the framing code: 0xE4 in this byte order for teletext, and 0x1B for inverted teletext;
- bit-reverse each of the following 42 bytes to obtain the packet in the internal form.

All other data units, including stuffing (0xFF), shall be skipped using their length field, whatever their length. The presentation time stamp, where present, shall be converted to the packet time stamp.
*Reason:* EN 300 472 clause 4.4 inserts bits "in the same order as they would appear in the VBI", which makes every byte the bit-reverse of the internal form.
*Verification:* Test with streams generated by vbit2 and with captured broadcast streams.

**REQ-INP-009 — Raw elementary stream files.**
The reader shall accept a file that is a plain sequence of the packetised elementary stream packets of REQ-INP-008, without a transport stream around them.
*Verification:* Test.

**REQ-INP-010 — Linux sliced data.**
The reader shall accept the Linux sliced-data record format: 64-byte records made up of four 32-bit fields (service identifier, field, line and reserved) followed by 48 bytes of data. Records with service identifier 0x0001 (teletext system B) shall give a packet from the first 42 data bytes, with field and line placed in the metadata. Records with identifier 0 shall be ignored. The 32-bit fields shall be read in the byte order of the computer that wrote the data, which is configurable and defaults to little-endian.
*Verification:* Test.

**REQ-INP-011 — Raw sampled video files.**
The reader shall accept headerless files of raw sampled lines from the blanking interval and pass each line to the software data slicer (area SLC). The file geometry shall be configurable:
- sample rate;
- samples per line;
- sample size (8-bit, or 16-bit little-endian);
- lines per field;
- which of those lines to use;
- the line number of the first line of each field.

Presets shall be provided matching the vhs-teletext tool:

| Preset | Sample rate | Samples per line | Sample size | Lines per field | Lines used |
|---|---|---|---|---|---|
| bt8x8 (default) | 35,468,950 Hz | 2048 | 8-bit | 16 | All 16 (7 to 22 and 320 to 335) |
| cx88 | 35,468,950 Hz | 2048 | 8-bit | 18 | 1 to 16 |
| saa7131 | 27,000,000 Hz | 1440 | 8-bit | 16 | |
| tbc | 17,730,000 Hz | 1135 | 16-bit | 313 | 6 to 21 |
| tbc-vbi | 17,730,000 Hz | 1135 | 16-bit | 16 | |

The last four bytes of each bt8x8 frame may hold a frame sequence number written by the Linux driver, and shall not be allowed to cause errors.
*Verification:* Test with the vhs-teletext sample captures.

**REQ-INP-012 — Withdrawn (live capture on Linux).**
*Withdrawn on 1 October 2026.* Capture devices are hardware supplied by the port through the data source of REQ-ARCH-009. The file formats such devices produce remain supported (REQ-INP-010 and REQ-INP-011). The number is kept and will not be reused.

**REQ-INP-013 — TTI page files.**
The reader shall accept TTI page files (MRG Systems format, with the extensions used by vbit2):
- Lines may end with carriage return and line feed, or line feed alone. Every carriage return directly before the line feed is part of the line ending, so no value read ends in one; a value that did could not be written back unchanged.
- Commands are recognised by their first two letters.

The commands shall be handled as follows:

| Command | Handling |
|---|---|
| `PN,mppss` | Page number and sub-page. A new `PN` line starts a new sub-page of a carousel. |
| `SC,hhhh` | Sub-code in hexadecimal. |
| `PS,hhhh` | Page status. The bits map to control bits as decision E.21 describes: 0x4000 is C4, 0x0001 C5, 0x0002 C6, 0x0004 C7, 0x0008 C8, 0x0010 C9, 0x0020 C10 and 0x0040 C11. Bits 7, 8 and 9 are C12, C13 and C14. |
| `OL,n,text` | Output line for rows 0 to 25. |
| `OL,26`, `OL,27`, `OL,28`, `OL,29` | Enhancement, link and magazine packets. The first character's low four bits are the designation code. Each following group of three characters carries 18 bits, six bits per character with the least significant character first. For link packets with designation codes 0 to 3, the characters carry four-bit values. |
| `FL,` | Six three-digit hexadecimal Fastext links, where 0 and 8FF mean no link. |
| `LN,key,target,label` | A link of the TTI Link Extension (Appendix E), which takes precedence over `FL`. |
| `RE,h` | Region, which sets the character set group. |
| `PF,f,c` | Page function and page coding. |
| `DE,text` and `DS,text` | Description, kept as metadata. |
| `CT` | Cycle time, ignored. |
| Unknown commands | Ignored with a diagnostic. |

In `OL` text, a control code may appear in any of these forms:
- as the byte plus 0x80;
- as the escape byte 0x1B followed by the code plus 0x40;
- in the case of double height, as 0x10, which shall be read as 0x0D, as the MRG format specifies.

The page shall be converted to packets (REQ-ARCH-004).
*Verification:* Test with files from vbit2, QTeletextMaker and edit.tf.

**REQ-INP-014 — TTI writer.**
A writer shall produce TTI files from stored pages. It shall write control codes in the escape form, so that the files are plain text, and write enhancement packets in the form described in REQ-INP-013. Reading a written file shall reproduce the same page.
*Verification:* Round-trip test (REQ-TST-020).

**REQ-INP-015 — EP1 page files.**
The reader shall accept EP1 page files:
- a 6-byte header (FE 01, language code, enhancement flag, two-byte little-endian offset);
- when the flag is 0xCA, an optional enhancement block of up to sixteen 40-byte packets, each holding a designation code and thirteen triplets stored as unprotected address, mode and data bytes, with the terminator stored as address 0x7F;
- 24 rows of 40 bytes;
- a further 40-byte row;
- a 2-byte terminator.

A file without enhancements is 1008 bytes long.
*Verification:* Test with files exported by the zxnet and edit.tf editors.

**REQ-INP-016 — edit.tf and zxnet page addresses.**
*Phase:* Phase 1 for the reader; Phase 2 for the writer.
The reader shall accept page data in the web address form used by the edit.tf and zxnet editors:
- a hexadecimal metadata digit, where the low three bits select a character set;
- a colon;
- a base64url string (alphabet A–Z, a–z, 0–9, "-" and "_", no padding).

The string carries 25 rows by 40 columns of 7-bit codes. The code for row r and column c occupies bits 280r + 7c to 280r + 7c + 6, most significant bit first, giving 1167 characters in all. The older 1120-character (24-row) form shall also be accepted. Optional `key=value` fields shall be read where defined:
- `PN` page number;
- `SC` sub-code;
- `PS` page status;
- `X270` Fastext links;
- `X26` enhancement triplets.

Unknown fields shall be ignored. A matching writer shall be provided.
*Verification:* Test with addresses produced by both editors.

**REQ-INP-017 — Raw screen dumps.**
*Phase:* Phase 1 for reading dumps; Phase 2 for detecting repeated double-height rows.
The reader shall accept raw page dumps of 1000 bytes (25 rows by 40 columns) or 960 bytes (24 rows), masking each byte to 7 bits. This includes the BBC Microcomputer form, in which every byte has its top bit set. Because that computer needs the row below a double-height row to repeat it, an option shall detect such repeated rows and treat them as the ignored lower row.
*Verification:* Test.

**REQ-INP-018 — T42 writer.**
A writer shall produce T42 packet streams. Sources are received packets (pass-through, optionally filtered by magazine and page) and packets encoded from pages. When encoding pages, it shall follow the packet order of Annex B.2:
1. header;
2. packets 27;
3. packets 28;
4. packets 26 in ascending designation code;
5. rows.
*Verification:* Round-trip test.

**REQ-INP-019 — The page encoder.**
The page encoder shall produce correctly coded packets for:
- headers, with all control bits;
- rows with odd parity;
- packets X/26 with Hamming 24/18;
- packets X/27/0 with correct relative magazine bits and a checksum computed by REQ-ECC-007;
- for pages with TTI Link Extension links, packet X/27/0 and a row 24 (packet X/24) generated as Appendix E.2 describes;
- packets X/28/0 format 1.

The packets of Levels 2.5 and 3.5 are encoded as REQ-INP-024 describes.
*Verification:* Test that decodes encoded packets back to the original page.

**REQ-INP-020 — Reporting input errors.**
Every reader shall report malformed input with the byte offset at which it was found. It shall continue with the next recognisable unit where one can be found.
*Verification:* Test with corrupted files.

**REQ-INP-021 — Other formats.**
*Phase:* Phase 2 (whole requirement).
The following should be implemented:
- the zvbi "old sliced" capture format;
- the TTA format (a 2048-entry index followed by T42 data).

The following may be implemented later:
- the video-recorder embedded sliced format of the Linux ivtv driver;
- the SMPTE RDD 8 ("OP-47") and SMPTE ST 2031 serial digital interface formats;
- the FAB and Cebra page formats.

They are listed in Part G until they are scheduled.
*Verification:* Test, when implemented.

**REQ-INP-022 — The live WebSocket feed used by the zxnet teletext viewer.**
An optional network module shall read the live teletext feeds served to the zxnet online teletext viewer (decision E.28).

**Connecting.** Each feed ("channel") is identified by a channel identifier: base64 text encoding a JSON array of three strings, the service name, the host name and the path. The reader shall connect to the secure WebSocket address formed from the host and path. For example, the Teefax channel identifier decodes to `["teefax","zxnet.co.uk","/ws/teefax"]`, giving the address `wss://zxnet.co.uk/ws/teefax`.

The feeds known on 1 October 2026 shall be available by short name:

| Short name | Service | Address |
|---|---|---|
| `teefax` | Teefax | `wss://zxnet.co.uk/ws/teefax` |
| `nms-ceefax` | Ceefax recreation | `wss://internal.nathanmediaservices.co.uk/websockets/ceefax` |
| `chunkytext` | ChunkyText | `wss://zxnet.co.uk/ws/chunkytext` |
| `spark` | SPARK | `wss://zxnet.co.uk/ws/spark` |
| `ch4` | Channel 4 capture: a real-time replay of a broadcast of 15 July 2007 | `wss://zxnet.co.uk/ws/ch4` |
| `test` | Test service | `wss://zxnet.co.uk/ws/test` |

A channel identifier or an address may also be given directly. The short name is only a label for the user. The service name sent to the server is the first string of the channel identifier, and it can differ from the short name: the `ch4` identifier carries the service name `channel4-capture`. Sending a different name draws a `wrongname` message.

**Messages sent by the client.** Client messages are text frames holding comma-separated values, which is how the viewer's JavaScript sends its arrays. After connecting, the reader shall send:
- `service` followed by the channel identifier;
- `neottx` followed by `true`, which turns teletext on;
- one `pagesearch` message for each page wanted. A `pagesearch` message carries, in order:
  - a slot number;
  - the magazine;
  - the page number;
  - the sub-code, where 16255 (hexadecimal 3F7F) means any sub-page;
  - three true or false flags.

  The viewer uses slot 0 for the displayed page and further slots, up to at least slot 10, for linked pages.
- `keepalive` every 5 seconds.

When the server sends `apiver`, the reader shall reply with `apiver` and the protocol version it understands, which is 1 at the time of writing.

**Messages received.** Server messages are JSON arrays whose first element names the message:

| Message | Contents | Handling |
|---|---|---|
| `header` | slot, the 40 bytes of header packet bytes 6 to 45 in base64 (raw, including the Hamming-coded page address and control bytes), a page-matched flag, the control bits as a number, the magazine, the sub-code and the page number | See the conversion rules below. |
| `row` | slot, magazine, packet number, the 40 raw bytes of packet bytes 6 to 45 in base64 | Parity and Hamming coding are intact, as verified by connecting on 1 October 2026. |
| `clock` | Header clock characters | |
| `pageExists` | A magazine and page number present in the service | Used to build the service's page list. |
| `initialpage` | The initial page and the status message | |
| `bsdp` | The 40 bytes of packet 8/30, bytes 6 to 45, in base64, sent about once a second (server protocol version 1 and later) | Rebuilt as a packet 8/30 in magazine 8 and decoded as service data (area SVC). Its date and time show whether a feed is live or a replay. |
| `channelSettings`, `secondTick`, `wrongname` | Viewer settings, a timing tick, and a service-name mismatch | `wrongname` means stop reconnecting. |

In the control bit number, C4 is 0x0010, C5 is 0x0020 and so on, up to C11 at 0x0800. The national option bits occupy 0x7000.

**Conversion to packets.** The reader shall rebuild each `header` and `row` message as a complete 42-byte packet in the internal form (REQ-ARCH-002) by Hamming-encoding the packet address from the magazine and packet number, and prepending it to the 40 received bytes. The packets then pass through normal acquisition (REQ-ARCH-004). The decoded fields that the server also sends in `header` messages shall be used only to cross-check the decoder, and any disagreement shall be reported.

**Filtering.** The server sends rows only for the pages requested in `pagesearch` slots, so the feed is a filtered stream, not a whole-service stream. To capture a whole service, the reader shall offer a mode that walks the `pageExists` list, requesting pages in turn through the available slots.

When tested on 10 October 2026, only slot 0 returned rows. A page then needed to be held for longer than one page cycle: requests that changed page every 3 seconds received headers but no rows. The walking mode shall therefore hold each page until its rows have arrived, or until a timeout. It shall keep to the pace of a person using the viewer, because these are hobby servers.

**Tolerance and reconnection.** Unknown message types and extra fields shall be ignored. After a dropped connection the reader shall reconnect after 10 seconds, giving up after 5 failed attempts. It shall treat 5 seconds without a `clock` or `header` message as a dropped connection, as the viewer does.

**Build.** This reader is part of the `neoteletext-web` crate. It uses that crate's sans-IO WebSocket code and the transport supplied by the port (REQ-WEB-001), including the port's encryption. The core library remains free of networking (REQ-GEN-002).
*Reason:* This is the only live source of teletext with raw packet bytes found on the internet. It gives continuous real-time test data (REQ-TST-063).
*Verification:* Test against a recorded session replayed from a file; demonstration against the live servers.

**REQ-INP-023 — Enhancement data and enhancement pages in TTI files.**
The TTI reader (REQ-INP-013) and writer (REQ-INP-014) shall carry every packet of Levels 2.5 and 3.5, in the form of the vbit2 extensions (`research/notes-formats.md` section 5.2):
- `OL,26`: up to sixteen lines per page, each with its own designation code;
- `OL,27`: designation codes 4 and 5 hold thirteen triplets, written as for X/26;
- `OL,28`: designation codes 0, 1, 3 and 4 hold thirteen triplets. A line with designation code 0, 3 or 4 also sets the page function and page coding, overriding an earlier `PF` line;
- `OL,29` on a page numbered mFF: the magazine packets M/29/0, M/29/1 and M/29/4 of magazine m, which are encoded as magazine packets and not as part of the page;
- `PF,f,c`: the page function and page coding (Table 3).

The rows of enhancement pages shall be read and written according to their page coding:
- coding 0, used by DRCS pages: 40 characters with parity added on encoding, as for display rows;
- coding 2, used by object pages: the first character's low four bits give byte 6, and the following 39 characters hold thirteen triplets, as for X/26;
- coding 3, used by the Magazine Organization Table: 40 four-bit values, each in the low four bits of a character from 4/0 to 4/F.

Reading a written file shall reproduce the same packets.
*Verification:* Round-trip test (REQ-TST-020) with object pages, DRCS pages, a Magazine Organization Table and magazine packets; test with files written by vbit2 where available.

**REQ-INP-024 — Encoding the packets of Levels 2.5 and 3.5.**
The page encoder (REQ-INP-019) shall also produce correctly coded packets for:
- packets X/26 with every designation code from 0 to 15;
- packets X/27/4 and X/27/5;
- packets X/28/0 format 1 with every field, X/28/1, X/28/3 and X/28/4;
- the magazine packets M/29/0, M/29/1 and M/29/4;
- object pages, with byte 6 of each packet Hamming 8/4 coded and thirteen Hamming 24/18 triplets;
- the Magazine Organization Table, with every byte Hamming 8/4 coded;
- DRCS pages, with each D-byte given odd parity.

When it writes a stream, it shall follow the packet order of Annex B.2, and the order of Annex B.7 for a set of pages: the Magazine Organization Table, then the DRCS pages, then the object pages, then the Level 1 pages.
*Verification:* Test that decodes encoded packets back to the original fields, and that zvbi accepts the encoded streams (REQ-TST-023).

**REQ-INP-025 — Authoring test pages for Levels 2.5 and 3.5.**
The formats crate shall provide, behind its `std` feature, a builder for authoring Level 2.5 and 3.5 test pages and streams by function rather than by bit pattern. It shall be able to:
- write every row and column triplet of Tables 28 and 29 by name, with its fields;
- build local objects, and POP and GPOP pages, with their pointer tables computed from the objects they define;
- build a Magazine Organization Table, with each byte pair placed at the position of its page number, and with object links, DRCS links and default objects;
- build DRCS characters of every mode from pixel patterns, with the mode identification of X/28/3 computed;
- set every field of X/28/0 format 1, X/28/1, X/28/4 and the M/29 packets;
- write the result as TTI files and as T42 streams, through REQ-INP-023 and REQ-INP-024.

The builder is the source of the authored test streams of REQ-TST-024. Because the builder and the decoder are written from the same reading of the standard, the builder's output shall also be checked against zvbi (REQ-TST-023) and against the worked encodings of the research notes.
*Verification:* Test that the builder reproduces the worked encodings of `research/notes-level25-objects.md` section 13.2 and `research/notes-level25-drcs.md` sections 3.5, 4, 5 and 11.
