# The Opus decoder: security analysis

The security analysis of decision E.123 and requirements REQ-AUD-022 to REQ-AUD-024: `neoteletext-opus`, the audio profile's Opus decoder, an integer-only port of libopus 1.5.2's fixed-point decoder that reads packets from broadcast, network and file carriage.

Written on 26 October 2026, after the decoder was built and integrated (commit 5ca20d8), from a review in three independent parts (the Opus layer with the packet parser, range decoder and tooling; CELT; SILK), fuzzing, and a comparison with the known vulnerabilities of audio decoders. It found no exploitable fault; it found three latent ones and some gaps, fixed in commits f5c6d9a, ce95ba6 and f9d3cb6 (section 6).

## 1. What is being protected

| Asset | Why it matters |
|---|---|
| The process that decodes | On a desktop, the decoder runs inside a port's process; on a microcontroller, it shares the only address space there is. A memory fault in it is a fault in everything around it. |
| The receiver's availability | A packet that hangs, crashes or starves the decoder stops sound, and on a microcontroller everything else. |
| What the listener hears | Output must be what the stream encodes, and never memory the decoder should not show. |
| The project's distribution | The decoder is ported from BSD-3-Clause code, and its oracle downloads and builds that code at test time. |

## 2. Trust boundaries

A packet crosses one boundary: from the carriage (EN 300 708 independent data lines, the network or a file, area AUD) into `Decoder::decode`. Every byte is untrusted (REQ-SEC-003, REQ-AUD-021). The decoder has no other input: no files, no network, no clock, no environment (REQ-SEC-016, checked by `cargo xtask check-io`). Its only outputs are the 960 samples it writes into the caller's `[i16; 960]` and its final range.

The oracle (`tests/oracles/run_opus.sh`) crosses a second boundary at test time: it downloads libopus over HTTPS and builds and runs it.

## 3. Threats and their answers

### 3.1 A malformed packet attacks memory
*Answer:* the crate is safe Rust with `#![forbid(unsafe_code)]`, so no input can make it read or write outside its own values; the output is a `&mut [i16; 960]`, so a caller cannot pass a buffer of the wrong size. Every index and slice taken from a stream value goes through `get`, a checked split, or a type whose length is fixed (the no-panic lint list, REQ-GEN-006).

### 3.2 A malformed packet panics the decoder
*Answer:* the crate root carries the engine's no-panic lint list (`indexing_slicing`, `arithmetic_side_effects`, `unwrap_used` and the rest), met without a site-level allow; the linked Cortex-M0 image at `opt-level = "z"` contains no panic path (REQ-GEN-019, `cargo xtask size`). Optimising for size keeps core's own checks in `copy_from_slice`, `copy_within`, `step_by`, `chunks_exact` and `clamp` out of line, so the decoder uses the forms of its `slices` module, which have no check to fail.

### 3.3 Arithmetic the C leaves undefined
Shifts by the type's width or more, sums that pass 2^31 and a range coder range of 0 are undefined in the C, and the lints of REQ-GEN-006 cannot see shifts.
*Answer:* every such case has a defined result in every build (REQ-GEN-008): sums wrap as two's complement; a shift past the width gives 0 to the left and the sign to the right (`fixed.rs`, `range.rs`); a range of 0 is a malformed stream and restarts (`range.rs`); a lag-0 autocorrelation that wraps is scaled down as the largest is (`celt/plc/lpc.rs`). The review found each of the last three reachable only from crafted internal state, never from a stream; each now has a test that failed on the old code.

### 3.4 Denial of service: time, loops and recursion
*Answer:* every loop is bounded by the frame's structure or by a table: pulse escapes at most 10, shell blocks 20, NLSF stabilisation 20 passes, the bandwidth expansion 16, the CELT partition recursion five deep (its band split halves the size and lowers the LM each time). The worst crafted packet decoded in under 1 ms on a desktop, against 0.05 ms for a typical one; on the Cortex-M0 the slowest frame of real speech took 0.79 million instructions at `opt-level = 3` (decision E.123). Fuzzing with a 10-second limit an input found no timeout.

### 3.5 Memory and stack
*Answer:* no allocation; the state is one value of 27,880 bytes, holding the scratch too; a decode needs about 2.6 KB of stack on the Cortex-M0 (REQ-AUD-024). A desktop build at full optimisation inlines more and measured about 17 KB of stack, so a port with a small thread stack should size it from the target's own build.

### 3.6 Stale or foreign data in the output
The claxon FLAC decoder's RUSTSEC-2018-0004 (CVE-2018-20992) is the class: a malformed stream left a buffer partly unwritten, and old memory reached the output.
*Answer:* every result but a refused packet writes all 960 samples, and every concealment does; a refused packet leaves the caller's buffer untouched, as `DecodeError::Packet` says, so a caller that plays it anyway replays its own last frame, never the decoder's memory. A unit test and the fuzz target check that a frame never depends on what the buffer held (REQ-AUD-023). Before the oracle, this was the one fault found in the decoder: a concealed CELT-only frame was added to the buffer rather than written.

### 3.7 State a hostile packet leaves for the next
*Answer:* each frame and each redundant frame starts a new range decoder; the decoder's modes, histories and filters only ever hold values the arithmetic of section 3.3 can produce, so a hostile packet can make the following frames sound wrong, as it would in the reference, but cannot make them fault. After 2,000 hostile packets and a `reset`, the decoder matched a new one; random hostile streams of 200 to 3,000 packets, with losses and mode changes, matched libopus sample for sample.

### 3.8 The known vulnerabilities of Opus decoders
- **CVE-2013-0899** (libopus before 1.0.2, found in Chrome): an integer overflow in the packet padding led to an out-of-bounds read. `packet.rs` refuses a packet over 160 bytes before reading its padding, adds each length with saturation and subtracts with a check; an exhaustive test of every padding layout up to 161 bytes found the frame always the exact slice.
- **CVE-2017-0381** (libopus before 1.1.4, Android): a wrap in SILK's NLSF stabilisation led to a read 256 bytes before the cosine table. The port clamps each NLSF to 0 to 32767 before stabilising and saturates the stabiliser's sum, as 1.1.4 does, and the cosine lookup cannot leave its table; crafted streams that drive the stabiliser's fallback 545 times decoded exactly as libopus.
- **libopus 1.2's "integer wrap-arounds discovered through fuzzing"**: every one of its ten fixes in SILK is present in the port.
- **CVE-2026-63633** (FreeRDP's Opus wrapper): a capacity check of the wrong buffer. Not possible here: the caller's buffer is a fixed-size array.
- **opusfile 0.11 and 0.12** (overflows, a stack overflow, a division by zero in Ogg stream handling): the engine reads no Ogg; the test tools that read streams bound every length.

### 3.9 The oracle's download
*Answer:* `run_opus.sh` fetches libopus over HTTPS only (`--proto =https --proto-redir =https`), checks the tarball against its pinned SHA-256 before anything in it runs, and builds it afresh on every run from the checked tarball, so nothing cached can stand in for it. It runs as separate programs at test time (REQ-TST-001) and nothing of it is kept in the repository.

### 3.10 The ported source
*Answer:* libopus is BSD-3-Clause; its notice travels with the crate (`LICENSE-libopus`, and the copyright line of each ported file), the crate is licensed `MIT AND BSD-3-Clause`, and CREDITS.md and LICENSING.md record it (decision E.123). The patent grants for Opus apply to any implementation of the specification.

## 4. Changes made to the requirements by this analysis

None to their statements. REQ-AUD-023's verification gains the oracle's hostile streams, and the fuzz target its comparison of two decoders.

## 5. Open points

- **Hardware.** The decoder has run only under QEMU; its timing and stack on a real Cortex-M0, and its build for the ESP32's Xtensa (REQ-GEN-015), are untested (TECH-DEBT.md).
- **Where the reference is undefined, the output is the port's.** The cases of section 3.3 have defined results that need not match any one build of libopus, whose result there depends on the compiler and processor. No valid stream reaches them.
- **SILK's own shifts** (`silk/macros.rs`) wrap their amount modulo 32, which cannot panic; the review bounded every shift that a stream can drive to 0 to 31.
- **A truncated frame is not reported.** As in the reference, the missing bits of a frame that runs short read as zeros and the frame decodes; only CELT's overrun is `Corrupt`.

## 6. As built (26 October 2026)

| Check | Result |
|---|---|
| Random hostile packets at the public decoder, debug builds with overflow checks | About 48 million packets in three reviews: no panic, no hang |
| Coverage-guided fuzzing with overflow checks and debug assertions | 25 minutes × 12 workers before the move, 45 minutes × 12 after it, and 25 minutes × 8 by the CELT review: no crash, no timeout |
| Hostile streams against libopus, sample for sample | About 8,000 random and crafted streams of 150 to 3,000 packets: no difference |
| `run_opus.sh` against libopus: encoder streams, received and with losses, and 40 hostile streams | 274 streams, 453,468 frames: no difference |
| The Cortex-M0 image's symbols | No panic path, no floating-point routine |

Findings and what was done:

| Finding | Severity | Done |
|---|---|---|
| A lag-0 autocorrelation that wraps shifts by -3 (`celt/plc/lpc.rs`) | Low: reached only from crafted state | Scaled down as the largest; test (f5c6d9a) |
| A range of 0 never normalises (`range.rs`) | Low: no caller makes one | A malformed stream, restarted; test (f5c6d9a) |
| Shifts of 32 bits or more panic in debug (`fixed.rs`, `range.rs`, `math.rs`) | Low: no caller passes one | Defined results; tests (f5c6d9a) |
| An unreachable SILK error left the last packet's state (`decoder.rs`) | Informational | The state moves on (ce95ba6) |
| The fuzz target used one decoder and no reset; the oracle compared only encoder streams; no fuzz seeds | Test gap | Two decoders, resets, hostile streams, seeds (f9d3cb6) |
| The size image rooted two of five entry points | Test gap | All five rooted (f9d3cb6) |
| The oracle reused a cached build unchecked, allowed a redirect from HTTPS, hid encoder failures | Low | Rebuilt each run, HTTPS only, failures reported (f9d3cb6) |
| The WAV reader of the oracle's tool raised on a malformed file | Informational | Each chunk checked, with a clear error (f9d3cb6) |
