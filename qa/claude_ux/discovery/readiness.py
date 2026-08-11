#!/usr/bin/env python3
"""
Automated Readiness State Machine & Security Gate Solver
Executes strict conjunctive readiness verification: LAUNCHING -> WINDOW_READY -> CLAUDE_PROCESS_READY -> SECURITY_GATE -> HOME_PROMPT -> PROMPT_READY.
Auto-resolves security gate prompts ('Yes, I trust this folder') by auto-pressing Enter and polling until PROMPT_READY.
"""

import time
from typing import Dict, Tuple, List


class ReadinessStateMachine:
    """Manages session launch readiness transitions and auto-resolves workspace security prompts."""

    SECURITY_GATE_MARKERS = [
        "trust this directory", "trust workspace", "trust this folder", "allow access", "security prompt", "security guide"
    ]

    IDENTITY_MARKERS = [
        "claude code", "welcome back", "welcome", "opus 5", "opust 5", "claude v", "claude"
    ]

    PROMPT_MARKERS = [
        "❯", ">", "run /init", "what's new", "how can claude help", "tips for getting started"
    ]

    def __init__(self, driver):
        self.driver = driver
        self.current_state = "LAUNCHING"

    def evaluate_readiness(self, launch_verif: Dict) -> Tuple[bool, str, str]:
        """Evaluates strict conjunctive readiness state machine and auto-resolves security gate if detected."""
        if not launch_verif.get("window_found"):
            self.current_state = "INVALID_WINDOW"
            return False, self.current_state, "Terminal window creation failed"

        self.current_state = "WINDOW_READY"

        if not launch_verif.get("claude_process_detected"):
            self.current_state = "INVALID_PROCESS"
            return False, self.current_state, "Claude process PID not detected"

        self.current_state = "CLAUDE_PROCESS_READY"

        # Poll text for up to 8.0 seconds to resolve security prompt or startup delay
        for attempt in range(10):
            text_lines = self.driver.get_terminal_text()
            full_text = " ".join(text_lines).lower()

            # Check for Security Gate prompt
            is_security_gate = any(marker in full_text for marker in self.SECURITY_GATE_MARKERS)
            if is_security_gate:
                print(f"[Readiness] Security gate detected ('Yes, I trust this folder'). Auto-pressing Enter...")
                self.driver.press_key("enter")
                time.sleep(1.0)
                continue

            has_identity = any(m in full_text for m in self.IDENTITY_MARKERS)
            has_prompt = any(m in full_text for m in self.PROMPT_MARKERS)

            # Strict readiness predicate: MUST have both identity AND explicit prompt/interactive marker
            if has_identity and has_prompt:
                self.current_state = "PROMPT_READY"
                return True, self.current_state, "Successfully reached PROMPT_READY"

            time.sleep(0.8)

        self.current_state = "UNAVAILABLE"
        return False, self.current_state, "Terminal text did not satisfy strict conjunctive HOME_PROMPT_READY"
