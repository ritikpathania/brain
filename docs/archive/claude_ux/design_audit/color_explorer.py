#!/usr/bin/env python3
"""
Dynamic /color Options & Persistence Lifecycle Matrix Explorer
Discovers color values dynamically from '/color ' completion (starts from empty list — no hardcoded seeds).
Evaluates each color across 3 INDEPENDENT causal stages:
  Stage 1: apply_verified          — color in prompt text after /color <color> enter
  Stage 2: subsequent_cmd_verified — color still in prompt after a subsequent command
  Stage 3: resume_verified         — color visible after exit + claude --resume <id>

No len() fallbacks. No stage evidence inherited from another stage. Zero hardcoded colors.
transport_verified is dependency-injected at construction time.
"""

import os
import sys
import time
import json
import re
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from qa.claude_ux.driver.session import ClaudeSession, SessionResult
from qa.claude_ux.discovery.readiness import ReadinessStateMachine
from qa.claude_ux.design_audit.state_machine import classify_evidence


def color_apply_matches(color: str, lines: list) -> bool:
    """Production predicate for Stage 1 and Stage 2.

    The ONLY valid evidence is that the color name appears in the observed
    terminal text. Line-count is not evidence. This function is kept small
    and pure so the regression tests can import and test it directly.
    """
    return color in "\n".join(lines)


def _parse_completion_rows(lines: list) -> list:
    """Parse actual completion popup rows to extract advertised option tokens.

    Each non-chrome, non-prompt line in the completion popup represents one
    option. Options are single alpha tokens (no spaces in color names).
    This function performs ZERO vocabulary matching against a fixed list —
    if Claude adds a new color option it will be discovered automatically.

    Chrome lines filtered out:
    - Lines starting with ❯, /, > (prompt / command prefix)
    - Lines containing horizontal rule characters (─, ─, ━, =, -)
    - Blank lines
    - Lines containing footer hint tokens ("enter to", "esc to", "·", etc.)
    """
    options = []
    rule_chars = set("─—━═┈╌=-")
    footer_keywords = ["enter to", "esc to", "tab to", "to select", "to cancel", "·", "shortcuts"]
    for line in lines:
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith(("❯", "/", ">")):
            continue
        lower = stripped.lower()
        if any(kw in lower for kw in footer_keywords):
            continue
        # Horizontal rule guard: majority rule chars
        non_rule = [c for c in stripped if c not in rule_chars]
        if len(non_rule) <= 2:
            continue
        parts = stripped.split()
        if not parts:
            continue
        token = parts[0]
        if token.isalpha() and 2 <= len(token) <= 12:
            options.append(token.lower())
    return options


class ColorOptionsExplorer:
    """Discovers /color options dynamically and tests persistence across independent causal stages.

    transport_verified is injected at construction — it is NOT a module global.
    """

    def __init__(self, run_dir: Path, *, transport_verified: bool):
        self.run_dir = run_dir
        self.transport_verified = transport_verified

    def _discover_colors(self, session: ClaudeSession) -> list:
        """Discover /color options by parsing the actual completion popup rows.

        Zero vocabulary matching. The completion popup rows are parsed
        structurally via _parse_completion_rows(). If Claude adds a new
        color option, this will find it automatically.
        """
        session.type("/color ")
        time.sleep(0.5)

        lines = session.observe_terminal_state()

        # Dismiss completion popup before continuing
        session.press_key("esc")
        time.sleep(0.3)

        discovered = _parse_completion_rows(lines)
        return discovered

    def explore_color(self, viewport: tuple = (80, 24)) -> dict:
        print("=== Starting Dynamic /color Options & Lifecycle Matrix Explorer ===")
        self.session_results = []

        # ── Discovery Stage Session ──────────────────────────────────────────
        disc_session = ClaudeSession(self.run_dir, "color_discovery", viewport)
        discovered_colors = []
        try:
            if disc_session.launch():
                readiness = ReadinessStateMachine(disc_session.driver)
                ok, _, _ = readiness.evaluate_readiness(disc_session.launch_record)
                if ok:
                    discovered_colors = self._discover_colors(disc_session)
                    print(f"  Discovered /color options dynamically: {discovered_colors}")
                    disc_session.mark_completed()
                else:
                    disc_session.mark_failed()
            else:
                disc_session.mark_failed()
        finally:
            self.session_results.append(disc_session.make_result())
            disc_session.close()
            time.sleep(0.4)

        if not discovered_colors:
            return {"discovered_colors": [], "lifecycle_matrix": []}, self.session_results

        lifecycle_matrix = []
        for color in discovered_colors:
            # ── Stage 1 & 2: Apply & Subsequent Command Session ─────────────
            apply_session = ClaudeSession(self.run_dir, f"color_apply_{color}", viewport)
            apply_ev = "UNAVAILABLE"
            subseq_ev = "UNAVAILABLE"
            resume_session_id = None

            try:
                if apply_session.launch():
                    readiness = ReadinessStateMachine(apply_session.driver)
                    ok, _, _ = readiness.evaluate_readiness(apply_session.launch_record)
                    if ok:
                        # Stage 1: Apply color
                        apply_session.press_key("esc")
                        time.sleep(0.2)
                        apply_session.type_slash_command_and_submit(f"/color {color}")
                        time.sleep(0.4)

                        lines_s1 = apply_session.observe_terminal_state()
                        apply_ok = color_apply_matches(color, lines_s1)
                        apply_ev = classify_evidence(
                            action_executed=True,
                            transport_verified=self.transport_verified,
                            parent_state_known=True,
                            post_state_observed=True,
                            transition_matches_expectation=apply_ok
                        )

                        # Stage 2: Subsequent Command
                        apply_session.type_slash_command_and_submit("/context")
                        time.sleep(0.4)

                        lines_s2 = apply_session.observe_terminal_state()
                        subseq_ok = color_apply_matches(color, lines_s2)
                        subseq_ev = classify_evidence(
                            action_executed=True,
                            transport_verified=self.transport_verified,
                            parent_state_known=True,
                            post_state_observed=True,
                            transition_matches_expectation=subseq_ok
                        )

                        if apply_ev == "VERIFIED" and subseq_ev == "VERIFIED":
                            apply_session.mark_completed()
                        else:
                            apply_session.mark_failed()

                        # Exit apply session to get resume command
                        apply_session.press_key("esc")
                        time.sleep(0.2)
                        apply_session.type("/quit")
                        apply_session.press_key("enter")
                        time.sleep(1.0)

                        exit_lines = apply_session.observe_terminal_state()
                        exit_text = " ".join(exit_lines)
                        resume_match = re.search(r"claude\s+--resume\s+([a-f0-9\-]{8,})", exit_text, re.IGNORECASE)
                        if resume_match:
                            resume_session_id = resume_match.group(1)
                    else:
                        apply_session.mark_failed()
                else:
                    apply_session.mark_failed()
            finally:
                self.session_results.append(apply_session.make_result())
                apply_session.close()
                time.sleep(0.4)

            # ── Stage 3: Independent Resume Session ────────────────────────
            resume_ev = "UNAVAILABLE"
            if resume_session_id:
                resume_session = ClaudeSession(self.run_dir, f"color_resume_{color}", viewport)
                try:
                    if resume_session.launch(f"claude --resume {resume_session_id}"):
                        readiness = ReadinessStateMachine(resume_session.driver)
                        ok_r, _, _ = readiness.evaluate_readiness(resume_session.launch_record)
                        if ok_r:
                            time.sleep(1.2)
                            lines_s3 = resume_session.observe_terminal_state()
                            resume_ok = color_apply_matches(color, lines_s3)
                            resume_ev = classify_evidence(
                                action_executed=True,
                                transport_verified=self.transport_verified,
                                parent_state_known=True,
                                post_state_observed=True,
                                transition_matches_expectation=resume_ok
                            )
                            if resume_ev == "VERIFIED":
                                resume_session.mark_completed()
                            else:
                                resume_session.mark_failed()
                        else:
                            resume_session.mark_failed()
                    else:
                        resume_session.mark_failed()
                finally:
                    self.session_results.append(resume_session.make_result())
                    resume_session.close()
                    time.sleep(0.4)

            entry = {
                "color": color,
                "checkpoints": {
                    "apply_verified": apply_ev,
                    "subsequent_command_persistence_verified": subseq_ev,
                    "resume_persistence_verified": resume_ev
                },
                "evidence_classification": apply_ev
            }
            lifecycle_matrix.append(entry)
            print(f"  Color '{color}': Apply={apply_ev}, Subsequent={subseq_ev}, Resume={resume_ev}")

        res = {
            "discovered_colors": discovered_colors,
            "lifecycle_matrix": lifecycle_matrix
        }

        out_json = self.run_dir / "color_exploration_results.json"
        with open(out_json, "w") as f:
            json.dump(res, f, indent=2)

        print(f"/color Exploration Complete. Saved to {out_json}")
        return res, self.session_results
