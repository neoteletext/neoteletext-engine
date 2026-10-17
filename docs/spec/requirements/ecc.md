# F.3 Error-protection codes (ECC)

**REQ-ECC-001 — Odd parity check.**
A function shall take a byte and report whether it has odd parity, meaning an odd number of "1" bits among its eight bits. It shall also report the 7-bit character value, which is the byte with bit 7 cleared (EN 300 706 clause 8.1).
*Verification:* Test over all 256 byte values.

**REQ-ECC-002 — Hamming 8/4 decoding.**
A function shall decode a Hamming 8/4 byte exactly as clause 8.2 specifies. The data bits are in positions 2, 4, 6 and 8, with bit 2 carrying the weight 1. The function shall return:
- the 4-bit value;
- a status of "no error", "corrected", or "uncorrectable".

A single error in a protection bit, including P4, shall be reported as "corrected", and the data shall be returned. A double error shall be reported as "uncorrectable". The function shall be table-driven: a 256-entry table generated from the encoding equations, not typed in by hand.
*Verification:* Exhaustive test (REQ-TST-002).

**REQ-ECC-003 — Hamming 8/4 encoding.**
A function shall encode a 4-bit value into a Hamming 8/4 byte using the equations of clause 8.2. It shall produce the sixteen codes 15, 02, 49, 5E, 64, 73, 38, 2F, D0, C7, 8C, 9B, A1, B6, FD and EA (hexadecimal) for the values 0 to 15.
*Verification:* Test.

**REQ-ECC-004 — Hamming 24/18 decoding.**
A function shall decode a three-byte triplet as clause 8.3 specifies:
- apply the six odd-parity tests A to F;
- accept the data when tests A to E all pass;
- correct a single error at the position 16E + 8D + 4C + 2B + A when tests A to E do not all pass and test F fails;
- report "uncorrectable" when tests A to E do not all pass and test F passes;
- report "uncorrectable" when the computed error position is greater than 24.

It shall return:
- the 18 data bits as a number, with data bit D1 as the least significant bit;
- the status.
*Verification:* Exhaustive test (REQ-TST-003).

**REQ-ECC-005 — Hamming 24/18 encoding.**
A function shall encode an 18-bit value into a triplet using the encoding equations of clause 8.3.
*Verification:* Test that every 18-bit value decodes back to itself.

**REQ-ECC-006 — Bit reversal.**
A function shall reverse the order of the bits within a byte. It is used when converting between broadcast order and the bit order of the Digital Video Broadcasting standard (area INP).
*Verification:* Test over all 256 byte values.

**REQ-ECC-007 — Page checksum.**
A function shall compute the 16-bit cyclic redundancy check of a page as clause 9.6.1 defines it for packet X/27/0:
- a 16-bit shift register initialised to zero, with the input combined with stages 7, 9, 12 and 16;
- the input bytes are 14 to 37 of the header followed by bytes 6 to 45 of packets X/1 to X/25 in order;
- absent packets are treated as spaces throughout;
- each byte is applied from bit 8 down to bit 1.

The decoder may use it to check received pages, and the encoder shall use it to generate packet X/27/0.
*Verification:* Test against a checksum from a broadcast page captured with a known-correct X/27/0.
