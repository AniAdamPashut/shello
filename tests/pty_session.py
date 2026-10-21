#!/usr/bin/env python3
"""pty_session.py INPUT CMD...: run CMD on a pty and type INPUT's lines into it.

nushell's line editor asks the terminal where the cursor is (ESC [ 6 n) and
waits for the answer, which a pipe into `script` can't give. This answers
those queries and types each line once the terminal output has gone quiet.
The terminal output goes to stdout; exits 1 if the session outlived 60s.
"""
import os
import pty
import select
import sys
import time

QUIET = 1.0
CURSOR_QUERY = b"\x1b[6n"


def main():
    lines = open(sys.argv[1]).read().splitlines()
    pid, fd = pty.fork()
    if pid == 0:
        os.execvp(sys.argv[2], sys.argv[2:])

    sent = 0
    quiet_since = time.time()
    deadline = time.time() + 60
    while time.time() < deadline:
        ready, _, _ = select.select([fd], [], [], 0.2)
        if ready:
            try:
                data = os.read(fd, 65536)
            except OSError:
                data = b""
            if not data:
                break
            sys.stdout.buffer.write(data)
            quiet_since = time.time()
            os.write(fd, b"\x1b[24;1R" * data.count(CURSOR_QUERY))
        elif sent < len(lines) and time.time() - quiet_since > QUIET:
            os.write(fd, lines[sent].encode() + b"\r")
            sent += 1
            quiet_since = time.time()
        elif sent == len(lines) and time.time() - quiet_since > 3 * QUIET:
            break

    sys.stdout.buffer.flush()
    done, _ = os.waitpid(pid, os.WNOHANG)
    if not done:
        os.kill(pid, 9)
        sys.exit(1)


main()
