#!/usr/bin/env python3
"""
Negative Test Suite for Canonical Exact-Grid Comparator (diff_exact_grid_frames)

Verifies that ANY visual, style, cursor, dimension, or semantic mutation between
an Oracle frame and a Target frame is reliably caught and classified.
"""

import sys
import copy
import unittest

sys.path.insert(0, "scripts")
from oracle_parity_engine import CanonicalCell, CanonicalFrame, diff_exact_grid_frames

def make_sample_frame(cols: int = 80, rows: int = 24) -> CanonicalFrame:
    """Constructs a deterministic, sample canonical frame."""
    grid = []
    lines = []
    for y in range(rows):
        row_cells = []
        for x in range(cols):
            if y == 0 and x < 15:
                ch = "Claude Code v2"[x] if x < len("Claude Code v2") else " "
                cell = CanonicalCell(char=ch, fg="blue", bg="default", bold=True)
            elif y == 5 and x < 10:
                ch = "❯ 2. Dark"[x] if x < len("❯ 2. Dark") else " "
                cell = CanonicalCell(char=ch, fg="default", bg="default", bold=(x == 0))
            else:
                cell = CanonicalCell(char=" ", fg="default", bg="default")
            row_cells.append(cell)
        grid.append(row_cells)
        lines.append("".join(c.char for c in row_cells))

    return CanonicalFrame(
        stage_index=5,
        stage_name="5_DEFAULT_SELECTION_FOCUS",
        cols=cols,
        rows=rows,
        grid=grid,
        screen_lines=lines,
        cursor_position=(5, 0),
        cursor_visible=True,
        active_modal="ThemePicker",
        focused_option="2. Dark mode",
        focused_option_index=2,
        options_catalog=["1. Auto", "2. Dark mode", "3. Light mode"],
        checked_options=["2. Dark mode"],
        composer_present=True,
        suggestions_present=False,
        terminal_modes=["224"],
        raw_pty_bytes=b"\x1b[?25h"
    )

class TestOracleComparator(unittest.TestCase):

    def setUp(self):
        self.cols = 80
        self.rows = 24
        self.oracle = make_sample_frame(self.cols, self.rows)

    def test_01_exact_match(self):
        """Identical frames must pass with EXACT_PARITY."""
        target = copy.deepcopy(self.oracle)
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertTrue(diff.passed)
        self.assertEqual(diff.divergence_type, "EXACT_PARITY")

    def test_02_single_character_difference(self):
        """Single character difference must fail with GRID_CHAR and exact coordinates."""
        target = copy.deepcopy(self.oracle)
        target.grid[0][0].char = "X"
        target.screen_lines[0] = "X" + target.screen_lines[0][1:]
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "GRID_CHAR")
        self.assertEqual(diff.first_mismatch_cell, (0, 0))
        self.assertEqual(diff.expected_value, "C")
        self.assertEqual(diff.actual_value, "X")

    def test_03_trailing_space_difference(self):
        """Trailing space difference must fail and NEVER be ignored by rstrip()."""
        target = copy.deepcopy(self.oracle)
        # Add a trailing 'X' character at the very end of row 0
        target.grid[0][79].char = " "
        self.oracle.grid[0][79].char = "X"
        self.oracle.screen_lines[0] = self.oracle.screen_lines[0][:79] + "X"
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertIn(diff.divergence_category, ["TRAILING_SPACE", "GRID_CHAR"])
        self.assertEqual(diff.first_mismatch_cell, (0, 79))

    def test_04_blank_row_difference(self):
        """Blank row vs non-blank row must fail with BLANK_ROW."""
        target = copy.deepcopy(self.oracle)
        # Row 10 is blank in oracle. Put a character in target.
        target.grid[10][5].char = "!"
        target.screen_lines[10] = target.screen_lines[10][:5] + "!" + target.screen_lines[10][6:]
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "BLANK_ROW")
        self.assertEqual(diff.first_mismatch_cell, (10, 5))

    def test_05_cursor_position_y_difference(self):
        """Cursor row mismatch must fail with CURSOR_POS."""
        target = copy.deepcopy(self.oracle)
        target.cursor_position = (6, 0)
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "CURSOR_POS")
        self.assertEqual(diff.expected_value, (5, 0))
        self.assertEqual(diff.actual_value, (6, 0))

    def test_06_cursor_position_x_difference(self):
        """Cursor column mismatch must fail with CURSOR_POS."""
        target = copy.deepcopy(self.oracle)
        target.cursor_position = (5, 2)
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "CURSOR_POS")
        self.assertEqual(diff.expected_value, (5, 0))
        self.assertEqual(diff.actual_value, (5, 2))

    def test_07_cursor_visibility_difference(self):
        """Cursor visibility mismatch must fail with CURSOR_VISIBILITY."""
        target = copy.deepcopy(self.oracle)
        target.cursor_visible = False
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "CURSOR_VISIBILITY")
        self.assertEqual(diff.expected_value, True)
        self.assertEqual(diff.actual_value, False)

    def test_08_foreground_color_difference(self):
        """Cell foreground color mismatch must fail with GRID_STYLE."""
        target = copy.deepcopy(self.oracle)
        target.grid[0][0].fg = "red"  # Oracle has blue
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "GRID_STYLE")
        self.assertEqual(diff.first_mismatch_cell, (0, 0))
        self.assertEqual(diff.expected_value["fg"], "blue")
        self.assertEqual(diff.actual_value["fg"], "red")

    def test_09_background_color_difference(self):
        """Cell background color mismatch must fail with GRID_STYLE."""
        target = copy.deepcopy(self.oracle)
        target.grid[0][0].bg = "white"
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "GRID_STYLE")
        self.assertEqual(diff.first_mismatch_cell, (0, 0))
        self.assertEqual(diff.expected_value["bg"], "default")
        self.assertEqual(diff.actual_value["bg"], "white")

    def test_10_bold_style_difference(self):
        """Bold attribute mismatch must fail with GRID_STYLE."""
        target = copy.deepcopy(self.oracle)
        target.grid[0][0].bold = False  # Oracle has bold=True
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "GRID_STYLE")
        self.assertEqual(diff.first_mismatch_cell, (0, 0))
        self.assertEqual(diff.expected_value["bold"], True)
        self.assertEqual(diff.actual_value["bold"], False)

    def test_11_underscore_style_difference(self):
        """Underscore attribute mismatch must fail with GRID_STYLE."""
        target = copy.deepcopy(self.oracle)
        target.grid[0][0].underscore = True
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "GRID_STYLE")
        self.assertEqual(diff.first_mismatch_cell, (0, 0))

    def test_12_modal_difference(self):
        """Active modal mismatch must fail with MODAL."""
        target = copy.deepcopy(self.oracle)
        target.active_modal = None
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "MODAL")
        self.assertEqual(diff.expected_value, "ThemePicker")
        self.assertEqual(diff.actual_value, None)

    def test_13_focus_index_difference(self):
        """Focused option index mismatch must fail with FOCUS."""
        target = copy.deepcopy(self.oracle)
        target.focused_option_index = 1  # Oracle has 2
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "FOCUS")
        self.assertEqual(diff.expected_value, 2)
        self.assertEqual(diff.actual_value, 1)

    def test_14_options_catalog_ordering_difference(self):
        """Catalog option reordering must fail with CATALOG_ORDER."""
        target = copy.deepcopy(self.oracle)
        target.options_catalog = ["2. Dark mode", "1. Auto", "3. Light mode"]
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "CATALOG_ORDER")

    def test_15_missing_checkmark_difference(self):
        """Missing checkmark on active option must fail with CHECKMARK."""
        target = copy.deepcopy(self.oracle)
        target.checked_options = []
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "CHECKMARK")

    def test_16_composer_presence_difference(self):
        """Composer presence mismatch must fail with COMPOSER."""
        target = copy.deepcopy(self.oracle)
        target.composer_present = False
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "COMPOSER")

    def test_17_suggestions_presence_difference(self):
        """Suggestions presence mismatch must fail with SUGGESTIONS."""
        target = copy.deepcopy(self.oracle)
        target.suggestions_present = True
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "SUGGESTIONS")

    def test_18_dimensions_difference(self):
        """Dimension mismatch must fail with DIMENSIONS."""
        target = copy.deepcopy(self.oracle)
        target.rows = 25
        
        diff = diff_exact_grid_frames(self.oracle, target, self.cols, self.rows)
        self.assertFalse(diff.passed)
        self.assertEqual(diff.divergence_category, "DIMENSIONS")

if __name__ == "__main__":
    unittest.main()
