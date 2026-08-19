"""
PTY Session Execution Harness
"""

import os
import sys
import pty
import time
import json
import pyte
import select
import codecs
import struct
import fcntl
import termios
import tempfile
import shutil
from typing import Optional, Callable
from .terminal import CanonicalFrame, extract_canonical_frame


def set_terminal_size(fd: int, cols: int = 80, rows: int = 24):
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)


class OracleSession:
    def __init__(self, target_type: str, editor_mode: str, cols: int = 80, rows: int = 24):
        self.target_type = target_type  # 'claude' or 'brain'
        self.editor_mode = editor_mode
        self.cols = cols
        self.rows = rows
        self.home_dir = tempfile.mkdtemp(prefix=f"parity_{target_type}_{editor_mode}_")
        self.brain_shell_dir = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"
        self.repo_root = "/Users/ritikpathania/Developer/PyCharm/brain"
        self.master_fd = None
        self.slave_fd = None
        self.pid = None
        self.screen = None
        self.stream = None
        self.decoder = None
        self.stage_raw_bytes: bytearray = bytearray()

    def setup_environment(self):
        claude_dir = os.path.join(self.home_dir, ".claude")
        cache_dir = os.path.join(claude_dir, "cache")
        os.makedirs(cache_dir, exist_ok=True)
        
        with open(os.path.join(cache_dir, "changelog.md"), "w") as f:
            f.write("## 2.1.233\n- Added GitLab merge request URL support to /pr\n- Added an opt-in `forward_user_identity` approval parameter\n- Added opt-in memory cgroup support for Bash tool execution\n")

        with open(os.path.join(claude_dir, "settings.json"), "w") as f:
            json.dump({
                "model": "claude-sonnet-4-6",
                "editorMode": self.editor_mode,
                "promptSuggestionEnabled": False
            }, f, indent=2)

        # Pre-seed official marketplace to prevent redundant git clones / network requests during tests
        plugins_dir = os.path.join(claude_dir, "plugins")
        marketplaces_dir = os.path.join(plugins_dir, "marketplaces")
        official_mkt_dir = os.path.join(marketplaces_dir, "claude-plugins-official")
        os.makedirs(official_mkt_dir, exist_ok=True)
        with open(os.path.join(plugins_dir, "known_marketplaces.json"), "w") as f:
            json.dump({
                "claude-plugins-official": {
                    "source": {
                        "source": "github",
                        "repo": "anthropics/claude-plugins-official"
                    },
                    "installLocation": official_mkt_dir,
                    "lastUpdated": "2026-08-18T00:00:00.000Z"
                }
            }, f, indent=2)

        cwd = self.brain_shell_dir
        real_cwd = os.path.realpath(cwd)

        try:
            import subprocess
            v_out = subprocess.check_output(["/Users/ritikpathania/.local/bin/claude", "--version"], text=True)
            claude_ver = v_out.strip().split()[0]
        except Exception:
            claude_ver = "2.1.235"

        with open(os.path.join(self.home_dir, ".claude.json"), "w") as f:
            json.dump({
                "editorMode": self.editor_mode,
                "hasCompletedOnboarding": True,
                "hasCompletedProjectOnboarding": True,
                "shownTips": ["opus_1m_tip"],
                "opus1mMergeNoticeSeenCount": 10,
                "projectOnboardingSeenCount": 10,
                "lastReleaseNotesSeen": claude_ver,
                "lastOnboardingVersion": claude_ver,
                "officialMarketplaceAutoInstallAttempted": True,
                "officialMarketplaceAutoInstalled": True,
                "projects": {
                    cwd: {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    real_cwd: {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    self.repo_root: {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    os.path.realpath(self.repo_root): {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    self.home_dir: {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    os.path.realpath(self.home_dir): {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    "/tmp": {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    "/private/tmp": {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10}
                }
            }, f, indent=2)

    def spawn(self):
        self.setup_environment()
        
        env = dict(
            os.environ,
            HOME=self.home_dir,
            TERM="xterm-256color",
            COLUMNS=str(self.cols),
            LINES=str(self.rows),
            DISABLE_AUTOUPDATER="1",
            CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1",
            DISABLE_TELEMETRY="1"
        )
        try:
            import subprocess
            v_out = subprocess.check_output(["/Users/ritikpathania/.local/bin/claude", "--version"], text=True)
            env["CLAUDE_VERSION"] = v_out.strip().split()[0]
        except Exception:
            env["CLAUDE_VERSION"] = "2.1.235"

        preload_path = os.path.join(self.brain_shell_dir, "src", "preload.ts")
        main_path = os.path.join(self.brain_shell_dir, "src", "main.tsx")

        if self.target_type == "claude":
            cmd = ["/Users/ritikpathania/.local/bin/claude"]
        else:
            cmd = [
                "/Users/ritikpathania/.bun/bin/bun",
                "run",
                "--feature", "AUTO_THEME",
                "--preload", preload_path,
                main_path
            ]

        self.master_fd, self.slave_fd = pty.openpty()
        winsize = struct.pack("HHHH", self.rows, self.cols, 0, 0)
        fcntl.ioctl(self.master_fd, termios.TIOCSWINSZ, winsize)
        fcntl.ioctl(self.slave_fd, termios.TIOCSWINSZ, winsize)

        self.pid = os.fork()
        if self.pid == 0:
            os.setsid()
            os.dup2(self.slave_fd, 0)
            os.dup2(self.slave_fd, 1)
            os.dup2(self.slave_fd, 2)
            os.close(self.master_fd)
            os.chdir(self.brain_shell_dir)
            os.execvpe(cmd[0], cmd, env)
        
        os.close(self.slave_fd)
        self.screen = pyte.Screen(self.cols, self.rows)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.stage_raw_bytes = bytearray()

    def respawn(self):
        self.terminate()
        self.master_fd, self.slave_fd = pty.openpty()
        winsize = struct.pack("HHHH", self.rows, self.cols, 0, 0)
        fcntl.ioctl(self.master_fd, termios.TIOCSWINSZ, winsize)
        fcntl.ioctl(self.slave_fd, termios.TIOCSWINSZ, winsize)

        preload_path = os.path.join(self.brain_shell_dir, "src", "preload.ts")
        main_path = os.path.join(self.brain_shell_dir, "src", "main.tsx")
        if self.target_type == "claude":
            cmd = ["/Users/ritikpathania/.local/bin/claude"]
        else:
            cmd = ["/Users/ritikpathania/.bun/bin/bun", "run", "--feature", "AUTO_THEME", "--preload", preload_path, main_path]

        env = dict(
            os.environ,
            HOME=self.home_dir,
            TERM="xterm-256color",
            COLUMNS=str(self.cols),
            LINES=str(self.rows),
            DISABLE_AUTOUPDATER="1",
            CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1",
            DISABLE_TELEMETRY="1"
        )
        try:
            import subprocess
            v_out = subprocess.check_output(["/Users/ritikpathania/.local/bin/claude", "--version"], text=True)
            env["CLAUDE_VERSION"] = v_out.strip().split()[0]
        except Exception:
            env["CLAUDE_VERSION"] = "2.1.235"

        self.pid = os.fork()
        if self.pid == 0:
            os.setsid()
            os.dup2(self.slave_fd, 0)
            os.dup2(self.slave_fd, 1)
            os.dup2(self.slave_fd, 2)
            os.close(self.master_fd)
            os.chdir(self.brain_shell_dir)
            os.execvpe(cmd[0], cmd, env)

        os.close(self.slave_fd)
        self.screen = pyte.Screen(self.cols, self.rows)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.stage_raw_bytes = bytearray()

    def drain(self, timeout_sec: float = 0.3):
        start = time.time()
        while time.time() - start < timeout_sec:
            r, _, _ = select.select([self.master_fd], [], [], 0.05)
            if self.master_fd in r:
                try:
                    chunk = os.read(self.master_fd, 8192)
                    if not chunk: break
                    self.stage_raw_bytes.extend(chunk)
                    self.stream.feed(self.decoder.decode(chunk))
                except OSError:
                    break
            else:
                break

    def wait_until(self, predicate: Callable[[pyte.Screen], bool], timeout_sec: float = 15.0) -> bool:
        start = time.time()
        while time.time() - start < timeout_sec:
            r, _, _ = select.select([self.master_fd], [], [], 0.05)
            if self.master_fd in r:
                try:
                    chunk = os.read(self.master_fd, 4096)
                    if not chunk: break
                    self.stage_raw_bytes.extend(chunk)
                    self.stream.feed(self.decoder.decode(chunk))
                except OSError: break
            if predicate(self.screen):
                return True
        return False

    def send(self, data: bytes):
        os.write(self.master_fd, data)

    def capture_canonical_frame(self, stage_index: int, stage_name: str) -> CanonicalFrame:
        self.drain(0.3)
        raw = bytes(self.stage_raw_bytes)
        self.stage_raw_bytes = bytearray()
        
        return extract_canonical_frame(
            self.screen,
            stage_index=stage_index,
            stage_name=stage_name,
            home_dir=self.home_dir,
            workspace_dir=self.brain_shell_dir,
            raw_bytes=raw
        )

    def get_persisted_theme(self) -> Optional[str]:
        settings_file = os.path.join(self.home_dir, ".claude", "settings.json")
        if os.path.exists(settings_file):
            try:
                with open(settings_file) as f:
                    t = json.load(f).get("theme")
                    if t: return t
            except: pass
            
        claude_file = os.path.join(self.home_dir, ".claude.json")
        if os.path.exists(claude_file):
            try:
                with open(claude_file) as f:
                    t = json.load(f).get("theme")
                    if t: return t
            except: pass
        return None

    def terminate(self):
        if self.pid:
            try:
                os.killpg(self.pid, 9)
            except:
                try:
                    os.kill(self.pid, 9)
                except:
                    pass
            try:
                os.waitpid(self.pid, os.WNOHANG)
            except:
                pass
        if self.master_fd:
            try:
                os.close(self.master_fd)
            except:
                pass

    def cleanup(self):
        self.terminate()
        for _ in range(5):
            if self.home_dir and os.path.exists(self.home_dir):
                shutil.rmtree(self.home_dir, ignore_errors=True)
                if not os.path.exists(self.home_dir):
                    break
                time.sleep(0.05)
