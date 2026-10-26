"""Sources and streams for the Opus oracle (run_opus.sh, decision E.123).

  opus_streams.py signals DIR        write the synthetic sources, 48 kHz mono 16-bit raw
  opus_streams.py wav IN.wav OUT.raw  a local source (48 kHz mono 16-bit WAV) as raw samples
  opus_streams.py lose IN.bit OUT.bit the stream with packets lost: single packets, runs of
                                      three, and every 700 packets a run of 60, which passes the
                                      point where concealment turns from pitch to noise
  opus_streams.py hostile COUNT DIR   write COUNT streams of random profile packets: every 20 ms
                                      mono configuration, bare or padded, 2 to 160 random bytes,
                                      a tenth of them lost; the same streams on every run
  opus_streams.py seeds DIR OUT       the streams of DIR as fuzzing inputs for `opus_packets`: a
                                      length byte and the packet each, at most 4,096 bytes a file
  opus_streams.py differ A B          where two sample files first differ

Every signal is generated from a fixed seed, so a run is repeatable; none is a recording.
"""

import math
import struct
import sys
from pathlib import Path

RATE = 48_000
SECONDS = 15
SAMPLES = RATE * SECONDS
FULL_SCALE = 32_767


class Noise:
    """A linear congruential generator (Numerical Recipes' constants), uniform in -1..1."""

    def __init__(self, seed: int) -> None:
        self.state = seed

    def next(self) -> float:
        self.state = (self.state * 1_664_525 + 1_013_904_223) % 2**32
        return self.state / 2**31 - 1.0


def resonator(signal: list[float], frequency: float, bandwidth: float) -> list[float]:
    """A two-pole resonator at `frequency`, `bandwidth` wide: a formant."""
    radius = math.exp(-math.pi * bandwidth / RATE)
    a1 = -2 * radius * math.cos(2 * math.pi * frequency / RATE)
    a2 = radius * radius
    gain = 1 - radius
    out, y1, y2 = [], 0.0, 0.0
    for x in signal:
        y = gain * x - a1 * y1 - a2 * y2
        out.append(y)
        y1, y2 = y, y1
    return out


def voice(seconds: float, seed: int) -> list[float]:
    """Speech-like: a pulse train whose pitch glides, through three formants that move from
    syllable to syllable, with unvoiced bursts between."""
    noise = Noise(seed)
    count = int(RATE * seconds)
    source, phase = [], 0.0
    for n in range(count):
        t = n / RATE
        syllable = (t * 4) % 1
        voiced = syllable < 0.7
        pitch = 120 + 60 * math.sin(2 * math.pi * 0.7 * t)
        phase += pitch / RATE
        pulse = 1.0 if phase >= 1 else 0.0
        phase -= math.floor(phase)
        envelope = math.sin(math.pi * syllable / 0.7) if voiced else 0.3
        source.append(envelope * (pulse * 4 if voiced else noise.next()))
    shaped = [0.0] * count
    for f1, f2, f3 in ((700, 1200, 2600), (400, 2000, 2800)):
        for k, (a, b, c) in enumerate(zip(resonator(source, f1, 90), resonator(source, f2, 110), resonator(source, f3, 150))):
            shaped[k] += (a + 0.6 * b + 0.3 * c) * 0.5
    return shaped


def music(seconds: float, seed: int) -> list[float]:
    """Music-like: plucked chords of four notes with harmonics, a new chord every half second."""
    noise = Noise(seed)
    count = int(RATE * seconds)
    chords = ((220.0, 277.2, 329.6, 440.0), (196.0, 246.9, 293.7, 392.0), (174.6, 220.0, 261.6, 349.2), (246.9, 311.1, 370.0, 493.9))
    out = []
    for n in range(count):
        t = n / RATE
        chord = chords[int(t * 2) % len(chords)]
        since = (t * 2) % 1 / 2
        decay = math.exp(-since * 5)
        value = sum(math.sin(2 * math.pi * f * h * t) / h for f in chord for h in (1, 2, 3, 5)) * decay * 0.08
        out.append(value + 0.002 * noise.next())
    return out


def signals() -> dict[str, list[float]]:
    noise = Noise(12345)
    t = [n / RATE for n in range(SAMPLES)]
    half = SAMPLES // 2
    sweep_rate = math.log(20_000 / 20) / SECONDS
    switching: list[float] = []
    while len(switching) < SAMPLES:
        switching += voice(1.5, len(switching)) + music(1.5, len(switching)) + [0.0] * (RATE // 2)
    return {
        "silence": [0.0] * SAMPLES,
        "sine": [0.3 * math.sin(2 * math.pi * (440 if n < half else 3000) * t[n]) for n in range(SAMPLES)],
        "sweep": [0.3 * math.sin(2 * math.pi * 20 * (math.exp(sweep_rate * x) - 1) / sweep_rate) for x in t],
        "noise": [0.3 * noise.next() for _ in range(SAMPLES)],
        "clicks": [math.exp(-((n % (RATE // 4)) / 40)) * (0.9 if (n // (RATE // 4)) % 2 else -0.9) for n in range(SAMPLES)],
        "loud": [max(-1.0, min(1.0, 1.4 * math.copysign(1, math.sin(2 * math.pi * 220 * x)) + 0.05 * noise.next())) for x in t],
        "quiet": [0.001 * math.sin(2 * math.pi * 1000 * x) + 0.0003 * noise.next() for x in t],
        "tones": [0.2 * (math.sin(2 * math.pi * 697 * x) + math.sin(2 * math.pi * 1209 * x)) * ((x * 5) % 1 < 0.5) for x in t],
        "voice": voice(SECONDS, 7),
        "music": music(SECONDS, 9),
        "switching": switching[:SAMPLES],
    }


def write_raw(path: Path, signal: list[float]) -> None:
    path.write_bytes(b"".join(struct.pack("<h", max(-FULL_SCALE, min(FULL_SCALE, round(x * FULL_SCALE)))) for x in signal))


def wav(source: Path, target: Path) -> None:
    """A local source's samples; every chunk is checked, since the file is outside the
    repository's control."""
    data = source.read_bytes()
    if len(data) < 12 or data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise SystemExit(f"{source}: not a WAV file")
    at, fmt = 12, None
    while at + 8 <= len(data):
        chunk, size = data[at : at + 4], struct.unpack("<I", data[at + 4 : at + 8])[0]
        if at + 8 + size > len(data):
            raise SystemExit(f"{source}: chunk {chunk!r} runs past the end of the file")
        body = data[at + 8 : at + 8 + size]
        if chunk == b"fmt ":
            if size < 16:
                raise SystemExit(f"{source}: format chunk of {size} bytes, fewer than 16")
            fmt = struct.unpack("<HHIIHH", body[:16])
        elif chunk == b"data":
            if fmt is None or fmt[0] != 1 or fmt[1] != 1 or fmt[2] != RATE or fmt[5] != 16:
                raise SystemExit(f"{source}: must be 16-bit PCM, mono, {RATE} Hz")
            if size % 2:
                raise SystemExit(f"{source}: data of {size} bytes is not whole 16-bit samples")
            target.write_bytes(body)
            return
        at += 8 + size + (size & 1)
    raise SystemExit(f"{source}: no data chunk")


def lost(n: int) -> bool:
    return n % 37 == 36 or n % 101 >= 98 or n % 700 >= 640


def lose(source: Path, target: Path) -> None:
    data, out, at, n = source.read_bytes(), bytearray(), 0, 0
    while at + 8 <= len(data):
        length = struct.unpack(">I", data[at : at + 4])[0]
        range_bytes = data[at + 4 : at + 8]
        payload = data[at + 8 : at + 8 + length]
        at += 8 + length
        out += struct.pack(">I", 0) + range_bytes if lost(n) else struct.pack(">I", length) + range_bytes + payload
        n += 1
    target.write_bytes(bytes(out))


# The TOC bytes of every 20 ms mono configuration (RFC 6716 §3.1, Table 2): SILK NB, MB, WB,
# Hybrid SWB, FB and CELT NB, WB, SWB, FB.
CONFIGURATIONS = (1, 5, 9, 13, 15, 19, 23, 27, 31)
# The profile's largest packet (`neoteletext_opus::MAX_PACKET`).
MAX_PACKET = 160
# Random packets a hostile stream holds, and how many in ten are lost.
HOSTILE_PACKETS = 300
LOST_IN_TEN = 1
# The fuzzer's largest input (`-max_len` in the gate).
SEED_BYTES = 4096


def hostile(count: int, directory: Path) -> None:
    """Profile packets of random bytes: what a hostile carriage could send, decoded by libopus and
    the port alike. Each packet's bytes are drawn in one of four styles, since random bytes alone
    rarely exercise the long runs of ones or zeros that drive a decoder's limits."""
    directory.mkdir(parents=True, exist_ok=True)
    noise = Noise(2024)
    pick = lambda n: int((noise.next() + 1) / 2 * n) % n
    for stream in range(count):
        out = bytearray()
        for _ in range(HOSTILE_PACKETS):
            if pick(10) < LOST_IN_TEN:
                out += struct.pack(">II", 0, 0)
                continue
            toc = CONFIGURATIONS[pick(len(CONFIGURATIONS))] << 3
            style = pick(4)
            body = bytes(
                (pick(256), 0xFF if pick(8) else pick(256), 0x00 if pick(8) else pick(256), (index * 37) & 0xFF)[style]
                for index in range(1 + pick(MAX_PACKET - 1))
            )
            if pick(4) == 0 and len(body) + 2 < MAX_PACKET:
                # Code 3, one frame, padded (RFC 6716 §3.2.5): count byte, padding length, padding.
                padding = pick(MAX_PACKET - len(body) - 2)
                packet = bytes([toc | 3, 0x41, padding]) + body + bytes(padding)
            else:
                packet = bytes([toc]) + body
            packet = packet[:MAX_PACKET]
            out += struct.pack(">II", len(packet), 0) + packet
        (directory / f"hostile-{stream:03d}.bit").write_bytes(bytes(out))


def seeds(directory: Path, target: Path) -> None:
    target.mkdir(parents=True, exist_ok=True)
    for stream in sorted(directory.glob("*.bit")):
        data, at, record, part = stream.read_bytes(), 0, bytearray(), 0
        while at + 8 <= len(data):
            length = struct.unpack(">I", data[at : at + 4])[0]
            packet = data[at + 8 : at + 8 + length]
            at += 8 + length
            if len(record) + 1 + len(packet) > SEED_BYTES:
                (target / f"{stream.stem}-{part:03d}").write_bytes(bytes(record))
                record, part = bytearray(), part + 1
            record += bytes([len(packet)]) + packet
        if record:
            (target / f"{stream.stem}-{part:03d}").write_bytes(bytes(record))


def differ(a: Path, b: Path) -> None:
    x, y = a.read_bytes(), b.read_bytes()
    for i in range(0, min(len(x), len(y)), 2):
        if x[i : i + 2] != y[i : i + 2]:
            print(f"first difference at sample {i // 2} (frame {i // 2 // 960}): {struct.unpack('<h', x[i:i + 2])[0]} against {struct.unpack('<h', y[i:i + 2])[0]}")
            return
    print(f"lengths differ: {len(x) // 2} and {len(y) // 2} samples")


def main() -> None:
    command, *arguments = sys.argv[1:]
    if command == "signals":
        directory = Path(arguments[0])
        directory.mkdir(parents=True, exist_ok=True)
        for name, signal in signals().items():
            write_raw(directory / f"{name}.raw", signal)
    elif command == "wav":
        wav(Path(arguments[0]), Path(arguments[1]))
    elif command == "lose":
        lose(Path(arguments[0]), Path(arguments[1]))
    elif command == "hostile":
        hostile(int(arguments[0]), Path(arguments[1]))
    elif command == "seeds":
        seeds(Path(arguments[0]), Path(arguments[1]))
    elif command == "differ":
        differ(Path(arguments[0]), Path(arguments[1]))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
