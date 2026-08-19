"""
Canonical Terminal Data Structures & Exact-Grid Cell Extraction
"""

import pyte
import re
from dataclasses import dataclass, field, asdict
from typing import List, Dict, Tuple, Optional, Any, Set


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
    grid: List[List[CanonicalCell]]  # Exact [rows][cols] matrix of cell attributes
    screen_lines: List[str]          # Exact text lines without rstrip
    cursor_position: Tuple[int, int] # (row y, col x)
    cursor_visible: bool
    active_modal: Optional[str]
    focused_option: Optional[str]
    focused_option_index: Optional[int]
    options_catalog: List[str]
    checked_options: List[str]
    composer_present: bool
    suggestions_present: bool
    terminal_modes: List[str]
    raw_pty_bytes: bytes = field(repr=False, default=b"")

    def to_dict(self) -> Dict[str, Any]:
        return {
            "stage_index": self.stage_index,
            "stage_name": self.stage_name,
            "cols": self.cols,
            "rows": self.rows,
            "grid": [[c.to_dict() for c in row] for row in self.grid],
            "screen_lines": self.screen_lines,
            "cursor_position": self.cursor_position,
            "cursor_visible": self.cursor_visible,
            "active_modal": self.active_modal,
            "focused_option": self.focused_option,
            "focused_option_index": self.focused_option_index,
            "options_catalog": self.options_catalog,
            "checked_options": self.checked_options,
            "composer_present": self.composer_present,
            "suggestions_present": self.suggestions_present,
            "terminal_modes": self.terminal_modes,
            "raw_pty_bytes_len": len(self.raw_pty_bytes)
        }


def normalize_identity_only(line: str, home_dir: str, workspace_dir: str) -> str:
    """Normalize purely environmental runtime paths so comparison is hermetic."""
    if home_dir and home_dir in line:
        line = line.replace(home_dir, "/tmp/home_dir")
    return line


def extract_canonical_frame(
    screen: pyte.Screen,
    stage_index: int,
    stage_name: str,
    home_dir: str = "",
    workspace_dir: str = "",
    raw_bytes: bytes = b""
) -> CanonicalFrame:
    cols = screen.columns
    rows = screen.lines
    
    grid: List[List[CanonicalCell]] = []
    screen_lines: List[str] = []
    
    for y in range(rows):
        row_cells: List[CanonicalCell] = []
        for x in range(cols):
            pyte_char = screen.buffer[y][x]
            cell = CanonicalCell.from_pyte(pyte_char)
            row_cells.append(cell)
        grid.append(row_cells)
        line_str = "".join(c.char for c in row_cells)
        if home_dir:
            line_str = normalize_identity_only(line_str, home_dir, workspace_dir)
        screen_lines.append(line_str)
        
    cursor_pos = (screen.cursor.y, screen.cursor.x)
    cursor_vis = not screen.cursor.hidden
    
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
