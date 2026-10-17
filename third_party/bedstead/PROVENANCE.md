# Bedstead: the bundled font

**Status:** Bedstead 3.261 is neoteletext's one bundled font (decision E.72, REQ-FNT-001). It is the default font of every page and the only font of the receiver's own drawing. Every other font is a BDF file loaded at run time (area FNT). The only other fonts kept in this repository are the Noto-derived fonts of `third_party/noto/` (decision E.77, REQ-FNT-012), which are not compiled in.

**Export SHA-256:** `5ee6246b7f9bd197f47ce0741ea799a55ac08af775869a08c085803e02a3f14d`, the SHA-256 of `bedstead-glyphs.txt`. `cargo xtask check-bedstead`, run by every gate, fails if the file differs.

## What is here

| File | What it is |
|---|---|
| `bedstead.c` | Bedstead 3.261 source. It is the C program that holds all of Bedstead's glyph bitmaps and generates the outline font. The copy was downloaded on 1 October 2026 from the Debian source archive: `https://sources.debian.org/data/non-free/b/bedstead/3.261-1/bedstead.c`. The upstream home is `https://bjh21.me.uk/bedstead/`. |
| `debian-copyright.txt` | Debian's copyright and licence analysis of Bedstead, downloaded from the same place. |
| `tools/dump_glyphs.c` | Our small program. It compiles Bedstead's glyph table and exports it as plain text. |
| `bedstead-glyphs.txt` | The exported glyphs (see the format below). `cargo xtask glyphs` generates the core's table, `crates/neoteletext-core/src/glyphs/bedstead.rs`, from it. |

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

A regenerated export must have the SHA-256 above; if it changes, the change is reviewed and the checksum updated in the same commit.

## Licence

- **Bedstead's own work is public domain.** Ben Harris and the other authors dedicated their code and the glyphs they designed to the public domain under CC0 1.0.
- **Many glyphs copy the SAA5050 designs.** They reproduce the typeface of the Mullard SAA5050 series, taken from the code tables in its datasheet, and copyright in that typeface is still owned by Mullard's corporate successors.
- **Why that use is allowed.** Under section 55 of the United Kingdom Copyright, Designs and Patents Act 1988, once 25 years have passed since articles for producing material in a typeface were first marketed, making and using such articles no longer infringes. The SAA5050 was marketed in 1982 or earlier.
- **Debian's view.** Debian puts Bedstead in "non-free" for this reason. In its words, "there are no restrictions on the distribution and use of the typefaces as typefaces", but use of the SAA5050 designs "other than by making and using typefaces might infringe that copyright".

Using these glyphs to display text in our decoder is use as a typeface, which section 55 permits. The owner accepts Debian's position (decision E.72).

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

A missing character that can be composed from a base letter and a spacing mark is composed (decision E.26); any other is drawn as the visible placeholder of REQ-CHR-004, unless a loaded font supplies it (REQ-FNT-005). `crates/neoteletext-core/tests/coverage.rs` lists every required character Bedstead lacks.
