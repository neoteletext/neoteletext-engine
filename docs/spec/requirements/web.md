# F.19 Web-native page sources (WEB)

This area makes neoteletext a web-native receiver (decision E.30). Pages and packets can be fetched from ordinary web servers, pushed by them, and interacted with through forms. Everything here lives in an optional web module. The core library stays free of networking (REQ-GEN-002), and the network transport itself is supplied by the port (REQ-WEB-001).

**REQ-WEB-001 — The network transport is supplied by the port.**
The web module shall contain the protocol logic for:
- HTTP/1.1;
- WebSockets;
- server-sent events;
- cookies;
- forms.

It shall not open sockets or perform encryption itself. It shall be written "sans-IO":
- it is a set of state machines;
- bytes received, the current time and user actions are fed in;
- bytes to send, timers and events come out.

The same code then works with a blocking port, an asynchronous port, a desktop program using the standard library, or a bare-metal network stack. The port shall supply, through an optional part of the platform interface (REQ-ARCH-009), a byte-stream transport with these operations:
- connect to a host and port, with or without Transport Layer Security (TLS);
- send;
- receive, without blocking, reporting data as it arrives;
- close.

Certificate checking is the port's responsibility and shall be on by default. Every operation shall be non-blocking and report its results as events (decision E.30 item 3).
*Verification:* Test with an in-memory transport that replays recorded exchanges; inspection that the module has no socket or cryptography code.

**REQ-WEB-002 — The service descriptor.**
A web teletext service shall be identified by the address of a small descriptor document. The descriptor is a JSON object with these members:
- the service name;
- the initial page;
- a page address template;
- optionally, a push address and the push method (WebSocket or server-sent events), and with a WebSocket only, `"input": true` in the `push` object: the channel takes the viewer's keys back on that socket (area INT, decision E.90; *amended 20 October 2026, decision E.92*);
- optionally, a list of other origins that forms may submit to;
- optionally, the default character set region (decision E.4);
- optionally, a list of other services that links may lead to (Appendix E). Each has a short name, a display name and the address of its own descriptor;
- optionally, `aliases`: an object whose members map a page number to a target, either `name:mpp` or an address (decisions E.63 and E.64). An alias is one page number, never a range. A relative address shall be refused, an address in a descriptor fetched over `https` shall be `https`, and there shall be at most 16 aliases;
- optionally, `refresh`: a whole number of seconds from 5 to 86 400, the interval at which the page on screen is asked for again when the service has no push (decision E.63).

A channel's descriptor is the file `channel.json` in the channel's folder, with the differences REQ-WEB-024 lists (decision E.70). A descriptor may also be given by any other address, as a service with a page address template (REQ-WEB-018).

The page address template uses the placeholders `{m}` (magazine), `{pp}` (page number as two hexadecimal digits), `{mpp}` and `{sub}` (sub-code as four hexadecimal digits), for example `https://example.org/ttx/pages/{mpp}`. The reader shall reject a descriptor with missing required members or unknown placeholders, with a clear diagnostic.
*Reason:* A page number is teletext's address. The template maps it to a web address without making the page files themselves aware of the web.
*Amended 12 October 2026* (decisions E.63 and E.70): `aliases`, `refresh`, and `channel.json` as a channel's descriptor.
*Verification:* Test with valid and malformed descriptors, including aliases and refresh intervals out of range.

**REQ-WEB-003 — Fetching pages.**
The web module shall fetch a page by expanding the template for the requested page, and optionally sub-page, and making an HTTP GET request. It shall accept these response types:
- **A TTI page file** (REQ-INP-013), media type `text/x-teletext-tti`. A file may contain several sub-pages.
- **A T42 packet stream** (REQ-INP-002), media type `application/x-teletext-t42`.

These two media types are defined by this project. For interoperability, the module shall also accept `text/plain` and `application/octet-stream`, deciding the format by the detection of REQ-INP-005. A channel's files are accepted and checked as REQ-WEB-027 says. The response shall pass through the normal acquisition path, as page files do (REQ-ARCH-004).

The module shall:
- follow at most 5 redirects. When the port asks for redirects to stay within the channel, a redirect that leaves the channel shall instead be reported, not followed (REQ-WEB-021);
- treat 404 as "page not in this service";
- report other failures as events without disturbing pages already stored.
*Verification:* Test against the local test server of REQ-WEB-013.

**REQ-WEB-004 — Caching and conditional requests.**
The web module shall honour the caching headers of each response. Before fetching a page again, it shall make a conditional request (`If-None-Match` or `If-Modified-Since`) whenever it holds a validator (`ETag` or `Last-Modified`). A response of 304 (not modified) shall mark the stored page as current without changing it.

Pages fetched from the web are held in the normal page store (REQ-ACQ-005), so they remain viewable when the network is lost (REQ-WEB-009). The validators are stored with each page. The number of fetches per second shall be limited by a configurable rate, so that the next-page and pre-fetching functions (REQ-NAV-002 and REQ-NAV-004) cannot flood a server.
*Verification:* Test that unchanged pages cost one conditional request and no body.

**REQ-WEB-005 — Server push.**
When the descriptor names a push address, the web module shall hold a push connection and apply what it receives as it arrives. Two push methods shall be supported:

- **WebSocket** (RFC 6455).
  - A binary message carries T42 packets, as whole 42-byte packets.
  - A text message carries a TTI page.
  - The client sends a text message `subscribe` followed by page numbers, to limit the push to the pages it wants. `subscribe all` asks for the whole service.
  - For a descriptor with `push.input`, a binary message from the client is a piece of the input stream, at most 4 096 bytes (REQ-INT-101; *amended 21 October 2026, decision E.100*: one key record of at most 46 bytes before), sent only while the socket is open; the web module sends nothing else from the client.
  - The rows of a page being typed into may be pushed alone; a whole page should still be pushed for a carousel.
- **Server-sent events** (the `text/event-stream` format).
  - An event named `page` carries a TTI page in its data lines.
  - An event named `packets` carries T42 packets encoded in base64.
  - Page numbers to subscribe to are given as a query parameter.

Pushed content enters the normal acquisition path. **A heartbeat** keeps the connection honest: a WebSocket silent for 25 seconds is pinged, and one that does not answer within 10 seconds is treated as lost; an event stream silent for 60 seconds is made again. After a lost connection, the module shall reconnect with increasing delays, starting at 1 second and doubling to at most 60 seconds. For server-sent events it shall resume with the `Last-Event-ID` header. A **long-polling fallback** (a repeated conditional GET that the server holds open until a page changes) should be available for servers that support neither push method.

The zxnet live feed reader (REQ-INP-022) is a separate, fixed protocol and is not changed by this requirement.
*Phase:* Phase 1 for WebSocket and server-sent events; Phase 2 for the long-polling fallback.
*Verification:* Test against the local test server, including dropped connections and resumption; tests of the heartbeat on each method, and of input messages sent only on a channel that declares input.

**REQ-WEB-006 — Cookies and sessions.**
The cookie store shall follow current cookie practice (decision E.35): the latest draft of RFC 6265bis, the replacement for RFC 6265, as implemented by current browsers, made stricter wherever neoteletext's single-origin model allows. The following rules apply.

*Accepting a cookie (processing `Set-Cookie`):*
- **Malformed or empty.** A `Set-Cookie` header containing a control character (other than horizontal tab) shall be ignored entirely. So shall a cookie whose name and value are both empty.
- **Size.** A cookie whose name and value together exceed 4096 bytes shall be ignored. An attribute whose value exceeds 1024 bytes shall be ignored.
- **Host-only.** The `Domain` attribute is ignored, so every cookie is host-only (REQ-WEB-015). This is stricter than the draft, and means no list of public suffixes is needed.
- **Lifetime.** `Max-Age` takes precedence over `Expires`. Any lifetime longer than 400 days shall be reduced to 400 days.
- **Secure cookies over plain HTTP.** A cookie with the `Secure` attribute shall be ignored if it arrives over a plain HTTP connection. A cookie arriving over plain HTTP shall not replace or delete a `Secure` cookie of the same name, even though plain HTTP is allowed (decision E.32).
- **Name prefixes.** Checked without regard to letter case:
  - `__Secure-` requires the `Secure` attribute and an HTTPS connection.
  - `__Host-` also requires `Path=/` and no `Domain` attribute.
  - `__Http-` requires `Secure` and `HttpOnly`.
  - `__Host-Http-` requires all the `__Host-` and `__Http-` conditions together.

  A cookie that breaks the rule for its prefix shall be ignored.
- **SameSite.** The attribute shall be stored as `Strict`, `Lax`, `None` or, when absent or unrecognised, the default. The default shall be enforced as `Lax`.
- **`HttpOnly` and `Partitioned`.** These shall be accepted and stored. neoteletext has no scripting and no embedded third-party content, so every cookie is in effect HTTP-only and first-party.

*Sending cookies:*
- **Which cookies.** Cookies shall be sent only to the host that set them, on paths matching their `Path`, and `Secure` cookies only over encrypted connections.
- **Which requests.** They shall be sent with every request that matches, including WebSocket upgrades and server-sent event connections, over both plain and encrypted connections (decision E.32).
- **Cross-origin form submissions.** A form submission to a different origin is a cross-site request. Only `SameSite=None` cookies that are also `Secure` shall be sent with it.
- **Navigation between services.** A top-level GET to a different service, made by the viewer following a link, shall carry the destination's `Lax`, default and `None` cookies, but not its `Strict` cookies.

*The store:*
- **Opaque values.** Cookie values are opaque: the client never interprets session contents.
- **Limits.** The store shall hold at most a configurable number of cookies per host, 50 by default, and in total, 100 by default, in caller-provided memory (REQ-GEN-003). The least recently used cookies beyond these limits are discarded, expired cookies first.
- **Persistence.** The store may be saved between runs only as REQ-SEC-013 allows. It can be wiped at any time. Saved data is untrusted when it is read back: each record is stored through the rules above with the path it was saved with, a record those rules refuse (one expired since it was saved, for example) is skipped and the import goes on, and only a record whose end cannot be found ends it (decision E.114 item 1).
- **Kept by the port.** A port may keep the store itself and give the agent an interface to it; the rules above are then the port's to apply, with the engine's jar to apply them (decision E.104).
- **Times.** Each cookie keeps when it was made and last sent, in seconds, and the store can list its cookies and remove those of a host or a time range (decision E.104).
- **`Clear-Site-Data`.** A response over an encrypted connection whose `Clear-Site-Data` names `"cookies"` or `"*"` shall clear its host's cookies before its own `Set-Cookie` values are stored; over plain HTTP the header shall be ignored (decision E.104).

*Verification:* A test for every rule above. Where the draft has a published test suite (the web-platform-tests cookie tests), the applicable cases shall be adapted and run.

**REQ-WEB-007 — Navigation on web services.**
The following shall all resolve to pages of the same service, fetched through REQ-WEB-003:
- page number entry;
- next and previous page;
- sub-pages;
- Fastext links, from `FL` lines in TTI or from packet X/27/0;
- Index;
- Home.

Pages already in the store shall be shown at once, with a conditional request made in the background to check they are current.
*Verification:* Test.

**REQ-WEB-008 — Security baseline.**
The requirements of area SEC apply to the web module in full and take precedence over this requirement. In particular:
- local-network protection (REQ-SEC-006);
- no downgrade (REQ-SEC-007);
- plain HTTP safeguards (REQ-SEC-008);
- deceptive forms (REQ-SEC-009);
- strict HTTP handling (REQ-SEC-011).

In addition, the web module shall:
- **allow both plain and encrypted connections.** It shall support both plain HTTP and HTTPS, and both plain and secure WebSockets (`ws://` and `wss://`). Neither requires any special configuration. The scheme of the service descriptor's addresses decides which is used.
  - Plain HTTP must be allowed. Many small devices that might display teletext cannot run TLS, and much teletext content is public in any case.
  - A connection made without encryption shall be shown in the status area (for example, a marker in row 0 columns 0 to 6) and reported as an event. The viewer can then see that the page and any form data travel unprotected.
  - The port may still refuse to offer TLS. The module shall then work over plain connections only, and report an HTTPS address as unreachable.
- **limit sizes.** It shall enforce configurable size limits, with documented defaults:
  - one page response, 256 kilobytes by default;
  - one push message, 64 kilobytes;
  - the service descriptor, 16 kilobytes;
  - the headers of one response, 16 kilobytes.

  Anything larger shall be abandoned with a diagnostic.
- **treat everything received as untrusted input.** Page files, packets, descriptors and headers all pass the robustness requirement REQ-GEN-008, including fuzzing (REQ-TST-040).
- **restrict form submission.** A form may submit only to the origin of the page containing it, or to an origin listed in the service descriptor (decision E.31 item 6). A redirect of a submission that keeps its method and body, a 307 or 308, shall not leave the submission's target origin (REQ-SEC-009, decision E.112). A form on a page fetched over plain HTTP may submit over plain HTTP. Before a form is submitted over plain HTTP, the status area shall show a warning that the entered values will be sent unencrypted.
- **ignore headers in page content.** The withdrawn `FH` form command shall be ignored with a diagnostic, never acted on (decision E.31 item 2).
*Verification:* Test for each rule; fuzzing of the HTTP, WebSocket, server-sent event and descriptor parsers.

**REQ-WEB-009 — Working offline.**
When the network is unavailable:
- Pages already in the store shall remain viewable.
- A page that has not been confirmed current for longer than a configurable period shall be marked stale (REQ-FSM-004), and the row 0 status area shall say so.
- Requests for pages not in the store shall show a "not available offline" status, not a blank screen.

When the network returns, stale pages that are being viewed shall be checked first.
*Verification:* Test with the transport switched off and on.

**REQ-WEB-010 — Forms.**
The web module shall implement the TTI Form Extension version 3 of Appendix C in full. This covers:
- field definitions with explicit columns;
- the field types;
- options;
- the form token;
- the submission target;
- success and error pages;
- client-side validation;
- submission;
- the response handling rules.

A decoder without the web module shall still display form pages correctly as ordinary pages, because it ignores the form commands (REQ-INP-013).
*Verification:* Test of each command and field type; test that the same page displays identically with the form commands stripped.

**REQ-WEB-011 — Form submission and the response.**
On submission, the web module shall:
1. validate every field against its definition (Appendix C.5). If any field fails, it shall not submit, but move to the first failing field and show why in the status area.
2. send the form as `application/x-www-form-urlencoded` data:
   - by POST or GET, as the `FS` command says;
   - to the `FS` address;
   - with the session cookies (REQ-WEB-006);
   - with the form token (`FT`) as the field `ttx_token`;
   - with the name of the submit button pressed as the field `ttx_action`.
3. handle the response:
   - If it is a TTI page (REQ-WEB-003), display that page.
   - Otherwise, a 2xx status leads to the success page of the `FP` command.
   - A 4xx or 5xx status, or a failure of the network, leads to the error page. Values already entered shall be kept, so the viewer can return and try again.

Only one submission of the same form may be in progress at a time.
*Verification:* Test against the local test server for success, validation failure, server error, network failure and a response carrying a TTI page.

**REQ-WEB-012 — Guidance for page servers.**
Servers built to serve neoteletext pages should follow these practices, which the reference test server (REQ-WEB-013) shall demonstrate:
- **Keep no session state on the server.** Hold it in a cookie whose value begins with a format version number, so that the format can change without breaking clients that hold old cookies (decision E.30 item 2).
- **Validate and version.** Issue a form token bound to the session, and check it on every submission. Send `ETag` validators with every page.
- **Push only what changed.** Push only changed pages, and push each page whole, not partial rows.
- **Keep pages simple.** Serve Level 1 and Level 1.5 pages only.
- **Publish channels as folders** (decision E.70). Serve each channel's files under their fixed names in its folder (REQ-WEB-019), whether they are files on disk or generated on request. Answer 404 for a file that does not exist, never a page with status 200. Send one of the media types of REQ-WEB-027. Keep each channel's pages, forms and redirects inside its folder, and put any variation in the folder path or the session cookie, never in a query string, which the browser never sends for a channel's files.
- **Point to it from the web** (decision E.73). Put `<link rel="alternate" type="text/x-teletext-tti" href="...">` in the first 8,192 bytes of each web page that has a teletext edition, directly after `<meta charset>`, naming the channel's folder or the page's file; or send the same as a `Link` header. A program may instead answer a request whose `Accept` puts the teletext types first with `303 See Other` to the page's file, sending `Vary: Accept` so that caches keep the two answers apart. Test with `neottx discover` (REQ-CLI-009).

*Verification:* Inspection of the reference server.

**REQ-WEB-013 — Test server and tests.**
The test suite shall include a small local test server, which may be written in any language because it is test code, not part of the library. It shall serve:
- a service descriptor;
- TTI and T42 pages, with validators;
- WebSocket and server-sent event push;
- cookies and sessions;
- forms with tokens.

It shall be able to inject faults:
- slow responses;
- dropped connections;
- oversized and malformed responses;
- wrong media types;
- redirect loops;
- expired and forged tokens.

The tests of this area shall run against it, and against the in-memory transport of REQ-WEB-001 for exact replay.
*Verification:* Test.


**REQ-WEB-014 — The minimal user agent profile.**
The web module shall implement exactly the following profile, and nothing beyond it, unless this requirement is changed by a recorded decision (decision E.34).

*Supported:*

| Feature | What is supported |
|---|---|
| Protocol | HTTP/1.1 over plain or TLS connections supplied by the port |
| Methods | GET, and POST for form submission only |
| Request headers sent | `Host`, `User-Agent` (fixed for the build, naming the port's program, its version and platform, and neoteletext and its version, REQ-WEB-031), `Accept` (the two page media types and JSON, or `text/event-stream` for server-sent events, decision E.39), `Accept-Encoding: identity`, `Cookie`, `If-None-Match`, `If-Modified-Since`, `Content-Type` and `Content-Length` (form submissions only), `Last-Event-ID` (server-sent events only), and the WebSocket upgrade headers. No other headers are sent, in particular no `Referer`. |
| Response handling | Status codes 200, 204, 301, 302, 303, 304, 307, 308, 404 and other 4xx and 5xx codes as errors. Headers: `Content-Type`, `Content-Length`, `Transfer-Encoding: chunked`, `Location`, `Set-Cookie`, `Clear-Site-Data` (decision E.104), `ETag`, `Last-Modified`, `Cache-Control` (`max-age`, `no-cache`, `no-store`), `Retry-After`. Other headers are ignored. |
| Connections | One request at a time per connection. Connections may be reused (persistent connections); pipelining is not used. |
| Push | WebSocket (RFC 6455) and server-sent events, as REQ-WEB-005 defines |
| Content | TTI pages, T42 packets and the JSON service descriptor. Nothing else is interpreted. |

*Permanently excluded:*
- HTTP/2, HTTP/3 and pipelining;
- compressed responses (REQ-SEC-011);
- authentication by HTTP headers or client certificates chosen by pages;
- HTML, CSS and any other markup;
- scripting of any kind (REQ-SEC-004);
- images, audio, video and other media;
- downloads to files;
- embedding one page inside another;
- content from a third party loaded into a page (the one exception, a font named by the channel or the page, was withdrawn on 23 October 2026 with REQ-FNT-006 and REQ-FNT-007, decision E.111);
- plugins;
- `file:`, `data:`, `javascript:` and all other address schemes except those of REQ-WEB-016;
- proxy auto-configuration;
- cross-origin requests other than the permitted form origins of REQ-WEB-008, and the fonts of REQ-FNT-003.

Any response or message outside the profile shall be rejected with a diagnostic, never partly interpreted.
*Verification:* Inspection; tests that each excluded feature is rejected; a test that every request sent contains only the permitted headers.

**REQ-WEB-015 — The origin.**
An origin is the combination of scheme, host and port of an address, after the address has been normalised by the parser of REQ-WEB-016:
- the host is compared in lower case;
- a missing port is the scheme's default port (80 for `http` and `ws`, 443 for `https` and `wss`).

Two addresses have the same origin only if all three parts are equal. The one exception is that `ws` and `wss` addresses count as the same origin as the `http` or `https` address with the same host and port. Every security rule that depends on "the same service" or "the same origin" shall use this definition and no other:
- which cookies are sent. Cookies are host-only: a cookie is sent only to the exact host that set it, and the cookie `Domain` attribute is ignored (REQ-WEB-006). This is stricter than RFC 6265bis requires, and is chosen because security overrides (REQ-SEC-001).
- where a form may submit to (REQ-WEB-008 and REQ-SEC-009);
- the no-downgrade rule (REQ-SEC-007), which applies to the host whatever the scheme;
- pre-fetching (REQ-SEC-013);
- the security indicator (REQ-SEC-005).

A service's origin is the origin of its descriptor. When a service is opened as a channel (REQ-WEB-018), the channel is narrower than an origin: the rules REQ-WEB-026 lists use the channel, and the rest use the origin.
*Verification:* Test with a table of address pairs covering case, default ports, scheme differences and internationalised host names.

**REQ-WEB-016 — The strict address parser.**
All web addresses shall be parsed by one small parser in `neoteletext-web`. This covers addresses in:
- descriptors;
- page address templates;
- redirects;
- push addresses;
- form targets;
- the configuration.

It shall accept only a strict subset of URL syntax:
- **Schemes:** `http`, `https`, `ws` and `wss`, in any letter case.
- **Hosts:**
  - A DNS name of ASCII letters, digits, hyphens and dots, with no empty labels and a total length of at most 253 characters. Internationalised names must already be in their ASCII (Punycode) form.
  - An IPv4 address in dotted decimal only.
  - An IPv6 address in brackets.
- **Port:** an optional decimal port from 1 to 65535.
- **Path and query:** an optional path and query of printable ASCII, with percent-encoding.

It shall reject:
- user names and passwords in the address;
- fragments;
- white space;
- control characters;
- backslashes;
- IPv4 addresses written in octal, hexadecimal or shortened forms;
- more than 2048 characters in total;
- anything else it cannot parse exactly.

Relative addresses shall be resolved against their base address by the rules of RFC 3986, and the result parsed again. The parser shall be `no_std`, shall not allocate, and shall be a fuzzing target (REQ-TST-040). The address checks of REQ-SEC-006 shall be applied to its output.
*Reason:* Different parsers disagreeing about what an address means is a classic source of browser security flaws. One strict parser, used everywhere, removes that class of fault. Rejecting unusual addresses costs nothing for a teletext service.
*Verification:* Test with a corpus of valid, invalid and deceptive addresses, including known cases where parsers disagree; fuzzing.


**REQ-WEB-017 — Device-bound sessions.**
*Phase:* Phase 2 (whole requirement).
Where the port can keep a signing key that cannot leave the device, the web module should support Device Bound Session Credentials (decision E.35). Examples are a secure element, a trusted platform module or a microcontroller's protected key storage. The port supplies the key through an optional part of the platform interface (REQ-ARCH-009).

The session is then bound to that device. The server issues short-lived cookies only after the client proves it holds the key, so cookies copied from the device, or read on a plain HTTP connection, soon become useless. Ports without such storage shall simply not offer it, and sessions then work as REQ-WEB-006 describes. The implementation shall follow the W3C specification as it stands when the work is scheduled.
*Verification:* Test against a test-server implementation of the protocol.
*Amended 23 October 2026 (decision E.110):* built behind the web crate's feature `dbsc`, after the W3C Web Application Security Working Group's Editor's Draft of 8 September 2026: over HTTPS only, ES256 only, registration at the responding origin's own endpoint, the session's scope and refresh address on that origin, a refresh before any request in scope that would go without one of the session's cookies, matched by name, and the session store in memory, at most four sessions. The port gives its keys with `UserAgent::set_device_keys`; the agent never sees a private key. What is not built is listed in decision E.110. Verified by `tests/dbsc.rs`, which plays a server through registration, use, a refresh with the server's challenge after a 403, and the ends of a session, by the module's tests of every reader and of the proofs, and by the fuzz target `dbsc`.
*Amended 24 October 2026 (decision E.114 item 6):* a session identifier shall be refused unless it is non-empty and every byte is visible ASCII, `0x21` to `0x7E`, since it is written back into `Sec-Secure-Session-Id`; and a structured-field string the agent writes shall hold only `0x20` to `0x7E` (RFC 9651 section 3.3.3), so no line break reaches a header. Verified by `a_session_identifier_outside_visible_ascii_is_refused` and `an_sf_string_refuses_bytes_outside_printable_ascii`, which fail on the code before it.

**REQ-WEB-018 — Channels: a channel is a folder.**
The web module shall be able to open a channel given only the address of its folder (decision E.70). A channel is a folder on a web server: its address is an `http` or `https` address whose path ends in `/`, written in the canonical form of REQ-WEB-023.

**Identity.** A channel is identified by its origin (REQ-WEB-015) together with its folder path, compared exactly, including letter case. Every rule that depends on "the same channel" shall use this identity and no other: cookies, forms, push, redirects and links (REQ-WEB-026), and history. The rules that concern the server as a whole keep using the origin: the plain-HTTP marker (REQ-SEC-005), no downgrade (REQ-SEC-007) and the local-network checks (REQ-SEC-006). A folder inside a channel's folder is a different channel, and inherits nothing from it.

**Opening a channel.** The web module shall make these requests in this order, each relative to the channel's folder, and no others:
1. **`channel.json`.**
   - When it answers 200 with a valid file (REQ-WEB-024), the channel takes its settings.
   - When it answers 404 or 410, the channel takes the defaults: initial page 100, no push address, no linked channels, no aliases, no refresh interval, no other form origins, and the configured default character set region.
   - Any other answer, including a server error, a refused media type (REQ-WEB-027) and an invalid file, shall be reported as a failure, and the channel shall not open. The defaults shall never replace a file that exists but cannot be used.
2. **`index.t42`.**
   - When it answers 200 with whole T42 packets, every page it holds shall be stored (REQ-WEB-019).
   - When it answers 404 or 410, the channel has no bundle.
   - Any other failure shall be reported, and the channel shall open without the bundle.
3. **The initial page,** from the store when the bundle held it, otherwise from its own file (REQ-WEB-019).

`contents.txt` is not asked for when a channel opens. It is asked for only when the channel's listing of pages is wanted.

**Descriptor services.** A service descriptor given by an address whose file name is not `channel.json` is a service of REQ-WEB-002, with its page address template. Its channel identity is its origin and the folder that holds the descriptor, its contents are the `contents.txt` in that folder, and every rule of this requirement other than "Opening a channel" applies to it.
*Reason:* A channel in a folder works on every static host, including a GitHub Pages project site at `user.github.io/project/`, and keeps strangers on one shared host apart. Three fixed requests answer every question about a channel, with nothing inferred.
*Amended 12 October 2026* (decision E.70): this requirement was "Sites", which made the origin the site and looked for the descriptor at `/.well-known/teletext.json`. *Amended 12 October 2026* (decision E.73): a folder that holds no channel leads to discovery.
*Verification:* Tests against the local test server with a channel at the root and two channels in different folders of one origin; with and without `channel.json` and `index.t42`; with a `channel.json` that answers a server error, one that is invalid, and one served as `text/html`; and that a channel never asks for any other file when it opens.

**REQ-WEB-019 — The files of a channel.**
A channel's files shall have these fixed names, in the channel's folder, and no others (decision E.70 item 4):

| File | Holds |
|---|---|
| `NNN.tti` | Page `NNN` as a TTI file (REQ-INP-013), with its sub-pages |
| `NNN.t42` | Page `NNN` as T42 packets (REQ-INP-002), with its sub-pages |
| `index.t42` | Any number of pages, as T42 packets |
| `channel.json` | The channel's settings (REQ-WEB-024) |
| `contents.txt` | The channel's contents (REQ-WEB-025) |

`NNN` is the page number as three hexadecimal digits, the magazine 1 to 8 first, with the digits `A` to `F` in capitals: `100.tti`, `1FE.t42`. The web module shall ask for page files only in this form.

**Finding a page.** A page shall be looked for:
1. in the store, when it is there and confirmed current (REQ-WEB-004);
2. otherwise in `NNN.tti`; when that answers 404 or 410, in `NNN.t42`;
3. otherwise it is "not found", and is reported so (REQ-WEB-003).

Within a channel, the extension that last answered 200 shall be tried first, and the other second. This changes only the number of requests, never which file is used when both exist: when the first answers 200, the second is not asked for. Conditional requests to keep a page current (REQ-WEB-004) shall go to the file that answered.

**What each file may fill.**
- **A page's own file** shall fill only pages that have been asked for in this channel, and enhancement pages, whose page numbers are not decimal and which a page may carry for its objects (decision E.63 item 5). Any other page in it shall be dropped, by its header, with a diagnostic.
- **`index.t42`,** a pushed message, a form response and a local file shall fill every page they hold.
- `index.t42` is bounded by the page response limit of REQ-WEB-008, 256 kilobytes by default. A channel larger than that publishes page files.

When the bundle or a file does not hold the page asked for, and the viewer has not chosen a page, the first page it holds shall be shown.
*Reason:* A page's file cannot overwrite the rest of its channel, while a whole capture published as `index.t42` is a working channel. Fixed names mean one layout to learn and to build.
*Amended 12 October 2026* (decision E.70): fixed file names in the channel's folder replace the page address template and the front door at the root.
*Verification:* Tests that a page is found as `.tti`, as `.t42`, and not at all; that the order of the two requests changes after a success but the result does not; that a page file holding other pages fills only the pages asked for and enhancement pages; and that `index.t42` fills every page.

**REQ-WEB-020 — Aliases and refresh.**
When the channel's `channel.json` has aliases (REQ-WEB-024), a typed number that has no link on the current page shall follow the alias for that number, if there is one (decision E.64 item 4). An alias to another channel is reported to the port as a link to another channel, as REQ-WEB-026 describes. When the descriptor has a refresh interval and the service has no push, the page on screen shall be asked for again at that interval, with a conditional request (REQ-WEB-004).
*Verification:* Test that an alias leads to its target and that a page's own link wins over it; test the refresh interval against the local test server.

**REQ-WEB-021 — Redirects within the channel.**
The web module shall offer a setting under which a redirect that leaves the channel is not followed, but reported as an event naming the new address (decisions E.65 item 5 and E.70 item 12). A redirect stays within the channel when the new address has the same origin and lies inside the channel's folder. A redirect from `http` to `https` also stays within the channel when the host and the folder are the same and both ports are the schemes' default ports; a redirect to any other port does not. A port that opens channels shall use this setting for every request of a channel except form submissions, which keep the rule of REQ-WEB-006; `neottx` follows redirects as REQ-WEB-003 says. The limit of five redirects applies in both cases.
*Reason:* Otherwise a redirect would take the viewer to another channel without the port confirming it.
*Amended 12 October 2026* (decision E.70): the boundary is the channel's folder, not the origin.
*Verification:* Tests that a redirect to another origin and a redirect to another folder of the same origin are reported and not followed; that a redirect to a file inside the folder and an upgrade to `https` on the same host and folder are followed; and that an upgrade to another port is reported.

**REQ-WEB-022 — Enhancement dependencies.**
When the viewer shows a page of a channel at Presentation Level 2.5 or 3.5 and its enhancement pages are not in the store, it shall ask once for its magazine's Organization Table (page xFE), then for the object pages that the table and the page name (decision E.63 item 5). Pages already stored shall not be asked for again. These requests shall not change which page is kept current. While they load, the page is shown with what has arrived, and is drawn again as they arrive. A dependency that cannot be fetched removes only what depends on it (decision E.41).
*Verification:* Test against the local test server that a Level 2.5 page fetches its table and object pages once.

**REQ-WEB-023 — Channel addresses.**
A channel's address shall be checked and written in one canonical form, so that two spellings never name one channel and one spelling never names two (decision E.70 item 3). After the strict parser of REQ-WEB-016:
- the scheme shall be `http` or `https`; the host is compared and written in lower case, because host names are not case-sensitive;
- the port is written only when it is not the scheme's default, and an explicit default port names the same channel;
- the address shall have no query and no fragment;
- the folder path shall begin and end with `/`. Each segment between the slashes shall be 1 to 64 characters from the unreserved set of RFC 3986 section 2.3: the ASCII letters, the digits, `-`, `.`, `_` and `~`. No segment may be `.`, `..` or made only of dots. Percent-encoding shall not appear in a folder path. There shall be at most 8 segments, and the whole path shall be at most 256 characters.

An address whose folder path breaks any of these rules shall be refused with a diagnostic that names the rule, never corrected. Letter case in the host and an explicit default port are not ambiguous, and are normalised. The root folder `/` is a channel like any other.

A channel's file addresses are its folder's address followed by one of the file names of REQ-WEB-019. An address whose path is a folder followed by one of those names belongs to that folder's channel. Any other address names no channel.
*Reason:* Path tricks such as `%2F`, `..` and doubled slashes are the classic way to make one address look like another. Refusing them outright costs nothing, because the allowed set holds the names hosting services use.
*Verification:* A table of addresses, valid and invalid, including `%2F`, `%2E`, `..`, `.`, doubled slashes, upper-case hosts, default and explicit ports, a query, a fragment, nine segments, a segment of 65 characters, and `/ab/` against `/a/`.

**REQ-WEB-024 — `channel.json`.**
`channel.json` is the service descriptor of REQ-WEB-002, read by the same strict, bounded reader with the same 16-kilobyte limit, with these differences (decision E.70 item 7):
- **Every member is optional.** An absent member takes the default of REQ-WEB-018.
- **`page_template` is not allowed.** A `channel.json` that has it shall be refused with a diagnostic, because a channel's files have fixed names.
- **`initial_page`** is a page number of three decimal digits, the magazine 1 to 8 first.
- **`name`,** when present, is the channel's own name for itself. A port may show it outside the page, but never in row 0, which belongs to the receiver (REQ-SEC-005).
- **`services`,** the linked channels, gives each one a short name, a display name, and either `channel`, the address of the linked channel's folder in the form of REQ-WEB-023, or `descriptor`, the address of a descriptor service (REQ-WEB-018). An entry with both, or neither, shall be refused.
- **`font`,** which named the channel's font, was withdrawn on 23 October 2026 (decision E.111): it is an unknown member.
- **`aliases`, `refresh`, `push` and `form_origins`** are as REQ-WEB-002 defines them. A push address shall be on the channel's origin, in any folder.

Unknown members shall be refused, as REQ-WEB-002 requires of every descriptor.
*Verification:* Tests of an empty object, of each member alone, of `page_template`, of a linked channel given each way and both ways, of a push address on another origin, and of an unknown member.

**REQ-WEB-025 — `contents.txt`.**
`contents.txt` lists a channel's sections, pages and linked channels, in the order they shall be shown (decision E.70 item 8). It shall be read by a strict, bounded reader:
- **Size.** At most 64 kilobytes and 1000 lines. A larger file shall be refused whole, with a diagnostic.
- **Characters.** Lines end with LF or CR LF. Fields are separated by one horizontal tab. A byte outside printable ASCII, other than the tab and the line ends, makes its line invalid.
- **Ignored lines.** Empty lines, and lines whose first character is `#`, are comments.
- **Entries.** The first field names the kind of entry:

  | Kind | Fields after the kind | Meaning |
  |---|---|---|
  | `s` | title | A section heading |
  | `p` | page, title, and optionally a date | A page of this channel |
  | `x` | target, title | Another channel or a page of one |

  - **The page** of a `p` entry is three decimal digits, the magazine 1 to 8 first: a page a viewer can type. Enhancement pages are not listed.
  - **The date** of a `p` entry, when present, is when the page last changed, as `YYYY-MM-DD` (ISO 8601).
  - **The target** of an `x` entry is `name:mpp`, a linked channel named in `channel.json`, or an address: a channel's folder, or a page file of a channel, as REQ-WEB-023 defines them.
  - **A title** is 1 to 32 characters. A longer title shall be cut to 32.
- **Faults.** A line of an unknown kind shall be ignored, so that later versions may add kinds. A line of a known kind with the wrong number of fields, an invalid page, date or target, or an empty title shall be ignored. The second and later `p` entries for one page shall be ignored. The number of ignored lines shall be reported, and shown in the contents view's status line.
- **At most 800 `p` entries** are kept; later ones shall be ignored and counted.

Nothing in `contents.txt` is checked against the channel's files: a listed page that does not exist is "not found" when chosen, as a typed number would be.
*Reason:* One small text format that a person can write by hand, a server can generate, and the author's tool can write (REQ-CLI-008), is the whole of a channel's map.
*Verification:* Tests of each kind of entry, of every fault listed, of comments and both line ends, of the size and count limits, and of a duplicate page.

**REQ-WEB-026 — The channel is the boundary.**
When a service is opened as a channel, the channel shall be the boundary for these rules (decision E.70 item 12). Each is stricter than the origin-based rule of the same name, which still applies.
- **Cookies.** Each channel has its own cookie store, under the rules of REQ-WEB-006. A cookie set by a response to a request inside the channel's folder belongs to that channel, and so does one set by its push connection's handshake, wherever in the origin the push address is. A response from outside the folder, such as a permitted form target's or a redirect's elsewhere, shall set and clear no cookies; the folder's upgrade to `https` on the same host (REQ-WEB-021) counts as inside it (decision E.114 item 2). A `Path` attribute outside the folder, including the default path, shall be narrowed to the folder; a `Path` inside it is kept. A channel's cookies shall be sent only with requests inside its folder, with its push connection, and with its form submissions, and never with another channel's requests, even on the same origin.
- **Forms.** A form shall submit only to an address inside the channel's folder, or to an origin that `channel.json` lists in `form_origins` (REQ-WEB-008). A form whose target is elsewhere shall be refused before anything is sent, with the target shown.
- **Push.** The push connection named in `channel.json` shall be on the channel's origin, and may be in any folder.
- **Redirects.** As REQ-WEB-021 says.
- **Links.** An address that is a page file in the channel's own folder is a page of the channel, followed at once. Every other address leads to another channel, and is reported to the port as such; the port decides how the viewer confirms it.
*Reason:* On a shared host, another folder is often another person. The channel must not share a session, a form target or a link's trust with its neighbours.
*Verification:* Tests with two channels on one origin: their cookies are kept apart, including a cookie set with `Path=/` by a page and by a push handshake (`a_push_handshake_cookie_stays_in_its_channel_folder`, finding F-15, which fails on the code before 24 October 2026); a form target's response stores none (`a_response_from_outside_the_channel_stores_no_cookies`, finding F-42, likewise); a form targeting the other channel's folder is refused; a link and a redirect into the other folder are reported as leaving the channel.

**REQ-WEB-027 — Media types and content checks of a channel's files.**
No teletext media type is registered, and common hosts send generic types (decision E.70 item 11). A channel's files shall therefore be accepted with these media types, whatever their parameters, such as `charset`:

| File | Accepted media types |
|---|---|
| `NNN.tti` | `text/x-teletext-tti`, `text/plain`, `application/octet-stream` |
| `NNN.t42`, `index.t42` | `application/x-teletext-t42`, `application/octet-stream`, `text/plain` |
| `channel.json` | `application/json`, `text/plain`, `application/octet-stream` |
| `contents.txt` | `text/plain`, `application/octet-stream` |

Any other media type, including `text/html`, shall be refused, with a diagnostic naming the file and the type. A response with no media type shall be refused. These rules apply to responses with status 200. A 404 or 410 means the file does not exist, whatever the type and body of the response, since servers commonly answer it with an HTML page.

Each file shall then pass the check for the format its name says, and is refused otherwise, never read as another format:
- **a TTI file** shall be a TTI page file with at least one `PN` line, read by REQ-INP-013;
- **a T42 file** shall be a whole number of 42-byte packets, every one with a valid Hamming 8/4 magazine and row address (EN 300 706 clause 8.2);
- **`channel.json`** shall pass REQ-WEB-024;
- **`contents.txt`** shall pass REQ-WEB-025.

A refused page file is reported as a failure of that page, not as "not found", and the other extension is not then tried.
*Reason:* A file's name and type cannot be trusted alone: `.tti` is also Template Toolkit's extension, `.t42` is also PostScript Type 42 fonts', and some hosts answer a missing file with an HTML page and status 200.
*Verification:* Tests serving each file with each accepted type and with `text/html`, with no type, with a Template Toolkit file named `100.tti`, with a Type 42 font named `100.t42`, and with an HTML "not found" page answered with status 200.

**REQ-WEB-028 — Discovery: from a web address to a channel.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**REQ-WEB-029 — Pointers: the `Link` header and the `<link>` element.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**REQ-WEB-030 — The target of a pointer.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**REQ-WEB-031 — The user agent string.**
The `User-Agent` header shall be `product/version (family)`, followed by ` neoteletext/version` when the product is not one of the engine's own programs, in the product syntax of RFC 9110 section 10.1.5, at most 64 bytes of printable ASCII. The product shall be the port's, given in the agent's configuration and fixed for the build; each version shall be the significant version only, the major version or, before 1.0, `0.` and the minor; the family shall be the operating system's family, or for a port without one its board or framework, and nothing more: no version of it, no architecture, no device model, and nothing about the person or the computer. A port that gives no product shall send `neoteletext/` and the engine's significant version. It shall be sent on every request and on the push WebSocket's upgrade, unchanged for the life of the agent (decision E.99).
*Reason:* A channel needs to know which receiver it serves, as a web site does, and nothing more: browsers learned that the rest of a user agent is a fingerprint or a compatibility lie (decision E.99).
*Verification:* Tests of the string for the engine's own program and another's, of the significant version, and of the syntax and length checks; that requests and the upgrade carry it.

**REQ-WEB-032 — Proxies.**
A port may send every connection through a proxy: an HTTP proxy by `CONNECT` (RFC 9110 section 9.3.6), for plain and TLS connections alike, so TLS stays end to end; or a SOCKS5 proxy (RFC 1928) with no authentication, sending the host's name for the proxy to resolve, or the address when the host is one. The request shall be written, and the proxy's answer read, by the engine's sans-IO encoders and bounded readers (`proxy`), the answer read in whole reads, bounded by the head reader, and any of the host's bytes read with it handed to the port before the stream's next; any answer but success shall fail the connection, and nothing shall go direct instead. Through a proxy the agent shall refuse, before it connects, a host that REQ-SEC-006's rule by name refuses (a blocked address, `localhost`, a name of one label, or a name under one of its local suffixes; decision E.113), unless the address is allowed, and shall not refuse the peer it reaches, which is the proxy. `neottx` takes `--proxy http://host:port` or `--proxy socks5://host:port` (decision E.102).
*Reason:* Many networks reach the web only through a proxy; a fallback would leak what the user chose to send through it, and the proxy, which resolves names, is the only one that sees where a name leads.
*Verification:* Tests of the encoders and readers, refusals included, and a fuzz target of the readers (`proxy`); a test of the blocking handshake given an answer and the host's first bytes in one read, which reads the answer once and hands those bytes back, and a test that they reach `neottx`'s connection; tests of the agent's refusals through a proxy; tests of `neottx`'s transport through an HTTP and a SOCKS5 proxy on this computer, and of a proxy that refuses.
*Amended 23 October 2026 (decision E.102):* the answer was read a byte at a time so that no byte of the host's was taken; it is read in whole reads, and the host's bytes read with it are handed back.

**REQ-WEB-033 — Each exchange is reported.**
The agent shall report each answered request on the fetch connection, each hop of a redirect its own, with its method, its address, the response's status, the body's bytes and the microseconds from the request to the end of its response, as an event the port may show or ignore (decision E.102).
*Reason:* A developer of a channel, or of a port, needs to see what the agent asked for and what came back, without a packet capture.
*Verification:* A test that a descriptor and a page are each reported with their address, status and size.

