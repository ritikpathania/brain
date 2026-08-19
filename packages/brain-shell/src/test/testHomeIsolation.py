#!/usr/bin/env python3
import os
import sys
import json
import tempfile
import hashlib
from forensicCellDiff import capture_screen

target_dir = os.path.expanduser("~/Developer/PyCharm/brain/packages/brain-shell")
bun_bin = "/Users/ritikpathania/.bun/bin/bun"
claude_bin = "/Users/ritikpathania/.local/bin/claude"
preload_path = os.path.join(target_dir, "src", "preload.ts")
main_path = os.path.join(target_dir, "src", "main.tsx")

with tempfile.TemporaryDirectory() as shared_home:
    claude_dir = os.path.join(shared_home, ".claude")
    os.makedirs(claude_dir, exist_ok=True)
    cache_dir = os.path.join(claude_dir, "cache")
    os.makedirs(cache_dir, exist_ok=True)

    # 1. settings.json
    with open(os.path.join(claude_dir, "settings.json"), "w") as f:
        json.dump({
            "model": "claude-sonnet-4-6",
            "promptSuggestionEnabled": False,
            "permissions": {"allow": []}
        }, f)

    # 2. .claude.json
    with open(os.path.join(shared_home, ".claude.json"), "w") as f:
        json.dump({
            "hasCompletedOnboarding": True,
            "projects": {
                target_dir: {
                    "hasTrustDialogAccepted": True
                }
            },
            "shownTips": ["opus_1m_tip", "opus_1m", "tip_opus_1m", "opus_default_1m"],
            "opus1mMergeNoticeSeenCount": 10,
            "voiceNoticeSeenCount": 10,
            "lastReleaseNotesSeen": "2.1.233",
            "lastOnboardingVersion": "2.1.233"
        }, f)

    # 3. cache/changelog.md
    with open(os.path.join(cache_dir, "changelog.md"), "w") as f:
        f.write("# Changelog\n\n## 2.1.233\n- Initial release notes\n")

    env_override = {
        "HOME": shared_home,
        "USER": "ritikpathania",
        "ANTHROPIC_MODEL": "claude-sonnet-4-6"
    }

    c_screen, _ = capture_screen([claude_bin, "--bare"], target_dir, env_override)
    b_screen, _ = capture_screen([bun_bin, "run", "--preload", preload_path, main_path, "--bare"], target_dir, env_override)

    print("CLAUDE DISPLAY:")
    for idx, l in enumerate(c_screen.display):
        print(f"[{idx:02d}] {repr(l)}")

    print("\nBRAIN DISPLAY:")
    for idx, l in enumerate(b_screen.display):
        print(f"[{idx:02d}] {repr(l)}")

    diffs = 0
    for idx in range(len(c_screen.display)):
        if c_screen.display[idx] != b_screen.display[idx]:
            diffs += 1
            print(f"Row {idx:02d} DIFF:")
            print(f"  CLAUDE: {repr(c_screen.display[idx])}")
            print(f"  BRAIN : {repr(b_screen.display[idx])}")

    print(f"\nTOTAL DIFF ROWS: {diffs}")
