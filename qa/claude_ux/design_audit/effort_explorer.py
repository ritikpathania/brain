#!/usr/bin/env python3
"""
Path-Replayed Interactive /effort Selector & Boundary Explorer
Launches a fresh path-replayed session for EVERY boundary trial.
Navigates to the required starting position within the /effort slider,
then performs a single action and observes pre/post state.

Required trials:
  low + Left Arrow        → UNCHANGED  (lower boundary no-op)
  ultracode + Right Arrow → UNCHANGED  (upper boundary no-op)
  Esc                     → CHANGED_TO (cancel restores prior state)

Each trial uses its own isolated session to guarantee clean path-replay.
transport_verified is dependency-injected at construction time.
"""

import os
import sys
import time
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import List, Optional

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from qa.claude_ux.driver.session import ClaudeSession, SessionResult
from qa.claude_ux.discovery.readiness import ReadinessStateMachine
from qa.claude_ux.design_audit.state_machine import StructuralStateAnalyzer, StateExpectation, classify_evidence


@dataclass
class EffortTrial:
    """Specifies one /effort slider boundary trial.

    navigate_to: the target position to reach before pressing `action`.
        "low"       — press Left repeatedly until state stops changing
        "ultracode" — press Right repeatedly until state stops changing
        None        — start from default position (first option selected)
    action: the key to press after reaching the navigation target.
    description: human-readable trial label.
    expectation: the expected state transition kind.
    """
    navigate_to: Optional[str]   # "low" | "ultracode" | None
    action: str
    description: str
    expectation: StateExpectation


# Navigation direction and key for each boundary position
_BOUNDARY_NAV: dict = {
    "low":       ("left",  8),    # press left up to 8 times to reach minimum
    "ultracode": ("right", 8),    # press right up to 8 times to reach maximum
}

TRIALS: List[EffortTrial] = [
    EffortTrial(
        navigate_to="low",
        action="left",
        description="low + Left Arrow (Lower Boundary No-Op)",
        expectation=StateExpectation(kind="UNCHANGED"),
    ),
    EffortTrial(
        navigate_to="ultracode",
        action="right",
        description="ultracode + Right Arrow (Upper Boundary No-Op)",
        expectation=StateExpectation(kind="UNCHANGED"),
    ),
    EffortTrial(
        navigate_to=None,
        action="esc",
        description="Esc Cancel Selection",
        expectation=StateExpectation(kind="CHANGED_TO", expected_screen="01_home"),
    ),
]


def _is_at_semantic_boundary(lines: List[str], target: str) -> bool:
    """Verifies that the exact requested option ('low' or 'ultracode') is structurally
    marked as selected/focused in the terminal buffer.

    Focus indicators:
    - Target option token enclosed in focus brackets, e.g. [low] or [ultracode]
    - Or target option token immediately preceded by selection marker (▶ low, ● ultracode, etc.)
    """
    target_token = target.lower()
    focus_markers = ["▶", "●", "*", "❯", "[x]"]

    for line in lines:
        stripped = line.strip()
        lower = stripped.lower()
        if target_token not in lower:
            continue

        # Check bracketed focus e.g. "[low]" or "[ultracode]"
        if f"[{target_token}]" in lower:
            return True

        # Check if target token is immediately preceded by a focus marker
        for marker in focus_markers:
            if f"{marker} {target_token}" in stripped or f"{marker}{target_token}" in stripped:
                return True

    return False


def _navigate_to_boundary(session: ClaudeSession, boundary: str, viewport: tuple) -> bool:
    """Presses the boundary-direction key repeatedly until the target option is
    structurally focused AND the structural fingerprint converges.

    Returns True if target boundary was successfully reached; False otherwise.
    """
    nav_key, max_presses = _BOUNDARY_NAV[boundary]
    prev_fp = None
    for _ in range(max_presses):
        lines = session.observe_terminal_state()
        if _is_at_semantic_boundary(lines, boundary):
            return True

        session.press_key(nav_key)
        time.sleep(0.3)
        post_lines = session.observe_terminal_state()
        fp, _, _ = StructuralStateAnalyzer.analyze(post_lines, viewport)

        if _is_at_semantic_boundary(post_lines, boundary):
            return True

        if prev_fp is not None and fp.state_id() == prev_fp.state_id():
            # Fingerprint converged; verify semantic focus one final time
            return _is_at_semantic_boundary(post_lines, boundary)
        prev_fp = fp

    final_lines = session.observe_terminal_state()
    return _is_at_semantic_boundary(final_lines, boundary)


class EffortSelectorExplorer:
    """Explores /effort slider state machine using isolated path-replayed sessions for every edge.

    transport_verified is injected at construction — it is NOT a module global.
    """

    def __init__(self, run_dir: Path, *, transport_verified: bool):
        self.run_dir = run_dir
        self.transport_verified = transport_verified
        self.results = []
        self.session_results: List[SessionResult] = []

    def explore_effort(self, viewport: tuple = (80, 24)) -> dict:
        print("=== Starting Path-Replayed /effort Interactive Selector Explorer ===")

        trial_records = []
        for trial in TRIALS:
            print(f"  Testing Edge: '{trial.description}'... ", end="", flush=True)

            session = ClaudeSession(
                self.run_dir,
                f"effort_trial_{trial.action}_{trial.navigate_to or 'default'}",
                viewport
            )
            try:
                if session.launch():
                    readiness = ReadinessStateMachine(session.driver)
                    ok, _, _ = readiness.evaluate_readiness(session.launch_record)
                    if ok:
                        # Open the /effort selector
                        session.type_slash_command_and_submit("/effort")
                        time.sleep(0.4)

                        # Navigate to required starting position
                        if trial.navigate_to is not None:
                            nav_ok = _navigate_to_boundary(session, trial.navigate_to, viewport)
                            if not nav_ok:
                                session.mark_failed()
                                trial_record = {
                                    "navigate_to": trial.navigate_to,
                                    "action": trial.action,
                                    "description": trial.description,
                                    "expectation_kind": trial.expectation.kind,
                                    "actual_state_changed": False,
                                    "evidence_classification": "UNAVAILABLE",
                                    "note": "boundary_precondition_not_reached"
                                }
                                trial_records.append(trial_record)
                                print("UNAVAILABLE (boundary_not_reached)")
                                continue

                        # Observe pre-state (at boundary / default position)
                        pre_lines = session.observe_terminal_state()
                        pre_fp, _, _ = StructuralStateAnalyzer.analyze(pre_lines, viewport)

                        # Perform the single trial action
                        session.press_key(trial.action)
                        time.sleep(0.4)

                        post_lines = session.observe_terminal_state()
                        post_fp, _, _ = StructuralStateAnalyzer.analyze(post_lines, viewport)

                        changed = (post_fp.state_id() != pre_fp.state_id())
                        if trial.expectation.kind == "UNCHANGED":
                            matches_exp = not changed
                        elif trial.expectation.kind == "CHANGED_TO":
                            matches_exp = changed and post_fp.screen_category == trial.expectation.expected_screen
                        else:
                            matches_exp = changed

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

                        trial_record = {
                            "navigate_to": trial.navigate_to,
                            "action": trial.action,
                            "description": trial.description,
                            "expectation_kind": trial.expectation.kind,
                            "actual_state_changed": changed,
                            "evidence_classification": evidence_str
                        }
                        trial_records.append(trial_record)
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

        effort_res = {
            "interactive_surface_detected": True,
            "boundary_trials": trial_records
        }

        out_json = self.run_dir / "effort_exploration_results.json"
        with open(out_json, "w") as f:
            json.dump(effort_res, f, indent=2)

        print(f"/effort Exploration Complete. Saved to {out_json}")
        return effort_res, self.session_results
