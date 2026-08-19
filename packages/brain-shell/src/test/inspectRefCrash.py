#!/usr/bin/env python3
import os
import pty
import select
import sys
import time

BRAIN_SHELL_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"
PRELOAD_PATH = os.path.join(BRAIN_SHELL_DIR, "src", "preload.ts")
REF_PATH = os.path.join(BRAIN_SHELL_DIR, "src", "test", "referenceRunner.tsx")

m_fd, s_fd = pty.openpty()
env = dict(os.environ)
env["TERM"] = "xterm-256color"
env["COLUMNS"] = "80"
env["LINES"] = "24"
env["NODE_ENV"] = "production"

pid = os.fork()
if pid == 0:
    os.close(m_fd)
    os.setsid()
    os.dup2(s_fd, 0)
    os.dup2(s_fd, 1)
    os.dup2(s_fd, 2)
    os.close(s_fd)
    os.chdir(BRAIN_SHELL_DIR)
    os.execvpe("bun", ["bun", "run", "--preload", PRELOAD_PATH, REF_PATH], env)
else:
    os.close(s_fd)
    time.sleep(1.0)
    # Type something to trigger React reconciler state update
    os.write(m_fd, b"Hello Claude\nSecond line\x7f\x7f")
    time.sleep(0.5)
    chunks = []
    while True:
        r, _, _ = select.select([m_fd], [], [], 0.1)
        if m_fd in r:
            try:
                d = os.read(m_fd, 4096)
                if not d: break
                chunks.append(d.decode("utf-8", errors="replace"))
            except OSError:
                break
        else:
            break
    os.close(m_fd)
    os.kill(pid, 9)
    os.waitpid(pid, 0)
    raw = "".join(chunks)
    print("REF OUTPUT:")
    print(raw)
