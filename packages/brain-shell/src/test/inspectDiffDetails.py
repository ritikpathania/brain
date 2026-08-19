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

def get_settled_output(script_path, source_root, cols=80, rows=24, theme="dark"):
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
        import shutil
        bun_bin = shutil.which("bun") or "/Users/ritikpathania/.bun/bin/bun"
        env["PATH"] = os.environ.get("PATH", "") + ":/Users/ritikpathania/.bun/bin:/usr/local/bin:/usr/bin:/bin"
        os.execvpe(bun_bin, [bun_bin, "run", "--preload", PRELOAD_PATH, script_path, "--bare"], env)

    else:
        os.close(s_fd)
        vt = VirtualTerminal(cols, rows)
        chunks = []
        start = time.time()
        last_data = time.time()
        while time.time() - start < 4.0:
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
                if chunks and (time.time() - last_data > 0.4):
                    break
        os.close(m_fd)
        os.kill(pid, 9)
        os.waitpid(pid, 0)
        raw = "".join(chunks)
        vt.feed(raw)
        return vt, raw

print("Running Reference (Developer/src)...")
vt_ref, raw_ref = get_settled_output(REF_PATH, DEV_SRC_DIR, 80, 24, "dark")
print("Running Candidate (vendor/claude)...")
vt_cand, raw_cand = get_settled_output(MAIN_PATH, VENDOR_CLAUDE_DIR, 80, 24, "dark")

print("\n=== RAW REF LENGTH ===", len(raw_ref))
print("=== RAW CAND LENGTH ===", len(raw_cand))

print("\n=== REF PLAIN TEXT ===")
print(vt_ref.render_plain_text())

print("\n=== CAND PLAIN TEXT ===")
print(vt_cand.render_plain_text())

diffs = []
for r in range(24):
    for c in range(80):
        c_ref = vt_ref.grid[r][c]
        c_cand = vt_cand.grid[r][c]
        if not c_ref.matches(c_cand):
            diffs.append((r, c, c_ref, c_cand))

print(f"\nTotal cell diffs at 80x24 landing: {len(diffs)} / {80*24}")
if diffs:
    print("Sample diffs (first 10):")
    for r, c, cr, cc in diffs[:10]:
        print(f"  Row {r}, Col {c}: REF char={repr(cr.char)} fg={cr.fg_rgb} bg={cr.bg_rgb} vs CAND char={repr(cc.char)} fg={cc.fg_rgb} bg={cc.bg_rgb}")
else:
    print("\n>>> ZERO CELL DIFFERENCES! EXACT 100% PARITY! <<<")
