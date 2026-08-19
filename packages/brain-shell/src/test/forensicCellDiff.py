#!/usr/bin/env python3
import os
import pty
import select
import sys
import time
import termios
import struct
import fcntl
import codecs
import pyte
import tempfile
import shutil
import json

COLS = 80
ROWS = 24

def set_terminal_size(fd, cols, rows):
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)

def capture_screen(cmd_args, cwd, env_vars, max_timeout=3.0, idle_timeout=0.6):
    master_fd, slave_fd = pty.openpty()
    set_terminal_size(master_fd, COLS, ROWS)

    env = dict(os.environ)
    env.pop("ANTHROPIC_API_KEY", None)
    env.pop("ANTHROPIC_CUSTOM_API_KEY", None)
    env["TERM"] = "xterm-256color"
    env["COLORTERM"] = "truecolor"
    env["COLUMNS"] = str(COLS)
    env["LINES"] = str(ROWS)
    env["FORCE_COLOR"] = "3"
    env["NODE_ENV"] = "production"
    env["DISABLE_AUTOUPDATER"] = "1"
    env["CLAUDE_CODE_NO_FLICKER"] = "1"
    env.update(env_vars)

    pid = os.fork()
    if pid == 0:
        os.close(master_fd)
        os.setsid()
        os.dup2(slave_fd, 0)
        os.dup2(slave_fd, 1)
        os.dup2(slave_fd, 2)
        os.close(slave_fd)
        os.chdir(cwd)
        os.execvpe(cmd_args[0], cmd_args, env)
    else:
        os.close(slave_fd)

    screen = pyte.Screen(COLS, ROWS)
    stream = pyte.Stream(screen)
    decoder = codecs.getincrementaldecoder('utf-8')('replace')

    start_time = time.time()
    last_data = time.time()
    raw_bytes = bytearray()

    while time.time() - start_time < max_timeout:
        r, _, _ = select.select([master_fd], [], [], 0.05)
        if master_fd in r:
            try:
                chunk = os.read(master_fd, 4096)
                if not chunk:
                    break
                raw_bytes.extend(chunk)
                text = decoder.decode(chunk)
                stream.feed(text)
                last_data = time.time()
            except OSError:
                break
        else:
            if len(raw_bytes) > 0 and (time.time() - last_data > idle_timeout):
                break

    try:
        os.kill(pid, 9)
        os.waitpid(pid, os.WNOHANG)
    except:
        pass
    try:
        os.close(master_fd)
    except:
        pass

    return screen, bytes(raw_bytes)

def main():
    target_dir = os.path.expanduser("~/Developer/PyCharm/brain/packages/brain-shell")
    bun_bin = "/Users/ritikpathania/.bun/bin/bun"
    claude_bin = "/Users/ritikpathania/.local/bin/claude"
    preload_path = os.path.join(target_dir, "src", "preload.ts")
    main_path = os.path.join(target_dir, "src", "main.tsx")

    try:
        import subprocess
        v_out = subprocess.check_output([claude_bin, "--version"], text=True)
        claude_ver = v_out.strip().split()[0]
    except Exception:
        claude_ver = "2.1.235"

    shared_home = tempfile.mkdtemp(prefix="forensic_diff_")
    try:
        claude_dir = os.path.join(shared_home, ".claude")
        os.makedirs(claude_dir, exist_ok=True)
        with open(os.path.join(claude_dir, "settings.json"), "w") as f:
            json.dump({
                "model": "claude-sonnet-4-6",
                "promptSuggestionEnabled": False,
            }, f)
        with open(os.path.join(shared_home, ".claude.json"), "w") as f:
            json.dump({
                "hasCompletedOnboarding": True,
                "projects": {target_dir: {"hasTrustDialogAccepted": True}},
                "shownTips": ["opus_1m_tip"],
                "opus1mMergeNoticeSeenCount": 10,
                "lastReleaseNotesSeen": claude_ver,
                "lastOnboardingVersion": claude_ver,
            }, f)

        env_override = {
            "HOME": shared_home,
            "CLAUDE_CONFIG_DIR": shared_home,
            "ANTHROPIC_MODEL": "claude-sonnet-4-6",
            "CLAUDE_VERSION": claude_ver,
        }

        print(f"[*] Capturing Reference Claude Screen in 80x24 at {target_dir}...")
        claude_screen, _ = capture_screen([claude_bin, "--bare"], target_dir, env_override)

        print(f"[*] Capturing Brain Shell Screen in 80x24 at {target_dir}...")
        brain_screen, _ = capture_screen([bun_bin, "run", "--preload", preload_path, main_path, "--bare"], target_dir, env_override)

    finally:
        for _ in range(5):
            if os.path.exists(shared_home):
                shutil.rmtree(shared_home, ignore_errors=True)
                if not os.path.exists(shared_home):
                    break
                time.sleep(0.05)

    print("\n" + "=" * 80)
    print("                 REFERENCE CLAUDE 80x24 CELL GRID DUMP                  ")
    print("=" * 80)
    for idx, line in enumerate(claude_screen.display):
        print(f"[{idx:02d}] {line}")

    print("\n" + "=" * 80)
    print("                   BRAIN SHELL 80x24 CELL GRID DUMP                     ")
    print("=" * 80)
    for idx, line in enumerate(brain_screen.display):
        print(f"[{idx:02d}] {line}")

    print("\n" + "=" * 80)
    print("                        ROW-BY-ROW CELL DIFF                            ")
    print("=" * 80)
    diff_rows = 0
    for idx in range(ROWS):
        c_line = claude_screen.display[idx]
        b_line = brain_screen.display[idx]
        if c_line != b_line:
            diff_rows += 1
            print(f"Row {idx:02d} DIFF:")
            print(f"  CLAUDE: {repr(c_line)}")
            print(f"  BRAIN : {repr(b_line)}")
        else:
            print(f"Row {idx:02d} MATCH: {repr(c_line)}")

    print("\n" + "=" * 80)
    print(f"TOTAL MISMATCHED ROWS: {diff_rows} / {ROWS}")
    print("=" * 80)

if __name__ == "__main__":
    main()
