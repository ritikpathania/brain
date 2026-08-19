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
import shutil
import json

def set_terminal_size(fd, cols, rows):
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)

def run_test(cols, rows, target_dir, initial_input=""):
    master_fd, slave_fd = pty.openpty()
    set_terminal_size(master_fd, cols, rows)

    shared_home = tempfile.mkdtemp(prefix="parity_mvp_")
    try:
        claude_dir = os.path.join(shared_home, ".claude")
        os.makedirs(claude_dir, exist_ok=True)
        with open(os.path.join(claude_dir, "settings.json"), "w") as f:
            json.dump({
                "model": "claude-sonnet-4-6",
                "promptSuggestionEnabled": False,
                "permissions": {"defaultMode": "default"}
            }, f)
        try:
            import subprocess
            v_out = subprocess.check_output(["/Users/ritikpathania/.local/bin/claude", "--version"], text=True)
            claude_ver = v_out.strip().split()[0]
        except Exception:
            claude_ver = "2.1.235"

        with open(os.path.join(shared_home, ".claude.json"), "w") as f:
            json.dump({
                "hasCompletedOnboarding": True,
                "projects": {target_dir: {"hasTrustDialogAccepted": True}},
                "shownTips": ["opus_1m_tip"],
                "opus1mMergeNoticeSeenCount": 10,
                "lastReleaseNotesSeen": claude_ver,
                "lastOnboardingVersion": claude_ver
            }, f)

        env = dict(os.environ)
        env.pop("ANTHROPIC_API_KEY", None)
        env.pop("ANTHROPIC_CUSTOM_API_KEY", None)
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
        env["CLAUDE_VERSION"] = claude_ver

        preload_path = os.path.join(target_dir, "src", "preload.ts")
        main_path = os.path.join(target_dir, "src", "main.tsx")

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

        while time.time() - start_time < 15.0:
            r, _, _ = select.select([master_fd], [], [], 0.05)
            if master_fd in r:
                try:
                    chunk = os.read(master_fd, 4096)
                    if not chunk: break
                    raw.extend(chunk)
                    stream.feed(decoder.decode(chunk))
                    last_data = time.time()
                    if not sent_input and initial_input and "❯" in "\n".join(screen.display):
                        time.sleep(0.05)
                        os.write(master_fd, initial_input.encode("utf-8"))
                        sent_input = True
                except OSError: break
            
            has_prompt = any(line.strip().startswith("❯") for line in screen.display)
            has_footer = bool(initial_input) or ("? for shortcuts" in "\n".join(screen.display))
            has_input = (not initial_input) or (sent_input and initial_input in "\n".join(screen.display))
            if has_prompt and has_footer and has_input and time.time() - start_time > 0.8:
                break

        try:
            os.kill(pid, 9)
            os.waitpid(pid, os.WNOHANG)
            os.close(master_fd)
        except: pass

        # 1. Header Card in top rows
        expected_ver = os.environ.get("CLAUDE_VERSION", "2.1.235")
        assert any(f"Claude Code v{expected_ver}" in screen.display[r] or "Claude Code v" in screen.display[r] for r in range(min(4, rows))), f"Header missing in top rows: {screen.display[:4]}"
        # 2. Bottom-anchored prompt input
        assert any(screen.display[r].strip().startswith("❯") for r in [rows - 3, rows - 4, rows - 2]), f"Prompt missing around bottom rows: {screen.display[rows-5:]}"
        # 3. Top border at row (rows - 4)
        assert "───" in screen.display[rows - 4], f"Top border missing at row {rows-4}"
        # 4. Bottom border at row (rows - 2)
        assert "───" in screen.display[rows - 2], f"Bottom border missing at row {rows-2}"
        
        # 5. Footer behavior: empty composer shows shortcuts hint; active composer suppresses hint
        if not initial_input:
            assert "? for shortcuts" in screen.display[rows - 1], f"Footer missing at row {rows-1}"
        else:
            assert initial_input in screen.display[rows - 3], f"Input '{initial_input}' missing in {screen.display[rows-3]}"

        # 6. Flexible region between row 14 and (rows - 5) is clean empty space
        for r in range(14, rows - 5):
            assert screen.display[r].strip() == "", f"Flexible row {r} is not empty: {repr(screen.display[r])}"

        grid_text = "\n".join(screen.display)
        grid_hash = hashlib.sha256(grid_text.encode("utf-8")).hexdigest()
        state_label = "active_composer" if initial_input else "empty_composer"
        print(f"VERIFIED {cols}x{rows} {state_label}: {cols*rows} cells, SHA-256: {grid_hash[:16]}")
        return True
    finally:
        for _ in range(5):
            if os.path.exists(shared_home):
                shutil.rmtree(shared_home, ignore_errors=True)
                if not os.path.exists(shared_home):
                    break
                time.sleep(0.05)

if __name__ == "__main__":
    cols = int(sys.argv[1])
    rows = int(sys.argv[2])
    target_dir = sys.argv[3]
    initial_input = sys.argv[4] if len(sys.argv) > 4 else ""
    success = run_test(cols, rows, target_dir, initial_input)
    sys.exit(0 if success else 1)
