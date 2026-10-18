# Credits

neoteletext is written from the teletext standards (ETSI EN 300 706 and the documents cited in
REQUIREMENTS.md). These outside works are part of it, or shaped it. Each is credited for what it
contributes, with its licence and how neoteletext uses it.

## Bundled

**Bedstead** — Ben Harris and contributors, <https://bjh21.me.uk/bedstead/>
- *What it provides:* neoteletext's one bundled font, the default for every page and the font of
  the receiver's own drawing (decision E.72, REQ-FNT-001). `third_party/bedstead/` holds the source
  (version 3.261) and the glyphs exported from it.
- *Licence:* Bedstead's own work is dedicated to the public domain under CC0 1.0. Many glyphs
  reproduce the typeface of the **Mullard SAA5050** character generator, designed by Mullard; their
  use as a typeface rests on section 55 of the UK Copyright, Designs and Patents Act 1988. The full
  analysis, and Debian's view of it, is in `third_party/bedstead/PROVENANCE.md`.
- *Effect on neoteletext's licence:* none. CC0 places no conditions on the code that uses it.

## Used at test time only (never copied, never linked into a release)

These programs are run as separate programs or test oracles, in containers with no network access
(REQ-TST-001). Their code and test files are never copied into this repository.

**zvbi** — Michael H. Schimek and contributors, <https://github.com/zapping-vbi/zvbi>
- *What it provides:* the independent oracle at Levels 2.5 and 3.5 (REQ-TST-023): `tests/oracles/`
  decodes the same recordings with zvbi and compares the result cell by cell. Where the standard is
  silent, neoteletext follows zvbi's observable behaviour, and records the places where it does
  not (REQUIREMENTS.md, the decision on the oracle's known differences).
- *Licence:* libzvbi is LGPL-2+ and the zvbi programs GPL-2+. The oracle programs in
  `tests/oracles/` are neoteletext's own; they call the installed libzvbi at test time only, so the
  receiver neoteletext ships contains no zvbi code.

**vhs-teletext** — Alistair Buxton, <https://github.com/ali1234/vhs-teletext>
**vbit2** — Peter Kwan and contributors, <https://github.com/peterkvt80/vbit2>
**QTeletextMaker** — gkthemac, <https://github.com/gkthemac/QTeletextMaker>
- *What they provide:* independent test oracles and fuzzing inputs (REQ-TST-001).
- *Licence:* vhs-teletext and QTeletextMaker are GPL-3.0; vbit2's repository states no licence
  identifier. All three are run as external programs only.

## Formats and services neoteletext reads

**zxnet** — <https://zxnet.co.uk/teletext/viewer/>
- *What it provides:* the live teletext feeds neoteletext can show (REQ-INP-022), and the page
  formats its editor exports. neoteletext implements the feed protocol and the formats from their
  observed behaviour; no zxnet code is used.

**edit.tf** — <https://edit.tf/>
- *What it provides:* the URL encoding of teletext pages that neoteletext reads and writes.
  Implemented from the encoding itself; no edit.tf code is used.

## Fonts loaded at run time

Fonts other than Bedstead are not part of neoteletext: the user loads them from font libraries at
run time (area FNT), and each keeps its own licence and record of where it came from. One open
font that draws every teletext character set is the **libzvbi teletext font** (Michael H.
Schimek, LGPL-2+). It must stay a loadable font, never a bundled one: bundling it would bring the
LGPL's conditions into the release.
