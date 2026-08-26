#!/usr/bin/env python3
"""
Safe Slash Command Execution Matrix
Consumes slash_command_inventory.json, executes every discovered SAFE command with arguments,
observes resulting screen state, and derives explicit InteractionEvidence via classify_evidence().

Evidence semantics:
  VERIFIED — expected UI behavior observed (structural state matched expectation)
             NOTE: this proves expected UI behavior, NOT semantic command success.
  FAILED   — structural state did NOT match expectation (unexpected state change / non-change)
  UNAVAILABLE — session could not launch or reach ready state

Command behaviors (CommandBehavior):
  UNCHANGED — command runs and returns to the same structural state (e.g. /doctor, /compact)
  CHANGED   — command produces a structural state change (e.g. /color cyan, /theme dark)

Unknown commands default to UNCHANGED (conservative): if the state unexpectedly changes,
the result is FAILED, making the regression visible rather than silently manufactured.

Zero static command fallbacks permitted!
"""

import os
import sys
import time
import json
from enum import Enum
from pathlib import Path
from dataclasses import dataclass, asdict
from typing import Dict, List

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from qa.claude_ux.driver.session import ClaudeSession, SessionResult
from qa.claude_ux.discovery.readiness import ReadinessStateMachine
from qa.claude_ux.design_audit.state_machine import StructuralStateAnalyzer, classify_evidence


class CommandBehavior(str, Enum):
    """Expected structural behavior of a slash command.

    UNCHANGED — the structural screen fingerprint should NOT change after execution
               (command runs and returns to the same TUI state).
    CHANGED   — the structural screen fingerprint IS expected to change.
    """
    UNCHANGED = "UNCHANGED"
    CHANGED   = "CHANGED"


# Explicit per-command behavioral expectations.
# Unknown commands default to UNCHANGED (conservative regression default).
COMMAND_BEHAVIORS: Dict[str, CommandBehavior] = {
    "/doctor":  CommandBehavior.UNCHANGED,
    "/init":    CommandBehavior.UNCHANGED,
    "/bug":     CommandBehavior.UNCHANGED,
    "/compact": CommandBehavior.UNCHANGED,
    "/context": CommandBehavior.UNCHANGED,
    "/color":   CommandBehavior.CHANGED,
    "/theme":   CommandBehavior.CHANGED,
}


@dataclass
class InteractionEvidence:
    source_state: str
    input_kind: str         # "command" | "command_argument"
    input_value: str
    advertised: bool
    source_confirmed: bool
    safe_to_test: bool
    executed: bool
    result_state: str
    state_changed: bool
    visual_changed: bool
    animation_detected: bool
    command_behavior: str   # CommandBehavior value
    evidence_classification: str
    # Human-readable semantics note (does NOT change the evidence tier)
    evidence_note: str


class CommandExecutionMatrix:
    """Executes safe commands discovered dynamically from slash_command_inventory.json."""

    COMMAND_ARGUMENTS = {
        "/color": "cyan",
        "/theme": "dark"
    }

    def __init__(self, run_dir: Path, *, transport_verified: bool):
        self.run_dir = run_dir
        self.transport_verified = transport_verified
        self.execution_results = []
        self.session_results: List[SessionResult] = []

    def run_matrix(self, viewport: tuple = (80, 24)) -> list:
        print("=== Executing Safe Slash Command Execution Matrix ===")
        inventory_file = self.run_dir / "slash_command_inventory.json"

        discovered_cmds = {}
        if inventory_file.exists():
            with open(inventory_file) as f:
                data = json.load(f)
                discovered_cmds = data.get("commands", {})

        # Zero static command fallback — consume strictly discovered SAFE commands.
        safe_cmds = [
            c_info["command"]
            for c_name, c_info in discovered_cmds.items()
            if c_info.get("classification") == "SAFE"
        ]

        for cmd_name in safe_cmds:
            arg = self.COMMAND_ARGUMENTS.get(cmd_name)
            full_cmd = f"{cmd_name} {arg}" if arg else cmd_name
            print(f"  Executing Command: '{full_cmd}'... ", end="", flush=True)

            session = ClaudeSession(self.run_dir, f"cmd_exec_{cmd_name.replace('/', '')}", viewport)
            try:
                if session.launch():
                    readiness = ReadinessStateMachine(session.driver)
                    ok, _, _ = readiness.evaluate_readiness(session.launch_record)
                    if ok:
                        init_lines = session.observe_terminal_state()
                        init_fp, _, _ = StructuralStateAnalyzer.analyze(init_lines, viewport)

                        session.type_slash_command_and_submit(full_cmd)
                        time.sleep(0.4)

                        post_lines = session.observe_terminal_state()
                        post_fp, _, _ = StructuralStateAnalyzer.analyze(post_lines, viewport)

                        state_changed = (post_fp.state_id() != init_fp.state_id())

                        # Derive expectation from the command's declared behavioral contract.
                        # Unknown commands default to UNCHANGED (conservative).
                        behavior = COMMAND_BEHAVIORS.get(cmd_name, CommandBehavior.UNCHANGED)
                        if behavior == CommandBehavior.UNCHANGED:
                            matches_exp = not state_changed
                        else:
                            matches_exp = state_changed

                        evidence_str = classify_evidence(
                            action_executed=True,
                            transport_verified=self.transport_verified,
                            parent_state_known=True,
                            post_state_observed=True,
                            transition_matches_expectation=matches_exp
                        )

                        if evidence_str == "VERIFIED":
                            session.mark_completed()
                        else:
                            session.mark_failed()
                        evidence = InteractionEvidence(
                            source_state=init_fp.state_id(),
                            input_kind="command" if not arg else "command_argument",
                            input_value=full_cmd,
                            advertised=True,
                            source_confirmed=True,
                            safe_to_test=True,
                            executed=True,
                            result_state=post_fp.state_id(),
                            state_changed=state_changed,
                            visual_changed=state_changed,
                            animation_detected=False,
                            command_behavior=behavior.value,
                            evidence_classification=evidence_str,
                            evidence_note="VERIFIED means expected UI behavior observed, not semantic command success"
                        )
                        self.execution_results.append(asdict(evidence))
                        print(evidence_str)
                    else:
                        session.mark_failed()
                        print("UNAVAILABLE")
                else:
                    session.mark_failed()
            finally:
                self.session_results.append(session.make_result())
                session.close()
                time.sleep(0.4)

        out_json = self.run_dir / "command_execution_results.json"
        with open(out_json, "w") as f:
            json.dump(self.execution_results, f, indent=2)

        print(f"Command Execution Matrix Complete: {len(self.execution_results)} safe commands executed.")
        print(f"Saved results to: {out_json}")
        return self.execution_results, self.session_results
