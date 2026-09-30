# Teletext input formats: definitions with sources

Compiled 2026-10-01. Every fact carries a source. "UNVERIFIED" marks anything I could not confirm from a primary source or source code.
Local spec text was extracted with pdftotext into this scratchpad (EN_300_472..., DVB_A041..., en300706.txt, en301775.txt, en300468.txt, ttiformat.txt, rdd08.txt).

Abbreviations for sources:
- [EN472] ETSI EN 300 472 V1.3.1 (2003-05), local file specifications/EN_300_472_v1.3.1_Teletext_in_DVB.pdf
- [A041] DVB BlueBook A041 (June 2016, EN 300 472 v1.4.1 content), local file specifications/DVB_A041_Teletext.pdf
- [EN706] ETSI EN 300 706 V1.2.1 (2003-04), local file specifications/EN_300_706_v1.2.1_Enhanced_Teletext.pdf
- [EN775] ETSI EN 301 775 V1.2.1 (2003-05), https://www.etsi.org/deliver/etsi_en/301700_301799/301775/01.02.01_60/en_301775v010201p.pdf
- [EN468] ETSI EN 300 468 V1.17.1 (2022-10), https://www.etsi.org/deliver/etsi_en/300400_300499/300468/01.17.01_60/en_300468v011701p.pdf
- [KSLICED] https://docs.kernel.org/userspace-api/media/v4l/dev-sliced-vbi.html
- [KRAW] https://docs.kernel.org/userspace-api/media/v4l/dev-raw-vbi.html
- [VDEV2] linux include/uapi/linux/videodev2.h (torvalds/linux master, fetched 2026-10-01)
- [VHS] github.com/ali1234/vhs-teletext (master)
- [VBIT2] github.com/peterkvt80/vbit2 (master) and its wiki page "Page-files"
- [RASPI] github.com/ali1234/raspi-teletext (master)
- [WIKI-T42] https://teletext.wiki.zxnet.co.uk/wiki/T42_packet_stream
- [MRG] "MRG Teletext Page Format, TTI Page file format", MRG Systems Ltd, v1.1 (04/02/05), https://zxnet.co.uk/teletext/documents/ttiformat.pdf
- [EDITTF] github.com/rawles/edit.tf (branch gh-pages): README.md and teletext-editor.js
- [ZXED] https://zxnet.co.uk/teletext/editor/editor.js (fetched 2026-10-01)

---------------------------------------------------------------------------------------------------

## 0. Bit order: the one fact every format depends on

- Teletext bytes go on air **LSB first**. [EN706] §3 definitions: "transmission bit order: bits of a Teletext data byte transmitted least significant bit first". [EN706] §7.1: "A Teletext packet comprises 360 bits organized as 45 bytes, numbered 1 to 45. In each byte, the bits are numbered 1 to 8 (LSB to MSB), and are normally transmitted LSB first."
- Clock run-in in transmission order is `1010101010101010`. [EN706] §6.1. Taken LSB-first, that is bytes **0x55 0x55**.
- Framing code in transmission order is `11100100`. [EN706] §6.2. Taken LSB-first, that is byte **0x27**. Read MSB-first it would be 0xE4.
  - Cross-check: vbit2 builds packets as `_packet[0]=0x55; _packet[1]=0x55; _packet[2]=0x27` ([VBIT2] packet.cpp lines 154-156).
  - Cross-check: SMPTE RDD 8 writes run-in as 255h 255h and framing code as 227h, i.e. 8-bit 0x55 0x55 0x27 plus parity bits b8/b9 (RDD 8-2008 §5.5.2).
- Packet address = bytes 4 and 5, both Hamming 8/4 coded. Magazine uses 3 bits and packet number (Y) 5 bits. Odd-numbered data bits carry the Hamming protection bits. [EN706] §7.1.2.
- Magazine value 0 means magazine 8. [EN706] §3 definitions ("magazine number 8").
- **Convention used below:** a byte is "LSB-first" (natural or T42 order) when bit 0 is the first bit transmitted. It is "MSB-first" (reversed, or DVB order) when bit 7 is the first bit transmitted.

---------------------------------------------------------------------------------------------------

## 1. T42 packet stream

### Definition
- [WIKI-T42]: "A t42 packet stream is simply a sequence of teletext packets minus the clock run-in and framing code."
- Each packet is 42 bytes: "two byte MRAG and 40 characters/bytes".
- The bytes "are encoded as they would be for transmission", so the stream keeps "all parity and hamming coding etc."
- There is no file header and no framing. A file may hold a whole service, a single page, "or anything in between". It has to be decoded the way a receiver decodes a live signal. [WIKI-T42]
- Each packet corresponds to EN 300 706 bytes 4..45 of the 45-byte packet ([EN706] §7.1).

### Byte bit order in T42
T42 bytes are stored LSB-first: bit 0 of each byte is the first bit transmitted. Odd parity is kept in bit 7 and Hamming bytes are stored as transmitted.

Evidence:
- vhs-teletext slicer: `Packet(np.packbits(result.reshape(-1,8)[:,::-1]))` reverses each group of 8 received bits before packing MSB-first. The first received bit therefore becomes bit 0. ([VHS] teletext/vbi/line.py, `slice()`, ~line 285)
- raspi-teletext reads 42-byte packets from stdin and serialises each byte with `*dest++ = b&1; b = b>>1;`, i.e. LSB first ([RASPI] buffer.c `copy_packet`). Its README says: "The packet format is 42 byte raw binary packets, without the clock run-in."
  - Its filler packet `{0xd0, 0xa1, 0x20 ...}` is commented "Magazine 8, packet 25". So MRAG bytes are raw Hamming 8/4 codewords and text bytes carry odd parity (0x20 has odd parity).
- vhs-teletext's DVB reader byte-reverses DVB bytes to obtain T42 bytes (`byte_reverse(b) for b in data[pos+4:pos+4+42]`, [VHS] teletext/ts.py). vbit2's DVB writer does the reverse (`ReverseByteTab[...] // bits are reversed in PES stream`, [VBIT2] service.cpp ~line 481). Both confirm that T42 order is the opposite of DVB order.
- The Linux sliced VBI payload uses the same convention: "Last 42 of the 45 byte Teletext packet, that is without clock run-in and framing code, lsb first transmitted." [KSLICED] §4.7.3.2

### Variants
- **45-byte "raw" packets (with CRI + FC):** vbit2 `--format raw` writes "full 45 byte teletext packets" (`std::cout.write(p->data(), 45)`). The first 3 bytes are 0x55 0x55 0x27. ([VBIT2] service.cpp `case Raw`; configure.cpp `--format none|t42|raw|ts|tsnpts`)
- **Bit-reversed T42:** vbit2 `--reverse` applies `ReverseByteTab` to every byte and is only allowed with `--format t42` ("--reverse requires t42 format"). ([VBIT2] configure.cpp, service.cpp `case T42`) Its output is a 42-byte MSB-first stream, the same byte order as the DVB txt_data_block.
- **TTA:** a T42 stream preceded by an index header of 2048 signed 16-bit integers, one per main page (with its subpages). Positive value = number of packets for that page; 0 = page not recorded; negative = reserved. Magazine-global packets are stored as page xFF. Packet order within a page is undefined. Used by teletext_ng. (https://teletext.wiki.zxnet.co.uk/wiki/TTA_format). The endianness of the 16-bit entries is not stated. UNVERIFIED.
- Packet-server framing in vbit2 (`--packetserver`) sends whole fields; its wire framing is not researched here (UNVERIFIED).

---------------------------------------------------------------------------------------------------

## 2. DVB teletext (EN 300 472 / EN 301 775 / EN 300 468) and MPEG-TS

### 2.1 TS / PES constraints ([EN472] §4, §4.1, §4.2; the same text with "shall" is in [A041] §4.1-4.2)
- The stream has `stream_type` **0x06** in the PMT ("PES stream carrying private data"). The ES_info loop "shall contain a Teletext descriptor as defined in EN 300 468". [EN472] §4
- A service may have more than one teletext stream if each stream has a different `data_identifier` and a distinguishing descriptor. [EN472] §4
- TS: `adaptation_field_control` — "only the values "01" and "10" are permitted". [EN472] §4.1
  - Note: this means payload-only or adaptation-only, never both in one packet. Real multiplexers do not always obey it (vbit2 sends an adaptation-only PCR packet with value 10; [VBIT2] service.cpp). A robust receiver should accept 11 too; that last point is a recommendation, not from the spec.
- PES header fields:
  - `stream_id` = `1011 1101` = **0xBD**, private_stream_1
  - `PES_packet_length` = **(N × 184) − 6**, so the PES packet ends exactly at the end of a TS packet
  - `data_alignment_indicator` = 1
  - `PES_header_data_length` = **0x24**
  - stuffing bytes pad the header "so that the entire PES header is **45 bytes** long"

  [EN472] §4.2
- "PTS and other optional fields may be present in the PES header, but the header length is always fixed". [EN472] §4.2. Annex A (informative) adds that encoders should insert PTS and decoders "shall also be able to perform the decoding process before the maximum retention time of 40 ms is elapsed".
- EN 301 775 §4.3 is stricter: "PTS shall be present". [EN775]
- Decoder model: TBttx = 480 bytes, Bttx = 1504 bytes, transfer rate 6.75 Mbit/s, data stays in Bttx at most 40 ms. [EN472] §5
- Worked PES header from vbit2 ([VBIT2] service.cpp):

  ```
  00 00 01 BD <len_hi> <len_lo> 85 <80 if PTS else 00> 24 [5 PTS bytes] FF... (to 45 bytes) 10
  ```

  Here 0x85 = '10' marker + data_alignment + original, and the last byte is data_identifier 0x10. After that come data units of 46 bytes each (2 + 44), padded with 0xFF-filled units.

### 2.2 PES_data_field syntax ([EN472] §4.3 Table 1, Table 2)

```
PES_data_field() {
  data_identifier        8  uimsbf
  for (i=0;i<N;i++) {
    data_unit_id         8  uimsbf
    data_unit_length     8  uimsbf
    data_field()
  }
}
data_field() {                         // EBU teletext
  reserved_future_use    2  bslbf      // default "11"
  field_parity           1  bslbf
  line_offset            5  uimsbf
  framing_code           8  bslbf
  magazine_and_packet_address 16 bslbf
  data_block           320  bslbf
}
```

- **data_identifier** ([EN472] Table 3; corrected in [A041] Table 3, change request 1452 "Typo in data_identifier table"):
  - 0x00-0x0F reserved
  - **0x10-0x1F EBU data**
  - 0x20-0x7F reserved. EN 472 v1.3.1 misprints this range as "0x02 to 0x7F".
  - 0x80-0xFF user defined
  - It is the same value for every PES packet of a stream.
- EN 301 775 widens the permitted data_identifier to **0x10-0x1F or 0x99-0x9B**. 0x99-0x9B = "EBU Teletext and/or VPS and/or WSS and/or Closed Captioning and/or VBI sample data". 0x80-0x98 and 0x9C-0xFF are user defined. [EN775] §4.1 and Table 2.
- **data_unit_id** ([EN472] Table 4):

  | Value | Meaning |
  |---|---|
  | 0x00-0x01 | reserved |
  | **0x02** | EBU Teletext non-subtitle data |
  | **0x03** | EBU Teletext subtitle data |
  | 0x04-0x7F | reserved |
  | 0x80-0xFE | user defined |
  | **0xFF** | data_unit for stuffing |

  "For streams identified in the PSI by the DVB Teletext descriptor, only values 0x02, 0x03 and 0xFF are permitted."
- **EN 301 775 extra data_unit_ids** ([EN775] Table 3, which applies when data_identifier is 0x10-0x1F or 0x99-0x9B):

  | Value | Meaning / decoder action |
  |---|---|
  | 0x80-0xBF | user defined |
  | **0xC0** | Inverted Teletext ("transcode as EBU Teletext with an inverted framing code") |
  | 0xC1 | reserved (but it is routed to txt_data_field in the §4.4.1 syntax) |
  | 0xC2 | reserved |
  | **0xC3** | VPS |
  | **0xC4** | WSS |
  | **0xC5** | Closed Captioning |
  | **0xC6** | monochrome 4:2:2 samples |
  | 0xC7-0xFE | user defined |
  | 0xFF | stuffing |

  - Decoders that only implement EN 300 472 "have to be robust against ... data units other than 0x02, 0x03, and 0xFF". [EN775] §4.1
  - Unknown units are to be discarded. [EN775] Table 3 NOTE
- **data_unit_length**: "number of bytes in the data unit following the length field. For data units carrying EBU Teletext data, this field shall always be set to **0x2C**" (44). [EN472] §4.4
  - [EN775] §4.4.2 repeats "shall always be set to 0x2C" when data_identifier is 0x10-0x1F, and otherwise allows 0x00-0xFF. A robust parser should skip units using data_unit_length.
  - [EN775] Table 1 also allows trailing stuffing_bytes 0xFF inside a data unit.
- **field_parity**: "1" = first field of a frame, "0" = second field. [EN472] §4.4
- **line_offset** ([EN472] Table 5):
  - 0x00 = line number undefined
  - 0x07-0x16 = line 7-22 when field_parity=1, or line 320-335 when field_parity=0
  - 0x01-0x06 and 0x17-0x1F are reserved
  - Only 0x00 and 0x07-0x16 are permitted with the DVB Teletext descriptor.
  - "The toggling of the field_parity flag indicates a new field."
- **reserved_future_use**: default "1" bits. [EN472] §4.4. vbit2 actually writes 0 in these two bits (`data.push_back(((_fieldCounter&1)^1) << 5 | ...)`, [VBIT2] service.cpp), so receivers must ignore them.
- **framing_code**: `11100100` (0xE4 as stored MSB-first). For Inverted Teletext (0xC0) it is `00011011` (0x1B). [EN775] §4.5.2
- **Bit-order statement (quote), [EN472] §4.4:**
  > "framing_code, magazine_and_packet_address, data_block: these fields correspond to the 43 bytes following the clock-run-in sequence of an EBU Teletext data packet as defined in ITU-R Recommendation BT.653 [3], and also in EN 300 706 [4]. Data packets are inserted in the same order as they are intended to arrive at the Teletext decoder or to be transcoded into the VBI. Data bits are inserted in the PES packet in the same order as they would appear in the VBI, e.g. the framing code is 11100100."

  [A041] has the same text with "shall", and puts "EXAMPLE: The framing code is 11100100." on its own line.
- **EN 301 775 txt_data_field** ([EN775] §4.5.1 Table 4): reserved(2)="11", field_parity(1), line_offset(5) (valid 0, 7..22), framing_code(8) ("11100100" or "00011011"), txt_data_block(336 bits). §4.5.2:
  > "txt_data_block: this 336-bit field corresponds to the 42 bytes following the clock-run-in and framing-code sequence of an EBU or Inverted Teletext data packet as defined in EN 300 706 [3]. ... Data bits are inserted in the same order, as they would appear in the VBI."
- **Consequence:** bslbf fields are sent MSB first, so the first bit on air is the MSB of each PES byte. Each PES byte is the bit-reverse of the LSB-first T42 byte.
  - To get T42 from a data unit, bit-reverse bytes [4..45] of the unit, counting from data_unit_id at [0].
  - This is exactly what [VHS] ts.py does (`data[pos+4:pos+4+42]`, keeping units whose data_unit_id is in {2,3} and stepping 46 bytes). [VBIT2] does the inverse.
- **Other EN 301 775 data fields** ([EN775] §4.6-4.9). All start with the same 1-byte header (reserved 2 bits = "11", field_parity 1 bit, line_offset 5 bits) except monochrome.
  - VPS (0xC3): field_parity='1', line_offset=16, vps_data_block = 104 bits = 13 bytes (EN 300 231 bytes 3..15, run-in and start code excluded). Bits are in VBI order. §4.6
  - WSS (0xC4): field_parity='1', line_offset=23, wss_data_block = 14 bits followed by 2 reserved bits "11". "WSS bit 0 corresponds to the left-most bit". §4.7
  - Closed captioning (0xC5): line_offset=21, closed_captioning_data_block = 16 bits, "bit b0 of character one corresponds to the left-most bit". §4.8
  - Monochrome 4:2:2 (0xC6), §4.9 Table 12:
    - first_segment_flag(1), last_segment_flag(1), field_parity(1), line_offset(5) (7..23)
    - first_pixel_position(16) (0..719), n_pixels(8) (1..251)
    - n_pixels × Y_value(8) (0x10..0xEB)
    - Samples follow BT.601/BT.656, 720 luma samples per line. Segments of one line are carried in adjacent data units.

### 2.3 EN 300 468 descriptors ([EN468])
- **teletext_descriptor, tag 0x56** ([EN468] Table 12 "Possible locations of descriptors"; §6.2.43 Table 101):

  ```
  descriptor_tag 8, descriptor_length 8,
  for(i<N){ ISO_639_language_code 24 bslbf; teletext_type 5 uimsbf;
            teletext_magazine_number 3 uimsbf; teletext_page_number 8 uimsbf }
  ```

  - Each entry is 5 bytes, so N = descriptor_length / 5 (derived).
  - ISO_639_language_code: 3 characters of ISO 639-2 (B or T), each coded in 8 bits per ISO/IEC 8859-1, in order. Example "fre" = 0x66 0x72 0x65.
  - teletext_type (Table 102):
    - 0x00 reserved
    - 0x01 initial teletext page
    - 0x02 teletext subtitle page
    - 0x03 additional information page
    - 0x04 programme schedule page
    - 0x05 teletext subtitle page for hearing impaired people
    - 0x06-0x1F reserved
  - teletext_magazine_number is "as defined in EN 300 706". A value of 0 therefore means magazine 8 ([EN706] §3).
  - teletext_page_number: "8-bit field giving two 4-bit hex digits" (tens nibble high, units nibble low; the nibble order is inferred from "two hex digits").
- **VBI_teletext_descriptor, tag 0x46** ([EN468] §6.2.48 Table 108): same syntax and semantics as the teletext_descriptor. The difference is that it "is not to be used to associate stream_type 0x06 with the VBI standard nor the EBU teletext standard". Decoders use it only to pick languages for magazines and subtitles.
- **VBI_data_descriptor, tag 0x45** ([EN468] §6.2.47 Table 106/107):
  - Structure: loop of data_service_id(8), data_service_descriptor_length(8), then for ids 0x01, 0x02, 0x04-0x07 one byte per line: reserved(2), field_parity(1), line_offset(5). Other ids have reserved bytes.
  - data_service_id values:
    - 0x01 EBU teletext (needs an additional teletext_descriptor)
    - 0x02 inverted teletext
    - 0x04 VPS
    - 0x05 WSS
    - 0x06 closed captioning
    - 0x07 monochrome 4:2:2
    - 0xF0-0xFF user defined
  - EN 301 775 streams must carry exactly one VBI_data_descriptor. [EN775] §4.2

### 2.4 MPEG-TS basics (ISO/IEC 13818-1; I used secondary sources because I had no local copy of 13818-1)
- **Packet:** 188 bytes, sync byte 0x47 (https://en.wikipedia.org/wiki/MPEG_transport_stream).
- **4-byte header, bit layout MSB first:**

  | Byte | Bits | Field |
  |---|---|---|
  | 0 | 8 | sync (0x47) |
  | 1 | bit 7 | transport_error_indicator |
  | 1 | bit 6 | payload_unit_start_indicator (PUSI) |
  | 1 | bit 5 | transport_priority |
  | 1 bits 4-0 + byte 2 | 13 | PID |
  | 3 | bits 7-6 | transport_scrambling_control |
  | 3 | bits 5-4 | adaptation_field_control (01 = payload only, 10 = adaptation only, 11 = adaptation then payload, 00 = reserved) |
  | 3 | bits 3-0 | continuity_counter |

  (Wikipedia, same page.) The same masks appear in [VHS] ts.py: `p&0x1fff` PID, `p & 0x4000` PUSI, `c & 0x20` adaptation present, payload offset `4 + packet[4] + 1`.
- **Adaptation field:** the first byte is adaptation_field_length, the number of bytes that follow. The payload starts at offset 4 + 1 + adaptation_field_length ([VHS] ts.py; Wikipedia).
- **PUSI on a PES PID:** a new PES packet starts at the first payload byte (00 00 01 start code). ([VHS] ts.py collects payloads from each PUSI to the next.)
- **Reserved PIDs:** PID 0x0000 = PAT; 0x1FFF = null packets (Wikipedia MPEG TS).
- **PSI:** PAT table_id 0x00 and PMT table_id 0x02 (https://en.wikipedia.org/wiki/Program-specific_information).
  - PAT entry: program_number(16), reserved(3), PID(13).
  - PMT: PCR_PID(13), program_info_length(10), then per ES: stream_type(8), reserved(3), elementary_PID(13), reserved(4)+ES_info_length(10 used of 12), descriptors.
  - When PUSI is set on a PSI packet, a 1-byte pointer_field comes before the section. Sections end with CRC32.
  - Teletext is stream_type 0x06 carrying descriptor 0x56 ([EN472] §4).
- **PES header (13818-1, as used by [VBIT2]):**

  ```
  00 00 01 | stream_id | PES_packet_length(16)
  | '10' scrambling(2) priority data_alignment copyright original
  | PTS_DTS_flags(2) ESCR ES_rate DSM_trick add_copy CRC ext
  | PES_header_data_length(8) | optional fields + stuffing
  ```

  - PTS is 5 bytes: `0010 PTS[32..30] 1 | PTS[29..15] 1 | PTS[14..0] 1` ([VBIT2] service.cpp push sequence 0x21|..., 0x01|..., 0x01|...).
  - Payload start = 9 + PES_header_data_length. For DVB teletext this is 9 + 0x24 = 45.
- **M2TS (BDAV):** 192-byte packets. "The standard MPEG-2 TS 188-byte packet is prefixed with a 4-byte extra header" consisting of "a 2-bit copy permission indicator and the 30-bit arrival timestamp with a resolution of 27 MHz" (https://en.wikipedia.org/wiki/.m2ts).
  - The sync byte is therefore at offset 4 of each 192-byte unit.
  - The byte order of the 30-bit timestamp is big-endian by 13818-1 convention (UNVERIFIED from a primary source).
  - Also seen: 204-byte packets (188 + 16 bytes Reed-Solomon, DVB) (Wikipedia MPEG TS "communication mediums may add 16 bytes (DVB)").

---------------------------------------------------------------------------------------------------

## 3. Linux V4L2 VBI

### 3.1 Sliced VBI ([KSLICED], [VDEV2])

```c
struct v4l2_sliced_vbi_data {      /* videodev2.h */
    __u32 id;        /* V4L2_SLICED_* flag; exactly one bit set; 0 = empty packet, ignore */
    __u32 field;     /* 0: first field, 1: second field */
    __u32 line;      /* field (not frame) line number; 0 if hardware can't identify lines */
    __u32 reserved;  /* must be 0 */
    __u8  data[48];  /* payload; unused tail bytes undefined */
};                   /* 64 bytes total (4*4 + 48) - derived */
```

- **V4L2_SLICED_TELETEXT_B = 0x0001.** Reference: ETS 300 706 and ITU BT.653. Lines "PAL/SECAM line 7-22, 320-335 (second field 7-22)". Payload: "Last 42 of the 45 byte Teletext packet, that is without clock run-in and framing code, lsb first transmitted." [KSLICED] §4.7.3.2; [VDEV2] `#define V4L2_SLICED_TELETEXT_B (0x0001)`
- Other service flags:
  - V4L2_SLICED_VPS = 0x0400 (13 bytes, EN 300 231 bytes 3-15, LSB first)
  - V4L2_SLICED_CAPTION_525 = 0x1000
  - V4L2_SLICED_WSS_625 = 0x4000
  - V4L2_SLICED_VBI_625 = 0x4401
  - V4L2_SLICED_VBI_525 = 0x1000

  [VDEV2]; [KSLICED]
- `line` is the field line number in ITU numbering. So a second-field teletext line carries field=1, line=7..22, not 320..335 ([KSLICED] "PAL/SECAM line 7-22, 320-335 (second field 7-22)"; struct field description).
- read(): "A single read() or write() call must pass all data belonging to one video frame. That is an array of struct v4l2_sliced_vbi_data structures with one or more elements and a total size not exceeding io_size bytes." Unused elements have id 0. Packets arrive in ascending line order; when line numbers are unknown they arrive in transmitted order. Drivers must send at least one empty packet for frames with no data. [KSLICED] §4.7.4
- `struct v4l2_sliced_vbi_format`: `__u16 service_set; __u16 service_lines[2][24]; __u32 io_size; __u32 reserved[2];` [KSLICED]
- **MPEG-embedded sliced VBI (ivtv/cx18), V4L2_MPEG_STREAM_VBI_FMT_IVTV** ([KSLICED] §4.7.5):
  - Carried in an MPEG-2 PS private_stream_1 PES.
  - Payload: `__u8 magic[4]` then a union.
  - Magic "itv0": `__le32 linemask[2]` + 1..35 × `v4l2_mpeg_vbi_itv0_line`. Linemask bit mapping: linemask[0] b0 = line 6 field 1 ... b17 = line 23 field 1, b18 = line 6 field 2 ... linemask[1] b3 = line 23 field 2.
  - Magic "ITV0": exactly 36 lines, line[0..17] = lines 6-23 of field 1, line[18..35] = field 2.
  - `v4l2_mpeg_vbi_itv0_line { __u8 id; __u8 data[42]; }`. id values: 1 = TELETEXT_B, 4 = CAPTION_525, 5 = WSS_625, 7 = VPS.
  - Payload is at most 1552 bytes.

### 3.2 Raw VBI ([KRAW] §4.6.3, [VDEV2])

```c
struct v4l2_vbi_format {
    __u32 sampling_rate;    /* samples per second (Hz) */
    __u32 offset;           /* samples from leading edge of line sync to first sample */
    __u32 samples_per_line;
    __u32 sample_format;    /* fourcc, usually V4L2_PIX_FMT_GREY: 8 bit, lower = nearer black */
    __s32 start[2];         /* ITU line number of first captured line of field 1 / field 2; 0 = unknown */
    __u32 count[2];         /* lines captured per field */
    __u32 flags;            /* V4L2_VBI_UNSYNC 0x0001, V4L2_VBI_INTERLACED 0x0002 */
    __u32 reserved[2];
};
```

- Default layout: all lines of field 1 followed by all lines of field 2 (the first field "stores the first field (lower scanning line numbers) first in memory"). V4L2_VBI_INTERLACED alternates lines between fields. V4L2_VBI_UNSYNC means the first stored field may be either field. [KRAW] Raw VBI Format Flags
- Frame size = (count[0] + count[1]) × samples_per_line × sample size. The smallest unit per read() is one frame. [KRAW] §4.6.4
- "Do not assume any other correlation of values with the signal level ... 128 may not be the mean value." [KRAW]
- Constants: V4L2_VBI_ITU_625_F1_START = 1, V4L2_VBI_ITU_625_F2_START = 314, V4L2_VBI_ITU_525_F1_START = 1, V4L2_VBI_ITU_525_F2_START = 264. [VDEV2]

---------------------------------------------------------------------------------------------------

## 4. Raw VBI capture files (vhs-teletext)

- The file is headerless: a concatenation of raw V4L2 VBI frames as read from /dev/vbiN. `teletext record -d /dev/vbi0 > capture.vbi` reads chunks of `line_length*field_lines*2` bytes and writes them unchanged. ([VHS] README; teletext/cli/teletext.py `record()`)
- **Card presets** ([VHS] teletext/vbi/config.py `Config.cards`; default card = 'bt8x8'):

  | card | sample_rate (Hz) | line_length (samples) | dtype | field_lines | field_range (lines used) | line_start_range |
  |---|---|---|---|---|---|---|
  | **bt8x8** (default) | 35468950.0 | 2048 | uint8 | 16 | range(0,16) | (60,130) |
  | cx88 | 35468950.0 | 2048 | uint8 | 18 | range(1,17) | (90,150) |
  | tbc (ld-decode/vhs-decode, full fields) | 17730000.0 | 1135 | uint16 | 313 | range(6,22) | (160,190) |
  | tbc-vbi (ld-decode/vhs-decode, VBI only) | 17730000.0 | 1135 | uint16 | 16 | range(0,16) | (160,190) |
  | saa7131 | 27000000.0 | 1440 | uint8 | 16 | range(0,16) | (0,20) |

  - `frame_lines = field_lines*2`.
  - `line_bytes = line_length * itemsize`, so a tbc line is 2270 bytes. The uint16 byte order (little-endian via numpy default) is UNVERIFIED.
  - `teletext_bitrate = 6937500.0`.
  - The clock run-in + framing pattern used for alignment is `crifc = 1,-1 ×8, then 1,1,1,-1,-1,1,-1,-1` (CRI 16 bits then FC 11100100).
- **bt8x8 frame:** 2 × 16 × 2048 = 65536 bytes.
  - Kernel bttv PAL defaults: `VBI_BPL 2048`, `VBI_DEFLINES 16`, `Fsc = 35468950` (sampling_rate), `vbistart = {7, 320}`, `VBI_OFFSET 244`. So field 1 = lines 7-22 and field 2 = lines 320-335, 8-bit GREY. (torvalds/linux drivers/media/pci/bt8xx/bttvp.h, bttv-vbi.c, bttv-driver.c tvnorm PAL entry)
  - The last 4 bytes of each frame buffer are overwritten with the frame sequence number (little-endian u32 in vhs-teletext's reading: `struct.unpack('<I', chunk[-4:])`). This happens only with read() I/O. Kernel comment: "Ugly hack for backwards compatibility. Some applications expect that the last 4 bytes of the VBI data contains the sequence number." (bttv-driver.c ~line 2776; [VHS] cli/teletext.py record())
  - Consequence: the last 4 samples of line 335 are corrupt. This is outside the teletext data window (derived).
- FileChunker reads `size`-byte lines. Per field it skips `field_range.start` lines, uses `len(field_range)` lines, and skips the rest. ([VHS] teletext/file.py)
- **zvbi "old sliced" capture format** (zvbi test/sliced.c `write_func_old_sliced` / `read_loop_old_sliced`, github.com/zapping-vbi/zvbi branch main). Per frame:
  1. ASCII text of the time delta since the previous frame (`"%f\n"`, seconds).
  2. One byte `n_lines` (+1 if a raw block follows).
  3. Per line: service index byte, line number (2 bytes, little-endian; the high byte is masked &15 on read), then payload.
     - Service index → payload size: 0 = Teletext B (42 bytes), 1 = Caption 625 (2), 2 = VPS (13), 3 = WSS 625 (2), 4 = WSS CPR1204 (3), 7 = Caption 525 (2).
     - Index 255 = raw block. It starts with a header: u16 line=0, u16 system, u32 sampling_rate, u16 samples_per_line, u16 bytes_per_line, u16 offset, u16 start[2], u16 count[2], u8 interlaced, u8 synchronous (all little-endian). Raw samples follow.
  - The teletext payload is in vbi_sliced order, which in zvbi is LSB-first like V4L2. UNVERIFIED from zvbi docs; inferred from V4L2 compatibility.
  - zvbi sliced.c also reads and writes DVB PES and TS (`FILE_FORMAT_DVB_PES/_TS`).

---------------------------------------------------------------------------------------------------

## 5. TTI (MRG) page format

### 5.1 Original MRG spec ([MRG] v1.1)
- "A teletext page uses the file extension .tti and contains lines of text. It is a text format file with each line terminated by carriage return and line feed. Each line in the file starts with a two character command code". [MRG] §2
- Commands in [MRG]:
  - **PN,mppss** — page number: m = magazine, pp = page, ss = subpage. Example `PN,10003`. §2.1
  - **CT,n,<t>** — cycle time. n = delay; t = `C` (cycled: number of magazine cycles) or `T` (timed: seconds). §2.2
  - **DE,<text>** — description, "up to 24 characters", informational. §2.3
  - **PS,ssss** — page status, 4 hex digits. `PS,8000` = normal parallel transmission, `PS,8040` = serial. Full bit table in §5.3. §2.4
  - **SC,ssss** — subcode, 0000 to 3F7F, "generally set to the same as ss in PN". §2.5
  - **OL,nn,<line>** — output line, "n is 1 to 27", with the translations below. §2.6
  - **FL,<red>,<green>,<yellow>,<cyan>,<link>,<index>** — Fastext link: "A three digit page number mpp for each link or 0 for no link." §2.7
  - **DS,<text>** — appears in the §4 sample (`DS,inserter`) but is not defined in the document. Meaning (destination/inserter name?) is UNVERIFIED; ignore it.
- **Control code translations** ([MRG] §3):
  - "For codes less than hex 0x20 then add 0x80." (0x00-0x1F are stored as 0x80-0x9F.)
  - "If using a system with seven bit characters then you can also use "Viewdata" escapes where if a character has a code less than 0x20 then it is replaced by <esc> followed by the character code plus hex 0x40." (ESC = 0x1B; e.g. 0x01 alpha red → 1B 41.)
  - "Double height is hex 0x0D which can upset text parsers so it is often replaced with 0x10 and an MRG inserter will map the character back when transmitting it." A reader should therefore map 0x10 to 0x0D, and 0x90 to 0x0D in the 8-bit form (the 8-bit case is inferred).
- There is no parity in TTI OL text. Characters are 7-bit codes; parity is added on transmission (vbit2 computes it; inferred from [VBIT2] wiki "before hamming encoding, which is performed internally by VBIT2").
- MS-DOS 8.3 filenames are recommended (MiniTED is 16-bit). [MRG] §2

### 5.2 vbit2 extensions ([VBIT2] wiki Page-files; the Teletext Wiki page "MRG_TTI_format" confirms that VBIT/VBIT2/wxTED use TTI and that VBIT2's extensions are documented there)
- **PN:** `mppss` where m = 1-8, pp = hex 00-FF, ss = decimal subpage.
  - Several PN lines in one file = a carousel; subsequent lines start new subpages, and mpp must match.
  - Default when absent is 1FF.
- **CT:** default 1 magazine cycle.
- **SC:** 4 hex digits per ETS 300 706 §9.3.1.2; bits 7, 14, 15 reserved. It is ignored for carousels (renumbered per Annex A.1) and default 0000.
- **OL:** row numbers 1-29 are accepted. A missing row 1-25 counts as all spaces.
  - Control codes use either the 8-bit translation (set bit 7) or the 7-bit ESC form (0x1B followed by code|0x40).
  - An mFF page may contain OL,0 as a custom header template.
- **FL:** six `mpp` hex page numbers, per ETS 300 706 Annex H (FLOF, the X/27/0 editorial links).
  - All links get subcode 3F7F.
  - Unused links are coded 0 or 8FF.
  - If FL is missing or all links are 8FF, no X/27 packet is generated.
  - edit.tf writes FL values with `toString(16)`, lower-case hex (so hex page numbers like 1A0 are possible) and omits the line when all links are 8FF ([EDITTF] teletext-editor.js export_frame).
- **RE,h** (non-standard): one hex digit = bits 3-6 of the Default G0/G2 Character Set designation (§15.2). Default 0.
- **PF,f,c** (non-standard): page function 0-B and page coding 0-3 (§9.4.2.1). Default LOP, 7-bit odd parity.
- **Enhancement packets in OL rows 26, 28, 29:**
  > "the second argument is interpreted as an enhancement packet. The low 4 bits of the first character is the packet designation code [$0-$F], the remaining characters encode 13 groups of 18-bit triplet values in the lower 6 bits of three characters. Bit 6 of each character is set, i.e. all characters are in the range $40-$7F. Within each group of three characters, the first contains the least significant bits, and the most in the last. This is the enhancement packet data before hamming encoding"

  - So: 1 + 39 characters, triplet = (c0&0x3F) | (c1&0x3F)<<6 | (c2&0x3F)<<12.
  - Up to 16 OL lines per row number, each with a unique designation code.
  - An X/28/0, 2, 3 or 4 line also sets the page function/coding.
  - Row 29 on an mFF page = M/29 magazine packets, not transmitted with the page.
- **OL,27 (linking):** the first char's low 4 bits = designation code.
  - DC 0-3: the rest is 39 nibbles in the low 4 bits of chars in $40-$4F. The CRC in X/27/0 is generated automatically.
  - DC 4-F: 13 triplets encoded as for X/26.
- **Page coding 2:** rows X/1-X/25 use the X/26 triplet encoding. **Page coding 3:** rows are 40 nibbles as in X/27/0-3. Page coding 1 (8-bit) is unsupported.
- **Transmission order:** with coding 0, X/26/0-15 go before X/1-X/25.
- The zxnet editor TTI reader takes the designation code from the character right after `OL,26,`, `OL,27,` or `OL,28,` (`charCodeAt(6) & 0xF`) and the data from the characters after it ([ZXED] importfileParseTTI). This matches vbit2.

### 5.3 PS page-status bits
[MRG] §2.4 table (PS hex value → C bit):

| PS hex | Bit | Meaning | C bit |
|---|---|---|---|
| 4000 | 14 | Erase page | C4 |
| 0001 | 0 | Newsflash | C5 |
| 0002 | 1 | Subtitle | C6 |
| 0004 | 2 | Suppress header | C7 |
| 0008 | 3 | Update | C8 |
| 0010 | 4 | Interrupted sequence | C9 |
| 0020 | 5 | Inhibit display | C10 |
| 0040 | 6 | Serial magazine | C11 |
| 0000-0380 | 7-9 | Language | C12-C14 |
| 8000 | 15 | Transmit page | — |
| 0800 | — | Substitute page | — |

- "Timed page: use CT instead".
- LLL languages (Western European): English 000, German 001, Swedish/Finnish 010, Italian 011, French 100, Portuguese/Spanish 101, Czech/Slovak 110.
- The doc's mnemonic bit string "TE.P.MLL L.DIUHSN" contradicts its own table for M (0040 = bit 6) and P (0800). Trust the hex values.
- vbit2's table ([VBIT2] wiki): bits 0-6 = C5..C11, bits 7-9 = NOS C12-C14, bits 10-13 ignored, bit 14 = C4, bit 15 = Transmit. Default when absent is 0000, and a page is only transmitted when bit 15 is set.
- **Ambiguity (flag for requirements):** the order of C12/C13/C14 within PS bits 7-9 differs between implementations.
  - vbit2 header generation: `cbit=(control & 0x0380) >> 6` goes into the C11-C14 nibble, so **bit7 = C12, bit8 = C13, bit9 = C14** ([VBIT2] packet.cpp Header()).
  - The zxnet T42 importer builds PS with `h9<<6` from the C11-C14 nibble, which gives the same mapping (bit7 = C12) ([ZXED] importfileParseT42).
  - edit.tf export maps m_control[12] → 0x0200, [13] → 0x0100, [14] → 0x0080, which is reversed ([EDITTF] export_frame).
  - Recommendation (not from any source): follow vbit2 (bit7 = C12) and test against real files.
- vbit2 behaviour: when the Update bit is set, C8 is sent once and then cleared. C4 is set to clear old rows. With C9 on a carousel, the subpage is sent immediately when its timer expires. ([VBIT2] wiki)

### 5.4 Commands requested but not found in any spec
- SP, RD, RT, CY, MS: not in [MRG] and not in [VBIT2] wiki.
  - No source defines SP or the others; their meaning is UNVERIFIED.
  - A reader should ignore unknown two-letter commands. (This is a recommendation.)

---------------------------------------------------------------------------------------------------

## 6. EP1 (Softel Flair) and other page files

### 6.1 EP1
Source: https://teletext.wiki.zxnet.co.uk/wiki/EP1_format ("reverse engineered by analysing test files generated by Flair32"). Cross-checked against the [ZXED] exportEP1/importfileParseEP1 source and the edit.tf export.

- **Header, bytes 0x00-0x05:**
  - 0x00 = 0xFE
  - 0x01 = 0x01
  - 0x02 = language code: 07 Czech/Slovak, 08 Danish, 09 English, 0B French, 0D German, 0E Greek, 11 Italian, 14 Polish, 16 Rumanian, 17 Portuguese/Spanish, 18 Swedish/Finnish, 1C Turkish, 1E Serbian/Croatian/Slovenian, FF other
  - 0x03 = 0xCA if enhancement data is present, else 0x00
  - 0x04 / 0x05 = offset (little-endian) from the end of the 6-byte header to the Level 1 data
- **Optional Level 1.5 enhancement block:**
  - 0x06 = 0xC2, 0x07 = 0x00
  - 0x08 / 0x09 = enhancement length (little-endian)
  - Then n packets (1-16) of 40 bytes each: designation code (0x00-0x0F) followed by 13 triplets × 3 bytes (address, 5-bit mode, 7-bit data). The terminator address is 0x7F instead of 0x3F.
  - Triplets are stored un-Hammed as (addr 6 bits, mode 5 bits, data 7 bits). [ZXED] exports `triplet & 0x3f, (triplet >> 6) & 0x1F, triplet >> 11`.
  - The wiki gives the per-packet offset as `0x0A + 40n`. [ZXED] sets offset = 40·n + 4, so the packet data starts at byte 0x0A (consistent).
- **Level 1 data, at 6 + offset:**
  - 960 bytes = rows 0-23 × 40 (7-bit codes, no parity)
  - +0x3C0: a 40-byte buffer (row pushed off the page while editing; [ZXED] writes spaces or row 24 here)
  - +0x3E8: a 2-byte terminator 0x0000
- **Exact sizes:**
  - Without enhancements: 6 + 960 + 40 + 2 = **1008 bytes** (derived).
  - With n enhancement packets: 1008 + 4 + 40n bytes (derived from [ZXED]).
  - The row-0 content is included (bytes 0-7 are normally the page header area).
  - edit.tf's "experimental EP1" writes 6 + 1000 + 2 = 1008 bytes: it puts rows 0-24 in the 1000 bytes ([EDITTF] export_frame). This fits the same size.
- **EPX:** a 6-byte header ("JWC", page count, 00 00) followed by concatenated EP1 files. (Same wiki page)

### 6.2 Other formats
- **FAB ETT .TTP** (F. A. Bernhardt), from https://teletext.wiki.zxnet.co.uk/wiki/FAB_ETT_format quoting the ETT manual:
  - Byte 0 = number of used rows (1-49).
  - Bytes 1-42 = row 0: SO (0x0E), 0x80, 30 bytes of page name + odd parity, 80h ×3, margins, display time, flag byte (inhibit header, C4, out of sequence, inhibit display, C5, C6), and character set (0=GB, 4=D, 2=S, 6=I, 1=F, 5=E, 3=CS) + parity.
  - Then 42-byte data rows: SO (0x0E), row number + odd parity, 40 data bytes with odd parity.
  - Bytes 2060-2069 = last 10 bytes of the page name.
  - Row order: X/27, X/26, X/1..23, X/24, X/25.
  - Unused space is filled with 0x20.
  - Size is up to 2070 bytes (derived).
- **CEBRA .ttx** ("CEBRA Teletext V1.0"): [ZXED] exportCebraText writes the ASCII magic `CEBRA Teletext V1.0` followed by nested TLV blocks.
  - Each block is type byte + BER-style length (<0x80 one byte; 0x81 n; 0x82 n16; 0x83 n24; 0x84 n32, big-endian) + contents.
  - Types used: 0x30 = container, 0x02 = small integer / bytes, 0x04 = octet string.
  - Rows = 0x30{0x02[row], 0x04[40 bytes]}; X/27 is Hamming-coded; the subpage header is 8 Hamming nibbles.
  - No official spec was found; this is PARTIAL / UNVERIFIED (reverse-engineered by zxnet).
- **Raw .bin page:** [ZXED] export "bin" = 25 × 40 = 1000 bytes of 7-bit codes, row-major, rows 0-24. On import it reads 24 or 25 rows × 40 and masks &0x7F. It is the importer's fallback for unknown extensions.
- **edit.tf raw exports:** 25 lines × 40 chars, each line terminated by "\n" (1025 bytes). One variant keeps 0x00-0x1F as-is; the other adds 0x80 to them. ([EDITTF] README "Esc E"; export_frame rawstring_0 / rawstring_1)
- **zxnet export options:** tti7 (7-bit ESC-form TTI), ep1, ttx, bin, bbc, t42 (with a "row adaptive" option), png/gif, edit.tf URL, viewdata ([ZXED] export dialog).
- zxnet's T42 import treats a .t42 page file as a single-magazine packet sequence. It reads row 0 (page number, subcode, control bits), rows 1-25, X/26 (sequential DCs), X/27/0 (FLOF links) and X/28 ([ZXED] importfileParseT42).

---------------------------------------------------------------------------------------------------

## 7. edit.tf / zxnet URL hash

Sources: [EDITTF] README "All frame data is kept in the URL"; teletext-editor.js save_to_hash / load_from_hashstring; [ZXED] update*Keypair functions.

- **Form:** `#<meta>:<data>[:<key>=<value>]...`
  - Seen as e.g. `https://edit.tf/#0:QIECBAgQ...` and `https://zxnet.co.uk/teletext/editor/#0:...`.
- **`<meta>`:** one hex nybble. The low 3 bits select the character set; the MSB (value 8) enables black foreground.
  - edit.tf charset names: 0 English, 1 German, 2 Swedish, 3 Italian, 4 Belgian, 5 ASCII, 6 Hebrew, 7 Cyrillic (teletext-editor.js ~line 1534).
  - Unparseable metadata → 0.
- **`<data>`:** base64url (RFC 4648 §5 alphabet `A-Z a-z 0-9 - _`, no padding) of a bit string. "the seven-bit character code for column c and row r appears at bit positions 280r+7c to 280r+7c+6 inclusive, the most significant bit appearing first."
  - Each base64 digit carries 6 bits, MSB first (`b64[...] |= bit << (5 - offset)`).
  - 25 rows × 40 cols × 7 bits = 7000 bits → **1167 characters**, with the last 2 bits unused.
  - Legacy lengths accepted on load:
    - 1120 characters = 24-row frames (6720 bits exactly)
    - 1920 hex digits = old hex format, 24 rows, byte at 2·(40r+c)
  - Row 0 is included in the data (the header row, which the editor normally leaves as-is).
- **Extended keys** (split on ':' then 'key=value'; edit.tf preserves unknown keys):
  - `PN` = hex page mpp (default 8FF; invalid if <0x100, >0x8FF or pp=FF)
  - `SC` = hex subcode (invalid if &0xC080 or >0x3F7E; default 3F7F)
  - `PS` = hex page status in TTI PS encoding (edit.tf rejects values > 0xC3FF)
  - `X270` = FLOF links, 6 × 7 hex chars (3 page + 4 subcode); zxnet appends one more char (`F`/`0`) for the link-control/row-24 flag
  - zxnet only:
    - `X25` = row 25 as 47 base64 chars, same 7-bit packing
    - `X26` = enhancement triplets, 3 base64 chars each, most-significant 6 bits first
    - `X280` / `X284` = page function/coding, G0/G2 and NOS, side panels, 16 CLUT entries as 3-hex-digit 12-bit colours, and dsc/drc/bbcs/ctm, all as hex strings
    - `RE` = region nybble
    - `zx` = editor UI state (cursor, render level, flags); stripped on export

  ([ZXED] updateX25Keypair, updateX26Keypair, updateX28keypair, loadFromX28String)

---------------------------------------------------------------------------------------------------

## 8. BBC Micro MODE 7 screen dumps
- MODE 7 is 40 × 25 characters, about 1000 bytes of screen memory. The screen normally starts at &7C00 in a 1 KB region (search summary from https://xania.org/201405/jsbeeb-part-two and https://www.dfstudios.co.uk/crypt/Online/47/bbc_screens.html; not checked against a primary Acorn document, UNVERIFIED).
- The zxnet "bbc" export defines a concrete convention ([ZXED] export "bbc"):
  - 1000 bytes, row-major, 25 rows × 40, rows 0-24.
  - **Every byte OR 0x80**, so control codes become 0x80-0x9F and printable characters 0xA0-0xFF.
  - If a row contains double height (0x0D), the **next row is written as a copy of the double-height row**. The BBC's SAA5050 needs the bottom half repeated; broadcast teletext leaves the next row to be ignored.
- The zxnet .bin importer masks every byte with &0x7F, so BBC dumps (high-bit set) and plain 7-bit dumps both load ([ZXED] importfileLoaded default case).
- Reader recommendation (derived): accept 1000 bytes (or 960 for 24 rows, or a 1024-byte padded dump — UNVERIFIED that such dumps exist), mask with 0x7F, and optionally de-duplicate the row after a double-height row.

---------------------------------------------------------------------------------------------------

## 9. SDI VANC carriage (low priority)

- **OP-47 / SMPTE RDD 8-2008** "Storage and Distribution of Teletext Subtitles and VBI Data for High-Definition Television" (https://www.drastic.tv/images/documentation/rdd08-2008.pdf; OP-47 Issue 6 at https://www.freetv.com.au/wp-content/uploads/2019/08/OP-47-Storage-and-Distribution-of-Teletext-Subtitle-and-VBI-Data-Issue-6-May-2018.pdf).
  - Type 2 ANC packet: ADF 000h 3FFh 3FFh.
  - Subtitling Distribution Packet (SDP): DID/SDID = **143h / 102h** (with parity, i.e. 0x43/0x02).
  - Multipacket: 143h / 203h. WSS: 250h / 101h.
  - UDW words: 8-bit data in b7-b0, b8 = even parity, b9 = !b8. UDW ≤ 255 words. §4.2
  - SDP UDW layout (§5.1):
    1. IDENTIFIER 151h, 115h
    2. LENGTH (words from the first identifier to the SDP checksum)
    3. FORMAT CODE 102h (WST teletext subtitles)
    4. 5 × Structure A descriptor words: b0-b4 line (0, 6-22), b5-b6 reserved, b7 field (1 = odd field, 0 = even), b8-b9 parity. All-zero = no packet.
    5. For each non-zero descriptor, a Structure B = the 45-byte WST packet: **run-in 255h 255h, framing code 227h, MRAG 2 words, 40 data words** (so 8-bit 0x55 0x55 0x27, i.e. LSB-first/T42 byte order)
    6. FOOTER ID 274h
    7. 16-bit footer sequence counter (MSB word first; increments per SDP, wraps at 65535)
    8. SDP checksum (8-bit sum of identifier..checksum = 0 mod 256)
    9. Standard ANC CS
- **SMPTE ST 2031** (2007; revised 2015; https://www.drastic.tv/images/documentation/st2031-2015.pdf): carries DVB/SCTE VBI data in VANC with **DID 41h, SDID 08h**. The UDW consists of EN 301 775 (or ANSI/SCTE 127) data_field() structures, identified by data_identifier and data_unit_id. So the teletext bytes are in DVB (MSB-first/bit-reversed) order with the field_parity/line_offset header.
  - This comes from a web search summary of the ST 2031 PDF; the exact UDW header layout is UNVERIFIED.

---------------------------------------------------------------------------------------------------

## Gaps / UNVERIFIED list
1. ISO/IEC 13818-1 itself was not consulted. TS/PES/PSI/M2TS details come from Wikipedia and from vbit2/vhs-teletext code. The M2TS TP_extra_header byte order is unverified.
2. TTI commands DS, SP and others outside the MRG/vbit2 set have no primary definition. The order of C12-C14 within PS bits 7-9 conflicts between vbit2 and edit.tf.
3. The TTA index endianness is not stated.
4. CEBRA .ttx has no official spec; only the zxnet writer was examined.
5. The BBC MODE 7 dump layout is conventional, based on the zxnet exporter, not an Acorn document.
6. The SMPTE 2031 UDW layout was not read from the standard.
7. zvbi's teletext payload bit order was inferred, not read from zvbi docs.
8. The byte order of vhs-teletext 'tbc' uint16 samples was not confirmed.
