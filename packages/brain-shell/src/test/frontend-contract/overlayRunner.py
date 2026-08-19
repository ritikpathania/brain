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
import tempfile
import json
import shutil

def set_terminal_size(fd, cols, rows):
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)

def run_overlay_session(target_dir, key_sequence, cols=80, rows=24, initial_env=None):
    master_fd, slave_fd = pty.openpty()
    set_terminal_size(master_fd, cols, rows)

    shared_home = tempfile.mkdtemp()
    try:
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
        env["CLAUDE_CODE_OAUTH_TOKEN"] = "mock-token-for-test"

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

        while time.time() - start_time < 6.0:
            r, _, _ = select.select([master_fd], [], [], 0.05)
            if master_fd in r:
                try:
                    chunk = os.read(master_fd, 4096)
                    if not chunk: break
                    raw.extend(chunk)
                    stream.feed(decoder.decode(chunk))
                    last_data = time.time()
                    if not sent_input and len(raw) > 200:
                        time.sleep(0.15)
                        for item in key_sequence:
                            if isinstance(item, str):
                                os.write(master_fd, item.encode("utf-8"))
                            elif isinstance(item, bytes):
                                os.write(master_fd, item)
                            elif isinstance(item, (int, float)):
                                time.sleep(item)
                        sent_input = True
                except OSError: break
            else:
                if len(raw) > 0 and sent_input and time.time() - last_data > 0.6:
                    break

        try:
            os.kill(pid, 9)
            os.waitpid(pid, os.WNOHANG)
            os.close(master_fd)
        except: pass

        return screen.display
    finally:
        shutil.rmtree(shared_home, ignore_errors=True)

if __name__ == "__main__":
    action = sys.argv[1]
    target_dir = sys.argv[2]
    cols = int(sys.argv[3]) if len(sys.argv) > 3 else 80
    rows = int(sys.argv[4]) if len(sys.argv) > 4 else 24

    if action == "help_question":
        # Press '?' in empty composer
        display = run_overlay_session(target_dir, ["?"], cols, rows)
        for i, l in enumerate(display): print(f"[{i:02d}] {l}")
    elif action == "help_escape":
        # Press '?' then Esc
        display = run_overlay_session(target_dir, ["?", 0.2, "\x1b", 0.3], cols, rows)
        for i, l in enumerate(display): print(f"[{i:02d}] {l}")
    elif action == "help_command":
        # Execute /help command
        display = run_overlay_session(target_dir, ["/help\n", 0.3], cols, rows)
        for i, l in enumerate(display): print(f"[{i:02d}] {l}")
    elif action == "model_picker_alt_p":
        # Press alt+p (Escape + 'p')
        display = run_overlay_session(target_dir, ["\x1bp", 0.3], cols, rows)
        for i, l in enumerate(display): print(f"[{i:02d}] {l}")
    elif action == "model_picker_slash":
        # Execute /model
        display = run_overlay_session(target_dir, ["/model\n", 0.3], cols, rows)
        for i, l in enumerate(display): print(f"[{i:02d}] {l}")
    elif action == "model_picker_nav":
        # Press alt+p, Arrow Down, Enter
        display = run_overlay_session(target_dir, ["\x1bp", 0.2, "\x1b[B", 0.2, "\n", 0.3], cols, rows)
        for i, l in enumerate(display): print(f"[{i:02d}] {l}")
    elif action == "model_picker_escape":
        # Press alt+p then Esc
        display = run_overlay_session(target_dir, ["\x1bp", 0.2, "\x1b", 0.3], cols, rows)
        for i, l in enumerate(display): print(f"[{i:02d}] {l}")
