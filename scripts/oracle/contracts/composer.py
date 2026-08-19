"""
Phase 2: Composer & Text Input Capability Contract
"""

from typing import List
from .base import CapabilityContract, StageSpec


class ComposerContract(CapabilityContract):
    name = "composer"
    description = "Phase 2: Composer text input, cursor navigation, inline editing, line killing, and autocomplete suggestions"

    def get_stages(self) -> List[StageSpec]:
        return [
            StageSpec(
                index=1,
                name="1_INITIAL_COMPOSER_MOUNT",
                action_type="wait",
                input_bytes=b"",
                settle_time_ms=300,
                wait_predicate=lambda s: any(l.strip().startswith("❯") for l in s.display),
                description="Composer prompt mounts with empty input buffer and cursor placed after prompt"
            ),
            StageSpec(
                index=2,
                name="2_TYPE_BASIC_TEXT",
                action_type="type",
                input_bytes=b"echo hello world",
                settle_time_ms=300,
                wait_predicate=lambda s: any("echo hello world" in l for l in s.display),
                description="Typing single-line text populates input buffer and advances cursor"
            ),
            StageSpec(
                index=3,
                name="3_ARROW_CURSOR_NAVIGATION",
                action_type="key",
                input_bytes=b"\x1b[D" * 5,  # 5 Left Arrows: moves cursor between 'hello ' and 'world'
                settle_time_ms=300,
                wait_predicate=lambda s: any("echo hello world" in l for l in s.display),
                description="Left arrow moves cursor backwards within text buffer without modifying characters"
            ),
            StageSpec(
                index=4,
                name="4_INLINE_INSERTION",
                action_type="type",
                input_bytes=b"beautiful ",
                settle_time_ms=300,
                wait_predicate=lambda s: any("echo hello beautiful world" in l for l in s.display),
                description="Typing text at cursor position inserts characters inline"
            ),
            StageSpec(
                index=5,
                name="5_CLEAR_LINE_CTRL_U",
                action_type="key",
                input_bytes=b"\x05\x15",  # Ctrl+E (move to end) + Ctrl+U (kill line)
                settle_time_ms=400,
                wait_predicate=lambda s: any("Ctrl+Y to paste deleted" in l for l in s.display) and not any("echo" in l for l in s.display),
                description="Ctrl+E moves to end and Ctrl+U kills line buffer, showing paste hint and empty prompt"
            ),
            StageSpec(
                index=6,
                name="6_TYPE_THEME_SUGGESTIONS",
                action_type="type",
                input_bytes=b"/theme",
                settle_time_ms=400,
                wait_predicate=lambda s: any("Change the theme" in l for l in s.display),
                description="Typing /theme displays command suggestion dropdown"
            ),
            StageSpec(
                index=7,
                name="7_DISMISS_SUGGESTIONS_ESC",
                action_type="key",
                input_bytes=b"\x1b",  # Esc dismisses dropdown
                settle_time_ms=400,
                wait_predicate=lambda s: not any("Change the theme" in l for l in s.display) and any("/theme" in l for l in s.display),
                description="Esc dismisses suggestion dropdown while preserving typed text"
            ),
            StageSpec(
                index=8,
                name="8_CLEAR_BUFFER_CTRL_U",
                action_type="key",
                input_bytes=b"\x05\x15",  # Ctrl+E + Ctrl+U kills '/theme'
                settle_time_ms=400,
                wait_predicate=lambda s: any(l.strip().startswith("❯") for l in s.display) and not any("/theme" in l for l in s.display),
                description="Ctrl+E and Ctrl+U clears typed buffer and restores empty prompt"
            ),
        ]
