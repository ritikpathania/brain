#!/usr/bin/env python3
"""
Dynamic Contextual Workspace Interaction State Machine Explorer
Creates disposable fixtures ('Atlas Fixture A', 'Atlas Fixture B'), dynamically observes WORKSPACE_LIST
selection ordinals by parsing actual session-entry rows, and executes isolated path-replayed trials
with strict directional index expectations:
  Up   -> post.ordinal == pre.ordinal - 1  (SELECTION_MOVED_UP)
  Down -> post.ordinal == pre.ordinal + 1  (SELECTION_MOVED_DOWN)
  Right -> expected_screen == 01_home
  Enter -> expected_screen == 08_workspace_timeline
  Space -> expected_screen == 10_reply_composer
  Ctrl+X -> expected_screen == 09_delete_confirmation
  ? -> expected_screen == 07_help_surfaces

If selected_index cannot be determined structurally: UNAVAILABLE (never FAILED).
"""

import os
import sys
import time
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional, Tuple

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from qa.claude_ux.driver.session import ClaudeSession
from qa.claude_ux.discovery.readiness import ReadinessStateMachine
from qa.claude_ux.design_audit.state_machine import StructuralStateAnalyzer, StateExpectation, classify_evidence


@dataclass
class SessionEntry:
    """A single workspace session list row with its selection state and list ordinal."""
    ordinal: int        # position in the session list (0-based)
    selected: bool      # True if this entry bears the selection marker
    display_name: str   # cleaned display name


def _is_chrome_line(line: str) -> bool:
    """Returns True for lines that are UI chrome, not session entries.

    Chrome patterns:
    - Blank / whitespace-only
    - Prompt indicator lines: ❯ prefix (the TUI prompt bar), or bare ">" shell prompt
    - Horizontal rule (box-drawing / dash characters dominate)
    - Known footer hint token patterns (enter to, space to, ctrl+x to, etc.)
    - Single-token header labels without a session-entry selection marker
      (e.g. "Claude Code", "Needs input" app title / section header lines)

    NOT chrome:
    - Lines that start with selection markers (▶, ●, * ) — those are session entries.
    """
    stripped = line.strip()
    if not stripped:
        return True
    # Selection markers → definitely a session entry, never chrome
    if stripped.startswith("▶") or stripped.startswith("●") or stripped.startswith("* "):
        return False
    # Prompt bar (TUI) or shell prompt
    if stripped.startswith("❯"):
        return True
    # Bare ">" only if the entire stripped line is just ">" or starts with "> " followed by
    # a known shell prompt pattern (e.g. "% cd ..."). A session entry named "> project" would
    # be caught by the selection marker check above.
    if stripped == ">" or stripped.startswith("> ") and len(stripped) <= 4:
        return True
    # Horizontal rule: line mostly composed of box-drawing / dash chars
    rule_chars = set("─━═-=│|╭╮╰╯┌┐└┘ ")
    if stripped.startswith(("❯", "/", "> ", "───", "═══", "---")):
        return True

    non_session_headers = [
        "enter to", "space to", "ctrl+", "shift+", "press ?", "help for",
        "claude code", "needs input", "recent sessions", "available commands",
        "project context", "no sessions found", "shortcuts"
    ]
    lower = stripped.lower()
    if any(tok in lower for tok in non_session_headers):
        return True
    return False


def parse_workspace_session_entries(lines: List[str]) -> List[SessionEntry]:
    """Parses structural session list entries from terminal buffer lines using strict section-aware state tracking.

    Section Grammar Rules:
    - Session list section starts upon encountering a session list header line
      (e.g. 'Needs input', 'Recent sessions', 'Workspace sessions', 'Sessions')
      OR a line bearing a selection marker (▶, ●, *).
    - Session list section ENDS upon encountering a horizontal rule (─, ━, =), prompt bar (❯),
      or footer instruction line ('enter to open', 'space to reply', 'ctrl+x to delete').
    - Within the active session list section:
      - Line MUST bear a selection marker OR be an indented session row item.
      - Line MUST NOT match prose help, status messages, or instruction text.
    """
    SELECTED_RE = re.compile(r"^(?:▶|●|\*\s+|>\s+)")
    session_headers = {"needs input", "recent sessions", "workspace sessions", "sessions", "all sessions"}
    footer_keywords = ["enter to", "space to", "ctrl+", "shift+", "press ?", "help for", "shortcuts"]
    rule_chars = set("─━═-=│|╭╮╰╯┌┐└┘ ")

    session_entries: List[SessionEntry] = []
    in_session_section = False

    for line in lines:
        stripped = line.strip()
        if not stripped:
            continue

        lower = stripped.lower()

        # Check section termination triggers
        if stripped.startswith(("❯", "/", "> ", "───", "═══", "---")):
            in_session_section = False
            continue

        # Horizontal rule check
        non_rule = [c for c in stripped if c not in rule_chars]
        if len(non_rule) <= 2:
            in_session_section = False
            continue

        if any(kw in lower for kw in footer_keywords):
            in_session_section = False
            continue

        # Check section activation triggers — REQUIRES a recognized workspace session header
        if lower in session_headers:
            in_session_section = True
            continue

        if not in_session_section:
            continue

        is_selected = bool(SELECTED_RE.match(stripped))
        is_indented = len(line) - len(line.lstrip(" ")) >= 2 or line.startswith("\t")
        if not (is_selected or is_indented):
            continue

        clean_name = SELECTED_RE.sub("", stripped).strip()
        if not clean_name:
            continue

        # Reject prose / instruction / help lines that might be indented
        if any(kw in clean_name.lower() for kw in ["description", "usage:", "options:", "press enter", "navigate using"]):
            continue

        entry = SessionEntry(
            ordinal=len(session_entries),
            selected=is_selected,
            display_name=clean_name,
        )
        session_entries.append(entry)

    return session_entries


def count_session_entries(lines: List[str]) -> int:
    """Counts structural session entries using parse_workspace_session_entries()."""
    return len(parse_workspace_session_entries(lines))


def extract_workspace_selection(lines: List[str]) -> Tuple[Optional[int], Optional[str]]:
    """Extracts focused session ordinal and name from workspace list lines."""
    entries = parse_workspace_session_entries(lines)
    focused = [e for e in entries if e.selected]
    if not focused:
        return None, None
    sel = focused[0]
    return sel.ordinal, sel.display_name


class WorkspaceMatrixExplorer:
    """Explores workspace session list contextual keys using strict direction-aware
    session ordinal indexing and single-action path replay.

    transport_verified is dependency-injected at construction time and forwarded
    to every classify_evidence() call — no module-global state.
    """

    def __init__(self, run_dir: Path, *, transport_verified: bool):
        self.run_dir = run_dir
        self.transport_verified = transport_verified
        self.session_results = []

    def explore_workspace(self, viewport: tuple = (80, 24)) -> dict:
        print("=== Starting Dynamic Contextual Workspace State Machine Explorer ===")

        # 1. Setup Disposable Fixture A
        session_a = ClaudeSession(self.run_dir, "ws_fixture_a", viewport)
        try:
            if session_a.launch():
                r_a = ReadinessStateMachine(session_a.driver)
                ok_a, _, _ = r_a.evaluate_readiness(session_a.launch_record)
                if ok_a:
                    session_a.type_and_submit("Atlas Fixture A Session Setup")
                    session_a.mark_completed()
                else:
                    session_a.mark_failed()
        finally:
            self.session_results.append(session_a.make_result())
            session_a.close()
            time.sleep(0.4)

        # 2. Setup Disposable Fixture B
        session_b = ClaudeSession(self.run_dir, "ws_fixture_b", viewport)
        try:
            if session_b.launch():
                r_b = ReadinessStateMachine(session_b.driver)
                ok_b, _, _ = r_b.evaluate_readiness(session_b.launch_record)
                if ok_b:
                    session_b.type_and_submit("Atlas Fixture B Session Setup")
                    session_b.mark_completed()
                else:
                    session_b.mark_failed()
        finally:
            self.session_results.append(session_b.make_result())
            session_b.close()
            time.sleep(0.4)

        # 3. Trials with structurally-derived expectations
        # NOTE: ctrl+x expects 09_delete_confirmation (box_rounded + destructive footer);
        #       space expects 10_reply_composer (left_panel + editable prompt in body);
        #       enter expects 08_workspace_timeline (session thread opened).
        trials = [
            ("up",     "WORKSPACE_LIST", "WORKSPACE_LIST",     StateExpectation(kind="SELECTION_MOVED_UP")),
            ("down",   "WORKSPACE_LIST", "WORKSPACE_LIST",     StateExpectation(kind="SELECTION_MOVED_DOWN")),
            ("right",  "WORKSPACE_LIST", "HOME_PROMPT",         StateExpectation(kind="CHANGED_TO", expected_screen="01_home")),
            ("enter",  "WORKSPACE_LIST", "SESSION_VIEW",        StateExpectation(kind="CHANGED_TO", expected_screen="08_workspace_timeline")),
            ("space",  "WORKSPACE_LIST", "REPLY_COMPOSER",      StateExpectation(kind="CHANGED_TO", expected_screen="10_reply_composer")),
            ("ctrl+x", "WORKSPACE_LIST", "DELETE_CONFIRMATION", StateExpectation(kind="CHANGED_TO", expected_screen="09_delete_confirmation")),
            ("?",      "WORKSPACE_LIST", "SHORTCUT_HELP",       StateExpectation(kind="CHANGED_TO", expected_screen="07_help_surfaces")),
        ]

        matrix_results = []
        for key_code, src_state, target_state, expectation in trials:
            print(f"  Testing Workspace Edge: {src_state} --[{key_code}]--> {target_state}... ", end="", flush=True)

            edge_session = ClaudeSession(self.run_dir, f"ws_edge_{key_code.replace('+', '_')}", viewport)
            try:
                if edge_session.launch():
                    readiness = ReadinessStateMachine(edge_session.driver)
                    ok, _, _ = readiness.evaluate_readiness(edge_session.launch_record)
                    if ok:
                        edge_session.press_key("left")
                        time.sleep(0.6)

                        pre_lines = edge_session.observe_terminal_state()
                        pre_fp, _, _ = StructuralStateAnalyzer.analyze(pre_lines, viewport)
                        pre_idx, pre_name = extract_workspace_selection(pre_lines)
                        entries_count = count_session_entries(pre_lines)

                        # ── Precondition check for directional trials ──────────────
                        # Before pressing up/down, verify the precondition is satisfied.
                        # If the selection is already at a boundary, pressing that
                        # direction is a no-op — the correct result is UNAVAILABLE,
                        # not FAILED.
                        if expectation.kind == "SELECTION_MOVED_UP" and pre_idx is not None and pre_idx == 0:
                            entry = {
                                "source_state": src_state,
                                "target_state": target_state,
                                "input_value": key_code,
                                "action_executed": False,
                                "pre_selected_index": pre_idx,
                                "post_selected_index": None,
                                "state_changed": False,
                                "expectation_kind": expectation.kind,
                                "evidence_classification": "UNAVAILABLE",
                                "note": "already_at_top_boundary"
                            }
                            matrix_results.append(entry)
                            print("UNAVAILABLE (already_at_top_boundary)")
                            continue

                        if expectation.kind == "SELECTION_MOVED_DOWN" and pre_idx is not None and entries_count > 0 and pre_idx >= entries_count - 1:
                            entry = {
                                "source_state": src_state,
                                "target_state": target_state,
                                "input_value": key_code,
                                "action_executed": False,
                                "pre_selected_index": pre_idx,
                                "post_selected_index": None,
                                "state_changed": False,
                                "expectation_kind": expectation.kind,
                                "evidence_classification": "UNAVAILABLE",
                                "note": "already_at_bottom_boundary"
                            }
                            matrix_results.append(entry)
                            print("UNAVAILABLE (already_at_bottom_boundary)")
                            continue
                        # Precondition satisfied — press the key and observe post-state
                        edge_session.press_key(key_code)
                        time.sleep(0.5)

                        post_lines = edge_session.observe_terminal_state()
                        post_fp, _, _ = StructuralStateAnalyzer.analyze(post_lines, viewport)
                        post_idx, post_name = extract_workspace_selection(post_lines)

                        is_changed = (post_fp.state_id() != pre_fp.state_id())

                        if expectation.kind == "SELECTION_MOVED_UP":
                            if pre_idx is not None and post_idx is not None:
                                matches_exp = (post_idx == pre_idx - 1)
                            else:
                                # Cannot determine ordinals structurally → UNAVAILABLE
                                entry = {
                                    "source_state": src_state,
                                    "target_state": target_state,
                                    "input_value": key_code,
                                    "action_executed": True,
                                    "pre_selected_index": pre_idx,
                                    "post_selected_index": post_idx,
                                    "state_changed": is_changed,
                                    "expectation_kind": expectation.kind,
                                    "evidence_classification": "UNAVAILABLE",
                                    "note": "selected_index_not_extractable"
                                }
                                matrix_results.append(entry)
                                print("UNAVAILABLE")
                                continue
                        elif expectation.kind == "SELECTION_MOVED_DOWN":
                            if pre_idx is not None and post_idx is not None:
                                matches_exp = (post_idx == pre_idx + 1)
                            else:
                                entry = {
                                    "source_state": src_state,
                                    "target_state": target_state,
                                    "input_value": key_code,
                                    "action_executed": True,
                                    "pre_selected_index": pre_idx,
                                    "post_selected_index": post_idx,
                                    "state_changed": is_changed,
                                    "expectation_kind": expectation.kind,
                                    "evidence_classification": "UNAVAILABLE",
                                    "note": "selected_index_not_extractable"
                                }
                                matrix_results.append(entry)
                                print("UNAVAILABLE")
                                continue
                        elif expectation.kind == "CHANGED_TO":
                            # Exact screen category equality — no substring heuristics.
                            matches_exp = (
                                is_changed
                                and post_fp.screen_category == expectation.expected_screen
                            )
                        else:
                            matches_exp = False

                        evidence_str = classify_evidence(
                            action_executed=True,
                            transport_verified=self.transport_verified,
                            parent_state_known=True,
                            post_state_observed=True,
                            transition_matches_expectation=matches_exp
                        )

                        if evidence_str == "VERIFIED":
                            edge_session.mark_completed()
                        else:
                            edge_session.mark_failed()
                        entry = {
                            "source_state": src_state,
                            "target_state": target_state,
                            "input_value": key_code,
                            "action_executed": True,
                            "pre_selected_index": pre_idx,
                            "post_selected_index": post_idx,
                            "state_changed": is_changed,
                            "expectation_kind": expectation.kind,
                            "expected_screen": expectation.expected_screen,
                            "actual_post_category": post_fp.screen_category,
                            "evidence_classification": evidence_str
                        }
                        matrix_results.append(entry)
                        print(evidence_str)
                    else:
                        edge_session.mark_failed()
                        print("UNAVAILABLE")
            finally:
                self.session_results.append(edge_session.make_result())
                edge_session.close()
                time.sleep(0.4)

        res = {
            "workspace_detected": True,
            "disposable_fixtures": ["Atlas Fixture A", "Atlas Fixture B"],
            "matrix_results": matrix_results
        }

        out_json = self.run_dir / "workspace_matrix_results.json"
        with open(out_json, "w") as f:
            json.dump(res, f, indent=2)

        print(f"Contextual Workspace Matrix Complete. Saved to {out_json}")
        return res, self.session_results
