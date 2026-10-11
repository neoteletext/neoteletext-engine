#!/usr/bin/env python3
"""Drives `ttx view` through a pseudo-terminal while the test server stops
and starts again (REQ-WEB-009): stored pages stay viewable, a page that is
not stored says it is not available offline, and pages arrive again once
the server is back. Test code.

Usage: drive_offline.py TTX SERVER_SCRIPT. Exit status 0 when every step
passed.
"""

import os
import pty
import re
import select
import subprocess
import sys
import time


def start_server(script, port):
    server = subprocess.Popen([sys.executable, "-I", script, str(port)], stdout=subprocess.PIPE)
    return server, int(server.stdout.readline())


def main():
    ttx, script = sys.argv[1], sys.argv[2]
    server, port = start_server(script, 0)
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
    try:
        pump(4)
        results["online index"] = "NEOTELETEXT TEST SERVICE" in screen()
        server.terminate()
        server.wait()
        output.clear()
        # Page 150 was never fetched (page 200 may have been pushed).
        send(b"150", 3)
        results["unstored page offline"] = "Page not available offline" in screen()
        output.clear()
        send(b"100", 2)
        results["stored page offline"] = "NEOTELETEXT TEST SERVICE" in screen()
        server, port = start_server(script, port)
        output.clear()
        send(b"300", 4)
        results["back online"] = "CONTACT US" in screen()
        try:
            send(b"q", 1)
        except OSError:
            pass
    finally:
        server.terminate()
        server.wait()
    for step, passed in results.items():
        print(f"{step}: {'ok' if passed else 'FAILED'}")
    sys.exit(0 if results and all(results.values()) else 1)


if __name__ == "__main__":
    main()
