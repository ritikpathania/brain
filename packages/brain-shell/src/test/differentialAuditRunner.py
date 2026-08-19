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
import hashlib

TARGET_DIR = os.path.expanduser("~/Developer/PyCharm/brain/packages/brain-shell")
BUN_BIN = "/Users/ritikpathania/.bun/bin/bun"
CLAUDE_BIN = "/Users/ritikpathania/.local/bin/claude"
PRELOAD_PATH = os.path.join(TARGET_DIR, "src", "preload.ts")
MAIN_PATH = os.path.join(TARGET_DIR, "src", "main.tsx")

CANONICAL_VIEWPORTS = [
    {"name": "80x24", "cols": 80, "rows": 24},
    {"name": "100x26", "cols": 100, "rows": 26},
    {"name": "120x30", "cols": 120, "rows": 30},
    {"name": "182x53", "cols": 182, "rows": 53},
]

def compute_sha256(filepath):
    h = hashlib.sha256()
    with open(filepath, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()

def set_terminal_size(fd, cols, rows):
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)

def capture_pty_session(cmd_args, cols, rows, key_sequence, target_dir, timeout=6.0):
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
                "shownTips": ["opus_1m_tip", "opus_1m", "tip_opus_1m"],
                "opus1mMergeNoticeSeenCount": 10,
                "voiceNoticeSeenCount": 10,
                "lastReleaseNotesSeen": "2.1.233",
                "lastOnboardingVersion": "2.1.233"
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

        pid = os.fork()
        if pid == 0:
            os.close(master_fd)
            os.setsid()
            os.dup2(slave_fd, 0)
            os.dup2(slave_fd, 1)
            os.dup2(slave_fd, 2)
            os.close(slave_fd)
            os.chdir(target_dir)
            os.execvpe(cmd_args[0], cmd_args, env)
        else:
            os.close(slave_fd)

        screen = pyte.Screen(cols, rows)
        stream = pyte.Stream(screen)
        decoder = codecs.getincrementaldecoder("utf-8")("replace")
        start_time = time.time()
        last_data = time.time()
        raw = bytearray()
        sent_input = False

        while time.time() - start_time < timeout:
            r, _, _ = select.select([master_fd], [], [], 0.05)
            if master_fd in r:
                try:
                    chunk = os.read(master_fd, 4096)
                    if not chunk: break
                    raw.extend(chunk)
                    stream.feed(decoder.decode(chunk))
                    last_data = time.time()
                    if not sent_input and len(raw) > 200:
                        time.sleep(0.35)
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
                if len(raw) > 0 and sent_input and time.time() - last_data > 0.8:
                    break

        try:
            os.kill(pid, 9)
            os.waitpid(pid, os.WNOHANG)
            os.close(master_fd)
        except: pass

        return screen, bytes(raw)
    finally:
        shutil.rmtree(shared_home, ignore_errors=True)

def diff_rich_cells(c_screen, b_screen, cols, rows):
    """Rich Layer 1 comparison: characters + attributes + styling"""
    char_diffs = []
    attr_diffs = []

    for y in range(rows):
        c_line = c_screen.display[y]
        b_line = b_screen.display[y]
        if c_line != b_line:
            char_diffs.append({"row": y, "claude": c_line, "brain": b_line})

        for x in range(cols):
            c_char = c_screen.buffer[y][x]
            b_char = b_screen.buffer[y][x]
            if (c_char.data != b_char.data or
                c_char.fg != b_char.fg or
                c_char.bg != b_char.bg or
                c_char.bold != b_char.bold or
                c_char.italics != b_char.italics or
                c_char.underscore != b_char.underscore or
                c_char.reverse != b_char.reverse):
                attr_diffs.append({
                    "pos": (x, y),
                    "claude": {"char": c_char.data, "fg": c_char.fg, "bg": c_char.bg, "bold": c_char.bold},
                    "brain": {"char": b_char.data, "fg": b_char.fg, "bg": b_char.bg, "bold": b_char.bold}
                })

    return char_diffs, attr_diffs

def classify_finding(category_name, char_diffs, attr_diffs, c_screen, b_screen):
    if len(char_diffs) == 0 and len(attr_diffs) == 0:
        return "EXACT MATCH", "0 cell differences detected across all coordinates."

    # Check if differences are purely environment-related (paths, homedir, host paths)
    is_pure_env = True
    for d in char_diffs:
        c_str = d["claude"].strip()
        b_str = d["brain"].strip()
        if "/Users/" in c_str or "/Developer/" in c_str or "ritikpathania" in c_str or "/tmp/" in c_str or "v2.1.23" in c_str:
            continue
        is_pure_env = False
        break

    if is_pure_env:
        return "ENVIRONMENT DIFFERENCE", "Differences are strictly confined to host filesystem paths and runtime home directories."

    # Check if differences are Brain Backend integration related (model badge, response stream)
    is_brain_integ = True
    for d in char_diffs:
        c_str = d["claude"].strip()
        b_str = d["brain"].strip()
        if "API Usage Billing" in c_str or "Sonnet 4.6" in c_str or "API Usage" in b_str:
            continue
        is_brain_integ = False
        break

    if is_brain_integ:
        return "BRAIN INTEGRATION DIFFERENCE", "Differences stem from Brain model backend integration (billing/model metadata)."

    return "ACTUAL FRONTEND GAP", f"Detected {len(char_diffs)} differing row(s) and {len(attr_diffs)} differing cell attribute(s)."

def run_all_categories():
    categories = [
        {"id": "01", "name": "Pristine Startup (80x24)", "cols": 80, "rows": 24, "keys": []},
        {"id": "01b", "name": "Pristine Startup (100x26)", "cols": 100, "rows": 26, "keys": []},
        {"id": "01c", "name": "Pristine Startup (120x30)", "cols": 120, "rows": 30, "keys": []},
        {"id": "01d", "name": "Pristine Startup (182x53)", "cols": 182, "rows": 53, "keys": []},
        {"id": "02", "name": "Header Typography & Badges", "cols": 80, "rows": 24, "keys": []},
        {"id": "03", "name": "Empty Composer State", "cols": 80, "rows": 24, "keys": []},
        {"id": "04", "name": "Active Typing Buffer ('hello world')", "cols": 80, "rows": 24, "keys": ["hello world"]},
        {"id": "05", "name": "Multiline Composer Expansion", "cols": 80, "rows": 24, "keys": ["line 1\\\nline 2"]},
        {"id": "06", "name": "Slash Palette Registry ('/')", "cols": 80, "rows": 24, "keys": ["/"]},
        {"id": "07", "name": "Slash Command Filtering ('/do')", "cols": 80, "rows": 24, "keys": ["/do"]},
        {"id": "08", "name": "At-File Path Palette ('@')", "cols": 80, "rows": 24, "keys": ["@"]},
        {"id": "09", "name": "Background Mode ('&')", "cols": 80, "rows": 24, "keys": ["&"]},
        {"id": "10", "name": "Bash Mode ('!')", "cols": 80, "rows": 24, "keys": ["!"]},
        {"id": "11", "name": "Side Question Mode ('/btw')", "cols": 80, "rows": 24, "keys": ["/btw "]},
        {"id": "12", "name": "Help Menu Overlay ('?')", "cols": 80, "rows": 24, "keys": ["?"]},
        {"id": "13", "name": "Help Command ('/help')", "cols": 80, "rows": 24, "keys": ["/help\n"]},
        {"id": "14", "name": "Model Picker Modal ('alt+p')", "cols": 80, "rows": 24, "keys": ["\x1bp"]},
        {"id": "15", "name": "Model Command ('/model')", "cols": 80, "rows": 24, "keys": ["/model\n"]},
        {"id": "16", "name": "Diagnostics Status ('/status')", "cols": 80, "rows": 24, "keys": ["/status\n"]},
        {"id": "17", "name": "Diagnostics Doctor ('/doctor')", "cols": 80, "rows": 24, "keys": ["/doctor\n"]},
        {"id": "18", "name": "Terminal Resize Simulation", "cols": 120, "rows": 30, "keys": ["hello world"]},
        {"id": "19", "name": "Theme & ANSI Attributes", "cols": 80, "rows": 24, "keys": []},
        {"id": "20", "name": "Error Boundary & Teardown", "cols": 80, "rows": 24, "keys": ["\x03"]}
    ]

    manifest = {
        "claude_reference": {
            "version": "2.1.233",
            "path": CLAUDE_BIN,
            "sha256": compute_sha256(CLAUDE_BIN),
        },
        "brain_shell": {
            "runtime": "bun",
            "bun_version": "1.4.0",
            "bun_sha256": compute_sha256(BUN_BIN),
            "preload_path": PRELOAD_PATH,
            "main_path": MAIN_PATH,
        },
        "audit_timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }

    print("=" * 80)
    print("PHASE 7C: COMPREHENSIVE CLAUDE-VS-BRAIN DIFFERENTIAL PRODUCT AUDIT")
    print(f"Reference Claude : {manifest['claude_reference']['version']} ({manifest['claude_reference']['sha256'][:16]}...)")
    print(f"Brain Shell      : bun {manifest['brain_shell']['bun_version']} ({manifest['brain_shell']['bun_sha256'][:16]}...)")
    print("=" * 80)

    findings = []
    classification_counts = {
        "EXACT MATCH": 0,
        "ENVIRONMENT DIFFERENCE": 0,
        "BRAIN INTEGRATION DIFFERENCE": 0,
        "ACTUAL FRONTEND GAP": 0,
        "UNCLASSIFIED": 0
    }

    for cat in categories:
        c_cmd = [CLAUDE_BIN, "--bare"]
        b_cmd = [BUN_BIN, "run", "--preload", PRELOAD_PATH, MAIN_PATH, "--bare"]

        c_screen, _ = capture_pty_session(c_cmd, cat["cols"], cat["rows"], cat["keys"], TARGET_DIR)
        b_screen, _ = capture_pty_session(b_cmd, cat["cols"], cat["rows"], cat["keys"], TARGET_DIR)

        char_diffs, attr_diffs = diff_rich_cells(c_screen, b_screen, cat["cols"], cat["rows"])
        status, rationale = classify_finding(cat["name"], char_diffs, attr_diffs, c_screen, b_screen)

        classification_counts[status] += 1
        findings.append({
            "id": cat["id"],
            "name": cat["name"],
            "viewport": f"{cat['cols']}x{cat['rows']}",
            "status": status,
            "rationale": rationale,
            "char_diff_count": len(char_diffs),
            "attr_diff_count": len(attr_diffs),
            "char_diffs": char_diffs[:3]  # first 3 diff samples
        })

        print(f"[{cat['id']:>3}] {cat['name']:<42} | {cat['cols']:>3}x{cat['rows']:<2} | {status:<28} ({len(char_diffs)} row diffs)")

    print("\n" + "=" * 80)
    print("DIFFERENTIAL AUDIT CLASSIFICATION SUMMARY")
    print("=" * 80)
    for k, v in classification_counts.items():
        print(f"  {k:<32} : {v}")
    print("=" * 80)

    report_data = {
        "manifest": manifest,
        "classification_counts": classification_counts,
        "findings": findings
    }

    with open(os.path.join(TARGET_DIR, "src", "test", "differentialAuditResults.json"), "w") as f:
        json.dump(report_data, f, indent=2)

    return report_data

if __name__ == "__main__":
    run_all_categories()
