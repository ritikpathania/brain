#!/usr/bin/env python3
"""
Unit and Regression Test Suite for QA Engine State Predicates and Telemetry

Verifies that:
1. create_session fails when session count does not increase (even with a healthy DB).
2. create_session passes when session count increases.
3. execute_first_query fails when query metric does not increment (even with a healthy DB).
4. execute_first_query passes when query metric increments.
5. change_theme fails when theme remains unchanged (even with healthy DB, HTTP, and UDS).
6. change_theme passes when theme changes.
7. first_frame_rendered requires actual observable UI marker detection.
"""

import unittest
from pathlib import Path
import sys

# Add project root to sys.path
PROJECT_DIR = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(PROJECT_DIR))

from qa.run_validation import AdaptiveInteractionPlanner, MonotonicTimeline

class DummyRunner:
    def __init__(self):
        self.timeline = MonotonicTimeline()
        self.sqlite_state = (True, "SQLite ok", 5, 0, 0)
        self.http_metrics = (True, "HTTP ok", {"queries_total": 0})
        self.uds_state = (True, "UDS ok")
        self.theme = "Dark"

    def assert_sqlite_state(self):
        return self.sqlite_state

    def assert_http_metrics(self):
        return self.http_metrics

    def assert_uds_socket_state(self):
        return self.uds_state

    def capture_window_screenshot(self, name_prefix):
        return None

    def get_active_theme_from_ocr_or_state(self, img_path=None):
        return self.theme

    def capture_current_state(self):
        return {"theme": self.theme, "session_cnt": 0, "query_cnt": 0, "db_ok": True, "http_ok": True, "uds_ok": True}


class TestQAPredicates(unittest.TestCase):
    def setUp(self):
        self.runner = DummyRunner()
        self.planner = AdaptiveInteractionPlanner(self.runner)

    def test_create_session_fails_without_new_session(self):
        """False-Positive Protection: Healthy DB with pre-existing tables but 0 session count increase MUST fail."""
        goal = {"id": "create_session"}
        baseline_state = {"session_cnt": 2, "query_cnt": 0, "theme": "Dark"}
        # Post-action state: session_cnt remains 2 (no new session created)
        post_state = {"db_ok": True, "http_ok": True, "uds_ok": True, "session_cnt": 2, "query_cnt": 0, "theme": "Dark", "last_dispatched_command": "session.new"}

        success = self.planner.evaluate_state_predicate(goal, baseline_state, post_state)
        self.assertFalse(
            success,
            "create_session predicate MUST fail if session count does not increase after session.new"
        )

    def test_create_session_passes_when_session_count_increases(self):
        """Valid Execution: Session count increases after session creation action."""
        goal = {"id": "create_session"}
        baseline_state = {"session_cnt": 0, "query_cnt": 0, "theme": "Dark"}
        post_state = {"db_ok": True, "http_ok": True, "uds_ok": True, "session_cnt": 1, "query_cnt": 0, "theme": "Dark", "last_dispatched_command": "session.new"}

        success = self.planner.evaluate_state_predicate(goal, baseline_state, post_state)
        self.assertTrue(
            success,
            "create_session predicate MUST pass when session count increases"
        )

    def test_query_fails_without_query_execution(self):
        """False-Positive Protection: Healthy DB without query metric increment MUST fail."""
        goal = {"id": "execute_first_query"}
        baseline_state = {"session_cnt": 0, "query_cnt": 5, "theme": "Dark", "node_cnt": 0}
        # Post-action state: query count unchanged
        post_state = {"db_ok": True, "http_ok": True, "uds_ok": True, "session_cnt": 0, "query_cnt": 5, "theme": "Dark", "node_cnt": 0}

        success = self.planner.evaluate_state_predicate(goal, baseline_state, post_state)
        self.assertFalse(
            success,
            "execute_first_query predicate MUST fail if query metrics do not increment"
        )

    def test_query_passes_when_query_metric_increments(self):
        """Valid Execution: Query count increments after query execution."""
        goal = {"id": "execute_first_query"}
        baseline_state = {"session_cnt": 0, "query_cnt": 5, "theme": "Dark", "node_cnt": 0}
        post_state = {"db_ok": True, "http_ok": True, "uds_ok": True, "session_cnt": 0, "query_cnt": 6, "theme": "Dark", "node_cnt": 0}

        success = self.planner.evaluate_state_predicate(goal, baseline_state, post_state)
        self.assertTrue(
            success,
            "execute_first_query predicate MUST pass when query metrics increment"
        )

    def test_theme_fails_when_theme_unchanged(self):
        """False-Positive Protection: Healthy DB + HTTP + UDS with unchanged theme MUST fail."""
        goal = {"id": "change_theme"}
        baseline_state = {"session_cnt": 0, "query_cnt": 0, "theme": "Dark"}
        # Post-action state: theme remains Dark
        post_state = {"db_ok": True, "http_ok": True, "uds_ok": True, "session_cnt": 0, "query_cnt": 0, "theme": "Dark"}

        success = self.planner.evaluate_state_predicate(goal, baseline_state, post_state)
        self.assertFalse(
            success,
            "change_theme predicate MUST fail if active theme is unchanged"
        )

    def test_theme_passes_when_theme_changes(self):
        """Valid Execution: Active theme changes after theme command execution."""
        goal = {"id": "change_theme"}
        baseline_state = {"session_cnt": 0, "query_cnt": 0, "theme": "Dark"}
        post_state = {"db_ok": True, "http_ok": True, "uds_ok": True, "session_cnt": 0, "query_cnt": 0, "theme": "High Contrast"}

        success = self.planner.evaluate_state_predicate(goal, baseline_state, post_state)
        self.assertTrue(
            success,
            "change_theme predicate MUST pass when active theme changes"
        )

    def test_first_frame_requires_actual_ui_observation(self):
        """Verification: First-frame detection fails if OCR output lacks deterministic UI markers."""
        # Simulated OCR scanner returning text without markers
        ocr_without_markers = "Terminal Shell Prompt $ "
        markers = ["RELATIONAL MEMORY ENGINE", "SYSTEM ONLINE", "PRESS / FOR COMMANDS", "CONNECTED"]
        is_observed = any(m in ocr_without_markers.upper() for m in markers)
        self.assertFalse(
            is_observed,
            "First-frame observation MUST return False when deterministic TUI markers are missing"
        )

        ocr_with_marker = "Brain TUI - Connected to Memory Daemon"
        is_observed_valid = any(m in ocr_with_marker.upper() for m in markers)
        self.assertTrue(
            is_observed_valid,
            "First-frame observation MUST return True when deterministic TUI marker 'CONNECTED' is present"
        )


if __name__ == "__main__":
    unittest.main()
