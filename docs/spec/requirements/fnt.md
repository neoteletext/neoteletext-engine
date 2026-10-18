# F.21 Fonts (FNT)

This area is the receiver's fonts (decision E.72): the bundled Bedstead font, and fonts in one 12 × 10 form loaded when the user, the channel or the page names them. The security analysis is `docs/spec/design/font-security.md`. Mosaics, G3 characters and DRCS characters are not fonts and are not affected.

**REQ-FNT-001 — The bundled font.**
The receiver shall carry one font, Bedstead 3.261, in the 5-dot by 9-line form of REQ-RND-010, drawn with the character rounding of REQ-RND-012. It is the default font of every page and the only font of the receiver's own drawing: row 0's status area, the receiver's row 24 messages and any confirmation. It is the only font compiled into the receiver; the Noto-derived fonts of REQ-FNT-012 are kept in the repository but loaded at run time (decision E.77). Its provenance and licence (CC0 1.0 and section 55 of the United Kingdom Copyright, Designs and Patents Act 1988) shall be recorded in `third_party/bedstead/PROVENANCE.md`, with the SHA-256 of its export, which the gate checks. A character it lacks, and that cannot be composed from a base letter and a spacing mark (decision E.26), shall be drawn as the placeholder of REQ-CHR-004.
*Verification:* Inspection of provenance; the gate's check of the export's SHA-256; a coverage test listing every required character Bedstead lacks.

**REQ-FNT-002 — The font form.**
A font other than the bundled one shall be a file in the Glyph Bitmap Distribution Format (BDF) version 2.1 or 2.2, accepted only when all of these hold:
- the file runs from `STARTFONT` to `ENDFONT`; `COMMENT` lines may appear anywhere;
- `FONTBOUNDINGBOX 12 10 0 0`;
- the properties include `CHARSET_REGISTRY "ISO10646"`, `CHARSET_ENCODING "1"` and `SPACING "C"` or `SPACING "M"`, and at most 64 properties in all;
- `CHARS` gives the number of characters, at most 1,024, and exactly that many follow;
- every character has `ENCODING` a Unicode scalar value, never −1 and never repeated; `DWIDTH 12 0`; `BBX 12 10 0 0`; and a `BITMAP` of exactly ten rows of four hexadecimal digits whose last four bits are zero;
- the file is at most 256 kilobytes, with lines of at most 512 bytes.

`FONT` and `SIZE` in the header, and `SWIDTH` in a character, which BDF 2.1 requires, are accepted once each, checked for their form (`SIZE` and `SWIDTH` as whole numbers) and not used. The structure lines `STARTPROPERTIES`, `ENDPROPERTIES`, `STARTCHAR` and `ENDCHAR` are part of the profile. The values `"ISO10646"`, `"C"` and `"M"` are compared without regard to case; lines may end in LF or CR LF; empty lines may follow `ENDFONT` and nowhere else.

Any other keyword, a malformed number, or any rule broken refuses the whole file, with the line number; nothing of a refused file is used. The font's name is its `FAMILY_NAME` property, and its `COPYRIGHT` and `NOTICE` properties are kept with it.
*Amended 12 October 2026* (issue #200): the owner asked for the profile to be sensible for its purpose. As first written it refused `FONT`, `SIZE` and `SWIDTH`, so it refused every file that follows BDF 2.1, including every font of the owner's collection; they are now accepted and ignored, and every other keyword outside the profile still refuses the file.
*Amended 18 October 2026* (decision E.77): a font may instead be 12 × 20: `FONTBOUNDINGBOX 12 20 0 0`, and in every character `BBX 12 20 0 0` and a `BITMAP` of exactly twenty rows. Every other rule is unchanged, and one font has one height.
*Verification:* Tests of a valid font of each height and of each rule broken; a fuzz target for the reader (REQ-TST-040) with fonts of both heights.

**REQ-FNT-003 — Where fonts come from.**
A font may be loaded from a local file the user names, as `neottx render --font` does, or from an `http` or `https` address, and only when the user, the channel (REQ-FNT-006) or the page (REQ-FNT-007) names it. Nothing else shall cause a font to be fetched. A font shall be fetched by the confined fetcher process of REQ-SEC-018 under the network rules of a channel's files: the local-network protection of REQ-SEC-006, the plain-HTTP marker of REQ-SEC-005, and no downgrade (REQ-SEC-007). A font at an address other than the channel's own origin, named by the channel or the page, is fetched only with its SHA-256, without cookies or `Referer`, and only while the viewer allows fonts from other sites (REQ-WEB-014).

Fonts and library indexes shall be read only in a confined process: the fetcher, or the decoder, of REQ-SEC-018. The process that draws the page shall never parse a font. It receives the font's bytes, whose SHA-256 it computes itself, and a glyph table of at most 1,024 entries, each a code point and ten or twenty 12-bit rows, all of one height (decision E.77), whose shape it checks (`docs/spec/design/font-security.md` section 3.1). When a font is named with a SHA-256, a file with any other checksum shall be refused.
*Verification:* Tests that a font named by each of the three is fetched, that nothing else fetches one, and that a wrong checksum is refused.

**REQ-FNT-004 — Font libraries.**
A font library is a folder, in the form of a channel's folder (REQ-WEB-023), holding an index named `fonts.txt`. The index has one entry per line, fields separated by one tab, and comments and empty lines as `contents.txt` has (REQ-WEB-025):

| Kind | Fields after the kind |
|---|---|
| `f` | the font: a file name in the library's folder or an address; its title, 1 to 32 characters; its SHA-256 as 64 hexadecimal digits |
| `c` | the conformance of a font listed by an `f` line: the same file name or address; then the character sets it conforms for (REQ-FNT-011), separated by single spaces, each a set's name as REQ-FNT-011 gives it, followed by `*` when the font also meets the SAA5050 cell profile for that set |

The index is at most 64 kilobytes and 1,000 lines; a line of an unknown kind is ignored, a malformed line is ignored and counted. A `c` line for a font no `f` line lists, a second `c` line for a font, or an unknown set name makes that line malformed. A port lists a library's fonts by title for the user to choose; a font is fetched only when chosen. The conformance a `c` line states is used only to list the font before it is loaded; once loaded, the conformance computed from the font is used (decision E.75 item 3).
*Amended 13 October 2026* (decision E.75, issue #213): the `c` line.
*Verification:* Tests of a library with fonts in its folder and elsewhere, of each fault, of `c` lines and their faults, and that listing a library fetches no font.

**REQ-FNT-005 — Which font draws a page.**
The user chooses at most one font for each character set of REQ-FNT-011 (decision E.75 item 6). A character on a page's rows 1 to 24 shall be drawn from the first of: the page's font (REQ-FNT-007), the channel's font (REQ-FNT-006), the user's font for the character's own set, and Bedstead. When the user has set "always use my fonts", the user's font for the character's set, or Bedstead if none is chosen for it, shall be used whatever the channel or page names. A character the font in use lacks shall come from the user's font for its set, then from Bedstead, then the placeholder (REQ-CHR-004). The receiver's own drawing shall always use Bedstead (REQ-FNT-001).
*Amended 13 October 2026* (decision E.75 item 6, issue #216): one user font per character set, replacing the single user font.
*Verification:* Tests of each step of the order, that characters of two sets on one page come from the fonts chosen for their sets, of the user's override, of the fallback for a missing character, and that row 0's status area is drawn in Bedstead whatever font a page names.

**REQ-FNT-006 — A channel's font.**
`channel.json` (REQ-WEB-024) may have a member `font`, an object with `address`, a file name in the channel's folder or an address, and `sha256`, 64 hexadecimal digits; both are required, so a font at another address always carries its checksum (REQ-WEB-014). The font is fetched when the channel opens, after its settings and bundle, and the channel's pages are drawn in the font that would apply without it until it arrives.
*Verification:* Tests of a channel with a font in its folder, one with a font elsewhere, and one whose font fails its checksum.

**REQ-FNT-007 — A page's font.**
A TTI page may name its font with a line `TF,address,sha256`, applying to the page or sub-page of the most recent `PN` line: the address of a font file, absolute or relative to the page's own address, and its SHA-256. Other TTI readers ignore the line as an unknown command. A page has at most one `TF` line; a second, a malformed line or a checksum that is not 64 hexadecimal digits makes the page's font null, with a diagnostic, and the page is still shown. A page from T42 packets or a broadcast cannot name a font. The TTI writer keeps the line, and the page checker (REQ-CLI-006) reports a malformed one as an error.
*Verification:* Reader, writer and checker tests; tests that a page's font draws that page only.

**REQ-FNT-008 — Drawing a loaded font.**
A loaded font's character shall be drawn as its 12 × 10 bitmap with each line repeated in the 12 × 20 cell, and enlarged for double height, double width and double size as a mode 0 DRCS character is (REQ-RND-038). Foreground and background colours, flash, conceal, boxing, underline and the other attributes apply as to any character. The glyphs shall reach a decoder process as a bounded message of the pipe, at most one font per message, checked again on arrival. A font whose every row of every glyph is made of identical pixel pairs, a doubled 5- or 6-dot matrix, shall be drawn with the character rounding of decision E.13 whenever rounding is on, as the bundled glyphs are; any other loaded font shall be drawn without rounding.
*Amended 13 October 2026* (decision E.72 item 8, issue #209): doubled dot matrices are rounded; before, no loaded font was.
*Amended 18 October 2026* (decision E.77): a 12 × 20 font's character is drawn line for line in a normal cell, each line twice in double height and double size, and never rounded.
*Verification:* Golden rasters of a test font at every size; tests that a font message beyond its bounds is refused; a test that Bedstead converted from its 5 × 9 matrices draws every line exactly as the bundled Bedstead, and that a half-dot font is not rounded.

**REQ-FNT-009 — The font cache.** Withdrawn on 18 October 2026: it concerned only the browser's profile directory and settings, and the browser was removed from this repository and is maintained in the separate neoteletext-browser repository.

**REQ-FNT-010 — The font converter.**
`neottx font convert INPUT OUTPUT` shall write a font in the form of REQ-FNT-002 from a BDF of another cell size, offline, only where the conversion is exact and reversible:
- a 5 × 10 or 6 × 10 cell: every pixel made two pixels wide, a 5-pixel cell placed in the right of the 6, so that its blank column is at the left, as Bedstead's is;
- a 5 × 9 or 6 × 9 cell: the same, with one blank line added at the top, where the SAA5050 leaves it;
- a 12 × 20 cell in which every pair of lines, 1 and 2, 3 and 4 and so on, is identical: every second line kept;
- any other 12 × 20 font: written unchanged in the 12 × 20 form of REQ-FNT-002 (decision E.77).

Any other cell, or any glyph larger than its cell, shall be refused, naming the first character that fails. When the input's `CHARSET_REGISTRY` is not ISO10646, the converter shall take the Unicode code point of each `ENCODING` from a mapping file given with `--map`, one `code<TAB>U+XXXX` line each, and refuse a character the map lacks. `FAMILY_NAME`, `COPYRIGHT` and `NOTICE` are carried over. The converter never reads from the network and runs under the sandbox of REQ-SEC-017.
*Amended 13 October 2026* (decision E.72 item 7, issue #206): a 5-pixel cell was placed in the left of the 6, one dot to the left of Bedstead.
*Verification:* Tests of each exact conversion and its reversal, of each refusal, and that the output passes REQ-FNT-002.

**REQ-FNT-011 — A font's conformance for a character set.**
A font *conforms* for a character set S when all of these rules hold (decision E.75). S is one of the 24 sets fonts draw, named in libraries and reports as: `G0-Latin`; `G0-Latin-English`, `G0-Latin-German`, `G0-Latin-SwedishFinnishHungarian`, `G0-Latin-Italian`, `G0-Latin-French`, `G0-Latin-PortugueseSpanish`, `G0-Latin-CzechSlovak`, `G0-Latin-Polish`, `G0-Latin-Turkish`, `G0-Latin-SerbianCroatianSlovenian`, `G0-Latin-Rumanian`, `G0-Latin-Estonian`, `G0-Latin-LettishLithuanian`; `G0-Cyrillic-1`, `G0-Cyrillic-2`, `G0-Cyrillic-3`; `G0-Greek`; `G0-Arabic`; `G0-Hebrew`; `G2-Latin`, `G2-Cyrillic`, `G2-Greek`, `G2-Arabic`.
1. **Format.** The file meets REQ-FNT-002.
2. **Every assigned position.** For each of S's 96 code positions 2/0 to 7/F to which EN 300 706 (Tables 35 to 46) assigns a character, the font has a glyph at the Unicode code point the character set tables of REQ-CHR-010 give for that position; for a national option sub-set, the positions of Table 35 with the sub-set's 13 characters of Table 36 in place of theirs. The non-spacing marks of a G2 set's column 4 count in the spacing forms of decision E.26.
3. **Unassigned positions.** A position to which the standard assigns no character is drawn as the space, and the font's glyph for U+0020 is blank: all its lines zero.
4. **No blank characters.** Every assigned character other than the space has at least one pixel set.
5. **From the glyphs alone.** Conformance depends on the glyph table only, never on the file's names, comments or properties.

The *SAA5050 cell profile* is met for S when, in addition, every glyph of S's assigned characters has its top line blank (one line space between rows) and its leftmost dot column, pixels 11 and 10, blank (one dot space between characters), as the SAA5050 draws them (Part C.3). It is reported, never required.

`neottx font sets FONT` shall read a font under the profile of REQ-FNT-002 and print, for each of the 24 sets, whether the font conforms, whether it meets the profile, and the first rule that fails with the positions that fail it; and with `--library-line NAME`, the `c` line of REQ-FNT-004 for the font under that file name. It never reads from the network and runs under the sandbox of REQ-SEC-017.
*Amended 18 October 2026* (decisions E.78 to E.80): the sets of REQ-CHR-012 are judged by the same rules, under names of the same pattern (for example `G0-Thai`, `G2-Thai`, `G0-Latin-HausaFula`, `G0-Arabic-Urdu`). For a group 1001 set the font must also have every form of every joining letter at the code points of REQ-CHR-015. The SAA5050 cell profile is not reported for 12 × 20 fonts.
*Verification:* Tests of each rule passing and failing, of a national option sub-set, of a G2 set's unassigned positions and marks, of the profile, that two fonts with the same glyphs and different names conform alike, and of `neottx font sets` and its library line.

**REQ-FNT-012 — The Noto-derived fonts.**
The repository shall keep, in `third_party/noto/` (decision E.77):
- **The converted fonts:** one font per script, each 12 × 20 in the form of REQ-FNT-002, with a `fonts.txt` index stating each font's conformance (REQ-FNT-004, REQ-FNT-011), so that the folder is a font library.
- **The converter, `convert.py`:** it uses `freetype-py` and `uharfbuzz` at exact versions pinned in its inline metadata, and runs with `uv run`.
- **`sources.txt`:** each Noto source font's address and SHA-256. The converter downloads the sources to a cache outside the repository and refuses a file whose checksum differs.
- **`settings.txt`:** each script's weight, hinting and metrics (decision E.77 item 7).
- **`OFL.txt` and `PROVENANCE.md`:** the licence with each source's copyright line, and the SHA-256 of every committed font.

`cargo xtask check-noto`, run by every gate, shall fail when a committed font differs from its recorded SHA-256. The fonts shall not be compiled into any crate. `neottx` and `neoteletext-app` shall use the folder as their default font library.
*Verification:* The gate's check; a test that regenerating the fonts from the pinned sources reproduces the committed files; inspection of the licence and provenance.
