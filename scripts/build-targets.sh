#!/usr/bin/env bash
# Builds the crates for every target of REQ-GEN-015 and prints a result
# table; exit status 1 if any target fails. Runs on the Linux build host
# (scripts/check.sh full runs it there through NEOTELETEXT_REMOTE).
#
# Mode "build" compiles and links; "test" also runs the tests under qemu
# user-mode emulation; "check" compiles without linking or code generation,
# for targets whose SDK cannot exist on this host (Apple, Windows MSVC and
# ESP-IDF). The Apple targets' neottx program is built and tested on the Mac by
# scripts/check.sh full. neottx for Windows is built with the MinGW toolchain,
# because its TLS library's C code needs the Windows SDK headers under MSVC
# (IMPLEMENTATION-LOG.md decision 39).
set -uo pipefail
cd "$(dirname "$0")/.."
. "$HOME/.cargo/env"

# REQ-GEN-001: the pinned toolchain (rust-toolchain.toml).
readonly RUST="1.99.0"
# Espressif's fork for Xtensa, installed with espup: 1.94 and 1.95 fail
# release builds (esp-rs/rust#277), so 1.93 is used, below the workspace's
# rust-version (IMPLEMENTATION-LOG.md decision 38).
readonly ESP="esp"

# The crates that run on bare metal (REQ-GEN-017), the pair built for
# WebAssembly, and the std crates for operating systems without a terminal.
readonly BARE="-p neoteletext-core -p neoteletext-formats -p neoteletext-slicer -p neoteletext-web -p neoteletext-image -p neoteletext-audio"
readonly WASM="-p neoteletext-core -p neoteletext-formats"
readonly LIBRARIES_STD="$BARE --features neoteletext-formats/std,neoteletext-image/std"
# Everything except the host-only xtask: the libraries and the neottx program.
readonly ALL="--workspace --exclude xtask"

# target | crates | mode | toolchain | extra flags
readonly MATRIX="
thumbv6m-none-eabi|$BARE|build|$RUST|
thumbv7em-none-eabihf|$BARE|build|$RUST|
thumbv8m.main-none-eabihf|$BARE|build|$RUST|
riscv32imc-unknown-none-elf|$BARE|build|$RUST|
riscv32imac-unknown-none-elf|$BARE|build|$RUST|
riscv32imafc-unknown-none-elf|$BARE|build|$RUST|
xtensa-esp32-none-elf|$BARE|build|$ESP|-Zbuild-std=core --ignore-rust-version
xtensa-esp32s3-none-elf|$BARE|build|$ESP|-Zbuild-std=core --ignore-rust-version
xtensa-esp32-espidf|$LIBRARIES_STD|check|$ESP|-Zbuild-std=std,panic_abort --ignore-rust-version
xtensa-esp32s3-espidf|$LIBRARIES_STD|check|$ESP|-Zbuild-std=std,panic_abort --ignore-rust-version
x86_64-unknown-none|$BARE|build|$RUST|
x86_64-unknown-linux-gnu|$ALL|build|$RUST|
aarch64-unknown-linux-gnu|$ALL|build|$RUST|
armv7-unknown-linux-gnueabihf|$ALL|build|$RUST|
riscv64gc-unknown-linux-gnu|$ALL|build|$RUST|
aarch64-unknown-linux-gnu|$BARE --all-features|test|$RUST|
armv7-unknown-linux-gnueabihf|$BARE --all-features|test|$RUST|
riscv64gc-unknown-linux-gnu|$BARE --all-features|test|$RUST|
x86_64-pc-windows-msvc|$LIBRARIES_STD|check|$RUST|
x86_64-pc-windows-gnu|$ALL|build|$RUST|
aarch64-apple-darwin|$LIBRARIES_STD|check|$RUST|
x86_64-apple-darwin|$LIBRARIES_STD|check|$RUST|
wasm32-unknown-unknown|$WASM|build|$RUST|
wasm32-wasip1|$WASM|build|$RUST|
"

# qemu runs the cross-compiled tests, with each target's C library.
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER="qemu-aarch64 -L /usr/aarch64-linux-gnu"
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUNNER="qemu-arm -L /usr/arm-linux-gnueabihf"
export CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_GNU_RUNNER="qemu-riscv64 -L /usr/riscv64-linux-gnu"

# Every standard library the matrix needs, for the pinned toolchain.
rustup -q target add --toolchain "$RUST" $(sed -n 's/^\([^|]*\)|.*|'"$RUST"'|.*$/\1/p' <<<"$MATRIX" | sort -u) >/dev/null

mkdir -p target/build-targets
failures=0
printf '%-32s %-6s %s\n' TARGET MODE RESULT
while IFS='|' read -r target crates mode toolchain extra; do
    [ -z "$target" ] && continue
    log="target/build-targets/$target.$mode.log"
    if ! rustup toolchain list | grep -q "^$toolchain"; then
        printf '%-32s %-6s %s\n' "$target" "$mode" "SKIPPED: toolchain $toolchain is not installed"
        continue
    fi
    runner_variable="CARGO_TARGET_$(tr 'a-z.-' 'A-Z__' <<<"$target")_RUNNER"
    if [ "$mode" = test ] && ! command -v "${!runner_variable%% *}" >/dev/null; then
        printf '%-32s %-6s %s\n' "$target" "$mode" "SKIPPED: ${!runner_variable%% *} is not installed"
        continue
    fi
    # shellcheck disable=SC2086 # the crate list and flags are split on purpose
    if cargo "+$toolchain" "$mode" --release --target "$target" $crates $extra >"$log" 2>&1; then
        result=OK
    else
        result="FAILED: $(grep -m1 -E '^error' "$log" || tail -n1 "$log")"
        failures=$((failures + 1))
    fi
    printf '%-32s %-6s %s\n' "$target" "$mode" "$result"
done <<<"$MATRIX"
exit $((failures > 0))
