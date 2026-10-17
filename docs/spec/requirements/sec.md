# F.0.3 Security and sandboxing (SEC) — this area overrides all others

Every requirement in this area takes precedence over any other requirement in this document (decision E.32). The requirements are numbered and verified like all others, and are tested in every milestone.

**REQ-SEC-001 — Security takes precedence.**
Where any requirement, decision or profile conflicts with a requirement of this area, this area shall prevail. The conflict shall be recorded as a decision in Part E, and the other requirement amended to remove it. No option, profile or configuration may switch off a requirement of this area. The only exceptions are the explicitly configurable points named in this area, such as the list of permitted local-network services in REQ-SEC-006.
*Verification:* Inspection at each milestone's security review (REQ-SEC-023).

**REQ-SEC-002 — The threat model.**
The repository shall contain a threat model in `THREAT-MODEL.md`. It shall list:
- **Assets.** These include:
  - the host device and its resources;
  - the user's session cookies and form data;
  - the user's trust in what is displayed;
  - the user's local network;
  - the user's terminal;
  - the integrity of the build and release;
  - the user's privacy.
- **Attackers.** These include:
  - anyone able to supply input, such as a file, a stream, a web service or a live feed;
  - anyone on the network path;
  - the software supply chain.
- **Trust boundaries**, and the requirement of this area that protects each asset from each attacker.

It shall be reviewed and updated at the end of every milestone, and whenever a new input source or output is added.
*Verification:* Inspection.

**REQ-SEC-003 — All input is untrusted.**
Everything the software receives shall be treated as hostile, except the configuration supplied by the user or port. This covers:
- every packet, file, stream and sample buffer;
- every web response and pushed message;
- every service descriptor and form;
- every live feed;
- every cookie.

The robustness requirements (REQ-GEN-008) and the fuzzing requirement (REQ-TST-040) apply to all of these without exception. No source is trusted more because of where it comes from, including the zxnet live feed (REQ-INP-022).
*Verification:* Inspection of the threat model; the fuzzing targets.

**REQ-SEC-004 — No executable content, ever.**
Pages, forms, descriptors, pushed messages and every other received content are data only. Nothing received shall ever be executed, interpreted as a script or used to load code or plugins. Forms (Appendix C) are declarative, and shall remain so. No future requirement may add scripting, macros or loadable code driven by content, unless this requirement is first changed by an explicit, recorded decision of the project owner.
*Verification:* Inspection.

**REQ-SEC-005 — The receiver-owned status area cannot be forged.**
Row 0, columns 0 to 7, belong to the receiver alone. Nothing a page transmits can write there:
- neither the header's own bytes;
- nor Level 1.5 overwrites (a triplet addressing those columns shall be discarded);
- nor any other data.

Every security indicator shall be shown only there:
- a marker for an unencrypted connection;
- the origin of a form while it has focus;
- the warning before submitting over plain HTTP.

While a security indicator is active, it shall be shown even on pages with bit C7 (Suppress Header) set.
*Reason:* Every warning in this area depends on the user being able to believe the status area. A page that could draw there could draw a fake "secure" marker.
*Verification:* Test that pages writing to row 0 columns 0 to 7 by every route leave the status area unchanged.

**REQ-SEC-006 — Protecting the local network.**
The web module shall refuse to connect to these addresses:
- **Loopback:** 127.0.0.0/8 and ::1.
- **Private:** 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 and fc00::/7.
- **Link-local:** 169.254.0.0/16 and fe80::/10.
- **Shared address space:** 100.64.0.0/10.
- **Unspecified and multicast addresses.**
- **The IPv4-mapped IPv6 forms of all of these.**

The check shall be applied:
- to the address the connection actually reaches, which the transport reports after name resolution (REQ-WEB-001);
- again after every redirect.

This covers descriptors, pages, push connections and form submissions. Local-network services can be allowed only by naming them explicitly in the configuration.
*Reason:* A web-native client on a home network is a classic target for requests forged to reach routers and other devices behind it ("server-side request forgery").
*Verification:* Test against a list of blocked addresses and encodings, including redirects and names that resolve to blocked addresses.

**REQ-SEC-007 — No downgrade.**
Once a service has been reached over HTTPS, every later connection for that service shall use encryption, for the rest of the session and for as long as its cookies last. This covers pages, push, redirects and form submissions. A redirect from HTTPS to plain HTTP shall be refused. A descriptor fetched over HTTPS shall not name plain `http://` or `ws://` addresses; if it does, the descriptor shall be rejected.
*Verification:* Test.

**REQ-SEC-008 — Plain HTTP, as decided.**
Plain HTTP is allowed for reading, cookies and forms (decision E.32). The following safeguards are mandatory:
- **Indicator.** An unencrypted connection shall be shown by the security indicator of REQ-SEC-005 whenever the page on screen came from plain HTTP.
- **Warning before submission.** Before every form submission over plain HTTP, the viewer shall see a warning that the entered values will travel unencrypted, and must confirm it.
- **`Secure` cookies.** Cookies marked `Secure` shall never be sent over plain HTTP.

*Verification:* Test.

**REQ-SEC-009 — Protecting against deceptive forms.**
- **Show where the form goes.** While a form has focus, the origin it submits to shall be shown by the security indicator.
- **Confirm the first submission.** The first submission to any origin shall require explicit confirmation showing that origin.
- **No password fields.** Forms shall offer no field type for passwords or secrets (Appendix C has none).
- **Restricted destinations.** Forms may submit only to the origins of REQ-WEB-008.

*Reason:* Any page can draw "enter your password". The user must always be able to see where their input will go.
*Verification:* Test.

**REQ-SEC-010 — Safe text and terminal output.**
- **Allow-list only.** The text and terminal renderers (REQ-RND-008 and REQ-RND-009) shall emit only an allow-list of printable characters. The terminal graphics renderer (REQ-RND-029) writes no page text at all, only image data it encodes itself.
- **Stripped characters.** Every C0 and C1 control character and DEL shall be removed, as shall every Unicode bidirectional control (U+061C, U+200E, U+200F, U+202A to U+202E and U+2066 to U+2069). Mapped characters from the Hebrew and Arabic sets are laid out by the renderer, not by embedded controls.
- **Own escape sequences only.** The terminal renderer shall emit no escape sequences except those it generates itself, from a fixed list.
- **Safe file names.** File names derived from received content, such as a service name or page title, shall be reduced to a safe character set and shall never contain path separators.

*Verification:* Test with a corpus of injection attempts through every character set.

**REQ-SEC-011 — Strict HTTP handling.**
- **No compression.** The web module shall ask for uncompressed responses (`Accept-Encoding: identity`) and reject compressed ones, so that no response can expand beyond the size limits.
- **Unambiguous framing.** It shall reject responses whose framing is ambiguous:
  - both `Content-Length` and chunked encoding;
  - conflicting `Content-Length` values;
  - invalid chunk sizes;
  - obsolete header folding.
- **Header limits.** It shall limit the number and total size of headers.
- **Time limits.** Every connection and request shall have time limits.

*Verification:* Test with a corpus of malformed and ambiguous responses; fuzzing.

**REQ-SEC-012 — Bounded work and fair shares.**
The work done for each packet shall have a fixed upper bound. The work done for each network message shall be at most proportional to its size, within its size limit. No input may cause work that grows faster, such as quadratic searches of the page store. When several sources feed one decoder, each shall get a fair share of processing, so that a hostile source cannot starve the others.
*Verification:* Tests with worst-case inputs that measure operation counts; the performance tests of REQ-TST-050.

**REQ-SEC-013 — Cookies and privacy.**
- **Not saved by default.** The cookie store shall not be saved between runs unless the port enables it. When it is saved, expired cookies and session cookies (those with no expiry) shall never be written. The porting guide shall tell ports to store the saved cookies as secrets.
- **Wipeable.** The store shall have a function that erases it.
- **Private requests.** The web module shall send no `Referer` header, and shall accept and send cookies only as REQ-WEB-006 allows. Pre-fetching (REQ-NAV-004) shall fetch only from the service of the page being viewed.

*Verification:* Test.

**REQ-SEC-014 — Supply chain.**
- **Locked dependencies.** `Cargo.lock` shall be committed.
- **Checked dependencies.** Dependencies shall be checked by `cargo-deny` on every change, and for new security advisories on a daily schedule (REQ-GEN-017).
- **Pinned CI actions.** Continuous integration actions shall be pinned to exact commit hashes.
- **Verified toolchains.** Toolchains, including Espressif's fork, shall be verified by checksum.
- **Signed releases.** Each release shall publish a software bill of materials in CycloneDX form, and shall be tagged and signed.
- **Reproducible builds.** Release builds should be reproducible, and the check shall be automated.

*Verification:* Inspection; continuous integration.

**REQ-SEC-015 — Unsafe code confined.**
Only the optional C interface crate (REQ-GEN-018) may contain `unsafe` code. It shall be kept as small as possible. It shall be tested under the address sanitiser and fuzzed through its C entry points.
*Verification:* Inspection; tests.

**REQ-SEC-016 — No ambient authority in the library.**
The library crates shall not use operating-system facilities of any kind. The library crates are `neoteletext-core`, `neoteletext-formats`, `neoteletext-slicer`, `neoteletext-web` and `neoteletext-image`, except the file-writing functions of the image writers, which take an already-open writer. They shall not use:
- `std::fs`, `std::net`, `std::process` or `std::env`;
- threads;
- the system clock.

All of these come only through the platform interface (REQ-ARCH-009). The rule shall be enforced by clippy's `disallowed-methods` and `disallowed-types` lists and by a check of each crate's dependencies. Only `neoteletext-cli` and port code may use operating-system facilities.
*Verification:* Continuous integration check that fails if a forbidden item appears.

**REQ-SEC-017 — The command-line program sandboxes itself.**
After opening its inputs and outputs, `neottx` shall restrict itself so that it cannot open other files or start other programs, and cannot use the network except in web mode.

| System | Mechanism | Phase |
|---|---|---|
| Linux | Landlock for file access, and a seccomp system-call filter | 1 |
| Windows | An AppContainer or restricted token, and a job object | 2 |
| OpenBSD | `pledge` and `unveil` | 2 |
| macOS | Whatever the operating system allows a command-line program, which is weaker; the documentation shall say exactly what is and is not enforced | 2 |

If the sandbox cannot be applied, `neottx` shall refuse to run unless told to continue without it, and shall say why.
*Phase:* Phase 1 on Linux; Phase 2 on the other systems.
*Verification:* Tests on each system that try, after start-up, to open another file, start a program and open a network connection, and expect all three to fail.

**REQ-SEC-018 — Privilege-separated web mode.**
In web mode on hosted systems, `neottx` shall run as two processes:
- **A fetcher.** It holds the network rights and runs the web module's transport and TLS.
- **A decoder and renderer.** It holds no rights beyond its output.

They shall be joined only by a pipe carrying TTI pages or T42 packets and a small, fixed set of framed control messages, such as page requests and form submissions. Each process validates everything it receives from the other. The decoder process shall run under the sandbox of REQ-SEC-017 with no network and no file access.
*Phase:* Phase 1 on Linux; Phase 2 on the other systems.
*Verification:* Test that the decoder process cannot open network connections or files; fuzzing of the control messages.

**REQ-SEC-019 — WebAssembly isolation.**
`neoteletext-core` and `neoteletext-formats` shall build for `wasm32-unknown-unknown` and `wasm32-wasip1` (REQ-GEN-015). An example host shall run the decoder inside a WebAssembly runtime with limits on memory and execution time. Hosts can then isolate the decoder completely, and C programs can use it without the unsafe C interface.
*Phase:* Phase 1 for the builds; Phase 2 for the example host.
*Verification:* Continuous integration builds; test of the example host with a hostile input that must hit its limits cleanly.

**REQ-SEC-020 — Secure defaults.**
Every configurable security-related setting shall default to the safer choice. Examples:
- no local-network access;
- cookie store not saved;
- the security indicator on;
- confirmation of form submission on.

The documentation shall list each such setting and the risk of changing it.
*Verification:* Inspection; a test of the default configuration.

**REQ-SEC-021 — Guidance on hardware isolation for microcontroller ports.**
The porting guide shall explain how a microcontroller port can use a memory protection unit, where the chip has one (the Arm Cortex-M MPU, or RISC-V physical memory protection), to separate the network stack from the decoder. The library cannot enforce this (REQ-ARCH-010).
*Phase:* Phase 2 (whole requirement).
*Verification:* Inspection.

**REQ-SEC-022 — Sandboxed test tools.**
Third-party programs used as test oracles or for fuzzing shall run in containers with no network access and read-only inputs. These are zvbi, vhs-teletext, vbit2 and QTeletextMaker (REQ-TST-001). Downloaded corpus files shall be verified by checksum before anything reads them (REQ-TST-022).
*Verification:* Inspection of the continuous integration configuration.

**REQ-SEC-023 — Security process.**
- **Reporting.** The repository shall contain `SECURITY.md`, saying how to report a vulnerability privately and how reports are handled.
- **Reviews.** Each milestone shall end with a security review against the threat model, recorded in the repository.
- **Release gate.** A release requires:
  - every requirement of this area passing its tests;
  - no open security advisories against any dependency;
  - clean fuzzing for a stated minimum time;
  - the software bill of materials published.
- **Regression tests.** Every security fault found shall get a regression test.

*Verification:* Inspection.


**REQ-SEC-024 — Links are always shown whole and never disguised.**
A link may be followed only while the receiver is showing its complete prompt in row 24. That prompt is drawn by the receiver for TTI Link Extension pages (REQ-NAV-011), or is the transmitted row 24 for broadcast pages. A page cannot:
- hide, conceal, flash, overlap or partly draw a link prompt;
- disguise a link to another service as a local one;
- give a key a link it does not show.

On pages where row 24 is not displayed, the coloured keys shall do nothing. That includes newsflash and subtitle pages shown boxed, and page zoom of the top half when row 24 is off screen. The only link mechanisms are the coloured keys (decision E.36) and the page-number and text links declared by `LK` and `LT` lines (decision E.64). Text on the page that looks like a link, but is not declared as one, is only text: it cannot be selected, and nothing shows a destination for it.
*Amended 12 October 2026* (decision E.64): page-number and text links.
*Verification:* Test with hostile pages that try each of these.

**REQ-SEC-025 — The browser draws row 0 on every page.** Moved to `prototypes/browser/docs/requirements.md` on 16 October 2026: it concerns the browser prototype only.

**REQ-SEC-026 — The browser's helper processes.**
*Phase:* Phase 1 on macOS and Linux; Phase 2 on Windows, Android and iOS (section F.0.1).
The browser shall run each tab's network access and decoding in separate helper processes, as REQ-SEC-018 describes for `neottx`: a fetcher, which holds only the network, and a decoder, which holds nothing (decision E.69). The interface process shall route every message between them, read every message as untrusted, and check each against the strict, bounded framing of the pipe. It shall drop any confirmation request a decoder sends: only the interface asks the viewer to confirm, outside the page (decision E.65 item 6).

The helpers shall be confined before they read any input:
- **on macOS,** by Seatbelt profiles applied with `/usr/bin/sandbox-exec` when the helper starts. The decoder's profile allows no files, no network and no other programs; the fetcher's allows outgoing connections and name resolution only;
- **on Linux,** by the Landlock and seccomp sandbox of REQ-SEC-017, applied by each helper.

If a helper cannot be confined, it shall not run, unless the browser was started with `--no-sandbox`, which exists for debugging only.
*Verification:* A test on every run that, under each helper's confinement, tries to read a file, open a network connection and start a program, and expects exactly the failures above, and that the same attempts succeed unconfined, so the test measures something.

---

Each requirement below gives:
- a statement of what shall be implemented;
- where helpful, the reason and the source;
- the verification method.
