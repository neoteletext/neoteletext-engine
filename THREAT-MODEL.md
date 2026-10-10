# Threat model

This is the threat model REQ-SEC-002 requires. Security overrides every other requirement (decision E.32). Review and update it at the end of every milestone, and whenever an input source or output is added.

## Assets

| Asset | Why it matters |
|---|---|
| The host device and its resources | A decoder bug must not become code execution, file access or denial of service. |
| The user's session cookies and form data | They identify the user to web teletext services. |
| The user's trust in what is displayed | A page must not be able to fake receiver messages, security markers or link destinations. |
| The user's local network | A web-native client on a home network can be used to reach routers and other devices. |
| The user's terminal | Escape sequences in output can rewrite the screen, change titles or worse. |
| The integrity of the build and release | Dependencies and tools can be replaced by attackers. |
| The user's privacy | Requests, cookies and history reveal what the user reads. |

## Attackers

1. **Anyone able to supply input**: a file, a broadcast or recording, a web service, a pushed message or a live feed.
2. **Anyone on the network path**, especially for plain HTTP, which is allowed (decision E.32).
3. **The software supply chain**: dependencies, toolchains and build tools.

## Trust boundaries and protections

| Boundary | Attacker | Asset | Protection |
|---|---|---|---|
| Untrusted bytes into the format readers (L0) | 1 | Host | Streaming parsers with bounded memory, no panics, checked arithmetic, fuzzing (REQ-GEN-008, REQ-SEC-003, REQ-TST-040). |
| Packets into acquisition (L1, L2) | 1 | Host | Fixed-size store over caller memory, bounded work per packet (REQ-SEC-012). |
| Page content into the grid (L3) | 1 | Trust | Row 0 columns 0 to 7 cannot be written by any page route (REQ-SEC-005, E.5 (j)). |
| Output to the terminal (L5) | 1 | Terminal | Allow-listed characters only; only our own fixed escape sequences; image data encoded from pixels (REQ-SEC-010, REQ-RND-029). |
| Untrusted bytes into the web user agent (L7) | 1, 2 | Host, network, cookies | Strict address parser, minimal user agent profile, strict HTTP framing, no compression, size and time limits, policy checks on every connection and redirect (REQ-WEB-014 to 016, REQ-SEC-006 to 011). |
| Plain HTTP connections | 2 | Cookies, form data, trust | Unencrypted marker in the status area, warning and confirmation before plain-HTTP submission, `Secure` cookies never sent, no downgrade (REQ-SEC-007, REQ-SEC-008). |
| Forms | 1 | Form data, trust | Origin shown, first submission confirmed, no password fields, same-origin targets only (REQ-SEC-009). |
| Links | 1 | Trust | Receiver-drawn link row, other services marked and confirmed (REQ-SEC-024, REQ-NAV-011, REQ-NAV-012). |
| The `ttx` process | 1 | Host | No `unsafe` code; the library crates have no operating-system access (REQ-SEC-016); Landlock and seccomp on Linux after inputs and outputs are open (REQ-SEC-017). |
| Fetcher to decoder (web mode) | 1, 2 | Host | Privilege separation (REQ-SEC-018): the fetcher process holds the network and TLS, the decoder holds the terminal and files. They speak a framed pipe protocol whose every frame is length-checked and validated by the receiver; the decoder ignores frame kinds only it may send. |
| Dependencies and tools | 3 | Build | Microcontroller crates have no dependencies beyond `embedded-io` and `httparse` (decision E.38); `Cargo.lock` committed; toolchain pinned (REQ-SEC-014). |

## Status at this milestone

| Item | State |
|---|---|
| REQ-SEC-005 status area | Enforced in the grid builder and the Level 1.5 walk, with tests. |
| REQ-SEC-010 terminal output | Allow-list and injection test in `ttx`. |
| REQ-SEC-016 no ambient authority | Checked by `cargo xtask check-io`. |
| REQ-SEC-017 Linux sandbox | Landlock (files, TCP) and seccomp (no programs, no new processes). macOS and Windows are Phase 2: `ttx` prints a notice (IMPLEMENTATION-LOG.md decision 15). |
| REQ-SEC-018 privilege separation | Done for every live input (`zxnet:`, `wss://`, `http://`, `https://`). On Linux both processes sandbox themselves: the fetcher may use the network and read only the resolver files (IMPLEMENTATION-LOG.md decision 28); the decoder has no network. Tested end to end against the local test server, including a form submission through the viewer. |
| REQ-SEC-024 links | The viewer ignores link keys unless row 24 is shown in full; tested in the core viewer tests. |
| REQ-SEC-014 supply chain | Lockfile and pinned toolchain in place. `cargo-deny`, the CycloneDX bill of materials and release signing are not set up yet. |
| REQ-TST-040 fuzzing | Not set up yet: needs `cargo-fuzz` and a nightly toolchain, which are not installed. |

## Review record

| Date | Milestone | Reviewer | Result |
|---|---|---|---|
| 10 October 2026 | Implementation in progress | Initial model | Gaps listed in the status table above. |
| 10 October 2026 | amd64 and iTerm2 implementation complete | Implementation review | REQ-SEC-018 and REQ-SEC-024 done. Open: fuzzing, `cargo-deny`, the bill of materials, release signing, and the macOS and Windows sandboxes. |
