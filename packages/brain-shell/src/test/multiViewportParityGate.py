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
import json
import hashlib
import pyte
import tempfile

VIEWPORTS = [
    (80, 24),
    (100, 26),
    (120, 30),
    (182, 53)
]

def set_terminal_size(fd, cols, rows):
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)

def capture_screen(cmd_args, cwd, env_vars, cols, rows, initial_input="", max_timeout=3.5, idle_timeout=0.5):
    master_fd, slave_fd = pty.openpty()
    set_terminal_size(master_fd, cols, rows)

    env = dict(os.environ)
    env["TERM"] = "xterm-256color"
    env["COLORTERM"] = "truecolor"
    env["COLUMNS"] = str(cols)
    env["LINES"] = str(rows)
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

    screen = pyte.Screen(cols, rows)
    stream = pyte.Stream(screen)
    decoder = codecs.getincrementaldecoder('utf-8')('replace')

    start_time = time.time()
    last_data = time.time()
    raw_bytes = bytearray()
    sent_input = False

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

                if not sent_input and initial_input and len(raw_bytes) > 200:
                    time.sleep(0.1)
                    os.write(master_fd, initial_input.encode('utf-8'))
                    sent_input = True
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

    return screen

def run_parity_test_matrix():
    target_dir = os.path.expanduser("~/Developer/PyCharm/brain/packages/brain-shell")
    bun_bin = "/Users/ritikpathania/.bun/bin/bun"
    claude_bin = "/Users/ritikpathania/.local/bin/claude"
    preload_path = os.path.join(target_dir, "src", "preload.ts")
    main_path = os.path.join(target_dir, "src", "main.tsx")

    all_passed = True
    results = []

    with tempfile.TemporaryDirectory() as shared_config_dir:
        # Create identical settings.json with deterministic settings (non-deprecated active model)
        settings_path = os.path.join(shared_config_dir, "settings.json")
        with open(settings_path, "w") as f:
            json.dump({
                "model": "claude-sonnet-4-6",
                "promptSuggestionEnabled": False,
                "permissions": {"allow": []}
            }, f)

        try:
            import subprocess
            v_out = subprocess.check_output([claude_bin, "--version"], text=True)
            claude_ver = v_out.strip().split()[0]
        except Exception:
            claude_ver = "2.1.235"

        # Create identical .claude.json with trust accepted and notices suppressed
        claude_json_path = os.path.join(shared_config_dir, ".claude.json")
        with open(claude_json_path, "w") as f:
            json.dump({
                "hasCompletedOnboarding": True,
                "projects": {
                    target_dir: {
                        "hasTrustDialogAccepted": True
                    }
                },
                "shownTips": ["opus_1m_tip", "opus_1m", "tip_opus_1m", "opus_default_1m"],
                "opus1mMergeNoticeSeenCount": 10,
                "voiceNoticeSeenCount": 10,
                "lastReleaseNotesSeen": claude_ver,
                "lastOnboardingVersion": claude_ver
            }, f)

        # Pre-seed cache/changelog.md to prevent background network fetches from altering changelog cache
        cache_dir = os.path.join(shared_config_dir, "cache")
        os.makedirs(cache_dir, exist_ok=True)
        with open(os.path.join(cache_dir, "changelog.md"), "w") as f:
            f.write(f"# Changelog\n\n## {claude_ver}\n- Performance improvements\n")

        env_override = {
            "HOME": shared_config_dir,
            "CLAUDE_CONFIG_DIR": shared_config_dir,
            "ANTHROPIC_MODEL": "claude-sonnet-4-6",
            "CLAUDE_VERSION": claude_ver,
        }

        test_cases = [
            ("empty_composer", ""),
            ("active_composer", "Hello world")
        ]

        for cols, rows in VIEWPORTS:
            for state_name, initial_input in test_cases:
                c_screen = capture_screen([claude_bin, "--bare"], target_dir, env_override, cols, rows, initial_input)
                b_screen = capture_screen([bun_bin, "run", "--preload", preload_path, main_path, "--bare"], target_dir, env_override, cols, rows, initial_input)

                c_text = "\n".join(c_screen.display)
                b_text = "\n".join(b_screen.display)

                c_hash = hashlib.sha256(c_text.encode('utf-8')).hexdigest()
                b_hash = hashlib.sha256(b_text.encode('utf-8')).hexdigest()

                diff_cells = 0
                diff_rows = 0

                for r in range(rows):
                    c_row = c_screen.display[r]
                    b_row = b_screen.display[r]
                    if c_row != b_row:
                        diff_rows += 1
                        for c in range(min(len(c_row), len(b_row))):
                            if c_row[c] != b_row[c]:
                                diff_cells += 1
                        diff_cells += abs(len(c_row) - len(b_row))

                passed = (diff_cells == 0 and c_hash == b_hash)
                if not passed:
                    all_passed = False

                results.append({
                    "viewport": f"{cols}x{rows}",
                    "cells_total": cols * rows,
                    "state": state_name,
                    "diff_cells": diff_cells,
                    "diff_rows": diff_rows,
                    "c_hash": c_hash[:12],
                    "b_hash": b_hash[:12],
                    "passed": passed,
                    "c_display": c_screen.display,
                    "b_display": b_screen.display
                })

                if not passed:
                    print(f"  [!] MISMATCH in {cols}x{rows} ({state_name}): {diff_cells} differing cells, {diff_rows} differing rows")
                    for r in range(rows):
                        if c_screen.display[r] != b_screen.display[r]:
                            print(f"    Row {r:02d} CLAUDE: {repr(c_screen.display[r])}")
                            print(f"    Row {r:02d} BRAIN : {repr(b_screen.display[r])}")
                else:
                    print(f"  [✓] {cols}x{rows} ({state_name}): 100% MATCH ({cols*rows}/{cols*rows} cells, SHA-256: {c_hash[:12]})")

    print("\n" + "=" * 90)
    print(f"{'VIEWPORT':<12} | {'STATE':<18} | {'CELLS':<8} | {'DIFF CELLS':<12} | {'DIFF ROWS':<10} | {'HASH MATCH':<12} | {'STATUS'}")
    print("=" * 90)
    for r in results:
        status_str = "PASS" if r["passed"] else "FAIL"
        hash_match = "YES" if r["c_hash"] == r["b_hash"] else "NO"
        print(f"{r['viewport']:<12} | {r['state']:<18} | {r['cells_total']:<8} | {r['diff_cells']:<12} | {r['diff_rows']:<10} | {hash_match:<12} | {status_str}")
    print("=" * 90)

    return 0 if all_passed else 1

if __name__ == "__main__":
    sys.exit(run_parity_test_matrix())
