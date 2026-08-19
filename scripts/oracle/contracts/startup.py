"""
Phase 1: Startup & Onboarding Capability Contract
"""

from typing import List
from .base import CapabilityContract, StageSpec


class StartupContract(CapabilityContract):
    name = "startup"
    description = "Phase 1: Startup, onboarding feeds, logo transition, and initial composer state"

    def get_stages(self) -> List[StageSpec]:
        return [
            StageSpec(
                index=1,
                name="1_FIRST_LAUNCH_FULL_LOGO",
                action_type="wait",
                input_bytes=b"",
                settle_time_ms=400,
                wait_predicate=lambda s: any(l.strip().startswith("❯") for l in s.display) and any("Claude Code v" in l for l in s.display),
                description="Fresh startup mounts full Logo box, onboarding feeds, and initial composer prompt"
            ),
            StageSpec(
                index=2,
                name="2_TYPE_TEXT_BUFFER",
                action_type="type",
                input_bytes=b"hello world",
                settle_time_ms=300,
                wait_predicate=lambda s: any("hello world" in l for l in s.display),
                description="Typing text updates composer buffer and moves cursor accurately"
            ),
            StageSpec(
                index=3,
                name="3_BACKSPACE_CLEANUP",
                action_type="key",
                input_bytes=b"\x7f" * 11,
                settle_time_ms=300,
                wait_predicate=lambda s: any(l.strip() == "❯" for l in s.display),
                description="Backspace clears text buffer and restores prompt cursor position"
            ),
            StageSpec(
                index=4,
                name="4_REBOOT_CONDENSED_TRANSITION",
                action_type="restart",
                input_bytes=b"",
                settle_time_ms=800,
                wait_predicate=lambda s: any("▝▜█████▛▘" in l for l in s.display) and any(l.strip().startswith("❯") for l in s.display),
                description="Post-onboarding process restart transitions from full Logo box to CondensedLogo"
            ),
        ]
