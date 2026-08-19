#!/usr/bin/env python3
import os
import pty
import select
import sys
import time
from terminalEmulator import VirtualTerminal

BRAIN_SHELL_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"
PRELOAD_PATH = os.path.join(BRAIN_SHELL_DIR, "src", "preload.ts")
MAIN_PATH = os.path.join(BRAIN_SHELL_DIR, "src", "main.tsx")
REF_PATH = os.path.join(BRAIN_SHELL_DIR, "src", "test", "referenceRunner.tsx")
VENDOR_CLAUDE_DIR = os.path.join(BRAIN_SHELL_DIR, "vendor", "claude")
DEV_SRC_DIR = "/Users/ritikpathania/Developer/src"

def get_settled_output(script_path, source_root, cols, rows, theme="dark"):
    m_fd, s_fd = pty.openpty()
    env = dict(os.environ)
    env["TERM"] = "xterm-256color"
    env["COLORTERM"] = "truecolor"
    env["COLUMNS"] = str(cols)
    env["LINES"] = str(rows)
    env["FORCE_COLOR"] = "3"
    env["CLAUDE_THEME"] = theme
    env["NODE_ENV"] = "production"
    env["DISABLE_AUTOUPDATER"] = "1"
    env["CLAUDE_SOURCE_ROOT"] = source_root

    pid = os.fork()
    if pid == 0:
        os.close(m_fd)
        os.setsid()
        os.dup2(s_fd, 0)
        os.dup2(s_fd, 1)
        os.dup2(s_fd, 2)
        os.close(s_fd)
        os.chdir(BRAIN_SHELL_DIR)
        os.execvpe("bun", ["bun", "run", "--preload", PRELOAD_PATH, script_path], env)
    else:
        os.close(s_fd)
        vt = VirtualTerminal(cols, rows)
        chunks = []
        start = time.time()
        last_data = time.time()
        while time.time() - start < 3.5:
            r, _, _ = select.select([m_fd], [], [], 0.05)
            if m_fd in r:
                try:
                    d = os.read(m_fd, 4096)
                    if not d: break
                    text = d.decode("utf-8", errors="replace")
                    chunks.append(text)
                    last_data = time.time()
                except OSError:
                    break
            else:
                if chunks and (time.time() - last_data > 0.25):
                    break
        os.close(m_fd)
        os.kill(pid, 9)
        os.waitpid(pid, 0)
        raw = "".join(chunks)
        vt.feed(raw)
        return vt, raw

scenarios = [
    (70, 37, "Compact 70x37"),
    (80, 24, "Canonical 80x24"),
    (100, 30, "Medium 100x30"),
    (120, 40, "Fullscreen 120x40"),
    (182, 53, "Ultrawide 182x53"),
]

for cols, rows, label in scenarios:
    vt_ref, raw_ref = get_settled_output(REF_PATH, DEV_SRC_DIR, cols, rows, "dark")
    vt_cand, raw_cand = get_settled_output(MAIN_PATH, VENDOR_CLAUDE_DIR, cols, rows, "dark")

    diffs = []
    for r in range(rows):
        for c in range(cols):
            c_ref = vt_ref.grid[r][c]
            c_cand = vt_cand.grid[r][c]
            if not c_ref.matches(c_cand):
                diffs.append((r, c, c_ref, c_cand))

    print(f"\n========================================================")
    print(f"Scenario: {label} ({cols}x{rows}) | Total Diffs: {len(diffs)} / {cols*rows}")
    print(f"========================================================")
    if not diffs:
        print("  >>> ZERO DIFFERENCES (100% MATCH) <<<")
    else:
        for r, c, cr, cc in diffs:
            print(f"  Row {r:2d}, Col {c:2d} | REF: char={repr(cr.char)} fg={cr.fg_rgb} bg={cr.bg_rgb} | CAND: char={repr(cc.char)} fg={cc.fg_rgb} bg={cc.bg_rgb}")
