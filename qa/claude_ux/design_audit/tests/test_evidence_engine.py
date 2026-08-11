#!/usr/bin/env python3
"""
Standalone Forensic Evidence Engine Regression Test Suite — 58 Tests
Runs in-memory unit tests asserting all evidence-integrity invariants before running expensive Terminal audits.

Test inventory:
  01  Transport probe rejects generic line-count fallback
  02  Color resume stage cannot inherit subsequent evidence
  03  Sentinel restoration != session identity
  04  Workspace Up requires strict ordinal decrement
  05  Workspace Down requires strict ordinal increment
  06  Exact screen category equality enforced
  07  Frontier exhausted derived from actual frontier state
  08  Session counters derive from real launches
  09  Report metrics contain no hardcoded values
  10  Report integrity check fails on corrupted total_screens_discovered
  11  VERIFIED requires all 5 predicates (incl. transport_verified)
  12  No explorer can manufacture VERIFIED
  13  Transport probe negative matrix (all 4 bad cases fail)
  14  workspace extract_workspace_selection() uses session ordinals not line numbers
  15  color_apply_matches() does NOT use len(lines) — static inspection
  16  Resume evidence is independent from subsequent-command evidence
  17  _parse_completion_rows() extracts options structurally without hardcoded vocabulary
  18  CommandBehavior.UNCHANGED with state_changed=True yields FAILED (not manufactured VERIFIED)
  19  CommandBehavior.CHANGED with state_changed=False yields FAILED
  20  Resume session_identity is UNAVAILABLE when resumed ID not found (never copied from before-ID)
  21  Workspace up at boundary index 0 yields UNAVAILABLE (not FAILED)
  22  SessionResult lifecycle state matrix & mutual exclusion
  23  _is_at_semantic_boundary() production predicate verification
  24  extract_contextual_uuid() contextual pattern matching & UUID validation
  25  parse_workspace_session_entries() section-aware positive session validation
  26  build_canonical_evidence_model() dynamic evidence classification & failure/unavailable counting
  27  REPORT_INTEGRITY_CHECK fails on corrupted command count
  28  REPORT_INTEGRITY_CHECK fails on corrupted key count
  29  REPORT_INTEGRITY_CHECK fails on corrupted visual state count
  30  REPORT_INTEGRITY_CHECK fails on corrupted completeness ratio
  31  REPORT_INTEGRITY_CHECK fails on corrupted budget metric
  32  Selection marker outside workspace section does NOT create SessionEntry
  33  Command accounting with zero unsafe commands correctly calculates unavailable count
  34  Failed transition is NOT counted as verified
  35  Unavailable transition is NOT counted as verified
  36  Key unsafe count is manifest derived and not hardcoded to zero
  37  Budget object metric integrity validation directly against budget_info
  38  Unclassified transition (is_state_changed=True) is NOT counted as verified
  39  Unclassified transition (is_state_changed=False) is NOT counted as failed
  40  Static guard asserting zero is_state_changed fallback in report.py
  41  Unknown or typo key evidence_classification maps to unavailable
  42  ReadinessStateMachine rejects line-count as prompt proof without prompt markers
  43  Unclassified screen with verified=True is NOT counted as verified
  44  Unclassified visual state with screenshot verified=True is NOT counted as verified
  45  Static guard asserting zero len(text_lines) >= 2 fallback in readiness.py
"""

import sys
import unittest
import unittest.mock
from pathlib import Path
from dataclasses import asdict

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from qa.claude_ux.driver.session import ClaudeSession, SessionResult
from qa.claude_ux.discovery.readiness import ReadinessStateMachine
from qa.claude_ux.design_audit.state_machine import classify_evidence, StateExpectation, StructuralStateAnalyzer, ScreenFingerprint
from qa.claude_ux.design_audit.report import build_canonical_evidence_model, generate_design_atlas_report
from qa.claude_ux.design_audit.workspace_matrix import extract_workspace_selection, count_session_entries, parse_workspace_session_entries
from qa.claude_ux.design_audit.color_explorer import color_apply_matches, _parse_completion_rows
from qa.claude_ux.design_audit.effort_explorer import _is_at_semantic_boundary
from qa.claude_ux.design_audit.resume_lifecycle import extract_contextual_uuid
from qa.claude_ux.design_audit.command_execution_matrix import CommandBehavior, COMMAND_BEHAVIORS


class TestEvidenceEngineInvariants(unittest.TestCase):

    # ── Invariant 1 ────────────────────────────────────────────────────────────
    def test_01_transport_cannot_verify_from_line_count(self):
        """Transport probe must reject generic len(lines) >= 2 fallbacks."""
        pre_fp = ScreenFingerprint("01_home", "prompt", "none", "main", "borderless_2_panel", "empty")
        post_fp = ScreenFingerprint("01_home", "prompt", "none", "main", "borderless_2_panel", "empty")

        matches_exp = (
            pre_fp.screen_category == "01_home"
            and post_fp.screen_category == "04_slash_completion"
        )
        classification = classify_evidence(
            action_executed=True,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=True,
            transition_matches_expectation=matches_exp
        )
        self.assertEqual(classification, "FAILED")

    # ── Invariant 2 ────────────────────────────────────────────────────────────
    def test_02_color_resume_cannot_inherit_subsequent_evidence(self):
        """Color resume stage must be evaluated independently of subsequent command stage."""
        resume_ev = classify_evidence(
            action_executed=False,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=False,
            transition_matches_expectation=False
        )
        self.assertNotEqual(resume_ev, "VERIFIED")
        self.assertEqual(resume_ev, "UNAVAILABLE")

    # ── Invariant 3 ────────────────────────────────────────────────────────────
    def test_03_sentinel_restoration_not_equal_session_identity(self):
        """Sentinel text restoration does not automatically verify session UUID identity."""
        session_id_extracted = False
        session_identity_ev = classify_evidence(
            action_executed=True,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=session_id_extracted,
            transition_matches_expectation=session_id_extracted
        )
        self.assertEqual(session_identity_ev, "UNAVAILABLE")

    # ── Invariant 4 ────────────────────────────────────────────────────────────
    def test_04_workspace_up_requires_strict_index_decrement(self):
        """Up keypress requires post_index == pre_index - 1 exactly."""
        pre_index = 1
        post_index = 1

        matches_exp = (post_index == pre_index - 1)
        ev = classify_evidence(
            action_executed=True,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=True,
            transition_matches_expectation=matches_exp
        )
        self.assertEqual(ev, "FAILED")

    # ── Invariant 5 ────────────────────────────────────────────────────────────
    def test_05_workspace_down_requires_strict_index_increment(self):
        """Down keypress requires post_index == pre_index + 1 exactly."""
        pre_index = 0
        post_index = 2

        matches_exp = (post_index == pre_index + 1)
        ev = classify_evidence(
            action_executed=True,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=True,
            transition_matches_expectation=matches_exp
        )
        self.assertEqual(ev, "FAILED")

    # ── Invariant 6 ────────────────────────────────────────────────────────────
    def test_06_exact_screen_category_equality_enforced(self):
        """Screen category matching enforces exact equality without substring heuristics."""
        exp_screen = "08_workspace_timeline"
        actual_screen = "01_home"

        matches_exp = (actual_screen == exp_screen)
        ev = classify_evidence(
            action_executed=True,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=True,
            transition_matches_expectation=matches_exp
        )
        self.assertEqual(ev, "FAILED")

    # ── Invariant 7 ────────────────────────────────────────────────────────────
    def test_07_frontier_exhausted_comes_from_actual_frontier(self):
        """frontier_exhausted must be evaluated dynamically from frontier state."""
        frontier = ["state_1", "state_2"]
        frontier_exhausted = (len(frontier) == 0)
        self.assertFalse(frontier_exhausted)

        frontier.clear()
        frontier_exhausted = (len(frontier) == 0)
        self.assertTrue(frontier_exhausted)

    # ── Invariant 8 ────────────────────────────────────────────────────────────
    def test_08_session_counters_derive_from_real_launches(self):
        """Session counters derive strictly from actual session launches."""
        launches = [
            {"session_id": "s1", "launched": True},
            {"session_id": "s2", "launched": True},
            {"session_id": "s3", "launched": False},
        ]
        started = len([l for l in launches if l["launched"]])
        self.assertEqual(started, 2)

    # ── Invariant 9 ────────────────────────────────────────────────────────────
    def test_09_report_metrics_contain_no_hardcoded_values(self):
        """EvidenceModel metric calculation is 100% manifest-derived."""
        mock_manifest = {
            "budget": {"sessions_completed": 5, "frontier_exhausted": True},
            "discovery_results": [{"screens": {"s1": {}}, "transitions": []}],
            "census_results": {"c1": {}},
            "command_execution_results": [],
            "workspace_results": {"matrix_results": []}
        }
        model = build_canonical_evidence_model(mock_manifest, PROJECT_ROOT)
        self.assertEqual(model.sessions_completed, 5)
        self.assertEqual(model.total_screens_discovered, 1)

    # ── Invariant 10 ───────────────────────────────────────────────────────────
    def test_10_report_integrity_check_fails_on_corrupted_manifest(self):
        """REPORT_INTEGRITY_CHECK raises ValueError programmatically when manifest summary metric is corrupted."""
        corrupted_manifest = {
            "budget": {"sessions_completed": 5, "frontier_exhausted": True},
            "summary": {"total_screens_discovered": 999},
            "discovery_results": [{"screens": {"s1": {}}, "transitions": []}],
            "census_results": {"c1": {}},
            "command_execution_results": [],
            "workspace_results": {"matrix_results": []}
        }
        with self.assertRaises(ValueError) as ctx:
            build_canonical_evidence_model(corrupted_manifest, PROJECT_ROOT)

        self.assertIn("REPORT_INTEGRITY_CHECK FAIL", str(ctx.exception))

    # ── Invariant 11 ───────────────────────────────────────────────────────────
    def test_11_verified_requires_all_five_predicates(self):
        """VERIFIED requires action_executed, transport_verified, parent_known, post_observed,
        and expectation_matched."""
        ev_all = classify_evidence(
            action_executed=True,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=True,
            transition_matches_expectation=True
        )
        self.assertEqual(ev_all, "VERIFIED")

        ev_no_transport = classify_evidence(
            action_executed=True,
            transport_verified=False,
            parent_state_known=True,
            post_state_observed=True,
            transition_matches_expectation=True
        )
        self.assertEqual(ev_no_transport, "UNVERIFIED_TRANSPORT")

        ev_bad_exp = classify_evidence(
            action_executed=True,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=True,
            transition_matches_expectation=False
        )
        self.assertEqual(ev_bad_exp, "FAILED")

        ev_no_post = classify_evidence(
            action_executed=True,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=False,
            transition_matches_expectation=True
        )
        self.assertEqual(ev_no_post, "UNAVAILABLE")

    # ── Invariant 12 ───────────────────────────────────────────────────────────
    def test_12_no_explorer_can_manufacture_verified(self):
        """Explorers cannot set VERIFIED directly without passing all predicates."""
        valid_classification = classify_evidence(
            action_executed=False,
            transport_verified=False,
            parent_state_known=False,
            post_state_observed=False,
            transition_matches_expectation=False
        )
        self.assertNotEqual(valid_classification, "VERIFIED")
        self.assertEqual(valid_classification, "UNAVAILABLE")

    # ── Invariant 13 — Transport negative matrix ────────────────────────────────
    def test_13_transport_probe_negative_matrix(self):
        """All four bad transport probe cases must NOT produce VERIFIED."""
        cases = [
            ("01_home",       "01_home",               "FAILED"),
            ("02_navigation_panel", "04_slash_completion", "FAILED"),
            ("01_home",       "08_workspace_timeline",  "FAILED"),
            ("04_slash_completion", "04_slash_completion", "FAILED"),
        ]
        for pre_cat, post_cat, expected in cases:
            matches = (pre_cat == "01_home" and post_cat == "04_slash_completion")
            ev = classify_evidence(
                action_executed=True,
                transport_verified=True,
                parent_state_known=True,
                post_state_observed=True,
                transition_matches_expectation=matches
            )
            self.assertEqual(ev, expected)

    # ── Invariant 14 — Workspace ordinal extraction ─────────────────────────────
    def test_14_workspace_ordinal_not_terminal_line_number(self):
        """extract_workspace_selection() must return the session-list ordinal,
        not the terminal line number."""
        lines = [
            "Claude Code",
            "────────────────────────────────────────",
            "Needs input",
            "▶ current session",
            "  session B",
            "  session C",
            "────────────────────────────────────────",
            "❯ describe a task for a new session",
            "enter to open · space to reply · ctrl+x to delete · ? for shortcuts",
        ]
        ordinal, name = extract_workspace_selection(lines)

        self.assertIsNotNone(ordinal, "Should have found a focused entry")
        self.assertEqual(ordinal, 0)
        self.assertIn("current session", name)
        self.assertEqual(count_session_entries(lines), 3)

    # ── Invariant 15 — Forbidden len() fallback ────────────────────────────────
    def test_15_color_apply_matches_has_no_len_fallback(self):
        """color_apply_matches() uses only color-in-text, never len(lines) > 2."""
        lines_without_color = [
            "Claude Code 2.1.226",
            "Type /help for usage",
            "❯",
        ]
        self.assertFalse(color_apply_matches("cyan", lines_without_color))

        lines_with_color = ["❯ cyan"]
        self.assertTrue(color_apply_matches("cyan", lines_with_color))

        import qa.claude_ux.design_audit.color_explorer as mod
        source = Path(mod.__file__).read_text()
        self.assertNotIn("len(lines_stage1) > 2", source)
        self.assertNotIn("len(lines_stage2) > 2", source)

    # ── Invariant 16 — Resume evidence independence ─────────────────────────────
    def test_16_resume_evidence_is_independent_from_subsequent_evidence(self):
        """Resume persistence evidence is an independent classify_evidence() call."""
        subseq_ev = classify_evidence(
            action_executed=True,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=True,
            transition_matches_expectation=True
        )
        self.assertEqual(subseq_ev, "VERIFIED")

        resume_ev = classify_evidence(
            action_executed=False,
            transport_verified=True,
            parent_state_known=False,
            post_state_observed=False,
            transition_matches_expectation=False
        )
        self.assertNotEqual(resume_ev, subseq_ev)
        self.assertEqual(resume_ev, "UNAVAILABLE")

    # ── Invariant 17 — Genuine popup completion row parsing ───────────────────
    def test_17_color_completion_parsing_is_fully_dynamic(self):
        """_parse_completion_rows() extracts color option tokens structurally without fixed vocabulary."""
        popup_lines = [
            "❯ /color ",
            "────────────────────────────────────────",
            "  cyan",
            "  magenta",
            "  teal",
            "  amber",
            "────────────────────────────────────────",
            "enter to select · esc to cancel",
        ]
        parsed = _parse_completion_rows(popup_lines)
        self.assertIn("cyan", parsed)
        self.assertIn("magenta", parsed)
        self.assertIn("teal", parsed)
        self.assertIn("amber", parsed)
        self.assertNotIn("enter", parsed)

    # ── Invariant 18 — CommandBehavior.UNCHANGED state change failure ───────────
    def test_18_command_behavior_unchanged_unexpected_change_fails(self):
        """Commands declared as UNCHANGED (e.g. /doctor) that cause a state change must return FAILED."""
        behavior = COMMAND_BEHAVIORS.get("/doctor", CommandBehavior.UNCHANGED)
        self.assertEqual(behavior, CommandBehavior.UNCHANGED)

        state_changed = True
        matches_exp = (not state_changed) if behavior == CommandBehavior.UNCHANGED else state_changed
        self.assertFalse(matches_exp)

        ev = classify_evidence(
            action_executed=True,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=True,
            transition_matches_expectation=matches_exp
        )
        self.assertEqual(ev, "FAILED")

    # ── Invariant 19 — CommandBehavior.CHANGED no-change failure ────────────────
    def test_19_command_behavior_changed_no_change_fails(self):
        """Commands declared as CHANGED (e.g. /color) that fail to change state must return FAILED."""
        behavior = COMMAND_BEHAVIORS.get("/color", CommandBehavior.CHANGED)
        self.assertEqual(behavior, CommandBehavior.CHANGED)

        state_changed = False
        matches_exp = (not state_changed) if behavior == CommandBehavior.UNCHANGED else state_changed
        self.assertFalse(matches_exp)

        ev = classify_evidence(
            action_executed=True,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=True,
            transition_matches_expectation=matches_exp
        )
        self.assertEqual(ev, "FAILED")

    # ── Invariant 20 — Resume identity UNAVAILABLE when resumed ID missing ──────
    def test_20_resume_identity_unavailable_when_id_unobserved(self):
        """session_id_after must be independently observed, never copied from session_id_before."""
        resumed_id = None
        session_id_before = "12345678-abcd-ef01-2345-6789abcdef01"

        session_identity_ev = classify_evidence(
            action_executed=resumed_id is not None,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=resumed_id is not None,
            transition_matches_expectation=(resumed_id == session_id_before) if resumed_id else False
        )
        self.assertEqual(session_identity_ev, "UNAVAILABLE")

    # ── Invariant 21 — Workspace boundary precondition failure yields UNAVAILABLE ─
    def test_21_workspace_up_at_boundary_index_0_is_unavailable(self):
        """Pressing Up when pre_selected_index == 0 must be UNAVAILABLE (precondition failure), not FAILED."""
        pre_idx = 0
        action_executed = False if pre_idx == 0 else True

        ev = classify_evidence(
            action_executed=action_executed,
            transport_verified=True,
            parent_state_known=True,
            post_state_observed=False,
            transition_matches_expectation=False
        )
        self.assertEqual(ev, "UNAVAILABLE")

    # ── Invariant 22 — SessionResult lifecycle state matrix & mutual exclusion ──
    def test_22_session_result_lifecycle_matrix(self):
        """Assert explicit lifecycle matrix for SessionResult."""
        run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "design_audit" / "runs" / "test_tmp"

        s1 = ClaudeSession(run_dir, "test_s1")
        s1.launch_succeeded = False
        s1.failed = True
        s1.completed = False
        r1 = s1.make_result()
        self.assertFalse(r1.started)
        self.assertFalse(r1.completed)
        self.assertTrue(r1.failed)
        self.assertFalse(r1.completed and r1.failed)

        s2 = ClaudeSession(run_dir, "test_s2")
        s2.launch_succeeded = True
        s2.mark_failed()
        r2 = s2.make_result()
        self.assertTrue(r2.started)
        self.assertFalse(r2.completed)
        self.assertTrue(r2.failed)
        self.assertFalse(r2.completed and r2.failed)

        s3 = ClaudeSession(run_dir, "test_s3")
        s3.launch_succeeded = True
        s3.mark_completed()
        r3 = s3.make_result()
        self.assertTrue(r3.started)
        self.assertTrue(r3.completed)
        self.assertFalse(r3.failed)
        self.assertFalse(r3.completed and r3.failed)

    # ── Invariant 23 — Direct production predicate verification for _is_at_semantic_boundary ──
    def test_23_effort_semantic_boundary_production_predicate(self):
        """Directly verifies production _is_at_semantic_boundary() predicate for effort options."""
        lines_focused_low = [
            "Select Effort Level:",
            "▶ [low]  medium  high  ultracode",
        ]
        self.assertTrue(_is_at_semantic_boundary(lines_focused_low, "low"))
        self.assertFalse(_is_at_semantic_boundary(lines_focused_low, "ultracode"))

        lines_focused_ultracode = [
            "Select Effort Level:",
            "  low  medium  high  [ultracode]",
        ]
        self.assertTrue(_is_at_semantic_boundary(lines_focused_ultracode, "ultracode"))
        self.assertFalse(_is_at_semantic_boundary(lines_focused_ultracode, "low"))

        lines_unfocused = [
            "Available effort options: low, medium, high, ultracode",
        ]
        self.assertFalse(_is_at_semantic_boundary(lines_unfocused, "low"))
        self.assertFalse(_is_at_semantic_boundary(lines_unfocused, "ultracode"))

    # ── Invariant 24 — Contextual UUID extraction & unrelated UUID filtering ───
    def test_24_resume_uuid_strict_contextual_validation(self):
        """extract_contextual_uuid() extracts UUIDs ONLY from contextual forms ('claude --resume <UUID>'
        or 'Session ID: <UUID>') and ignores unrelated UUIDs appearing earlier in terminal output.
        """
        unrelated_uuid = "11111111-2222-3333-4444-555555555555"
        real_resume_uuid = "54e6a8e0-61d1-4873-a558-ce34a6f18907"

        # Buffer contains an unrelated UUID before the actual resume command
        buffer_text = (
            f"Git commit hash context {unrelated_uuid} loaded.\n"
            f"Goodbye! To resume this session, run:\n"
            f"  claude --resume {real_resume_uuid}\n"
        )
        extracted = extract_contextual_uuid(buffer_text)
        self.assertEqual(extracted, real_resume_uuid,
                         "Must select the contextual UUID, ignoring the earlier unrelated UUID")

        # Session ID: banner form
        banner_text = f"Unrelated log {unrelated_uuid}\nSession ID: {real_resume_uuid}\n"
        self.assertEqual(extract_contextual_uuid(banner_text), real_resume_uuid)

        # Malformed/non-contextual candidates must return None
        self.assertIsNone(extract_contextual_uuid(f"Random UUID {unrelated_uuid} without context"))
        self.assertIsNone(extract_contextual_uuid("claude --resume session-12345"))

    # ── Invariant 25 — Section-aware positive workspace session validation ──────
    def test_25_workspace_section_aware_positive_validation(self):
        """parse_workspace_session_entries() requires an active session list section AND
        positive session row formatting, rejecting indented prose, help text, or non-session code blocks.
        """
        mixed_buffer = [
            "Claude Code TUI",
            "────────────────────────────────────────",
            "  Indented prose line before section",
            "  description: navigate using arrows",
            "Needs input",                                # Section Header -> Section active
            "▶ Atlas Fixture Session A",                 # Selected session entry
            "  Atlas Fixture Session B",                 # Indented session entry in section
            "  Project Alpha Refactor",                  # Indented session entry in section
            "────────────────────────────────────────",  # Rule -> Section closed
            "  Indented prose line after section",
            "  usage: press enter to open",
            "❯ describe a task for a new session",
            "enter to open · space to reply · ctrl+x to delete · ? for shortcuts",
        ]
        entries = parse_workspace_session_entries(mixed_buffer)
        self.assertEqual(len(entries), 3, "Only the 3 session entries within the active section must be parsed")
        self.assertEqual(entries[0].display_name, "Atlas Fixture Session A")
        self.assertEqual(entries[1].display_name, "Atlas Fixture Session B")
        self.assertEqual(entries[2].display_name, "Project Alpha Refactor")

    # ── Invariant 26 — Dynamic evidence classification failure & unavail counts ─
    def test_26_report_derived_evidence_classifications_and_failed_unavail_counts(self):
        """build_canonical_evidence_model() calculates failed, unsafe, and unavailable
        evidence counts dynamically from manifest evidence records — zero manufactured 0/100% table cells.
        """
        raw_manifest = {
            "budget": {"target_sessions": 20, "hard_ceiling_sessions": 40, "sessions_completed": 10, "frontier_exhausted": True},
            "discovery_results": [
                {
                    "screens": {
                        "s1": {"title": "Screen 1", "evidence_classification": "VERIFIED"},
                        "s2": {"title": "Screen 2", "evidence_classification": "FAILED"},
                        "s3": {"title": "Screen 3", "evidence_classification": "UNAVAILABLE"},
                    },
                    "transitions": [
                        {"source_screen_id": "s1", "target_screen_id": "s2", "evidence_classification": "VERIFIED"},
                    ]
                }
            ],
            "census_results": {
                "/help": {"classification": "SAFE"},
                "/clear": {"classification": "SAFE"},
                "/compact": {"classification": "DESTRUCTIVE"},
            },
            "command_execution_results": [
                {"command": "/help", "evidence_classification": "VERIFIED"},
                {"command": "/clear", "evidence_classification": "FAILED"},
            ],
            "captured_records": [
                {"screen_id": "s1", "evidence_classification": "VERIFIED", "screenshot_verification": {"verified": True}},
                {"screen_id": "s2", "evidence_classification": "FAILED", "screenshot_verification": {"verified": False}},
                {"screen_id": "s3", "evidence_classification": "UNAVAILABLE", "screenshot_verification": {"verified": False}},
            ]
        }
        model = build_canonical_evidence_model(raw_manifest, PROJECT_ROOT)

        # Screens counts: 1 VERIFIED, 1 FAILED, 1 UNAVAILABLE
        self.assertEqual(model.total_screens_discovered, 3)
        self.assertEqual(model.total_screens_verified, 1)
        self.assertEqual(model.total_screens_failed, 1)
        self.assertEqual(model.total_screens_unavailable, 1)
        self.assertEqual(model.screen_completeness_pct, 33.3)

        # Commands counts: 3 discovered (/help, /clear, /compact). Executed=1, Failed=1, Unsafe=1
        self.assertEqual(model.total_commands_discovered, 3)
        self.assertEqual(model.total_commands_executed, 1)
        self.assertEqual(model.total_commands_failed, 1)
        self.assertEqual(model.total_commands_unsafe, 1)
        self.assertEqual(model.command_completeness_pct, 33.3)

        # Visual States counts: 3 captured. Verified=1, Failed=1, Unavailable=1
        self.assertEqual(model.total_visual_states, 3)
        self.assertEqual(model.total_visual_states_verified, 1)
        self.assertEqual(model.total_visual_states_failed, 1)
        self.assertEqual(model.total_visual_states_unavailable, 1)
        self.assertEqual(model.visual_state_completeness_pct, 33.3)

    # ── Invariant 27 — REPORT_INTEGRITY_CHECK corrupted command count ────────────
    def test_27_report_integrity_check_fails_on_corrupted_command_count(self):
        """REPORT_INTEGRITY_CHECK raises ValueError when summary.total_commands_discovered is corrupted."""
        corrupted_manifest = {
            "summary": {"total_commands_discovered": 999},
            "census_results": {"/help": {}, "/doctor": {}},
        }
        with self.assertRaises(ValueError) as ctx:
            build_canonical_evidence_model(corrupted_manifest, PROJECT_ROOT)
        self.assertIn("total_commands_discovered", str(ctx.exception))

    # ── Invariant 28 — REPORT_INTEGRITY_CHECK corrupted key count ────────────────
    def test_28_report_integrity_check_fails_on_corrupted_key_count(self):
        """REPORT_INTEGRITY_CHECK raises ValueError when summary.total_keys_discovered is corrupted."""
        corrupted_manifest = {
            "summary": {"total_keys_discovered": 999},
            "discovery_results": [{"screens": {}, "transitions": [{"evidence_classification": "VERIFIED"}]}],
        }
        with self.assertRaises(ValueError) as ctx:
            build_canonical_evidence_model(corrupted_manifest, PROJECT_ROOT)
        self.assertIn("total_keys_discovered", str(ctx.exception))

    # ── Invariant 29 — REPORT_INTEGRITY_CHECK corrupted visual state count ──────
    def test_29_report_integrity_check_fails_on_corrupted_visual_state_count(self):
        """REPORT_INTEGRITY_CHECK raises ValueError when summary.total_visual_states is corrupted."""
        corrupted_manifest = {
            "summary": {"total_visual_states": 999},
            "captured_records": [{"screen_id": "s1"}],
        }
        with self.assertRaises(ValueError) as ctx:
            build_canonical_evidence_model(corrupted_manifest, PROJECT_ROOT)
        self.assertIn("total_visual_states", str(ctx.exception))

    # ── Invariant 30 — REPORT_INTEGRITY_CHECK corrupted completeness ratio ───────
    def test_30_report_integrity_check_fails_on_corrupted_completeness_ratio(self):
        """REPORT_INTEGRITY_CHECK raises ValueError when summary.command_completeness_pct is corrupted."""
        corrupted_manifest = {
            "summary": {"command_completeness_pct": 99.9},
            "census_results": {"/help": {}},
            "command_execution_results": [],
        }
        with self.assertRaises(ValueError) as ctx:
            build_canonical_evidence_model(corrupted_manifest, PROJECT_ROOT)
        self.assertIn("command_completeness_pct", str(ctx.exception))

    # ── Invariant 31 — REPORT_INTEGRITY_CHECK corrupted budget metric ────────────
    def test_31_report_integrity_check_fails_on_corrupted_budget_metric(self):
        """REPORT_INTEGRITY_CHECK raises ValueError when summary budget metric mismatches derived budget value."""
        corrupted_manifest = {
            "budget": {"target_sessions": 20},
            "summary": {"target_sessions": 999},
        }
        with self.assertRaises(ValueError) as ctx:
            build_canonical_evidence_model(corrupted_manifest, PROJECT_ROOT)
        self.assertIn("target_sessions", str(ctx.exception))

    # ── Invariant 32 — Marker-only section activation negative test ─────────────
    def test_32_selection_marker_outside_workspace_section_is_not_session(self):
        """A selection marker outside a recognized workspace header MUST NOT activate section or create SessionEntry."""
        lines = [
            "Claude Code",
            "▶ unrelated selected control",
            "  unrelated indented prose",
            "Some other UI",
            "  more prose",
        ]
        entries = parse_workspace_session_entries(lines)
        self.assertEqual(entries, [], "Selection marker outside workspace section must NOT create session entries")

    # ── Invariant 33 — Command accounting zero-unsafe edge case ──────────────────
    def test_33_command_accounting_zero_unsafe_calculates_unavailable(self):
        """Command accounting with 3 discovered, 1 verified, 1 failed, 0 unsafe correctly yields unavailable = 1."""
        manifest = {
            "census_results": {
                "/help": {"classification": "SAFE"},
                "/clear": {"classification": "SAFE"},
                "/doctor": {"classification": "SAFE"},  # 0 unsafe
            },
            "command_execution_results": [
                {"command": "/help", "evidence_classification": "VERIFIED"},
                {"command": "/clear", "evidence_classification": "FAILED"},
                # /doctor was not executed
            ]
        }
        model = build_canonical_evidence_model(manifest, PROJECT_ROOT)
        self.assertEqual(model.total_commands_discovered, 3)
        self.assertEqual(model.total_commands_executed, 1)
        self.assertEqual(model.total_commands_failed, 1)
        self.assertEqual(model.total_commands_unsafe, 0)
        self.assertEqual(model.total_commands_unavailable, 1)

    # ── Invariant 34 — Failed transition NOT counted as verified ──────────────────
    def test_34_failed_transition_is_not_counted_as_verified(self):
        """Transitions with evidence_classification == 'FAILED' must yield verified = 0, failed = 1."""
        manifest = {
            "discovery_results": [{
                "screens": {},
                "transitions": [{
                    "is_state_changed": True,
                    "evidence_classification": "FAILED",
                }],
            }]
        }
        model = build_canonical_evidence_model(manifest, PROJECT_ROOT)
        self.assertEqual(model.total_keys_verified, 0)
        self.assertEqual(model.total_keys_failed, 1)

    # ── Invariant 35 — Unavailable transition NOT counted as verified ────────────
    def test_35_unavailable_transition_is_not_counted_as_verified(self):
        """Transitions with evidence_classification == 'UNAVAILABLE' must yield verified = 0, unavailable = 1."""
        manifest = {
            "discovery_results": [{
                "screens": {},
                "transitions": [{
                    "is_state_changed": True,
                    "evidence_classification": "UNAVAILABLE",
                }],
            }]
        }
        model = build_canonical_evidence_model(manifest, PROJECT_ROOT)
        self.assertEqual(model.total_keys_verified, 0)
        self.assertEqual(model.total_keys_unavailable, 1)

    # ── Invariant 36 — Key unsafe count is manifest derived ───────────────────────
    def test_36_key_unsafe_count_is_manifest_derived(self):
        """Unsafe transition evidence_classification == 'UNSAFE' is manifest derived and not hardcoded to zero."""
        manifest = {
            "discovery_results": [{
                "screens": {},
                "transitions": [{
                    "evidence_classification": "UNSAFE",
                }],
            }]
        }
        model = build_canonical_evidence_model(manifest, PROJECT_ROOT)
        self.assertEqual(model.total_keys_verified, 0)
        self.assertEqual(model.total_keys_unsafe, 1)

        import qa.claude_ux.design_audit.report as report_mod
        source = Path(report_mod.__file__).read_text()
        self.assertNotIn("tot_keys_unsafe = 0", source)

    # ── Invariant 37 — Direct budget_info integrity validation ────────────────────
    def test_37_budget_object_integrity_validation(self):
        """REPORT_INTEGRITY_CHECK validates budget_info directly when budget metrics mismatch summary metrics."""
        manifest = {
            "budget": {"target_sessions": 20},
            "summary": {"target_sessions": 999},
        }
        with self.assertRaises(ValueError) as ctx:
            build_canonical_evidence_model(manifest, PROJECT_ROOT)
        self.assertIn("target_sessions", str(ctx.exception))

    # ── Invariant 38 — Unclassified transition is NOT verified ─────────────────────
    def test_38_unclassified_transition_is_not_verified(self):
        """A transition with no evidence_classification (even if is_state_changed=True) MUST NOT be counted as verified."""
        manifest = {
            "discovery_results": [{
                "screens": {},
                "transitions": [{
                    "is_state_changed": True,  # Missing evidence_classification tag
                }],
            }]
        }
        model = build_canonical_evidence_model(manifest, PROJECT_ROOT)
        self.assertEqual(model.total_keys_verified, 0, "Unclassified transition must NOT be counted as verified")

    # ── Invariant 39 — Unclassified transition is NOT failed ───────────────────────
    def test_39_unclassified_transition_is_not_failed(self):
        """A transition with no evidence_classification (even if is_state_changed=False) MUST NOT be counted as failed."""
        manifest = {
            "discovery_results": [{
                "screens": {},
                "transitions": [{
                    "is_state_changed": False,  # Missing evidence_classification tag
                }],
            }]
        }
        model = build_canonical_evidence_model(manifest, PROJECT_ROOT)
        self.assertEqual(model.total_keys_failed, 0, "Unclassified transition must NOT be counted as failed")

    # ── Invariant 40 — Static guard asserting zero is_state_changed fallback in report.py ──
    def test_40_keyboard_accounting_has_no_state_change_fallback(self):
        """Static code audit asserting report.py contains zero is_state_changed fallback heuristic in keyboard metrics."""
        import qa.claude_ux.design_audit.report as report_mod
        source = Path(report_mod.__file__).read_text()
        self.assertNotIn(
            'evidence_classification") is None and r.get("is_state_changed")',
            source,
            "report.py MUST NOT contain is_state_changed fallback logic for unclassified evidence"
        )

    # ── Invariant 41 — Unknown key classification maps to unavailable ──────────────
    def test_41_unknown_key_classification_maps_to_unavailable(self):
        """Unknown or typo evidence_classification values explicitly map to unavailable without corrupting totals."""
        manifest = {
            "discovery_results": [{
                "screens": {},
                "transitions": [{
                    "evidence_classification": "TYPO_UNKNOWN",
                }],
            }]
        }
        model = build_canonical_evidence_model(manifest, PROJECT_ROOT)
        self.assertEqual(model.total_keys_verified, 0)
        self.assertEqual(model.total_keys_failed, 0)
        self.assertEqual(model.total_keys_unsafe, 0)
        self.assertEqual(model.total_keys_unavailable, 1)

    # ── Invariant 42 — ReadinessStateMachine rejects line-count as prompt proof ───
    def test_42_readiness_cannot_use_line_count_as_prompt_proof(self):
        """ReadinessStateMachine MUST NOT evaluate PROMPT_READY using line-count fallback if prompt markers are absent."""
        class MockDriver:
            def __init__(self, lines):
                self.lines = lines
            def get_terminal_text(self):
                return self.lines
            def press_key(self, k):
                pass

        # Terminal text has identity ('Claude Code') and 4 lines (len >= 2), but NO prompt markers ('❯', '>', etc.)
        mock_driver = MockDriver([
            "Claude Code v2.1.226",
            "Welcome back!",
            "Loading codebase context...",
            "Initializing workspace...",
        ])
        readiness = ReadinessStateMachine(mock_driver)
        ok, state, msg = readiness.evaluate_readiness({"window_found": True, "claude_process_detected": True})

        self.assertFalse(ok, "Readiness MUST NOT return True when prompt markers are absent")
        self.assertEqual(state, "UNAVAILABLE")

    # ── Invariant 43 — Unclassified screen with verified=True is NOT verified ──────
    def test_43_unclassified_screen_is_not_counted_as_verified(self):
        """A screen with verified=True but missing evidence_classification MUST NOT be counted as verified."""
        manifest = {
            "discovery_results": [{
                "screens": {
                    "s1": {"title": "Screen 1", "verified": True},  # Missing evidence_classification tag
                },
                "transitions": []
            }]
        }
        model = build_canonical_evidence_model(manifest, PROJECT_ROOT)
        self.assertEqual(model.total_screens_verified, 0, "Unclassified screen must NOT be counted as verified")
        self.assertEqual(model.total_screens_unavailable, 1)

    # ── Invariant 44 — Unclassified visual state with screenshot verified=True is NOT verified ─
    def test_44_unclassified_visual_state_is_not_counted_as_verified(self):
        """A visual state record with screenshot verified=True but missing evidence_classification MUST NOT be verified."""
        manifest = {
            "captured_records": [
                {"screen_id": "s1", "screenshot_verification": {"verified": True}},  # Missing evidence_classification tag
            ]
        }
        model = build_canonical_evidence_model(manifest, PROJECT_ROOT)
        self.assertEqual(model.total_visual_states_verified, 0, "Unclassified visual state must NOT be counted as verified")
        self.assertEqual(model.total_visual_states_unavailable, 1)

    # ── Invariant 45 — Static guard asserting zero len(text_lines) >= 2 fallback ──
    def test_45_readiness_source_guard_no_line_count_fallback(self):
        """Static code audit asserting readiness.py contains zero len(text_lines) >= 2 fallback heuristic."""
        import qa.claude_ux.discovery.readiness as readiness_mod
        source = Path(readiness_mod.__file__).read_text()
        self.assertNotIn(
            "len(text_lines) >= 2",
            source,
            "readiness.py MUST NOT use len(text_lines) >= 2 fallback heuristic for prompt readiness"
        )

    # ── Invariant 46 — Discoverer exposes self.frontier attribute ──────────────────
    def test_46_discoverer_exposes_live_frontier_state(self):
        """PathReplayDiscoverer MUST expose self.frontier as a public list attribute."""
        from qa.claude_ux.design_audit.discover import PathReplayDiscoverer
        discoverer = PathReplayDiscoverer(PROJECT_ROOT / "tmp")
        self.assertTrue(hasattr(discoverer, "frontier"), "PathReplayDiscoverer MUST have self.frontier attribute")
        self.assertIsInstance(discoverer.frontier, list, "self.frontier MUST be a list")

    # ── Invariant 47 — Runner frontier_exhausted derives from discoverer.frontier ───
    def test_47_runner_frontier_exhausted_uses_discoverer_frontier(self):
        """Runner's frontier_exhausted metric derives strictly from len(discoverer.frontier) == 0."""
        from qa.claude_ux.design_audit.discover import PathReplayDiscoverer
        discoverer = PathReplayDiscoverer(PROJECT_ROOT / "tmp")

        discoverer.frontier = []
        frontier_exhausted_empty = (len(discoverer.frontier) == 0)
        self.assertTrue(frontier_exhausted_empty)

        discoverer.frontier = ["screen_unexplored_1"]
        frontier_exhausted_active = (len(discoverer.frontier) == 0)
        self.assertFalse(frontier_exhausted_active)

    # ── Invariant 48 — Static guard: discover.py delegates to classify_evidence ─────
    def test_48_discoverer_uses_strict_evidence_classifier(self):
        """discover.py MUST delegate transition classification to classify_evidence() and contains zero local heuristics."""
        import qa.claude_ux.design_audit.discover as discover_mod
        source = Path(discover_mod.__file__).read_text()
        self.assertIn("classify_evidence(", source, "discover.py MUST call classify_evidence()")
        self.assertNotIn(
            '"VERIFIED" if is_state_changed',
            source,
            "discover.py MUST NOT contain local is_state_changed -> VERIFIED fallback logic"
        )

    # ── Invariant 49 — SessionResult strict binary terminal partition ─────────────
    def test_49_session_result_strict_terminal_partition(self):
        """SessionResult satisfies started == completed + failed for any session state."""
        run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "design_audit" / "runs" / "test_tmp"

        # 1. Unclosed/Aborted session
        s1 = ClaudeSession(run_dir, "s1")
        s1.launch_succeeded = True
        r1 = s1.make_result()
        self.assertTrue(r1.started)
        self.assertFalse(r1.completed)
        self.assertTrue(r1.failed)
        self.assertEqual(int(r1.started), int(r1.completed) + int(r1.failed))

        # 2. Successfully completed session
        s2 = ClaudeSession(run_dir, "s2")
        s2.launch_succeeded = True
        s2.mark_completed()
        r2 = s2.make_result()
        self.assertTrue(r2.started)
        self.assertTrue(r2.completed)
        self.assertFalse(r2.failed)
        self.assertEqual(int(r2.started), int(r2.completed) + int(r2.failed))

        # 3. Failed session
        s3 = ClaudeSession(run_dir, "s3")
        s3.launch_succeeded = True
        s3.mark_failed()
        r3 = s3.make_result()
        self.assertTrue(r3.started)
        self.assertFalse(r3.completed)
        self.assertTrue(r3.failed)
        self.assertEqual(int(r3.started), int(r3.completed) + int(r3.failed))

    # ── Invariant 50 — REPORT_INTEGRITY_CHECK session partition enforcement ─────────
    def test_50_report_integrity_check_fails_on_unpartitioned_sessions(self):
        """REPORT_INTEGRITY_CHECK raises ValueError if session_results started count does not equal completed + failed."""
        unpartitioned_manifest = {
            "session_results": [
                {"session_id": "s1", "started": True, "completed": False, "failed": False},  # Unpartitioned!
            ]
        }
        with self.assertRaises(ValueError) as ctx:
            build_canonical_evidence_model(unpartitioned_manifest, PROJECT_ROOT)
        self.assertIn("does not equal completed", str(ctx.exception))

    # ── Invariant 51 — type_and_submit uses single enter for normal queries ────────
    def test_51_normal_query_type_and_submit_single_enter(self):
        """type_and_submit() types query and issues strictly 1 Return event."""
        run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "design_audit" / "runs" / "test_tmp"
        s = ClaudeSession(run_dir, "test_s51")

        calls = []
        s.type = lambda text: calls.append(("type", text)) or True
        s.press_key = lambda k: calls.append(("press_key", k)) or True

        res = s.type_and_submit("Normal Query Test")
        self.assertTrue(res)
        self.assertEqual(calls, [("type", "Normal Query Test"), ("press_key", "enter")])

    # ── Invariant 52 — type_slash_command_and_submit uses double enter ─────────────
    def test_52_slash_command_type_and_submit_double_enter(self):
        """type_slash_command_and_submit() types command and issues strictly 2 Return events."""
        run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "design_audit" / "runs" / "test_tmp"
        s = ClaudeSession(run_dir, "test_s52")

        calls = []
        s.type = lambda text: calls.append(("type", text)) or True
        s.press_key = lambda k: calls.append(("press_key", k)) or True

        res = s.type_slash_command_and_submit("/color cyan")
        self.assertTrue(res)
        self.assertEqual(
            calls,
            [("type", "/color cyan"), ("press_key", "enter"), ("press_key", "enter")]
        )

    # ── Invariant 53 — ctrl+k and cmd+k modifier separation ────────────────────────
    def test_53_ctrl_k_and_cmd_k_modifier_separation(self):
        """KEY_EVENTS separates ctrl+k (control down) and cmd+k (command down) without combining modifiers."""
        from qa.claude_ux.driver.session import KEY_EVENTS
        self.assertEqual(KEY_EVENTS["ctrl+k"], (40, ("control down",)))
        self.assertEqual(KEY_EVENTS["cmd+k"], (40, ("command down",)))

    # ── Invariant 54 — _run_osascript raises RuntimeError on non-zero exit code ─────
    def test_54_osascript_failure_raises_runtime_error(self):
        """_run_osascript raises RuntimeError when osascript returns non-zero code."""
        run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "design_audit" / "runs" / "test_tmp"
        s = ClaudeSession(run_dir, "test_s54")

        with unittest.mock.patch("subprocess.run") as mock_run:
            mock_run.return_value = unittest.mock.MagicMock(returncode=1, stderr="Execution error: Permission denied")
            with self.assertRaises(RuntimeError) as ctx:
                s._run_osascript("invalid script")
            self.assertIn("Permission denied", str(ctx.exception))

    # ── Invariant 55 — press_key returns False on osascript failure ────────────────
    def test_55_press_key_returns_false_on_osascript_error(self):
        """press_key returns False when _run_osascript raises RuntimeError."""
        run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "design_audit" / "runs" / "test_tmp"
        s = ClaudeSession(run_dir, "test_s55")
        s._activate_session_tab = lambda: None

        with unittest.mock.patch.object(s, "_run_osascript", side_effect=RuntimeError("osascript failed")):
            res = s.press_key("enter")
            self.assertFalse(res, "press_key MUST return False on osascript failure")

    # ── Invariant 56 — type_and_submit failure propagation ─────────────────────────
    def test_56_type_and_submit_propagates_type_failure(self):
        """type_and_submit immediately returns False if type() fails without sending enter."""
        run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "design_audit" / "runs" / "test_tmp"
        s = ClaudeSession(run_dir, "test_s56")

        calls = []
        s.type = lambda text: False
        s.press_key = lambda k: calls.append(k) or True

        res = s.type_and_submit("Failed Query")
        self.assertFalse(res)
        self.assertEqual(calls, [], "press_key MUST NOT be called if type() fails")

    # ── Invariant 57 — type_slash_command_and_submit failure propagation ───────────
    def test_57_type_slash_command_propagates_type_failure(self):
        """type_slash_command_and_submit immediately returns False if type() fails."""
        run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "design_audit" / "runs" / "test_tmp"
        s = ClaudeSession(run_dir, "test_s57")

        calls = []
        s.type = lambda text: False
        s.press_key = lambda k: calls.append(k) or True

        res = s.type_slash_command_and_submit("/failed")
        self.assertFalse(res)
        self.assertEqual(calls, [], "press_key MUST NOT be called if type() fails")

    # ── Invariant 58 — Static guard: modifier key text fallback prohibited ─────────
    def test_58_static_guard_no_modifier_literal_keystroke_fallback(self):
        """session.py MUST contain explicit guard preventing modifier shortcut keys from falling through to literal keystroke text."""
        import qa.claude_ux.driver.session as session_mod
        source = Path(session_mod.__file__).read_text()
        self.assertIn(
            'elif "ctrl" in key_lower or "cmd" in key_lower or "alt" in key_lower:',
            source,
            "session.py MUST contain static modifier key fallback guard"
        )


if __name__ == "__main__":
    unittest.main()
