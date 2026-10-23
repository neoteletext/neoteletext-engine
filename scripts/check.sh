#!/usr/bin/env bash
# Every local gate (decision 4 of IMPLEMENTATION-LOG.md).
#   scripts/check.sh fast   formatting, lints, tests, architecture checks;
#                           the pre-commit and pre-push hooks run it
#   scripts/check.sh full   fast, plus the amd64 target builds, release
#                           tests and, if NEOTELETEXT_REMOTE names a Linux
#                           host, the x86_64 Linux tests, the build for
#                           every target (scripts/build-targets.sh) and the
#                           fuzzing targets run there (NEOTELETEXT_FUZZ_SECONDS
#                           each). Run it before merging to main and
#                           before a release; it takes about 40 minutes
set -euo pipefail
cd "$(dirname "$0")/.."
mode="${1:-fast}"

step() { printf '\n== %s\n' "$*"; }

# DEVELOPMENT.md section 6 (REQ-SEC-003): every crate root reads input from
# outside its process, and each carries one no-panic lint list, the same
# in all of them.
readonly NO_PANIC_ROOTS=(crates/neoteletext-core/src/lib.rs crates/neoteletext-formats/src/lib.rs
    crates/neoteletext-slicer/src/lib.rs crates/neoteletext-web/src/lib.rs crates/neoteletext-image/src/lib.rs
    crates/neoteletext-audio/src/lib.rs crates/neoteletext-audio-mix/src/lib.rs crates/neoteletext-input/src/lib.rs
    crates/neoteletext-qrkeyboard/src/lib.rs crates/neoteletext-app/src/lib.rs crates/neoteletext-cli/src/lib.rs
    crates/neoteletext-cli/src/main.rs)

check_no_panic_lists() {
    local first="" root list listed
    for root in "${NO_PANIC_ROOTS[@]}"; do
        [[ -f "$root" ]] || { echo "$root is listed in NO_PANIC_ROOTS but does not exist" >&2; return 1; }
        list="$(sed -n '/^#!\[deny(/,/^)\]/p' "$root")"
        [[ -n "$list" ]] || { echo "$root has no #![deny(...)] list" >&2; return 1; }
        if [[ -z "$first" ]]; then
            first="$list"
        elif [[ "$list" != "$first" ]]; then
            echo "$root's #![deny(...)] list differs from ${NO_PANIC_ROOTS[0]}'s" >&2
            return 1
        fi
    done
    # A new crate root must be listed, so that none escapes the check.
    for root in crates/*/src/lib.rs crates/*/src/main.rs crates/*/src/bin/*.rs; do
        [[ -f "$root" ]] || continue
        listed=no
        for named in "${NO_PANIC_ROOTS[@]}"; do
            [[ "$named" == "$root" ]] && listed=yes
        done
        [[ "$listed" == yes ]] || { echo "$root is a crate root missing from NO_PANIC_ROOTS" >&2; return 1; }
    done
    echo "ok: ${#NO_PANIC_ROOTS[@]} crate roots carry the same list"
}

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

step "the no-panic lint list is the same in every crate root (REQ-SEC-003, DEVELOPMENT.md section 6)"
check_no_panic_lists

step "the bundled font: Bedstead's export matches its recorded SHA-256 (REQ-FNT-001)"
cargo xtask check-bedstead
cargo xtask check-noto

if [ "$mode" = "full" ]; then
    step "no_std crates on the amd64 bare-metal target (REQ-GEN-002)"
    cargo build --quiet --target x86_64-unknown-none \
        -p neoteletext-core -p neoteletext-formats -p neoteletext-slicer -p neoteletext-web -p neoteletext-image \
        -p neoteletext-audio -p neoteletext-audio-mix -p neoteletext-input

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

    # REQ-SEC-014: a dependency nothing uses is supply-chain risk for
    # nothing. cargo-machete is pinned at 0.9.2 (installed with --locked).
    if command -v cargo-machete >/dev/null 2>&1; then
        step "cargo-machete: no unused dependencies (REQ-SEC-014)"
        cargo machete
    else
        step "cargo-machete not installed: unused dependency check SKIPPED"
    fi

    deny_done=no
    if command -v cargo-deny >/dev/null 2>&1; then
        step "cargo-deny: licences and advisories (REQ-GEN-017, REQ-SEC-014)"
        cargo deny check
        deny_done=yes
    fi

    if [ -n "${NEOTELETEXT_REMOTE:-}" ]; then
        step "x86_64 Linux tests on $NEOTELETEXT_REMOTE"
        rsync -a --delete --exclude target --exclude .git --exclude tmp --exclude tletxt.com \
            --exclude fuzz/corpus --exclude fuzz/artifacts ./ "$NEOTELETEXT_REMOTE:neoteletext-check/"
        ssh "$NEOTELETEXT_REMOTE" 'cd neoteletext-check && . ~/.cargo/env && cargo test --workspace --all-features --quiet'

        step "every target of REQ-GEN-015 on $NEOTELETEXT_REMOTE"
        ssh "$NEOTELETEXT_REMOTE" 'cd neoteletext-check && scripts/build-targets.sh'

        step "linked Cortex-M0 test image on $NEOTELETEXT_REMOTE: size and symbols (REQ-GEN-009, REQ-GEN-019)"
        ssh "$NEOTELETEXT_REMOTE" 'cd neoteletext-check && . ~/.cargo/env && cargo xtask size'

        if [ "$deny_done" = no ] && ssh "$NEOTELETEXT_REMOTE" '. ~/.cargo/env && command -v cargo-deny >/dev/null'; then
            step "cargo-deny on $NEOTELETEXT_REMOTE: licences and advisories (REQ-GEN-017, REQ-SEC-014)"
            ssh "$NEOTELETEXT_REMOTE" 'cd neoteletext-check && . ~/.cargo/env && cargo deny check 2>&1 | tail -1; exit ${PIPESTATUS[0]}'
            deny_done=yes
        fi

        if ssh "$NEOTELETEXT_REMOTE" '. ~/.cargo/env && command -v cargo-llvm-cov >/dev/null'; then
            step "core line coverage on $NEOTELETEXT_REMOTE, at least 90% (REQ-TST-041)"
            ssh "$NEOTELETEXT_REMOTE" 'cd neoteletext-check && . ~/.cargo/env && \
                cargo llvm-cov -p neoteletext-core --summary-only --fail-under-lines 90 2>&1 | grep -E "^TOTAL"; \
                exit ${PIPESTATUS[0]}'
        else
            step "cargo-llvm-cov not installed on $NEOTELETEXT_REMOTE: coverage SKIPPED"
        fi

        # REQ-TST-040: every target for a fixed time on each change, after
        # replaying every input that has ever found a fault.
        seconds="${NEOTELETEXT_FUZZ_SECONDS:-20}"
        if ssh "$NEOTELETEXT_REMOTE" '. ~/.cargo/env && command -v cargo-fuzz >/dev/null'; then
            step "fuzzing on $NEOTELETEXT_REMOTE, $seconds seconds per target (REQ-TST-040)"
            ssh "$NEOTELETEXT_REMOTE" "cd neoteletext-check/fuzz && . ~/.cargo/env && \
                cargo +nightly clippy --quiet -- -D warnings && \
                for target in \$(cargo +nightly fuzz list); do \
                    mkdir -p corpus/\$target regressions/\$target && \
                    cargo +nightly fuzz run \$target corpus/\$target regressions/\$target -- \
                        -max_total_time=$seconds -timeout=10 -rss_limit_mb=2048 > fuzz.log 2>&1 \
                        || { tail -40 fuzz.log; exit 1; }; \
                    echo \"\$target: \$(tail -1 fuzz.log)\"; \
                done"
        else
            step "cargo-fuzz not installed on $NEOTELETEXT_REMOTE: fuzzing SKIPPED"
        fi
    fi
fi

if [ "$mode" = "full" ] && [ "$deny_done" = no ]; then
    step "cargo-deny not installed here or on a remote host: licence and advisory checks SKIPPED"
fi

printf '\nall %s checks passed\n' "$mode"
