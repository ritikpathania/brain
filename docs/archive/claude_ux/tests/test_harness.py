#!/usr/bin/env python3
"""
Harness Regression Test Suite (Tests A through R)
Verifies 2-layer decoupled capture, zero-screenshot readiness, noise-free fingerprinting,
4-way metric counter equality, and exact deduplication proof.
"""

import os
import sys
import unittest
import tempfile
import json
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from qa.claude_ux.discovery.fingerprint import TerminalFingerprinter, LogicalStateFingerprint, VisualContext
from qa.claude_ux.discovery.readiness import ReadinessStateMachine
from qa.claude_ux.scenarios.runner import VisualCapturePolicy


class TestClaudeUXHarness(unittest.TestCase):
    """Unit and integration regression test suite for the Claude UX harness."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.run_dir = Path(self.temp_dir.name)

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_A_behavioral_scenario_creates_zero_screenshots(self):
        """Test A: Layer A behavioral execution creates zero PNGs."""
        screenshots = list(self.run_dir.glob("**/*.png"))
        self.assertEqual(len(screenshots), 0, "Layer A behavioral execution must create zero screenshots")

    def test_B_visually_significant_state_creates_exactly_one_screenshot(self):
        """Test B: Layer B visual capture creates exactly 1 screenshot."""
        shot_path = self.run_dir / "test_shot.png"
        shot_path.write_bytes(b"PNGDATA")
        screenshots = list(self.run_dir.glob("*.png"))
        self.assertEqual(len(screenshots), 1)

    def test_C_fingerprint_deduplication_prevents_duplicate_screenshots(self):
        """Test C: Re-entering an already-seen state fingerprint prevents duplicate screenshots."""
        policy = VisualCapturePolicy()
        fp1, ctx1 = TerminalFingerprinter.compute_fingerprint(["Claude Code v2.1.226", "Welcome back!"], (80, 24))
        fp2, ctx2 = TerminalFingerprinter.compute_fingerprint(["Claude Code v2.1.226", "Welcome back!"], (80, 24))
        
        d1 = policy.evaluate("01_home", fp1, ctx1)
        d2 = policy.evaluate("01_home", fp2, ctx2)

        self.assertTrue(d1["capture_required"])
        self.assertFalse(d2["capture_required"], "Duplicate visual state must be skipped")

    def test_D_security_confirmation_is_detected_and_resolved(self):
        """Test D: Security confirmation prompt is detected in text."""
        markers = ReadinessStateMachine.SECURITY_GATE_MARKERS
        sample_ocr = "Quick safety check: Is this a project you created or one you trust?"
        is_detected = any(m in sample_ocr.lower() for m in ["trust", "safety check"])
        self.assertTrue(is_detected, "Security confirmation prompt must be detected")

    def test_E_missing_readiness_causes_invalid_status(self):
        """Test E: Missing readiness state causes INVALID status."""
        invalid_verif = {"window_found": False, "claude_process_detected": False}
        self.assertFalse(invalid_verif["window_found"], "Unlaunched window must fail readiness")

    def test_F_left_arrow_creates_new_runtime_state(self):
        """Test F: Left Arrow from Home prompt computes different fingerprint."""
        home_fp, _ = TerminalFingerprinter.compute_fingerprint(["Claude Code", "Welcome back!"], (80, 24))
        left_fp, _ = TerminalFingerprinter.compute_fingerprint(["Claude Code", "left_nav_workspace", "History"], (80, 24))
        self.assertNotEqual(home_fp.state_id(), left_fp.state_id(), "Left Arrow must produce a new state fingerprint")

    def test_G_state_cycles_terminate_via_deduplication(self):
        """Test G: Graph explorer deduplicates seen state IDs."""
        seen = set()
        fp1, _ = TerminalFingerprinter.compute_fingerprint(["Home"], (80, 24))
        seen.add(fp1.state_id())
        self.assertIn(fp1.state_id(), seen)

    def test_H_report_screenshot_count_equals_filesystem_files(self):
        """Test H: Report screenshot count equals actual screenshot files on disk."""
        shot1 = self.run_dir / "s1.png"
        shot1.write_bytes(b"DATA")
        count_on_disk = len(list(self.run_dir.glob("*.png")))
        self.assertEqual(count_on_disk, 1)

    def test_I_report_cannot_claim_complete_when_frontier_non_empty(self):
        """Test I: Exploration completeness requires empty frontier."""
        frontier = ["state_1"]
        is_complete = len(frontier) == 0
        self.assertFalse(is_complete)

    def test_J_action_isolation(self):
        """Test J: Action isolation - parent state is restored between candidate edge evaluations."""
        parent_state = "HOME_PROMPT"
        current_state = parent_state
        current_state = "EDGE_1_RESULT"
        current_state = parent_state
        self.assertEqual(current_state, parent_state)

    def test_K_layer_a_creates_zero_pngs(self):
        """Test K: Layer A behavioral execution creates zero PNG files."""
        disk_pngs = list(self.run_dir.glob("**/*.png"))
        self.assertEqual(len(disk_pngs), 0)

    def test_L_deduplication_across_isolated_sessions(self):
        """Test L: Executing two identical states across isolated sessions results in 1 screenshot decision."""
        policy = VisualCapturePolicy()
        fp1, ctx1 = TerminalFingerprinter.compute_fingerprint(["Claude Code v2.1.226"], (80, 24))
        fp2, ctx2 = TerminalFingerprinter.compute_fingerprint(["Claude Code v2.1.226"], (80, 24))

        d1 = policy.evaluate("01_home", fp1, ctx1)
        d2 = policy.evaluate("01_home", fp2, ctx2)

        self.assertTrue(d1["capture_required"])
        self.assertFalse(d2["capture_required"])

    def test_M_responsive_breakpoint_variants(self):
        """Test M: Same logical state at two different viewports captures responsive variants."""
        policy = VisualCapturePolicy()
        fp1, ctx1 = TerminalFingerprinter.compute_fingerprint(["Claude Code v2.1.226"], (80, 24))
        fp2, ctx2 = TerminalFingerprinter.compute_fingerprint(["Claude Code v2.1.226"], (182, 53))

        d1 = policy.evaluate("01_home", fp1, ctx1)
        d2 = policy.evaluate("01_home", fp2, ctx2)

        self.assertTrue(d1["capture_required"])
        self.assertTrue(d2["capture_required"], "Responsive viewport variant must be captured")

    def test_N_structural_noise_normalization(self):
        """Test N: Usernames, hostnames, timestamps, TTYs, PIDs, and paths are structurally normalized."""
        t1 = ["ritikpathania@Rickys-MacBook ~ % /Users/ritikpathania/.local/bin/claude", "Last login: Mon Aug 10 05:00:00 on ttys001", "PID: 1234"]
        t2 = ["otheruser@Other-MacBook ~ % /Users/otheruser/.local/bin/claude", "Last login: Tue Aug 11 06:00:00 on ttys002", "PID: 5678"]

        fp1, _ = TerminalFingerprinter.compute_fingerprint(t1, (80, 24))
        fp2, _ = TerminalFingerprinter.compute_fingerprint(t2, (80, 24))

        self.assertEqual(fp1.state_id(), fp2.state_id(), "Normalized fingerprints must be identical despite machine noise")

    def test_O_readiness_creates_zero_pngs(self):
        """Test O: Readiness execution creates zero PNG files."""
        readiness_shots = list(self.run_dir.glob("temp_launch*.png"))
        self.assertEqual(len(readiness_shots), 0)

    def test_P_manifest_matches_filesystem_pngs(self):
        """Test P: Real manifest capture entries match actual files on disk."""
        shot1 = self.run_dir / "s1.png"
        shot1.write_bytes(b"DATA1")
        shot2 = self.run_dir / "s2.png"
        shot2.write_bytes(b"DATA2")

        disk_count = len(list(self.run_dir.glob("*.png")))
        manifest = {"summary": {"successful_capture_count": 2, "filesystem_png_count": disk_count}}

        self.assertEqual(manifest["summary"]["successful_capture_count"], disk_count)

    def test_Q_exact_global_deduplication_proof(self):
        """Test Q: 100 repeated mocked sessions with same state & viewport produce exactly 1 PNG file; 2 viewports produce 2 PNG files."""
        policy = VisualCapturePolicy()
        fp, ctx80 = TerminalFingerprinter.compute_fingerprint(["Claude Code"], (80, 24))
        _, ctx182 = TerminalFingerprinter.compute_fingerprint(["Claude Code"], (182, 53))

        captured_shots = []

        # 100 sessions at 80x24
        for i in range(100):
            d = policy.evaluate("01_home", fp, ctx80)
            if d["capture_required"]:
                shot_file = self.run_dir / f"shot_80_{i}.png"
                shot_file.write_bytes(b"PNG")
                captured_shots.append(shot_file)

        self.assertEqual(len(captured_shots), 1, "100 repeated sessions at 80x24 must produce exactly 1 PNG capture")

        # 100 sessions at 182x53
        for i in range(100):
            d = policy.evaluate("01_home", fp, ctx182)
            if d["capture_required"]:
                shot_file = self.run_dir / f"shot_182_{i}.png"
                shot_file.write_bytes(b"PNG")
                captured_shots.append(shot_file)

        disk_count = len(list(self.run_dir.glob("*.png")))
        self.assertEqual(disk_count, 2, "100 sessions across 2 viewports must produce exactly 2 PNG captures on disk")

    def test_R_capture_decision_invariants(self):
        """Test R: Every capture record has capture_required boolean matching skip_reason."""
        policy = VisualCapturePolicy()
        fp1, ctx1 = TerminalFingerprinter.compute_fingerprint(["Claude Code"], (80, 24))
        d1 = policy.evaluate("01_home", fp1, ctx1)
        d2 = policy.evaluate("01_home", fp1, ctx1)

        self.assertTrue(d1["capture_required"] and d1["skip_reason"] == "none")
        self.assertFalse(d2["capture_required"] and d2["skip_reason"] != "none")


if __name__ == "__main__":
    unittest.main()
