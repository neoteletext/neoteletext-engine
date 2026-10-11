#!/usr/bin/env python3
"""Local teletext channels for the browser's end-to-end tests (REQ-TST-081).

Test code, not part of the library. Every file is generated on request, so
it also shows that a program and a folder of files look the same to the
browser (decision E.70, docs/channels.md). Each server listens on its own
port of 127.0.0.1:

  news       a channel at the root: channel.json, contents.txt, TTI pages
             served as text/plain, links to the weather channel, a form
  weather    a channel without channel.json: index.t42 holds pages 100 and
             300, and 200.t42 and 300.t42 are page files, all served as
             application/octet-stream
  redirect   its channel.json redirects to the weather channel
  hostile    a channel whose pages try to forge the receiver's row 0 and
             the tuning screen, and one of whose files never ends
  pair       two channels in two folders of one origin, /one/ and /two/:
             their own cookies, a link and a redirect between them, a
             contents file, and a missing page answered as HTML with 200

Standard library only. Prints the ports as one JSON line, then serves until
it is killed.

Usage: sites.py
"""

import hashlib
import hmac
import http.server
import json
import os
import sys
import threading
import time

SECRET = os.urandom(16)


def esc(code):
    """A control code in the TTI escape form (REQ-INP-013)."""
    return "\x1b" + chr(code + 0x40)


RED, GREEN, YELLOW, BLUE, CYAN, WHITE = 1, 2, 3, 4, 6, 7
DOUBLE_HEIGHT = 0x0D


def tti(lines):
    return "\r\n".join(lines + [""])


NEWS_PAGES = {
    "100": tti([
        "DE,News index",
        "PN,10000",
        "OL,1," + esc(YELLOW) + esc(DOUBLE_HEIGHT) + "TEST NEWS",
        "OL,4," + esc(WHITE) + "Top story ...................... 200",
        "OL,5," + esc(WHITE) + "Contact form ................... 300",
        "OL,6," + esc(WHITE) + "Weather (another site) ......... 300",
        "OL,8," + esc(CYAN) + "Served over plain HTTP by sites.py.",
        "LN,R,200,Story",
        "LN,G,300,Contact",
        "LN,Y,weather:300,Weather",
        "LN,C,http://127.0.0.1:{weather}/200.t42,Radar",
        "LN,I,100,",
    ]),
    "200": tti([
        "PN,20000",
        "OL,1," + esc(GREEN) + "TOP STORY",
        "OL,3," + esc(WHITE) + "Teletext returns to the web.",
        "OL,5," + esc(WHITE) + "Back to the index: 100",
        "OL,7," + esc(WHITE) + "Weather forecast: 400",
        "OL,8," + esc(WHITE) + "Somewhere else",
        "LK,400,http://127.0.0.1:{weather}/300.t42",
        "LT,8,1,9,http://127.0.0.1:{hostile}/",
        "LN,R,100,Index",
    ]),
}

NEWS_CONTENTS = "\n".join([
    "# The news channel's contents (REQ-WEB-025).",
    "s\tNews",
    "p\t100\tIndex",
    "p\t200\tTop story\t2026-10-12",
    "s\tElsewhere",
    "x\tweather:300\tWeather forecast",
    "q\ta kind from a later version",
    "",
])

PAIR_CONTENTS = "\n".join([
    "s\tChannel one",
    "p\t100\tFront page",
    "x\thttp://127.0.0.1:{port}/two/\tChannel two",
    "",
])

FORM_PAGE = tti([
    "PN,30000",
    "DE,Contact form",
    "FT,{token}",
    "OL,1," + esc(YELLOW) + "CONTACT US",
    "OL,3,Name:  ...............................",
    "FD,TEXT,name,7,31,50,Y,,",
    "OL,8,  < SEND MESSAGE >",
    "FD,SUBMIT,send,2,16,Send message,send_contact",
    "FS,POST,/contact",
    "FP,30100,30200",
])

THANKS_PAGE = tti([
    "PN,30100",
    "OL,1," + esc(GREEN) + "THANK YOU",
    "OL,3,Your message was received.",
])

# Hamming 8/4 (EN 300 706 clause 8.2): the code byte for each nibble.
HAMMING_8_4 = [0x15, 0x02, 0x49, 0x5E, 0x64, 0x73, 0x38, 0x2F,
               0xD0, 0xC7, 0x8C, 0x9B, 0xA1, 0xB6, 0xFD, 0xEA]
# Header control bit C4, "erase page", in the fourth header nibble.
ERASE_PAGE = 0x8


def odd(byte):
    """A 7-bit code with odd parity (EN 300 706 clause 8.1)."""
    return byte | (0x80 if bin(byte).count("1") % 2 == 0 else 0)


def mrag(magazine, row):
    """The magazine and row address group: two Hamming 8/4 bytes."""
    m = magazine % 8
    return bytes([HAMMING_8_4[m | ((row & 1) << 3)], HAMMING_8_4[row >> 1]])


def text40(text):
    codes = [ord(c) if isinstance(c, str) else c for c in text]
    codes = (codes + [0x20] * 40)[:40]
    return bytes(odd(c & 0x7F) for c in codes)


def t42_page(mpp, rows, header="SITES.PY"):
    """Packets of one page: a header and rows. `rows` maps row numbers to
    lists of codes or strings."""
    magazine, tens, units = mpp >> 8, (mpp >> 4) & 0xF, mpp & 0xF
    head = bytes(HAMMING_8_4[n] for n in [units, tens, 0, ERASE_PAGE, 0, 0, 0, 0])
    packets = [mrag(magazine, 0) + head + text40(header)[:32]]
    for row, text in sorted(rows.items()):
        packets.append(mrag(magazine, row) + text40(text))
    return b"".join(packets)


def codes(*parts):
    out = []
    for part in parts:
        out.extend([part] if isinstance(part, int) else [ord(c) for c in part])
    return out


WEATHER = {
    0x100: {1: codes(CYAN, DOUBLE_HEIGHT, "WEATHER SITE"), 4: codes(WHITE, "Forecast ............ 300"),
            5: codes(WHITE, "Rain radar .......... 200")},
    0x300: {1: codes(GREEN, DOUBLE_HEIGHT, "WEATHER FORECAST"), 4: codes(WHITE, "Sunny, 21 degrees.")},
    0x200: {1: codes(BLUE, DOUBLE_HEIGHT, "RAIN RADAR"), 4: codes(WHITE, "No rain today.")},
}

HOSTILE_FRONT = {
    # The page's own header text claims to be a bank; row 1 and row 2 draw
    # a fake tuning screen.
    0x100: {1: codes(YELLOW, "GO? bank.example  Enter: go"),
            2: codes(WHITE, "You are now on bank.example"),
            4: codes(WHITE, "Type your password ........ 200")},
}


def token_for(session):
    return hmac.new(SECRET, session.encode(), hashlib.sha256).hexdigest()[:32]


class Site(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    name = "?"
    ports = {}

    def log_message(self, fmt, *args):
        if os.environ.get("TTX_SERVER_LOG"):
            sys.stderr.write(self.name + ": " + fmt % args + "\n")

    def send(self, status, body, media="text/x-teletext-tti", extra=()):
        data = body.encode("latin-1") if isinstance(body, str) else body
        self.send_response(status)
        self.send_header("Content-Type", media)
        self.send_header("Content-Length", str(len(data)))
        for header, value in extra:
            self.send_header(header, value)
        self.end_headers()
        self.wfile.write(data)

    def not_found(self):
        self.send(404, "not here", "text/plain")

    def do_GET(self):
        path = self.path.split("?")[0]
        getattr(self, "site_" + self.name)(path)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(min(length, 65536)).decode("latin-1")
        if self.name == "news" and self.path == "/contact" and "name=" in body:
            return self.send(303, "", "text/plain", [("Location", "/thanks")])
        return self.send(400, "bad", "text/plain")

    def site_news(self, path):
        session = "s1"
        # Served the way raw.githubusercontent.com serves them (decision
        # E.70 item 11).
        plain = "text/plain; charset=utf-8"
        if path == "/channel.json":
            settings = {
                "name": "Test News",
                "services": [{"name": "weather", "display_name": "Weather",
                              "channel": "http://127.0.0.1:%d/" % self.ports["weather"]}],
                "aliases": {"500": "weather:200", "600": "http://127.0.0.1:%d/300.t42" % self.ports["weather"]},
            }
            return self.send(200, json.dumps(settings), "application/json",
                             [("Set-Cookie", "session=v1.%s; Path=/; HttpOnly; SameSite=Lax" % session)])
        if path == "/contents.txt":
            return self.send(200, NEWS_CONTENTS, plain)
        if path == "/thanks":
            return self.send(200, THANKS_PAGE)
        if path.endswith(".tti"):
            page = path[1:-len(".tti")]
            if page == "300":
                return self.send(200, FORM_PAGE.format(token=token_for(session)), plain)
            if page == "301":
                return self.send(200, THANKS_PAGE, plain)
            if page in NEWS_PAGES:
                return self.send(200, NEWS_PAGES[page].format(**self.ports), plain)
        return self.not_found()

    def site_weather(self, path):
        binary = "application/octet-stream"
        if path == "/index.t42":
            front = t42_page(0x100, WEATHER[0x100], "WEATHER 100") + t42_page(0x300, WEATHER[0x300], "WEATHER 300")
            return self.send(200, front, binary)
        if path == "/200.t42":
            return self.send(200, t42_page(0x200, WEATHER[0x200], "WEATHER 200"), binary)
        if path == "/300.t42":
            return self.send(200, t42_page(0x300, WEATHER[0x300], "WEATHER 300"), binary)
        return self.not_found()

    def site_pair(self, path):
        plain = "text/plain"
        port = self.ports["pair"]
        cookies = self.headers.get("Cookie", "none")
        if path == "/one/channel.json":
            return self.send(200, json.dumps({"name": "Channel One"}), plain,
                             [("Set-Cookie", "one=1; Path=/")])
        if path == "/one/contents.txt":
            return self.send(200, PAIR_CONTENTS.format(port=port), plain)
        if path == "/one/100.tti":
            return self.send(200, tti([
                "PN,10000",
                "OL,1," + esc(YELLOW) + "CHANNEL ONE",
                "OL,3," + esc(WHITE) + "Cookies here: " + cookies,
                "OL,5," + esc(WHITE) + "Channel two ....... 200",
                "LK,200,http://127.0.0.1:%d/two/100.tti" % port,
            ]), plain)
        if path == "/one/300.tti":
            return self.send(302, "", plain, [("Location", "/two/100.tti")])
        if path == "/one/400.tti":
            # A host that answers a missing file with an HTML page and 200.
            return self.send(200, "<html><body>Not found</body></html>", "text/html")
        if path == "/two/100.tti":
            return self.send(200, tti([
                "PN,10000",
                "OL,1," + esc(CYAN) + "CHANNEL TWO",
                "OL,3," + esc(WHITE) + "Cookies here: " + cookies,
            ]), plain)
        return self.not_found()

    def site_redirect(self, path):
        location = "http://127.0.0.1:%d/" % self.ports["weather"]
        return self.send(302, "", "text/plain", [("Location", location)])

    def site_hostile(self, path):
        if path == "/index.t42":
            return self.send(200, t42_page(0x100, HOSTILE_FRONT[0x100], "bank.example   12:00:00"),
                             "application/x-teletext-t42")
        if path == "/200.t42":
            # A body that claims to be longer than it is, then stalls.
            self.send_response(200)
            self.send_header("Content-Type", "application/x-teletext-t42")
            self.send_header("Content-Length", "100000")
            self.end_headers()
            self.wfile.write(b"\x15" * 100)
            self.wfile.flush()
            time.sleep(30)
            return None
        return self.not_found()


def main():
    names = ["news", "weather", "redirect", "hostile", "pair"]
    servers = {}
    for name in names:
        handler = type(name, (Site,), {"name": name})
        servers[name] = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    ports = {name: server.server_address[1] for name, server in servers.items()}
    Site.ports.update(ports)
    for server in servers.values():
        server.daemon_threads = True
        threading.Thread(target=server.serve_forever, daemon=True).start()
    print(json.dumps(ports), flush=True)
    try:
        while True:
            time.sleep(3600)
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
