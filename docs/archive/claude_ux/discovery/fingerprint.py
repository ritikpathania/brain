#!/usr/bin/env python3
"""
Terminal State Fingerprinter with Comprehensive Structural Noise Normalization
Decouples LogicalStateFingerprint (semantic state identity) from VisualContext (viewport).
Normalizes usernames, hostnames, shell prompts, PIDs, TTYs, binary paths, and timestamps.
"""

import re
import hashlib
from dataclasses import dataclass, asdict
from typing import List, Tuple, Dict, Any


@dataclass(frozen=True)
class LogicalStateFingerprint:
    """Hashable representation of a logical TUI state."""
    semantics_key: str
    controls_key: str
    focus_region: str
    prompt_mode: str
    overlay_identity: str
    footer_grammar: str

    def state_id(self) -> str:
        raw = f"{self.semantics_key}|{self.controls_key}|{self.focus_region}|{self.prompt_mode}|{self.overlay_identity}"
        h = hashlib.sha256(raw.encode("utf-8")).hexdigest()[:10]
        return f"state_{self.prompt_mode}_{h}"


@dataclass(frozen=True)
class VisualContext:
    """Visual geometry context (viewport dimension & layout variant)."""
    viewport_width: int
    viewport_height: int

    def variant_id(self) -> str:
        return f"{self.viewport_width}x{self.viewport_height}"


class TerminalFingerprinter:
    """Constructs LogicalStateFingerprint and VisualContext from terminal text lines and layout metadata."""

    @staticmethod
    def normalize_terminal_text(text_lines: List[str]) -> List[str]:
        """Regex-normalizes dynamic usernames, hostnames, shell prompts, PIDs, TTYs, binary paths, and timestamps."""
        normalized = []
        for line in text_lines:
            # 1. Normalize shell login lines & TTY references
            line = re.sub(r"Last login:.*on ttys\d+", "<login_header>", line)
            line = re.sub(r"on ttys\d+", "on <tty>", line)
            line = re.sub(r"/dev/ttys\d+", "<tty>", line)
            # 2. Normalize shell prompt strings (e.g. ritikpathania@Rickys-MacBook ~ %)
            line = re.sub(r"[a-zA-Z0-9_\-\.]+@[a-zA-Z0-9_\-\.]+\s+~?\s*[%$#]", "<user_prompt>", line)
            # 3. Normalize home directory paths & binary path invocations
            line = re.sub(r"/Users/[a-zA-Z0-9_\-\./]+", "<user_home>", line)
            line = re.sub(r"\S+/claude", "<claude_bin>", line)
            # 4. Normalize PIDs
            line = re.sub(r"\b(pid|PID)\s*:?\s*\d+\b", "<pid>", line)
            # 5. Normalize timestamps (HH:MM:SS)
            line = re.sub(r"\b\d{2}:\d{2}:\d{2}\b", "<time>", line)

            if line.strip():
                normalized.append(line.strip())
        return normalized

    @classmethod
    def compute_fingerprint(cls, text_lines: List[str], viewport: Tuple[int, int]) -> Tuple[LogicalStateFingerprint, VisualContext]:
        norm_lines = cls.normalize_terminal_text(text_lines)
        full_text = " ".join(norm_lines).lower()

        # Prompt mode detection
        prompt_mode = "home_prompt"
        if "/" in full_text and ("session" in full_text or "theme" in full_text or "help" in full_text):
            prompt_mode = "slash_completion"
        elif "search" in full_text and "command" in full_text:
            prompt_mode = "ctrl_k_palette"
        elif "left_nav" in full_text or "workspace" in full_text:
            prompt_mode = "left_nav_workspace"

        # Overlay identity
        overlay = "none"
        if "theme" in full_text and ("dark" in full_text or "light" in full_text):
            overlay = "theme_picker"
        elif "help" in full_text and "commands" in full_text:
            overlay = "help_overview"
        elif "global search" in full_text:
            overlay = "global_search"

        # Focus region
        focus = "prompt_input"
        if overlay != "none":
            focus = f"overlay_{overlay}"

        # Semantics key (sorted top 8 significant words, ignoring noise tokens)
        noise_tokens = {"<login_header>", "<user_prompt>", "<user_home>", "<tty>", "<pid>", "<claude_bin>", "<time>", "claude", "v2.1.226"}
        words = [w for w in full_text.split() if len(w) > 3 and w not in noise_tokens]
        semantics_key = "_".join(sorted(list(set(words)))[:8])

        controls_key = "default_controls"
        footer_grammar = "standard_footer"

        logical_fp = LogicalStateFingerprint(
            semantics_key=semantics_key,
            controls_key=controls_key,
            focus_region=focus,
            prompt_mode=prompt_mode,
            overlay_identity=overlay,
            footer_grammar=footer_grammar
        )

        visual_ctx = VisualContext(
            viewport_width=viewport[0],
            viewport_height=viewport[1]
        )

        return logical_fp, visual_ctx
