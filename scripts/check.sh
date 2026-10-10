#!/usr/bin/env bash
# Every local gate (decision 4 of IMPLEMENTATION-LOG.md).
#   scripts/check.sh fast   formatting, lints, tests, architecture checks
#   scripts/check.sh full   fast, plus the amd64 target builds, release
#                           tests and, if TELLYTXT_REMOTE names a Linux
#                           host, the x86_64 Linux tests and the fuzzing
#                           targets run there (TELLYTXT_FUZZ_SECONDS each)
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

    deny_done=no
    if command -v cargo-deny >/dev/null 2>&1; then
        step "cargo-deny: licences and advisories (REQ-GEN-017, REQ-SEC-014)"
        cargo deny check
        deny_done=yes
    fi

    if [ -n "${TELLYTXT_REMOTE:-}" ]; then
        step "x86_64 Linux tests on $TELLYTXT_REMOTE"
        rsync -a --delete --exclude target --exclude .git --exclude tmp --exclude tletxt.com \
            --exclude fuzz/corpus --exclude fuzz/artifacts ./ "$TELLYTXT_REMOTE:tellytxt-check/"
        ssh "$TELLYTXT_REMOTE" 'cd tellytxt-check && . ~/.cargo/env && cargo test --workspace --all-features --quiet'

        if [ "$deny_done" = no ] && ssh "$TELLYTXT_REMOTE" '. ~/.cargo/env && command -v cargo-deny >/dev/null'; then
            step "cargo-deny on $TELLYTXT_REMOTE: licences and advisories (REQ-GEN-017, REQ-SEC-014)"
            ssh "$TELLYTXT_REMOTE" 'cd tellytxt-check && . ~/.cargo/env && cargo deny check 2>&1 | tail -1; exit ${PIPESTATUS[0]}'
            deny_done=yes
        fi

        # REQ-TST-040: every target for a fixed time on each change, after
        # replaying every input that has ever found a fault.
        seconds="${TELLYTXT_FUZZ_SECONDS:-20}"
        if ssh "$TELLYTXT_REMOTE" '. ~/.cargo/env && command -v cargo-fuzz >/dev/null'; then
            step "fuzzing on $TELLYTXT_REMOTE, $seconds seconds per target (REQ-TST-040)"
            ssh "$TELLYTXT_REMOTE" "cd tellytxt-check/fuzz && . ~/.cargo/env && \
                cargo +nightly clippy --quiet -- -D warnings && \
                for target in \$(cargo +nightly fuzz list); do \
                    mkdir -p corpus/\$target regressions/\$target && \
                    cargo +nightly fuzz run \$target corpus/\$target regressions/\$target -- \
                        -max_total_time=$seconds -timeout=10 -rss_limit_mb=2048 > fuzz.log 2>&1 \
                        || { tail -40 fuzz.log; exit 1; }; \
                    echo \"\$target: \$(tail -1 fuzz.log)\"; \
                done"
        else
            step "cargo-fuzz not installed on $TELLYTXT_REMOTE: fuzzing SKIPPED"
        fi
    fi
fi

if [ "$mode" = "full" ] && [ "$deny_done" = no ]; then
    step "cargo-deny not installed here or on a remote host: licence and advisory checks SKIPPED"
fi

printf '\nall %s checks passed\n' "$mode"
