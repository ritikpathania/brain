#!/usr/bin/env python3
"""
Structural Screen State Machine, Multi-Line Screen Chrome & Expectation-Aware Evidence Classifier
Models TUI screen states strictly from structural geometry, focus, overlay identity,
navigation region, prompt mode, multi-line status bar footer grammar, and StateExpectation matching.
"""

import re
import hashlib
from dataclasses import dataclass, asdict
from typing import List, Tuple, Dict, Any, Optional


@dataclass
class StateExpectation:
    """Explicit expected transition outcome for evidence verification."""
    kind: str  # "CHANGED_TO" | "UNCHANGED" | "SELECTION_MOVED"
    expected_screen: Optional[str] = None
    expected_selection_index: Optional[int] = None


def classify_evidence(
    *,
    action_executed: bool,
    transport_verified: bool,
    parent_state_known: bool,
    post_state_observed: bool,
    transition_matches_expectation: bool,
    unsafe: bool = False
) -> str:
    """Enforces strict 5-predicate evidence classification.

    Tiers (in evaluation order):
        UNSAFE_TO_TEST     — action was flagged as destructive/untestable
        UNAVAILABLE        — action not executed or parent/post state unknown
        UNVERIFIED_TRANSPORT — transport probe did not confirm key delivery
        VERIFIED           — all five predicates true
        FAILED             — transport ok but observed transition didn't match

    VERIFIED ⇔
        action_executed
        AND transport_verified
        AND parent_state_known
        AND post_state_observed
        AND transition_matches_expectation
    """
    if unsafe:
        return "UNSAFE_TO_TEST"
    if not action_executed:
        return "UNAVAILABLE"
    if not (parent_state_known and post_state_observed):
        return "UNAVAILABLE"
    if not transport_verified:
        return "UNVERIFIED_TRANSPORT"
    if transition_matches_expectation:
        return "VERIFIED"
    return "FAILED"


@dataclass
class StatusBarControl:
    """Represents a screen-local status bar / footer shortcut control."""
    key: str
    label: str
    action: str
    evidence: str  # STATUS_ADVERTISED | SOURCE_CONFIRMED | OBSERVED | VERIFIED | FAILED | UNAVAILABLE | UNSAFE_TO_TEST


@dataclass
class ScreenChrome:
    """Represents the structural chrome of a TUI screen."""
    header_region: str
    body_region: str
    prompt_region: str
    overlay_region: str
    footer_region: str
    footer_controls: List[Dict[str, Any]]


@dataclass(frozen=True)
class ScreenFingerprint:
    """Hashable representation of a structural screen state."""
    screen_category: str
    focused_element: str
    overlay_identity: str
    navigation_region: str
    structural_geometry: str
    prompt_mode: str

    def state_id(self) -> str:
        raw = f"{self.screen_category}|{self.focused_element}|{self.overlay_identity}|{self.navigation_region}|{self.structural_geometry}|{self.prompt_mode}"
        h = hashlib.sha256(raw.encode("utf-8")).hexdigest()[:10]
        return f"screen_{self.screen_category}_{h}"


@dataclass
class ScreenState:
    """Complete representation of a discovered screen state."""
    screen_id: str
    title: str
    category: str
    chrome: Dict[str, Any]
    focused_element: str
    overlay_identity: str
    navigation_region: str
    structural_geometry: str
    prompt_mode: str
    text_sample: List[str]
    viewport: str
    available_controls: List[Dict[str, Any]]
    path_from_root: List[str]
    selected_index: int = 0

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


class StructuralStateAnalyzer:
    """Analyzes normalized terminal text lines to extract ScreenChrome, Footer Controls, and ScreenFingerprint."""

    @staticmethod
    def parse_footer_controls(footer_text: str) -> List[StatusBarControl]:
        """Parses advertised shortcuts from multi-line status bar footer text."""
        controls = []
        if not footer_text:
            return controls

        patterns = [
            (r"(?:↑↓|up/down|\^\/v)", "up/down", "Navigate / Move selection"),
            (r"enter\s+to\s+([a-z]+)", "enter", "Confirm / Select"),
            (r"esc\s+to\s+([a-z]+)", "esc", "Back / Close overlay"),
            (r"space\s+to\s+([a-z]+)", "space", "Contextual toggle / Reply"),
            (r"ctrl\+x\s+to\s+([a-z]+)", "ctrl+x", "Delete item"),
            (r"tab\s+([a-z]+)", "tab", "Switch panel / Accept completion"),
            (r"\?\s+for\s+shortcuts", "?", "Open quick help"),
            (r"ctrl\+k\s+([a-z]+)", "ctrl+k", "Search commands"),
            (r"left\s+([a-z]+)", "left", "Navigate left panel"),
            (r"right\s+([a-z]+)", "right", "Navigate right panel"),
        ]

        footer_lower = footer_text.lower()
        for pat, key_code, default_action in patterns:
            match = re.search(pat, footer_lower)
            if match:
                action_text = match.group(1) if match.groups() else default_action
                ctrl = StatusBarControl(
                    key=key_code,
                    label=match.group(0),
                    action=action_text.strip().capitalize(),
                    evidence="STATUS_ADVERTISED"
                )
                controls.append(ctrl)

        return controls

    @classmethod
    def analyze(cls, text_lines: List[str], viewport: Tuple[int, int]) -> Tuple[ScreenFingerprint, ScreenChrome, str]:
        full_text = " ".join(text_lines).lower()
        vp_str = f"{viewport[0]}x{viewport[1]}"

        # Multi-line structural region detection
        header_region = text_lines[0] if text_lines else ""
        footer_lines = text_lines[-4:] if len(text_lines) >= 4 else text_lines
        footer_region = "\n".join(footer_lines)
        prompt_region = next((line for line in text_lines if "❯" in line or ">" in line), "")
        body_lines = [line for line in text_lines if line not in [header_region, prompt_region] and line not in footer_lines]
        body_region = " ".join(body_lines[:5])
        overlay_region = "overlay_active" if any(w in full_text for w in ["search commands", "quick safety check", "theme"]) else "none"

        footer_controls = cls.parse_footer_controls(footer_region)
        chrome = ScreenChrome(
            header_region=header_region,
            body_region=body_region,
            prompt_region=prompt_region,
            overlay_region=overlay_region,
            footer_region=footer_region,
            footer_controls=[asdict(c) for c in footer_controls]
        )

        # 1. Overlay Identity from container signatures
        has_slash_in_prompt = any(re.search(r"[❯>]\s*/", line) for line in text_lines)
        overlay = "none"
        if "theme" in full_text and ("dark" in full_text or "light" in full_text):
            overlay = "theme_picker"
        elif "help" in full_text and ("commands" in full_text or "usage" in full_text):
            overlay = "help_overlay"
        elif "search" in full_text and "command" in full_text:
            overlay = "ctrl_k_dialog"
        elif has_slash_in_prompt:
            overlay = "slash_popup"

        # 2. Navigation Region
        nav_region = "main_feed"
        if any(h in full_text for h in ["recent sessions", "workspace sessions", "needs input", "recent chats"]):
            nav_region = "left_panel"

        # 3. Focused Element
        focus = "prompt_input"
        if overlay != "none":
            focus = f"overlay_focus_{overlay}"
        elif nav_region == "left_panel":
            focus = "left_panel_item"

        # 4. Prompt Mode
        prompt_mode = "empty_prompt"
        if overlay == "slash_popup":
            prompt_mode = "slash_completion"
        elif overlay == "ctrl_k_dialog":
            prompt_mode = "ctrl_k_search"
        elif overlay == "theme_picker":
            prompt_mode = "theme_selection"
        elif nav_region == "left_panel":
            prompt_mode = "navigation_focus"

        # 5. Structural Geometry
        border_box = "box_rounded" if "╭" in full_text or "┌" in full_text else "borderless"
        split_layout = "2_panel" if viewport[0] >= 70 else "1_panel"
        geom_str = f"{border_box}_{split_layout}"

        # 6. Screen Category — derived from geometry + grammar, NOT raw word presence.
        footer_lower = footer_region.lower()
        body_lower = body_region.lower()

        # Structural DELETE_CONFIRMATION: box_rounded geometry + destructive commit control
        has_modal_geometry = (border_box == "box_rounded")
        has_destructive_commit = (
            any(v in footer_lower for v in ["delete", "remove", "confirm", "yes", "permanently"])
            and any(c in footer_lower for c in ["cancel", "esc", "no"])
        )
        is_delete_confirmation = has_modal_geometry and has_destructive_commit

        # Structural REPLY_COMPOSER: workspace left-panel context + editable text-entry prompt visible in body
        has_entry_prompt_in_body = (
            nav_region == "left_panel"
            and any(m in body_region for m in ["❯", ">"])
        )
        is_reply_composer = has_entry_prompt_in_body

        category = "01_home"
        if is_delete_confirmation:
            category = "09_delete_confirmation"
        elif is_reply_composer:
            category = "10_reply_composer"
        elif overlay == "slash_popup":
            category = "04_slash_completion"
        elif overlay == "ctrl_k_dialog":
            category = "05_ctrl_k_palette"
        elif overlay == "theme_picker":
            category = "06_theme_picker"
        elif overlay == "help_overlay":
            category = "07_help_surfaces"
        elif nav_region == "left_panel":
            category = "02_navigation_panel"
        elif "assistant" in full_text or "unread" in full_text:
            category = "08_workspace_timeline"

        fingerprint = ScreenFingerprint(
            screen_category=category,
            focused_element=focus,
            overlay_identity=overlay,
            navigation_region=nav_region,
            structural_geometry=geom_str,
            prompt_mode=prompt_mode
        )

        return fingerprint, chrome, vp_str
