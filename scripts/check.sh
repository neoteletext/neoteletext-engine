#!/usr/bin/env bash
# Every local gate (decision 4 of IMPLEMENTATION-LOG.md; tiers, row 96):
# the engine's own steps, on the gate machinery every neoteletext
# repository shares (scripts/gate.sh, copied from neoteletext-common;
# gates.md there).
#   scripts/check.sh commit  formatting, Clippy and the no-panic lint lists:
#                            the code compiles clean, on what is staged
#   scripts/check.sh push    the tests, the desktop audio profile, the
#                            architecture and rule checks, the
#                            documentation and the fonts: a push of
#                            livedevelopment
#   scripts/check.sh full    the amd64 target builds, the Opus oracle,
#                            release tests, Rosetta, machete, deny, and on
#                            the Linux host (NEOTELETEXT_REMOTE) alongside
#                            the x86_64 Linux tests, every target
#                            (scripts/build-targets.sh), size, coverage and
#                            fuzzing (NEOTELETEXT_FUZZ_SECONDS each): a push
#                            to main. The tools scripts/tools.sh pins are
#                            refused, here and on the host, before it starts
set -euo pipefail
cd "$(dirname "$0")/.."

# The Linux host of the neoteletext full gates (DEVELOPMENT.md section 12).
linux_host="${NEOTELETEXT_REMOTE:-foundintelquiet4060}"
# Every file is the engine, its checks or the documents they trace.
readonly CHECKED_PATHS=(.)

. scripts/gate.sh

. scripts/tools.sh

# The full gate's tools, here and on the Linux host: a missing one fails before
# anything long starts, not an hour in. The host's copy of the repository has a
# tools/ of its own, which the copy to it leaves alone.
check_full_tools() {
    local missing
    missing="$(tools_out_of_date)"
    [[ -z "$missing" ]] || fail "tools missing or at another version here; run scripts/tools.sh to install them:
$missing"
    need_host
    rsync --relative --no-times --checksum scripts/tools.sh rust-toolchain.toml "$linux_host:neoteletext-check/"
    missing="$(ssh "$linux_host" '. ~/.cargo/env && cd neoteletext-check && . scripts/tools.sh && tools_out_of_date')"
    [[ -z "$missing" ]] || fail "tools missing or at another version on $linux_host; run scripts/tools.sh in ~/neoteletext-check there to install them:
$missing"
}

# The audio profile with PCM16 out and the encoder in, and the test of REQ-AUD-001
# that needs both.
readonly AUDIO_WITHOUT_PCM16="idl,opus,encode"
readonly PCM16_OUT_TEST="without_pcm16_a_pcm_sound_is_unsupported_and_skipped"

# DEVELOPMENT.md section 6 (REQ-SEC-003): every crate root reads input from
# outside its process, and each carries one no-panic lint list, the same
# in all of them.
readonly NO_PANIC_ROOTS=(crates/neoteletext-core/src/lib.rs crates/neoteletext-formats/src/lib.rs
    crates/neoteletext-slicer/src/lib.rs crates/neoteletext-web/src/lib.rs crates/neoteletext-image/src/lib.rs
    crates/neoteletext-audio/src/lib.rs crates/neoteletext-audio-mix/src/lib.rs crates/neoteletext-opus/src/lib.rs
    crates/neoteletext-input/src/lib.rs
    crates/neoteletext-transport/src/lib.rs crates/neoteletext-qrkeyboard/src/lib.rs
    crates/neoteletext-app/src/lib.rs crates/neoteletext-cli/src/lib.rs
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


# DEC-1726 item 5: the owner expects to change the entry file's name and the
# link scheme, and documents name them literally, so they must say what the
# constants say.
check_document_names() {
    local entry_file link_scheme named
    entry_file="$(sed -n 's/^pub const SETTINGS_FILE: &str = "\(.*\)";$/\1/p' crates/neoteletext-web/src/channel.rs)"
    link_scheme="$(sed -n 's/^pub const LINK_SCHEME: &str = "\(.*\)";$/\1/p' crates/neoteletext-core/src/consts/link.rs)"
    [[ -n "$entry_file" && -n "$link_scheme" ]] || { echo "the constants were not found" >&2; return 1; }
    grep -qF "\`$link_scheme\`" docs/user-guide.md || { echo "docs/user-guide.md does not name $link_scheme" >&2; return 1; }
    grep -qF "$entry_file\`" docs/user-guide.md || { echo "docs/user-guide.md does not name $entry_file" >&2; return 1; }
    # The generated document marks a name as code with a tag, not a backtick.
    grep -qF "<code>$link_scheme" "$common/PROTOCOL.html" || { echo "$common/PROTOCOL.html does not name $link_scheme" >&2; return 1; }
    grep -qF "$entry_file</code>" "$common/PROTOCOL.html" || { echo "$common/PROTOCOL.html does not name $entry_file" >&2; return 1; }
    echo "ok: the documents name $entry_file and $link_scheme"
}

commit_steps() {
    step "the wire vectors are neoteletext-common's table (DEC-1759 lesson 3)"
    cmp "$common/wire-vectors.tsv" crates/neoteletext-web/src/wire_vectors.tsv

    step "rustfmt (REQ-GEN-006)"
    cargo fmt --all --check

    step "clippy, warnings as errors (REQ-GEN-006)"
    cargo clippy --workspace --all-targets --all-features -- -D warnings

    step "the no-panic lint list is the same in every crate root (REQ-SEC-003, DEVELOPMENT.md section 6)"
    check_no_panic_lists

    step "the documents name the entry file and the link scheme the code does (DEC-1726)"
    check_document_names
}

push_steps() {
    step "tests (REQ-TST-001)"
    cargo test --workspace --all-features --quiet

    # Decision E.131: the desktop builds compile PCM16 and IMA ADPCM out of the
    # audio profile and play Opus alone; microcontroller builds keep both, as the
    # workspace run above and scripts/build-targets.sh build them.
    step "the audio profile as the desktops build it: PCM16 and IMA ADPCM compiled out (decision E.131)"
    cargo clippy --quiet -p neoteletext-audio --all-targets --no-default-features --features idl,opus -- -D warnings
    cargo test --quiet -p neoteletext-audio --no-default-features --features idl,opus

    # tests/receiver.rs writes its sounds with the encoder, so the shape above
    # compiles it out, and with it the test REQ-AUD-001 names. The encoder brings
    # IMA ADPCM and leaves PCM16 out, so this shape holds that test.
    step "the audio profile without PCM16, with the encoder the receiver's tests need (REQ-AUD-001, decision E.131)"
    cargo clippy --quiet -p neoteletext-audio --all-targets --no-default-features --features "$AUDIO_WITHOUT_PCM16" -- -D warnings
    cargo test --quiet -p neoteletext-audio --no-default-features --features "$AUDIO_WITHOUT_PCM16" --test receiver
    local listed
    listed="$(cargo test --quiet -p neoteletext-audio --no-default-features --features "$AUDIO_WITHOUT_PCM16" --test receiver -- --list)"
    [[ "$listed" == *"$PCM16_OUT_TEST: test"* ]] || fail "$PCM16_OUT_TEST is compiled out of this shape, so no gate runs it"

    # gates.md: a pass says nothing of a test that is ignored, so the gate says
    # which ones, each with what would make it run.
    step "the tests that have not run: each ignored test and its reason (gates.md)"
    git grep -n -A1 -F '#[ignore' -- 'crates/*.rs' 'xtask/*.rs'

    step "architecture and rule checks (REQ-ARCH-011, REQ-GEN-014, REQ-GEN-017, REQ-SEC-016)"
    cargo xtask check-layers
    cargo xtask check-constants
    cargo xtask check-io
    cargo xtask check-deps

    step "documentation, warnings as errors (REQ-DOC-001)"
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --quiet

    step "the compiled font: the Noto Teletext files match their recorded SHA-256 (REQ-FNT-001, REQ-FNT-012)"
    cargo xtask check-noto
}

linux_steps() {
    step "x86_64 Linux tests on $linux_host"
    # Compared by content and written with the host's own times: with this
    # machine's times kept, a file edited here before the host's last build
    # looks older than that build, and Cargo reuses it.
    rsync -a --no-times --checksum --delete --exclude target --exclude /tools --exclude .git --exclude tmp --exclude tletxt.com \
        --exclude fuzz/corpus --exclude fuzz/artifacts ./ "$linux_host:neoteletext-check/"
    # The Opus oracle's streams seed the fuzzer, which reaches the decoder's deep paths only
    # from real packets; the corpus itself is the host's and is never sent back.
    opus_seeds="${NEOTELETEXT_CORPUS:-target}/opus-oracle/fuzz-seeds"
    if [ -d "$opus_seeds" ]; then
        ssh "$linux_host" 'mkdir -p neoteletext-check/fuzz/corpus/opus_packets'
        rsync -a "$opus_seeds/" "$linux_host:neoteletext-check/fuzz/corpus/opus_packets/"
    fi
    ssh "$linux_host" 'cd neoteletext-check && . ~/.cargo/env && cargo test --workspace --all-features --quiet'

    step "every target of REQ-GEN-015 on $linux_host"
    ssh "$linux_host" 'cd neoteletext-check && scripts/build-targets.sh'

    step "linked Cortex-M0 test image on $linux_host: size and symbols (REQ-GEN-009, REQ-GEN-019)"
    ssh "$linux_host" 'cd neoteletext-check && . ~/.cargo/env && cargo xtask size'

    step "core line coverage on $linux_host, at least 90% (REQ-TST-041)"
    ssh "$linux_host" 'cd neoteletext-check && . ~/.cargo/env && . scripts/tools.sh && \
        cargo llvm-cov -p neoteletext-core --summary-only --fail-under-lines 90 > coverage.log 2>&1 \
            || { tail -40 coverage.log; exit 1; }; \
        grep -E "^TOTAL" coverage.log'

    # REQ-TST-040: every target for a fixed time on each change, after
    # replaying every input that has ever found a fault.
    seconds="${NEOTELETEXT_FUZZ_SECONDS:-20}"
    step "fuzzing on $linux_host, $seconds seconds per target (REQ-TST-040)"
    ssh "$linux_host" "cd neoteletext-check && . ~/.cargo/env && . scripts/tools.sh && cd fuzz && \
        cargo +$FUZZ_NIGHTLY clippy --quiet -- -D warnings && \
        for target in \$(cargo +$FUZZ_NIGHTLY fuzz list); do \
            mkdir -p corpus/\$target regressions/\$target && \
            cargo +$FUZZ_NIGHTLY fuzz run \$target corpus/\$target regressions/\$target -- \
                -max_total_time=$seconds -timeout=10 -rss_limit_mb=2048 > fuzz.log 2>&1 \
                || { tail -40 fuzz.log; exit 1; }; \
            echo \"\$target: \$(tail -1 fuzz.log)\"; \
        done"
}

full_steps() {
    step "no_std crates on the amd64 bare-metal target (REQ-GEN-002)"
    cargo build --quiet --target x86_64-unknown-none \
        -p neoteletext-core -p neoteletext-formats -p neoteletext-slicer -p neoteletext-web -p neoteletext-image \
        -p neoteletext-audio -p neoteletext-audio-mix -p neoteletext-opus -p neoteletext-input

    step "each crate with no optional area and with each area alone (decision E.61)"
    cargo xtask features

    step "linked test image on the amd64 bare-metal target: every build links (decision E.61)"
    cargo xtask size --target x86_64-unknown-none

    # REQ-AUD-023: libopus is downloaded, pinned by checksum and built on first use, then
    # cached in $NEOTELETEXT_CORPUS (or target/); a failed download or build fails the gate.
    step "the Opus decoder against libopus 1.5.2 fixed point, every stream bit for bit (REQ-AUD-023)"
    tests/oracles/run_opus.sh

    # REQ-TST-022: the files of the corpus that can be fetched are, once, and
    # the tests that read only those run. The other corpus tests stay ignored:
    # no machine holds their recordings (TODO-2081).
    step "the corpus tests whose recordings can be fetched (REQ-TST-012, REQ-TST-022, REQ-L25-010)"
    local corpus="${NEOTELETEXT_CORPUS:-$PWD/target/corpus}"
    NEOTELETEXT_CORPUS="$corpus" cargo xtask corpus
    NEOTELETEXT_CORPUS="$corpus" cargo test --quiet -p neoteletext-formats --test corpus -- \
        --ignored --exact national_option_order_matches_broadcast_languages
    NEOTELETEXT_CORPUS="$corpus" cargo test --quiet -p neoteletext-cli --test level25_corpus -- \
        --ignored --exact polish_letter_from_x26

    step "release tests, including the exhaustive error-code tests"
    cargo test --workspace --all-features --release --quiet

    if [ "$(uname -s)" = "Darwin" ]; then
        step "x86_64-apple-darwin build and tests under Rosetta (iTerm2 target)"
        cargo test --workspace --all-features --quiet --target x86_64-apple-darwin
    fi

    # REQ-SEC-014: a dependency nothing uses is supply-chain risk for nothing.
    step "cargo-machete: no unused dependencies (REQ-SEC-014)"
    cargo machete

    step "cargo-deny: licences and advisories (REQ-GEN-017, REQ-SEC-014)"
    cargo deny check
}

if [[ "${1:-}" == full ]]; then check_full_tools; fi
gate_main "$@"
