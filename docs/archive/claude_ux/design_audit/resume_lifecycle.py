#!/usr/bin/env python3
"""
Sentinel Session Resume Lifecycle Tester
Tests session resume functionality by writing a unique sentinel prompt string,
quitting Claude, extracting the session resume ID from the goodbye screen or banner,
re-launching via `claude --resume <session_id>`, and verifying sentinel restoration & session identity.
"""

import os
import sys
import time
import json
import uuid
import re
from typing import Optional
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from qa.claude_ux.driver.session import ClaudeSession, SessionResult
from qa.claude_ux.discovery.readiness import ReadinessStateMachine
from qa.claude_ux.design_audit.state_machine import classify_evidence


CONTEXTUAL_PATTERNS = [
    re.compile(r"claude\s+--resume\s+([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})", re.IGNORECASE),
    re.compile(r"Session\s+ID:\s+([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})", re.IGNORECASE),
]


def extract_contextual_uuid(text: str) -> Optional[str]:
    """Extracts and validates a UUID strictly from observed contextual patterns:
    - 'claude --resume <UUID>'
    - 'Session ID: <UUID>'

    Validates candidate using uuid.UUID(). Unrelated UUIDs appearing elsewhere
    in the buffer are ignored.
    """
    if not text:
        return None
    for pattern in CONTEXTUAL_PATTERNS:
        match = pattern.search(text)
        if match:
            candidate = match.group(1)
            try:
                val = uuid.UUID(candidate)
                return str(val)
            except ValueError:
                continue
    return None


class ResumeLifecycleTester:
    """Tests sentinel conversation restoration and session identity persistence across session resume."""

    SENTINEL_PROMPT = "CLAUDE_UX_ATLAS_RESUME_SENTINEL_7F3A"

    def __init__(self, run_dir: Path, *, transport_verified: bool):
        self.run_dir = run_dir
        self.transport_verified = transport_verified
        self.session_results = []

    def test_resume_lifecycle(self, viewport: tuple = (80, 24)) -> tuple:
        print("=== Starting Sentinel Session Resume Lifecycle Tester ===")
        session = ClaudeSession(self.run_dir, "resume_tester", viewport)
        try:
            if not session.launch():
                session.mark_failed()
                print("[Resume Tester Error] Launch failed")
                return {}, self.session_results

            readiness = ReadinessStateMachine(session.driver)
            ok, _, _ = readiness.evaluate_readiness(session.launch_record)
            if not ok:
                session.mark_failed()
                print("[Resume Tester Error] Readiness failed")
                return {}, self.session_results

            # 1. Type unique sentinel prompt into session
            print(f"  Typing Sentinel Prompt: '{self.SENTINEL_PROMPT}'...")
            session.type(self.SENTINEL_PROMPT)
            time.sleep(0.5)

            # 2. Exit session via /quit to observe goodbye screen and session ID
            print("  Exiting session via '/quit' to capture resume session ID...")
            session.press_key("esc")
            time.sleep(0.2)
            session.type("/quit")
            session.press_key("enter")
            time.sleep(1.0)

            post_exit_lines = session.observe_terminal_state()
            post_exit_text = " ".join(post_exit_lines)

            # Strict contextual UUID extraction from observed exit text
            session_id = extract_contextual_uuid(post_exit_text)

            if not session_id:
                session.mark_failed()
                print("[Resume Tester] Session ID UUID extraction failed — returning UNAVAILABLE")
                resume_record = {
                    "sentinel_prompt": self.SENTINEL_PROMPT,
                    "session_id_extracted": False,
                    "evidence_classification": "UNAVAILABLE"
                }
                out_json = self.run_dir / "resume_lifecycle_results.json"
                with open(out_json, "w") as f:
                    json.dump(resume_record, f, indent=2)
                return resume_record, self.session_results

            print(f"  Extracted Real Resume Session ID: {session_id}")
            session.mark_completed()
        finally:
            self.session_results.append(session.make_result())
            session.close()
            time.sleep(0.4)

        # 3. Execute claude --resume <session_id> in fresh Terminal tab
        print(f"  Executing 'claude --resume {session_id}' in new Terminal tab...")
        resume_session = ClaudeSession(self.run_dir, "resume_tester_restored", viewport)
        try:
            if not resume_session.launch(f"claude --resume {session_id}"):
                resume_session.mark_failed()
                return {}, self.session_results

            readiness_r = ReadinessStateMachine(resume_session.driver)
            ok_r, _, _ = readiness_r.evaluate_readiness(resume_session.launch_record)
            if not ok_r:
                resume_session.mark_failed()
                return {}, self.session_results

            resumed_lines = resume_session.observe_terminal_state()
            resumed_text = " ".join(resumed_lines)

            # ── Sentinel conversation restoration ────────────────────────────────
            sentinel_restored = self.SENTINEL_PROMPT in resumed_text
            sentinel_conv_ev = classify_evidence(
                action_executed=True,
                transport_verified=self.transport_verified,
                parent_state_known=True,
                post_state_observed=True,
                transition_matches_expectation=sentinel_restored
            )

            # ── Session identity (independent from sentinel restoration) ─────────
            # Strict contextual UUID extraction from resumed session buffer
            resumed_id = extract_contextual_uuid(resumed_text)

            session_identity_ev = classify_evidence(
                action_executed=resumed_id is not None,
                transport_verified=self.transport_verified,
                parent_state_known=True,
                post_state_observed=resumed_id is not None,
                transition_matches_expectation=(
                    resumed_id == session_id if resumed_id is not None else False
                )
            )

            if sentinel_conv_ev == "VERIFIED":
                resume_session.mark_completed()
            else:
                resume_session.mark_failed()

            resume_record = {
                "sentinel_prompt": self.SENTINEL_PROMPT,
                "session_id_before": session_id,
                "session_id_after": resumed_id,        # independently observed, never copied
                "session_identity": session_identity_ev,
                "sentinel_conversation_restored": sentinel_conv_ev,
                "evidence_classification": sentinel_conv_ev,
            }

            out_json = self.run_dir / "resume_lifecycle_results.json"
            with open(out_json, "w") as f:
                json.dump(resume_record, f, indent=2)

            print(f"Resume Lifecycle Test Complete (sentinel={sentinel_conv_ev}, identity={session_identity_ev}). Saved to {out_json}")
            return resume_record, self.session_results
        finally:
            self.session_results.append(resume_session.make_result())
            resume_session.close()
            time.sleep(0.4)
