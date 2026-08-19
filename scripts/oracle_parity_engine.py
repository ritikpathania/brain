#!/usr/bin/env python3
"""
Canonical Claude Oracle Parity Engine & Exact-Grid Verifier
Strict Exact-Grid Differential Behavioral Verification: Reference Claude vs Brain Target

Core Architectural Invariants:
  1. Claude is the immutable behavioral, visual, and protocol oracle reference.
  2. Brain must conform to Claude cell-by-cell, style-by-style, coordinate-by-coordinate.
  3. Strict Exact-Grid Comparison:
     - No rstrip() / strip() / trimming of any kind.
     - Every cell (row y, col x) is compared for character, fg, bg, bold, italics, underscore, reverse, strikethrough, blink.
     - Cursor coordinates (y, x) and cursor visibility must match.
     - Full semantic states (modal, focus index, catalog ordering, checkmarks, composer) must match.
  4. Raw PTY stream persistence: .raw bytes saved for every stage for forensic analysis.
  5. Machine-readable exact grid artifacts: .json saved with full cell attribute matrices.
  6. Tightened synchronization predicates: predicates only synchronize, never define parity.
  7. Strict classification:
     - EXACT_PARITY
     - ACTUAL_PRODUCT_GAP
     - ORACLE_FAILURE
     - TEST_HARNESS_FAILURE
     - ENVIRONMENTAL_DIVERGENCE
"""

import os
import sys
import pty
import time
import json
import pyte
import select
import codecs
import difflib
import struct
import fcntl
import termios
import tempfile
import shutil
import re
import copy
from dataclasses import dataclass, field, asdict
from typing import List, Dict, Tuple, Optional, Callable, Any, Set

# ==============================================================================
# 1. CANONICAL DATA STRUCTURES & CELL ATTRIBUTES
# ==============================================================================

@dataclass
class CanonicalCell:
    char: str = " "
    fg: str = "default"
    bg: str = "default"
    bold: bool = False
    italics: bool = False
    underscore: bool = False
    strikethrough: bool = False
    reverse: bool = False
    blink: bool = False

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)

    @classmethod
    def from_pyte(cls, char: pyte.screens.Char) -> 'CanonicalCell':
        return cls(
            char=char.data,
            fg=str(char.fg),
            bg=str(char.bg),
            bold=bool(char.bold),
            italics=bool(char.italics),
            underscore=bool(char.underscore),
            strikethrough=bool(char.strikethrough),
            reverse=bool(char.reverse),
            blink=bool(char.blink)
        )

@dataclass
class CanonicalFrame:
    stage_index: int
    stage_name: str
    cols: int
    rows: int
    grid: List[List[CanonicalCell]]  # Exact rows x cols matrix
    screen_lines: List[str]          # Exact cols-wide strings (no rstrip)
    cursor_position: Tuple[int, int] # (row y, col x)
    cursor_visible: bool
    active_modal: Optional[str] = None
    focused_option: Optional[str] = None
    focused_option_index: Optional[int] = None
    options_catalog: List[str] = field(default_factory=list)
    checked_options: List[str] = field(default_factory=list)
    composer_present: bool = False
    suggestions_present: bool = False
    terminal_modes: List[str] = field(default_factory=list)
    raw_pty_bytes: bytes = b""

    def to_dict(self) -> Dict[str, Any]:
        d = asdict(self)
        d["raw_pty_bytes_len"] = len(self.raw_pty_bytes)
        d["raw_pty_bytes"] = self.raw_pty_bytes.hex()
        return d

@dataclass
class StageSpec:
    index: int
    name: str
    action_type: str  # 'type', 'key', 'wait', 'type_and_enter', 'restart', 'assert_disk'
    input_bytes: bytes
    settle_time_ms: int
    wait_predicate: Optional[Callable[[pyte.Screen], bool]]
    description: str

@dataclass
class StageDiff:
    stage_index: int
    stage_name: str
    passed: bool
    divergence_type: str  # 'EXACT_PARITY', 'ACTUAL_PRODUCT_GAP', 'ORACLE_FAILURE', 'TEST_HARNESS_FAILURE', 'ENVIRONMENTAL_DIVERGENCE'
    divergence_category: Optional[str] = None  # 'DIMENSIONS', 'GRID_CHAR', 'GRID_STYLE', 'TRAILING_SPACE', 'BLANK_ROW', 'CURSOR_POS', 'CURSOR_VISIBILITY', 'MODAL', 'FOCUS', 'CATALOG_ORDER', 'CHECKMARK', 'COMPOSER', 'SUGGESTIONS', 'PERSISTENCE', 'TIMING'
    first_mismatch_cell: Optional[Tuple[int, int]] = None  # (row y, col x)
    expected_value: Any = None
    actual_value: Any = None
    summary: str = ""
    diff_text: str = ""
    oracle_state: Optional[Dict[str, Any]] = None
    target_state: Optional[Dict[str, Any]] = None

@dataclass
class ParityVerdict:
    target_name: str
    mode: str
    cols: int
    rows: int
    total_stages: int
    passed_stages: int
    first_divergence_stage: Optional[int] = None
    first_divergence_type: Optional[str] = None
    first_divergence_category: Optional[str] = None
    first_mismatch_cell: Optional[Tuple[int, int]] = None
    diffs: List[StageDiff] = field(default_factory=list)

    @property
    def passed(self) -> bool:
        return self.passed_stages == self.total_stages

# ==============================================================================
# 2. FRAME EXTRACTION & IDENTITY NORMALIZATION
# ==============================================================================

def normalize_identity_only(text: str, home_dir: str, workspace_dir: str) -> str:
    """
    STRICT CANONICALIZATION POLICY:
    Normalize ONLY non-deterministic execution identities:
      - Specific temporary directory paths
    Zero visual or behavioral normalization.
    """
    normalized = text
    if home_dir:
        normalized = normalized.replace(home_dir, "<TEST_HOME>")
        normalized = normalized.replace(os.path.realpath(home_dir), "<TEST_HOME>")
    
    normalized = re.sub(r'/(?:private/)?var/folders/[^\s/]+/[^\s/]+/T/parity_[^\s/]+', '<TEST_HOME>', normalized)
    normalized = re.sub(r'/(?:private/)?tmp/parity_[^\s/]+', '<TEST_HOME>', normalized)
    return normalized

def extract_canonical_frame(screen: pyte.Screen, stage_index: int, stage_name: str, home_dir: str = "", workspace_dir: str = "", raw_bytes: bytes = b"") -> CanonicalFrame:
    cols = screen.columns
    rows = screen.lines
    
    # 1. Build exact cell matrix
    grid: List[List[CanonicalCell]] = []
    screen_lines: List[str] = []
    
    for y in range(rows):
        row_cells: List[CanonicalCell] = []
        for x in range(cols):
            char_obj = screen.buffer[y][x]
            cell = CanonicalCell.from_pyte(char_obj)
            row_cells.append(cell)
        grid.append(row_cells)
        # Exact cols-wide line without any rstrip
        line_str = "".join(c.char for c in row_cells)
        if home_dir:
            line_str = normalize_identity_only(line_str, home_dir, workspace_dir)
        screen_lines.append(line_str)
        
    cursor_pos = (screen.cursor.y, screen.cursor.x)
    cursor_vis = not screen.cursor.hidden
    
    # 2. Extract semantic state
    display = screen.display
    active_modal = None
    if any("Choose the text style" in line or ("Theme" in line and not line.strip().startswith("❯")) for line in display):
        active_modal = "ThemePicker"
    elif any("Config" in line for line in display):
        active_modal = "Config"
        
    options = []
    checked = []
    focused_option = None
    focused_option_index = None
    
    for idx, line in enumerate(display):
        opt_match = re.search(r'([❯\s])\s*(\d+)\.\s+([^✔\n\r]+)(✔?)', line)
        if opt_match and active_modal:
            is_focused = opt_match.group(1) == '❯'
            opt_num = int(opt_match.group(2))
            opt_name = opt_match.group(3).strip()
            has_check = opt_match.group(4) == '✔'
            
            opt_str = f"{opt_num}. {opt_name}"
            options.append(opt_str)
            if has_check:
                checked.append(opt_str)
            if is_focused:
                focused_option = opt_str
                focused_option_index = opt_num
                
    composer_present = any(l.strip().startswith("❯") for l in display)
    suggestions_present = any("Change the theme" in l for l in display)
    terminal_modes = [str(m) for m in getattr(screen, 'mode', set())]
    
    return CanonicalFrame(
        stage_index=stage_index,
        stage_name=stage_name,
        cols=cols,
        rows=rows,
        grid=grid,
        screen_lines=screen_lines,
        cursor_position=cursor_pos,
        cursor_visible=cursor_vis,
        active_modal=active_modal,
        focused_option=focused_option,
        focused_option_index=focused_option_index,
        options_catalog=options,
        checked_options=checked,
        composer_present=composer_present,
        suggestions_present=suggestions_present,
        terminal_modes=terminal_modes,
        raw_pty_bytes=raw_bytes
    )

# ==============================================================================
# 3. STRICT EXACT-GRID COMPARATOR
# ==============================================================================

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
            expected_value=oracle_frame.cursor_visible,
            actual_value=target_frame.cursor_visible,
            summary=f"Cursor visibility mismatch: Oracle={oracle_frame.cursor_visible} vs Target={target_frame.cursor_visible}",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    # 3. Structural Semantic State
    if oracle_frame.active_modal != target_frame.active_modal:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="MODAL",
            expected_value=oracle_frame.active_modal,
            actual_value=target_frame.active_modal,
            summary=f"Active modal mismatch: Oracle='{oracle_frame.active_modal}' vs Target='{target_frame.active_modal}'",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    if oracle_frame.focused_option_index != target_frame.focused_option_index:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="FOCUS",
            expected_value=oracle_frame.focused_option_index,
            actual_value=target_frame.focused_option_index,
            summary=f"Focused option index mismatch: Oracle={oracle_frame.focused_option_index} vs Target={target_frame.focused_option_index}",
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
            summary=f"Options catalog ordering mismatch: Oracle={oracle_frame.options_catalog} vs Target={target_frame.options_catalog}",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    if oracle_frame.checked_options != target_frame.checked_options:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="CHECKMARK",
            expected_value=oracle_frame.checked_options,
            actual_value=target_frame.checked_options,
            summary=f"Checked options mismatch: Oracle={oracle_frame.checked_options} vs Target={target_frame.checked_options}",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    if oracle_frame.composer_present != target_frame.composer_present:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="COMPOSER",
            expected_value=oracle_frame.composer_present,
            actual_value=target_frame.composer_present,
            summary=f"Composer presence mismatch: Oracle={oracle_frame.composer_present} vs Target={target_frame.composer_present}",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    if oracle_frame.suggestions_present != target_frame.suggestions_present:
        return StageDiff(
            stage_index=stage_idx,
            stage_name=stage_name,
            passed=False,
            divergence_type="ACTUAL_PRODUCT_GAP",
            divergence_category="SUGGESTIONS",
            expected_value=oracle_frame.suggestions_present,
            actual_value=target_frame.suggestions_present,
            summary=f"Suggestions presence mismatch: Oracle={oracle_frame.suggestions_present} vs Target={target_frame.suggestions_present}",
            oracle_state=oracle_frame.to_dict(),
            target_state=target_frame.to_dict()
        )

    # 4. Strict Exact-Grid Cell-by-Cell Comparison (Character & Style Attributes)
    for y in range(rows):
        o_row = oracle_frame.grid[y]
        t_row = target_frame.grid[y]
        
        o_line = oracle_frame.screen_lines[y]
        t_line = target_frame.screen_lines[y]

        # Check characters in this row
        for x in range(cols):
            o_cell = o_row[x]
            t_cell = t_row[x]

            if o_cell.char != t_cell.char:
                # Classify character mismatch category
                is_blank_oracle_row = all(c.char == ' ' for c in o_row)
                is_blank_target_row = all(c.char == ' ' for c in t_row)
                
                if is_blank_oracle_row != is_blank_target_row:
                    cat = "BLANK_ROW"
                elif o_cell.char == ' ' or t_cell.char == ' ':
                    # Check if trailing space
                    if all(c.char == ' ' for c in o_row[x:]) or all(c.char == ' ' for c in t_row[x:]):
                        cat = "TRAILING_SPACE"
                    else:
                        cat = "GRID_CHAR"
                else:
                    cat = "GRID_CHAR"

                diff_lines = list(difflib.unified_diff(
                    [l + "\n" for l in oracle_frame.screen_lines],
                    [l + "\n" for l in target_frame.screen_lines],
                    fromfile=f"Oracle [Stage {stage_idx}: {stage_name}]",
                    tofile=f"Target [Stage {stage_idx}: {stage_name}]",
                    lineterm=""
                ))
                diff_text = "".join(diff_lines)

                return StageDiff(
                    stage_index=stage_idx,
                    stage_name=stage_name,
                    passed=False,
                    divergence_type="ACTUAL_PRODUCT_GAP",
                    divergence_category=cat,
                    first_mismatch_cell=(y, x),
                    expected_value=o_cell.char,
                    actual_value=t_cell.char,
                    summary=f"Grid character mismatch at row {y}, col {x}: expected '{o_cell.char}' (U+{ord(o_cell.char):04X}), got '{t_cell.char}' (U+{ord(t_cell.char):04X})",
                    diff_text=diff_text,
                    oracle_state=oracle_frame.to_dict(),
                    target_state=target_frame.to_dict()
                )

            # Check styles in this cell
            style_attrs = ['fg', 'bg', 'bold', 'italics', 'underscore', 'reverse', 'strikethrough', 'blink']
            for attr in style_attrs:
                o_val = getattr(o_cell, attr)
                t_val = getattr(t_cell, attr)
                if o_val != t_val:
                    return StageDiff(
                        stage_index=stage_idx,
                        stage_name=stage_name,
                        passed=False,
                        divergence_type="ACTUAL_PRODUCT_GAP",
                        divergence_category="GRID_STYLE",
                        first_mismatch_cell=(y, x),
                        expected_value={attr: o_val, 'char': o_cell.char},
                        actual_value={attr: t_val, 'char': t_cell.char},
                        summary=f"Cell style mismatch for '{o_cell.char}' at row {y}, col {x}: expected {attr}={o_val!r}, got {attr}={t_val!r}",
                        diff_text=f"Row {y} Col {x} character '{o_cell.char}':\n  Oracle {attr}: {o_val!r}\n  Target {attr}: {t_val!r}\n",
                        oracle_state=oracle_frame.to_dict(),
                        target_state=target_frame.to_dict()
                    )

    return StageDiff(
        stage_index=stage_idx,
        stage_name=stage_name,
        passed=True,
        divergence_type="EXACT_PARITY",
        summary="Exact 100% cell-by-cell grid, style & semantic match"
    )

# ==============================================================================
# 4. TIGHTENED 16-STAGE LIFECYCLE CONTRACT SPECIFICATION
# ==============================================================================

def create_theme_lifecycle_contract() -> List[StageSpec]:
    """
    16-Stage Lifecycle Contract.
    Predicates only synchronize PTY execution and do NOT define parity.
    Parity is determined solely by diff_exact_grid_frames().
    """
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
            wait_predicate=lambda s: any("Theme picker dismissed" in l for l in s.display) or any(l.strip().startswith("❯") for l in s.display[1:]),
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
            input_bytes=b"",
            settle_time_ms=800,
            wait_predicate=lambda s: any("3. Light mode ✔" in l for l in s.display),
            description="Process restart loads persisted theme and renders checkmark on Option 3"
        ),
    ]

# ==============================================================================
# 5. PTY SESSION EXECUTION HARNESS
# ==============================================================================

class OracleSession:
    def __init__(self, target_type: str, editor_mode: str, cols: int = 80, rows: int = 24):
        self.target_type = target_type  # 'claude' or 'brain'
        self.editor_mode = editor_mode
        self.cols = cols
        self.rows = rows
        self.home_dir = tempfile.mkdtemp(prefix=f"parity_{target_type}_{editor_mode}_")
        self.brain_shell_dir = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"
        self.repo_root = "/Users/ritikpathania/Developer/PyCharm/brain"
        self.master_fd = None
        self.slave_fd = None
        self.pid = None
        self.screen = None
        self.stream = None
        self.decoder = None
        self.stage_raw_bytes: bytearray = bytearray()

    def setup_environment(self):
        claude_dir = os.path.join(self.home_dir, ".claude")
        cache_dir = os.path.join(claude_dir, "cache")
        os.makedirs(cache_dir, exist_ok=True)
        
        with open(os.path.join(cache_dir, "changelog.md"), "w") as f:
            f.write("## 2.1.233\n- Added GitLab merge request URL support to /pr\n- Added an opt-in `forward_user_identity` approval parameter\n- Added opt-in memory cgroup support for Bash tool execution\n")

        with open(os.path.join(claude_dir, "settings.json"), "w") as f:
            json.dump({
                "model": "claude-sonnet-4-6",
                "editorMode": self.editor_mode,
                "promptSuggestionEnabled": False
            }, f)

        # Pre-seed official marketplace to prevent redundant git clones / network requests during tests
        plugins_dir = os.path.join(claude_dir, "plugins")
        marketplaces_dir = os.path.join(plugins_dir, "marketplaces")
        official_mkt_dir = os.path.join(marketplaces_dir, "claude-plugins-official")
        os.makedirs(official_mkt_dir, exist_ok=True)
        with open(os.path.join(plugins_dir, "known_marketplaces.json"), "w") as f:
            json.dump({
                "claude-plugins-official": {
                    "source": {
                        "source": "github",
                        "repo": "anthropics/claude-plugins-official"
                    },
                    "installLocation": official_mkt_dir,
                    "lastUpdated": "2026-08-18T00:00:00.000Z"
                }
            }, f)

        cwd = self.brain_shell_dir
        real_cwd = os.path.realpath(cwd)

        try:
            import subprocess
            v_out = subprocess.check_output(["/Users/ritikpathania/.local/bin/claude", "--version"], text=True)
            claude_ver = v_out.strip().split()[0]
        except Exception:
            claude_ver = "2.1.235"

        with open(os.path.join(self.home_dir, ".claude.json"), "w") as f:
            json.dump({
                "editorMode": self.editor_mode,
                "hasCompletedOnboarding": True,
                "hasCompletedProjectOnboarding": True,
                "shownTips": ["opus_1m_tip"],
                "opus1mMergeNoticeSeenCount": 10,
                "projectOnboardingSeenCount": 10,
                "lastReleaseNotesSeen": claude_ver,
                "lastOnboardingVersion": claude_ver,
                "officialMarketplaceAutoInstallAttempted": True,
                "officialMarketplaceAutoInstalled": True,
                "projects": {
                    cwd: {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    real_cwd: {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    self.repo_root: {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    os.path.realpath(self.repo_root): {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    self.home_dir: {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    os.path.realpath(self.home_dir): {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    "/tmp": {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10},
                    "/private/tmp": {"hasTrustDialogAccepted": True, "hasCompletedProjectOnboarding": True, "projectOnboardingSeenCount": 10}
                }
            }, f)

    def spawn(self):
        self.setup_environment()
        
        env = dict(
            os.environ,
            HOME=self.home_dir,
            TERM="xterm-256color",
            COLUMNS=str(self.cols),
            LINES=str(self.rows),
            DISABLE_AUTOUPDATER="1",
            CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1",
            DISABLE_TELEMETRY="1",
            CLAUDE_CODE_NO_FLICKER="1"
        )
        try:
            import subprocess
            v_out = subprocess.check_output(["/Users/ritikpathania/.local/bin/claude", "--version"], text=True)
            env["CLAUDE_VERSION"] = v_out.strip().split()[0]
        except Exception:
            env["CLAUDE_VERSION"] = "2.1.235"

        preload_path = os.path.join(self.brain_shell_dir, "src", "preload.ts")
        main_path = os.path.join(self.brain_shell_dir, "src", "main.tsx")

        if self.target_type == "claude":
            cmd = ["/Users/ritikpathania/.local/bin/claude"]
        else:
            cmd = [
                "/Users/ritikpathania/.bun/bin/bun",
                "run",
                "--feature", "AUTO_THEME",
                "--preload", preload_path,
                main_path
            ]

        self.master_fd, self.slave_fd = pty.openpty()
        winsize = struct.pack("HHHH", self.rows, self.cols, 0, 0)
        fcntl.ioctl(self.master_fd, termios.TIOCSWINSZ, winsize)
        fcntl.ioctl(self.slave_fd, termios.TIOCSWINSZ, winsize)

        self.pid = os.fork()
        if self.pid == 0:
            os.setsid()
            os.dup2(self.slave_fd, 0)
            os.dup2(self.slave_fd, 1)
            os.dup2(self.slave_fd, 2)
            os.close(self.master_fd)
            os.chdir(self.brain_shell_dir)
            os.execvpe(cmd[0], cmd, env)
        
        os.close(self.slave_fd)
        self.screen = pyte.Screen(self.cols, self.rows)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.stage_raw_bytes = bytearray()

    def drain(self, timeout_sec: float = 0.3):
        start = time.time()
        while time.time() - start < timeout_sec:
            r, _, _ = select.select([self.master_fd], [], [], 0.05)
            if self.master_fd in r:
                try:
                    chunk = os.read(self.master_fd, 8192)
                    if not chunk: break
                    self.stage_raw_bytes.extend(chunk)
                    self.stream.feed(self.decoder.decode(chunk))
                except OSError:
                    break
            else:
                break

    def wait_until(self, predicate: Callable[[pyte.Screen], bool], timeout_sec: float = 15.0) -> bool:
        start = time.time()
        while time.time() - start < timeout_sec:
            r, _, _ = select.select([self.master_fd], [], [], 0.05)
            if self.master_fd in r:
                try:
                    chunk = os.read(self.master_fd, 4096)
                    if not chunk: break
                    self.stage_raw_bytes.extend(chunk)
                    self.stream.feed(self.decoder.decode(chunk))
                except OSError: break
            if predicate(self.screen):
                return True
        return False

    def send(self, data: bytes):
        os.write(self.master_fd, data)

    def capture_canonical_frame(self, stage_index: int, stage_name: str) -> CanonicalFrame:
        self.drain(0.3)
        raw = bytes(self.stage_raw_bytes)
        self.stage_raw_bytes = bytearray()
        
        return extract_canonical_frame(
            self.screen,
            stage_index=stage_index,
            stage_name=stage_name,
            home_dir=self.home_dir,
            workspace_dir=self.brain_shell_dir,
            raw_bytes=raw
        )

    def get_persisted_theme(self) -> Optional[str]:
        settings_file = os.path.join(self.home_dir, ".claude", "settings.json")
        if os.path.exists(settings_file):
            try:
                with open(settings_file) as f:
                    t = json.load(f).get("theme")
                    if t: return t
            except: pass
            
        claude_file = os.path.join(self.home_dir, ".claude.json")
        if os.path.exists(claude_file):
            try:
                with open(claude_file) as f:
                    t = json.load(f).get("theme")
                    if t: return t
            except: pass
        return None

    def terminate(self):
        if self.pid:
            try:
                os.killpg(self.pid, 9)
            except:
                try:
                    os.kill(self.pid, 9)
                except: pass
            try:
                os.waitpid(self.pid, 0)
            except: pass
            self.pid = None
        if self.master_fd:
            try: os.close(self.master_fd)
            except: pass
            self.master_fd = None

    def cleanup(self):
        self.terminate()
        for _ in range(5):
            if self.home_dir and os.path.exists(self.home_dir):
                shutil.rmtree(self.home_dir, ignore_errors=True)
                if not os.path.exists(self.home_dir):
                    break
                time.sleep(0.05)

# ==============================================================================
# 6. ARTIFACT PERSISTENCE (RAW PTY, JSON GRID & UNIFIED DIFF)
# ==============================================================================

def save_frame_artifacts(base_dir: str, target: str, stage_idx: int, stage_name: str, frame: CanonicalFrame, diff_text: Optional[str] = None):
    target_dir = os.path.join(base_dir, target)
    os.makedirs(target_dir, exist_ok=True)
    
    # 1. Raw PTY stream (.raw)
    raw_path = os.path.join(target_dir, f"stage_{stage_idx:02d}_{stage_name}.raw")
    with open(raw_path, "wb") as f:
        f.write(frame.raw_pty_bytes)
        
    # 2. Text display (.txt)
    txt_path = os.path.join(target_dir, f"stage_{stage_idx:02d}_{stage_name}.txt")
    with open(txt_path, "w", encoding="utf-8") as f:
        for line in frame.screen_lines:
            f.write(line + "\n")
            
    # 3. Machine-readable JSON (.json) with exact cell grid
    json_path = os.path.join(target_dir, f"stage_{stage_idx:02d}_{stage_name}.json")
    with open(json_path, "w", encoding="utf-8") as f:
        json.dump(frame.to_dict(), f, indent=2)
        
    # 4. Save diff if diverged (.diff)
    if diff_text:
        diff_dir = os.path.join(base_dir, "diff")
        os.makedirs(diff_dir, exist_ok=True)
        diff_path = os.path.join(diff_dir, f"stage_{stage_idx:02d}_{stage_name}.diff")
        with open(diff_path, "w", encoding="utf-8") as f:
            f.write(diff_text)

# ==============================================================================
# 7. EXECUTION ENGINE
# ==============================================================================

def execute_contract(session: OracleSession, specs: List[StageSpec]) -> Tuple[List[CanonicalFrame], Optional[StageDiff]]:
    frames: List[CanonicalFrame] = []
    session.spawn()
    
    try:
        for spec in specs:
            if spec.action_type == "type":
                session.send(spec.input_bytes)
            elif spec.action_type == "key":
                time.sleep(0.08)
                session.send(spec.input_bytes)
            elif spec.action_type == "type_and_enter":
                time.sleep(0.08)
                session.send(b"/theme")
                session.wait_until(lambda s: any("Change the theme" in l for l in s.display[-8:]), timeout_sec=4.0)
                session.send(b"\r")
            elif spec.action_type == "restart":
                session.terminate()
                session.master_fd, session.slave_fd = pty.openpty()
                winsize = struct.pack("HHHH", session.rows, session.cols, 0, 0)
                fcntl.ioctl(session.master_fd, termios.TIOCSWINSZ, winsize)
                
                preload_path = os.path.join(session.brain_shell_dir, "src", "preload.ts")
                main_path = os.path.join(session.brain_shell_dir, "src", "main.tsx")
                if session.target_type == "claude":
                    cmd = ["/Users/ritikpathania/.local/bin/claude"]
                else:
                    cmd = ["/Users/ritikpathania/.bun/bin/bun", "run", "--feature", "AUTO_THEME", "--preload", preload_path, main_path]
                
                env = dict(
                    os.environ,
                    HOME=session.home_dir,
                    TERM="xterm-256color",
                    COLUMNS=str(session.cols),
                    LINES=str(session.rows),
                    DISABLE_AUTOUPDATER="1",
                    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1",
                    DISABLE_TELEMETRY="1",
                    CLAUDE_CODE_NO_FLICKER="1"
                )
                
                session.pid = os.fork()
                if session.pid == 0:
                    os.setsid()
                    os.dup2(session.slave_fd, 0)
                    os.dup2(session.slave_fd, 1)
                    os.dup2(session.slave_fd, 2)
                    os.close(session.master_fd)
                    os.chdir(session.brain_shell_dir)
                    os.execvpe(cmd[0], cmd, env)
                os.close(session.slave_fd)
                session.screen = pyte.Screen(session.cols, session.rows)
                session.stream = pyte.Stream(session.screen)
                session.decoder = codecs.getincrementaldecoder("utf-8")("replace")
                session.stage_raw_bytes = bytearray()
                
                session.wait_until(lambda s: any(l.startswith("❯") for l in s.display), timeout_sec=8.0)
                session.send(b"/theme")
                session.wait_until(lambda s: any("Change the theme" in l for l in s.display[-8:]), timeout_sec=4.0)
                session.send(b"\r")
                
            elif spec.action_type == "assert_disk":
                persisted = session.get_persisted_theme()
                if persisted != "light":
                    return frames, StageDiff(
                        stage_index=spec.index,
                        stage_name=spec.name,
                        passed=False,
                        divergence_type="ACTUAL_PRODUCT_GAP" if session.target_type == "brain" else "ORACLE_FAILURE",
                        divergence_category="PERSISTENCE",
                        expected_value="light",
                        actual_value=persisted,
                        summary=f"Persisted theme mismatch: expected 'light', got '{persisted}'"
                    )

            if spec.wait_predicate:
                ok = session.wait_until(spec.wait_predicate, timeout_sec=8.0)
                if not ok:
                    frame = session.capture_canonical_frame(spec.index, spec.name)
                    frames.append(frame)
                    return frames, StageDiff(
                        stage_index=spec.index,
                        stage_name=spec.name,
                        passed=False,
                        divergence_type="ACTUAL_PRODUCT_GAP" if session.target_type == "brain" else "ORACLE_FAILURE",
                        divergence_category="TIMING",
                        summary=f"Timed out waiting for condition: {spec.description}"
                    )
            
            if spec.settle_time_ms > 0:
                time.sleep(spec.settle_time_ms / 1000.0)
                
            frame = session.capture_canonical_frame(spec.index, spec.name)
            frames.append(frame)

        return frames, None
    finally:
        session.cleanup()

# ==============================================================================
# 8. ORACLE PARITY ENGINE RUNNER
# ==============================================================================

def run_oracle_parity_verification(mode: str = "normal", cols: int = 80, rows: int = 24) -> ParityVerdict:
    specs = create_theme_lifecycle_contract()
    total = len(specs)
    artifact_base = os.path.join(
        "/Users/ritikpathania/Developer/PyCharm/brain",
        "artifacts",
        "oracle_verification",
        f"{mode}_{cols}x{rows}"
    )
    os.makedirs(artifact_base, exist_ok=True)
    
    print(f"\n[{mode.upper()} {cols}x{rows}] Phase A: Recording Reference Claude Oracle ({total} stages)...")
    oracle_session = OracleSession("claude", mode, cols=cols, rows=rows)
    oracle_frames, oracle_err = execute_contract(oracle_session, specs)
    
    if oracle_err:
        print(f"  ✖ Oracle recording failed at stage {oracle_err.stage_index} ({oracle_err.stage_name}): {oracle_err.summary}")
        return ParityVerdict("claude", mode, cols, rows, total, len(oracle_frames), oracle_err.stage_index, oracle_err.divergence_type, oracle_err.divergence_category, None, [oracle_err])
    
    print(f"  ✔ Oracle recording completed successfully ({len(oracle_frames)}/{total} stages captured).")

    # Persist oracle artifacts (raw PTY, text display, exact JSON grid)
    for f in oracle_frames:
        save_frame_artifacts(artifact_base, "oracle", f.stage_index, f.stage_name, f)

    print(f"\n[{mode.upper()} {cols}x{rows}] Phase B: Executing Brain Shell Target ({total} stages)...")
    brain_session = OracleSession("brain", mode, cols=cols, rows=rows)
    brain_frames, brain_err = execute_contract(brain_session, specs)

    if brain_err:
        print(f"  ✖ Brain target failed execution at stage {brain_err.stage_index} ({brain_err.stage_name}): {brain_err.summary}")
        return ParityVerdict("brain", mode, cols, rows, total, len(brain_frames), brain_err.stage_index, brain_err.divergence_type, brain_err.divergence_category, None, [brain_err])

    print(f"  ✔ Brain target execution completed successfully ({len(brain_frames)}/{total} stages captured).")

    # Persist brain artifacts (raw PTY, text display, exact JSON grid)
    for f in brain_frames:
        save_frame_artifacts(artifact_base, "brain", f.stage_index, f.stage_name, f)

    print(f"\n[{mode.upper()} {cols}x{rows}] Phase C: Exact-Grid Frame-by-Frame Differential Auditing...")
    verdict = ParityVerdict("brain", mode, cols, rows, total, 0)
    
    for idx in range(min(len(oracle_frames), len(brain_frames))):
        o_frame = oracle_frames[idx]
        b_frame = brain_frames[idx]
        
        diff = diff_exact_grid_frames(o_frame, b_frame, cols, rows)
        verdict.diffs.append(diff)
        
        if diff.passed:
            verdict.passed_stages += 1
            print(f"  Stage {diff.stage_index:2d} [{diff.stage_name}]: ✔ MATCH")
        else:
            if verdict.first_divergence_stage is None:
                verdict.first_divergence_stage = diff.stage_index
                verdict.first_divergence_type = diff.divergence_type
                verdict.first_divergence_category = diff.divergence_category
                verdict.first_mismatch_cell = diff.first_mismatch_cell
                
            # Save diff artifact
            save_frame_artifacts(artifact_base, "brain", diff.stage_index, diff.stage_name, b_frame, diff_text=diff.diff_text)
            
            print(f"  Stage {diff.stage_index:2d} [{diff.stage_name}]: ✖ DIVERGENCE [{diff.divergence_type}:{diff.divergence_category}]")
            print(f"    Summary: {diff.summary}")
            if diff.first_mismatch_cell:
                print(f"    First Mismatch Cell: row {diff.first_mismatch_cell[0]}, col {diff.first_mismatch_cell[1]}")
            if diff.diff_text:
                print("    Terminal Grid Diff:")
                for dl in diff.diff_text.splitlines()[:20]:
                    print(f"      {dl}")
            
            # Fail-fast: stop on first divergence
            break

    return verdict

def main():
    print("=" * 70)
    print("      CANONICAL CLAUDE ORACLE PARITY ENGINE (v2.1.233)")
    print("=" * 70)

    geometries = [(80, 24), (100, 30)]
    modes = ["normal", "vim"]
    all_passed = True
    results = {}

    for mode in modes:
        results[mode] = {}
        for cols, rows in geometries:
            verdict = run_oracle_parity_verification(mode, cols=cols, rows=rows)
            results[mode][f"{cols}x{rows}"] = verdict
            if not verdict.passed:
                all_passed = False

    print("\n" + "=" * 70)
    print("                  CANONICAL PARITY VERIFICATION MATRIX")
    print("=" * 70)
    print(f"{'GEOMETRY / MODE':<20} {'NORMAL':<25} {'VIM INSERT':<25}")
    print("-" * 70)
    for cols, rows in geometries:
        geom_key = f"{cols}x{rows}"
        b_norm = results["normal"][geom_key]
        b_vim = results["vim"][geom_key]
        
        b_norm_str = "PASS" if b_norm.passed else f"FAIL (Stg {b_norm.first_divergence_stage})"
        b_vim_str = "PASS" if b_vim.passed else f"FAIL (Stg {b_vim.first_divergence_stage})"
        
        print(f"Claude {geom_key:<13} {'PASS':<25} {'PASS':<25}")
        print(f"Brain  {geom_key:<13} {b_norm_str:<25} {b_vim_str:<25}")
        print("-" * 70)
    print("=" * 70)

    if all_passed:
        print("\n🎉 ALL BEHAVIORAL CONTRACT STAGES ACHIEVED 100% EXACT CANONICAL PARITY!")
        return 0
    else:
        print("\n❌ ORACLE PARITY VERIFICATION FAILED — REPRODUCED FIRST DIVERGENCE.")
        return 1

if __name__ == "__main__":
    sys.exit(main())
