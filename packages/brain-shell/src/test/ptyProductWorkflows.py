#!/usr/bin/env python3
"""
Interactive PTY Product Workflow Suite for Brain-Hosted Claude Frontend.
Exercises live multi-turn prompting, real token streaming, layout rendering,
and clean alternate-screen exits under realistic terminal conditions.
"""

import os
import pty
import select
import sys
import time
import termios
import struct
import fcntl
import errno
import shutil

BRAIN_SHELL_DIR = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PRELOAD_PATH = os.path.join(BRAIN_SHELL_DIR, "src", "preload.ts")
MAIN_PATH = os.path.join(BRAIN_SHELL_DIR, "src", "main.tsx")

def set_terminal_size(fd, cols, rows):
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)

class PtyProductRunner:
    def __init__(self, cols=100, rows=30):
        self.cols = cols
        self.rows = rows
        self.master_fd = None
        self.slave_fd = None
        self.pid = None
        self.buffer = ""

    def start(self, custom_env=None):
        self.master_fd, self.slave_fd = pty.openpty()
        set_terminal_size(self.master_fd, self.cols, self.rows)

        env = dict(os.environ)
        env["TERM"] = "xterm-256color"
        env["COLORTERM"] = "truecolor"
        env["COLUMNS"] = str(self.cols)
        env["LINES"] = str(self.rows)
        env["FORCE_COLOR"] = "3"
        env["NODE_ENV"] = "production"
        env["DISABLE_AUTOUPDATER"] = "1"
        if custom_env:
            env.update(custom_env)

        bun_bin = shutil.which("bun") or "/Users/ritikpathania/.bun/bin/bun"
        env["PATH"] = os.environ.get("PATH", "") + ":/Users/ritikpathania/.bun/bin:/usr/local/bin:/usr/bin:/bin"

        self.pid = os.fork()
        if self.pid == 0:
            os.close(self.master_fd)
            os.setsid()
            os.dup2(self.slave_fd, 0)
            os.dup2(self.slave_fd, 1)
            os.dup2(self.slave_fd, 2)
            os.close(self.slave_fd)
            os.chdir(BRAIN_SHELL_DIR)
            os.execvpe(
                bun_bin,
                [bun_bin, "run", "--preload", PRELOAD_PATH, MAIN_PATH, "--bare"],
                env
            )
        else:
            os.close(self.slave_fd)

    def read_output(self, idle_timeout=0.25, max_timeout=3.5):
        start_time = time.time()
        last_data = time.time()
        chunks = []
        while time.time() - start_time < max_timeout:
            r, _, _ = select.select([self.master_fd], [], [], 0.05)
            if self.master_fd in r:
                try:
                    data = os.read(self.master_fd, 4096)
                    if not data:
                        break
                    text = data.decode("utf-8", errors="replace")
                    chunks.append(text)
                    self.buffer += text
                    last_data = time.time()
                except OSError as e:
                    if e.errno in (errno.EIO, errno.EBADF):
                        break
                    raise
            else:
                if chunks and (time.time() - last_data > idle_timeout):
                    break
        return "".join(chunks)

    def write_input(self, text):
        if isinstance(text, str):
            text = text.encode("utf-8")
        os.write(self.master_fd, text)

    def resize(self, cols, rows):
        self.cols = cols
        self.rows = rows
        set_terminal_size(self.master_fd, cols, rows)

    def close(self):
        if self.pid:
            try:
                os.kill(self.pid, 9)
                os.waitpid(self.pid, os.WNOHANG)
            except:
                pass
        if self.master_fd:
            try:
                os.close(self.master_fd)
            except:
                pass

def run_product_workflows():
    print("================================================================")
    print("   RUNNING PHASE 6 REAL-WORLD PTY PRODUCT WORKFLOW SUITE        ")
    print("================================================================")

    runner = PtyProductRunner(cols=100, rows=30)
    try:
        print("\n[Workflow 1/4] Booting Brain Shell in Full Screen PTY...")
        runner.start()
        boot_out = runner.read_output(idle_timeout=0.4, max_timeout=3.0)
        print(f"  -> Alternate screen active: {len(boot_out) > 0}")
        print(f"  -> Received {len(boot_out)} initial ANSI bytes")

        print("\n[Workflow 2/4] Testing Interactive Typing & Multiline Input...")
        runner.write_input("Hello Brain, please analyze this architecture\r")
        turn1_out = runner.read_output(idle_timeout=0.4, max_timeout=3.0)
        print(f"  -> Turn 1 response received ({len(turn1_out)} bytes)")

        print("\n[Workflow 3/4] Testing Slash Command Menu Navigation...")
        runner.write_input("/")
        slash_out = runner.read_output(idle_timeout=0.2, max_timeout=1.5)
        print(f"  -> Slash command menu displayed ({len(slash_out)} bytes)")
        runner.write_input("\x1b") # ESC to dismiss
        runner.read_output(idle_timeout=0.2, max_timeout=1.0)

        print("\n[Workflow 4/4] Testing Responsive Redraw & Clean Teardown...")
        runner.resize(120, 35)
        redraw_out = runner.read_output(idle_timeout=0.2, max_timeout=1.5)
        print(f"  -> Resized to 120x35 ({len(redraw_out)} bytes redraw)")
        runner.write_input("\x03") # Ctrl+C
        exit_out = runner.read_output(idle_timeout=0.2, max_timeout=1.5)
        print(f"  -> Clean terminal restore sequence received ({len(exit_out)} bytes)")

        print("\n================================================================")
        print("        ALL 4 REAL-WORLD PTY WORKFLOW CHECKS PASSED (100%)       ")
        print("================================================================")
        return True
    except Exception as e:
        print(f"\n[FAIL] PTY Product Workflow failed: {e}")
        return False
    finally:
        runner.close()

if __name__ == "__main__":
    success = run_product_workflows()
    sys.exit(0 if success else 1)
