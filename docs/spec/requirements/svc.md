# F.6 Broadcast service data (SVC)

**REQ-SVC-001 — Packet 8/30 format 1.**
For packet number 30 in magazine 8 with designation code 0 or 1, the decoder shall extract the following (clause 9.8.1 and Table 18):

| Field | Source and handling |
|---|---|
| Multiplexed or full-field flag | Designation code bit 1 |
| Initial page | Bytes 7 to 12, Hamming 8/4, with an **absolute** magazine in byte 10 data bit 4 and byte 12 data bits 3 and 4 |
| Network identification | Bytes 13 and 14, a 16-bit value transmitted most significant bit first, reassembled accordingly |
| Time offset | Byte 15. Half-hours in bits 2 to 6; negative when bit 7 is set |
| Modified Julian Date | Bytes 16 to 18, five digits, each reduced by one after reception |
| Coordinated Universal Time | Bytes 19 to 21, six digits, each reduced by one |
| Status message | Bytes 26 to 45, 20 characters with parity |

*Verification:* Test against packets built by the encoder and against a captured packet.

**REQ-SVC-002 — Date and time.**
The decoder shall convert the Modified Julian Date and the time to a calendar date and time of day, where day 45000 is 31 January 1982. It shall report:
- the time as Coordinated Universal Time;
- the local time given by the offset.

The time refers to the next second boundary after the packet (clause 9.8.1), and the report shall say so. Out-of-range digits, such as an hour above 23, shall make the date or time invalid.
*Verification:* Test.

**REQ-SVC-003 — Packet 8/30 format 2.**
For designation codes 2 and 3, the decoder shall extract:
- the initial page and the status message, coded as in format 1;
- bytes 13 to 25 (the Programme Delivery Control data), stored raw without interpretation (Part G).
*Verification:* Test.

**REQ-SVC-004 — Use of the initial page.**
The initial page from packet 8/30 shall be used:
- as the page shown when the decoder starts;
- as the target of the Index key when a page has no index link.

When no 8/30 packet has been received, page 100 shall be used (Annex D.3).
*Verification:* Test.

**REQ-SVC-005 — Service name.**
The 20-character status message shall be available to the caller as text, decoded with the default G0 set without national option substitution.
*Verification:* Test.
