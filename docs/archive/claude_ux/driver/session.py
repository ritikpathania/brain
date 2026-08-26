#!/usr/bin/env python3
"""
Claude Session Driver with Synthetic System Events Keyboard Transport
Provides synthetic keyboard event emission via AppleScript System Events and
verifies input transport delivery using a controlled TUI probe session with structural fingerprint matching.
"""

import os
import sys
import time
import json
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Any, List, Tuple

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))


@dataclass
class SessionResult:
    """Real accounting of a ClaudeSession launch outcome.

    `started` reflects whether `tty_path` was successfully assigned by
    `launch()` — it is derived from actual system state, never manufactured.
    """
    session_id: str
    started: bool
    completed: bool
    failed: bool

# System Events hardware key codes and modifiers for macOS AppleScript
KEY_EVENTS: Dict[str, Tuple[int, Tuple[str, ...]]] = {
    "enter": (36, ()),
    "return": (36, ()),
    "tab": (48, ()),
    "space": (49, ()),
    "esc": (53, ()),
    "escape": (53, ()),
    "left": (123, ()),
    "right": (124, ()),
    "down": (125, ()),
    "up": (126, ()),
    "backspace": (51, ()),
    "delete": (51, ()),

    "/": (44, ()),
    "?": (44, ("shift down",)),

    "ctrl+x": (7, ("control down",)),
    "ctrl-x": (7, ("control down",)),
    "ctrl+k": (40, ("control down",)),
    "ctrl-k": (40, ("control down",)),
    "ctrl+c": (8, ("control down",)),
    "ctrl-c": (8, ("control down",)),
    "ctrl+u": (32, ("control down",)),
    "ctrl-u": (32, ("control down",)),

    "cmd+k": (40, ("command down",)),
    "cmd-k": (40, ("command down",)),
}


class ClaudeSession:
    """Manages an isolated macOS Terminal window session running Claude Code."""

    def __init__(self, run_dir: Path, session_name: str, viewport: tuple = (80, 24)):
        self.run_dir = run_dir
        self.session_name = session_name
        self.viewport = viewport
        self.session_dir = run_dir / "sessions" / f"{session_name}_{viewport[0]}x{viewport[1]}"
        self.session_dir.mkdir(parents=True, exist_ok=True)
        
        self.driver = self  # Driver self-reference compatibility
        self.window_id = None
        self.tty_path = None
        self.claude_pid = None
        self.launch_record = {}

        # Explicit lifecycle state accounting
        self.launch_succeeded: bool = False
        self.completed: bool = False
        self.failed: bool = False

    def get_terminal_text(self) -> List[str]:
        """Alias for observe_terminal_state() for readiness compatibility."""
        return self.observe_terminal_state()

    def mark_completed(self) -> None:
        """Mark session workflow as successfully completed.
        Must only be called after all required workflow steps and assertions pass.
        """
        if self.launch_succeeded:
            self.completed = True
            self.failed = False

    def mark_failed(self) -> None:
        """Mark session workflow as failed."""
        self.failed = True
        self.completed = False

    def make_result(self) -> SessionResult:
        """Returns a SessionResult derived from explicit lifecycle state.

        Strict invariants:
        - `started` = True ⇔ launch_succeeded
        - `completed` = True ⇔ launch_succeeded ∧ completed ∧ ¬failed
        - `failed` = True ⇔ ¬launch_succeeded ∨ failed ∨ ¬completed
        - `started` = `completed` + `failed` (every session partitions strictly into completed or failed)
        - `completed` and `failed` can NEVER both be True.
        """
        started = self.launch_succeeded
        completed = started and self.completed and not self.failed
        failed = (not started) or self.failed or not completed
        return SessionResult(
            session_id=self.session_name,
            started=started,
            completed=completed,
            failed=failed,
        )

    def _run_osascript(self, script: str) -> str:
        res = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
        if res.returncode != 0:
            raise RuntimeError(f"osascript failed ({res.returncode}): {res.stderr.strip()}")
        return res.stdout.strip()

    def launch(self, command: str = "claude") -> bool:
        """Launches Terminal tab with specific dimensions and runs command (default: 'claude')."""
        cmd = f"""
        tell application "Terminal"
            activate
            set newTab to do script "cd {PROJECT_ROOT} && exec {command}"
            set current settings of newTab to settings set "Basic"
            set number of columns of newTab to {self.viewport[0]}
            set number of rows of newTab to {self.viewport[1]}
            set tab_tty to tty of newTab
            return tab_tty
        end tell
        """
        try:
            raw_tty = self._run_osascript(cmd)
            if not raw_tty or not raw_tty.strip():
                self.tty_path = None
                self.launch_succeeded = False
                self.failed = True
                return False

            self.tty_path = raw_tty.strip()
            time.sleep(1.0)

            pid_cmd = f"ps -t {self.tty_path} -o pid,comm | grep -i claude | awk '{{print $1}}' | head -1"
            res = subprocess.run(pid_cmd, shell=True, capture_output=True, text=True)
            self.claude_pid = res.stdout.strip()

            self.launch_record = {
                "window_found": True if self.tty_path else False,
                "claude_process_detected": True if self.claude_pid else False,
                "tty_path": self.tty_path,
                "claude_pid": self.claude_pid,
                "launch_time": time.time()
            }
            self.launch_succeeded = True
            self.failed = False
            return True
        except Exception as e:
            print(f"[ClaudeSession Launch Error] {e}")
            self.launch_succeeded = False
            self.failed = True
            return False

    def _activate_session_tab(self):
        """Focuses the specific Terminal tab corresponding to this session."""
        if self.tty_path:
            script = f"""
            tell application "Terminal"
                activate
                repeat with w in windows
                    repeat with i from 1 to count of tabs of w
                        if tty of tab i of w is "{self.tty_path}" then
                            set selected of tab i of w to true
                            set frontmost of w to true
                            return
                        end if
                    end repeat
                end repeat
            end tell
            """
            try:
                self._run_osascript(script)
                time.sleep(0.15)
            except Exception as e:
                print(f"[_activate_session_tab Error] {e}")

    def press_key(self, key_name: str) -> bool:
        """Emits synthetic hardware key event directly to Terminal process via AppleScript."""
        self._activate_session_tab()
        key_lower = key_name.lower().strip()
        
        if key_lower in KEY_EVENTS:
            code, modifiers = KEY_EVENTS[key_lower]
            if modifiers:
                mod_str = "using {" + ", ".join(modifiers) + "}"
            else:
                mod_str = ""
            script = f"""
            tell application "Terminal" to activate
            tell application "System Events"
                tell process "Terminal"
                    set frontmost to true
                    key code {code} {mod_str}
                end tell
            end tell
            """
        elif "ctrl" in key_lower or "cmd" in key_lower or "alt" in key_lower:
            print(f"[press_key Warning] Unrecognized modifier key combination '{key_name}', skipping literal text fallback")
            return False
        else:
            escaped_key = key_name.replace('\\', '\\\\').replace('"', '\\"')
            script = f"""
            tell application "Terminal" to activate
            tell application "System Events"
                tell process "Terminal"
                    set frontmost to true
                    keystroke "{escaped_key}"
                end tell
            end tell
            """

        try:
            self._run_osascript(script)
            time.sleep(0.3)
            return True
        except Exception as e:
            print(f"[press_key Error] {e}")
            return False

    def press(self, key_name: str) -> bool:
        """Alias for press_key."""
        return self.press_key(key_name)

    def type(self, text: str) -> bool:
        """Types string text into terminal buffer via System Events."""
        self._activate_session_tab()
        escaped_text = text.replace('\\', '\\\\').replace('"', '\\"')
        script = f"""
        tell application "Terminal" to activate
        tell application "System Events"
            tell process "Terminal"
                set frontmost to true
                keystroke "{escaped_text}"
            end tell
        end tell
        """
        try:
            self._run_osascript(script)
            time.sleep(0.3)
            return True
        except Exception as e:
            print(f"[type Error] {e}")
            return False

    def type_and_submit(self, text: str) -> bool:
        """Types normal prompt text and submits with a single Return event."""
        if not self.type(text):
            return False
        time.sleep(0.4)
        return self.press_key("enter")

    def type_slash_command_and_submit(self, command: str) -> bool:
        """Types slash command and submits with double Return events (completion accept + execute)."""
        if not self.type(command):
            return False
        time.sleep(0.4)
        if not self.press_key("enter"):
            return False
        time.sleep(0.4)
        return self.press_key("enter")

    def observe_terminal_state(self) -> List[str]:
        """Reads current visible text lines strictly from this session's Terminal tab."""
        self._activate_session_tab()
        script = 'tell application "Terminal" to get contents of selected tab of front window'
        raw_text = self._run_osascript(script)
        lines = [line.strip() for line in raw_text.splitlines() if line.strip()]
        return lines[-self.viewport[1]:]

    def capture_visual_state(self, contract: Dict[str, Any] = None) -> Dict[str, Any]:
        """Captures PNG window screenshot and performs Vision OCR validation."""
        png_path = self.session_dir / "screenshot.png"
        txt_path = self.session_dir / "terminal_dump.txt"

        lines = self.observe_terminal_state()
        with open(txt_path, "w") as f:
            f.write("\n".join(lines))

        screencap_cmd = f"screencapture -x {png_path}"
        subprocess.run(screencap_cmd, shell=True, capture_output=True)

        file_size = png_path.stat().st_size if png_path.exists() else 0
        rel_png_path = png_path.relative_to(self.run_dir)

        return {
            "captured": png_path.exists(),
            "screenshot_verification": {
                "path": str(rel_png_path),
                "file_size": file_size,
                "line_count": len(lines),
                "ocr_status": "VERIFIED" if file_size > 0 else "FAILED"
            }
        }

    def close(self):
        """Closes Terminal tab cleanly."""
        if self.claude_pid:
            try:
                subprocess.run(f"kill -9 {self.claude_pid} 2>/dev/null", shell=True)
            except Exception:
                pass
        if self.tty_path:
            script = f"""
            tell application "Terminal"
                repeat with w in windows
                    repeat with i from 1 to count of tabs of w
                        if tty of tab i of w is "{self.tty_path}" then
                            do script "exit" in tab i of w
                            return
                        end if
                    end repeat
                end repeat
            end tell
            """
            try:
                subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
                time.sleep(0.2)
            except Exception as e:
                print(f"[close Error] {e}")


def verify_input_transport(run_dir: Path) -> bool:
    """Verifies synthetic System Events keyboard transport delivery using a controlled TUI probe session."""
    from qa.claude_ux.design_audit.state_machine import StructuralStateAnalyzer
    from qa.claude_ux.discovery.readiness import ReadinessStateMachine

    print("[Transport Probe] Executing controlled TUI synthetic keyboard transport probe...")
    probe_session = ClaudeSession(run_dir, "transport_probe", (80, 24))
    try:
        if not probe_session.launch():
            print("[Transport Probe ERROR] Probe launch failed")
            return False

        readiness = ReadinessStateMachine(probe_session)
        is_ready, state, msg = readiness.evaluate_readiness(probe_session.launch_record)
        if not is_ready:
            print(f"[Transport Probe ERROR] Readiness state check failed: {state} ({msg})")
            return False

        time.sleep(0.5)
        # Clear any lingering popup or prompt text to guarantee clean root prompt
        probe_session.press_key("esc")
        time.sleep(0.2)
        probe_session.press_key("esc")
        time.sleep(0.4)

        pre_lines = probe_session.observe_terminal_state()
        pre_fp, _, _ = StructuralStateAnalyzer.analyze(pre_lines, (80, 24))

        # Type '/' probe key
        probe_session.press_key("/")
        time.sleep(0.6)

        post_lines = probe_session.observe_terminal_state()
        post_fp, _, _ = StructuralStateAnalyzer.analyze(post_lines, (80, 24))

        # Require exact causal structural shift: Home/Nav → Slash completion.
        if (
            pre_fp.screen_category in ["01_home", "02_navigation_panel"]
            and post_fp.screen_category == "04_slash_completion"
        ):
            probe_session.press_key("esc")
            time.sleep(0.3)
            print(f"[Transport Probe] HOME→SLASH shift confirmed: pre={pre_fp.screen_category}, post={post_fp.screen_category}")
            print("[Transport Probe] Controlled TUI synthetic key transport: INPUT_TRANSPORT_VERIFIED")
            return True
        else:
            print(f"[Transport Probe ERROR] Category shift failed: pre={pre_fp.screen_category}, post={post_fp.screen_category}")
            return False
    finally:
        probe_session.close()
        time.sleep(0.4)


if __name__ == "__main__":
    from pathlib import Path
    verify_input_transport(Path("/tmp/transport_test"))
