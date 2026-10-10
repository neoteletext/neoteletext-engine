#!/usr/bin/env python3
"""Compares `ttx text FILE all --cells` with zvbi_cells at Level 2.5 or 3.5,
cell by cell (REQ-TST-023). Test code; run on a host with zvbi, see
run_cells.sh.

What is compared, for rows 1 to 24 of the main page area of each page and
sub-page both decoders have (row 0 belongs to each receiver):
- the character, when the cell is not the covered part of a double-size
  character (the two decoders fill those differently);
- the size code;
- the background index always, and the foreground index when the cell
  shows a character;
- underline, flash and conceal.
Colours are Colour Map indices: zvbi's default RGB values differ from EN
300 706 Table 30 at CLUT 1 entries 5 and 6 (decision E.40).

Pages listed with --skip are left out and named in the report, with the
reason given on the command line; they cover zvbi faults recorded in the
implementation log.

Usage: compare_cells.py TTX_OUTPUT ZVBI_OUTPUT [--skip PAGE,...]
Exit status 0 when every compared cell agrees, 1 otherwise.
"""

import collections
import sys

# zvbi's private-use code points (its lang.c conventions).
ZVBI_MOSAIC_FIRST, ZVBI_MOSAIC_LAST = 0xEE00, 0xEE7F
ZVBI_CONTIGUOUS_BIT = 0x20
ZVBI_G3_FIRST, ZVBI_G3_LAST = 0xEF20, 0xEF7F
ZVBI_DRCS_FIRST, ZVBI_DRCS_LAST = 0xF000, 0xF7FF
ZVBI_DRCS_CHARACTERS = 64
DRCS_SUBTABLES = 16
# zvbi's vbi_size values, by (vertical, horizontal) role in ttx's dump.
ZVBI_SIZES = {
    "NN": 0, "NL": 1, "TN": 2, "TL": 3, "NR": 4, "TR": 4, "BR": 5, "BN": 6, "BL": 7,
}
PRIMARY_SIZES = {0, 1, 2, 3}
COMPARED_FLAGS = set("ufc")
SPACE = 0x20


def zvbi_character(code):
    """zvbi's character code in the dump's notation."""
    if code == SPACE:
        return "S"
    if ZVBI_MOSAIC_FIRST <= code <= ZVBI_MOSAIC_LAST:
        low = code - ZVBI_MOSAIC_FIRST
        pattern = (low & 0x1F) | ((low & 0x40) >> 1)
        if pattern == 0:
            return "S"
        return f"M{pattern:02X}{'c' if low & ZVBI_CONTIGUOUS_BIT else 's'}"
    if ZVBI_G3_FIRST <= code <= ZVBI_G3_LAST:
        return f"G3:{code - 0xEF00:02X}"
    if ZVBI_DRCS_FIRST <= code <= ZVBI_DRCS_LAST:
        table_and_subtable, character = divmod(code - ZVBI_DRCS_FIRST, ZVBI_DRCS_CHARACTERS)
        table, subtable = divmod(table_and_subtable, DRCS_SUBTABLES)
        return f"D{'n' if table else 'g'}{subtable:X}:{character}"
    return f"U+{code:04X}"


def ttx_character(token):
    """ttx's character, with an empty mosaic counted as a space as zvbi does."""
    return "S" if token in ("M00c", "M00s") else token


def read(path, zvbi):
    """Maps (page, sub-code) to rows of (character, fg, bg, size, flags)."""
    pages, key = {}, None
    with open(path, encoding="utf-8", errors="replace") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if line.startswith("=== "):
                page, sub = line[4:].split()[0].split(":")
                key = (page.upper(), int(sub, 16))
                pages[key] = {}
                continue
            if key is None or line.startswith("screen"):
                continue
            number, *cells = line.split(" ")
            row = []
            for cell in cells[:40]:
                character, foreground, background, size, flags = cell.split("/")
                if zvbi:
                    character, size = zvbi_character(int(character, 16)), int(size)
                else:
                    character, size = ttx_character(character), ZVBI_SIZES[size]
                row.append((character, int(foreground), int(background), size, set(flags) & COMPARED_FLAGS))
            pages[key][int(number)] = row
    return pages


def main():
    arguments = sys.argv[1:]
    skip = set()
    if "--skip" in arguments:
        at = arguments.index("--skip")
        skip = {page.upper() for page in arguments[at + 1].split(",")}
        del arguments[at:at + 2]
    ours, theirs = read(arguments[0], False), read(arguments[1], True)
    common = sorted(key for key in set(ours) & set(theirs) if key[0] not in skip)
    print(f"pages: ttx {len(ours)}, zvbi {len(theirs)}, compared {len(common)}, skipped {' '.join(sorted(skip)) or '-'}")
    only_ours = sorted(f"{p}:{s:04X}" for p, s in set(ours) - set(theirs) if p not in skip)
    only_theirs = sorted(f"{p}:{s:04X}" for p, s in set(theirs) - set(ours) if p not in skip)
    print(f"  only ttx: {' '.join(only_ours) or '-'}")
    print(f"  only zvbi: {' '.join(only_theirs) or '-'}")
    compared = 0
    differences = collections.Counter()
    examples = []
    for key in common:
        for number in range(1, 25):
            our_row, their_row = ours[key].get(number, []), theirs[key].get(number, [])
            for column, (a, b) in enumerate(zip(our_row, their_row)):
                compared += 1
                fields = []
                if a[3] != b[3]:
                    fields.append("size")
                if b[3] in PRIMARY_SIZES and a[0] != b[0]:
                    fields.append("character")
                if a[2] != b[2]:
                    fields.append("background")
                if a[0] != "S" and b[3] in PRIMARY_SIZES and a[1] != b[1]:
                    fields.append("foreground")
                if a[4] != b[4]:
                    fields.append("flags")
                for field in fields:
                    differences[field] += 1
                if fields and len(examples) < 60:
                    examples.append((key, number, column, fields, a, b))
    print(f"cells compared: {compared}; differences by field: {dict(differences) or 'none'}")
    for (page, sub), number, column, fields, a, b in examples:
        print(f"  {page}:{sub:04X} r{number:02} c{column:02} {','.join(fields)}: ttx {a[:4]} {''.join(sorted(a[4]))}"
              f" zvbi {b[:4]} {''.join(sorted(b[4]))}")
    sys.exit(0 if not differences else 1)


if __name__ == "__main__":
    main()
