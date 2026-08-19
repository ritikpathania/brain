"""
16-Stage Certified /theme Capability Contract
"""

from typing import List
from .base import CapabilityContract, StageSpec


class ThemeContract(CapabilityContract):
    name = "theme"
    description = "16-stage /theme interactive lifecycle contract"

    def get_stages(self) -> List[StageSpec]:
        return [
            StageSpec(
                index=1,
                name="1_INITIAL_PROMPT",
                action_type="wait",
                input_bytes=b"",
                settle_time_ms=300,
                wait_predicate=lambda s: any(l.strip().startswith("❯") for l in s.display) and any("Claude Code v" in l for l in s.display),
                description="Initial prompt mounts cleanly"
            ),
            StageSpec(
                index=2,
                name="2_TYPE_THEME_SUGGESTIONS",
                action_type="type",
                input_bytes=b"/theme",
                settle_time_ms=400,
                wait_predicate=lambda s: any("Change the theme" in l for l in s.display),
                description="Typing /theme displays command suggestion dropdown"
            ),
            StageSpec(
                index=3,
                name="3_ENTER_MOUNT_THEMEPICKER",
                action_type="key",
                input_bytes=b"\r",
                settle_time_ms=600,
                wait_predicate=lambda s: any("Choose the text style" in l for l in s.display) and any("1. Auto" in l for l in s.display),
                description="Submitting Enter unmounts composer and mounts ThemePicker"
            ),
            StageSpec(
                index=4,
                name="4_OPTION_CATALOG_ORDERING",
                action_type="wait",
                input_bytes=b"",
                settle_time_ms=300,
                wait_predicate=lambda s: any("1. Auto" in l for l in s.display) and any("8. New custom theme" in l for l in s.display),
                description="ThemePicker renders options catalog in canonical order"
            ),
            StageSpec(
                index=5,
                name="5_DEFAULT_SELECTION_FOCUS",
                action_type="wait",
                input_bytes=b"",
                settle_time_ms=200,
                wait_predicate=lambda s: any("❯ 2. Dark mode" in l for l in s.display),
                description="ThemePicker initializes with Option 2 focused by default"
            ),
            StageSpec(
                index=6,
                name="6_ARROW_NAV_FOCUS_MOVE",
                action_type="key",
                input_bytes=b"\x1b[B",
                settle_time_ms=400,
                wait_predicate=lambda s: any("❯ 3. Light mode" in l for l in s.display),
                description="Arrow Down advances focus to Option 3 (Light mode)"
            ),
            StageSpec(
                index=7,
                name="7_SYNTAX_PREVIEW_VISIBLE",
                action_type="wait",
                input_bytes=b"",
                settle_time_ms=300,
                wait_predicate=lambda s: any("function greet" in l for l in s.display) and any("Syntax theme:" in l for l in s.display),
                description="Syntax diff preview box is rendered below options list"
            ),
            StageSpec(
                index=8,
                name="8_TOGGLE_SYNTAX_HIGHLIGHT_CTRL_T",
                action_type="key",
                input_bytes=b"\x14",  # Ctrl+T
                settle_time_ms=400,
                wait_predicate=lambda s: any("ctrl+t to enable" in l for l in s.display),
                description="Ctrl+T disables syntax highlighting"
            ),
            StageSpec(
                index=9,
                name="9_RESTORE_SYNTAX_HIGHLIGHT_CTRL_T",
                action_type="key",
                input_bytes=b"\x14",  # Ctrl+T restore
                settle_time_ms=400,
                wait_predicate=lambda s: any("ctrl+t to disable" in l for l in s.display),
                description="Ctrl+T re-enables syntax highlighting"
            ),
            StageSpec(
                index=10,
                name="10_ESCAPE_CANCELLATION",
                action_type="key",
                input_bytes=b"\x1b",
                settle_time_ms=500,
                wait_predicate=lambda s: any("Theme picker dismissed" in l for l in s.display) and any(l.strip().startswith("❯") for l in s.display),
                description="Esc dismisses ThemePicker and prints dismiss confirmation"
            ),
            StageSpec(
                index=11,
                name="11_REMOUNT_THEMEPICKER",
                action_type="type_and_enter",
                input_bytes=b"/theme\r",
                settle_time_ms=600,
                wait_predicate=lambda s: any("Choose the text style" in l for l in s.display),
                description="Re-typing /theme + Enter cleanly re-mounts ThemePicker"
            ),
            StageSpec(
                index=12,
                name="12_NAVIGATE_TO_LIGHT_MODE",
                action_type="key",
                input_bytes=b"\x1b[B",
                settle_time_ms=400,
                wait_predicate=lambda s: any("❯ 3. Light mode" in l for l in s.display),
                description="Arrow Down selects Option 3 (Light mode)"
            ),
            StageSpec(
                index=13,
                name="13_ENTER_COMMIT_THEME",
                action_type="key",
                input_bytes=b"\r",
                settle_time_ms=600,
                wait_predicate=lambda s: any("Theme set to light" in l for l in s.display),
                description="Enter commits theme selection and outputs confirmation"
            ),
            StageSpec(
                index=14,
                name="14_COMPOSER_RESTORED",
                action_type="wait",
                input_bytes=b"",
                settle_time_ms=400,
                wait_predicate=lambda s: any("Theme set to light" in l for l in s.display) and any(l.strip().startswith("❯") for l in s.display),
                description="Composer prompt is fully restored after theme commit"
            ),
            StageSpec(
                index=15,
                name="15_DISK_PERSISTENCE",
                action_type="assert_disk",
                input_bytes=b"",
                settle_time_ms=300,
                wait_predicate=None,
                description="Selected theme is persisted to configuration file"
            ),
            StageSpec(
                index=16,
                name="16_RESTART_CHECKMARK_PRESERVED",
                action_type="restart",
                input_bytes=b"/theme",
                settle_time_ms=800,
                wait_predicate=lambda s: any("3. Light mode ✔" in l for l in s.display),
                description="Process restart loads persisted theme and renders checkmark on Option 3"
            ),
        ]
