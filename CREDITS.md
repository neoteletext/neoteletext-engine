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

## Ported

**libopus 1.5.2** — Xiph.Org Foundation, Skype Limited, Octasic, Jean-Marc Valin, Timothy B.
Terriberry, CSIRO, Gregory Maxwell, Mark Borgerding, Erik de Castro Lopo, Mozilla and Amazon,
<https://opus-codec.org/>
- *What it provides:* the Opus decoder (RFC 6716, RFC 8251) that `neoteletext-opus` is ported from,
  function for function from the C of its fixed-point build (decision E.123): the range decoder,
  SILK, CELT and the Opus layer, so that the port's output is bit-exact with it (REQ-AUD-023).
- *Licence:* BSD-3-Clause. Its copyright notice and licence travel with the ported source
  (`crates/neoteletext-opus/LICENSE-libopus`, and the copyright line of each ported file) and must
  be reproduced in the documentation of any binary distribution. Opus is also covered by
  royalty-free patent licences from Xiph.Org, Broadcom and Microsoft for implementations of the
  specification; they are patent grants, not copyright, and apply to any implementation.
- *Effect on neoteletext's licence:* none beyond keeping the notice. BSD-3-Clause is on the list of
  LICENSING.md section 2; the crate's licence is `MIT AND BSD-3-Clause`.

**opus-decoder 0.1.1** — the opus-decoder authors, <https://github.com/TadeuszWolfGang/Rusopus>
- *What it provides:* the starting point the owner chose for the decoder ("use opus-decoder and
  convert it to integer only"): a pure-Rust Opus decoder whose structure the first version
  followed, and its range decoder and SILK were the first versions of those parts. Its floating
  point could not be made bit-exact with the reference, so both were rewritten as ports of libopus;
  what still resembles it (names, tables, a few lines) is what both take from libopus.
- *Licence:* MIT OR Apache-2.0, the project's own terms.
- *Effect on neoteletext's licence:* none.

## Used at test time only (never copied, never linked into a release)

These programs are run as separate programs or test oracles, in containers with no network access
(REQ-TST-001). Their code and test files are never copied into this repository.

**libopus 1.5.2, fixed-point build** — <https://opus-codec.org/>
- *What it provides:* the oracle of REQ-AUD-023. `tests/oracles/run_opus.sh` downloads its source,
  checks it against its pinned SHA-256 and builds it; its `opus_demo` encodes the test streams and
  `tests/oracles/opus_decode.c`, neoteletext's own, decodes them with it, for a byte-for-byte
  comparison with `neoteletext-opus`.
- *Licence:* BSD-3-Clause; nothing of the build is kept in the repository.

**vbit2** — Peter Kwan and contributors, <https://github.com/peterkvt80/vbit2>
**QTeletextMaker** — gkthemac, <https://github.com/gkthemac/QTeletextMaker>
- *What they provide:* independent test oracles and fuzzing inputs (REQ-TST-001).
- *Licence:* QTeletextMaker is GPL-3.0; vbit2's repository states no licence identifier. Both are
  run as external programs only.

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
run time (area FNT), and each keeps its own licence and record of where it came from.
