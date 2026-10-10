#!/usr/bin/env bash
# Every local gate (decision 4 of IMPLEMENTATION-LOG.md).
#   scripts/check.sh fast   formatting, lints, tests, architecture checks
#   scripts/check.sh full   fast, plus the amd64 target builds, release
#                           tests and, if TELLYTXT_REMOTE names a Linux
#                           host, the x86_64 Linux tests, the build for
#                           every target (scripts/build-targets.sh) and the
#                           fuzzing targets run there (TELLYTXT_FUZZ_SECONDS
#                           each)
set -euo pipefail
cd "$(dirname "$0")/.."
mode="${1:-fast}"

step() { printf '\n== %s\n' "$*"; }

step "rustfmt (REQ-GEN-006)"
cargo fmt --all --check

step "clippy, warnings as errors (REQ-GEN-006)"
cargo clippy --workspace --all-targets --all-features -- -D warnings

step "tests (REQ-TST-001)"
cargo test --workspace --all-features --quiet

step "architecture and rule checks (REQ-ARCH-011, REQ-GEN-014, REQ-GEN-017, REQ-SEC-016)"
cargo xtask check-layers
cargo xtask check-constants
cargo xtask check-io
cargo xtask check-deps

step "original glyph set: format, rules, confusables, independence (decision E.62)"
cargo xtask glyph-check

if [ "$mode" = "full" ]; then
    step "no_std crates on the amd64 bare-metal target (REQ-GEN-002)"
    cargo build --quiet --target x86_64-unknown-none \
        -p tellytxt-core -p tellytxt-formats -p tellytxt-slicer -p tellytxt-web -p tellytxt-image

    step "each crate with no optional area and with each area alone (decision E.61)"
    cargo xtask features

    step "linked test image on the amd64 bare-metal target: every build links (decision E.61)"
    cargo xtask size --target x86_64-unknown-none

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

        step "every target of REQ-GEN-015 on $TELLYTXT_REMOTE"
        ssh "$TELLYTXT_REMOTE" 'cd tellytxt-check && scripts/build-targets.sh'

        step "linked Cortex-M0 test image on $TELLYTXT_REMOTE: size and symbols (REQ-GEN-009, REQ-GEN-019)"
        ssh "$TELLYTXT_REMOTE" 'cd tellytxt-check && . ~/.cargo/env && cargo xtask size'

        if [ "$deny_done" = no ] && ssh "$TELLYTXT_REMOTE" '. ~/.cargo/env && command -v cargo-deny >/dev/null'; then
            step "cargo-deny on $TELLYTXT_REMOTE: licences and advisories (REQ-GEN-017, REQ-SEC-014)"
            ssh "$TELLYTXT_REMOTE" 'cd tellytxt-check && . ~/.cargo/env && cargo deny check 2>&1 | tail -1; exit ${PIPESTATUS[0]}'
            deny_done=yes
        fi

        if ssh "$TELLYTXT_REMOTE" '. ~/.cargo/env && command -v cargo-llvm-cov >/dev/null'; then
            step "core line coverage on $TELLYTXT_REMOTE, at least 90% (REQ-TST-041)"
            ssh "$TELLYTXT_REMOTE" 'cd tellytxt-check && . ~/.cargo/env && \
                cargo llvm-cov -p tellytxt-core --summary-only --fail-under-lines 90 2>&1 | grep -E "^TOTAL"; \
                exit ${PIPESTATUS[0]}'
        else
            step "cargo-llvm-cov not installed on $TELLYTXT_REMOTE: coverage SKIPPED"
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
