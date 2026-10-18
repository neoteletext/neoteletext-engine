# Noto-derived fonts

**Status:** the fonts in this folder are the only glyph data in the repository besides Bedstead (decision E.77, REQ-FNT-012). They are loaded at run time like any loaded font (REQ-FNT-005 to REQ-FNT-008) and never compiled into a crate. Bedstead remains the one bundled font (REQ-FNT-001).

## What is here

| File | What it is |
|---|---|
| `convert.py` | The converter. It shapes every character with HarfBuzz and draws it with FreeType in monochrome at the weight, hinting and metrics of its script, then places it in the 12 × 20 cell. Its dependencies are pinned in its inline metadata. |
| `settings.txt` | Each script's sources, weight, hinting and metrics, and each output font's characters. The values were measured in the prototypes of October 2026 (`docs/spec/design/languages.md` section 5). |
| `sources.txt` | The Noto source fonts: their addresses in the google/fonts repository, pinned to commit `47d54ca1ebb14fd864695f1d39dc734f39017c33` (16 October 2026), and their SHA-256. The converter downloads them to a cache outside the repository and refuses any file whose checksum differs. The sources are never committed. |
| `*.bdf` | The converted fonts, 12 × 20, in the form of REQ-FNT-002. |
| `fonts.txt` | The folder's index as a font library (REQ-FNT-004), with each font's conformance (REQ-FNT-011). |
| `OFL.txt` | The SIL Open Font License 1.1, with the copyright line of every source. |

## Licence

Noto is licensed under the SIL Open Font License 1.1, which `OFL.txt` reproduces with each source's copyright line. The licence declares no Reserved Font Name, so the converted fonts may keep "Noto" in their names. A converted font is a Modified Version under the licence and is distributed under it.

## How the fonts are made

```sh
cargo build --profile iterate -p neoteletext-cli
uv run third_party/noto/convert.py --neottx target/iterate/neottx
```

The converter writes every font of `settings.txt`, then `fonts.txt`, then the checksums below. `cargo xtask check-noto`, run by every gate, fails if a committed font differs from its checksum here or if a font and its line do not match.

## Checksums

<!-- checksums: written by convert.py -->
- `noto-teletext.bdf`: `82fe9e2bb463bfc301ba4509c21542fecef7f5a8a27d0c87de0f5a529a7a4a4a`
- `noto-teletext-arabic.bdf`: `17cfa3817914c2a05ffadc1dd55f834356397ac5e9864fc82a0d6337b9c45629`
- `noto-teletext-thai.bdf`: `168f95b7ca735fd3a17bb5625a77ad4bf998838c9c6e6a71e378a925a030d8cb`
<!-- end checksums -->
