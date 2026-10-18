# Discovery: from a web address to a channel

How an ordinary web page announces its teletext edition, how the browser finds it, and why it works this way.

The rules are in REQUIREMENTS.md: decision E.73, REQ-WEB-028 to REQ-WEB-030 and REQ-CLI-009, with amendments to REQ-WEB-012, REQ-WEB-014, REQ-WEB-018, REQ-BRW-002, REQ-NAV-013, REQ-NAV-014, REQ-TST-040 and REQ-TST-081. Where this document and REQUIREMENTS.md differ, REQUIREMENTS.md wins. The security analysis is `prototypes/browser/docs/web-discovery-security.md`. Channels themselves are described in `prototypes/browser/docs/channels.md`.

**Status, 12 October 2026:** specified, not built. The work is tracked in GitHub issue #190 and its sub-issues.

## Contents

1. [The problem](#1-the-problem)
2. [Pointing, not carrying](#2-pointing-not-carrying)
3. [Publishing a pointer](#3-publishing-a-pointer)
4. [What the browser does](#4-what-the-browser-does)
5. [The readers](#5-the-readers)
6. [The tuning screen](#6-the-tuning-screen)
7. [`neottx discover`](#7-neottx-discover)
8. [Building it](#8-building-it)
9. [What is given up, and what is still open](#9-what-is-given-up-and-what-is-still-open)

---

## 1. The problem

A channel is a folder of fixed-name files (decision E.70). To read one, a viewer has to know where that folder is. People know ordinary web addresses: a site's name, or a link to a story that someone shared. Discovery lets those addresses lead to teletext:

- **A viewer types the address they already know.** `example.org` opens the site's teletext edition, and `example.org/news/story-123` opens the page for that story, such as page 104.
- **A shared link works in both kinds of browser.** It is an ordinary `https` address: a web browser shows the web page, and neoteletext shows the teletext page.
- **A publisher adds one line** to a web page template, and keeps their addresses, host and content management system.

What a niche format lacks most is being found, and discovery is aimed at that. `rel="alternate"` is already how web pages announce their RSS and Atom feeds, their translations and their AMP versions, so web developers and crawlers know it.

## 2. Pointing, not carrying

Four places to put teletext into an ordinary web answer were considered. Only pointing was kept (decision E.73 item 1).

| Approach | What it would be | Why not |
|---|---|---|
| A page in an HTTP header | `Teletext-Page:` holding a page in base64 | A page is about 1 kilobyte as T42 (1.4 in base64) and 1 to 3 kilobytes as TTI, and more with sub-pages. Proxies and content delivery networks commonly limit all of an answer's headers to 4 to 8 kilobytes. Static hosts such as GitHub Pages cannot set a header per file, intermediaries may drop headers they do not know, and every ordinary visitor would download a page they never use. |
| A page in the HTML | A data block, `<script type="text/x-teletext-tti">…</script>`, which a web browser never runs | An HTML reader would face the network. The page would belong to no channel, so its cookies, forms and links would have nowhere to live. The HTML and the teletext would be two copies of one story, and they would drift apart. |
| `HEAD` requests | Ask for the headers only, and read a `Link` header | A `GET` brings the same header in the same round trip, and the body as well when the pointer is in the HTML. Servers often answer `HEAD` through a different path from `GET`, with different headers. Hosts that cannot set headers can only point from the HTML, which `HEAD` never returns. |
| DNS or `/.well-known/` | A TXT record, or a file at the server's root | The transport of REQ-WEB-001 cannot look up TXT records. `/.well-known/` belongs to a server's root, which is what kept channels out of folders (decision E.70 item 7). Either would be a second mechanism for the same job. |
| **A pointer** | `<link rel="alternate">` in the HTML, or the same as a `Link` header, naming a channel or a page file | **Kept.** One line on any host. The pages stay files in channels, checked as they are today (REQ-WEB-027). Finding the pointer needs a small, bounded scanner, not an HTML parser. |

A dynamic server can also negotiate. The browser's discovery request puts the teletext media types first in `Accept`, and such a server answers `303 See Other` with the address of the page's file. The page still comes from its channel; the redirect is the pointer.

## 3. Publishing a pointer

### 3.1 In the HTML, which works on every host

Put the pointer directly after `<meta charset>`, in the first 8,192 bytes of the page:

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<link rel="alternate" type="text/x-teletext-tti" href="https://example.org/ttx/104.tti" title="Teletext">
<title>Storm warning for the coast</title>
```

- **`href`** names the channel's folder (`https://example.org/ttx/`, which opens at the channel's initial page) or one of its page files (`https://example.org/ttx/104.tti`, which opens at that page). A story's own page usually points to its page file, and the home page points to the folder.
- **`type`** is `text/x-teletext-tti` or `application/x-teletext-t42`. Either may point to a folder. A page file is found as `.tti` or `.t42` whatever its extension, so the type does not have to match it (REQ-WEB-019).
- **Write the address in full.** A relative address works when the page has no `<base>` element before the pointer; an absolute one always works.
- **Do not escape it.** A channel address never needs `&`, so a pointer that holds `&` or `&amp;` is refused (REQ-WEB-030).
- **Why the first 8,192 bytes.** Real heads are long: measured on 12 October 2026, the BBC News home page's head ran to 198,533 bytes and the Guardian's to 741,595, with its alternate link at byte 741,499. HTML already requires `<meta charset>` within the first 1,024 bytes so that a reader can decide from a fixed prefix. The pointer follows the same idea, with a wider limit.

### 3.2 As a header, where the host allows it

```
Link: <https://example.org/ttx/>; rel="alternate"; type="text/x-teletext-tti"
```

Netlify, Cloudflare Pages and every program can send it; GitHub Pages cannot. The header is read before the body, so it costs the browser nothing extra.

### 3.3 From a program, by negotiation

When the request's `Accept` header puts `text/x-teletext-tti` or `application/x-teletext-t42` before `text/html`, answer:

```
HTTP/1.1 303 See Other
Location: https://example.org/ttx/104.tti
```

Answer ordinary browsers with the HTML as usual, and send `Vary: Accept` so that caches keep the two answers apart. Never answer teletext at the story's own address: the browser refuses it, because pages live in channels (REQ-WEB-028).

### 3.4 Check it

`neottx discover https://example.org/news/story-123` shows what the browser would find, and why every other candidate was ignored (section 7).

## 4. What the browser does

### 4.1 When discovery runs

Only after the viewer has confirmed an address on the tuning screen, and only for these addresses (REQ-WEB-028):

- **A folder that holds no channel.** `example.org`, `example.org/` and `user.github.io/blog` are folders. The browser first tries the folder as a channel, with the same four requests as always (`channel.json`, `index.t42`, then the page as `.tti` and `.t42`). When every answer is 404 or 410, a redirect out of the folder, or status 200 with `text/html`, the folder holds no channel, and discovery runs on the address exactly as it was given.
- **An address that cannot be a channel's:** a file name that is not one of a channel's (`story.html`), or an address with a query (`?id=123`). Discovery runs at once.

It never runs for a channel's own files, never to pre-fetch, never on the address a pointer led to, and never because of anything a page sends. Discovery is one step, never a chain.

Testing the folder first keeps every existing channel address working exactly as before: a channel at `example.org/` is found by its files and never reaches discovery. The cost for an ordinary site is up to four small refusals. The headers decide each one, and the browser never reads the HTML bodies that hosts send for missing files.

### 4.2 The request

```
GET /news/story-123 HTTP/1.1
Host: example.org
User-Agent: neoteletext/0.x
Accept: text/x-teletext-tti, application/x-teletext-t42, text/html;q=0.5
Accept-Encoding: identity
```

The request sends no cookie and no `Referer`, and the browser stores no cookie from the answer. It follows up to five redirects to any origin, such as `example.org` to `https://www.example.org/`, checking each for the local network and refusing `https` to `http`. It reads the headers and at most 8,192 bytes of an HTML body, then closes the connection. Since the request asks for no compression, those 8,192 bytes are what crosses the network, plus whatever the server had already sent.

### 4.3 The order of the answer

```
final answer
  ├─ final address is a page file, answered as teletext?  → that page          (negotiation, §3.3)
  ├─ a Link header pointer that passes the checks?        → its target          (§3.2)
  ├─ HTML: a <link> pointer in the first 8,192 bytes?     → its target          (§3.1)
  └─ otherwise                                             → "no teletext edition found"
```

The first pointer that passes the checks of REQ-WEB-030 wins. A publisher who wants a story to lead to its page, and the site to its channel, puts the story's own pointer first.

### 4.4 The target

A target must plainly be a channel: a folder ending in `/`, or one of a channel's file names (REQ-WEB-030). A folder without its trailing slash is refused rather than guessed. The fetcher finds the target, and the interface, which never sees the HTML, parses it again before it acts on it. If the target's channel cannot be opened, that is the failure that is reported. Discovery does not run again on the target.

## 5. The readers

Both are in `neoteletext-web` (`discovery`). They are `no_std`, do not allocate, are fuzzed, and refuse whatever they cannot read exactly (REQ-WEB-029).

**The `Link` header reader** follows RFC 8288 section 3: `<target>` and then `;`-separated parameters, with link values separated by commas outside quoted strings. A value counts as a pointer when its `rel` contains `alternate`, its `type` is a teletext type, and it has no `anchor`. A link with an `anchor` describes some other resource. At most 16 link values are examined.

**The `<link>` scanner** is a byte loop over the 8,192-byte prefix, not an HTML parser:

| It meets | It does |
|---|---|
| `<!--` | skips to `-->` |
| `<!…>`, `<?…>`, `</…>` | skips to `>` |
| `<script>`, `<style>`, `<noscript>`, `<template>`, `<textarea>`, `<title>`, `<xmp>` | skips to the matching end tag, so a `<link>` written inside a script is not taken |
| `<base href=…>` | notes it: a relative pointer after it is refused, not resolved |
| `<link …>` | reads its attributes, quoted or not, first one of each name wins; takes it when `rel` contains `alternate` and `type` is a teletext type |
| any other start tag | reads past its attributes, so a `>` inside a quoted value does not end it |
| `<body`, `</head`, a zero byte, a UTF-16 byte-order mark, the 8,192th byte | stops |

It never decodes a character reference. A target with `&` in it is refused, and a channel's address never needs one. HTML's own rules allow `<head>` and `</head>` to be left out (danluu.com's pages have neither), so the scanner does not need them: it stops at the limit or at `<body`.

## 6. The tuning screen

The viewer confirms the address they asked for, as always, before any request is made (REQ-NAV-014). Discovery then leads somewhere:

- **Inside what was confirmed:** on the same origin, and in the confirmed address's folder or a folder inside it. `example.org` pointing to `example.org/ttx/`, or `user.github.io/` pointing to `user.github.io/ceefax/`, opens at once.
- **Anywhere else:** the tuning screen is shown again, naming the target and the reason, "announced by" and the announcing address. If the viewer asked for a different address, that is shown too.

So `bbc.co.uk`, which redirects to `www.bbc.co.uk`, costs two key presses: one for `bbc.co.uk`, and one for the channel that `www.bbc.co.uk` announces. That is deliberate. Another origin is another owner as far as the browser can tell, and the viewer has agreed only to the first (decision E.73 item 7).

Before discovery, the tuning screen shows an ordinary web address as such, with the note that the browser will look for its teletext edition.

## 7. `neottx discover`

```
$ neottx discover example.org/news/story-123
asked    https://example.org/news/story-123
final    https://example.org/news/story-123   (200, text/html)
found    https://example.org/ttx/104.tti   from <link> at byte 412
channel  https://example.org/ttx/   page 104
ignored  Link header 1: rel="alternate" type="application/rss+xml" (not a teletext type)
ignored  <link> at byte 9,844: past the first 8,192 bytes; the browser will not see it
```

It makes the same request as the browser, then reads on to at most 1 megabyte for its report only, to tell a publisher about pointers they put too late. It does not open the channel, and it runs confined to the network (REQ-CLI-009).

## 8. Building it

| Part | Where | Requirement |
|---|---|---|
| The discovery state machine: when it runs, the request, redirects, the prefix limit, the order | `neoteletext-web::discovery` | REQ-WEB-028 |
| The `Link` header reader and the `<link>` scanner, with fuzz targets | `neoteletext-web::discovery` | REQ-WEB-029, REQ-TST-040 |
| The target checks | `neoteletext-web::discovery`, using `channel` and the address parser | REQ-WEB-030 |
| "No channel here" from the four opening requests | `neoteletext-web::channel` and the agent | REQ-WEB-018 |
| The fetcher's discovery message, and the interface's second check | `neoteletext-app` and the browser's pipe | REQ-SEC-026, REQ-WEB-030 |
| Web addresses in the address bar; the tuning screen's new reasons and its skip rule | `neoteletext-browser` (`location`, `tuning`) | REQ-BRW-002, REQ-NAV-014 |
| `neottx discover` | `neoteletext-cli` | REQ-CLI-009 |
| Test pages | `tests/web-server/sites.py` | REQ-TST-081 |

## 9. What is given up, and what is still open

**Given up, deliberately:**

- **A pointer after the first 8,192 bytes is not seen,** even when it is inside the head. Publishers are told where to put it, and `neottx discover` warns them.
- **Relative pointers after `<base>`** are refused rather than resolved as a web browser would.
- **A second tuning screen** for a pointer to another origin, including the common redirect to `www.`.
- **No memory of what was discovered.** Bookmarks and history keep the channel, not the web address, so the next visit goes straight to the channel. Typing the web address again repeats the discovery.
- **Teletext served at a web address** is refused; it must be a redirect to a page file.

**Still open:**

- whether to show, for a channel page that a web page pointed to, a way back to that web page. That would mean opening a web browser, which the browser does not do today.
- whether a channel may declare, in `channel.json`, the web origins allowed to point to it. That would stop a site from presenting someone else's channel as its edition, but it would also stop casual, well-meant links to a channel.
