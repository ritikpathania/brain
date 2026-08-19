#!/usr/bin/env python3
import os
import pty
import select
import sys
import time

BRAIN_SHELL_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"
ORIGINAL_SRC_DIR = "/Users/ritikpathania/Developer/src"

def test_spawn(name, cwd, cmd):
    m_fd, s_fd = pty.openpty()
    env = dict(os.environ)
    env["TERM"] = "xterm-256color"
    env["COLUMNS"] = "80"
    env["LINES"] = "24"

    pid = os.fork()
    if pid == 0:
        os.close(m_fd)
        os.setsid()
        os.dup2(s_fd, 0)
        os.dup2(s_fd, 1)
        os.dup2(s_fd, 2)
        os.close(s_fd)
        os.chdir(cwd)
        os.execvpe("bun", cmd, env)
    else:
        os.close(s_fd)
        chunks = []
        start = time.time()
        while time.time() - start < 1.5:
            r, _, _ = select.select([m_fd], [], [], 0.05)
            if m_fd in r:
                try:
                    d = os.read(m_fd, 4096)
                    if not d: break
                    chunks.append(d.decode("utf-8", errors="replace"))
                except OSError:
                    break
        os.close(m_fd)
        os.kill(pid, 9)
        os.waitpid(pid, 0)
        return "".join(chunks)

out_cand = test_spawn("Cand", BRAIN_SHELL_DIR, ["bun", "run", "--preload", "./src/preload.ts", "src/main.tsx"])
out_ref = test_spawn("Ref", ORIGINAL_SRC_DIR, ["bun", "run", "--preload", f"{BRAIN_SHELL_DIR}/src/preload.ts", f"{BRAIN_SHELL_DIR}/src/main.tsx"])

print("--- CAND OUTPUT (first 500 chars) ---")
print(repr(out_cand[:500]))
print("\n--- REF OUTPUT (first 500 chars) ---")
print(repr(out_ref[:500]))
