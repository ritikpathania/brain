"""
Strict Exact-Grid Differential Comparator
"""

import difflib
from dataclasses import dataclass, field
from typing import List, Dict, Tuple, Optional, Any
from .terminal import CanonicalFrame, CanonicalCell


@dataclass
class StageDiff:
    stage_index: int
    stage_name: str
    passed: bool
    divergence_type: str = "EXACT_PARITY"  # EXACT_PARITY, ACTUAL_PRODUCT_GAP, etc.
    divergence_category: str = "NONE"      # GRID_CHAR, GRID_STYLE, CURSOR_POS, MODAL_STATE, DIMENSIONS
    first_mismatch_cell: Optional[Tuple[int, int]] = None
    expected_value: Any = None
    actual_value: Any = None
    summary: str = ""
    diff_lines: List[str] = field(default_factory=list)
    oracle_state: Dict[str, Any] = field(default_factory=dict)
    target_state: Dict[str, Any] = field(default_factory=dict)


def diff_exact_grid_frames(oracle_frame: CanonicalFrame, target_frame: CanonicalFrame, cols: int, rows: int) -> StageDiff:
    """
    STRICT EXACT-GRID COMPARISON:
      1. Dimensions must match exactly (rows x cols).
      2. Cursor position (row y, col x) and cursor visibility must match.
      3. Modal state, focused item, catalog ordering, checkmarks, composer presence must match.
      4. Every cell (y, x) is compared character-by-character and style-by-style.
      5. Zero trimming (rstrip/strip). Trailing spaces and blank rows are strictly compared.
    """
    stage_idx = oracle_frame.stage_index
    stage_name = oracle_frame.stage_name

    # 1. Dimensions Verification
    if oracle_frame.rows != rows or target_frame.rows != rows or oracle_frame.cols != cols or target_frame.cols != cols:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="DIMENSIONS",
            expected_value=(cols, rows),
            actual_value=(target_frame.cols, target_frame.rows),
            summary=f"Screen dimension mismatch: Expected {cols}x{rows}, got Target={target_frame.cols}x{target_frame.rows}",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    if len(oracle_frame.grid) != rows or len(target_frame.grid) != rows:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="DIMENSIONS",
            summary=f"Grid row count mismatch: Oracle={len(oracle_frame.grid)} rows, Target={len(target_frame.grid)} rows",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    for r_idx in range(rows):
        if len(oracle_frame.grid[r_idx]) != cols or len(target_frame.grid[r_idx]) != cols:
            return StageDiff(
                stage_index=stage_idx,
                stage_name=stage_name,
                passed=False,
                divergence_type="ACTUAL_PRODUCT_GAP",
                divergence_category="DIMENSIONS",
                first_mismatch_cell=(r_idx, 0),
                summary=f"Grid column count mismatch at row {r_idx}: Oracle cols={len(oracle_frame.grid[r_idx])}, Target cols={len(target_frame.grid[r_idx])}",
                oracle_state=oracle_frame.to_dict(),
                target_state=target_frame.to_dict()
            )

    # 2. Cursor Position and Visibility
    if oracle_frame.cursor_position != target_frame.cursor_position:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="CURSOR_POS",
            first_mismatch_cell=target_frame.cursor_position,
            expected_value=oracle_frame.cursor_position,
            actual_value=target_frame.cursor_position,
            summary=f"Cursor position mismatch: Oracle={oracle_frame.cursor_position} (y,x) vs Target={target_frame.cursor_position} (y,x)",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    if oracle_frame.cursor_visible != target_frame.cursor_visible:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="CURSOR_VISIBILITY",
            first_mismatch_cell=target_frame.cursor_position,
            expected_value=oracle_frame.cursor_visible,
            actual_value=target_frame.cursor_visible,
            summary=f"Cursor visibility mismatch: Oracle={oracle_frame.cursor_visible} vs Target={target_frame.cursor_visible}",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    # 3. Modal & Semantic State Verification
    if oracle_frame.active_modal != target_frame.active_modal:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="MODAL_STATE",
            expected_value=oracle_frame.active_modal,
            actual_value=target_frame.active_modal,
            summary=f"Active modal mismatch: Oracle={oracle_frame.active_modal} vs Target={target_frame.active_modal}",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    if oracle_frame.focused_option_index != target_frame.focused_option_index:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="FOCUS_STATE",
            expected_value=oracle_frame.focused_option_index,
            actual_value=target_frame.focused_option_index,
            summary=f"Focused option index mismatch: Oracle={oracle_frame.focused_option_index} ({oracle_frame.focused_option}) vs Target={target_frame.focused_option_index} ({target_frame.focused_option})",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    if oracle_frame.options_catalog != target_frame.options_catalog:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="CATALOG_ORDER",
            expected_value=oracle_frame.options_catalog,
            actual_value=target_frame.options_catalog,
            summary=f"Options catalog mismatch: Oracle has {len(oracle_frame.options_catalog)} items, Target has {len(target_frame.options_catalog)} items",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    if oracle_frame.checked_options != target_frame.checked_options:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="CHECKMARK_STATE",
            expected_value=oracle_frame.checked_options,
            actual_value=target_frame.checked_options,
            summary=f"Checked options mismatch: Oracle={oracle_frame.checked_options} vs Target={target_frame.checked_options}",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    # 4. Strict Exact-Grid Character and Attribute Matrix Comparison
    first_char_mismatch = None
    first_style_mismatch = None

    for y in range(rows):
        for x in range(cols):
            o_cell = oracle_frame.grid[y][x]
            t_cell = target_frame.grid[y][x]

            # Compare Unicode character
            if o_cell.char != t_cell.char:
                if not first_char_mismatch:
                    first_char_mismatch = (y, x, o_cell, t_cell)
                    break

            # Compare full visual style attributes
            if (
                o_cell.fg != t_cell.fg or
                o_cell.bg != t_cell.bg or
                o_cell.bold != t_cell.bold or
                o_cell.italics != t_cell.italics or
                o_cell.underscore != t_cell.underscore or
                o_cell.strikethrough != t_cell.strikethrough or
                o_cell.reverse != t_cell.reverse or
                o_cell.blink != t_cell.blink
            ):
                if not first_style_mismatch:
                    first_style_mismatch = (y, x, o_cell, t_cell)
                    break
        if first_char_mismatch:
            break

    # Generate unified text diff of screen lines for forensics
    diff = list(difflib.unified_diff(
        oracle_frame.screen_lines,
        target_frame.screen_lines,
        fromfile=f"Oracle [Stage {stage_idx}: {stage_name}]",
        tofile=f"Target [Stage {stage_idx}: {stage_name}]",
        lineterm=""
    ))

    if first_char_mismatch:
        y, x, o_c, t_c = first_char_mismatch
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="GRID_CHAR",
            first_mismatch_cell=(y, x),
            expected_value=f"'{o_c.char}' (U+{ord(o_c.char):04X})",
            actual_value=f"'{t_c.char}' (U+{ord(t_c.char):04X})",
            summary=f"Grid character mismatch at row {y}, col {x}: expected '{o_c.char}' (U+{ord(o_c.char):04X}), got '{t_c.char}' (U+{ord(t_c.char):04X})",
            diff_lines=diff,
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    if first_style_mismatch:
        y, x, o_c, t_c = first_style_mismatch
        style_diffs = []
        if o_c.fg != t_c.fg: style_diffs.append(f"fg: expected '{o_c.fg}' got '{t_c.fg}'")
        if o_c.bg != t_c.bg: style_diffs.append(f"bg: expected '{o_c.bg}' got '{t_c.bg}'")
        if o_c.bold != t_c.bold: style_diffs.append(f"bold: expected {o_c.bold} got {t_c.bold}")
        if o_c.italics != t_c.italics: style_diffs.append(f"italics: expected {o_c.italics} got {t_c.italics}")
        if o_c.underscore != t_c.underscore: style_diffs.append(f"underscore: expected {o_c.underscore} got {t_c.underscore}")
        if o_c.reverse != t_c.reverse: style_diffs.append(f"reverse: expected {o_c.reverse} got {t_c.reverse}")

        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="GRID_STYLE",
            first_mismatch_cell=(y, x),
            expected_value=o_c.to_dict(),
            actual_value=t_c.to_dict(),
            summary=f"Cell style mismatch for '{o_c.char}' at row {y}, col {x}: {', '.join(style_diffs)}",
            diff_lines=diff,
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    return StageDiff(
        stage_index=stage_idx,
        stage_name=stage_name,
        passed=True,
        divergence_type="EXACT_PARITY",
        divergence_category="NONE",
        summary=f"Exact match across all {rows}x{cols} cells, styles, cursor, and semantic state.",
        diff_lines=[],
        oracle_state=oracle_frame.to_dict(),
        target_state=target_frame.to_dict()
    )
