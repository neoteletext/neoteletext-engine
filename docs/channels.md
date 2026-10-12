# Channels: publishing teletext on the web

How neoteletext finds teletext on the web, why it works this way, and how to publish a channel, as files or from a program.

The rules are requirements in REQUIREMENTS.md: decision E.70, REQ-WEB-018 to REQ-WEB-027, REQ-BRW-009 and REQ-CLI-008. Where this document and REQUIREMENTS.md differ, REQUIREMENTS.md wins. This document gives the reasons and the background.

**Status, 12 October 2026:** built in the browser (`neoteletext`) and in `ttx contents`, and tested end to end against the local channels of `tests/web-server/sites.py`. `ttx view` opens services by their descriptors, not channels.

## Contents

1. [How a television channel's teletext is laid out](#1-how-a-television-channels-teletext-is-laid-out)
2. [From a television channel to a web channel](#2-from-a-television-channel-to-a-web-channel)
3. [The rules: a channel is a folder](#3-the-rules-a-channel-is-a-folder)
4. [A channel's structure: the contents file](#4-a-channels-structure-the-contents-file)
5. [Publishing a channel](#5-publishing-a-channel)
6. [Media types](#6-media-types)
7. [What is given up, and what is still open](#7-what-is-given-up-and-what-is-still-open)

---

## 1. How a television channel's teletext is laid out

A broadcast teletext service is a small magazine: about 800 numbered pages, grouped by subject into hundreds, entered from an index on page 100. Nothing in the standard fixes which subject goes where; each broadcaster chooses.

### The address space, fixed by the standard

- **Magazines 1 to 8** are the first digit of the page number: page 1xx is magazine 1, and magazine 8 is written 8xx. Each magazine is its own stream within the broadcast, and some data, such as the Level 2.5 tables, belongs to a magazine.
- **Pages** are three digits, 100 to 899: the 800 numbers a viewer can type.
- **Sub-pages** are versions of one page number that rotate on screen ("1/4, 2/4 …"), for a story or a table too long for one screen.
- **Hidden pages** have hexadecimal digits, such as 1FE or 1F0, which a remote control cannot type. They carry machinery: the Level 2.5 and 3.5 tables, object and character pages, and the German Table Of Pages.

### The editorial layout, fixed by convention

- **Page 100 is the front page:** the main index, listing the sections and their page numbers.
- **Sections take the hundreds,** each with its own index at its first page. BBC Ceefax, for example, put news at 1xx with headlines from 101, sport at 3xx, weather and travel at 4xx, entertainment at 5xx, and television and radio listings at 6xx.
- **Within a section:** an index page, then the stories in order, then a page leading back. Ceefax's news ran a headlines page, then one page per story of about 80 to 100 words.
- **Special pages:** subtitles (888 in the United Kingdom), newsflash and live-score pages shown boxed over the picture, and engineering and test pages in 8xx.

### How pages connect

- **Printed page numbers** were the first links: the index says "Sport 300", and the viewer types 300.
- **Fastext** added four coloured prompts on row 24, usually the next story, the section index, a related section and the main index. The broadcaster sends the destinations of each page in its packet X/27/0.
- **Next and previous** step through page numbers, so related pages sit next to each other.
- **The Table Of Pages,** used in Germany, Austria and Switzerland, sent hidden tables that grouped pages into blocks and groups with titles, from which the receiver drew its own prompts and lists of contents. It is the nearest broadcast teletext came to a site map. Its full specification is not in this project's sources (REQUIREMENTS.md Part G item 2).

### Delivery

- Pages are not fetched; they are **broadcast in a loop**, a carousel that repeats every 20 to 30 seconds. Typing a number waits for the page to come round, while the header counts through the page numbers.
- Broadcasters sent important pages, such as 100, the section indexes and the headlines, more often, so they arrived sooner. Fastext receivers stored the linked pages in advance, so they appeared at once.
- **Row 0,** the header, shows the page number, the service name, the date and the time on every page: the channel's constant identity.

---

## 2. From a television channel to a web channel

### The first model, and why it was too narrow

The browser's first model (decision E.63) made a site a whole **origin**: a scheme, a host and a port, such as `https://example.org`. Its descriptor lived at `/.well-known/teletext.json`, and its pages at `/{mpp}.t42`. That broke in common cases:

| Case | Example | What happened |
|---|---|---|
| GitHub Pages project sites | `user.github.io/ceefax/` | The browser looked for the descriptor and pages at the root, found nothing, and the site failed. |
| Shared hosts with a folder per person | `raw.githubusercontent.com/owner/repo/`, `tilde.example/~ann/` | Every owner on the host was one site, so moving between strangers showed no tuning screen. |
| One broadcaster, several channels | `tv.example/one/`, `tv.example/two/` | They could not be separate channels, each with its own page 100. |
| Archives | `archive.example/ceefax/1990-03-01/` | Each recording should be a channel with its own page 100. |
| Object storage and content delivery networks | `bucket.s3.example/teletext/` | As for GitHub Pages. |

The fix separates two jobs that "site" was doing:

1. **Trust: who controls the server.** This stays the origin. It decides the plain-HTTP marker, no downgrade and the local-network checks, and it is the web's security unit for good reason: a path is no boundary against the server itself.
2. **Identity: which channel is on screen.** This decides what page 100 means, what row 0 names, and when the tuning screen appears. It is editorial, and can be narrower than an origin.

In a web browser a path cannot separate content, because a script at `/a/` can read `/b/`. Teletext pages run nothing; they are only displayed. So neoteletext can safely let a folder be a channel. In television terms, the origin is the transmitter and each folder is a channel it carries.

### Options considered

| Option | Verdict |
|---|---|
| Keep the origin as the site | Rejected: GitHub Pages project sites and shared hosts keep failing. |
| **A channel is a folder: the origin plus a folder path** | **Chosen** (decision E.70). |
| The root's descriptor lists the server's channels | Not now. It needs the server's owner, where a folder needs only the channel's owner. It may come later for broadcasters, with directory sites. |
| Any address can be a root, with no fixed rules | Rejected: the same page file could mean different things depending on how it was reached, and row 0 could not name the channel reliably. |

---

## 3. The rules: a channel is a folder

The design follows five principles:

- **Convention over configuration:** a fixed name for each file, not a setting.
- **One request answers each question.** Is there a settings file? Ask for `channel.json`. Is there a map? Ask for `contents.txt`.
- **Nothing is inferred.** If a file is not there, its feature is not there.
- **No nesting and no inheritance.** One folder is one channel.
- **Fail closed.** Any answer other than "not found" that cannot be used is reported, never replaced by defaults.

### 3.1 A channel and its address (REQ-WEB-018, REQ-WEB-023)

- A channel is a folder on a web server. Its address is an `http` or `https` address ending in `/`: `https://example.org/`, `https://user.github.io/ceefax/`.
- A channel is identified by its origin and its folder path, compared exactly, letter case included. A folder inside a channel is a different channel.
- Folder names may use only letters, digits, `-`, `.`, `_` and `~`: the unreserved characters of RFC 3986. Percent-encoding, `.` and `..` segments, and doubled slashes are refused, never repaired, so two spellings can never name one channel. A folder has at most 8 segments and 256 characters. GitHub user and repository names, and `~user` folders, fit.

### 3.2 The files (REQ-WEB-019)

| File | Holds | Required |
|---|---|---|
| `NNN.tti` or `NNN.t42` | Page `NNN`, with its sub-pages: `100.tti`, `1FE.t42` | Only the initial page |
| `index.t42` | Any number of pages: a bundle, a capture or an archive | No |
| `channel.json` | Settings: push, aliases, refresh, linked channels, form origins | No |
| `contents.txt` | The channel's sections, pages and linked channels | No |

Page numbers are written as three hexadecimal digits, with `A` to `F` in capitals.

### 3.3 Opening a channel and finding a page

Opening a channel is three requests, in order:

1. `channel.json`: its settings, or the defaults when it answers 404 or 410. Any other failure stops the channel opening.
2. `index.t42`: every page in it is stored. 404 or 410 means no bundle; any other failure is reported, and the channel opens without it.
3. The initial page, 100 unless `channel.json` says otherwise, from the store or from its own file.

A page is found in the store, if it is there and current; otherwise from `NNN.tti`, then `NNN.t42`; otherwise it is "not found". The browser tries first whichever extension last worked in that channel, which saves a request without changing the result. A page's own file may fill only pages that were asked for, plus enhancement pages, so one page's file can never overwrite another page of the channel.

### 3.4 `channel.json` (REQ-WEB-024)

The service descriptor of REQ-WEB-002, with every member optional and no `page_template`:

```json
{
  "name": "Ceefax revival",
  "initial_page": "100",
  "services": [
    { "name": "weather", "display_name": "Weather", "channel": "https://weather.example/" }
  ],
  "aliases": { "400": "weather:100" },
  "refresh": 60
}
```

The `name` is the channel's own claim about itself. It may be the tab's title, but row 0 and the tuning screen never show it: they name a channel by its host and folder.

A channel may also name its font, a 12 × 10 BDF file with its SHA-256: `"font": { "address": "teletext.bdf", "sha256": "..." }` (REQ-FNT-006). Its pages are drawn in that font unless the viewer chose otherwise; row 0 and the receiver's own pages always use Bedstead (decision E.72).

### 3.5 The boundary (REQ-WEB-021, REQ-WEB-026)

The channel is the boundary for everything a site was:

- **Tuning screen:** shown before anything from another channel, including another folder on the same server.
- **Cookies:** each channel has its own. A `Path` outside the folder is narrowed to the folder.
- **Forms** submit only inside the folder, or to an origin `channel.json` lists.
- **Push** may be anywhere on the channel's origin.
- **Redirects** are followed only within the folder; an upgrade from `http` to `https` on the same host and folder is allowed.
- **Links** to a page file in the same folder stay in the channel; every other address is another channel.

### 3.6 Row 0 (REQ-SEC-025)

Row 0 names the channel by its host and folder, such as `user.github.io/ceefax`, with the clock while the name fits before it. A longer name keeps the folder's last segment and the end of the host: `raw.githubusercontent.com/spark-teletext/spark-teletext/master` becomes `<thubusercontent.com/.../master`. The name `channel.json` gives is never shown there, so a channel cannot call itself someone else.

### 3.7 Descriptor services

A descriptor at any other address, such as `https://host/services/news.json`, is still a service with a page address template (REQ-WEB-002). Its channel is the folder holding the descriptor. `ttx` opens services this way. New channels do not need templates.

---

## 4. A channel's structure: the contents file

### 4.1 Why the structure is not inferred from addresses

Addresses give some structure for free:

- **page numbers:** `140.tti` is page 140;
- **channels:** the folder.

They cannot give the rest:

- **Which pages exist.** HTTP has no standard directory listing. The browser could only probe all 800 numbers, which is far too many requests for a hobby server; parse a server's HTML listing, which is HTML and excluded (REQ-WEB-014) and different on every server; or use one host's own interface, which is code for one provider.
- **Titles and sections.** A folder named `news` could suggest a section, but turning names into page numbers needs a mapping, and a mapping is a contents file.
- **Links.** Only the pages hold them, so a map of links means fetching every page, that is crawling. Viewers' browsers never crawl.

What can be known without extra requests is what the viewer has already seen. The contents view lists the pages the tab has shown on the channel when the channel has no contents file.

### 4.2 Alternatives considered

| Option | Verdict |
|---|---|
| `sitemap.xml`, found through `robots.txt` | Rejected: a second format (XML) to harden, addresses only with no titles or sections, and made for search engines. |
| **A contents file at a fixed name** | **Chosen:** one known place, one request, bounded, readable by people. |
| An ordinary teletext page as the contents | Works today, and stays possible, but the browser cannot tell it is the contents. |
| Teletext's broadcast tables (the Table Of Pages at 1F0; the Magazine Inventory Page at mFD) | The precedent. The Table Of Pages specification is not available, and the inventory page has page types but no titles. |

### 4.3 The format (REQ-WEB-025)

One entry per line, fields separated by a tab; lines starting with `#` are comments:

```
# Ceefax revival contents
s	News
p	100	Index
p	101	Headlines	2026-10-12
p	102	World
s	Sport
p	300	Sport index
x	weather:100	Weather
x	https://radio.example/	Radio
```

| Kind | Fields | Meaning |
|---|---|---|
| `s` | title | A section heading |
| `p` | page, title, optional date | A page of this channel; the date (`YYYY-MM-DD`) is when it last changed |
| `x` | target, title | Another channel (`name:mpp`, or a channel or page file address) |

- **Limits:** 64 kilobytes, 1000 lines, 800 pages, titles of 32 characters, printable ASCII only.
- **Faults:** a bad line is ignored and counted, never fatal; a line of an unknown kind is ignored, so later versions can add kinds; a file over the limits is refused whole.

### 4.4 The contents view (REQ-BRW-009)

`C` opens it. The receiver draws it as a classic teletext index under its own row 0, which shows `LIST`: section headings in yellow, pages with dot leaders and their numbers on the right, and other channels in cyan, marked `»`. A `*` marks pages changed since your last visit. Up and Down select, Enter goes, Left and Right turn the screen, and Escape goes back. Entries for other channels go through the tuning screen.

### 4.5 Writing the contents file (REQ-CLI-008)

The author's tool writes it from the channel's own folder, so the only crawl is the owner's, of their own files, before publishing:

```sh
ttx contents public/ceefax > public/ceefax/contents.txt
```

It lists every page in number order, titled from the TTI `DE` line or the first line of text, dated from the file, with a section heading per magazine. Lines for other channels are added by hand after it.

---

## 5. Publishing a channel

"Folder" is a rule about addresses, not about files on disk. The browser asks for addresses such as `…/ceefax/140.tti` and reads what comes back; whether a file or a program answered makes no difference.

### 5.1 As static files

```
public/
  ceefax/
    channel.json
    contents.txt
    100.tti
    101.tti
    300.tti
  weather/
    index.t42
```

Upload the folders to any static host: GitHub Pages, an S3 bucket, nginx. Nothing runs and nothing is configured. On GitHub Pages, a project repository named `ceefax` with `100.tti` at its top level is the channel `https://user.github.io/ceefax/`.

### 5.2 As a program

A server generates the same addresses on request. A minimal example, using only Python's standard library:

```python
#!/usr/bin/env python3
"""Every channel and page is made on request: /<channel>/<page>.tti."""
import http.server
import re

PAGE_FILE = re.compile(r"^/([a-z0-9-]+)/([1-8][0-9A-F]{2})\.tti$")


def page(channel, number):
    """The TTI text of a page, or None when it does not exist."""
    if number != "100":
        return None
    return f"PN,10000\r\nOL,1,{channel.upper()} INDEX\r\n"


class Channels(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        found = PAGE_FILE.match(self.path)
        body = page(*found.groups()) if found else None
        if body is None:
            self.send_error(404)  # never a "not found" page with status 200
            return
        data = body.encode("ascii")
        self.send_response(200)
        self.send_header("Content-Type", "text/x-teletext-tti")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


http.server.ThreadingHTTPServer(("127.0.0.1", 8070), Channels).serve_forever()
```

Every folder name is now a channel: `http://127.0.0.1:8070/news/` and `http://127.0.0.1:8070/sport/` are two channels, each with a page 100. `channel.json` and `index.t42` answer 404, so each channel takes the defaults.

- **Channels needn't exist on disk.** `/users/ann/` and `/users/bob/` can each be a channel made from a database row; one program can serve thousands.
- **Live pages:** generate the page on each request, with an `ETag` so an unchanged page costs only "not modified"; or name a push address in `channel.json` and send changed pages over WebSocket or server-sent events (REQ-WEB-005).
- **Forms and sessions:** a form posts to the program inside the channel's folder; the program sets a cookie and answers with a TTI page or a redirect (Appendix C, REQ-WEB-010).
- **Mixed:** static files for most pages and one route for the live ones, or a reverse proxy in front.

The project's own end-to-end test server, `tests/web-server/sites.py`, works the second way: every page it serves is generated.

### 5.3 What every server must do (REQ-WEB-012)

1. **Answer 404 for a file that does not exist,** never a "not found" page with status 200. The browser relies on 404 to mean "no settings", "no bundle", "no contents" and "no such page". The body of a 404 does not matter: an HTML error page is fine.
2. **Send one of the accepted media types** (section 6).
3. **Stay within the limits:** 256 kilobytes for a page or `index.t42`, 16 kilobytes for `channel.json`, 64 kilobytes for `contents.txt`.
4. **Use only the fixed names, with no query strings.** The browser never adds a query to a channel's files, so `140.tti?user=ann` does not exist to it. Put variation in the folder path (`/users/ann/140.tti`) or in the session cookie.
5. **Keep each channel inside its folder.** Links, forms and redirects that leave it lead to another channel, through the tuning screen.

---

## 6. Media types

No teletext media type is registered, so real hosts send generic ones. Measured on 12 October 2026 with read-only header requests:

| Host | `.tti` | `.t42` |
|---|---|---|
| raw.githubusercontent.com (where `github.com/…/raw/…` redirects) | `text/plain; charset=utf-8` | `application/octet-stream` |
| jsDelivr (GitHub through a content delivery network) | `application/octet-stream` | `application/octet-stream` |
| feeds.nmsni.co.uk (NMS Ceefax) | `text/plain` | — |

- **IANA**, the official registry, has no teletext media type.
- **`mime-db`**, the list GitHub Pages, Netlify and most Node.js servers use, has no entry for `tti`, `t42`, `ttx` or `ep1`; those hosts send `application/octet-stream`.
- **Apache's `mime.types`** and **Python's `mimetypes`** have none either.
- **The extensions are shared:** `.tti` is also Template Toolkit's include files, and `.t42` is also PostScript Type 42 fonts.

So the browser accepts this project's own types and the generic ones, and then the content decides (REQ-WEB-027):

| File | Accepted media types |
|---|---|
| `NNN.tti` | `text/x-teletext-tti`, `text/plain`, `application/octet-stream` |
| `NNN.t42`, `index.t42` | `application/x-teletext-t42`, `application/octet-stream`, `text/plain` |
| `channel.json` | `application/json`, `text/plain`, `application/octet-stream` |
| `contents.txt` | `text/plain`, `application/octet-stream` |

- **Each file must pass the check for the format its name says:** a TTI file with a `PN` line; whole 42-byte T42 packets with valid addresses; a valid `channel.json` or `contents.txt`. A Template Toolkit file or a Type 42 font is refused, not misread.
- **Any other type is refused,** including `text/html`. That also catches hosts that answer a missing file with an HTML page and status 200.
- **On a server you configure,** send the project's own types, `text/x-teletext-tti` and `application/x-teletext-t42`; for nginx, one line in a `types` block. Nothing depends on it.

Registering `vnd.` types with IANA is possible later; nothing waits for it.

---

## 7. What is given up, and what is still open

**Given up, deliberately:**

- **Collections with other file names,** such as SPARK's `P100.tti`, are not served as they are. They are converted once into a channel folder, or read through a descriptor service with a template.
- **A broadcaster with several channels on one server** shows the tuning screen between them.
- **A folder name containing a dot** must be typed with its trailing slash, since without it the last segment reads as a file name.
- **TTI bundles:** `index.t42` is the only bundle; authors of TTI pages publish page files.

**Still open** (REQUIREMENTS.md Appendix D.2):

- whether to allow `index.tti` beside `index.t42`;
- whether sister channels that one server declares should skip the tuning screen between them;
- whether the tuning screen should flag host names that imitate others.
