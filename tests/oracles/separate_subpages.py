#!/usr/bin/env python3
"""Copies a T42 stream, putting a time-filling header (page FF, EN 300 706
clause 9.3.1.3) before any page header that repeats the page number of the
previous header in the same magazine. Test code for REQ-TST-023.

Why: a page ends at the next header of its magazine (clause 7.1.2), but
zvbi starts the new sub-page without storing the one before when the two
headers carry the same page number, so it never stores, for example,
sub-page 0 of a DRCS page sent just before sub-page 1. The time-filling
header ends the page for zvbi too, and changes nothing for a decoder that
follows the standard.

Usage: separate_subpages.py IN.t42 OUT.t42
"""

import sys

PACKET_BYTES = 42
# Hamming 8/4 codes of the values 0 to 15 (EN 300 706 clause 8.2, Table 7).
HAMMING_8_4 = [0x15, 0x02, 0x49, 0x5E, 0x64, 0x73, 0x38, 0x2F, 0xD0, 0xC7, 0x8C, 0x9B, 0xA1, 0xB6, 0xFD, 0xEA]
DECODE_8_4 = {code: value for value, code in enumerate(HAMMING_8_4)}
# Page units and tens of a time-filling header: F and F.
FILLER_PAGE_NIBBLE = 0xF
HEADER_CONTROL_BYTES = 6
HEADER_TEXT_BYTES = 32
# 2/0 already has odd parity.
SPACE = 0x20


def address(packet):
    """(magazine, row) of a packet, or None when its address is damaged."""
    low, high = DECODE_8_4.get(packet[0]), DECODE_8_4.get(packet[1])
    if low is None or high is None:
        return None
    return low & 7, (low >> 3) | (high << 1)


def main():
    data = open(sys.argv[1], "rb").read()
    out = bytearray()
    last_page = {}
    inserted = 0
    for start in range(0, len(data) - PACKET_BYTES + 1, PACKET_BYTES):
        packet = data[start:start + PACKET_BYTES]
        where = address(packet)
        if where is not None and where[1] == 0:
            magazine = where[0]
            units, tens = DECODE_8_4.get(packet[2]), DECODE_8_4.get(packet[3])
            page = None if units is None or tens is None else (tens << 4) | units
            if page is not None and page != 0xFF and last_page.get(magazine) == page:
                out += packet[:2] + bytes([HAMMING_8_4[FILLER_PAGE_NIBBLE]] * 2)
                out += bytes([HAMMING_8_4[0]] * HEADER_CONTROL_BYTES) + bytes([SPACE] * HEADER_TEXT_BYTES)
                inserted += 1
            last_page[magazine] = page
        out += packet
    open(sys.argv[2], "wb").write(out)
    print(f"separate_subpages: {inserted} time-filling headers inserted", file=sys.stderr)


if __name__ == "__main__":
    main()
