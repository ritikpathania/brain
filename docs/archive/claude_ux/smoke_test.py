#!/usr/bin/env python3
"""
Empirical Smoke Test Suite for Claude UX Harness (Positive & Negative Test Verification)
Verifies that validation fails when Claude is not present, and passes when Claude TUI responds.
"""

import os
import sys
import time
import json
from pathlib import Path
from datetime import datetime

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from qa.claude_ux.driver.terminal import TerminalDriver
from qa.claude_ux.driver.session import ClaudeSession
from qa.claude_ux.scenarios.definitions import SCENARIOS


def run_smoke_tests() -> bool:
    print("=== Running Empirical Harness Smoke Test Suite ===")
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    test_run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "runs" / f"smoke_{timestamp}"
    test_run_dir.mkdir(parents=True, exist_ok=True)

    # 1. Negative Smoke Test: Verify harness fails validation when Claude is NOT visible
    print("\n[Test 1] Negative Smoke Test (Non-Claude window)... ", end="", flush=True)
    neg_driver = TerminalDriver()
    neg_verif = neg_driver.launch_claude("/bin/bash -c 'echo Hello World; sleep 5'", width=80, height=24)
    time.sleep(1.0)
    
    neg_session = ClaudeSession(test_run_dir, "neg_test", (80, 24))
    neg_session.driver = neg_driver
    neg_session.launch_record = neg_verif
    
    neg_contract = {
        "expected_markers": ["claude", "welcome", "code"],
        "preconditions": ["Terminal created"],
        "postconditions": ["Claude UI rendered"]
    }
    neg_result = neg_session.capture_visual_state(neg_contract)
    neg_session.close()

    if neg_result.get("status") == "INVALID" or neg_result.get("validation_status") == "INVALID":
        print("PASS (Correctly failed validation for non-Claude window)")
    else:
        print(f"FAIL (Unexpectedly marked non-Claude window as {neg_result.get('status')})")
        return False

    # 2. Positive Smoke Test: Verify real interactive Claude session & slash completion
    print("[Test 2] Positive Smoke Test (Real Claude interactive TUI)... ", end="", flush=True)
    pos_session = ClaudeSession(test_run_dir, "03_slash_completion", (80, 24))
    try:
        launched = pos_session.launch()
        if not launched:
            print("FAIL (Claude launch verification failed)")
            return False

        pos_session.press("esc")
        pos_session.type("/")
        time.sleep(0.6)

        sc_03_contract = next(sc for sc in SCENARIOS if sc["id"] == "03_slash_completion")
        pos_result = pos_session.capture_visual_state(sc_03_contract)

        if pos_result.get("status") == "PASS" and pos_result.get("validation_status") in ["VALIDATED", "CAPTURED"]:
            print(f"PASS (Status: {pos_result.get('status')}, Validation: {pos_result.get('validation_status')})")
            print("\nSmoke Test Raw Evidence Paths:")
            print(f"  Session ID:   {pos_session.session_id}")
            print(f"  Screenshot:   {test_run_dir}/sessions/{pos_session.session_id}/screenshot.png")
            print(f"  OCR Output:   {test_run_dir}/sessions/{pos_session.session_id}/ocr.json")
            print(f"  Result JSON:  {test_run_dir}/sessions/{pos_session.session_id}/result.json")
            return True
        else:
            print(f"FAIL (Status: {pos_result.get('status')}, Validation: {pos_result.get('validation_status')})")
            return False
    finally:
        pos_session.close()


if __name__ == "__main__":
    success = run_smoke_tests()
    sys.exit(0 if success else 1)
