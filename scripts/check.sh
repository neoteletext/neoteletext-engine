#!/usr/bin/env bash
# Every local gate (decision 4 of IMPLEMENTATION-LOG.md).
#   scripts/check.sh fast   formatting, lints, tests, architecture checks
#   scripts/check.sh full   fast, plus the amd64 target builds, release
#                           tests and, if TELLYTXT_REMOTE names a Linux
#                           host, the x86_64 Linux tests run there
set -euo pipefail
cd "$(dirname "$0")/.."
mode="${1:-fast}"

step() { printf '\n== %s\n' "$*"; }

step "rustfmt (REQ-GEN-006)"
cargo fmt --all --check

step "clippy, warnings as errors (REQ-GEN-006)"
cargo clippy --workspace --all-targets --all-features -- -D warnings

step "tests"
cargo test --workspace --all-features --quiet

step "architecture and rule checks (REQ-ARCH-011, REQ-GEN-014, REQ-GEN-017, REQ-SEC-016)"
cargo xtask check-layers
cargo xtask check-constants
cargo xtask check-io
cargo xtask check-deps

if [ "$mode" = "full" ]; then
    step "no_std crates on the amd64 bare-metal target (REQ-GEN-002)"
    cargo build --quiet --target x86_64-unknown-none \
        -p tellytxt-core -p tellytxt-formats -p tellytxt-slicer -p tellytxt-web -p tellytxt-image

    step "release tests, including the exhaustive error-code tests"
    cargo test --workspace --all-features --release --quiet

    if [ "$(uname -s)" = "Darwin" ]; then
        step "x86_64-apple-darwin build and tests under Rosetta (iTerm2 target)"
        cargo test --workspace --all-features --quiet --target x86_64-apple-darwin
    fi

    if command -v cargo-deny >/dev/null 2>&1; then
        step "cargo-deny: licences and advisories (REQ-GEN-017, REQ-SEC-014)"
        cargo deny check
    else
        step "cargo-deny not installed: licence and advisory checks SKIPPED"
    fi

    if [ -n "${TELLYTXT_REMOTE:-}" ]; then
        step "x86_64 Linux tests on $TELLYTXT_REMOTE"
        rsync -a --delete --exclude target --exclude .git ./ "$TELLYTXT_REMOTE:tellytxt-check/"
        ssh "$TELLYTXT_REMOTE" 'cd tellytxt-check && . ~/.cargo/env && cargo test --workspace --all-features --quiet'
    fi
fi

printf '\nall %s checks passed\n' "$mode"
