#!/usr/bin/env python3
"""
Targeted Design Evidence Capture Policy
Captures PNG screenshots strictly for unique logical ScreenStates, material visual changes,
and representative viewports (~15-30 total design research screenshots).
"""

from typing import Set, Dict, Any


class DesignCapturePolicy:
    """Evaluates design research screenshot capture necessity."""

    def __init__(self):
        self.captured_visual_keys: Set[str] = set()

    def evaluate(self, screen_id: str, viewport_str: str) -> Dict[str, Any]:
        visual_key = f"{screen_id}_{viewport_str}"
        is_new_visual_key = visual_key not in self.captured_visual_keys

        if is_new_visual_key:
            self.captured_visual_keys.add(visual_key)

        return {
            "capture_required": is_new_visual_key,
            "visual_key": visual_key,
            "skip_reason": "none" if is_new_visual_key else "duplicate_visual_screen_key"
        }
