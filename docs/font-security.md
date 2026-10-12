# Font strategy: security analysis

The security analysis of decision E.72 and area FNT of REQUIREMENTS.md: Bedstead as the one bundled font, and every other font a 12 × 10 BDF file loaded when the user, the channel or the page names it. Security overrides every other requirement (decision E.32). Each threat below names the requirement that answers it, and what risk remains.

Written on 12 October 2026, before the fonts are built. It changed two requirements as it was written (sections 3.1 and 3.4).

## 1. What is being protected

| Asset | Why it matters |
|---|---|
| The interface process | It holds everything trusted: the profile, history, bookmarks, the confirmed channels, and the drawing of row 0 and the tuning screen. It is not sandboxed (decision E.69). |
| The trusted drawing | Row 0, the tuning screen, the confirmations and the receiver's own pages tell the viewer where they are and what they are agreeing to (REQ-SEC-025). |
| The viewer's reading of a page | What the screen shows should be what the page says. |
| The viewer's privacy | Which channels they visit, and when. |
| The profile directory | Saved data and the font cache. |
| The project's distribution | What the repository ships, and its legal standing. |

## 2. Trust boundaries

A font crosses four boundaries:

1. **The network to the fetcher.** Any `http` or `https` address, named by the user, a channel or a page (REQ-FNT-003).
2. **The fetcher to the interface.** A font is bytes from a confined process, and those bytes are untrusted.
3. **The profile directory to the interface.** Cached fonts on disk can be changed by anything (REQ-FNT-009).
4. **The interface to a decoder.** The glyphs a decoder draws with (REQ-FNT-008).

## 3. Threats and their answers

### 3.1 A malformed font attacks the code that reads it

A crafted BDF file aims at the reader: huge numbers, endless lines, millions of characters, nested properties, malformed hexadecimal.

- **Answer:** the BDF profile is small and exact (REQ-FNT-002). Every size is bounded (256 KB, 1,024 characters, 512-byte lines, 64 properties), and anything outside the profile refuses the whole file. The reader is Rust with no unsafe code (REQ-SEC-015), panics are denied, and it is a fuzz target (REQ-TST-040).
- **Changed by this analysis:** the requirements first had the interface process read fonts. The interface is the one process without a sandbox, so a reader fault there would expose everything. **Fonts are now read only in a confined process:** the tab's fetcher, or a font helper under the same confinement (REQ-SEC-026). The interface receives two things: the bytes, whose SHA-256 it computes itself, and a fixed-shape glyph table of at most 1,024 entries of code point and ten 12-bit rows, which it checks for shape. It never parses BDF.
- **Remaining risk:** a fault in the reader is contained by the helper's sandbox: no files, no programs, and on macOS and Linux only the network.

### 3.2 A font lies about the text

A font can draw any character as any shape: "1" as "7", "a" as "o", a blank for a letter, a letter for a blank. A page can then show text that is not what its codes say: a different phone number, a different page number to type, a different price.

- **Answer:** this is the same power DRCS characters already give a page (REQ-DRC-001), and the defence is the same: nothing trusted is ever drawn in a loaded font. Row 0, the tuning screen, the contents view, the receiver's own pages and every confirmation use Bedstead (REQ-FNT-001, REQ-FNT-005). The address a link really leads to is shown in row 0 and the status line in Bedstead (REQ-NAV-015), so a font cannot disguise where a link goes. The status line names the font in use and who chose it, and the user can set "always use my font" (REQ-FNT-005).
- **Remaining risk:** within the page area, a channel can mislead a viewer who reads only the page. That is true of every page's content anyway: a page can simply write a false number in Bedstead. A font adds the ability to make the same codes look different, which matters for copied text and screen readers (section 3.3).

### 3.3 What the screen shows differs from the page's text

A screen reader reads the page's codes (REQ-BRW-008), and copying copies the codes. A font can make the screen show something else, or hide text from sighted viewers that a screen reader still reads.

- **Answer:** the status line names a font chosen by the channel or page, and the user's override removes it.
- **Remaining risk:** accepted, as for DRCS. A future check could flag a font whose glyph for a letter is blank or matches another letter; noted in section 5.

### 3.4 Fonts from other sites track the viewer

A channel or page naming a font at another address makes the browser contact that address. Its host learns the viewer's network address and when they read the channel. A request on a page's behalf to a third party is what REQ-WEB-014 permanently excludes ("content from a third party loaded into a page"), and what the web-native concept refused (`docs/web-native-concept.md` section 12: "The browser makes no requests to other sites on a page's behalf").

- **The conflict:** the owner requires fonts from any address, named by the channel or the page. **Changed by this analysis:** REQ-WEB-014 is amended to allow exactly this one kind of third-party request, under these limits, in place of a silent exception:
  - a channel's or page's font from another address must be named with its SHA-256 (REQ-FNT-006, REQ-FNT-007), so the request fetches one known file and can carry nothing back to the page;
  - the request sends no cookies, no `Referer`, and the one fixed `User-Agent` (REQ-WEB-014);
  - a font named with a checksum is fetched once and then used from the cache, so the host sees the first visit only;
  - a setting, "fonts from other sites", lets the viewer refuse them; the channel's pages then use the fallback of REQ-FNT-005.
- **Remaining risk:** the font's host learns the network address and time of the first visit to a channel using that font. Fonts in the channel's own folder have no such cost.

### 3.5 The cache reveals or leaks between channels

A shared cache lets one channel learn whether another's font is cached (by how fast it loads), and so whether the viewer visited it. A poisoned cache entry could give one channel's font to another.

- **Answer:** fonts are stored by content, named by their SHA-256, so an entry can only ever be the file with that checksum. Fonts named by a channel or page are kept per channel (REQ-FNT-009); a channel can only find its own.
- **Remaining risk:** the fonts the user chose are shared, and reveal only the user's own choices.

### 3.6 A cached font is changed on disk

Anything with access to the profile directory could replace a cached font with a hostile one.

- **Answer:** every load recomputes the SHA-256 and re-reads the font in a confined process (REQ-FNT-009); a file that fails is deleted and fetched again. File names come only from checksums, never from addresses, so no address can choose a path.
- **Remaining risk:** none beyond whatever already has write access to the profile.

### 3.7 A font is changed in transit

A font fetched over plain HTTP can be replaced by anyone on the network path.

- **Answer:** a font named with a SHA-256 cannot be replaced: a changed file fails the check. A font the user names by address alone over plain HTTP carries the plain-HTTP marker (REQ-SEC-005), and HTTPS is never downgraded (REQ-SEC-007).
- **Remaining risk:** a font the user names by a plain `http` address without a checksum can be altered in transit, with the effects of section 3.2 on the pages it draws. The user chose it, and is told it is unencrypted.

### 3.8 Denial of service

A channel names a font that never finishes downloading, is enormous, or changes on every visit; a library lists thousands of fonts.

- **Answer:** fetches have the time limits of REQ-SEC-011 and the size limit of REQ-FNT-002; a library index is at most 64 KB and 1,000 lines; a page names at most one font and a channel one; the cache is at most 4 MB and evicts the least recently used. While a font loads, the page is drawn in the fallback font (REQ-FNT-009), so a slow font never stops a page from showing.
- **Remaining risk:** a channel can make the browser fetch up to 256 KB once per channel. That is less than one page response may be (REQ-WEB-008).

### 3.9 The glyphs reach the decoder

The decoder draws the page, so it needs the font's glyphs.

- **Answer:** the interface sends a glyph table in one bounded pipe message (REQ-FNT-008), checked again by the decoder. The decoder never sees a font file, and a decoder cannot ask for a font: only the interface decides which font draws a page (REQ-FNT-005).
- **Remaining risk:** none beyond the pipe's existing checks (REQ-SEC-026).

### 3.10 The converter reads untrusted fonts

`ttx font convert` reads BDF files of any size and origin, for example from a collection downloaded from the internet.

- **Answer:** it runs offline, under the sandbox of REQ-SEC-017, with the same bounded, strict reading, and refuses anything that is not an exact conversion (REQ-FNT-010). It is a fuzz target.
- **Remaining risk:** contained by the sandbox, as for every `ttx` input.

### 3.11 The bundled font's provenance

The one font the project ships must be exactly what it claims to be.

- **Answer:** Bedstead 3.261 is in `third_party/bedstead/` with its source, its download address and Debian's copyright analysis (`PROVENANCE.md`), and its export is regenerated by a recorded command. REQ-FNT-001 now also requires the export's SHA-256 to be recorded in the provenance record and checked by the gate: `cargo xtask check-bedstead` does it in every gate.
- **Remaining risk:** legal rather than security. Bedstead reproduces the SAA5050 designs under section 55 of the United Kingdom Copyright, Designs and Patents Act 1988; Debian files it as non-free (decision E.72). The owner accepts this.

## 4. Changes made to the requirements by this analysis

1. **REQ-FNT-003 and REQ-FNT-009:** fonts, and library indexes, are read only in a confined process; the interface handles only bytes, checksums and a fixed-shape glyph table.
2. **REQ-WEB-014:** fonts from other addresses are the one permitted third-party request, only with a SHA-256, without cookies or `Referer`, and refusable by the viewer.
3. **REQ-FNT-006 and REQ-FNT-007:** a channel's or page's font at another address must carry its SHA-256 (it already had to).
4. **REQ-BRW-007:** the setting "fonts from other sites".

## 5. Open points

- **A check for deceptive fonts:** flag a loaded font whose glyph for a letter or digit is blank, or identical to another letter's or digit's, before using it. Not required yet; recorded for a decision.
- **Whether user-chosen fonts by plain `http` address should need a checksum,** removing the risk of section 3.7.
- **Legal advice** on section 55 and Debian's position, if the project becomes widely distributed.

## 6. As built (12 October 2026)

- **Readers.** `neoteletext_formats::bdf::read` is the loaded-font reader of REQ-FNT-002 and `bdf::source::read_source` the converter's; both are `no_std`, allocate nothing, refuse a file at its first fault and are fuzz targets (`bdf`, `bdf_convert`). Library indexes are read by `neoteletext_web::fonts`.
- **Where fonts are read (section 3.1).** Only in the one-shot helpers `font-fetch` (the fetcher's confinement) and `font-read` (a decoder's). The interface computes the SHA-256 of the bytes it receives, refuses a font whose SHA-256 differs from the one it was named with, and checks the table's shape with `LoadedFont::new`; `xtask check-io` fails the gate if the interface's crates name a font reader. The pipe's reader checks every table's shape, so a decoder refuses a malformed one too.
- **Third-party requests (section 3.4).** `neoteletext_browser::fonts::may_fetch` decides; every font and index request goes without cookies, and nothing its answer sets is kept, whatever its origin, which is stricter than REQ-FNT-003 asks for fonts at the channel's own origin.
- **The cache (sections 3.5 and 3.6).** Files are named by their SHA-256 alone, written through a temporary file, checked again by the interface and a `font-read` helper on every load, kept per channel or shared, and evicted least recently used first within 4 MB, pinned fonts kept.
- **Not built.** The daily check of a font named by address alone fetches it again unconditionally rather than with a conditional request (REQ-FNT-009); see IMPLEMENTATION-LOG.md.

