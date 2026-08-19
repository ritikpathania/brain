#!/usr/bin/env python3
import os
import sys
import pty
import select
import time
import struct
import termios
import fcntl
import codecs
import pyte
import hashlib
import tempfile
import json

def set_terminal_size(fd, cols, rows):
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)

def run_interactive_session(target_dir, key_sequence, cols=80, rows=24, initial_env=None):
    master_fd, slave_fd = pty.openpty()
    set_terminal_size(master_fd, cols, rows)

    with tempfile.TemporaryDirectory() as shared_home:
        claude_dir = os.path.join(shared_home, ".claude")
        os.makedirs(claude_dir, exist_ok=True)
        with open(os.path.join(claude_dir, "settings.json"), "w") as f:
            json.dump({
                "model": "claude-sonnet-4-6",
                "promptSuggestionEnabled": False,
                "permissions": {"defaultMode": "default"}
            }, f)
        with open(os.path.join(shared_home, ".claude.json"), "w") as f:
            json.dump({
                "hasCompletedOnboarding": True,
                "customApiKeyApproved": True,
                "projects": {
                    target_dir: {
                        "hasTrustDialogAccepted": True,
                        "hasCustomApiKeyApproved": True
                    }
                },
                "shownTips": ["opus_1m_tip"],
                "opus1mMergeNoticeSeenCount": 10,
                "lastReleaseNotesSeen": "2.1.234",
                "lastOnboardingVersion": "2.1.234"
            }, f)

        env = dict(os.environ)
        if "ANTHROPIC_API_KEY" in env:
            del env["ANTHROPIC_API_KEY"]
        env["HOME"] = shared_home
        env["TERM"] = "xterm-256color"
        env["COLORTERM"] = "truecolor"
        env["COLUMNS"] = str(cols)
        env["LINES"] = str(rows)
        env["FORCE_COLOR"] = "3"
        env["NODE_ENV"] = "production"
        env["DISABLE_AUTOUPDATER"] = "1"
        env["CLAUDE_CODE_NO_FLICKER"] = "1"
        env["ANTHROPIC_MODEL"] = "claude-sonnet-4-6"

        if initial_env:
            env.update(initial_env)

        preload_path = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/preload.ts"
        main_path = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/main.tsx"

        pid = os.fork()
        if pid == 0:
            os.close(master_fd)
            os.setsid()
            os.dup2(slave_fd, 0)
            os.dup2(slave_fd, 1)
            os.dup2(slave_fd, 2)
            os.close(slave_fd)
            os.chdir(target_dir)
            os.execvpe("/Users/ritikpathania/.bun/bin/bun", ["bun", "run", "--preload", preload_path, main_path, "--bare"], env)
        else:
            os.close(slave_fd)

        screen = pyte.Screen(cols, rows)
        stream = pyte.Stream(screen)
        decoder = codecs.getincrementaldecoder("utf-8")("replace")
        start_time = time.time()
        last_data = time.time()
        raw = bytearray()
        sent_input = False

        while time.time() - start_time < 5.0:
            r, _, _ = select.select([master_fd], [], [], 0.05)
            if master_fd in r:
                try:
                    chunk = os.read(master_fd, 4096)
                    if not chunk: break
                    raw.extend(chunk)
                    stream.feed(decoder.decode(chunk))
                    last_data = time.time()
                    if not sent_input and key_sequence and any("❯" in l for l in screen.display):
                        time.sleep(0.08)
                        for item in key_sequence:
                            if isinstance(item, str):
                                os.write(master_fd, item.encode("utf-8"))
                            elif isinstance(item, bytes):
                                os.write(master_fd, item)
                            elif isinstance(item, (int, float)):
                                time.sleep(item)
                        sent_input = True
                        last_data = time.time()
                except OSError: break
            else:
                has_prompt = any("❯" in l for l in screen.display)
                if len(raw) > 0 and (sent_input or not key_sequence) and has_prompt and (time.time() - last_data > 0.5):
                    break

        try:
            os.kill(pid, 9)
            os.waitpid(pid, os.WNOHANG)
            os.close(master_fd)
        except: pass

        return screen.display

if __name__ == "__main__":
    test_type = sys.argv[1]
    target_dir = sys.argv[2]
    
    if test_type == "idle":
        display = run_interactive_session(target_dir, [])
        for i, l in enumerate(display):
            print(f"[{i:02d}] {l}")
    elif test_type == "typing":
        display = run_interactive_session(target_dir, ["echo test", 0.4])
        for i, l in enumerate(display):
            print(f"[{i:02d}] {l}")
    elif test_type == "slash":
        # Type '/' and wait for suggestions overlay
        display = run_interactive_session(target_dir, ["/", 0.4])
        for i, l in enumerate(display):
            print(f"[{i:02d}] {l}")
    elif test_type == "slash_filter":
        # Type '/do' to filter to /doctor
        display = run_interactive_session(target_dir, ["/do", 0.4])
        for i, l in enumerate(display):
            print(f"[{i:02d}] {l}")
    elif test_type == "slash_tab":
        # Type '/do' then Tab to complete /doctor
        display = run_interactive_session(target_dir, ["/do", 0.2, "\t", 0.3])
        for i, l in enumerate(display):
            print(f"[{i:02d}] {l}")
    elif test_type == "slash_escape":
        # Type '/' then Double-Escape to clear input and dismiss
        display = run_interactive_session(target_dir, ["/", 0.2, "\x1b\x1b", 0.3])
        for i, l in enumerate(display):
            print(f"[{i:02d}] {l}")
    elif test_type == "at_path":
        # Type '@src/' to trigger file completion in synthetic-repo
        display = run_interactive_session(target_dir, ["@src/", 0.4])
        for i, l in enumerate(display):
            print(f"[{i:02d}] {l}")
    elif test_type == "at_tab":
        # Type '@src/d' and Tab to complete @src/dispatcher.rs
        display = run_interactive_session(target_dir, ["@src/d", 0.2, "\t", 0.3])
        for i, l in enumerate(display):
            print(f"[{i:02d}] {l}")
    elif test_type == "bash_mode":
        # Type '!' to enter bash mode
        display = run_interactive_session(target_dir, ["!", 0.3])
        for i, l in enumerate(display):
            print(f"[{i:02d}] {l}")
    elif test_type == "background_mode":
        # Type '&' to enter background prompt mode
        display = run_interactive_session(target_dir, ["&analyze codebase", 0.3])
        for i, l in enumerate(display):
            print(f"[{i:02d}] {l}")
