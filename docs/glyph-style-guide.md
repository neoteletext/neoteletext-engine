# Glyph style guide

The rules for drawing the project's original teletext glyphs (REQ-RND-028, decision E.62). The rules come from first principles and general typographic practice, never from any existing teletext character generator or font.

## The grid

- Every glyph is **5 dots wide and 9 rows tall**.
  - Columns are numbered 0 (left) to 4 (right).
  - Rows are numbered 0 (top) to 8 (bottom).
- The renderer places the glyph in a cell 6 dots wide and 10 rows tall, so neighbouring glyphs and rows are always separated by at least one blank dot. Use the full width; do not leave a margin inside the 5 columns.
- At 12 by 20 pixels the renderer doubles each dot and smooths diagonals (character rounding). Draw for the 5 by 9 grid; the smoothing is automatic.

## Vertical metrics

| Rows | Use |
|---|---|
| 0 to 6 | Capital letters and digits: cap height is 7 rows, standing on the baseline below row 6 |
| 0 to 1 | Ascenders of lower-case letters (b, d, f, h, k, l, t) and marks above lower-case letters |
| 2 to 6 | Lower-case letters without ascenders: x-height is 5 rows |
| 7 to 8 | Descenders (g, j, p, q, y), marks below the baseline (cedilla, ogonek, comma below), and the tails of punctuation |

- Every letter stands on the baseline: its lowest stroke above the descender rows is in row 6.
- A glyph that puts ink in rows 7 or 8 carries the `descender` tag.

## Strokes

- Strokes are **one dot wide**. Vertical and horizontal strokes are straight runs of dots.
- Diagonals step one column per row, or one column per two rows for steep strokes. Avoid jumps of two columns in one row.
- Curves are made by leaving out the corner dot where two strokes meet. This gives round letters such as O, C and S their roundness at this size.
- No stroke may break: every part of a letter is one connected group of dots, touching side by side or corner to corner. The exceptions are parts that are separate by nature, such as the dot of i and j, the two dots of a colon, and marks above or below a letter. Such glyphs carry the `dots` tag.

## Proportions

- Wide letters (M, W, m, w) use all 5 columns.
- Most letters use all 5 columns, or 4 with the left column empty.
- Narrow letters and punctuation (I, i, l, j, t, f, 1, full stop, comma, brackets) are centred on column 2.
- Round letters (O, C, G, Q, o, c, e) are as wide as straight-sided letters of the same height, so text has an even rhythm.

## Marks

**Lower case:**
- A mark above a lower-case letter sits in rows 0 to 1.
- A mark below sits in rows 7 to 8, joined to the letter or one row below it.

**Capitals:**
- A capital has no room above cap height. A capital with a mark above is drawn **five rows tall, in rows 2 to 6**, with the mark in rows 0 to 1.
- Such a capital keeps the width and the character of its full-height form.

**Spacing marks:**
- The spacing marks (acute, grave, circumflex, tilde, macron, breve, dot above, diaeresis, ring, double acute, caron) are drawn in rows 0 to 1 alone, so that the renderer can overlay them on a lower-case letter.
- The cedilla and ogonek are drawn in rows 7 to 8.

## Distinguishing characters

These must never look alike:
- O and 0: give the zero a distinguishing feature, such as a narrower body or an inner stroke.
- I, l and 1.
- S and 5.
- B and 8.
- Z and 2.
- G and 6.
- The pairs listed in `crates/neoteletext-core/src/glyphs/confusables.txt`.

Within one character set, no two different characters may share a glyph.

## Other scripts

- **Greek and Cyrillic** capitals and lower-case letters follow the Latin metrics above.
  - Letters that are the same shape as a Latin letter are copies of it (`copy` entries), not redrawn. Examples: Greek Α and Cyrillic А; Greek Ο and Cyrillic О.
- **Hebrew** letters sit between rows 1 and 6. Final forms that descend use rows 7 to 8.
- **Arabic** letters in the teletext set have separate initial, medial, final and isolated forms.
  - Joining strokes meet the cell edge at row 6, so that letters in neighbouring cells connect.
  - Arabic is drawn last.

## The file

The glyphs are in `crates/neoteletext-core/src/glyphs/original.txt`.

**A drawn entry** is a header line, then nine rows of five characters: `#` for ink and `.` for none.

```
U+25A1 drawn
#####
#...#
#...#
#...#
#...#
#...#
#####
.....
.....
```

The header gives:
- the code point;
- the kind;
- any tags, from `descender` (ink in rows 7 to 8), `dots` (a dot with no neighbour) and `raised` (a letter that does not stand on row 6, such as ª and º).

**Copies and overlays** have a header line only:

```
U+0410 copy U+0041
U+00E9 overlay U+0065 U+00B4
```

- **`copy`:** uses another entry's glyph.
- **`overlay`:** combines a base glyph and a spacing mark dot by dot, as the renderer does for marks at run time.

Copies and overlays carry their tags after their operands, describing the glyph they produce, for example `U+00E7 overlay U+0063 U+00B8 descender`.

Lines starting with `;` are comments. Each entry is separated from the next by a blank line.
