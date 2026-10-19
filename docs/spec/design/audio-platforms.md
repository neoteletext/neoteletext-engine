# Teletext audio on each platform

How the audio profile (`audio.md`, area AUD) is built on each platform neoteletext and T42 Browser target: macOS, Windows, Linux and microcontrollers. Built on 19 October 2026: the engine crates of section 1 and T42 Browser on macOS (section 2). Windows, Linux and microcontrollers are not built yet.

The profile is the source for what a receiver does. This document says where each part of that runs and what each platform supplies. The microcontroller part stays at the level of architecture, so it is compatible with the tellyboy driver without fixing its interface. That interface is specified when it is implemented.

---

## 1. One core, one port trait, one sink per platform

```
                       ┌─────────────────────────────────────────────────────┐
  bytes ─────────────► │ neoteletext-audio  (no_std, no allocation, fuzzed)  │
  IDL packets,         │  layer 1  carriage: reassembly, CRCs, record bounds │
  .ttau records,       │  layer 2  object store (compressed, fixed size)     │
  WebSocket messages   │  layer 3  control: channels, sequence rule, pages   │
                       │  codec    IMA ADPCM → 16-bit PCM (feature-gated;    │
                       │           a sink that decodes it itself, such as    │
                       │           tellyboy, leaves it out)                  │
                       └──────────────────────┬──────────────────────────────┘
                                              │ voice operations (audio.md §10.1)
                                              ▼
                              trait VoiceSink   ← the port supplies this
                    ┌─────────────────────────┴─────────────────────────┐
                    ▼                                                   ▼
     desktop sink: neoteletext-audio-mix                 microcontroller sink: tellyboy
     integer mixer, 5 ms ramps, resampler,               tb_audio_voice_rom / stream /
     over the operating system's audio output            fade / set_gain, via C
```

### 1.1 The core: a new engine crate, `neoteletext-audio`

It goes in the engine, beside `neoteletext-core`, because every receiver needs the same rules: the browser, a microcontroller receiver, and any other port.

- **No standard library and no allocation.** Every buffer is a slice the port provides, as the page store already is (`porting.md`, Memory). The receiver level (audio.md §12) sets the slice sizes.
- **No panics.** It has the same no-panic lints as the engine's other parsers of outside input, and is fuzzed: the record parser, the fragment reassembler and each codec decoder.
- **Cargo features** (decision E.61's pattern):
  - `idl`, the broadcast carriage;
  - `records`, the network and file carriage;
  - `ima`, the IMA ADPCM decoder.

  A port leaves out what it doesn't use. Leaving out `ima` also suits a sink that decodes IMA ADPCM itself, as tellyboy does.
- **Time** comes from the port's existing `Clock` (`porting.md`), for stall timers, and from the sink's own count of samples played, for positions. Nothing in the core reads a wall clock.

### 1.2 The port trait

The porting checklist gains one optional row: **an audio sink**, implementing `neoteletext_audio::port::VoiceSink`. It takes exactly the operations of audio.md §10.1:
- start, queue, stop, gain and duck, each with a channel and a fade time;
- position, which the core asks.

A sink also declares:
- how many channels it can play (audio.md §10.4);
- whether it decodes IMA ADPCM itself. If it does, the core hands it IMA ADPCM blocks undecoded; if not, the core decodes them to 16-bit PCM.

### 1.3 Where the packets come from

The engine already acquires every teletext packet. It must also pass **independent data line packets on data channel 12** (EN 300 708 table 16) to `neoteletext-audio`, instead of discarding them. This is a change to the engine, made in the engine's own repository by its own process, as T42 Browser's architecture §5.1 requires.

Records from a network or a file arrive as bytes, the way page bytes do now.

---

## 2. Desktop: macOS, Windows and Linux

### 2.1 Where each part runs in T42 Browser

The process model (`neoteletext-browser/docs/architecture.md` §6) decides the placement.

| Part | Process | Why there |
|---|---|---|
| Carriage, store, control and codec decoding (the whole of `neoteletext-audio`) | **Decoder**, one per channel | It reads untrusted bytes, so it runs where nothing can be stolen (architecture principle 2). It already receives *Showing*, so it knows which page is open, which page binding (audio.md §7.3) needs. |
| The user's rules: mute, master volume, the *page sounds* setting, which channel is audible | **Trusted core** | These are the user's decisions. Only the active tab's channel is audible, and a background tab fades out. |
| Mixer, resampler, audio output | **Interface process** | It already holds the one device right it needs, the display. Audio output is a second device of the same kind. It never parses outside data, because it receives only 16-bit PCM that a decoder produced and the trusted core checked, as it receives only images the core produced. |

A compromised decoder can make sounds of its choosing in its own channel's four voices, while the trusted core lets that channel be heard. That is the same exposure as a decoder drawing pictures of its choosing in its pages. It can't make a background tab audible, override mute, or exceed the master volume, because those are applied by the trusted core and then by the interface's mixer.

### 2.2 New messages

These go in the `protocol` crate. Each is bounded before anything is allocated, and each side accepts only its own kinds (architecture §6.4).

| Direction | Message | Content and bounds |
|---|---|---|
| Decoder → core → interface | **Sound** | Channel-local sound id, a sample offset, and a run of 16-bit PCM at the object's rate: at most 64 KB a message. The total held for a channel is bounded by the receiver level's store. |
| Decoder → core → interface | **Voice** | One audio.md §10.1 operation: channel 0–3, sound id, gain 0–100, fade 5–10 000 ms, duck code 0–6, loop and after-current flags |
| Interface → core → decoder | **Played** | Per channel: the position and whether the voice ended, so the decoder can start a pending sound and report events |
| Core → interface | **Audio** | The user's settings: mute, master volume, the audible channel |

The trusted core re-checks every field of Sound and Voice against these bounds, as it re-checks frames. Any violation closes the decoder.

A page shown in T42 Browser's page wall isn't "open". **Page binding applies to the page the user has opened** (zoomed or chosen) in the active tab, not to every page visible on the wall. This is open decision D1 (section 5).

### 2.3 The mixer: a new crate, `neoteletext-audio-mix`

It is portable Rust, with no operating-system code. It lives in the engine beside `neoteletext-audio`, because any desktop or phone port needs it. It behaves like tellyboy's mixer, so a page sounds alike everywhere.

| Function | Implementation |
|---|---|
| Voices | 4, one per channel, each a position in a 16-bit PCM buffer |
| Gain | Whole percentages, applied as integers. Every change ramps linearly over its fade time, at least 5 ms (audio.md §10.2). |
| Ducking | The EN 300 708 table 19 steps, as the percentages of audio.md §10.3 |
| Mix | Sum in 32 bits, then master volume, then clamp to 16 bits |
| Resampling | Each voice from its object's rate to the device's rate, often 48 000 Hz. A polyphase windowed-sinc filter, written here with no library. It is better than tellyboy's linear interpolation: desktop speakers reproduce the images linear interpolation leaves, and the processor cost is negligible. |
| Live drift | For a live sound (audio.md §8.3), the resampling ratio is trimmed, on the same feed-forward principle as tellyboy's `tb_ares`, to keep the buffer half full |
| Real time | The mixer runs in the audio thread's callback. That callback never allocates, never takes a lock and never frees. Commands arrive on a single-producer, single-consumer ring. Sound buffers are allocated on the interface's main thread and handed over; when a voice ends, its buffer goes back on a return ring to be freed on the main thread. |
| Testing | The mixer renders into a buffer with no device, so the gate tests it on every host: golden checksums for mixes, ramps, ducking and resampling, independent of any sound card |

### 2.4 Audio output on each operating system

T42 Browser's crates forbid `unsafe` code (`native/Cargo.toml`). Every operating system's audio interface is a C interface, so output needs a library that wraps it. **Library choice is open decision D2**:

| Option | What it is | Pros | Cons |
|---|---|---|---|
| **A. `cpal`** (Apache-2.0) | One crate over Core Audio on macOS and iOS, WASAPI on Windows, ALSA on Linux, AAudio on Android | One interface on every desktop and phone; widely used; the mixer is called from one callback the same way everywhere | One more dependency tree to pin and audit. On Linux it reaches PipeWire and PulseAudio through ALSA's plugins rather than directly. |
| **B. One crate per system:** `objc2-audio-toolbox` (macOS), `windows` (WASAPI), `alsa` (Linux) | The systems' own interfaces, each wrapped thinly | Fewer layers | Three back ends to write and test, and a different threading model on each |

The recommendation is **A**, pinned to its release current when approved, and added only on approval.

Per system:

| System | Output path | Build and licence | Sandbox (architecture §6.5) |
|---|---|---|---|
| **macOS** | Core Audio, through the HAL output audio unit. The device rate is usually 48 000 Hz, with a buffer of about 10 ms. | System framework, nothing to ship | The interface's `sandbox-exec` profile must allow the Mach lookups Core Audio needs to reach `coreaudiod`. Under the App Sandbox, audio *output* needs no entitlement; only input does. Both statements are to be verified when built. |
| **Windows** | WASAPI shared mode at the mix format the device reports | System DLLs, nothing to ship | AppContainer processes can play audio through WASAPI. To be verified when built, as Windows is not yet built at all. |
| **Linux** | ALSA, which on current desktops routes through the PipeWire or PulseAudio plugin | Builds against `libasound` (the `alsa-sys` crate): `libasound2-dev` on the Linux check host. At run time it uses the system's `libasound`, which is **LGPL**. It is linked dynamically and not shipped, but it is outside cargo-deny's view, so the owner must accept it explicitly (decision D3). | Landlock must allow reading ALSA's configuration (`/usr/share/alsa`, `/etc/asound.conf`) and either the PipeWire or PulseAudio socket in `$XDG_RUNTIME_DIR` or `/dev/snd`. The seccomp filter must allow Unix-domain `socket`/`connect` and the shared-memory calls PipeWire uses. Path-named Unix sockets work inside the interface's empty network namespace. This must be exercised on foundintelquiet4060 or oldvga before the filter is fixed. |

### 2.5 What the desktop gate adds

- Unit tests and fuzz targets for `neoteletext-audio`, in the engine's gate.
- Golden checksums for `neoteletext-audio-mix`, with no device, in both repositories' gates.
- The new protocol messages in the protocol crate's round-trip tests, and in the hostile-interface and hostile-decoder harnesses (requirement 3.9). A decoder that sends an out-of-bounds Sound or Voice is closed.
- The Linux gate on the check host builds with `libasound2-dev` installed and runs the mixer tests. Real playback is checked by hand once per platform, as the window is.

---

## 3. Microcontrollers: tellyboy

### 3.1 The shape

On a microcontroller there is one program (architecture §6.5). It runs the engine and `neoteletext-audio` in Rust, and the tellyboy driver in C. The sink implements `VoiceSink` by calling tellyboy's voice API through a small C interface.

| Profile concept | tellyboy |
|---|---|
| A channel | One tellyboy voice. The receiver declares how many it may use, because the device's own sounds share tellyboy's 4. |
| A complete object (audio.md §8.1) | A **ROM voice** (`tb_audio_voice_rom`), playing straight from the object store. IMA ADPCM and PCM are decoded by tellyboy as it plays, so the core is built without `ima`. |
| A progressive or live sound (§8.2, §8.3) | A **stream voice** (`tb_audio_stream_open`), with a ring the receiver provides. A live sound also uses `tb_audio_stream_frame` for drift. |
| The end of a progressive sound | `tb_audio_stream_end`, or its tail never plays |
| Gain, fades, stop | `tb_audio_voice_set_gain`, `tb_audio_voice_fade`, `tb_audio_voice_fade_out`. Never `tb_audio_voice_stop`, which clicks. |
| Ducking | `tb_audio_voice_fade` on the other channels to the §10.3 percentage |
| Position | `tb_audio_voice_position` |

### 3.2 Constraints the port must respect

These come from tellyboy's design.

- **Store placement:** the object store lives in RAM that tellyboy's refill interrupt can read. On ESP32 that is internal RAM (`DRAM_ATTR`), not flash. An object a voice is playing is never moved or evicted (audio.md §5).
- **No calls from the interrupt:** tellyboy runs no application code in its refill interrupt. All of `neoteletext-audio` runs in the main program and only calls the driver.
- **Stream rings are sized by the client:** for its worst gap between deliveries, measured on the target, not estimated.
- **The voice count is fixed at compile time:** because it sets the interrupt's worst-case run time. The receiver never asks for more voices than the build has.

### 3.3 Left for implementation

- The C interface itself.
- Whether the Rust core and tellyboy share one build system.
- Memory budgets on each microcontroller.
- Isolation where a memory protection unit exists: the engine's REQ-SEC-021 guidance applies to the audio parser as it does to the decoder.

---

## 4. Phones (later)

iOS and Android are later targets (architecture §13). With option A of §2.4, the same mixer runs over Core Audio on iOS and AAudio on Android. On iOS everything runs in one process (architecture §6.5), so the decoder, trusted-core and interface roles of §2.1 become modules, not processes.

---

## 5. Decisions

Taken on 19 October 2026, while the owner was away, as the owner had instructed (decision E.85 and T42 Browser's decision 37):
- **D1.** A page-bound sound follows the page the user has opened, the zoomed page of the active tab, not every page visible on the wall.
- **D2.** Desktop audio output uses `cpal`, version 0.18.2 (Apache-2.0).
- **D4.** `neoteletext-audio` and `neoteletext-audio-mix` live in the engine.

Still open:
- **D3.** Accepting the system's LGPL `libasound` on Linux, linked dynamically and not shipped. It arises only when Linux is built.
