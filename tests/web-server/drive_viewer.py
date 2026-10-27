#!/usr/bin/env python3
"""Drives `neottx view` through a pseudo-terminal against the test server:
index, link row, a page keyed by number and a coloured link followed
(REQ-NAV-001, REQ-NAV-011). Test code.

Usage: drive_viewer.py TTX PORT. Exit status 0 when every step passed.
"""

import os
import pty
import re
import select
import sys
import time


def main():
    neottx, port = sys.argv[1], sys.argv[2]
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(neottx, ["neottx", "view", f"http://127.0.0.1:{port}/service.json",
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
    results["link row"] = "Weather" in screen() and "News" in screen()
    output.clear()
    send(b"300", 3)
    results["keyed page"] = "TEST NEWS" in screen()
    output.clear()
    send(b"r", 3)
    results["coloured link"] = "NEOTELETEXT TEST SERVICE" in screen()
    try:
        send(b"q", 1)
    except OSError:
        pass
    for step, passed in results.items():
        print(f"{step}: {'ok' if passed else 'FAILED'}")
    sys.exit(0 if all(results.values()) else 1)


if __name__ == "__main__":
    main()
