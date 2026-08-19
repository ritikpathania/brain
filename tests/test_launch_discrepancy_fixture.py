#!/usr/bin/env python3
"""
Regression Fixture for Launch Discrepancy (Screenshot Divergence Reproduction)

Tests that any divergence in the initial launch state (such as shell invocation
output, command echo in scrollback, banner line shift, or prompt cursor position)
is deterministically detected as a failure by diff_exact_grid_frames().
"""

import sys
import unittest

sys.path.insert(0, "scripts")
from oracle_parity_engine import CanonicalCell, CanonicalFrame, diff_exact_grid_frames

class TestLaunchDiscrepancyFixture(unittest.TestCase):

    def setUp(self):
        self.cols = 80
        self.rows = 24

        # 1. Construct canonical reference Claude initial launch frame
        claude_grid = []
        claude_lines = []
        for y in range(self.rows):
            row = []
            for x in range(self.cols):
                if y == 0:
                    ch = "╭─── Claude Code v2.1.233 "[x] if x < len("╭─── Claude Code v2.1.233 ") else "─"
                    if x == 79: ch = "╮"
                    row.append(CanonicalCell(char=ch, fg="default", bg="default"))
                elif y == 13:
                    row.append(CanonicalCell(char="─", fg="default", bg="default"))
                elif y == 14 and x < 2:
                    ch = "❯ "[x]
                    row.append(CanonicalCell(char=ch, fg="default", bg="default"))
                elif y == 15:
                    row.append(CanonicalCell(char="─", fg="default", bg="default"))
                elif y == 16 and x < len("  ⏸ manual mode on · ? for shortcuts · ← for agents"):
                    ch = "  ⏸ manual mode on · ? for shortcuts · ← for agents"[x]
                    row.append(CanonicalCell(char=ch, fg="default", bg="default"))
                else:
                    row.append(CanonicalCell(char=" ", fg="default", bg="default"))
            claude_grid.append(row)
            claude_lines.append("".join(c.char for c in row))

        self.oracle_launch_frame = CanonicalFrame(
            stage_index=1,
            stage_name="1_INITIAL_PROMPT",
            cols=self.cols,
            rows=self.rows,
            grid=claude_grid,
            screen_lines=claude_lines,
            cursor_position=(14, 2),
            cursor_visible=True,
            active_modal=None,
            focused_option=None,
            focused_option_index=None,
            options_catalog=[],
            checked_options=[],
            composer_present=True,
            suggestions_present=False,
            terminal_modes=["224"],
            raw_pty_bytes=b""
        )

    def test_reproduce_shell_invocation_echo_divergence(self):
        """
        If target frame contains shell invocation echo (e.g. '$ bun run ...')
        occupying the top rows instead of a clean LogoCard at row 0,
        the comparator MUST detect it immediately at row 0, col 0.
        """
        brain_grid = []
        brain_lines = []
        for y in range(self.rows):
            row = []
            for x in range(self.cols):
                if y == 0 and x < len("$ bun run --feature AUTO_THEME src/main.tsx"):
                    ch = "$ bun run --feature AUTO_THEME src/main.tsx"[x]
                    row.append(CanonicalCell(char=ch, fg="default", bg="default"))
                elif y == 2 and x < len("╭─── Claude Code v2.1.233"):
                    ch = "╭─── Claude Code v2.1.233"[x]
                    row.append(CanonicalCell(char=ch, fg="default", bg="default"))
                else:
                    row.append(CanonicalCell(char=" ", fg="default", bg="default"))
            brain_grid.append(row)
            brain_lines.append("".join(c.char for c in row))

        brain_dirty_launch = CanonicalFrame(
            stage_index=1,
            stage_name="1_INITIAL_PROMPT",
            cols=self.cols,
            rows=self.rows,
            grid=brain_grid,
            screen_lines=brain_lines,
            cursor_position=(16, 2),  # Shifted down
            cursor_visible=True,
            active_modal=None,
            focused_option=None,
            focused_option_index=None,
            options_catalog=[],
            checked_options=[],
            composer_present=True,
            suggestions_present=False,
            terminal_modes=["224"],
            raw_pty_bytes=b""
        )

        diff = diff_exact_grid_frames(self.oracle_launch_frame, brain_dirty_launch, self.cols, self.rows)
        self.assertFalse(diff.passed, "Comparator must fail on dirty launch state")
        # Must detect cursor shift or row 0 character divergence
        self.assertIn(diff.divergence_category, ["CURSOR_POS", "GRID_CHAR"])
        print(f"✔ Confirmed: Launch discrepancy detected ({diff.divergence_category}): {diff.summary}")

    def test_reproduce_prompt_line_offset_divergence(self):
        """
        If target prompt line is rendered at row 13 instead of row 14,
        the comparator MUST detect it with exact cell coordinates.
        """
        brain_grid = []
        brain_lines = []
        for y in range(self.rows):
            row = []
            for x in range(self.cols):
                if y == 13 and x < 2:  # Shifted up by 1 row
                    ch = "❯ "[x]
                    row.append(CanonicalCell(char=ch, fg="default", bg="default"))
                else:
                    row.append(CanonicalCell(char=" ", fg="default", bg="default"))
            brain_grid.append(row)
            brain_lines.append("".join(c.char for c in row))

        brain_offset_launch = CanonicalFrame(
            stage_index=1,
            stage_name="1_INITIAL_PROMPT",
            cols=self.cols,
            rows=self.rows,
            grid=brain_grid,
            screen_lines=brain_lines,
            cursor_position=(13, 2),  # Row 13 instead of Row 14
            cursor_visible=True,
            active_modal=None,
            focused_option=None,
            focused_option_index=None,
            options_catalog=[],
            checked_options=[],
            composer_present=True,
            suggestions_present=False,
            terminal_modes=["224"],
            raw_pty_bytes=b""
        )

        diff = diff_exact_grid_frames(self.oracle_launch_frame, brain_offset_launch, self.cols, self.rows)
        self.assertFalse(diff.passed, "Comparator must fail on prompt row offset")
        self.assertEqual(diff.divergence_category, "CURSOR_POS")
        self.assertEqual(diff.expected_value, (14, 2))
        self.assertEqual(diff.actual_value, (13, 2))
        print(f"✔ Confirmed: Prompt offset detected: {diff.summary}")

if __name__ == "__main__":
    unittest.main()
