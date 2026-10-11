# Bedstead glyphs: temporary development glyph source

**Status:** replaced. The project's own glyphs (decision E.62) are the default; the core builds these only with the non-default `bedstead` feature. The export stays in the repository because `cargo xtask glyph-check` compares every original glyph with it, to show that none was copied (REQ-RND-020). Whether to keep it, or move the reference outside the repository, is the owner's decision.

## What is here

| File | What it is |
|---|---|
| `bedstead.c` | Bedstead 3.261 source. It is the C program that holds all of Bedstead's glyph bitmaps and generates the outline font. The copy was downloaded on 1 October 2026 from the Debian source archive: `https://sources.debian.org/data/non-free/b/bedstead/3.261-1/bedstead.c`. The upstream home is `https://bjh21.me.uk/bedstead/`. |
| `debian-copyright.txt` | Debian's copyright and licence analysis of Bedstead, downloaded from the same place. |
| `tools/dump_glyphs.c` | Our small program. It compiles Bedstead's glyph table and exports it as plain text. |
| `bedstead-glyphs.txt` | The exported glyphs (see the format below). This is the file the decoder uses. |

## Format of `bedstead-glyphs.txt`

There is one glyph per line:

```
U+XXXX name r0 r1 r2 r3 r4 r5 r6 r7 r8
```

- `U+XXXX` is the Unicode code point.
- `name` is Bedstead's glyph name.
- `r0` to `r8` are the nine rows of the 5-dot by 9-line glyph, top to bottom, in hexadecimal. Bit 4 (value 0x10) is the leftmost dot and bit 0 (value 0x01) is the rightmost.

This is exactly the SAA5050 glyph geometry the renderer expects (REQ-RND-010).

The export contains:
- 2066 glyphs, one per code point;
- aliases, resolved to the glyph they point to.

The export leaves out:
- block mosaics, because the renderer computes them (REQ-CHR-005);
- glyphs with no code point;
- glyphs that exist only as variation-selector forms.

To regenerate it:

```sh
cc -w -o tools/dump_glyphs tools/dump_glyphs.c && ./tools/dump_glyphs | sort -u -k1,1 > bedstead-glyphs.txt
```

## Licence, and why this is only temporary

- **Bedstead's own work is public domain.** Ben Harris and the other authors dedicated their code and the glyphs they designed to the public domain under CC0 1.0.
- **Many glyphs copy the SAA5050 designs.** They reproduce the typeface of the Mullard SAA5050 series, taken from the code tables in its datasheet, and copyright in that typeface is still owned by Mullard's corporate successors.
- **Why that use is allowed.** Under section 55 of the United Kingdom Copyright, Designs and Patents Act 1988, once 25 years have passed since articles for producing material in a typeface were first marketed, making and using such articles no longer infringes. The SAA5050 was marketed in 1982 or earlier.
- **Debian's view.** Debian puts Bedstead in "non-free" for this reason. In its words, "there are no restrictions on the distribution and use of the typefaces as typefaces", but use of the SAA5050 designs "other than by making and using typefaces might infringe that copyright".

Using these glyphs to display text in our decoder is use as a typeface, which section 55 permits. It still conflicts with the project's aim of containing no designs copied from a character generator. That is why the glyphs are a stopgap, to be replaced by original drawings.

## Known gaps

These code points needed by the teletext character sets are not in the export:
- **Horizontal bar, U+2015.** Use the em dash U+2014, which is Bedstead's form of the SAA5050 long dash.
- **Trade mark sign, U+2122.**
- **Turkish lira sign, U+20BA.**
- **Letters that should be composed from a base letter and a mark** (decision E.26):
  - A with breve, U+0102 and U+0103;
  - G with breve, U+011E and U+011F.
- **All Arabic letters.**
- **Most of the G3 smoothed-mosaic shapes** in the Symbols for Legacy Computing block. The four G3 characters used at Level 1.5 are present: U+2500, U+2190, U+2192 and U+2191.

Missing glyphs are drawn as the visible placeholder required by REQ-CHR-004.
