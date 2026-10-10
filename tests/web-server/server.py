#!/usr/bin/env python3
"""A local web teletext test server (REQ-WEB-013, REQ-WEB-012).

Test code, not part of the library. Serves a service descriptor, TTI pages
with ETag validators, a versioned session cookie, a form with a
session-bound token, server-sent event and WebSocket push, and fault
injection. Standard library only.

Usage: server.py PORT
"""

import base64
import hashlib
import hmac
import http.server
import os
import socketserver
import sys
import threading
import time

SECRET = os.urandom(16)
# REQ-WEB-012: the session lives in a cookie whose value starts with a
# format version.
SESSION_VERSION = "v1"


def esc(code):
    """A control code in the TTI escape form (REQ-INP-013)."""
    return "\x1b" + chr(code + 0x40)


RED, GREEN, YELLOW, CYAN, WHITE = 1, 2, 3, 6, 7
DOUBLE_HEIGHT = 0x0D

PAGES = {
    "100": "\r\n".join([
        "DE,Test service index",
        "PN,10000",
        "PS,8000",
        "OL,1," + esc(YELLOW) + esc(DOUBLE_HEIGHT) + "TELLYTXT TEST SERVICE",
        "OL,4," + esc(WHITE) + "Weather ......................... 200",
        "OL,5," + esc(WHITE) + "Contact form .................... 300",
        "OL,7," + esc(CYAN) + "Served over plain HTTP from a",
        "OL,8," + esc(CYAN) + "local test server.",
        "LN,R,200,Weather",
        "LN,G,300,Contact",
        "LN,Y,other:100,Other",
        "LN,I,100,",
        "",
    ]),
    "200": "\r\n".join([
        "PN,20000",
        "OL,1," + esc(GREEN) + "WEATHER",
        "OL,3," + esc(WHITE) + "Sunny, 21 degrees.",
        "LN,R,100,Index",
        "",
    ]),
}

FORM_PAGE = "\r\n".join([
    "PN,30000",
    "DE,Contact form",
    "FT,{token}",
    "OL,1," + esc(YELLOW) + "CONTACT US",
    "OL,3,Name:  ...............................",
    "FD,TEXT,name,7,31,50,Y,,",
    "OL,4,Email: ...............................",
    "FD,EMAIL,email,7,31,100,Y,",
    "OL,8,  < SEND MESSAGE >",
    "FD,SUBMIT,send,2,16,Send message,send_contact",
    "FS,POST,/contact",
    "FP,30100,30200",
    "OL,22,Or telephone 0800 000 000.",
    "",
])

THANKS_PAGE = "\r\n".join([
    "PN,30100",
    "OL,1," + esc(GREEN) + "THANK YOU",
    "OL,3,Your message was received.",
    "",
])

DESCRIPTOR = """{{
  "name": "Test service",
  "initial_page": "100",
  "page_template": "/pages/{{mpp}}.tti",
  "push": {{"address": "/push", "method": "sse"}},
  "services": [{{"name": "other", "display_name": "Other service", "descriptor": "http://127.0.0.1:{port}/other.json"}}]
}}"""


def token_for(session):
    return hmac.new(SECRET, session.encode(), hashlib.sha256).hexdigest()[:32]


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):
        if os.environ.get("TTX_SERVER_LOG"):
            sys.stderr.write("server: " + fmt % args + "\n")

    def session(self):
        for part in self.headers.get("Cookie", "").split(";"):
            name, _, value = part.strip().partition("=")
            if name == "session" and value.startswith(SESSION_VERSION + "."):
                return value
        return None

    def send(self, status, body, media="text/x-teletext-tti", extra=()):
        data = body.encode("latin-1") if isinstance(body, str) else body
        self.send_response(status)
        self.send_header("Content-Type", media)
        self.send_header("Content-Length", str(len(data)))
        for name, value in extra:
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        path = self.path.split("?")[0]
        cookies = []
        session = self.session()
        if session is None:
            session = SESSION_VERSION + "." + base64.urlsafe_b64encode(os.urandom(9)).decode()
            cookies.append(("Set-Cookie", "session=" + session + "; Path=/; HttpOnly; SameSite=Lax"))
        port = self.server.server_address[1]
        if path == "/service.json":
            return self.send(200, DESCRIPTOR.format(port=port), "application/json", cookies)
        if path == "/other.json":
            other = DESCRIPTOR.format(port=port).replace("Test service", "Other service")
            return self.send(200, other, "application/json", cookies)
        if path.startswith("/pages/") and path.endswith(".tti"):
            page = path[len("/pages/"):-len(".tti")]
            if page == "300":
                body = FORM_PAGE.format(token=token_for(session))
            elif page in PAGES:
                body = PAGES[page]
            else:
                return self.send(404, "", "text/plain", cookies)
            etag = '"' + hashlib.sha256(body.encode("latin-1")).hexdigest()[:16] + '"'
            if self.headers.get("If-None-Match") == etag:
                self.send_response(304)
                self.send_header("ETag", etag)
                self.send_header("Content-Length", "0")
                self.end_headers()
                return None
            return self.send(200, body, "text/x-teletext-tti", cookies + [("ETag", etag)])
        if path == "/push":
            return self.push_sse()
        if path == "/ws":
            return self.push_websocket()
        return self.fault(path)

    def push_sse(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        page = PAGES["200"].replace("Sunny, 21 degrees.", "Pushed: rain, 15 degrees.")
        try:
            for _ in range(3):
                lines = "".join("data: " + line + "\n" for line in page.split("\r\n") if line)
                self.wfile.write(("event: page\nid: 1\n" + lines + "\n").encode("latin-1"))
                self.wfile.flush()
                time.sleep(1)
        except (BrokenPipeError, ConnectionResetError):
            pass
        self.close_connection = True
        return None

    def push_websocket(self):
        key = self.headers.get("Sec-WebSocket-Key", "")
        accept = base64.b64encode(hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        self.send_response(101)
        self.send_header("Upgrade", "websocket")
        self.send_header("Connection", "Upgrade")
        self.send_header("Sec-WebSocket-Accept", accept)
        self.end_headers()
        payload = PAGES["200"].encode("latin-1")
        header = bytes([0x81, 126]) + len(payload).to_bytes(2, "big") if len(payload) >= 126 else bytes([0x81, len(payload)])
        try:
            self.wfile.write(header + payload)
            self.wfile.flush()
            time.sleep(2)
        except (BrokenPipeError, ConnectionResetError):
            pass
        self.close_connection = True
        return None

    def fault(self, path):
        """Fault injection (REQ-WEB-013)."""
        if path == "/fault/slow":
            time.sleep(40)
            return self.send(200, PAGES["200"])
        if path == "/fault/drop":
            self.close_connection = True
            self.wfile.write(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\npartial")
            return None
        if path == "/fault/big":
            return self.send(200, "OL,1," + "X" * (300 * 1024))
        if path == "/fault/media":
            return self.send(200, "<html></html>", "text/html")
        if path == "/fault/loop":
            self.send_response(302)
            self.send_header("Location", "/fault/loop")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return None
        if path == "/fault/ambiguous":
            self.wfile.write(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n")
            return None
        return self.send(404, "", "text/plain")

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(min(length, 65536)).decode("latin-1")
        fields = dict(part.split("=", 1) for part in body.split("&") if "=" in part)
        session = self.session()
        if self.path != "/contact" or session is None:
            return self.send(404, "", "text/plain")
        if not hmac.compare_digest(fields.get("ttx_token", ""), token_for(session)):
            # Forged or expired token (REQ-WEB-012).
            return self.send(403, "", "text/plain")
        return self.send(200, THANKS_PAGE)


class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = True


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 0
    server = Server(("127.0.0.1", port), Handler)
    print(server.server_address[1], flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
