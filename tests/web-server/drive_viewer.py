#!/usr/bin/env python3
"""Drives `ttx view` through a pseudo-terminal against the test server:
index, link row, form entry, plain-HTTP confirmation, submission and the
response page (REQ-NAV-009, REQ-WEB-011, REQ-SEC-008). Test code.

Usage: drive_viewer.py TTX PORT. Exit status 0 when every step passed.
"""

import os
import pty
import re
import select
import sys
import time


def main():
    ttx, port = sys.argv[1], sys.argv[2]
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(ttx, ["ttx", "view", f"http://127.0.0.1:{port}/service.json",
                       "--allow-local", "127.0.0.1", "--graphics", "text", "--no-sandbox"])
    output = bytearray()

    def pump(seconds):
        end = time.time() + seconds
        while time.time() < end:
            ready, _, _ = select.select([fd], [], [], 0.1)
            if ready:
                try:
                    output.extend(os.read(fd, 65536))
                except OSError:
                    return

    def send(keys, wait=0.4):
        os.write(fd, keys)
        pump(wait)

    def screen():
        return re.sub(r"\x1b\[[0-9;?]*[A-Za-z]", "", output.decode("utf-8", "replace"))

    results = {}
    pump(4)
    results["index"] = "NEOTELETEXT TEST SERVICE" in screen()
    results["link row"] = "Weather" in screen() and "Contact" in screen()
    output.clear()
    send(b"300", 3)
    results["form"] = "CONTACT US" in screen()
    for key in b"Ann":
        send(bytes([key]), 0.15)
    send(b"\t", 0.3)
    for key in b"ann@example.org":
        send(bytes([key]), 0.1)
    send(b"\t", 0.3)
    output.clear()
    send(b"\r", 2)
    results["confirmation"] = "UNENCRYPTED" in screen()
    output.clear()
    send(b"\r", 4)
    results["response page"] = "THANK YOU" in screen()
    try:
        send(b"q", 1)
    except OSError:
        pass
    for step, passed in results.items():
        print(f"{step}: {'ok' if passed else 'FAILED'}")
    sys.exit(0 if all(results.values()) else 1)


if __name__ == "__main__":
    main()
