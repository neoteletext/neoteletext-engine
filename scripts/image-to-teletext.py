# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy==2.4.6", "pillow==13.0.0"]
# ///
"""Converts a picture into a teletext page of mosaic graphics, using what
the chosen presentation level offers.

    uv run scripts/image-to-teletext.py picture.png page.tti [--level 3.5]
        [--hold-mosaics] [--no-objects] [--t42 page.t42]

The picture is cropped to the shape rows 1 to 24 have on the display
(--display: square on neottx's own output, or 4:3 television) and reduced to
80 x 72 mosaic pixels (sixels), two by three per cell; row 0 is the header.
Sixels are not square and not equal: as neottx draws a cell (decision E.13)
they are 6 dots wide and 6, 8 and 6 lines high, so each sixel is the average
of the area it covers on screen, and its error counts by its height. A
picture of exactly 80 x 72 to 75 is taken as drawn sixel for sixel (pixel
art), losing its top rows.
Row 24 is shown because the page carries X/27/0 Fastext links (to itself),
as Annex H requires.

Each cell shows two colours, a foreground and a background. The encoder
chooses them, cell by cell along each row, with a dynamic programme that
weighs the colour error against what each change costs at the level.

Levels 1 and 1.5 (EN 300 706 clause 12.2): the eight colours of CLUT 0, and
every change is a spacing attribute that takes a cell: Graphics colour
(set-after), New Background and Black Background (set-at). The cell an
attribute takes shows the background, or with --hold-mosaics the last
mosaic of the row (Hold Mosaics, set-at at column 0). Level 1.5 adds only
characters, none of them useful to a picture, so it encodes as Level 1.

Levels 2.5 and 3.5 (clause 12.4, Tables 4, 10, 27 and 30):
- the colours are redefined by X/28/0 format 1 (CLUTs 2 and 3) and, at
  Level 3.5, X/28/4 (CLUTs 0 and 1); CLUT 1 entry 0 stays transparent. At
  Level 2.5 the sixteen colours of CLUTs 0 and 1 are fixed: 16 colours are
  fitted and 31 are used. At Level 3.5 CLUT 0 entries 0 and 7 stay black
  and white for the header, and the other 29 are fitted;
- colour changes are non-spacing X/26 attributes, so every column is
  picture: column 0 holds Graphics colour and an X/26 mosaic over it;
- each row starts on its Full Row Colour, with Black Background Colour
  Substitution set;
- by default the row data is carried in Active objects on a Public Object
  Page, magazine page 1A0, linked by X/27/4, and the page's own X/26 only
  invokes them. With --no-objects it is in the page's own X/26, at most
  208 triplets (16 packets of 13), and the encoder spends them where they
  help most.

The palette is fitted to the two-colour rule: encode, move each fitted
colour to the mean of the pixels drawn in it, rounded to 12-bit, and repeat
until nothing changes. This starts from three reductions of the picture
(median cut, maximum coverage, octree) and keeps the best; after each fit,
a colour split from the worst one is added (into a free slot, or in place
of the least-used colour) while that lowers the error. With --no-objects
the palette is then refitted at the change cost the 208-triplet budget
sets. On the ten test pictures this lowers the error 6 to 8 per cent over
a single median-cut fit, at about four times the time.

Render with `neottx render page.t42 100 --level <level>`. The Level 2.5 and
3.5 colour changes take effect only in a neottx built with the core feature
`nonspacing-attributes`.
"""

import argparse
import subprocess
import sys

import numpy as np
from PIL import Image

# Page geometry (clause 12.2): rows 1 to 24 of 40 cells, two by three
# sixels per cell.
ROWS = 24
COLUMNS = 40
SIXELS_ACROSS = 2
SIXELS_DOWN = 3
SIXELS_PER_CELL = SIXELS_ACROSS * SIXELS_DOWN
WIDTH = COLUMNS * SIXELS_ACROSS
HEIGHT = ROWS * SIXELS_DOWN
# A picture this tall also fits: rows 0 to 24, of which row 0 is dropped.
HEIGHT_WITH_HEADER = (ROWS + 1) * SIXELS_DOWN
PATTERNS = 1 << SIXELS_PER_CELL

# The cell as neottx draws it (decision E.13, from the SAA5050 drawing):
# 12 dots by 20 lines, sixels 6 dots wide and, from top to bottom, 6, 8 and
# 6 lines high. Rows 1 to 24 are 480 by 480 raster pixels, square on
# neottx's own output; a 4:3 television widens them by 16/13 (the core's
# ASPECT_NUMERATOR / ASPECT_DENOMINATOR).
CELL_DOTS = 12
SIXEL_DOTS = CELL_DOTS // SIXELS_ACROSS
SIXEL_LINES = (6, 8, 6)
CELL_LINES = sum(SIXEL_LINES)
RASTER_WIDTH = COLUMNS * CELL_DOTS
RASTER_HEIGHT = ROWS * CELL_LINES
DISPLAY_WIDEN = {"neottx": 1.0, "tv": 16 / 13}

# The sextant bit of each sixel of a cell, top left to bottom right
# (Table 47: bit 7 is 0x40; 0x20 is always set).
SEXTANT_BITS = [0x01, 0x02, 0x04, 0x08, 0x10, 0x40]
SEXTANT_BASE = 0x20

# Level 1 spacing attributes (Table 26 of clause 12.2).
GRAPHICS_COLOUR_BASE = 0x10  # 0x11 to 0x17: Graphics red to white
BLACK_BACKGROUND = 0x1C
NEW_BACKGROUND = 0x1D
HOLD_MOSAICS = 0x1E

# Colour Map (Table 30): 4 CLUTs of 8, index 8 x CLUT + entry; four bits
# per primary.
CLUT_ENTRIES = 8
CLUTS = 4
TRANSPARENT = CLUT_ENTRIES  # CLUT 1 entry 0
BLACK = 0
WHITE = 7  # the foreground every row starts with
LEVEL_1_COLOURS = range(1, CLUT_ENTRIES)  # reachable by Graphics colour codes
COLOUR_LEVELS = 15
LEVEL_STEP = 255 // COLOUR_LEVELS
HALF = 7
CLUT_0 = [(r, g, b) for b in (0, COLOUR_LEVELS) for g in (0, COLOUR_LEVELS) for r in (0, COLOUR_LEVELS)]
CLUT_1 = [(r, g, b) for b in (0, HALF) for g in (0, HALF) for r in (0, HALF)]
CLUT_0.sort(key=lambda c: (c[2] > 0) << 2 | (c[1] > 0) << 1 | (c[0] > 0))
CLUT_1.sort(key=lambda c: (c[2] > 0) << 2 | (c[1] > 0) << 1 | (c[0] > 0))
# Level 2.5 keeps CLUTs 0 and 1; CLUT 1 entry 0 is transparent, not usable.
FIXED_LEVEL_25 = {slot: CLUT_0[slot] for slot in range(CLUT_ENTRIES)} | {
    CLUT_ENTRIES + entry: CLUT_1[entry] for entry in range(1, CLUT_ENTRIES)
}
REDEFINABLE_LEVEL_25 = list(range(2 * CLUT_ENTRIES, CLUTS * CLUT_ENTRIES))
# Level 3.5 keeps CLUT 0 entries 0 and 7 black and white, so that row 0's
# header stays white on black; the other 29 slots are fitted.
FIXED_LEVEL_35 = {BLACK: CLUT_0[BLACK], WHITE: CLUT_0[WHITE]}
REDEFINABLE_LEVEL_35 = [slot for slot in range(CLUTS * CLUT_ENTRIES) if slot not in (TRANSPARENT, BLACK, WHITE)]

# Triplets (clause 12.3.1): address 6 bits, mode 5, data 7.
MODE_SHIFT = 6
DATA_SHIFT = 11
TRIPLET_BITS = 18
TRIPLETS_PER_PACKET = 13
LOCAL_TRIPLETS = 16 * TRIPLETS_PER_PACKET  # X/26/0 to X/26/15
ROW_ADDRESS_ROW_24 = 40  # row addresses 41 to 63 are rows 1 to 23
ROW_ADDRESS_LAST = 63
MODE_FULL_ROW_COLOUR = 0x01  # data bits 6 and 7 zero: this row only
MODE_TERMINATION = 0x1F
MODE_RESERVED_ROW = 0x02  # filler after a Termination Marker
MODE_FOREGROUND = 0x00
MODE_G1 = 0x01
MODE_BACKGROUND = 0x03
MODE_INVOKE_ACTIVE = 0x11
MODE_DEFINE_ACTIVE = 0x15
OBJECT_ADDRESS_BIT = 0x20
OBJECT_SOURCE_SHIFT = 3
SOURCE_PUBLIC = 0b10
USAGE_BOTH_LEVELS = 0b11

# X/28 colour words (Table 4): sixteen 12-bit words from stream bit 28;
# triplet 13 bit 15 is Black Background Colour Substitution.
COLOUR_WORDS_START = 28
COLOUR_WORD_BITS = 12
SUBSTITUTION_BIT = 14
DESIGNATION_PAGE = 0  # X/28/0, CLUTs 2 and 3
DESIGNATION_LEVEL_35 = 4  # X/28/4, CLUTs 0 and 1

# X/27/4 link 1 is the POP (Tables 15 and 16).
POP_MAGAZINE_PAGE = 0xA0
LINK_FUNCTION_POP = 0b01
LINK_VALID_BOTH = 0b11
LINK_VALIDITY_SHIFT = 2
LINK_UNITS_SHIFT = 6
LINK_MARKER_SHIFT = 10
LINK_TENS_SHIFT = 14
LINK_SUBPAGES_SHIFT = 2
ALL_SUBPAGES = 0xFFFF
LINKS_IN_PACKET = 6
DESIGNATION_LINKS = 4

# Object pages (clause 10.5.1): pointers in packets 1 and 2, definitions
# from packet 3 to 25 then X/26/0 to 15, 507 triplets a sub-page.
OBJECT_TRIPLETS = 507
POINTER_UNUSED = 0x1FF
POINTER_BITS = 9
FIRST_DEFINITION_PACKET = 3
DEFINITION_ROW_PACKETS = 23
MAX_SUBPAGES = 16
PAGE_FUNCTION_POP = 3
PAGE_CODING_TRIPLETS = 2

# TTI packed lines (REQ-INP-023): a designation character, then each
# triplet as three six-bit characters from 0x40.
PACKED_BASE = 0x40
PACKED_BITS = 6
ESCAPE = "\x1b"

# With objects, a change costs this much colour error, so that equal
# pictures prefer fewer changes; the object pages hold far more than a
# picture needs.
CHANGE_COST = 1
MAX_ROUNDS = 20
# Splits tried after the first fit, and budget refits in --no-objects;
# each stops early at the first that does not lower the error.
MAX_SPLITS = 8
BUDGET_ROUNDS = 4
STARTING_METHODS = [Image.Quantize.MEDIANCUT, Image.Quantize.MAXCOVERAGE, Image.Quantize.FASTOCTREE]
INFINITY = 1 << 60


def load(path, display="neottx", smooth="area"):
    """The picture as 80 x 72 sixel colours. A picture of exactly 80 x 72
    to 75 is taken as drawn sixel for sixel. Any other is cropped to the
    shape rows 1 to 24 have on the display, scaled to the raster, and each
    sixel is the average of the raster pixels it covers: 6 dots by 6, 8 or
    6 lines."""
    image = Image.open(path).convert("RGB")
    width, height = image.size
    if width == WIDTH and HEIGHT <= height <= HEIGHT_WITH_HEADER:
        return np.asarray(image, dtype=np.int64)[height - HEIGHT :]
    target = RASTER_WIDTH * DISPLAY_WIDEN[display] / RASTER_HEIGHT
    if width / height > target:
        keep = round(height * target)
        image = image.crop(((width - keep) // 2, 0, (width - keep) // 2 + keep, height))
    else:
        keep = round(width / target)
        image = image.crop((0, (height - keep) // 2, width, (height - keep) // 2 + keep))
    method = Image.Resampling.BOX if smooth == "area" else Image.Resampling.LANCZOS
    raster = np.asarray(image.resize((RASTER_WIDTH, RASTER_HEIGHT), method), dtype=np.float64)
    bounds = np.cumsum((0, *SIXEL_LINES))
    out = np.zeros((HEIGHT, WIDTH, 3))
    for row in range(ROWS):
        for band in range(SIXELS_DOWN):
            top = row * CELL_LINES + bounds[band]
            lines = raster[top : top + SIXEL_LINES[band]]
            out[row * SIXELS_DOWN + band] = lines.reshape(SIXEL_LINES[band], WIDTH, SIXEL_DOTS, 3).mean((0, 2))
    return np.round(out).astype(np.int64)


def sixel_weights():
    """Each sixel's share of the screen: its height in lines, so the taller
    middle sixels count for more (6, 8, 6)."""
    per_row = np.array([SIXEL_LINES[y % SIXELS_DOWN] for y in range(HEIGHT)], dtype=np.int64)
    return np.repeat(per_row, WIDTH)


WEIGHTS = sixel_weights()


def to_12_bit(colours):
    return np.clip(np.round(colours / LEVEL_STEP), 0, COLOUR_LEVELS).astype(np.int64) * LEVEL_STEP


def rgb(levels):
    return np.array(levels, dtype=np.int64).reshape(-1, 3) * LEVEL_STEP


def cell_pixel_indices():
    out = np.zeros((ROWS, COLUMNS, SIXELS_PER_CELL), dtype=np.int64)
    for row in range(ROWS):
        for column in range(COLUMNS):
            for k in range(SIXELS_PER_CELL):
                y, x = divmod(k, SIXELS_ACROSS)
                out[row, column, k] = (row * SIXELS_DOWN + y) * WIDTH + column * SIXELS_ACROSS + x
    return out


def sextant_code(pattern):
    return SEXTANT_BASE | sum(bit for k, bit in enumerate(SEXTANT_BITS) if pattern >> k & 1)


def pattern_bits():
    """bits[p, k]: whether sixel k of pattern p is foreground."""
    return np.array([[p >> k & 1 for k in range(SIXELS_PER_CELL)] for p in range(PATTERNS)], dtype=bool)


# ---------------------------------------------------------------------------
# Levels 1 and 1.5: spacing attributes
# ---------------------------------------------------------------------------

ACTION_MOSAIC, ACTION_GRAPHICS, ACTION_NEW_BACKGROUND, ACTION_BLACK_BACKGROUND = range(4)


def encode_level_1(pixels, cells, hold):
    """Rows of (code, pattern shown, foreground, background) per cell."""
    palette = rgb(CLUT_0)
    count = len(palette)
    held_states = PATTERNS if hold else 1
    bits = pattern_bits()[:held_states]
    distance = ((pixels.reshape(-1, 3)[:, None, :] - palette[None, :, :]) ** 2).sum(-1) * WEIGHTS[:, None]
    first_free = 2 if hold else 1
    rows = []
    for row in range(ROWS):
        # Column 0 (and 1 with hold) show the space of the row start, black.
        lead = sum(distance[cells[row, column]][:, BLACK].sum() for column in range(first_free))
        current = np.full((count, count, held_states), INFINITY, dtype=np.int64)
        for colour in LEVEL_1_COLOURS:
            current[colour, BLACK, 0] = lead
        back = []
        for column in range(first_free, COLUMNS):
            d = distance[cells[row, column]]  # sixels x colours
            fore = d.T[:, None, :]  # f, 1, sixels
            back_d = d.T[None, :, :]  # 1, b, sixels
            mosaic_cost = np.minimum(fore, back_d).sum(-1)  # f, b
            mosaic_pattern = ((fore < back_d) * (1 << np.arange(SIXELS_PER_CELL))).sum(-1)
            # shown[h, f, b]: pattern h in foreground f on background b.
            shown = np.where(bits[:, None, None, :], d.T[None, :, None, :], d.T[None, None, :, :]).sum(-1)
            shown = shown.transpose(1, 2, 0)  # f, b, h
            new = np.full_like(current, INFINITY)
            pointer = np.zeros(current.shape + (4,), dtype=np.int64)  # action, f, b, h
            # Mosaic: any held pattern before, this cell's best pattern after.
            best_h = current.min(2)
            from_h = current.argmin(2)
            for f in range(count):
                for b in range(count):
                    h = mosaic_pattern[f, b] if hold else 0
                    value = best_h[f, b] + mosaic_cost[f, b]
                    if value < new[f, b, h]:
                        new[f, b, h] = value
                        pointer[f, b, h] = (ACTION_MOSAIC, f, b, from_h[f, b])
            # Graphics colour c, set-after: shows h in the old colours.
            value = current + shown[..., :held_states]
            best_f = value.min(0)  # b, h
            from_f = value.argmin(0)
            b_index, h_index = np.meshgrid(np.arange(count), np.arange(held_states), indexing="ij")
            graphics = np.stack([np.full_like(b_index, ACTION_GRAPHICS), from_f, b_index, h_index], -1)
            for c in LEVEL_1_COLOURS:
                better = best_f < new[c]
                new[c] = np.where(better, best_f, new[c])
                pointer[c] = np.where(better[..., None], graphics, pointer[c])
            # New Background, set-at: background becomes the foreground.
            for f in range(count):
                value = current[f] + shown[f, f, :held_states][None, :]
                best_b = value.min(0)
                from_b = value.argmin(0)
                better = best_b < new[f, f]
                new[f, f] = np.where(better, best_b, new[f, f])
                moves = np.stack(
                    [np.full(held_states, ACTION_NEW_BACKGROUND), np.full(held_states, f), from_b, np.arange(held_states)], -1
                )
                pointer[f, f] = np.where(better[:, None], moves, pointer[f, f])
            # Black Background, set-at.
            for f in range(count):
                value = current[f] + shown[f, BLACK, :held_states][None, :]
                best_b = value.min(0)
                from_b = value.argmin(0)
                better = best_b < new[f, BLACK]
                new[f, BLACK] = np.where(better, best_b, new[f, BLACK])
                moves = np.stack(
                    [np.full(held_states, ACTION_BLACK_BACKGROUND), np.full(held_states, f), from_b, np.arange(held_states)], -1
                )
                pointer[f, BLACK] = np.where(better[:, None], moves, pointer[f, BLACK])
            back.append((pointer, mosaic_pattern))
            current = new
        state = np.unravel_index(current.argmin(), current.shape)
        cells_out = []
        for pointer, mosaic_pattern in reversed(back):
            f, b, h = (int(v) for v in state)
            action, pf, pb, ph = (int(v) for v in pointer[f, b, h])
            if action == ACTION_MOSAIC:
                cells_out.append((sextant_code(mosaic_pattern[f, b]), int(mosaic_pattern[f, b]), f, b))
            elif action == ACTION_GRAPHICS:
                cells_out.append((GRAPHICS_COLOUR_BASE + f, ph, pf, pb))
            elif action == ACTION_NEW_BACKGROUND:
                cells_out.append((NEW_BACKGROUND, ph, f, f))
            else:
                cells_out.append((BLACK_BACKGROUND, ph, f, BLACK))
            state = (pf, pb, ph)
        f = int(state[0])
        lead = [(HOLD_MOSAICS, 0, WHITE, BLACK)] if hold else []
        lead.append((GRAPHICS_COLOUR_BASE + f, 0, WHITE, BLACK))
        rows.append(lead + list(reversed(cells_out)))
    return rows, palette


# ---------------------------------------------------------------------------
# Levels 2.5 and 3.5: redefined colours and non-spacing attributes
# ---------------------------------------------------------------------------


class Palette:
    """The colours in use and their Colour Map slots. Fixed colours keep
    their slots; fitted ones take the free slots, the seven most used as
    the foreground of column 1 taking CLUT 0 entries 1 to 7 where they are
    free (Graphics colour reaches them without a triplet), the most used of
    them entry 7, the foreground every row starts with."""

    def __init__(self, fixed, fitted, redefinable, use=None):
        self.fixed = fixed
        self.fitted = fitted
        self.colours = np.concatenate([rgb(list(fixed.values())), fitted])
        self.slot = dict(enumerate(fixed))
        count = len(self.colours)
        use = np.zeros(count) if use is None else use
        order = [p for p in np.argsort(-use, kind="stable") if p >= len(fixed)]
        level_1 = [s for s in [WHITE, *LEVEL_1_COLOURS, BLACK] if s in redefinable]
        rest = [s for s in redefinable if s not in level_1]
        for entry, slot in zip(order, list(dict.fromkeys(level_1 + rest))):
            self.slot[int(entry)] = slot
        self.free = {p for p, s in self.slot.items() if s in LEVEL_1_COLOURS}
        self.start = next(p for p, s in self.slot.items() if s == WHITE)

    def with_use(self, use, redefinable):
        return Palette(self.fixed, self.fitted, redefinable, use)


def encode_enhanced(pixels, palette, cells, change_cost):
    """The (foreground, background) of every cell of every row, and whether
    column 0 needs a G1 character, by dynamic programming over the pair."""
    colours = palette.colours
    count = len(colours)
    distance = ((pixels.reshape(-1, 3)[:, None, :] - colours[None, :, :]) ** 2).sum(-1) * WEIGHTS[:, None]
    per_cell = distance[cells]  # rows, columns, sixels, colours
    cost = np.minimum(per_cell[..., :, None], per_cell[..., None, :]).sum(2)
    # Column 0 holds the Graphics colour code: a space in the background,
    # unless an enhancement G1 character is placed there.
    space = per_cell[:, 0].sum(1)
    foreground, background = np.meshgrid(np.arange(count), np.arange(count), indexing="ij")
    free = np.isin(np.arange(count), list(palette.free))
    rows = []
    for row in range(ROWS):
        with_character = cost[row, 0] + change_cost
        start_cost = np.where(np.arange(count) == palette.start, 0, change_cost)[:, None]
        current = np.minimum(with_character, space[row][None, :]) + start_cost
        back = []
        for column in range(1, COLUMNS):
            best_for_b = current.min(0)
            from_f = current.argmin(0)
            best_for_f = current.min(1)
            from_b = current.argmin(1)
            best = current.min()
            best_pair = np.unravel_index(current.argmin(), current.shape)
            if column == 1:
                # The code at column 0 sets any free colour; another
                # foreground costs a triplet even if unchanged.
                set_f = np.where(free, 0, change_cost)[:, None]
                options = np.stack(
                    [best_for_b[None, :] + set_f + 0 * foreground, best + set_f + change_cost + 0 * foreground]
                )
                choice = options.argmin(0)
                previous_f = np.where(choice == 0, from_f[background], best_pair[0])
                previous_b = np.where(choice == 0, background, best_pair[1])
            else:
                options = np.stack(
                    [
                        current,
                        best_for_b[None, :] + change_cost + 0 * foreground,
                        best_for_f[:, None] + change_cost + 0 * foreground,
                        np.full((count, count), best + 2 * change_cost),
                    ]
                )
                choice = options.argmin(0)
                previous_f = np.choose(
                    choice, [foreground, from_f[background], foreground, np.full_like(foreground, best_pair[0])]
                )
                previous_b = np.choose(
                    choice, [background, background, from_b[foreground], np.full_like(background, best_pair[1])]
                )
            back.append((previous_f, previous_b))
            current = options.min(0) + cost[row, column]
        f, b = np.unravel_index(current.argmin(), current.shape)
        states = [(int(f), int(b))]
        for previous_f, previous_b in reversed(back):
            f, b = previous_f[f, b], previous_b[f, b]
            states.append((int(f), int(b)))
        states.reverse()
        rows.append((states, bool(with_character[states[0]] < space[row][states[0][1]])))
    return rows, distance


def drawn(rows, distance, cells):
    """For each pixel, the palette entry it is drawn in."""
    out = np.zeros(len(distance), dtype=np.int64)
    for row, (states, character) in enumerate(rows):
        for column, (f, b) in enumerate(states):
            for pixel in cells[row, column]:
                if column == 0 and not character:
                    out[pixel] = b
                else:
                    out[pixel] = f if distance[pixel, f] < distance[pixel, b] else b
    return out


def column_1_use(rows, count):
    use = np.zeros(count)
    for states, _ in rows:
        use[states[1][0]] += 1
    return use


def triplet(address, mode, data):
    return address | mode << MODE_SHIFT | data << DATA_SHIFT


TERMINATION = triplet(ROW_ADDRESS_LAST, MODE_TERMINATION, 0)
FILLER = triplet(ROW_ADDRESS_ROW_24 + 1, MODE_RESERVED_ROW, 0)


def enhanced_content(rows, palette, distance, cells):
    """The Level 1 rows and each row's triplets."""

    def pattern(row, column):
        f, b = rows[row][0][column]
        return sum(1 << k for k, pixel in enumerate(cells[row, column]) if distance[pixel, f] < distance[pixel, b])

    level_1 = []
    row_triplets = []
    slot = palette.slot
    for row, (states, character) in enumerate(rows):
        page_row = row + 1
        address = ROW_ADDRESS_ROW_24 if page_row == ROWS else ROW_ADDRESS_ROW_24 + page_row
        f0, b0 = states[0]
        out = [triplet(address, MODE_FULL_ROW_COLOUR, slot[b0])]
        if f0 != palette.start:
            out.append(triplet(0, MODE_FOREGROUND, slot[f0]))
        if character:
            out.append(triplet(0, MODE_G1, sextant_code(pattern(row, 0))))
        code = None
        previous_f, previous_b = f0, b0
        for column in range(1, COLUMNS):
            f, b = states[column]
            if column == 1:
                if f in palette.free:
                    code = GRAPHICS_COLOUR_BASE + slot[f]
                else:
                    code = GRAPHICS_COLOUR_BASE + slot[next(iter(palette.free))]
                    out.append(triplet(column, MODE_FOREGROUND, slot[f]))
            elif f != previous_f:
                out.append(triplet(column, MODE_FOREGROUND, slot[f]))
            if b != previous_b:
                out.append(triplet(column, MODE_BACKGROUND, slot[b]))
            previous_f, previous_b = f, b
        row_triplets.append(out)
        level_1.append([code] + [sextant_code(pattern(row, column)) for column in range(1, COLUMNS)])
    return level_1, row_triplets


def fit(pixels, palette, cells, redefinable, change_cost):
    """Alternate encoding and moving each fitted colour to its pixels'
    mean; returns the final palette and encoding."""
    flat = pixels.reshape(-1, 3)
    fixed_count = len(palette.fixed)
    for _ in range(MAX_ROUNDS):
        rows, _ = encode_enhanced(pixels, palette, cells, change_cost)
        palette = palette.with_use(column_1_use(rows, len(palette.colours)), redefinable)
        rows, distance = encode_enhanced(pixels, palette, cells, change_cost)
        who = drawn(rows, distance, cells)
        moved = palette.fitted.copy()
        for index in range(len(moved)):
            mask = who == fixed_count + index
            if mask.any():
                moved[index] = to_12_bit(np.average(flat[mask], axis=0, weights=WEIGHTS[mask]))
        moved = np.unique(moved, axis=0)
        if len(moved) == len(palette.fitted) and (moved == palette.fitted).all():
            return palette, rows, distance
        palette = Palette(palette.fixed, moved, redefinable)
    rows, _ = encode_enhanced(pixels, palette, cells, change_cost)
    palette = palette.with_use(column_1_use(rows, len(palette.colours)), redefinable)
    rows, distance = encode_enhanced(pixels, palette, cells, change_cost)
    return palette, rows, distance


def local_triplet_count(row_triplets):
    return sum(map(len, row_triplets))


def squared_error(rows, distance, cells):
    """The total squared error of an encoding: each pixel against the
    colour it is drawn in."""
    who = drawn(rows, distance, cells)
    return int(distance[np.arange(len(who)), who].sum())


def split_worst(pixels, palette, rows, distance, cells, redefinable, drop_least_used):
    """A palette with one more fitted colour, made from the pixels the
    worst colour draws worst; with `drop_least_used`, the least-used fitted
    colour makes room for it."""
    flat = pixels.reshape(-1, 3)
    who = drawn(rows, distance, cells)
    error = distance[np.arange(len(who)), who]
    per_colour = np.bincount(who, weights=error, minlength=len(palette.colours))
    worst = int(np.argmax(per_colour))
    mine = who == worst
    far = flat[mine][error[mine] >= np.median(error[mine])]
    fitted = palette.fitted
    if drop_least_used:
        use = np.bincount(who, minlength=len(palette.colours))[len(palette.fixed) :]
        fitted = np.delete(fitted, int(np.argmin(use)), axis=0)
    fitted = np.unique(np.vstack([fitted, to_12_bit(far.mean(0))[None, :]]), axis=0)
    return Palette(palette.fixed, fitted, redefinable)


def refine(pixels, palette, cells, redefinable, change_cost):
    """Fits a palette, then keeps adding a colour split from the worst one
    (into a free slot, or in place of the least-used colour when the slots
    are full) while that lowers the error. Colours nothing is drawn in are
    dropped first, so their slots are used."""
    best = fit(pixels, palette, cells, redefinable, change_cost)
    best_error = squared_error(best[1], best[2], cells)
    for _ in range(MAX_SPLITS):
        current, rows, distance = best
        use = np.bincount(drawn(rows, distance, cells), minlength=len(current.colours))[len(current.fixed) :]
        if (use == 0).any():
            current = Palette(current.fixed, current.fitted[use > 0], redefinable)
            rows, distance = encode_enhanced(pixels, current, cells, change_cost)
        full = len(current.fitted) >= len(redefinable)
        trial = fit(
            pixels, split_worst(pixels, current, rows, distance, cells, redefinable, full), cells, redefinable,
            change_cost,
        )
        error = squared_error(trial[1], trial[2], cells)
        if error >= best_error:
            break
        best, best_error = trial, error
    return best, best_error


def best_palette(pixels, starts, cells, fixed, redefinable, change_cost):
    """The best refined palette over several starting palettes."""
    results = [refine(pixels, Palette(fixed, start, redefinable), cells, redefinable, change_cost) for start in starts]
    return min(results, key=lambda result: result[1])


def within_budget(pixels, palette, cells, redefinable):
    """The smallest change cost whose encoding fits X/26, and that
    encoding."""
    low, high = CHANGE_COST, 1 << 24
    best = None
    while low < high:
        middle = (low + high) // 2
        rows, _ = encode_enhanced(pixels, palette, cells, middle)
        trial = palette.with_use(column_1_use(rows, len(palette.colours)), redefinable)
        rows, distance = encode_enhanced(pixels, trial, cells, middle)
        _, row_triplets = enhanced_content(rows, trial, distance, cells)
        if local_triplet_count(row_triplets) <= LOCAL_TRIPLETS:
            high = middle
            best = (trial, rows, distance, middle)
        else:
            low = middle + 1
    return best


def fit_within_page(pixels, palette, cells, redefinable):
    """--no-objects: the palette is refitted at the change cost the budget
    sets, then the cost is found again for the new palette, while the error
    within the budget falls."""
    palette, rows, distance, cost = within_budget(pixels, palette, cells, redefinable)
    best, best_error = (palette, rows, distance), squared_error(rows, distance, cells)
    for _ in range(BUDGET_ROUNDS):
        refitted, _, _ = fit(pixels, palette, cells, redefinable, cost)
        palette, rows, distance, cost = within_budget(pixels, refitted, cells, redefinable)
        error = squared_error(rows, distance, cells)
        if error >= best_error:
            break
        best, best_error = (palette, rows, distance), error
    return best


def objects_of(row_triplets):
    """Whole rows packed into one Active object per sub-page."""
    body = OBJECT_TRIPLETS - 2  # less the definition and the termination
    objects = [[]]
    for row in row_triplets:
        if len(objects[-1]) + len(row) > body:
            objects.append([])
        objects[-1].extend(row)
    if len(objects) > MAX_SUBPAGES:
        sys.exit(f"{len(objects)} object sub-pages needed; at most {MAX_SUBPAGES}")
    return objects


# ---------------------------------------------------------------------------
# The TTI file (REQ-INP-013, REQ-INP-023)
# ---------------------------------------------------------------------------


def packed(designation, values):
    text = chr(PACKED_BASE | designation)
    for value in values:
        for shift in range(0, TRIPLET_BITS, PACKED_BITS):
            text += chr(PACKED_BASE | (value >> shift) & ((1 << PACKED_BITS) - 1))
    return text


def colour_packet(words):
    bits = [0] * (TRIPLETS_PER_PACKET * TRIPLET_BITS)
    for index, (red, green, blue) in enumerate(words):
        word = red | green << 4 | blue << 8
        for k in range(COLOUR_WORD_BITS):
            bits[COLOUR_WORDS_START + index * COLOUR_WORD_BITS + k] = (word >> k) & 1
    bits[(TRIPLETS_PER_PACKET - 1) * TRIPLET_BITS + SUBSTITUTION_BIT] = 1
    return [sum(bits[t * TRIPLET_BITS + k] << k for k in range(TRIPLET_BITS)) for t in range(TRIPLETS_PER_PACKET)]


def padded(values, fill):
    values = list(values)
    while len(values) % TRIPLETS_PER_PACKET:
        values.append(fill)
    return values


def row_lines(level_1):
    lines = []
    for row, codes in enumerate(level_1, start=1):
        text = "".join(ESCAPE + chr(code + PACKED_BASE) if code < SEXTANT_BASE else chr(code) for code in codes)
        lines.append(f"OL,{row},{text}")
    return lines


PAGE_START = ["DE,Picture", "PN,10000", "SC,0000", "PS,8000", "FL,100,100,100,100,100,100"]


def write(path, lines):
    with open(path, "w", encoding="latin-1", newline="\n") as file:
        file.write("\n".join(lines) + "\n")


def enhanced_lines(level, palette, level_1, row_triplets, use_objects):
    colour_map = [(0, 0, 0)] * (CLUTS * CLUT_ENTRIES)
    for entry, slot in palette.slot.items():
        colour_map[slot] = tuple(int(v) // LEVEL_STEP for v in palette.colours[entry])
    lines = list(PAGE_START)
    lines.append("OL,28," + packed(DESIGNATION_PAGE, colour_packet(colour_map[2 * CLUT_ENTRIES :])))
    if level == "3.5":
        lines.append("OL,28," + packed(DESIGNATION_LEVEL_35, colour_packet(colour_map[: 2 * CLUT_ENTRIES])))
    objects = objects_of(row_triplets) if use_objects else []
    if use_objects:
        pop_link = [
            LINK_FUNCTION_POP
            | LINK_VALID_BOTH << LINK_VALIDITY_SHIFT
            | (POP_MAGAZINE_PAGE & 0xF) << LINK_UNITS_SHIFT
            | 1 << LINK_MARKER_SHIFT
            | (POP_MAGAZINE_PAGE >> 4) << LINK_TENS_SHIFT,
            ALL_SUBPAGES << LINK_SUBPAGES_SHIFT,
        ]
        links = [0, 0] + pop_link + [0, 0] * (LINKS_IN_PACKET - 2) + [0]
        lines.append("OL,27," + packed(DESIGNATION_LINKS, links))
        local = [
            triplet(OBJECT_ADDRESS_BIT | SOURCE_PUBLIC << OBJECT_SOURCE_SHIFT, MODE_INVOKE_ACTIVE, subpage)
            for subpage in range(len(objects))
        ]
    else:
        local = [t for row in row_triplets for t in row]
    if len(local) < LOCAL_TRIPLETS:
        local.append(TERMINATION)
    local = padded(local, FILLER)
    for designation in range(len(local) // TRIPLETS_PER_PACKET):
        lines.append("OL,26," + packed(designation, local[designation * TRIPLETS_PER_PACKET :][:TRIPLETS_PER_PACKET]))
    lines += row_lines(level_1)
    unused = POINTER_UNUSED | POINTER_UNUSED << POINTER_BITS
    for subpage, body in enumerate(objects):
        lines += [
            f"PN,1{POP_MAGAZINE_PAGE:02X}{subpage:02d}",
            f"SC,{subpage:04X}",
            "PS,8000",
            f"PF,{PAGE_FUNCTION_POP},{PAGE_CODING_TRIPLETS}",
        ]
        # Object 0, Active: pointer packet 1, triplet 1, even half.
        pointers = [FILLER, 0 | POINTER_UNUSED << POINTER_BITS] + [unused] * (TRIPLETS_PER_PACKET - 2)
        lines.append("OL,1," + packed(1, pointers))
        lines.append("OL,2," + packed(1, [FILLER] + [unused] * (TRIPLETS_PER_PACKET - 1)))
        definition = triplet(OBJECT_ADDRESS_BIT | USAGE_BOTH_LEVELS << OBJECT_SOURCE_SHIFT, MODE_DEFINE_ACTIVE, subpage)
        definitions = padded([definition, *body, TERMINATION], TERMINATION)
        for k in range(len(definitions) // TRIPLETS_PER_PACKET):
            chunk = definitions[k * TRIPLETS_PER_PACKET :][:TRIPLETS_PER_PACKET]
            if k < DEFINITION_ROW_PACKETS:
                lines.append(f"OL,{FIRST_DEFINITION_PACKET + k}," + packed(0, chunk))
            else:
                lines.append("OL,26," + packed(k - DEFINITION_ROW_PACKETS, chunk))
    return lines, objects


# ---------------------------------------------------------------------------


def starting_palettes(pixels, palette_path, count):
    """The palettes the fitting starts from: the given picture's colours, or
    three reductions of the picture by different methods."""
    if palette_path:
        colours = np.unique(to_12_bit(load(palette_path).reshape(-1, 3)), axis=0)
        if len(colours) > count:
            sys.exit(f"{palette_path}: {len(colours)} colours; at most {count} can be redefined at this level")
        return [colours]
    image = Image.fromarray(pixels.astype(np.uint8))
    starts = []
    for method in STARTING_METHODS:
        reduced = image.quantize(colors=count, method=method, dither=Image.Dither.NONE)
        starts.append(np.unique(to_12_bit(np.asarray(reduced.convert("RGB"), dtype=np.int64).reshape(-1, 3)), axis=0))
    return starts


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("picture", help="the picture, any format Pillow reads")
    parser.add_argument("tti", help="the TTI page file to write")
    parser.add_argument("--level", choices=["1", "1.5", "2.5", "3.5"], default="3.5", help="presentation level")
    parser.add_argument(
        "--hold-mosaics", action="store_true", help="Levels 1 and 1.5: attribute cells repeat the last mosaic"
    )
    parser.add_argument(
        "--no-objects",
        action="store_true",
        help="Levels 2.5 and 3.5: keep everything in the page's own X/26, at most 208 triplets",
    )
    parser.add_argument("--palette-from", help="Levels 2.5 and 3.5: start the fitted colours from this picture's")
    parser.add_argument(
        "--display", choices=sorted(DISPLAY_WIDEN), default="neottx",
        help="the shape to fit the picture to: neottx's own square-pixel output, or a 4:3 television",
    )
    parser.add_argument(
        "--smooth", choices=["area", "lanczos"], default="area",
        help="scaling to the raster: area averaging (the least squared error) or Lanczos (crisper)",
    )
    parser.add_argument("--t42", help="also write T42 with `neottx convert`")
    parser.add_argument("--neottx", default="neottx", help="the neottx program, for --t42")
    parser.add_argument("--expected", help="write the sixels the page should show, 80 x 72, to this PNG")
    args = parser.parse_args()
    enhanced = args.level in ("2.5", "3.5")
    if args.hold_mosaics and enhanced:
        parser.error("--hold-mosaics is for Levels 1 and 1.5")
    if (args.no_objects or args.palette_from) and not enhanced:
        parser.error("--no-objects and --palette-from are for Levels 2.5 and 3.5")

    pixels = load(args.picture, args.display, args.smooth)
    cells = cell_pixel_indices()
    flat = pixels.reshape(-1, 3)
    if enhanced:
        fixed = FIXED_LEVEL_25 if args.level == "2.5" else FIXED_LEVEL_35
        redefinable = REDEFINABLE_LEVEL_25 if args.level == "2.5" else REDEFINABLE_LEVEL_35
        starts = starting_palettes(pixels, args.palette_from, len(redefinable))
        (palette, rows, distance), _ = best_palette(pixels, starts, cells, fixed, redefinable, CHANGE_COST)
        if args.no_objects:
            palette, rows, distance = fit_within_page(pixels, palette, cells, redefinable)
        level_1, row_triplets = enhanced_content(rows, palette, distance, cells)
        lines, objects = enhanced_lines(args.level, palette, level_1, row_triplets, not args.no_objects)
        shown = palette.colours[drawn(rows, distance, cells)]
        where = f"in {len(objects)} objects" if objects else "in the page's X/26"
        summary = f"{len(palette.colours)} colours, {local_triplet_count(row_triplets)} triplets {where}"
    else:
        rows, palette = encode_level_1(pixels, cells, args.hold_mosaics)
        lines = PAGE_START + row_lines([[code for code, *_ in cells_of_row] for cells_of_row in rows])
        bits = pattern_bits()
        shown = np.zeros_like(flat)
        for row, cells_of_row in enumerate(rows):
            for column, (_, pattern, f, b) in enumerate(cells_of_row):
                for k, pixel in enumerate(cells[row, column]):
                    shown[pixel] = palette[f] if bits[pattern, k] else palette[b]
        attributes = sum(code < SEXTANT_BASE for cells_of_row in rows for code, *_ in cells_of_row)
        summary = f"8 colours, {attributes} attribute cells"
    write(args.tti, lines)
    # Per raster pixel: each sixel weighted by the lines it covers.
    error = np.average(((flat - shown) ** 2).sum(-1), weights=WEIGHTS)
    print(f"Level {args.level}: {summary}, mean squared error {error:.0f}", file=sys.stderr)
    if args.expected:
        Image.fromarray(shown.reshape(HEIGHT, WIDTH, 3).astype(np.uint8)).save(args.expected)
    if args.t42:
        subprocess.run([args.neottx, "convert", args.tti, "--to", "t42", "-o", args.t42], check=True)


if __name__ == "__main__":
    main()
