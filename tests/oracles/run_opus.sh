#!/usr/bin/env bash
# REQ-AUD-023 (decision E.123): neoteletext-opus decodes every stream exactly as libopus 1.5.2's
# fixed-point decoder does, sample for sample and final range for final range, as received and
# with packets lost.
#   run_opus.sh
# libopus is downloaded at test time, checked against its pinned SHA-256 and built from source
# (a C compiler and make); it runs as separate programs and none of it is in the repository.
# The sources are generated here (opus_streams.py signals); 48 kHz mono 16-bit WAV files in
# $NEOTELETEXT_CORPUS/opus-sources/ are added when present, and never committed (REQ-TST-022).
# Each source is encoded by opus_demo at the profile's rate and frame length under every setting
# that changes the bitstream, and each stream decoded by opus_decode.c and by decode_bit.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
cache="${NEOTELETEXT_CORPUS:-$root/target}/opus-oracle"
readonly LIBOPUS=opus-1.5.2
readonly LIBOPUS_URL="https://downloads.xiph.org/releases/opus/$LIBOPUS.tar.gz"
readonly LIBOPUS_SHA256=65c1d2f78b9f2fb20082c38cbe47c951ad5839345876e46941612ee87f9a7ce1
# The profile (decision E.123): mono, 16 kbit/s, 20 ms frames, 48 kHz.
readonly RATE=48000 BITRATE=16000 FRAME_MS=20
# Every encoder setting that changes the bitstream: application, complexity, rate control,
# in-band FEC (whose data the decoder must read past without using) and discontinuous
# transmission.
readonly SETTINGS=(
    "audio-c0:audio -complexity 0"
    "audio-c5-cvbr:audio -complexity 5 -cvbr"
    "audio-c10:audio -complexity 10"
    "audio-c10-cbr:audio -complexity 10 -cbr"
    "voip-c10:voip -complexity 10"
    "lowdelay-c10:restricted-lowdelay -complexity 10"
    "voip-fec:voip -complexity 10 -inbandfec -loss 20"
    "audio-fec:audio -complexity 10 -inbandfec -loss 10"
    "voip-dtx:voip -complexity 10 -dtx"
)

sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }

mkdir -p "$cache"
tarball="$cache/$LIBOPUS.tar.gz"
if [ ! -f "$tarball" ]; then
    curl -sSfL -o "$tarball.part" "$LIBOPUS_URL"
    mv "$tarball.part" "$tarball"
fi
[ "$(sha256 "$tarball")" = "$LIBOPUS_SHA256" ] || { echo "$tarball: SHA-256 differs from the pin" >&2; exit 1; }
build="$cache/build-$LIBOPUS_SHA256"
if [ ! -x "$build/opus_demo" ]; then
    rm -rf "$cache/source"
    mkdir -p "$cache/source"
    tar -xzf "$tarball" -C "$cache/source"
    # The fixed-point build the decoder was ported from, with nothing the profile lacks.
    (cd "$cache/source/$LIBOPUS" && ./configure --quiet --enable-fixed-point --disable-shared --disable-doc \
        --disable-dred --disable-osce --disable-deep-plc >/dev/null && make --quiet -j4 opus_demo libopus.la >/dev/null)
    mkdir -p "$build"
    cp "$cache/source/$LIBOPUS/opus_demo" "$cache/source/$LIBOPUS/.libs/libopus.a" "$build/"
    cp -R "$cache/source/$LIBOPUS/include" "$build/include"
fi

work="$(mktemp -d "${TMPDIR:-/tmp}/opus-oracle.XXXXXX")"
trap 'rm -rf "$work"' EXIT
cc -O2 -Wall -Wextra -Werror -I"$build/include" "$here/opus_decode.c" "$build/libopus.a" -lm -o "$work/opus_decode"
(cd "$root" && cargo build --quiet --release -p neoteletext-opus --example decode_bit)
decode_bit="$root/target/release/examples/decode_bit"

mkdir -p "$work/sources"
python3 -I "$here/opus_streams.py" signals "$work/sources"
for wav in "${NEOTELETEXT_CORPUS:-/nonexistent}"/opus-sources/*.wav; do
    [ -f "$wav" ] || continue
    python3 -I "$here/opus_streams.py" wav "$wav" "$work/sources/local-$(basename "$wav" .wav).raw"
done

streams=0 comparisons=0 frames=0 failures=0
for source in "$work/sources"/*.raw; do
    name="$(basename "$source" .raw)"
    for setting in "${SETTINGS[@]}"; do
        label="${setting%%:*}"
        read -r application options <<< "${setting#*:}"
        stream="$work/$name-$label.bit"
        # shellcheck disable=SC2086 # the options are words
        "$build/opus_demo" -e "$application" "$RATE" 1 "$BITRATE" -framesize "$FRAME_MS" $options \
            "$source" "$stream" >/dev/null 2>&1
        streams=$((streams + 1))
        python3 -I "$here/opus_streams.py" lose "$stream" "$work/lossy.bit"
        for variant in received lossy; do
            input="$stream"
            [ "$variant" = lossy ] && input="$work/lossy.bit"
            if ! "$work/opus_decode" "$input" "$work/c.pcm" "$work/c.ranges" \
                || ! "$decode_bit" "$input" "$work/rust.pcm" "$work/rust.ranges"; then
                echo "FAIL $name $label $variant: a decoder stopped" >&2
                failures=$((failures + 1))
                continue
            fi
            if ! cmp -s "$work/c.pcm" "$work/rust.pcm"; then
                echo "FAIL $name $label $variant: $(python3 -I "$here/opus_streams.py" differ "$work/c.pcm" "$work/rust.pcm")" >&2
                failures=$((failures + 1))
            elif ! cmp -s "$work/c.ranges" "$work/rust.ranges"; then
                echo "FAIL $name $label $variant: final ranges differ" >&2
                failures=$((failures + 1))
            fi
            comparisons=$((comparisons + 1))
            frames=$((frames + $(wc -l < "$work/c.ranges")))
        done
    done
done
echo "opus oracle: $streams streams, $comparisons decodes compared, $frames frames, $failures differing"
[ "$failures" -eq 0 ]
