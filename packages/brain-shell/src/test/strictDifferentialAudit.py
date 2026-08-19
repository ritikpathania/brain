#!/usr/bin/env python3
"""
Strict Exhaustive Differential Audit Suite:
Original Claude Source (Developer/src) vs Brain-Hosted Claude Shell (vendor/claude)

Compares cell-by-cell terminal state across:
- Viewports: 70x37, 80x24, 100x30, 120x40, 182x53
- Themes: dark, light
- Interactive grammar: landing, typing, multiline, backspace, delete, cursor navigation, slash modal, escape, submission, SIGWINCH resize.
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
import json
from typing import Dict, List, Tuple, Any

from terminalEmulator import VirtualTerminal, Cell

BRAIN_SHELL_DIR = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PRELOAD_PATH = os.path.join(BRAIN_SHELL_DIR, "src", "preload.ts")
MAIN_PATH = os.path.join(BRAIN_SHELL_DIR, "src", "main.tsx")
REF_PATH = os.path.join(BRAIN_SHELL_DIR, "src", "test", "referenceRunner.tsx")
VENDOR_CLAUDE_DIR = os.path.join(BRAIN_SHELL_DIR, "vendor", "claude")
DEV_SRC_DIR = "/Users/ritikpathania/Developer/src"

def set_terminal_size(fd, cols, rows):
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)

class PtyProcess:
    def __init__(self, name: str, script_path: str, source_root: str, cols: int, rows: int, theme: str = "dark"):
        self.name = name
        self.script_path = script_path
        self.source_root = source_root
        self.cols = cols
        self.rows = rows
        self.theme = theme
        self.master_fd = None
        self.slave_fd = None
        self.pid = None
        self.raw_output = ""
        self.vt = VirtualTerminal(cols, rows)

    def start(self):
        self.master_fd, self.slave_fd = pty.openpty()
        set_terminal_size(self.master_fd, self.cols, self.rows)

        env = dict(os.environ)
        env["TERM"] = "xterm-256color"
        env["COLORTERM"] = "truecolor"
        env["COLUMNS"] = str(self.cols)
        env["LINES"] = str(self.rows)
        env["FORCE_COLOR"] = "3"
        env["CLAUDE_THEME"] = self.theme
        env["NODE_ENV"] = "production"
        env["DISABLE_AUTOUPDATER"] = "1"
        env["CLAUDE_SOURCE_ROOT"] = self.source_root

        self.pid = os.fork()
        if self.pid == 0:
            os.close(self.master_fd)
            os.setsid()
            os.dup2(self.slave_fd, 0)
            os.dup2(self.slave_fd, 1)
            os.dup2(self.slave_fd, 2)
            os.close(self.slave_fd)
            import shutil
            bun_bin = shutil.which("bun") or "/Users/ritikpathania/.bun/bin/bun"
            env["PATH"] = os.environ.get("PATH", "") + ":/Users/ritikpathania/.bun/bin:/usr/local/bin:/usr/bin:/bin"
            os.chdir(BRAIN_SHELL_DIR)
            os.execvpe(
                bun_bin,
                [bun_bin, "run", "--preload", PRELOAD_PATH, self.script_path, "--bare", "--settings", '{"promptSuggestionEnabled":false}'],
                env
            )


        else:
            os.close(self.slave_fd)

    def write(self, data: str):
        if isinstance(data, str):
            data = data.encode("utf-8")
        os.write(self.master_fd, data)

    def resize(self, cols: int, rows: int):
        self.cols = cols
        self.rows = rows
        set_terminal_size(self.master_fd, cols, rows)

    def close(self):
        if self.master_fd:
            try:
                os.close(self.master_fd)
            except OSError:
                pass
            self.master_fd = None
        if self.pid:
            try:
                os.kill(self.pid, 9)
                os.waitpid(self.pid, 0)
            except OSError:
                pass
            self.pid = None


def read_both_settled(p1: PtyProcess, p2: PtyProcess, idle_timeout: float = 0.3, max_timeout: float = 3.5):
    """
    Reads from both PTY master descriptors simultaneously in a single select() loop
    until both processes have stopped producing output for idle_timeout seconds.
    """
    start = time.time()
    last1 = time.time()
    last2 = time.time()
    while time.time() - start < max_timeout:
        r, _, _ = select.select([p1.master_fd, p2.master_fd], [], [], 0.04)
        if p1.master_fd in r:
            try:
                d = os.read(p1.master_fd, 4096)
                if d:
                    t = d.decode("utf-8", errors="replace")
                    p1.raw_output += t
                    p1.vt.feed(t)
                    last1 = time.time()
            except OSError:
                pass
        if p2.master_fd in r:
            try:
                d = os.read(p2.master_fd, 4096)
                if d:
                    t = d.decode("utf-8", errors="replace")
                    p2.raw_output += t
                    p2.vt.feed(t)
                    last2 = time.time()
            except OSError:
                pass

        if p1.raw_output and p2.raw_output:
            if (time.time() - last1 > idle_timeout) and (time.time() - last2 > idle_timeout):
                break


def compare_grids(vt_ref: VirtualTerminal, vt_cand: VirtualTerminal) -> Tuple[int, int, List[Dict[str, Any]]]:
    total = vt_ref.rows * vt_ref.cols
    diff_count = 0
    diff_details = []

    for r in range(vt_ref.rows):
        for c in range(vt_ref.cols):
            cell_ref = vt_ref.grid[r][c]
            cell_cand = vt_cand.grid[r][c]

            if not cell_ref.matches(cell_cand):
                diff_count += 1
                if len(diff_details) < 10:
                    diff_details.append({
                        "row": r,
                        "col": c,
                        "ref": cell_ref.to_dict(),
                        "cand": cell_cand.to_dict()
                    })

    return total, diff_count, diff_details


def run_audit():
    print("=========================================================================")
    print("      STRICT DIFFERENTIAL AUDIT: CLAUDE SOURCE VS HOSTED SHELL           ")
    print("=========================================================================")

    # Viewports requested: 70x37, 80x24, 100x30, 120x40, 182x53
    viewports = [
        (70, 37, "Compact 70x37"),
        (80, 24, "Canonical 80x24"),
        (100, 30, "Medium 100x30"),
        (120, 40, "Fullscreen 120x40"),
        (182, 53, "Ultrawide 182x53"),
    ]

    themes = ["dark", "light"]

    audit_records = []
    grand_total_cells = 0
    grand_total_diffs = 0

    for cols, rows, vp_label in viewports:
        for theme in themes:
            scenario_name = f"{vp_label} [{theme}]"
            print(f"\n>>> Running Audit Matrix Scenario: {scenario_name} ({cols}x{rows})")

            p_ref = PtyProcess("Reference(DevSrc)", REF_PATH, DEV_SRC_DIR, cols, rows, theme=theme)
            p_cand = PtyProcess("HostedShell(Vendor)", MAIN_PATH, VENDOR_CLAUDE_DIR, cols, rows, theme=theme)

            try:
                p_ref.start()
                p_cand.start()

                # 1. Initial Landing
                print("  [Step 1/9] Initial Landing State... ", end="", flush=True)
                read_both_settled(p_ref, p_cand, idle_timeout=0.35, max_timeout=3.5)
                tot, diffs, details = compare_grids(p_ref.vt, p_cand.vt)
                grand_total_cells += tot
                grand_total_diffs += diffs
                status_str = "MATCH (0 diff)" if diffs == 0 else f"DIFF: {diffs}"
                print(f"[{status_str}] (Cells: {tot})")
                audit_records.append((f"{scenario_name} - Landing", tot, diffs, details))

                # 2. Character Typing
                print("  [Step 2/9] Typing 'Hello Claude Differential Audit'... ", end="", flush=True)
                seq_type = "Hello Claude Differential Audit"
                p_ref.write(seq_type)
                p_cand.write(seq_type)
                read_both_settled(p_ref, p_cand, idle_timeout=0.25, max_timeout=2.0)
                tot, diffs, details = compare_grids(p_ref.vt, p_cand.vt)
                grand_total_cells += tot
                grand_total_diffs += diffs
                status_str = "MATCH (0 diff)" if diffs == 0 else f"DIFF: {diffs}"
                print(f"[{status_str}] (Cells: {tot})")
                audit_records.append((f"{scenario_name} - Typing", tot, diffs, details))

                # 3. Multiline Input
                print("  [Step 3/9] Multiline Text (Shift+Enter)... ", end="", flush=True)
                seq_multi = "\nLine 2 multiline content"
                p_ref.write(seq_multi)
                p_cand.write(seq_multi)
                read_both_settled(p_ref, p_cand, idle_timeout=0.25, max_timeout=2.0)
                tot, diffs, details = compare_grids(p_ref.vt, p_cand.vt)
                grand_total_cells += tot
                grand_total_diffs += diffs
                status_str = "MATCH (0 diff)" if diffs == 0 else f"DIFF: {diffs}"
                print(f"[{status_str}] (Cells: {tot})")
                audit_records.append((f"{scenario_name} - Multiline", tot, diffs, details))

                # 4. Backspace & Delete Editing
                print("  [Step 4/9] Backspace (8x) and Delete (4x)... ", end="", flush=True)
                seq_bs = "\x7f" * 8 + "\x1b[3~" * 4
                p_ref.write(seq_bs)
                p_cand.write(seq_bs)
                read_both_settled(p_ref, p_cand, idle_timeout=0.25, max_timeout=2.0)
                tot, diffs, details = compare_grids(p_ref.vt, p_cand.vt)
                grand_total_cells += tot
                grand_total_diffs += diffs
                status_str = "MATCH (0 diff)" if diffs == 0 else f"DIFF: {diffs}"
                print(f"[{status_str}] (Cells: {tot})")
                audit_records.append((f"{scenario_name} - Backspace/Delete", tot, diffs, details))

                # 5. Cursor Movement (Home, Left, Right, End)
                print("  [Step 5/9] Cursor Navigation (Home, End, Arrows)... ", end="", flush=True)
                seq_nav = "\x1b[H\x1b[C\x1b[C\x1b[C\x1b[D\x1b[F"
                p_ref.write(seq_nav)
                p_cand.write(seq_nav)
                read_both_settled(p_ref, p_cand, idle_timeout=0.25, max_timeout=2.0)
                tot, diffs, details = compare_grids(p_ref.vt, p_cand.vt)
                grand_total_cells += tot
                grand_total_diffs += diffs
                status_str = "MATCH (0 diff)" if diffs == 0 else f"DIFF: {diffs}"
                print(f"[{status_str}] (Cells: {tot})")
                audit_records.append((f"{scenario_name} - Cursor Movement", tot, diffs, details))

                # 6. Slash Command Modal
                print("  [Step 6/9] Slash Command Modal ('/')... ", end="", flush=True)
                p_ref.write("\x15/") # Ctrl+U clear then /
                p_cand.write("\x15/")
                read_both_settled(p_ref, p_cand, idle_timeout=0.25, max_timeout=2.0)
                tot, diffs, details = compare_grids(p_ref.vt, p_cand.vt)
                grand_total_cells += tot
                grand_total_diffs += diffs
                status_str = "MATCH (0 diff)" if diffs == 0 else f"DIFF: {diffs}"
                print(f"[{status_str}] (Cells: {tot})")
                audit_records.append((f"{scenario_name} - Slash Modal", tot, diffs, details))

                # 7. Escape Modal Dismissal
                print("  [Step 7/9] Escape Modal Dismissal... ", end="", flush=True)
                p_ref.write("\x1b")
                p_cand.write("\x1b")
                read_both_settled(p_ref, p_cand, idle_timeout=0.25, max_timeout=2.0)
                tot, diffs, details = compare_grids(p_ref.vt, p_cand.vt)
                grand_total_cells += tot
                grand_total_diffs += diffs
                status_str = "MATCH (0 diff)" if diffs == 0 else f"DIFF: {diffs}"
                print(f"[{status_str}] (Cells: {tot})")
                audit_records.append((f"{scenario_name} - Escape", tot, diffs, details))

                # 8. Prompt Submission
                print("  [Step 8/9] Prompt Submission (Enter)... ", end="", flush=True)
                p_ref.write("test submission\r")
                p_cand.write("test submission\r")
                read_both_settled(p_ref, p_cand, idle_timeout=0.35, max_timeout=3.0)
                tot, diffs, details = compare_grids(p_ref.vt, p_cand.vt)
                grand_total_cells += tot
                grand_total_diffs += diffs
                status_str = "MATCH (0 diff)" if diffs == 0 else f"DIFF: {diffs}"
                print(f"[{status_str}] (Cells: {tot})")
                audit_records.append((f"{scenario_name} - Submission", tot, diffs, details))

                # 9. Dynamic SIGWINCH Resize
                print("  [Step 9/9] Dynamic SIGWINCH Resize... ", end="", flush=True)
                p_ref.resize(cols + 10, rows + 5)
                p_cand.resize(cols + 10, rows + 5)
                read_both_settled(p_ref, p_cand, idle_timeout=0.25, max_timeout=2.0)
                p_ref.resize(cols, rows)
                p_cand.resize(cols, rows)
                read_both_settled(p_ref, p_cand, idle_timeout=0.25, max_timeout=2.0)
                tot, diffs, details = compare_grids(p_ref.vt, p_cand.vt)
                grand_total_cells += tot
                grand_total_diffs += diffs
                status_str = "MATCH (0 diff)" if diffs == 0 else f"DIFF: {diffs}"
                print(f"[{status_str}] (Cells: {tot})")
                audit_records.append((f"{scenario_name} - Dynamic Resize", tot, diffs, details))

            finally:
                p_ref.close()
                p_cand.close()

    print("\n=========================================================================")
    print("                    STRICT DIFFERENTIAL AUDIT SUMMARY                    ")
    print("=========================================================================")
    print(f"Total Scenarios Evaluated: {len(audit_records)}")
    print(f"Total Cells Compared:      {grand_total_cells:,}")
    print(f"Total Cell Differences:    {grand_total_diffs:,}")
    if grand_total_cells > 0:
        match_rate = ((grand_total_cells - grand_total_diffs) / grand_total_cells) * 100
        print(f"Exhaustive Match Parity:   {match_rate:.4f}%")
    print("-------------------------------------------------------------------------")
    for name, tot, diff, _ in audit_records:
        status = "[MATCH 0 DIFF]" if diff == 0 else f"[DIFF: {diff}]"
        print(f"  {status:16} | {name:50} | {tot:,} cells")
    print("=========================================================================")

    return grand_total_diffs == 0

if __name__ == "__main__":
    success = run_audit()
    sys.exit(0 if success else 1)
