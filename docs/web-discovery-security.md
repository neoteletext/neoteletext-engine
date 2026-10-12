# Discovery: security analysis

The security analysis of decision E.73 and REQ-WEB-028 to REQ-WEB-030: ordinary web pages point to channels, and the browser follows those pointers. The design is `docs/web-discovery.md`. Security overrides every other requirement (decision E.32). Each threat below names the requirement that answers it, and what risk remains.

Written on 12 October 2026, before discovery is built. It changed one requirement as it was written (section 3.11).

## 1. What is being protected

| Asset | Why it matters |
|---|---|
| The interface process | It holds the profile, history, bookmarks, the confirmed channels, and the drawing of row 0 and the tuning screen. It is not sandboxed (decision E.69). |
| The viewer's decision of where to go | The tuning screen exists so that nothing takes the viewer to another channel without their key press (REQ-NAV-014). |
| The local network | The browser must not become a way to reach routers and devices behind it (REQ-SEC-006). |
| Encryption | Once a viewer is on `https`, nothing may quietly move them to `http` (REQ-SEC-007). |
| Each channel's session | Cookies are kept per channel (REQ-WEB-026); nothing may plant or read them from outside. |
| The viewer's privacy | Which sites they visit, and when. |
| The publisher's terminal | `ttx discover` prints what a server sent. |

## 2. Trust boundaries

1. **An arbitrary web server to the fetcher.** The discovery answer is from any server the viewer names, or any server it redirects to. Its headers and the first 8,192 bytes of its body are untrusted.
2. **The fetcher to the interface.** The fetcher sends one address and how it was found. The interface treats it as untrusted, since a fault in the scanner could have taken over the fetcher.
3. **The interface to the viewer.** The tuning screen states where a pointer leads, and who announced it.

Discovery adds no new process and no new kind of message beyond the fetcher's report of a target.

## 3. Threats and their answers

### 3.1 Hostile HTML attacks the code that reads it

A server sends crafted headers or HTML: endless attributes, unterminated quotes, comments that never close, nested scripts, megabytes of padding, UTF-16, zero bytes.

- **Answer:** the scanner is not an HTML parser. It is a byte loop over at most 8,192 bytes that knows comments, raw-text elements, attribute quoting and the `link` and `base` elements, and stops at the first thing it cannot read exactly (REQ-WEB-029). It does not allocate, panics are denied, it is Rust with no unsafe code (REQ-SEC-015), and it is a fuzz target, as the `Link` header reader is (REQ-TST-040). It runs in the tab's fetcher, which is confined to the network (REQ-SEC-026). The interface never receives HTML.
- **Remaining risk:** a fault in the scanner is contained by the fetcher's sandbox. A fetcher taken over that way already holds the network, so section 3.11 covers what it could then do.

### 3.2 The scanner and a web browser read the page differently

The scanner reads a subset of HTML. Where it disagrees with a web browser, a pointer could be found that a web browser would never see, or the other way round. The danger is someone who can inject text into a site's page, such as a comment form, planting a pointer that sends the site's teletext viewers elsewhere.

- **Answer:** the scanner stops at `<body` and `</head`, so content injected into a page's body is never read. It skips comments and the contents of `script`, `style`, `noscript`, `template`, `textarea`, `title` and `xmp`, as the HTML standard does, so a pointer written as text inside them is not taken. Attribute values are read with HTML's quoting rules, so a `>` inside a quoted value does not end a tag early. Of two attributes with the same name it takes the first, as HTML does. It decodes no character references, and a target containing `&` is refused, so an encoded address cannot hide what it is (REQ-WEB-030).
- **Remaining risk:** a site that lets strangers write into its `<head>` before the 8,192-byte limit already has a script-injection fault on the web. A planted pointer is then one more effect of a fault the site has. And any target it names still passes every check of section 3.3 and the tuning screen of section 3.4.

### 3.3 A pointer leads somewhere dangerous

A pointer names a router's address, a `file:` or `javascript:` address, an `http` address from an `https` page, or an address that is not a channel at all.

- **Answer:** a target must be plainly a channel's folder or file, by the strict address parser and the channel rules (REQ-WEB-030 rules 3 and 4). It may not be `http` when the answer came over `https` (rule 5). Its host is checked against the local-network ranges when connected to, as every address is (REQ-SEC-006), and a literal blocked address is refused at once (rule 6). Every redirect during discovery is checked the same way (REQ-WEB-028).
- **Remaining risk:** none beyond those of a typed address, which a pointer cannot exceed.

### 3.4 A site presents someone else's channel as its own edition

`example.org` points to a channel on another host made to look like its edition, or a look-alike web address points to a channel that imitates a real one.

- **Answer:** the tuning screen is shown for every target outside the confirmed folder on the same origin. It names the target's host and folder, and the address that announced it (REQ-NAV-014, decision E.73 item 7). Row 0 always names the channel by its host and folder, never by anything the channel or the web page says (REQ-SEC-025). A pointer cannot change what row 0 shows.
- **Remaining risk:** a site can call any channel its edition, just as it can link to anything on the web. The browser shows the truth about where the viewer is, and does not judge whose edition it is. Appendix D's open question about host names that imitate others applies here too. `docs/web-discovery.md` section 9 records the open question of letting a channel list the web origins allowed to point to it.

### 3.5 Redirects take discovery to another origin

`example.org` redirects to `example.net`, which announces a channel. The viewer confirmed only `example.org`.

- **Answer:** redirects during discovery are followed to any origin, as needed for the common redirects to `https` and to `www.`. Each is checked for the local network and for downgrade, and there are at most five (REQ-WEB-028). The confirmed folder is the one the viewer agreed to, not the final address. A target outside it, which any target on `example.net` is, shows the tuning screen naming both the address asked for and the address that announced the target.
- **Remaining risk:** the discovery request itself reaches `example.net` before the viewer has seen that name. It carries no cookie, no `Referer` and nothing about the viewer, and the redirect was `example.org`'s own choice, as it would be in any web browser.

### 3.6 Discovery leaks the viewer's activity

- **Answer:**
  - Discovery runs only after the viewer's own key press, never to pre-fetch, and never because of anything a page or decoder sends (REQ-WEB-028), so a page cannot make the browser contact a web address.
  - Its request carries no cookie and no `Referer`, and the fixed `User-Agent` of REQ-WEB-014.
- **Remaining risk:**
  - The site learns that a neoteletext viewer asked for that address, which is what the viewer asked to do.
  - The `Accept` header and the four channel requests before it show that the client is neoteletext. The `User-Agent` already says so.

### 3.7 Discovery plants or reads a channel's session

A web server sets a cookie in its discovery answer, hoping it will be sent to a channel on the same origin and fix that channel's session. Or a channel's cookie is sent to the web server.

- **Answer:** discovery sends no cookie and keeps none: every `Set-Cookie` in its answers is ignored (REQ-WEB-028). Cookies belong only to channels (REQ-WEB-026).
- **Remaining risk:** none.

### 3.8 Teletext served at a web address

A server answers the discovery request directly with a TTI page, at an address that is not a channel's. Accepting it would create a page with no channel, and so no rules for its cookies, forms and links.

- **Answer:** a teletext answer counts only when the final address is a channel's page file. Anything else is refused with a diagnostic (REQ-WEB-028). Even then, the body read during discovery is not used: the channel opens and fetches the page through its normal path and checks (REQ-WEB-019, REQ-WEB-027). A `303 See Other` to a page file on another origin is a target like any other, with the tuning screen of section 3.4.
- **Remaining risk:** none.

### 3.9 Denial of service

A server sends endless headers, an endless body, a body one byte a minute, a redirect loop, thousands of `<link>` elements, or a pointer to a channel whose folder points back.

- **Answer:**
  - **Headers:** the 16-kilobyte limit of REQ-WEB-008.
  - **Body:** at most 8,192 bytes, after which the connection is closed and not reused.
  - **Slow servers:** the time limits of REQ-SEC-011.
  - **Redirects:** at most five.
  - **Candidates:** at most 16 link values and 16 `<link>` elements.
  - **Loops:** discovery is one step and never runs on a target (REQ-WEB-028).
  - **The four channel requests before discovery:** decided from their headers, and HTML bodies are never read.
- **Remaining risk:** a site costs at most five small requests and 8,192 bytes per address the viewer confirms.

### 3.10 The interface trusts the fetcher's answer

If the scanner were taken over, the fetcher could report any address.

- **Answer:** the interface parses the reported target again by the strict parser, the channel rules and the downgrade rule (REQ-WEB-030), and decides itself whether the tuning screen is needed (REQ-NAV-014).
- **Remaining risk:** a compromised fetcher could report a target inside the confirmed folder, which opens without a second tuning screen. The viewer has already agreed to go to that folder, so this gives the attacker nothing a pointer could not. A compromised fetcher holds the network in any case. That is the existing limit of the fetcher's confinement (decision E.69), not something discovery adds.

### 3.11 `ttx discover` prints what a server sent

The report prints addresses, header values and attribute values from an arbitrary server to a publisher's terminal. Escape sequences in them could rewrite the screen, change the window title or worse.

- **Changed by this analysis:** REQ-CLI-009 now prints every value from the answer through the allow-list of REQ-SEC-010: printable ASCII as it is, every other byte as `\xHH`, cut to 200 characters. The command runs confined to the network, with no file access.
- **Remaining risk:** none beyond REQ-SEC-010's.

### 3.12 Cached answers mix the two forms

A publisher using content negotiation forgets `Vary: Accept`. A cache in between then gives web browsers the `303` meant for teletext, or gives the browser the HTML.

- **Answer:** this is a fault in the publisher's site, and the browser's behaviour is still safe either way. With the HTML, it reads the pointer; with the `303`, it follows it through the checks. REQ-WEB-012 and `docs/web-discovery.md` section 3.3 tell publishers to send `Vary: Accept`.
- **Remaining risk:** web visitors to such a site may be sent to a `.tti` file.

## 4. Changes made to the requirements by this analysis

1. **REQ-CLI-009:** every value printed from an answer goes through the allow-list of REQ-SEC-010.

The rules of sections 3.3, 3.6, 3.7, 3.8 and 3.10 were in the design from the start, and are recorded in REQ-WEB-028 and REQ-WEB-030.

## 5. Open points

- **Origins allowed to point to a channel:** a `channel.json` member listing them would stop impersonation (section 3.4), but also casual links. Not required.
- **Imitating host names:** the tuning screen does not flag host names made to look like others, for channels or for announcing addresses (Appendix D.2).
