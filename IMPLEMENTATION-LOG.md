# Implementation log

Questions that came up while implementing the system without the project owner present, and the decision taken for each. Behaviour where the standard is silent is recorded in REQUIREMENTS.md Part E as REQ-DOC-004 requires; this log covers implementation and scope choices.

Started 10 October 2026. Instruction: implement the whole system, targeting only amd64 and iTerm2 for now.

| # | Question | Decision | Reason |
|---|---|---|---|
| 1 | What does "amd64 only" mean for build and test? | Build and test for `x86_64-unknown-linux-gnu` (on `oldvga`) and `x86_64-apple-darwin` (on the Mac under Rosetta, where iTerm2 runs). The `no_std` crates are also built for `x86_64-unknown-none`, the amd64 bare-metal target, so they stay honestly `no_std`. Day-to-day iteration runs natively on the Mac. | iTerm2 exists only on macOS, so the amd64 build that iTerm2 users run is `x86_64-apple-darwin`. Linux is where the Phase 1 sandbox lives. |
| 2 | Which toolchain? | Rust 1.99.0, pinned in `rust-toolchain.toml`. | REQ-GEN-001; the current stable release. |
| 3 | Which terminal graphics protocols? | iTerm2 inline images only. kitty and Sixel are deferred. | Scope given by the project owner. |
| 4 | Continuous integration or local gates? | Local gates only: `scripts/check.sh` runs every check, and git hooks run it (fast on pre-commit, full on pre-push). | The owner's standing rule is "no CI unless asked"; it overrides the CI wording of REQ-TST-080. |
| 5 | Commit while working? | Work on branch `implementation`, one commit per completed part. | The instruction to finish the whole system unattended implies saving progress; a branch keeps `main` untouched. |
