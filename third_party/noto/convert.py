# /// script
# requires-python = ">=3.11"
# dependencies = ["freetype-py==2.5.1", "uharfbuzz==0.56.3", "numpy==2.4.6"]
# ///
"""Converts Noto fonts into 12 x 20 BDF fonts for neoteletext (decision
E.77, REQ-FNT-012).

Every glyph is shaped by HarfBuzz and drawn by FreeType in monochrome, at the
weight, hinting and metrics `settings.txt` gives its script, then placed in
the 12 x 20 cell: its baseline on the script's baseline line, its ink centred
across the cell. A glyph that would leave the cell is drawn smaller, first
shorter and then narrower, never clipped. Arabic letters in a joining form
have their joining stroke carried to the cell's edge, so that letters in
adjacent cells meet. Marks set to sit in a row of their own (Thai and Lao,
decision E.80) are drawn at the bottom or top of the cell.

The sources are the Noto fonts of `sources.txt`, pinned to a commit of the
google/fonts repository and checked by SHA-256. They are downloaded to a
cache outside the repository and never committed.

Usage:
  uv run convert.py [--cache DIR] [--required FILE] [--forms FILE] [--neottx PATH] [FONT ...]

FONT names a `[font NAME]` section of settings.txt; without one, every font
is made. Afterwards the folder's `fonts.txt` index (REQ-FNT-004) is written,
with each font's conformance from `neottx font sets` when --neottx is given,
and the checksums in PROVENANCE.md are brought up to date for
`cargo xtask check-noto`.
"""

import argparse
import configparser
import hashlib
import subprocess
import sys
import unicodedata
import urllib.request
from pathlib import Path

import freetype
import numpy as np
import uharfbuzz as hb

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parent.parent

CELL_WIDTH, CELL_HEIGHT = 12, 20
FIXED = 64                      # FreeType's and HarfBuzz's 26.6 fixed point
MARGIN = 40                     # canvas lines and columns around the glyph
DISPLAY_PIXEL_ASPECT = 1.39     # a pixel of the 480 x 500 page on a 4:3 screen
ZWJ = "‍"
FLAGS = {
    "auto": freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO | freetype.FT_LOAD_FORCE_AUTOHINT,
    "font": freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO,
}
SPACES = {0x20, 0xA0}


# --- sources -------------------------------------------------------------

def sources(cache):
    """The source fonts by id, downloaded and checked."""
    found = {}
    for line in (HERE / "sources.txt").read_text().splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        ident, address, digest = line.split("\t")
        path = cache / f"{ident}.ttf"
        if not path.exists():
            cache.mkdir(parents=True, exist_ok=True)
            with urllib.request.urlopen(address) as response:
                path.write_bytes(response.read())
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != digest:
            sys.exit(f"{ident}: SHA-256 {actual} is not the pinned {digest}")
        found[ident] = path
    return found


# --- drawing -------------------------------------------------------------

class Face:
    """One Noto face at one weight and hinting, shaped by HarfBuzz and drawn
    by FreeType in monochrome."""

    def __init__(self, path, weight, hinting):
        self.face = freetype.Face(str(path))
        self.hb_font = hb.Font(hb.Face(path.read_bytes()))
        self.flags = FLAGS[hinting]
        self.cmap = {code for code, _ in self.face.get_chars()}
        if self.face.has_multiple_masters:
            coordinates, variations = [], {}
            for axis in self.face.get_variation_info().axes:
                value = axis.default
                if axis.tag == "wght" and weight is not None:
                    value = min(max(weight, axis.minimum), axis.maximum)
                coordinates.append(value)
                variations[axis.tag] = value
            self.face.set_var_design_coords(coordinates)
            self.hb_font.set_variations(variations)

    def draw(self, text, across, lines, rtl=False):
        """The shaped text as a boolean canvas, with the baseline at line
        MARGIN and the pen starting at column MARGIN."""
        size = (round(across * FIXED), round(lines * FIXED))
        self.face.set_char_size(size[0], size[1], 72, 72)
        self.hb_font.scale = size
        buffer = hb.Buffer()
        buffer.add_str(text)
        buffer.guess_segment_properties()
        if rtl:
            buffer.direction = "rtl"
        hb.shape(self.hb_font, buffer, {"kern": True, "liga": True})
        width = sum(position.x_advance for position in buffer.glyph_positions) // FIXED + 2 * MARGIN
        canvas = np.zeros((2 * MARGIN, max(width, 1)), dtype=bool)
        pen = MARGIN * FIXED
        for info, position in zip(buffer.glyph_infos, buffer.glyph_positions):
            self.face.load_glyph(info.codepoint, self.flags)
            slot = self.face.glyph
            bitmap = slot.bitmap
            if bitmap.rows and bitmap.width:
                raw = np.frombuffer(bytes(bitmap.buffer), dtype=np.uint8).reshape(bitmap.rows, bitmap.pitch)
                ink = np.unpackbits(raw, axis=1)[:, :bitmap.width].astype(bool)
                x0 = (pen + position.x_offset) // FIXED + slot.bitmap_left
                y0 = MARGIN - position.y_offset // FIXED - slot.bitmap_top
                ys = slice(max(y0, 0), min(y0 + ink.shape[0], canvas.shape[0]))
                xs = slice(max(x0, 0), min(x0 + ink.shape[1], canvas.shape[1]))
                if ys.start < ys.stop and xs.start < xs.stop:
                    canvas[ys, xs] |= ink[ys.start - y0:ys.stop - y0, xs.start - x0:xs.stop - x0]
            pen += position.x_advance
        return canvas


def extent(canvas):
    """(top, bottom, left, right) of the ink, bottom and right exclusive;
    None when there is no ink."""
    rows = np.nonzero(canvas.any(axis=1))[0]
    cols = np.nonzero(canvas.any(axis=0))[0]
    if rows.size == 0:
        return None
    return rows.min(), rows.max() + 1, cols.min(), cols.max() + 1


# --- a script's metrics --------------------------------------------------

class Script:
    """A script's faces and size, from its `[script NAME]` section."""

    def __init__(self, name, section, found):
        self.name = name
        weight = section.getint("weight", fallback=None)
        hinting = section.get("hinting", "auto")
        self.faces = [Face(found[ident], weight, hinting) for ident in section["sources"].split()]
        self.baseline = section.getint("baseline")
        self.rtl = section.getboolean("rtl", fallback=False)
        self.join = section.getboolean("join", fallback=False)
        self.mark_row = section.getboolean("mark_row", fallback=False)
        reference = section["reference"]
        height = section.getint("reference_height")
        width = section.getint("reference_width", fallback=None)
        face = self.faces[0]
        # The em whose reference letters are `height` lines tall, then the
        # width per line that makes them `width` pixels wide (Bedstead's
        # proportions), or the display's true proportions.
        self.lines = 4.0
        while self.lines < 60:
            box = extent(face.draw(reference, self.lines / DISPLAY_PIXEL_ASPECT, self.lines, self.rtl))
            if box and box[1] - box[0] >= height:
                break
            self.lines += 0.25
        self.across = self.lines / DISPLAY_PIXEL_ASPECT
        if width:
            self.across = 2.0
            while self.across < 60:
                widest = max(
                    (lambda box: box[3] - box[2] if box else 0)(extent(face.draw(letter, self.across, self.lines)))
                    for letter in reference
                )
                if widest >= width:
                    break
                self.across += 0.25

    def face_for(self, character):
        code = ord(character)
        return next((face for face in self.faces if code in face.cmap), None)


# --- one glyph -----------------------------------------------------------

def joining(character):
    """(joins on the right, joins on the left) of an Arabic presentation
    form, by its Unicode name."""
    name = unicodedata.name(character, "")
    if "MEDIAL FORM" in name:
        return True, True
    if "INITIAL FORM" in name:
        return False, True
    if "FINAL FORM" in name:
        return True, False
    return False, False


def mark_position(character):
    """'above', 'below' or None: where a non-spacing mark sits (Thai and
    Lao), by its Unicode category and name."""
    if not all(unicodedata.category(c) == "Mn" for c in character):
        return None
    names = " ".join(unicodedata.name(c, "") for c in character)
    if "SARA U" in names or "SARA UU" in names or "PHINTHU" in names or "VOWEL SIGN U" in names \
            or "VOWEL SIGN UU" in names or "SIGN PALI VIRAMA" in names or "SEMIVOWEL SIGN LO" in names:
        return "below"
    return "above"


def place(canvas, baseline, centre=True):
    """The canvas's ink in the 12 x 20 cell, baseline on line `baseline`,
    centred across; None when the ink does not fit."""
    box = extent(canvas)
    if box is None:
        return np.zeros((CELL_HEIGHT, CELL_WIDTH), dtype=bool)
    top, bottom, left, right = box
    width = right - left
    first = top - MARGIN + baseline
    last = bottom - MARGIN + baseline
    if width > CELL_WIDTH or first < 0 or last > CELL_HEIGHT:
        return None
    cell = np.zeros((CELL_HEIGHT, CELL_WIDTH), dtype=bool)
    x = (CELL_WIDTH - width) // 2 if centre else 0
    cell[first:last, x:x + width] = canvas[top:bottom, left:right]
    return cell


def read_forms(path):
    """The joined forms the engine draws (REQ-CHR-015): code point to
    (letter, form), from the core's `forms.txt`."""
    found = {}
    if not path.exists():
        return found
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("U+"):
            point, letters, form = line.split()[:3]
            text = "".join(chr(int(part.strip("+"), 16)) for part in letters.split("U+") if part.strip("+"))
            found[chr(int(point[2:], 16))] = (text, form)
    return found


# The shaping context of each joined form: zero-width joiners on the sides
# that join, in logical order (the next letter follows).
FORM_TEXT = {"initial": "{}" + ZWJ, "medial": ZWJ + "{}" + ZWJ, "final": ZWJ + "{}", "sequence": "{}"}
# (joins on the right, joins on the left) of each form, right to left.
FORM_JOINS = {"initial": (False, True), "medial": (True, True), "final": (True, False)}


def draw_glyph(script, character, forms=None):
    """The 12 x 20 glyph of a character, or None when no face has it. A
    joined form of `forms` is drawn by shaping its letter between joiners."""
    if ord(character[0]) in SPACES:
        return np.zeros((CELL_HEIGHT, CELL_WIDTH), dtype=bool)
    form = (forms or {}).get(character)
    face = script.face_for(form[0][0] if form else character[0])
    if face is None:
        return None
    text = FORM_TEXT[form[1]].format(form[0]) if form else character
    position = mark_position(text) if script.mark_row else None
    if position:
        return mark_glyph(script, face, text, position)
    lines, across = script.lines, script.across
    for _ in range(80):
        canvas = face.draw(text, across, lines, script.rtl)
        cell = place(canvas, script.baseline)
        if cell is not None:
            break
        box = extent(canvas)
        too_wide = box is not None and box[3] - box[2] > CELL_WIDTH
        if too_wide:
            across *= 0.94
        else:
            lines *= 0.96
    else:
        return None
    if script.join:
        right, left = FORM_JOINS.get(form[1], (False, False)) if form else joining(character)
        if right or left:
            cell = joined(cell, script, face, right, left)
    return cell


def mark_glyph(script, face, marks, position):
    """A Thai or Lao mark, or two marks, as the Philips SAA5246AP/S draws
    them (decision E.80): shaped on a consonant, the consonant taken away,
    and the marks moved to the bottom of the cell (above a letter, for the
    letter in the row below) or the top (below a letter, for the row
    above), keeping their place across the consonant, which sits in the
    middle of its own cell."""
    consonant = "ก" if script.name == "thai" else "ກ"
    lines, across = script.lines, script.across
    for _ in range(80):
        canvas = face.draw(consonant + marks, across, lines, script.rtl)
        drawn = face.draw(consonant, across, lines, script.rtl)
        # The consonant alone may have a different advance: compare the
        # canvases over their common part.
        base = np.zeros_like(canvas)
        rows, columns = min(canvas.shape[0], drawn.shape[0]), min(canvas.shape[1], drawn.shape[1])
        base[:rows, :columns] = drawn[:rows, :columns]
        ink, letter = extent(canvas & ~base), extent(base)
        if ink is None or letter is None:
            return np.zeros((CELL_HEIGHT, CELL_WIDTH), dtype=bool)
        top, bottom, left, right = ink
        height, width = bottom - top, right - left
        if height <= CELL_HEIGHT and width <= CELL_WIDTH:
            break
        across *= 0.94
        lines *= 0.96
    else:
        return None
    letter_width = letter[3] - letter[2]
    x = (CELL_WIDTH - letter_width) // 2 + (left - letter[2])
    x = min(max(x, 0), CELL_WIDTH - width)
    y = CELL_HEIGHT - height if position == "above" else 0
    cell = np.zeros((CELL_HEIGHT, CELL_WIDTH), dtype=bool)
    cell[y:y + height, x:x + width] = (canvas & ~base)[top:bottom, left:right]
    return cell


def joined(cell, script, face, right, left):
    """An Arabic letter's joining stroke carried to the cell's right or left
    edge, at the rows of the tatweel."""
    tatweel = place(face.draw("ـ", script.across, script.lines, script.rtl), script.baseline)
    stroke_rows = np.nonzero(tatweel.any(axis=1))[0] if tatweel is not None else []
    out = cell.copy()
    columns = np.nonzero(cell.any(axis=0))[0]
    if columns.size == 0:
        return out
    for row in stroke_rows:
        inked = np.nonzero(cell[row])[0]
        if right:
            out[row, (inked.max() if inked.size else columns.max()):] = True
        if left:
            out[row, :(inked.min() if inked.size else columns.min()) + 1] = True
    return out


# --- output --------------------------------------------------------------

def write_bdf(path, family, copyright_text, glyphs):
    """A BDF 2.1 font in the 12 x 20 form of REQ-FNT-002."""
    lines = [
        "STARTFONT 2.1",
        "COMMENT Converted from Noto by third_party/noto/convert.py (decision E.77).",
        f"FONT -neoteletext-{family.replace(' ', '')}-medium-r-normal--20-200-75-75-c-120-iso10646-1",
        "SIZE 20 75 75",
        "FONTBOUNDINGBOX 12 20 0 0",
        "STARTPROPERTIES 6",
        f'FAMILY_NAME "{family}"',
        f'COPYRIGHT "{copyright_text}"',
        'NOTICE "SIL Open Font License 1.1; see third_party/noto/OFL.txt"',
        'CHARSET_REGISTRY "ISO10646"',
        'CHARSET_ENCODING "1"',
        'SPACING "C"',
        "ENDPROPERTIES",
        f"CHARS {len(glyphs)}",
    ]
    for code in sorted(glyphs):
        lines += [f"STARTCHAR U+{code:04X}", f"ENCODING {code}", "SWIDTH 600 0", "DWIDTH 12 0", "BBX 12 20 0 0", "BITMAP"]
        for row in glyphs[code]:
            value = int("".join("1" if bit else "0" for bit in row), 2) << 4
            lines.append(f"{value:04X}")
        lines.append("ENDCHAR")
    lines.append("ENDFONT")
    path.write_text("\n".join(lines) + "\n")


def characters_of(spec, required, forms):
    """The characters a font draws:
    - `required` reads the engine's list of every character its sets need;
    - `uses:NAME` adds those of the required characters used by set NAME
      (`G0_Persian` for the uses `G0 Persian ...`);
    - `except:NAME` leaves out characters no set but NAME uses;
    - `forms:NAME` adds the joined forms of NAME's letters;
    - `range:XXXX-YYYY` adds a Unicode range; `list:` the characters that
      follow."""
    lines = [line for line in required.read_text(encoding="utf-8").splitlines() if line.startswith("U+")]

    def character(line):
        return chr(int(line.split()[0][2:], 16))

    def uses(line):
        return [use.strip() for use in line.split(" ", 2)[2].split(";")] if line.count(" ") >= 2 else []

    found, excluded = [], set()
    for part in spec.split():
        if part == "required":
            found += [character(line) for line in lines]
        elif part.startswith("uses:"):
            name = part[5:].replace("_", " ") + " "
            found += [character(line) for line in lines if any(use.startswith(name) for use in uses(line))]
        elif part.startswith("except:"):
            names = [name.replace("_", " ") + " " for name in part[7:].split(",")]
            excluded |= {character(line) for line in lines
                         if all(any(use.startswith(name) for name in names) for use in uses(line))}
        elif part.startswith("forms:"):
            name = part[6:].replace("_", " ") + " "
            letters = {character(line) for line in lines if any(use.startswith(name) for use in uses(line))}
            found += [point for point, (letter, _) in forms.items() if letter in letters]
        elif part.startswith("range:"):
            first, last = (int(value, 16) for value in part[6:].split("-"))
            found += [chr(code) for code in range(first, last + 1) if unicodedata.name(chr(code), "")]
        elif part.startswith("list:"):
            found += list(part[5:])
    return [character for character in dict.fromkeys(found) if character not in excluded]


def script_of(character, scripts, forms=None):
    """The script section that draws a character, by its Unicode name; a
    joined form by its letter's."""
    if character in (forms or {}):
        character = forms[character][0]
    name = unicodedata.name(character[0], "")
    for script in scripts:
        script_name = script.name.upper().replace("_", " ")
        if script.name != "latin" and (name.startswith(script_name) or name.startswith("EXTENDED " + script_name)):
            return script
    return next(script for script in scripts if script.name == "latin")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--cache", type=Path, default=Path.home() / ".cache" / "neoteletext-noto")
    parser.add_argument("--required", type=Path,
                        default=REPOSITORY / "crates" / "neoteletext-core" / "src" / "glyphs" / "required.txt")
    parser.add_argument("--forms", type=Path,
                        default=REPOSITORY / "crates" / "neoteletext-core" / "src" / "glyphs" / "forms.txt")
    parser.add_argument("--neottx", type=Path, help="neottx, for the conformance lines of fonts.txt")
    parser.add_argument("fonts", nargs="*")
    args = parser.parse_args()
    settings = configparser.ConfigParser(interpolation=None, inline_comment_prefixes=("#",))
    settings.read(HERE / "settings.txt", encoding="utf-8")
    found = sources(args.cache)
    forms = read_forms(args.forms)
    scripts = {name.split()[1]: Script(name.split()[1], settings[name], found)
               for name in settings.sections() if name.startswith("script ")}
    wanted = args.fonts or [name.split()[1] for name in settings.sections() if name.startswith("font ")]
    for font in wanted:
        section = settings[f"font {font}"]
        uses = [scripts[name] for name in section["scripts"].split()]
        glyphs, missing = {}, []
        for character in characters_of(section["characters"], args.required, forms):
            cell = draw_glyph(script_of(character, uses, forms), character, forms)
            if cell is None:
                missing.append(character)
            else:
                glyphs[ord(character)] = cell
        out = HERE / f"{font}.bdf"
        write_bdf(out, section["family"], section["copyright"], glyphs)
        print(f"{out.name}: {len(glyphs)} glyphs" + (f", {len(missing)} missing: {''.join(missing)}" if missing else ""))
    write_records(settings, args.neottx)


CHECKSUMS_START = "<!-- checksums: written by convert.py -->"
CHECKSUMS_END = "<!-- end checksums -->"


def write_records(settings, neottx):
    """The library index fonts.txt (REQ-FNT-004) and the checksum lines of
    PROVENANCE.md, for every font of settings.txt that exists."""
    index = ["# Noto-derived 12 x 20 fonts (REQ-FNT-012), written by convert.py."]
    checksums = []
    for name in (section.split()[1] for section in settings.sections() if section.startswith("font ")):
        path = HERE / f"{name}.bdf"
        if not path.exists():
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        index.append(f"f\t{path.name}\t{settings[f'font {name}']['family']}\t{digest}")
        if neottx:
            line = subprocess.run([str(neottx), "font", "sets", str(path), "--library-line", path.name],
                                  capture_output=True, text=True, check=True).stdout.strip()
            index.append(line)
        checksums.append(f"- `{path.name}`: `{digest}`")
    (HERE / "fonts.txt").write_text("\n".join(index) + "\n")
    provenance = HERE / "PROVENANCE.md"
    text = provenance.read_text()
    start, end = text.index(CHECKSUMS_START), text.index(CHECKSUMS_END)
    provenance.write_text(text[:start + len(CHECKSUMS_START)] + "\n" + "\n".join(checksums) + "\n" + text[end:])


if __name__ == "__main__":
    main()
