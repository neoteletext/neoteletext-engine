# The teletext audio profile

Version 2 of the profile, adopted on 19 October 2026 (decisions E.84 to E.86). **The normative rules are the requirements of area AUD** (`../requirements/aud.md`); this document explains them, cites their sources and records why they depart from the standards. Where the two differ, the requirements win. Parts not yet built are listed in the requirements and in `TECH-DEBT.md`.

This profile carries sound with teletext: short sounds attached to pages, longer recordings, and live speech. It is one design for every receiver neoteletext targets: desktop (macOS, Windows, Linux) and microcontrollers, where sound plays through the tellyboy audio driver.

The profile follows the teletext and audio standards wherever they fit, cites the clause it follows at each point, and says why wherever it departs from them (section 13). Every figure is either cited or derived in the text.

MUST, SHOULD and MAY below are the profile's own words. They add no requirements to the cited standards.

---

## 1. What the sources establish

Every source below is in `teletext-reference/` unless stated otherwise.

| Source | What it gives this profile |
|---|---|
| ETSI EN 300 708 V1.2.1 (2003-04) §6.7, *Low bit-rate audio* (`teletext-standards/en_300_708_2003.pdf`, pp. 37–39) | Teletext's own carriage for audio. Independent data lines (IDL) on data channel 4 (programme-related, e.g. Audetel) and data channel 12 (programme-independent), table 16. Bytes 8–45 carry audio, 38 bytes a packet (§6.7.3.1). Table 21 allows one to four VBI lines per frame, single- or two-channel, and service byte bit 4 = 1 marks a **user-defined service** whose other bits, and whose control byte, are the application's (§6.7.3.2). A decoder mutes when packets of the service stop arriving, after a time the service's own protocol defines (§6.7.1.1). Table 19, the Audetel *fade byte*, attenuates the programme sound in steps of 0, 4, 8, 12, 18, 24 and 30 dB, and table 20 mutes the speech. |
| EN 300 708 §6.5.8 and §6.8 | A 16-bit CRC for IDL packets (§6.5.8.1, polynomial x¹⁶+x⁹+x⁷+x⁴+1). IDL Format B, with Reed–Solomon error correction over 16-packet blocks (§6.8.2) and an Application Identifier from the EBU register TR 101 231 (§6.8.1.2). |
| ETSI ETS 300 149 (March 1992), *Videotex Audio syntax* (`teletext-audio/standards/ets_300_149_1992.pdf`) | A sound syntax for **ISDN videotex terminals** (§1 Scope): sound headers and blocks inside ISO/IEC 9281 picture entities. The encodings it allows are G.711 A-law and µ-law, G.721/G.723 ADPCM, G.722, GSM RPE-LTP, J.41 and J.42 (§7.2.2). Its encoding codes 2/7 to 2/14 are reserved and "shall be ignored when received". Samples narrower than 8 bits are packed most significant bit first (§8, figure 2). |
| Audetel: Kondoz, Evans and Suddle 1993; Black et al. 1995; Suddle 1996 thesis chapter 7 (`teletext-audio/audetel/`) | The one teletext audio service that was broadcast. It used a CELP speech coder at 7.6 kbit/s narrowband in **one teletext line**, and PRELP at 14.1 kbit/s wideband in **two lines** (Black 1995 §1). A 20 ms speech frame was 160 bits, of which 152 were data (19 bytes); two frames went in each packet (Suddle 1996 p. 189; EN 300 708 §6.7.2.3). Hamming (7,4) protected the most sensitive bits, and a lost frame was concealed rather than silenced (Suddle §7.5.4). The decoder needed about 2 MFLOPS. The coder was never published as a standard. |
| *Television*, December 1994, pp. 110–111 (`teletext-audio/audetel/reports/television_1994_12.pdf`) | "Up to 7.6 kbit/s of data can be transmitted on a teletext line." Audetel used data channel 4. A dropped transmission was filled in by interpolation. |
| ITC annual reports 1991 and 1992 (`teletext-audio/audetel/reports/`) | Audetel's history. Its speech coder was tuned to the elderly ear (1992 report p. 45). |
| Japanese coded teletext music, MUSCOT: Sugimori et al. 1982; Isobe et al. 1986; Araki and Hanafusa 1987; the MUSCOT features article (`teletext-audio/japanese-teletext-music/`) | Music sent as **notes, not waveforms**: a whole tune in a few kilobytes (Level A at most 200 packets, Level B 400; Araki 1987 table 1). Level A plays 6 melody and 5 rhythm voices; Level B plays 16 and 8. Parts are sent in priority order, so a smaller receiver plays the first parts it can and drops the rest. Araki 1987 lists **eight rules** by which a Level A receiver plays Level B data. A part with corrupted data is stopped, and if several parts are corrupted, all are (Sugimori 1982 §4.3(5)). Used for title music, background music and page-turn cues. **Not adopted** (section 9). Only its idea of playing the highest-priority parts on a smaller receiver is used, for channels (§10.4). |
| tellyboy audio driver (`~/devel/tellyboy/components/audio-sd`, `<tellyboy/audio.h>`) | The microcontroller playback this profile must fit (section 11). |

The two documents that define the codec are in `teletext-reference/teletext-audio/codecs/` since 20 October 2026: the IMA *Recommended Practices for Enhancing Digital Audio Compatibility in Multimedia Systems*, revision 3.00 (21 October 1992), whose Appendix D section 6 gives the ADPCM reference algorithms, and Microsoft's *Multimedia Standards Update: New Multimedia Data Types and Data Techniques*, revision 3.0 (15 April 1994), whose "IMA ADPCM Wave Type" and "DVI ADPCM Wave Type" sections define format tag `0x0011` and its block layout.

The three speech-coding theses in `teletext-audio/codecs/` and Ong 1994 hold no teletext material. Ong's abstract does make one point used here: losing frames is normal, and frame recovery is part of the design (section 8).

---

## 2. The shape of the design

```
  broadcast (EN 300 708 §6.7 IDL)        network (WebSocket)        file (.ttau)
            │ fragments                         │ one record             │ records
            ▼                                   ▼                        ▼
  ┌──────────────────────────── 1. carriage: reassemble, check ──────────────────────────┐
  │                          records: OBJECT, DATA, CONTROL (section 4)                    │
  └────────────────────────────────────────────┬───────────────────────────────────────────┘
                                               ▼
  2. object store: compressed sound, bounded, by handle and generation (section 5)
                                               ▼
  3. control: channels, latest command wins, page binding (section 7)
                                               ▼
  4. voice operations: start, stop, fade, gain, duck  (section 10)
            │                                                   │
            ▼                                                   ▼
  desktop voice sink: an integer mixer      microcontroller voice sink: the tellyboy driver
  over Core Audio / WASAPI / ALSA           (tb_audio_voice_rom, tb_audio_stream_*, fades)
```

Layers 1 to 3 form one Rust library: `no_std`, every buffer a fixed size chosen by the receiver level (section 12), and no allocation after start. It mixes nothing. It decodes a codec only for a sink that can't decode it itself. Its output is voice operations on numbered voices.

The voice sink is the only platform code. On a microcontroller it is the tellyboy driver, which already decodes IMA ADPCM, mixes four voices, fades and tracks clock drift. The desktop sink reproduces the same behaviour in software, so a page sounds the same on both.

How each platform builds this is in `audio-platforms.md`. In T42 Browser, layers 1 to 3 and codec decoding run in the sandboxed decoder process. The trusted core applies the user's settings, and the interface process mixes and plays 16-bit PCM.

---

## 3. Sound formats

### 3.1 Codecs

The profile carries two formats, 16-bit linear PCM and IMA ADPCM, and nothing else: the owner decided on 20 October 2026 that there will be no speech codec (decision E.89). tellyboy plays both natively. The owner decided on 19 October 2026 that the older standards' codecs (G.711, G.726) and 8-bit PCM are not carried: content in them is converted when it is published (§13). Music is recorded sound. Note formats (MIDI, MUSCOT) are out of scope (section 9).

| Code | Codec | Bits/sample | Support | Standard | tellyboy |
|---:|---|---:|---|---|---|
| 0 | Linear PCM, signed 16-bit little-endian | 16 | every receiver | WAVE format tag `0x0001`. Not in any teletext or videotex standard (§13). | native, `TB_AUDIO_FMT_S16LE` |
| 1 | IMA ADPCM in WAVE blocks (format tag `0x0011`) | 4 | every receiver | IMA 1992. ADPCM at 32 kbit/s is ETS 300 149's default and mandatory encoding (§7.3 table 2, §9.1 table 3), but that ADPCM is G.721/G.726, not IMA (§13). | native, `TB_AUDIO_FMT_IMA_ADPCM`, assembly on every ISA |
| 2–255 | reserved: a receiver ignores the object. Code 2 was held for a speech codec until the owner decided against one (E.89) | | | | |

Every object is one channel. The output of a microcontroller receiver is mono, and so is the broadcast budget (§6.3).

**IMA ADPCM is the baseline codec.** It is 4 bits a sample, the same as G.726 at 32 kbit/s. Each WAVE block begins with a header holding the predictor, as a signed 16-bit value that is the block's first sample, then the step index, then a reserved byte. So **every block decodes on its own**, and a receiver can:
- start in the middle of a carousel at any block;
- lose a block and lose only that block's samples.

G.726, the codec of version 1, has neither property: one lost byte breaks every sample after it. IMA ADPCM also already runs, in assembly and checked against a reference decoder, on every microcontroller family tellyboy supports (ARMv6-M, RV32, Xtensa).

A block of B bytes holds 1 + 2 × (B − 4) samples. Allowed block sizes are 256 and 512 bytes. 1 024, ffmpeg's default, doesn't fit a DATA record, whose body leaves 1 011 bytes for blocks (§4.1, §4.3). Encode with `-block_size 512` or smaller.

### 3.2 Sample rates

The common audio sample rates are all allowed. The owner authorised this on 19 October 2026, whether or not a teletext standard lists the rate. ETS 300 149 itself names only bit rates (§7.2.3). Its encodings imply 8 kHz, and 16 kHz for G.722.

| Code | Rate (Hz) | Typical use |
|---:|---:|---|
| 0 | 8 000 | telephone-band speech; the lowest bandwidth |
| 1 | 11 025 | |
| 2 | 16 000 | wideband speech, as Audetel's 14.1 kbit/s coder (Black 1995) |
| 3 | 22 050 | music and effects; tellyboy's movie sound (BASELINE.md) |
| 4 | 24 000 | |
| 5 | 32 000 | tellyboy's output rate: no resampling on a microcontroller |
| 6 | 44 100 | CD |
| 7 | 48 000 | broadcast and video production |

Every rate is inside tellyboy's accepted range of 8 000–48 000 Hz (`TB_AUDIO_SOURCE_MIN_HZ`/`MAX_HZ`).

On a microcontroller, rates above tellyboy's 32 kHz output carry sound it can't reproduce. They are also cut down by linear interpolation without a filter: tellyboy judges 44.1 kHz fine and 48 kHz's aliasing mild. They cost 38–50 % more bytes for no audible gain there, so publishers who target microcontrollers SHOULD use 32 kHz or below.

A stereo source is mixed down before encoding, as tellyboy does: (L + R) / 2.

### 3.3 Bit rates

The IMA ADPCM figures include block headers: 505 samples per 256-byte block, 1 017 per 512-byte block.

| Rate (Hz) | IMA ADPCM, 256-byte blocks (bit/s) | IMA ADPCM, 512-byte blocks (bit/s) | PCM 16-bit (bit/s) |
|---:|---:|---:|---:|
| 8 000 | 32 444 | 32 220 | 128 000 |
| 11 025 | 44 711 | 44 403 | 176 400 |
| 16 000 | 64 887 | 64 440 | 256 000 |
| 22 050 | 89 423 | 88 806 | 352 800 |
| 24 000 | 97 331 | 96 661 | 384 000 |
| 32 000 | 129 774 | 128 881 | 512 000 |
| 44 100 | 178 845 | 177 612 | 705 600 |
| 48 000 | 194 661 | 193 322 | 768 000 |

---

## 4. Records

Every carriage delivers the same records. Integers are unsigned and big-endian, except the PCM samples inside a DATA payload, which are little-endian as WAVE and tellyboy expect.

### 4.1 Frame

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 1 | Profile version: `2` |
| 1 | 1 | Record type: `1` OBJECT, `2` DATA, `3` CONTROL. Any other value: the record is discarded. |
| 2 | 2 | Body length L, at most 1 016 |
| 4 | L | Body |
| 4 + L | 4 | CRC-32 |

A whole record is at most **1 024 bytes**. That keeps the reassembly buffer a microcontroller needs at 1 KB per record in flight. A record is checked against every bound before any of it is stored.

The CRC is CRC-32/ISO-HDLC over bytes 0 to 3 + L: polynomial `0x04C11DB7`, reflected, initial and final XOR `0xFFFFFFFF`, check value `0xCBF43926` for ASCII `123456789`. It detects damage, not forgery.

Rejected outright: a CRC failure, a length beyond any bound, a field outside its range, or a body shorter than its fixed fields. Bytes after the fixed fields of an OBJECT are optional extensions (§4.2). In DATA they are blocks. CONTROL has none, and trailing bytes there reject the record.

### 4.2 OBJECT: announce a sound

| Bytes | Field |
|---:|---|
| 1 | Handle, 0–255: the short name DATA and CONTROL use |
| 2 | Generation: serial number arithmetic (RFC 1982, 16 bits). A changed sound or definition gets a newer generation. |
| 4 | Object id: stable across generations within a service |
| 1 | Codec (§3.1) |
| 1 | Sample-rate code (§3.2) |
| 2 | Block bytes: 256 or 512 for IMA ADPCM; for 16-bit PCM an even number, at most 1 000 |
| 4 | Sample count. 0 marks a **live** sound with no fixed end (§8.3). |
| 2 | Block count, ⌈sample count / samples per block⌉; 0 for a live sound |
| … | Extensions: tag (1 byte), length (1 byte), value. Tag 1 is a label (UTF-8, at most 64 bytes, cleaned like page text before display); tag 2 a BCP 47 language tag (at most 16 bytes). Unknown tags are skipped. |

A repeat of the same OBJECT changes nothing.

An OBJECT with a newer generation for the same handle replaces the definition. A voice already playing the older generation plays to its end, and DATA for the older generation is then discarded.

An OBJECT that differs from a stored one without a newer generation is ignored, and the receiver reports `conflict`.

### 4.3 DATA: whole codec blocks

| Bytes | Field |
|---:|---|
| 1 | Handle |
| 2 | Generation |
| 2 | Index of the first block |
| rest | One or more whole blocks of the object's block size. Only the object's final block may be short. |

DATA for an unknown handle, or for a generation that doesn't match, is discarded. A receiver MAY hold such DATA within its limits until the OBJECT arrives.

A repeated block is ignored. A block that differs from the stored one at the same index and generation is a `conflict`: the object is invalid until a newer generation arrives.

### 4.4 CONTROL: commands

| Bytes | Field |
|---:|---|
| 1 | Channel, 0–3 |
| 2 | Sequence: RFC 1982 serial arithmetic, per channel (§7.1) |
| 1 | Operation: `1` play, `2` stop, `3` set gain |
| 1 | Handle (play only) |
| 2 | Generation (play only) |
| 1 | Gain, 0–100 per cent |
| 2 | Fade in milliseconds. Every change takes at least 5 ms (§10.2). |
| 1 | Flags: bit 0 *progressive* (§8.2); bit 1 *loop*; bit 2 *after current* (§7.2); others 0 |
| 1 | Duck: 0–6, the attenuations of EN 300 708 table 19, applied to the other channels (§10.3) |
| 2 | Page: a page number in EN 300 706 form, `0x100` to `0x8FF`, binding the command to that page (§7.3); `0xFFFF` for the whole service |

Fields an operation doesn't use are zero.

---

## 5. The object store

The store keeps sound **compressed**. A microcontroller plays IMA ADPCM straight from the store through `tb_audio_voice_rom`, which decodes as it plays, so decoded samples are never held.

The store has a fixed size set by the receiver level (section 12). An object is admitted only if it fits once it is complete, judged by its sample count and codec. When an object doesn't fit, the receiver reports `resource-limit`.

An object is evicted only when no channel is playing it, has it armed for a page, or holds it pending. Any other object may be evicted.

---

## 6. Carriage

### 6.1 Broadcast: EN 300 708 low bit-rate audio IDL

| Bytes | Field | Rule |
|---:|---|---|
| 1–3 | Clock run-in, framing code | EN 300 706 |
| 4 | Data channel | 12, programme-independent audio (EN 300 708 table 16). Hamming 8/4. |
| 5 | Designation code | 1111 (§6.4.1) |
| 6 | Service byte | Hamming 8/4. Bit 4 = 1, a user-defined service (table 21). Bits 1–3 select one of 8 audio services on the channel. |
| 7 | Control byte | User-defined (§6.7.3.2), Hamming 8/4. Bit 1 = first fragment of a record, bit 2 = last fragment; bits 3–4 are 0. |
| 8 | Record id | Incremented modulo 256 for each new record of the service, not for a repeat: the rule of EN 300 708 §6.5.5's continuity indicator |
| 9 | Fragment index | 0–30 |
| 10–43 | 34 bytes of the record | |
| 44–45 | CRC-16 over bytes 8–43 | EN 300 708 §6.5.8.1 polynomial, the remainder sent highest term first |

How the packet rate per line is derived:
- Audetel carried two 20 ms frames per packet (§6.7.2.3), so one packet every 40 ms: 25 packets a second for each VBI line-per-frame.
- 38 bytes × 8 bits × 25 = **7 600 bit/s a line**. This matches *Television* 1994 and the 7.6 kbit/s of Kondoz 1993.
- Of this, the record carries 34 × 8 × 25 = **6 800 bit/s a line**, or 27 200 bit/s at table 21's maximum of four lines.

A receiver reassembles fragments by record id. A packet that fails its CRC is dropped, and the transmitter's repeats fill the gap.

At most 2 records are reassembled at once on Level A, and 8 on Level B. When a fragment arrives for a further record, the oldest incomplete record is discarded.

A transmitter SHOULD cycle each object, as a page carousel cycles pages.

Following §6.7.1.1, a live sound fades out when its service has sent no packet for its stall time (§8.3).

**Format B (§6.8) was considered and not chosen for version 2.** Its Reed–Solomon code corrects far more than a CRC can detect, but it needs an Application Identifier allocated in the EBU register (TR 101 231), which this profile doesn't have. §6.7 is the clause that allocates audio its own channel, and its user-defined service needs no registration. Open decision 1 records the trade.

### 6.2 Network and file

- **WebSocket:** one binary message carries exactly one record. Text messages are ignored.
- **File (`.ttau`):** records concatenated. A truncated final record is discarded. A record whose length breaks a bound ends the file, because nothing after it can be framed safely.
- **Source:** the host associates each connection or file with one service. A record never names its own service, and the CRC is not authentication.

### 6.3 What each carriage can deliver

| Sound | Size | 1 broadcast line | 4 broadcast lines | Network |
|---|---:|---:|---:|---|
| 5 s speech, IMA ADPCM 8 kHz | 20 277 B | 23.9 s to download | 6.0 s | immediate |
| 30 s of music, IMA ADPCM 22.05 kHz | 335 KB | 6.6 min | 1.6 min | immediate |
| Live IMA ADPCM at 8 kHz | 32 444 bit/s | **does not fit** | **does not fit** (27 200) | fits |
| Live Audetel-class speech codec | 7 600 bit/s | fits, as Audetel did | fits | fits |

So, over broadcast:
- **Recorded sound is downloaded, then played.** That's the `complete` start of §8.1.
- **Live sound over broadcast is IMA ADPCM** at a low rate; there will be no speech codec (E.89), so a live sound costs about a third of a typical teletext allocation at 16 kHz.

Over a network, progressive IMA ADPCM streaming works at any rate in §3.3.

---

## 7. Control

### 7.1 The latest command wins, per channel

There are four channels, 0 to 3, each with a voice. For each (service, channel), a receiver keeps the sequence number of the last command it accepted, S.

A command is accepted only if its sequence is newer than S by RFC 1982 arithmetic. Accepting it sets S. Anything else is a repeat or arrived late, and is ignored.

That single rule replaces version 1's sessions, command history, cancellation barriers and instance ids, in constant memory:
- A repeated record never replays, restops or refades.
- A stop that arrives before the older play it cancels still wins, because the play's sequence is then stale.
- A play newer than a stop survives the stop.
- A new deliberate play takes a new sequence.

On a change of service, every channel's state is cleared. On tuning in, the first command seen is accepted.

### 7.2 Operations

- **Play** starts the object on the channel, fading out whatever was playing over the fade time.
- **Play with *after current*** goes into the channel's **one** pending slot, replacing anything already there. It starts when the current sound ends. This is version 1's queue, held to one slot so its memory is fixed.
- **Stop** fades the channel out and clears its pending slot.
- **Set gain** ramps to the new gain over the fade time.

With *loop* set, a play repeats the object until a later command on the channel stops it. Each pass decodes from the start, as tellyboy's looping ADPCM voices do.

### 7.3 Page binding

Teletext attaches things to pages, and MUSCOT's use was page sounds: title music, background music, a cue on turning a page.

A play whose page field is not `0xFFFF` is **armed** for that page:
- It starts when the page is shown and whenever it is shown again.
- It fades out when the page is left.
- A repeat of the record re-arms nothing.
- Entering the page with nothing armed plays nothing.
- A later command on the channel replaces the armed play.

Page sounds play only while the user's **page sounds** setting is on (open decision 4). A play for the whole service (`0xFFFF`) plays once, on acceptance.

Receiving data, reconnecting, or refreshing a page never starts a sound by itself.

---

## 8. Starting, loss and live sound

### 8.1 Complete (the default)

A play starts once the whole object has arrived and been checked. Over broadcast this is the only start for recorded sound (§6.3). A sound that has started this way can never run out of data.

### 8.2 Progressive

A play with the *progressive* flag starts once 40 ms of contiguous blocks are held, or the rest of the object if it is shorter. If a block is missing when it is due:
- that block's samples are **silence**, with a 5 ms fade on each side so the gap doesn't click;
- the timeline continues at the object's rate, so the sound is never shifted or stretched;
- a block that arrives after its time has passed is kept for a later play, but never plays out of time.

Because IMA ADPCM blocks are independent (§3.1), this one rule serves both codecs, and version 1's *wait* policy is no longer needed. PCM needs it even less.

Audetel concealed losses by interpolating speech parameters (Suddle §7.5.4). A waveform codec can't do that, so silence with fades is the waveform equivalent.

The receiver reports `concealed`, with each silenced range.

### 8.3 Live

An object with sample count 0 is live: blocks arrive indefinitely and their index wraps (RFC 1982). A live sound always plays progressively.

The sender's sample clock and the receiver's are independent, so a receiver MUST track the drift by resampling slightly to stay level with the stream. On tellyboy that is `tb_audio_stream_frame` with its drift-tracking ring. A desktop sink does the same with its own rate control.

A receiver MUST absorb at least ±1 % of sender clock error. That figure is a design target, not yet measured against a real broadcast clock. The engine's mixer buffers 400 ms of a live sound before it plays (a port may hand over packets every 200 ms) and trims its rate by up to ±5 %, in proportion to how far the buffer is from that level; its tests hold the buffer level with a sender 1 % fast and 1 % slow.

The receiver keeps the newest 32 blocks of a live sound and joins at the newest when a play can start. It gives the sink the blocks in order; a block still missing once three newer ones are held is silence. Over a T42 file or the network, live IMA ADPCM fits where a broadcast line does not (§6.3): T42 Browser reads 320 packets a second, and 8 kHz IMA ADPCM in 256-byte blocks needs about 127.

A live sound fades out after its **stall time** without new blocks. That is EN 300 708 §6.7.1.1's mute, with the time this profile defines: 2 seconds.

---

## 9. Music

Music is carried as recorded sound in the codecs of §3.1. Note formats, MIDI and MUSCOT among them, are out of scope: the owner decided this on 19 October 2026.

Over broadcast, recorded music is downloaded before it plays (§6.3), so its length is bounded by the time a carousel can spend on it. Over a network or from a file there is no such bound.

---

## 10. Voices, mixing and level

### 10.1 Voice operations

Layer 3 issues these operations and nothing else:
- *start* (channel, object, gain, fade, loop);
- *queue* (channel, object);
- *stop* (channel, fade);
- *gain* (channel, gain, fade);
- *duck* (channel, attenuation).

Layer 3 asks the sink where a voice has got to, measured in samples.

This is the shape of tellyboy's voice API: `tb_audio_voice_rom`, `tb_audio_stream_open`, `tb_audio_voice_fade`, `tb_audio_voice_set_gain`, `tb_audio_voice_position`. The microcontroller sink is therefore a direct mapping. Its detailed interface is left for implementation.

### 10.2 Gain and fades

Gain is a whole percentage, as tellyboy's `gain_percent`, so every platform computes the same level.

No level changes in a step. Every start, stop, gain change and loss boundary ramps over at least **5 ms**. That is tellyboy's `TB_AUDIO_DECLICK_MS`, adopted because tellyboy measured a level step as a click.

The desktop sink mixes in integers:
- each voice's samples are scaled by its gain and summed in 32 bits;
- the sum is clamped;
- master volume and mute, both user controls, are applied last.

Resampling to the output device is the sink's own business and outside conformance.

### 10.3 Ducking

Audetel's fade byte lowered the programme sound while the description spoke (EN 300 708 table 19). A play's *duck* field applies the same attenuation steps (0, 4, 8, 12, 18, 24 or 30 dB) to every other channel while its voice plays, and to the programme sound on a receiver that has one.

Gain is a whole percentage, so the sink applies each step as the percentage nearest to it: 63, 40, 25, 13, 6 and 3 per cent. The ramp in and out is the command's fade time.

### 10.4 Fewer voices than channels

A receiver with fewer than four voices plays the lowest-numbered channels and ignores the rest, as a smaller MUSCOT receiver kept the first parts. Publishers put the most important sound on channel 0, for example a description or speech.

---

## 11. tellyboy: what the microcontroller output can do

These are tellyboy's constraints as built (`<tellyboy/audio.h>`; `components/AUDIO-REALTIME-DESIGN.md`; `components/audio-sd/tests/BASELINE.md`). The profile is shaped to fit them.

| Constraint | Value | Effect on this profile |
|---|---|---|
| Output | **Mono**, one pin, a 1-bit sigma-delta stream that the receiver's input filter turns back into sound | Objects are mono (§3.2) |
| Output rate | 32 000 Hz nominal: 31 746 achieved on ESP32 and SAMD21, 32 000 on RP2350 and STM32H7; audio up to about 15 kHz | 32 kHz is the top rate (§3.2) |
| Source formats | u8, s8, s16le, IMA ADPCM (WAVE blocks or raw codes), mono or stereo | Both of the profile's formats play natively, so nothing needs adding to tellyboy. G.711, G.726 and the other ETS 300 149 encodings are **missing**, which is one reason they aren't carried. |
| Source rates | 8 000–48 000 Hz, resampled by linear interpolation with **no anti-imaging filter** | An 8 kHz source carries images of its band above 4 kHz, which are audible as fizz. 16 or 22.05 kHz sounds better on tellyboy. |
| Voices | **4**, fixed at compile time (`TB_AUDIO_VOICES`), shared with whatever else the device plays | Four channels at most. A device that keeps voices for itself offers fewer (§10.4). |
| Gain | Whole percentages; line level 16 % by default, 50 % at most; changes glide over 5 ms | Gain is a percentage, and every change ramps (§10.2) |
| Output noise | First-order modulator (the default): **37.8 dB** in band. Second-order: **61.9 dB**. Both measured on a sine at 1/6 full scale, oversample 60. | The output stage limits quality, not the codec |
| IMA ADPCM quality | 30.5 dB SNR from ffmpeg's plain encoder; **34.2 dB** with `-trellis 12` (22 050 Hz, measured). Worst tenth of 20 ms segments 21.9 dB, at transients. | With the default modulator, IMA ADPCM (34 dB) is about as clean as the output (38 dB), so 16-bit PCM's 4× size buys almost nothing audible. Encoders SHOULD use trellis quantisation. |
| 8-bit PCM | Audible hiss under quiet passages (measured); widened to 16 bits on the way in | Not carried: 16-bit PCM and IMA ADPCM cover it better |
| IMA ADPCM cost | One 16 kHz voice: 2 124 cycles per refill more than PCM on a Cortex-M0+ (measured). Decoder state 144 B of RAM, decoder code about 870 B. | Affordable on the smallest part |
| Data placement | A playing sound's data must stay valid and readable from the interrupt, in internal RAM on ESP32 (`DRAM_ATTR`) | The store must not evict a playing object (§5) and lives in interrupt-readable RAM |
| Streams | The ring belongs to the client and is sized to its worst delivery gap. A stream starts at half full and needs `tb_audio_stream_end` to play its tail. | Progressive start and live sound map to stream voices (§8) |
| Clock drift | `tb_audio_stream_frame` tracks a source with its own clock, up to ±8 % | Satisfies §8.3 |
| Hard stop | `tb_audio_voice_stop` cuts at once and may click; `tb_audio_voice_fade_out` exists | Stops are fades (§7.2) |
| Simultaneous start of several voices | No such call | Version 1's *cue* is dropped (section 13) |

---

## 12. Receiver levels

| | Level A: microcontroller | Level B: desktop |
|---|---|---|
| Channels | at least 2 | 4 |
| Codecs | 16-bit PCM, IMA ADPCM | 16-bit PCM, IMA ADPCM |
| Sample rates | all of §3.2 | all of §3.2 |
| Object store | at least 16 KB (4 s of IMA ADPCM at 8 kHz) | at least 4 MB |
| Records being reassembled | 2 | 8 |
| Objects held | 8 | 256 |
| Progressive and live sound | network only | yes |

A receiver publishes its actual figures.

The Level A numbers are proposals. They must be checked against the RAM the teletext renderer leaves on its smallest microcontroller target (open decision 5). The SAMD21's 32 KB, the floor of tellyboy's own budget, is too small to hold the renderer, the store and the stream rings at once.

---

## 13. Where this profile departs from the standards

| Standard | Departure | Reason |
|---|---|---|
| ETS 300 149 | Its framing is **not** used | It specifies sound for ISDN videotex terminals (§1), not teletext. Its length-indicator grammar (`x10DDDDD`, five data bits) contradicts its own prose (six bits, §6.3, p. 12), so it can't be implemented without guessing. IMA ADPCM, the one codec every target decodes, has no code in it: 2/7 to 2/14 "shall be ignored" (§7.2.2). Its sample packing, most significant bit first (§8, figure 2), contradicts WAVE IMA ADPCM's low-nibble-first order, so IMA ADPCM couldn't be carried in it even under a private code. A tool MAY convert ETS 300 149 sound to this profile. |
| ETS 300 149 §7.2.2 codecs | None carried: not G.711 A-law or µ-law, not G.721/G.723/G.726 ADPCM | No teletext service ever carried audio in them (Audetel used its own coder), and this profile doesn't use ETS 300 149's framing, so carrying them would be compatible with nothing. Content in them is converted to PCM or IMA ADPCM when it is published. G.726 can't be joined midway or survive a loss (§3.1). G.711 is twice IMA ADPCM's size for a few dB more. tellyboy decodes neither. |
| Version 1's 8 kHz only | The common rates, 8–48 kHz | The owner's authorisation (§3.2). 8 kHz is the telephone band of the 1992 encodings. tellyboy accepts 8–48 kHz, and Audetel already went wideband at 16 kHz (Black 1995). |
| No teletext or videotex standard | 16-bit linear PCM (code 0) | The only PCM any of them names is G.711's companded 8-bit PCM (ETS 300 149 §7.2.2, codes 2/0 and 2/1). Linear PCM is the WAVE and AES baseline, and tellyboy plays it natively. EN 300 708 §6.7 doesn't constrain the codec at all, so carrying it there breaks no standard. |
| ETS 300 149 §9.1 (ADPCM at 32 kbit/s mandatory) | IMA ADPCM in place of G.721/G.726 | The same family, bit depth and rate (4 bits at 8 kHz = 32 kbit/s), but a different algorithm, so IMA ADPCM is not ETS-conformant. It is chosen because its blocks decode independently (§3.1) and tellyboy decodes it on every ISA. |
| EN 300 708 §6.7.2 Audetel | Its coder is not used; its channel layout, frame timing and fade semantics are | The PRELP coder was never published. The packet structure (§6.1) and ducking (§10.3) follow the standard. |
| EN 300 708 §6.7.3 | The user-defined service's control byte and data are defined here | The standard leaves both to the application (§6.7.3.2). |
| MUSCOT, and note formats generally | Not adopted | The owner's decision (19 October 2026): the profile carries PCM and ADPCM only. Only MUSCOT's channel-priority idea is kept (§10.4). |
| Version 1 | JSON, string identities, sessions, command history, cancellation barriers, cue groups, a queue of any length, the *wait* loss policy | Each needed memory proportional to history or to its input, which rules out a microcontroller. Each is replaced by fixed state: §7.1's sequence rule, a single pending slot, and independent blocks. |

---

## 14. Decisions taken, and those still open

Taken on 19 October 2026, while the owner was away, as the owner had instructed:
- **The broadcast carrier** is EN 300 708 §6.7's user-defined service on data channel 12 (decision E.84). IDL Format B remains the alternative if an EBU Application Identifier is ever allocated.
- **Page sounds are on by default** in T42 Browser, which has no settings screen for them yet (`TECH-DEBT.md`).
- **The crates live in the engine** (decision E.85).

Decided by the owner on 20 October 2026 (decision E.89):
1. **No speech codec.** Live sound over broadcast is IMA ADPCM or nothing; code 2 is reserved like the rest.
3. **No simultaneous start** is added to tellyboy; cue groups stay dropped.
4. **The IMA 1992 recommendation and Microsoft's 1994 RIFF/WAVE update are obtained** (section 2), so the codec can be checked against its source.

Still open:
2. **Level A's numbers** against the renderer's smallest target, once the microcontroller build exists.

---

## 15. Required validation

Before a receiver is called conformant:

- **Records:** every bound, a CRC failure, an unknown type, a short body, unknown extension tags, and every OBJECT and DATA conflict. Fuzz the record parser and the fragment reassembler.
- **Broadcast:** reassembly with fragments lost, repeated and reordered; record-id wrap; CRC-16 against EN 300 708 §6.5.8.1; the oldest incomplete record dropped at the reassembly limit.
- **Codecs:** IMA ADPCM against the IMA 1992 recommendation's arithmetic, and the same bytes decoded identically by the Rust core and by tellyboy; every rate; one-block and many-block objects; a short final block.
- **Control:** a stop before an older play; an older stop after a newer play; repeats after completion; wrap of the sequence numbers; the pending slot replaced; loop.
- **Page binding:** play on each entry, fade on exit, no play on entry without an armed command, no replay from a repeated record, nothing while page sounds is off.
- **Loss:** a missing middle block gives exact silence for its range, with 5 ms fades and unchanged later timing; a late block doesn't play; join at any block.
- **Live:** ±1 % sender clock error absorbed without underrun; fade after 2 s of silence.
- **Output:** the desktop sink and tellyboy, given the same object, gain and fades, produce the same sample-level envelope; a duck step attenuates by its table 19 value.

This document is not evidence that any receiver passes these checks.
