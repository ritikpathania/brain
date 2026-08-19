#!/usr/bin/env python3
"""
Phase 8C: Comprehensive Claude Behavioral Parity Audit Harness
Executes side-by-side behavioral test scenarios across Reference Claude v2.1.233
and Brain Shell under identical sandboxed fixture environments.
"""

import os
import sys
import pty
import time
import json
import fcntl
import termios
import struct
import select
import shutil
import tempfile
import re

CLAUDE_BIN = "/Users/ritikpathania/.local/bin/claude"
BUN_BIN = "/Users/ritikpathania/.bun/bin/bun"
SHELL_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"

def clean_ansi(text: str) -> str:
    """Removes ANSI escape sequences from text."""
    ansi_escape = re.compile(r'\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])')
    return ansi_escape.sub('', text)

class ControlledPtySession:
    def __init__(self, target_type: str, cwd: str, home_dir: str, cols: int = 80, rows: int = 24):
        self.target_type = target_type
        self.cwd = cwd
        self.home_dir = home_dir
        self.cols = cols
        self.rows = rows
        self.master_fd = None
        self.pid = None

    def start(self):
        master_fd, slave_fd = pty.openpty()
        winsize = struct.pack("HHHH", self.rows, self.cols, 0, 0)
        fcntl.ioctl(master_fd, termios.TIOCSWINSZ, winsize)

        env = dict(os.environ)
        env["HOME"] = self.home_dir
        env["TERM"] = "xterm-256color"
        env["COLUMNS"] = str(self.cols)
        env["LINES"] = str(self.rows)
        env["USER"] = "testuser"
        env["CLAUDE_TEST_MODE"] = "1"
        env["BRAIN_TEST_MODE"] = "1"
        env["NODE_ENV"] = "test"

        pid = os.fork()
        if pid == 0:
            os.close(master_fd)
            os.setsid()
            os.dup2(slave_fd, 0)
            os.dup2(slave_fd, 1)
            os.dup2(slave_fd, 2)
            os.close(slave_fd)
            os.chdir(self.cwd)

            if self.target_type == "claude":
                os.execvpe(CLAUDE_BIN, [CLAUDE_BIN], env)
            else:
                os.execvpe(BUN_BIN, [BUN_BIN, "run", "--preload", "./src/preload.ts", "src/main.tsx"], env)
        else:
            os.close(slave_fd)
            self.master_fd = master_fd
            self.pid = pid

    def write(self, data: bytes):
        if self.master_fd is not None:
            os.write(self.master_fd, data)

    def read_buffer(self, timeout: float = 1.0) -> str:
        out = b""
        start = time.time()
        while time.time() - start < timeout:
            r, _, _ = select.select([self.master_fd], [], [], 0.2)
            if self.master_fd in r:
                try:
                    chunk = os.read(self.master_fd, 4096)
                    if not chunk:
                        break
                    out += chunk
                except OSError:
                    break
            else:
                if out:
                    break
        return out.decode("utf-8", "replace")

    def close(self):
        if self.pid is not None:
            try:
                os.kill(self.pid, 9)
                os.waitpid(self.pid, 0)
            except OSError:
                pass
        if self.master_fd is not None:
            try:
                os.close(self.master_fd)
            except OSError:
                pass

def run_behavioral_scenario(name: str, category: str, input_steps: list, timeout: float = 2.0) -> dict:
    """Runs a behavioral scenario across both Claude and Brain in identical fixture environments."""
    temp_root = tempfile.mkdtemp(prefix="brain_phase8c_")
    home_claude = os.path.join(temp_root, "home_claude")
    home_brain = os.path.join(temp_root, "home_brain")
    fixture_cwd = os.path.join(temp_root, "workspace")
    os.makedirs(home_claude, exist_ok=True)
    os.makedirs(home_brain, exist_ok=True)
    os.makedirs(fixture_cwd, exist_ok=True)

    # Initialize pristine settings
    settings = {"hasCompletedProjectOnboarding": True, "theme": "dark", "hasTrustDialogAccepted": True}
    with open(os.path.join(home_claude, ".claude.json"), "w") as f:
        json.dump(settings, f)
    with open(os.path.join(home_brain, ".claude.json"), "w") as f:
        json.dump(settings, f)

    claude_trace = []
    brain_trace = []

    # 1. Run on Reference Claude
    try:
        session_c = ControlledPtySession("claude", fixture_cwd, home_claude)
        session_c.start()
        time.sleep(1.0)
        initial_c = clean_ansi(session_c.read_buffer(0.8))
        claude_trace.append({"step": "initial", "output": initial_c})

        for step in input_steps:
            if isinstance(step, str):
                session_c.write(step.encode("utf-8"))
            elif isinstance(step, bytes):
                session_c.write(step)
            time.sleep(0.5)
            step_out = clean_ansi(session_c.read_buffer(0.8))
            claude_trace.append({"step": f"input_{step}", "output": step_out})
        session_c.close()
    except Exception as e:
        claude_trace.append({"step": "error", "error": str(e)})

    # 2. Run on Brain Shell
    try:
        session_b = ControlledPtySession("brain", SHELL_DIR, home_brain)
        session_b.start()
        time.sleep(1.0)
        initial_b = clean_ansi(session_b.read_buffer(0.8))
        brain_trace.append({"step": "initial", "output": initial_b})

        for step in input_steps:
            if isinstance(step, str):
                session_b.write(step.encode("utf-8"))
            elif isinstance(step, bytes):
                session_b.write(step)
            time.sleep(0.5)
            step_out = clean_ansi(session_b.read_buffer(0.8))
            brain_trace.append({"step": f"input_{step}", "output": step_out})
        session_b.close()
    except Exception as e:
        brain_trace.append({"step": "error", "error": str(e)})

    shutil.rmtree(temp_root, ignore_errors=True)

    return {
        "scenario": name,
        "category": category,
        "claude_trace": claude_trace,
        "brain_trace": brain_trace,
    }

def main():
    scenarios = [
        # 1. Commands
        {"name": "Command: /theme", "category": "commands", "steps": ["/theme\n", "\x1b[B", "\n"]},
        {"name": "Command: /model", "category": "commands", "steps": ["/model\n", "\x1b"]},
        {"name": "Command: /config", "category": "commands", "steps": ["/config\n", "\x1b"]},
        {"name": "Command: /status", "category": "commands", "steps": ["/status\n"]},
        {"name": "Command: /doctor", "category": "commands", "steps": ["/doctor\n"]},
        {"name": "Command: /help", "category": "commands", "steps": ["/help\n"]},
        {"name": "Command: /clear", "category": "commands", "steps": ["hello world\n", "/clear\n"]},
        {"name": "Command: /compact", "category": "commands", "steps": ["/compact\n"]},
        {"name": "Command: /resume", "category": "commands", "steps": ["/resume\n", "\x1b"]},
        {"name": "Command: /init", "category": "commands", "steps": ["/init\n"]},
        {"name": "Command: /add-dir", "category": "commands", "steps": ["/add-dir\n"]},
        {"name": "Command: /agents", "category": "commands", "steps": ["/agents\n"]},
        {"name": "Command: /branch", "category": "commands", "steps": ["/branch\n"]},
        {"name": "Command: /btw", "category": "commands", "steps": ["/btw what is this?\n"]},
        
        # 2. Keyboard Shortcuts
        {"name": "Shortcut: ? Help Overlay", "category": "keyboard", "steps": ["?", "\x1b"]},
        {"name": "Shortcut: Alt+P Model Picker", "category": "keyboard", "steps": ["\x1bp", "\x1b"]},
        {"name": "Shortcut: Shift+Tab Mode Cycle", "category": "keyboard", "steps": ["\x1b[Z", "\x1b[Z"]},
        {"name": "Shortcut: Escape Dismiss", "category": "keyboard", "steps": ["/", "\x1b"]},
        {"name": "Shortcut: Ctrl+C Interrupt", "category": "keyboard", "steps": ["typing some input", "\x03"]},

        # 3. Composer & Input Modes
        {"name": "Composer: Multiline Input", "category": "composer", "steps": ["line 1\\\n", "line 2\n"]},
        {"name": "Composer: Slash Completion Tab", "category": "composer", "steps": ["/doc", "\t", "\x1b"]},
        {"name": "Composer: @ File Mention", "category": "composer", "steps": ["@", "\x1b"]},
        {"name": "Composer: ! Shell Mode", "category": "composer", "steps": ["!", "ls", "\x1b"]},
        {"name": "Composer: & Background Mode", "category": "composer", "steps": ["&", "test task", "\x1b"]},
        {"name": "Composer: History Navigation", "category": "composer", "steps": ["command_one\n", "\x1b[A", "\x1b"]},
    ]

    results = []
    print(f"[Phase 8C] Running {len(scenarios)} Behavioral Parity Scenarios...")
    for idx, sc in enumerate(scenarios):
        print(f"  [{idx+1}/{len(scenarios)}] Executing scenario: {sc['name']} ({sc['category']})...")
        res = run_behavioral_scenario(sc["name"], sc["category"], sc["steps"])
        results.append(res)

    out_path = os.path.join(SHELL_DIR, "src/test/phase8c_behavioral_results.json")
    with open(out_path, "w") as f:
        json.dump(results, f, indent=2)

    print(f"[Phase 8C] Complete. Audit results written to {out_path}")

if __name__ == "__main__":
    main()
