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

Normalised before comparing:
- a transparent cell (index 8) in ttx's dump takes its row colour, as zvbi
  does when it builds the page; the renderer shows the same (Table C.1);
- an inverted cell in ttx's dump has its colours swapped, as zvbi stores it;
- the unassigned G3 positions and 5/F (Table 48 notes 1 and 2) are spaces
  in both;
- G2 characters for which the two decoders chose different but equivalent
  Unicode code points (EQUIVALENT below) count as equal;
- a character zvbi gives as U+0000 (one it could not compose) is not
  compared.

KNOWN names a file of pages that differ for a recorded reason: a zvbi
fault, or a decision of REQUIREMENTS.md Part E that zvbi does not follow.
Those pages are still compared and their counts reported, but they do not
fail the run; a page not in the file must agree in every compared cell.

Usage: compare_cells.py TTX_OUTPUT ZVBI_OUTPUT [--known FILE]
Exit status 0 when every page not in FILE agrees, 1 otherwise.
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
# G3 5/F is the background-colour space; 6/E, 6/F, 7/E and 7/F are
# unassigned (Table 48 notes 1 and 2).
G3_SPACES = {"G3:5F", "G3:6E", "G3:6F", "G3:7E", "G3:7F"}
# zvbi's code point -> ttx's, for G2 characters both decoders draw alike.
EQUIVALENT = {
    "U+00A0": "S",  # G2 2/0: no-break space; a space
    "U+00A6": "U+007C",  # broken bar / vertical line
    "U+00D0": "U+0110",  # eth / D with stroke: the same capital
    "U+0251": "U+03B1",  # Latin alpha / Greek alpha
    "U+2014": "U+2015",  # em dash / horizontal bar
    "U+20A0": "U+20AC",  # euro-currency sign / euro sign
    "U+2126": "U+03A9",  # ohm sign / Greek capital omega
    "U+02C9": "U+00AF",  # spacing macron
    "U+02CA": "U+00B4",  # spacing acute
    "U+02CB": "U+0060",  # spacing grave
    "U+02CD": "U+005F",  # spacing low macron
    "U+02CF": "U+00B8",  # spacing cedilla
}
ZVBI_UNKNOWN = "U+0000"
SPACE = 0x20
# Colour Map index 8, CLUT 1 entry 0, is transparent (Table 30).
TRANSPARENT = 8


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
        g3 = f"G3:{code - 0xEF00:02X}"
        return "S" if g3 in G3_SPACES else g3
    if ZVBI_DRCS_FIRST <= code <= ZVBI_DRCS_LAST:
        table_and_subtable, character = divmod(code - ZVBI_DRCS_FIRST, ZVBI_DRCS_CHARACTERS)
        table, subtable = divmod(table_and_subtable, DRCS_SUBTABLES)
        return f"D{'n' if table else 'g'}{subtable:X}:{character}"
    unicode = f"U+{code:04X}"
    return EQUIVALENT.get(unicode, unicode)


def ttx_character(token):
    """ttx's character, with a space character and an empty mosaic counted as
    nothing in the foreground, as zvbi does."""
    return "S" if token in ("M00c", "M00s", f"U+{SPACE:04X}") or token in G3_SPACES else token


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
            row_colour = None
            if "/" in number:
                number, row_colour = number.split("/")
                row_colour = int(row_colour)
            row = []
            for cell in cells[:40]:
                character, foreground, background, size, flags = cell.split("/")
                foreground, background = int(foreground), int(background)
                if zvbi:
                    character, size = zvbi_character(int(character, 16)), int(size)
                else:
                    character, size = ttx_character(character), ZVBI_SIZES[size]
                    # zvbi puts the row colour into a transparent cell when it
                    # builds the page; ttx keeps index 8 and the renderer
                    # shows the row colour there (Table C.1). Same display.
                    if foreground == TRANSPARENT:
                        foreground = row_colour
                    if background == TRANSPARENT:
                        background = row_colour
                    # zvbi swaps the colours of an inverted cell; ttx keeps
                    # the flag and the renderer swaps them (REQ-RND-036).
                    if "x" in flags:
                        foreground, background = background, foreground
                row.append((character, foreground, background, size, set(flags) & COMPARED_FLAGS))
            pages[key][int(number)] = row
    return pages


def known_pages(path):
    """Page -> reason, from lines `PAGE reason`; # starts a comment."""
    known = {}
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.split("#", 1)[0].strip()
            if line:
                page, reason = line.split(None, 1)
                known[page.upper()] = reason
    return known


def main():
    arguments = sys.argv[1:]
    known = {}
    if "--known" in arguments:
        at = arguments.index("--known")
        known = known_pages(arguments[at + 1])
        del arguments[at:at + 2]
    ours, theirs = read(arguments[0], False), read(arguments[1], True)
    common = sorted(set(ours) & set(theirs))
    print(f"pages: ttx {len(ours)}, zvbi {len(theirs)}, compared {len(common)}")
    only_ours = sorted(f"{p}:{s:04X}" for p, s in set(ours) - set(theirs))
    only_theirs = sorted(f"{p}:{s:04X}" for p, s in set(theirs) - set(ours))
    print(f"  only ttx: {' '.join(only_ours) or '-'}")
    print(f"  only zvbi: {' '.join(only_theirs) or '-'}")
    compared = 0
    differences = collections.Counter()
    per_page = collections.Counter()
    examples = []
    for key in common:
        for number in range(1, 25):
            our_row, their_row = ours[key].get(number, []), theirs[key].get(number, [])
            for column, (a, b) in enumerate(zip(our_row, their_row)):
                compared += 1
                fields = []
                if a[3] != b[3]:
                    fields.append("size")
                if b[3] in PRIMARY_SIZES and a[0] != b[0] and b[0] != ZVBI_UNKNOWN:
                    fields.append("character")
                if a[2] != b[2]:
                    fields.append("background")
                if a[0] != "S" and b[3] in PRIMARY_SIZES and a[1] != b[1]:
                    fields.append("foreground")
                if a[4] != b[4]:
                    fields.append("flags")
                if not fields:
                    continue
                per_page[key[0]] += 1
                if key[0] in known:
                    continue
                for field in fields:
                    differences[field] += 1
                if len(examples) < 60:
                    examples.append((key, number, column, fields, a, b))
    agreeing = sorted({page for page, _ in common} - set(per_page))
    print(f"cells compared: {compared}")
    print(f"pages agreeing in every cell ({len(agreeing)}): {' '.join(agreeing) or '-'}")
    print("pages differing for a recorded reason:")
    for page in sorted(set(per_page) & set(known)):
        print(f"  {page} ({per_page[page]} cells): {known[page]}")
    stale = sorted(set(known) - set(per_page) - {p for p, _ in set(ours) ^ set(theirs)})
    if stale:
        print(f"listed as known but now agreeing: {' '.join(stale)}")
    unexplained = sorted(set(per_page) - set(known))
    print(f"pages differing without a recorded reason: {' '.join(f'{p}={per_page[p]}' for p in unexplained) or 'none'}")
    if differences:
        print(f"  by field: {dict(differences)}")
    for (page, sub), number, column, fields, a, b in examples:
        print(f"  {page}:{sub:04X} r{number:02} c{column:02} {','.join(fields)}: ttx {a[:4]} {''.join(sorted(a[4]))}"
              f" zvbi {b[:4]} {''.join(sorted(b[4]))}")
    sys.exit(0 if not unexplained else 1)


if __name__ == "__main__":
    main()
