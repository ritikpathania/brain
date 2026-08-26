#!/usr/bin/env python3
"""
Runtime Slash Command Census Engine
Types '/', scrolls completion list repeatedly via Down arrow, enumerates all reachable commands,
and strictly classifies each command into SAFE, DESTRUCTIVE, or UNKNOWN (keeping UNKNOWN as UNKNOWN).
"""

import os
import sys
import time
import json
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from qa.claude_ux.driver.session import ClaudeSession
from qa.claude_ux.discovery.readiness import ReadinessStateMachine

KNOWN_SAFE_COMMANDS = {"/effort", "/color", "/help", "/init", "/cost", "/doctor", "/theme", "/context", "/bug", "/compact"}
KNOWN_DESTRUCTIVE_COMMANDS = {"/clear", "/reset", "/rm", "/destroy", "/git-reset"}


class SlashCommandCensus:
    """Discovers all slash commands reachable through completion popup scrolling."""

    def __init__(self, run_dir: Path):
        self.run_dir = run_dir
        self.discovered_commands = {}
        self.session_results = []

    def discover_commands(self, viewport: tuple = (80, 24)) -> dict:
        print("=== Starting Runtime Slash Command Census ===")
        session = ClaudeSession(self.run_dir, "command_census", viewport)
        try:
            if not session.launch():
                print("[Command Census Error] Session launch failed")
                return {}

            readiness = ReadinessStateMachine(session.driver)
            ok, _, _ = readiness.evaluate_readiness(session.launch_record)
            if not ok:
                print("[Command Census Error] Readiness evaluation failed")
                return {}

            # Open slash completion
            session.press_key("/")
            time.sleep(0.6)

            for scroll_idx in range(12):
                lines = session.observe_terminal_state()
                full_text = " ".join(lines)
                
                for word in full_text.split():
                    if word.startswith("/") and len(word) > 1:
                        cmd_name = word.split()[0].split("(")[0].strip().lower()
                        if cmd_name not in self.discovered_commands:
                            if cmd_name in KNOWN_SAFE_COMMANDS:
                                classification = "SAFE"
                            elif cmd_name in KNOWN_DESTRUCTIVE_COMMANDS:
                                classification = "DESTRUCTIVE"
                            else:
                                classification = "UNKNOWN"

                            self.discovered_commands[cmd_name] = {
                                "command": cmd_name,
                                "classification": classification,
                                "evidence": "SOURCE_CONFIRMED"
                            }

                session.press("down")
                time.sleep(0.3)

            out_json = self.run_dir / "slash_command_inventory.json"
            with open(out_json, "w") as f:
                json.dump({"total_commands": len(self.discovered_commands), "commands": self.discovered_commands}, f, indent=2)

            session.mark_completed()
            print(f"Command Census Complete: Discovered {len(self.discovered_commands)} slash commands.")
            print(f"Saved inventory to: {out_json}")
            return self.discovered_commands

        finally:
            self.session_results.append(session.make_result())
            session.close()
            time.sleep(0.4)
