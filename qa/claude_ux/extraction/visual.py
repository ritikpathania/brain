#!/usr/bin/env python3
"""
Visual Design & Token Extractor
Categorizes Claude's design language into SOURCE-CONFIRMED, OBSERVED, and INFERRED channels.
"""

import json
from pathlib import Path

VISUAL_TOKENS = {
    "colors": {
        "claude_brand_orange": {
            "value": "rgb(215, 119, 87)",
            "token": "claude",
            "usage": "Logo art, focused borders, active spinner, model name",
            "channel": "SOURCE-CONFIRMED (DESIGN.md)"
        },
        "claude_shimmer": {
            "value": "rgb(235, 159, 127)",
            "token": "claudeShimmer",
            "usage": "Pulsing/breathing spinner animation frame pair",
            "channel": "SOURCE-CONFIRMED (DESIGN.md)"
        },
        "security_purple": {
            "value": "rgb(177, 185, 249)",
            "token": "permission",
            "usage": "Security confirmation dialogs & tool approval borders",
            "channel": "SOURCE-CONFIRMED (DESIGN.md)"
        },
        "default_border_gray": {
            "value": "rgb(136, 136, 136)",
            "token": "promptBorder",
            "usage": "Unfocused text input box borders",
            "channel": "SOURCE-CONFIRMED (DESIGN.md)"
        },
        "selection_blue": {
            "value": "rgb(38, 79, 120)",
            "token": "selectionBg",
            "usage": "Highlighted item in FuzzyPicker and selection menus",
            "channel": "SOURCE-CONFIRMED (DESIGN.md)"
        }
    },
    "borders": {
        "type": "Rounded box borders (┌─┐│└─┘) with UnicodeSupport::Full",
        "nested_boxes": "Avoided; single coherent container preferred",
        "dividers": "Subtle horizontal rules (─) with neutral muted gray",
        "channel": "SOURCE-CONFIRMED (DESIGN.md)"
    },
    "typography": {
        "hierarchy": "Bold header titles, regular text primary, dimmed secondary description",
        "command_names": "Visually distinct aligned columns (e.g. /:<18)",
        "channel": "SOURCE-CONFIRMED (PromptInputFooterSuggestions.tsx)"
    },
    "responsive_breakpoints": {
        "compact_mode": "< 70 columns (vertical stacking)",
        "horizontal_mode": ">= 70 columns (left Clawd <=50 cols, right feed >=30 cols)",
        "quick_open_preview_right": ">= 120 columns",
        "global_search_preview_right": ">= 140 columns",
        "channel": "SOURCE-CONFIRMED (logoV2Utils.ts & GlobalSearchDialog.tsx)"
    }
}


class VisualExtractor:
    """Writes machine-readable visual design extraction specs."""

    def __init__(self, run_dir: Path):
        self.run_dir = run_dir

    def extract_visual_spec(self) -> Path:
        out_path = self.run_dir / "visual_tokens.json"
        with open(out_path, "w") as f:
            json.dump(VISUAL_TOKENS, f, indent=2)
        return out_path
