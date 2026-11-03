# Noto-derived fonts

**Status:** these files are Noto Teletext, neoteletext's one font (DEC-1761, REQ-FNT-001), and the only glyph data in the repository. `cargo xtask glyphs` merges them into `crates/neoteletext-core/src/glyphs/noto.rs`, which is compiled into the core. A font file holds at most 1,024 characters (REQ-FNT-002), so the font is spread over five files, one font together; they agree wherever two hold the same character, and the generator refuses them if they do not. Each file can also be loaded at run time like any other font (REQ-FNT-005 to REQ-FNT-008).

## What is here

| File | What it is |
|---|---|
| `convert.py` | The converter. It shapes every character with HarfBuzz and draws it with FreeType in monochrome at the weight, hinting and metrics of its script, then places it in the 12 × 20 cell. Its dependencies are pinned in its inline metadata. |
| `settings.txt` | Each script's sources, weight, hinting and metrics, and each output font's characters. The values were measured in the prototypes of October 2026 (decision E.77, rdm DEC-601, which gives the reasons for them). |
| `sources.txt` | The Noto source fonts: their addresses in the google/fonts repository, pinned to commit `47d54ca1ebb14fd864695f1d39dc734f39017c33` (16 October 2026), and their SHA-256. The converter downloads them to a cache outside the repository and refuses any file whose checksum differs. The sources are never committed. |
| `*.bdf` | The converted fonts, 12 × 20, in the form of REQ-FNT-002: `noto-teletext.bdf`, the standard's 24 sets and neoteletext's Latin, Cyrillic, Georgian, Armenian and Tifinagh sets; `noto-teletext-composed.bdf`, every letter a Latin base and a diacritical mark compose to; `noto-teletext-arabic.bdf`, the Arabic-script sets and their joined forms; `noto-teletext-thai.bdf`, Thai and Lao; `noto-teletext-japanese.bdf`, the Japanese kana sets and the rōmaji option (DEC-2184, DEC-2187, REQ-2189), drawn from Noto Sans JP. |
| `fonts.txt` | The folder's index as a font library (REQ-FNT-004), with each font's conformance (REQ-FNT-011). |
| `OFL.txt` | The SIL Open Font License 1.1, with the copyright line of every source. |

## Licence

Noto is licensed under the SIL Open Font License 1.1, which `OFL.txt` reproduces with each source's copyright line. Its notices declare no Reserved Font Name for "Noto", so the converted fonts may keep "Noto" in their names. Noto Sans JP, the source of the Japanese file, is Adobe's Source Han Sans with the Reserved Font Name "Source": no converted font's name uses that word. A converted font is a Modified Version under the licence and is distributed under it.

## How the fonts are made

```sh
cargo build --profile iterate -p neoteletext-cli
uv run third_party/noto/convert.py --neottx target/iterate/neottx
```

The converter writes every font of `settings.txt`, then `fonts.txt`, then the checksums below. Then `cargo xtask glyphs` compiles the files into the core. `cargo xtask check-noto`, run by every gate, fails if a committed font differs from its checksum here or if a font and its line do not match, and a test of `xtask` fails if the compiled table is not these files.

## Checksums

<!-- checksums: written by convert.py -->
- `noto-teletext.bdf`: `82fe9e2bb463bfc301ba4509c21542fecef7f5a8a27d0c87de0f5a529a7a4a4a`
- `noto-teletext-arabic.bdf`: `17cfa3817914c2a05ffadc1dd55f834356397ac5e9864fc82a0d6337b9c45629`
- `noto-teletext-thai.bdf`: `168f95b7ca735fd3a17bb5625a77ad4bf998838c9c6e6a71e378a925a030d8cb`
- `noto-teletext-composed.bdf`: `e500e27361addb68119d02834192206216a3fd672c6926d082947d9f6092fe87`
- `noto-teletext-japanese.bdf`: `a6bcc0a95f0746cbba59a82cbc871fa6c2727611716e279fe589f2b5a7eea398`
<!-- end checksums -->
