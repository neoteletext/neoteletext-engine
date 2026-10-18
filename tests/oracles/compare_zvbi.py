#!/usr/bin/env python3
"""Compares `neottx text FILE all` with the zvbi oracle on the same T42 files
(REQ-TST-021). Test code; run on a host with zvbi, see run_zvbi.sh.

What is compared, and why only this:
- the set of page numbers each decoder found;
- for each page and sub-page both have, the letters and digits of rows 1
  to 24. Row 0 is left out because columns 0 to 7 belong to each receiver
  and the clock rolls. zvbi prints a double-height row's characters again
  in the row below, where neottx leaves the lower row empty (REQ-L1-008);
  such rows are counted apart, not as differences. Mosaics, spaces and punctuation are left out
  because the two print mosaics differently (neottx as Unicode block mosaics,
  zvbi as approximations), so only text decoding is compared: parity,
  Hamming, character sets and national options.

Usage: compare_zvbi.py TTX_OUTPUT ZVBI_OUTPUT. Prints a report; exit status
0 unless an input cannot be read.
"""

import sys
import unicodedata


def pages(path):
    """Maps (page, sub-code) to its rows, from `=== MPP:SSSS` sections."""
    result = {}
    key = None
    with open(path, encoding="utf-8", errors="replace") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if line.startswith("=== ") and ":" in line:
                page, sub = line[4:].split(":", 1)
                key = (page.strip().upper(), int(sub.strip(), 16))
                result[key] = []
            elif key is not None:
                result[key].append(line)
    return result


def text_of(row):
    """The letters and digits of a row, in order."""
    return "".join(c for c in row if unicodedata.category(c)[0] in "LN" and not 0xE000 <= ord(c) <= 0xF8FF)


def main():
    ours, theirs = pages(sys.argv[1]), pages(sys.argv[2])
    our_numbers = {page for page, _ in ours}
    their_numbers = {page for page, _ in theirs}
    print(f"pages: neottx {len(our_numbers)}, zvbi {len(their_numbers)}, both {len(our_numbers & their_numbers)}")
    print(f"  only neottx: {' '.join(sorted(our_numbers - their_numbers)) or '-'}")
    print(f"  only zvbi: {' '.join(sorted(their_numbers - our_numbers)) or '-'}")
    common = sorted(set(ours) & set(theirs))
    rows = same = lower_halves = 0
    differing = []
    for key in common:
        for index in range(1, 25):
            a = text_of(ours[key][index]) if index < len(ours[key]) else ""
            b = text_of(theirs[key][index]) if index < len(theirs[key]) else ""
            if not a and not b:
                continue
            above_ours = text_of(ours[key][index - 1]) if index - 1 < len(ours[key]) else ""
            above_theirs = text_of(theirs[key][index - 1]) if index - 1 < len(theirs[key]) else ""
            if not a and b and b == above_theirs == above_ours:
                lower_halves += 1
                continue
            rows += 1
            if a == b:
                same += 1
            else:
                differing.append((key, index, a, b))
    share = 100.0 * same / rows if rows else 100.0
    print(f"sub-pages in both: {len(common)}; text rows {rows}, identical {same} ({share:.1f}%);"
          f" double-height lower rows {lower_halves}")
    for (page, sub), index, a, b in differing[:40]:
        print(f"  {page}:{sub:04X} row {index:2}: neottx {a[:34]!r}")
        print(f"  {' ' * len(page)} {' ' * 4}        zvbi {b[:34]!r}")
    if len(differing) > 40:
        print(f"  ... {len(differing) - 40} more rows differ")


if __name__ == "__main__":
    main()
