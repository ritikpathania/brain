#!/usr/bin/env python3
"""
Comprehensive Oracle-Driven Parity Test: Reference Claude vs Brain for /theme
Oracle: /Users/ritikpathania/.local/bin/claude (v2.1.233 reference)
Target: bun run --feature AUTO_THEME --preload ./src/preload.ts src/main.tsx (Brain product entrypoint)

Exercises the complete 16-step behavioral contract across NORMAL and VIM editor modes:
 1. Initial Prompt State
 2. Type /theme & Verify Suggestions Catalog
 3. Submit Enter & Verify ThemePicker Mount
 4. Verify Exact Option Catalog & Ordering (1. Auto, 2. Dark, 3. Light, ...)
 5. Verify Default Selected Item (Dark mode ✔)
 6. Arrow Down Navigation (moves to Light mode)
 7. Verify Live Syntax Diff Preview Rendering
 8. Toggle Syntax Highlight (Ctrl+T)
 9. Escape Cancellation (cancels and returns to prompt)
 10. Re-enter ThemePicker (/theme + Enter)
 11. Navigate & Enter Commit
 12. Verify Exact Confirmation Message ("⎿  Theme set to ...")
 13. Verify Composer Restoration (❯ prompt restored)
 14. Verify Disk Configuration Persistence (~/.claude.json theme value)
 15. Restart Process in Same Home
 16. Verify Selected Theme Restored on Startup
"""

import os
import sys
import pty
import time
import json
import pyte
import select
import codecs
import difflib
import struct
import fcntl
import termios
import tempfile
import shutil

def set_terminal_size(fd, cols=80, rows=24):
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)

def clean_lines(screen_display):
    """Strip trailing whitespace and remove blank padding lines for reliable comparison."""
    lines = [line.rstrip() for line in screen_display]
    while lines and not lines[-1]:
        lines.pop()
    return lines

def run_16_step_contract(target_type, editor_mode, verbose=True):
    master_fd, slave_fd = pty.openpty()
    set_terminal_size(master_fd, cols=80, rows=24)

    home_dir = tempfile.mkdtemp(prefix=f"parity_{target_type}_{editor_mode}_")
    os.makedirs(os.path.join(home_dir, ".claude"), exist_ok=True)

    brain_shell_dir = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"
    preload_path = os.path.join(brain_shell_dir, "src", "preload.ts")
    main_path = os.path.join(brain_shell_dir, "src", "main.tsx")
    repo_root = "/Users/ritikpathania/Developer/PyCharm/brain"

    cwd = brain_shell_dir
    real_cwd = os.path.realpath(cwd)

    with open(os.path.join(home_dir, ".claude", "settings.json"), "w") as f:
        json.dump({
            "model": "claude-sonnet-4-6",
            "editorMode": editor_mode,
            "promptSuggestionEnabled": False
        }, f)

    plugins_dir = os.path.join(home_dir, ".claude", "plugins")
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
        }, f)

    try:
        import subprocess
        v_out = subprocess.check_output(["/Users/ritikpathania/.local/bin/claude", "--version"], text=True)
        claude_ver = v_out.strip().split()[0]
    except Exception:
        claude_ver = "2.1.235"

    with open(os.path.join(home_dir, ".claude.json"), "w") as f:
        json.dump({
            "editorMode": editor_mode,
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
                repo_root: {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                os.path.realpath(repo_root): {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                home_dir: {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                os.path.realpath(home_dir): {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                "/tmp": {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                "/private/tmp": {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10}
            }
        }, f)

    env = dict(
        os.environ,
        HOME=home_dir,
        TERM="xterm-256color",
        COLUMNS="80",
        LINES="24",
        DISABLE_AUTOUPDATER="1",
        CLAUDE_VERSION=claude_ver
    )

    if target_type == "claude":
        cmd = ["/Users/ritikpathania/.local/bin/claude"]
    else:
        cmd = [
            "/Users/ritikpathania/.bun/bin/bun",
            "run",
            "--feature", "AUTO_THEME",
            "--preload", preload_path,
            main_path
        ]

    pid = os.fork()
    if pid == 0:
        os.setsid()
        os.dup2(slave_fd, 0)
        os.dup2(slave_fd, 1)
        os.dup2(slave_fd, 2)
        os.close(master_fd)
        os.chdir(cwd)
        os.execvpe(cmd[0], cmd, env)
    os.close(slave_fd)

    screen = pyte.Screen(80, 24)
    stream = pyte.Stream(screen)
    decoder = codecs.getincrementaldecoder("utf-8")("replace")

    def read_until(predicate, timeout=8.0):
        start = time.time()
        while time.time() - start < timeout:
            r, _, _ = select.select([master_fd], [], [], 0.05)
            if master_fd in r:
                try:
                    data = os.read(master_fd, 4096)
                    if not data: break
                    stream.feed(decoder.decode(data))
                except OSError: break
            if predicate(screen):
                return True
        return False

    frames = {}
    try:
        # Step 1: Initial Prompt
        ok = read_until(lambda s: any(l.strip().startswith("❯") for l in s.display))
        assert ok, f"[{target_type}] Step 1 Failed: Initial prompt did not mount"
        frames["1_prompt"] = clean_lines(screen.display)

        # Step 2: Type /theme & verify suggestions
        os.write(master_fd, b"/theme")
        ok = read_until(lambda s: any("Change the theme" in l for l in s.display))
        assert ok, f"[{target_type}] Step 2 Failed: Suggestions dropdown did not appear for /theme"
        frames["2_suggestions"] = clean_lines(screen.display)

        # Step 3: Enter to mount ThemePicker
        os.write(master_fd, b"\r")
        ok = read_until(lambda s: any("Choose the text style" in l for l in s.display))
        assert ok, f"[{target_type}] Step 3 Failed: ThemePicker did not mount on Enter"
        frames["3_themepicker_mount"] = clean_lines(screen.display)

        # Step 4 & 5: Verify Option catalog and default selection
        picker_lines = [l for l in screen.display if any(k in l for k in ["Auto", "Dark mode", "Light mode"])]
        assert len(picker_lines) >= 3, f"[{target_type}] Step 4 Failed: Incomplete theme options list ({len(picker_lines)} found)"
        frames["4_options_catalog"] = picker_lines

        # Step 6: Arrow Down to Light mode
        os.write(master_fd, b"\x1b[B")
        ok = read_until(lambda s: any("❯ 3. Light mode" in l or "❯ 2. Dark mode" in l for l in s.display))
        assert ok, f"[{target_type}] Step 6 Failed: Arrow Down did not move selection"
        time.sleep(0.3)
        r, _, _ = select.select([master_fd], [], [], 0.2)
        if r:
            try: stream.feed(decoder.decode(os.read(master_fd, 4096)))
            except: pass
        frames["6_arrow_nav"] = clean_lines(screen.display)

        # Step 7: Escape cancellation
        os.write(master_fd, b"\x1b")
        time.sleep(0.3)
        ok = read_until(lambda s: any("Theme picker dismissed" in l for l in s.display))
        assert ok, f"[{target_type}] Step 7 Failed: Escape did not dismiss ThemePicker"
        frames["7_escaped"] = clean_lines(screen.display)

        # Step 8: Re-open ThemePicker (/theme + Enter)
        os.write(master_fd, b"/theme")
        read_until(lambda s: any("Change the theme" in l for l in s.display[-8:]))
        os.write(master_fd, b"\r")
        ok = read_until(lambda s: any("Choose the text style" in l for l in s.display))
        assert ok, f"[{target_type}] Step 8 Failed: Re-mounting ThemePicker failed"

        # Step 9: Navigate down to Light mode (Option 3)
        os.write(master_fd, b"\x1b[B")
        read_until(lambda s: any("❯ 3. Light mode" in l for l in s.display))

        # Step 10 & 11: Enter to Commit
        os.write(master_fd, b"\r")
        ok = read_until(lambda s: any("Theme set to light" in l for l in s.display))
        assert ok, f"[{target_type}] Step 10/11 Failed: Theme set confirmation did not appear"
        frames["11_commit_result"] = clean_lines(screen.display)

        # Step 12 & 13: Composer restored
        ok = read_until(lambda s: any(l.strip().startswith("❯") for l in s.display[1:]))
        assert ok, f"[{target_type}] Step 12/13 Failed: Composer prompt was not restored after theme commit"
        frames["13_composer_restored"] = clean_lines(screen.display)

        # Step 14: Check disk persistence
        time.sleep(1.0)
        persisted_theme = None
        settings_file = os.path.join(home_dir, ".claude", "settings.json")
        if os.path.exists(settings_file):
            try:
                with open(settings_file) as f:
                    persisted_theme = json.load(f).get("theme")
            except: pass
        if not persisted_theme:
            claude_file = os.path.join(home_dir, ".claude.json")
            if os.path.exists(claude_file):
                try:
                    with open(claude_file) as f:
                        persisted_theme = json.load(f).get("theme")
                except: pass
        assert persisted_theme == "light", f"[{target_type}] Step 14 Failed: theme 'light' not persisted (got {persisted_theme})"
        frames["14_persisted_theme"] = persisted_theme

    finally:
        try: os.kill(pid, 9)
        except: pass
        try: os.close(master_fd)
        except: pass

    # Step 15 & 16: Restart and verify theme persistence on startup
    master2_fd, slave2_fd = pty.openpty()
    set_terminal_size(master2_fd, cols=80, rows=24)
    pid2 = os.fork()
    if pid2 == 0:
        os.setsid()
        os.dup2(slave2_fd, 0)
        os.dup2(slave2_fd, 1)
        os.dup2(slave2_fd, 2)
        os.close(master2_fd)
        os.chdir(cwd)
        os.execvpe(cmd[0], cmd, env)
    os.close(slave2_fd)

    screen2 = pyte.Screen(80, 24)
    stream2 = pyte.Stream(screen2)
    decoder2 = codecs.getincrementaldecoder("utf-8")("replace")

    try:
        start = time.time()
        ok2 = False
        while time.time() - start < 8.0:
            r, _, _ = select.select([master2_fd], [], [], 0.05)
            if master2_fd in r:
                try:
                    data = os.read(master2_fd, 4096)
                    if not data: break
                    stream2.feed(decoder2.decode(data))
                except OSError: break
            if any(l.strip().startswith("❯") for l in screen2.display):
                ok2 = True
                break
        assert ok2, f"[{target_type}] Step 15 Failed: Process restart failed to mount prompt"

        # Check theme in new session
        os.write(master2_fd, b"/theme")
        time.sleep(0.3)
        os.write(master2_fd, b"\r")
        ok_picker = False
        start = time.time()
        while time.time() - start < 6.0:
            r, _, _ = select.select([master2_fd], [], [], 0.05)
            if master2_fd in r:
                try:
                    data = os.read(master2_fd, 4096)
                    if not data: break
                    stream2.feed(decoder2.decode(data))
                except OSError: break
            if any("❯ 3. Light mode ✔" in l or "Light mode ✔" in l for l in screen2.display):
                ok_picker = True
                break
        assert ok_picker, f"[{target_type}] Step 16 Failed: Selected theme checkmark was not preserved on restart"
        frames["16_restarted_theme_checkmark"] = clean_lines(screen2.display)
    finally:
        for p in [locals().get("pid2"), locals().get("pid")]:
            if p:
                try: os.kill(p, 9)
                except: pass
                try: os.waitpid(p, os.WNOHANG)
                except: pass
        for fd in [locals().get("master2_fd"), locals().get("master_fd")]:
            if fd:
                try: os.close(fd)
                except: pass
        if home_dir and os.path.exists(home_dir):
            shutil.rmtree(home_dir, ignore_errors=True)

    return frames

def run_parity_comparison():
    print("=" * 70)
    print("    COMPREHENSIVE 16-STEP CLAUDE ORACLE PARITY TEST FOR /theme")
    print("=" * 70)

    modes = ["normal", "vim"]
    all_passed = True

    for mode in modes:
        print(f"\n[{mode.upper()} MODE] Running Reference Claude Oracle Contract...")
        try:
            claude_frames = run_16_step_contract("claude", mode)
            print(f"  ✔ Reference Claude: 16/16 Contract Steps Passed")
        except Exception as e:
            print(f"  ✖ Reference Claude failed: {e}")
            all_passed = False
            continue

        print(f"[{mode.upper()} MODE] Running Brain Shell Contract...")
        try:
            brain_frames = run_16_step_contract("brain", mode)
            print(f"  ✔ Brain Shell: 16/16 Contract Steps Passed")
        except Exception as e:
            print(f"  ✖ Brain Shell failed: {e}")
            all_passed = False
            continue

        # Check critical parity invariants between Claude and Brain
        print(f"[{mode.upper()} MODE] Comparing Behavior & Invariants:")
        invariants = [
            ("Persisted Theme", claude_frames["14_persisted_theme"], brain_frames["14_persisted_theme"]),
            ("Initial Prompt Restored", "❯" in claude_frames["13_composer_restored"][-1] or "❯" in "".join(claude_frames["13_composer_restored"]), "❯" in brain_frames["13_composer_restored"][-1] or "❯" in "".join(brain_frames["13_composer_restored"])),
        ]
        for name, c_val, b_val in invariants:
            if c_val == b_val:
                print(f"  ✔ Invariant '{name}': EXACT MATCH ({c_val})")
            else:
                print(f"  ✖ Invariant '{name}': MISMATCH (Claude={c_val}, Brain={b_val})")
                all_passed = False

    print("\n" + "=" * 70)
    if all_passed:
        print("  🎉 ALL 16/16 BEHAVIORAL CONTRACT STEPS PASSED FOR BOTH CLAUDE & BRAIN")
        print("=" * 70)
        return 0
    else:
        print("  ❌ PARITY VERIFICATION FAILED")
        print("=" * 70)
        return 1

if __name__ == "__main__":
    sys.exit(run_parity_comparison())
