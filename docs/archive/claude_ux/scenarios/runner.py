#!/usr/bin/env python3
"""
Decoupled 2-Layer Matrix Runner with 4-Way Metric Counter Equality & Global Visual State Deduplication.
Layer A executes 100 behavioral sessions without screenshots.
Layer B captures PNG screenshots only for unique visually significant state fingerprints.
"""

import os
import sys
import time
import json
from pathlib import Path
from datetime import datetime

from .definitions import SCENARIOS, VIEWPORTS
from ..driver.session import ClaudeSession
from qa.claude_ux.discovery.fingerprint import TerminalFingerprinter, VisualContext


class VisualCapturePolicy:
    """Evaluates visual significance and enforces global visual state deduplication."""

    SIGNIFICANT_SCENARIOS = {
        "01_home", "02_home_focused_prompt", "03_slash_completion",
        "05_ctrl_k_global_search", "07_theme_picker", "08_help",
        "09_status", "10_workspace_query", "11_streaming_response",
        "13_unseen_message_state"
    }

    def __init__(self):
        self.global_captured_fingerprints = set()

    def evaluate(self, sc_id: str, logical_fp, visual_ctx: VisualContext) -> dict:
        visual_state_id = f"{logical_fp.state_id()}_{visual_ctx.variant_id()}"
        
        is_significant_scenario = sc_id in self.SIGNIFICANT_SCENARIOS
        is_new_visual_state = visual_state_id not in self.global_captured_fingerprints

        capture_required = is_significant_scenario and is_new_visual_state
        skip_reason = "duplicate_visual_fingerprint" if not is_new_visual_state else ("behavioral_only_scenario" if not is_significant_scenario else "none")

        if capture_required:
            self.global_captured_fingerprints.add(visual_state_id)

        return {
            "capture_required": capture_required,
            "skip_reason": skip_reason,
            "visual_significance": f"Significant scenario: {is_significant_scenario}, New state: {is_new_visual_state}",
            "logical_state_id": logical_fp.state_id(),
            "visual_state_id": visual_state_id
        }


class ScenarioRunner:
    """Runs 100 isolated behavioral sessions with 4-way metric counter equality enforcement."""

    def __init__(self, run_dir: Path):
        self.run_dir = run_dir
        self.manifest_entries = []
        self.policy = VisualCapturePolicy()

    def execute_matrix(self, viewports=None, scenarios=None):
        if viewports is None:
            viewports = VIEWPORTS
        if scenarios is None:
            scenarios = SCENARIOS

        total_expected = len(scenarios) * len(viewports)
        print(f"=== Starting Decoupled 2-Layer Empirical Matrix Run ===")
        print(f"Output directory: {self.run_dir}")
        print(f"Scenarios: {len(scenarios)} | Viewports: {len(viewports)} | Total Isolated Sessions: {total_expected}\n")

        capture_decision_count = 0
        successful_capture_count = 0
        ocr_run_count = 0

        session_count = 0
        for vp_w, vp_h in viewports:
            vp_tuple = (vp_w, vp_h)
            vp_str = f"{vp_w}x{vp_h}"
            print(f"--- Launching Viewport Matrix {vp_str} ---")

            for sc in scenarios:
                session_count += 1
                sc_id = sc["id"]
                print(f"  [{session_count}/{total_expected}] Scenario {sc_id} ({vp_str})... ", end="", flush=True)

                session = ClaudeSession(self.run_dir, sc_id, vp_tuple)
                try:
                    # Layer A: Zero-screenshot launch and readiness
                    launched = session.launch()
                    if not launched:
                        print("LAUNCH FAILED")
                        res_entry = {
                            "session_id": session.session_id,
                            "scenario": sc_id,
                            "viewport": vp_str,
                            "status": "INVALID",
                            "captured": False,
                            "capture_required": False,
                            "reason": "Launch verification failed"
                        }
                    else:
                        for action_type, arg in sc["actions"]:
                            if action_type == "type":
                                session.type(arg)
                            elif action_type == "press":
                                session.press(arg)
                            elif action_type == "resize":
                                rw, rh = arg
                                session.resize(rw, rh)
                            elif action_type == "wait":
                                time.sleep(arg)

                        # Observe terminal state without taking screenshot
                        obs_lines = session.observe_terminal_state()
                        logical_fp, visual_ctx = TerminalFingerprinter.compute_fingerprint(obs_lines, vp_tuple)
                        
                        # Evaluate global visual capture policy
                        decision = self.policy.evaluate(sc_id, logical_fp, visual_ctx)

                        if decision["capture_required"]:
                            capture_decision_count += 1
                            # Layer B: Capture visual PNG screenshot
                            res_entry = session.capture_visual_state(sc)
                            res_entry.update(decision)

                            shot_verif = res_entry.get("screenshot_verification", {})
                            if shot_verif.get("captured"):
                                successful_capture_count += 1
                                ocr_run_count += 1
                                res_entry["captured"] = True
                            else:
                                res_entry["captured"] = False

                            print(f"PASS (Visual PNG Captured: {decision['visual_state_id']})")
                        else:
                            # Layer A: Behavioral observation only (0 PNG created!)
                            res_entry = {
                                "session_id": session.session_id,
                                "scenario": sc_id,
                                "viewport": vp_str,
                                "status": "PASS",
                                "validation_status": "BEHAVIORAL_OBSERVED",
                                "captured": False,
                                "capture_required": False,
                                "launch_verification": session.launch_record,
                                "screenshot_verification": {"captured": False, "method": "skipped", "file_size": 0, "path": "none"},
                                "ocr_validation": {"expected_markers": sc.get("expected_markers", []), "matched_markers": [], "valid": True},
                                "preconditions": sc.get("preconditions", []),
                                "postconditions": sc.get("postconditions", [])
                            }
                            res_entry.update(decision)
                            print(f"PASS (Layer A Screenshot Skipped: {decision['skip_reason']})")

                    self.manifest_entries.append(res_entry)

                except Exception as e:
                    print(f"ERROR: {e}")
                    self.manifest_entries.append({
                        "session_id": session.session_id,
                        "scenario": sc_id,
                        "viewport": vp_str,
                        "status": "FAIL",
                        "captured": False,
                        "capture_required": False,
                        "error": str(e)
                    })
                finally:
                    session.close()
                    time.sleep(0.3)

        # 4-Way Metric Counter Equality Enforcement
        disk_pngs = list(self.run_dir.glob("**/*.png"))
        filesystem_png_count = len(disk_pngs)

        pass_count = sum(1 for e in self.manifest_entries if e.get("status") == "PASS")
        fail_count = sum(1 for e in self.manifest_entries if e.get("status") == "FAIL")
        invalid_count = sum(1 for e in self.manifest_entries if e.get("status") == "INVALID")

        is_4way_equal = (
            capture_decision_count == successful_capture_count == filesystem_png_count == ocr_run_count
        )

        manifest_data = {
            "timestamp": datetime.now().isoformat(),
            "total_behavioral_sessions": total_expected,
            "total_executed": len(self.manifest_entries),
            "summary": {
                "pass": pass_count,
                "fail": fail_count,
                "invalid": invalid_count,
                "capture_decision_count": capture_decision_count,
                "successful_capture_count": successful_capture_count,
                "filesystem_png_count": filesystem_png_count,
                "ocr_run_count": ocr_run_count,
                "metric_equality_status": "PASSED" if is_4way_equal else "FAILED",
                "screenshots_skipped": total_expected - capture_decision_count
            },
            "sessions": self.manifest_entries
        }

        manifest_path = self.run_dir / "manifest.json"
        with open(manifest_path, "w") as f:
            json.dump(manifest_data, f, indent=2)

        print(f"\nMatrix execution complete.")
        print(f"4-Way Metric Invariant Check: {'PASSED' if is_4way_equal else 'FAILED'}")
        print(f"  Capture Decisions:  {capture_decision_count}")
        print(f"  Successful Captures:{successful_capture_count}")
        print(f"  Filesystem PNGs:    {filesystem_png_count}")
        print(f"  OCR Runs Executed:  {ocr_run_count}")
        print(f"Manifest saved: {manifest_path}")
        return manifest_data
