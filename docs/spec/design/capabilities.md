# Capabilities and the modern-classic profile

How the decoder's optional capabilities are switched on and off at compile time, which ones the project recommends, and why.

The rules are REQUIREMENTS.md decision E.71 and REQ-GEN-020, built on decision E.61 (compile-time features). Where this document and REQUIREMENTS.md differ, REQUIREMENTS.md wins.

Downloadable characters (DRCS), monochrome and in colour, were removed on 20 October 2026 with their features `drcs` and `drcs-multicolour` (decision E.95), and bold and italic characters with `bold` and `italic` (decision E.96).

## 1. The principle

Teletext looks the way it does because of its constraints:

- a fixed grid of 40 columns by 24 rows, every character the same width;
- eight bright, saturated colours;
- block graphics built from six-cell mosaics, contiguous or separated;
- colour and style changes made inside the text itself, so the page is drawn one character cell at a time;
- no proportional spacing, no photographs, and no free placement of anything.

Those limits are the style. A decoder that removes them can still show teletext pages, but what it shows stops looking like teletext.

The later levels of the standard, 2.5 and 3.5 (ETSI EN 300 706), add capabilities of two kinds:

- **those that keep the constraints and make pages easier to write and to read:** more languages, colour without spending a character cell, reusable page parts, and custom characters that sit in the grid like any other character;
- **those that remove the constraints:** thousands of colours, typographic styling, a wider page, per-pixel colour images, and motion.

The project keeps the first kind and leaves out the second. The second kind is not deleted: every one of those capabilities is implemented and tested, and can be compiled in when a use needs it. The recommended build, the **modern-classic** profile, has them switched off.

## 2. What is kept and what is left out

### Kept: the constraints that make the style

| Capability | Why it is kept |
|---|---|
| The 40 by 24 fixed-width grid | It is the page. |
| The eight-colour palette as the default | The saturated eight colours are teletext's colour. |
| Contiguous and separated mosaics | The block graphics of every teletext page. |
| No proportional spacing | Fixed pitch keeps every character in its cell. The receiver never lays out proportional text (REQUIREMENTS.md Part G item 1). |

### Kept: the capabilities that make it more usable without changing the style

| Capability | Why it is kept | Feature |
|---|---|---|
| Non-spacing colour and character attributes | Colour and style changes no longer take up a character cell, so text and graphics can sit side by side. The page is still a grid of cells. | `level25` |
| The additional international character sets | Cyrillic, Greek, Arabic, Hebrew and the accented Latin options, so services can publish in their own languages. | always compiled |
| Reusable page objects | Parts of a page defined once and placed many times. They change how pages are sent, not how they look. | `objects` |

### Implemented, but switched off in the profile: the capabilities that would dilute the style

| Capability | What it would do to the page | Feature |
|---|---|---|
| The 32-colour palette | Lets a page redefine its colours from 4,096, which allows photographic shading and brand colours in place of the eight. | `level25-32colors`, `level35-32colors` |
| Additional Flash Functions (Table 29) | Flash in several modes, rates and phases, with "apparent movement" across the page, beyond the single slow flash of Level 1. | `additional-flash-functions` |

When a capability is compiled out, pages that use it still display. Its data is ignored and the page shows as it would on a receiver without that capability. That is how teletext was always meant to degrade, since every page must also work on a Level 1 television.

## 3. How the mechanism works

### Cargo features

Each capability is a Cargo feature of `neoteletext-core`:

| Feature | Needs | What it adds |
|---|---|---|
| `level15` | — | Level 1.5: accented characters and the G3 set placed by the page's enhancement data |
| `level25` | — | Levels 2.5 and 3.5: the enhancement processor, placing characters by the page's enhancement data |
| `nonspacing-attributes` | `level25` | The non-spacing attributes of Table 27: colours, flash and display attributes placed by X/26 triplets |
| `level25-32colors` | `level25` | The Colour Map of Level 2.5 (clause 12.4): CLUTs 2 and 3, redefined by X/28/0 Format 1 and M/29/0 |
| `level35-32colors` | `level25-32colors` | The Colour Map of Level 3.5: as Level 2.5, and CLUTs 0 and 1 redefined by X/28/4 and M/29/4, never CLUT 1 entry 0 |
| `objects` | `level25` | Active, Adaptive and Passive objects, and default objects |
| `additional-flash-functions` | `nonspacing-attributes` | The Additional Flash Functions of Table 29, column mode 00111 |
| `modern-classic` | — | The profile: `level15`, `level25`, `nonspacing-attributes`, `render`, `bedstead`, `viewer` and `objects` |

**The default build** (decision E.105, the owner's instruction of 22 October 2026, amending decision E.61's rule that every feature is on): Levels 1.0 and 1.5, and of Levels 2.5 and 3.5 the non-spacing attributes and the Colour Map of both levels: `level15`, `level25`, `nonspacing-attributes`, `level25-32colors`, `level35-32colors`, with `render`, `bedstead`, `viewer` and `extended-sets`. Objects and the Additional Flash Functions are not default: a build names them, as the conformance tests do (the gate runs them with every feature). `neottx` uses the default. A port chooses the profile by turning the defaults off:

```toml
neoteletext-core = { version = "...", default-features = false, features = ["modern-classic"] }
```

A port can also name capabilities one by one, for example the profile plus `level35-32colors`.

### What a feature switches

A capability is decided in one place in the decoder and, where it is drawn differently, in one place in the renderer:

| Feature | Where the decoder decides | Where the renderer decides |
|---|---|---|
| `objects` | Object invocations and default objects in the enhancement walk | — |
| `level25-32colors` | The colour words of X/28/0 and M/29/0, and CLUTs 2 and 3 apart from 0 and 1 | — (the Colour Map itself) |
| `level35-32colors` | The colour words of X/28/4 and M/29/4 | — (the Colour Map itself) |
| `additional-flash-functions` | The Additional Flash Functions triplet | — |

Each test is a constant such as `cfg!(feature = "additional-flash-functions")`. The compiler removes the branch that cannot run, and the linker removes the code only that branch used. A build without a capability therefore contains neither its decoding nor its drawing, and pays nothing for it at run time.

### What happens to a page that uses a capability compiled out

| Feature compiled out | The page's data | What the viewer sees |
|---|---|---|
| `objects` | Invocations and default objects ignored | The page without its objects; its own enhancement data still applies |
| `level25-32colors` | No redefinition; tables 2 and 3 are copies of tables 0 and 1 | At most the sixteen default colours, with the eight classic colours as the default |
| `level35-32colors` (with `level25-32colors`) | Tables 0 and 1 keep their defaults; tables 2 and 3 are redefined | The eight classic colours as the default, and sixteen of the page's own |
| `additional-flash-functions` | The triplet ignored | Steady text, except Level 1 flash, which still flashes |

None of these is an error. Fault statistics count only what they always did, such as absent objects.

### Size

Measured on 12 October 2026 with `cargo xtask size`, on the linked Cortex-M0 image (`opt-level = "z"`, link-time optimisation) of a receiver with Level 1.5, the renderer and the viewer:

| Build | Flash |
|---|---|
| Level 1.5 | 43,002 bytes |
| Level 3.5 with the modern-classic profile's capabilities | 62,894 bytes |
| Level 3.5 with every capability | 65,758 bytes |

The profile saves 2,864 bytes, about 4 percent. The profile is chosen for how pages look, not for size: most of the enhancement processor is the non-spacing attributes and the walk over the page's data, which the profile keeps.

## 4. Testing

- **The gate** runs the whole conformance suite with every feature (`--all-features`); the default build runs the tests its features allow.
- **Each test of a capability** requires that capability's feature, so a build without it does not run the test.
- **`crates/neoteletext-core/tests/capabilities.rs`** checks each capability compiled out: that its data is ignored exactly as section 3 says.
- **`cargo xtask features`** builds, lints and tests the core with Levels 1.5 and 2.5 and every capability off, and with the `modern-classic` profile, besides the builds of decision E.61.
- **`cargo xtask size`** measures the profile's image beside the full one.

## 5. Adding a capability

A new switchable capability follows the same steps:

1. **Record it first:** a decision in rdm that says what it is, what happens to page data when it is compiled out, and which side of the principle of section 1 it falls on.
2. **Add the feature** to `crates/neoteletext-core/Cargo.toml`, in the default list only if the owner says so (decision E.105), and in `modern-classic` if it keeps the style.
3. **Gate its decision point** in the decoder, and its drawing in the renderer, with `cfg!(feature = "...")`. Behaviour when it is compiled out must be the "not supported" behaviour of a lower level, never an error.
4. **Gate its tests** with `#[cfg(feature = "...")]`, and add a test to `tests/capabilities.rs` for the compiled-out behaviour.
5. **Add it to the builds** of `xtask/src/features.rs` and the test image's features in `firmware/test-image/Cargo.toml`, and measure with `cargo xtask size`.
6. **Update this document,** the porting guide and the feature table of `ARCHITECTURE.md`.
