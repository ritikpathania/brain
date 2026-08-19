#!/usr/bin/env python3
"""
Lightweight VT100 / XTerm / TrueColor ANSI Terminal Grid Emulator
Maintains a 2D screen buffer of dimensions (cols x rows),
tracking character glyphs, foreground RGB, background RGB, and styling attributes.
"""

import re
from dataclasses import dataclass
from typing import Optional, Tuple, List, Dict, Any

@dataclass
class Cell:
    char: str = " "
    fg_rgb: Optional[Tuple[int, int, int]] = None
    bg_rgb: Optional[Tuple[int, int, int]] = None
    bold: bool = False
    dim: bool = False
    italic: bool = False
    underline: bool = False
    inverse: bool = False

    def to_dict(self) -> Dict[str, Any]:
        return {
            "char": self.char,
            "fg": self.fg_rgb,
            "bg": self.bg_rgb,
            "bold": self.bold,
            "dim": self.dim,
            "italic": self.italic,
            "inverse": self.inverse,
        }

    def matches(self, other: 'Cell') -> bool:
        if self.char != other.char:
            return False
        if self.fg_rgb != other.fg_rgb:
            return False
        if self.bg_rgb != other.bg_rgb:
            return False
        if self.bold != other.bold or self.dim != other.dim:
            return False
        if self.italic != other.italic or self.inverse != other.inverse:
            return False
        return True


class VirtualTerminal:
    def __init__(self, cols: int = 80, rows: int = 24):
        self.cols = cols
        self.rows = rows
        self.cursor_r = 0
        self.cursor_c = 0
        self.saved_cursor = (0, 0)
        self.alt_screen_active = False
        self.cursor_visible = True
        
        # Current active style
        self.cur_fg: Optional[Tuple[int, int, int]] = None
        self.cur_bg: Optional[Tuple[int, int, int]] = None
        self.cur_bold: bool = False
        self.cur_dim: bool = False
        self.cur_italic: bool = False
        self.cur_underline: bool = False
        self.cur_inverse: bool = False

        self.grid: List[List[Cell]] = [[Cell() for _ in range(cols)] for _ in range(rows)]

    def clear_screen(self):
        self.grid = [[Cell() for _ in range(self.cols)] for _ in range(self.rows)]
        self.cursor_r = 0
        self.cursor_c = 0

    def clear_line(self, mode: int = 0):
        # 0: cursor to end, 1: start to cursor, 2: entire line
        r = self.cursor_r
        if 0 <= r < self.rows:
            if mode == 0:
                for c in range(self.cursor_c, self.cols):
                    self.grid[r][c] = Cell()
            elif mode == 1:
                for c in range(0, min(self.cursor_c + 1, self.cols)):
                    self.grid[r][c] = Cell()
            elif mode == 2:
                for c in range(self.cols):
                    self.grid[r][c] = Cell()

    def set_cell(self, r: int, c: int, char: str):
        if 0 <= r < self.rows and 0 <= c < self.cols:
            self.grid[r][c] = Cell(
                char=char,
                fg_rgb=self.cur_fg,
                bg_rgb=self.cur_bg,
                bold=self.cur_bold,
                dim=self.cur_dim,
                italic=self.cur_italic,
                underline=self.cur_underline,
                inverse=self.cur_inverse,
            )

    def handle_sgr(self, params: List[int]):
        if not params or params == [0]:
            self.cur_fg = None
            self.cur_bg = None
            self.cur_bold = False
            self.cur_dim = False
            self.cur_italic = False
            self.cur_underline = False
            self.cur_inverse = False
            return

        i = 0
        while i < len(params):
            code = params[i]
            if code == 0:
                self.cur_fg = None
                self.cur_bg = None
                self.cur_bold = False
                self.cur_dim = False
                self.cur_italic = False
                self.cur_underline = False
                self.cur_inverse = False
            elif code == 1:
                self.cur_bold = True
            elif code == 2:
                self.cur_dim = True
            elif code == 3:
                self.cur_italic = True
            elif code == 4:
                self.cur_underline = True
            elif code == 7:
                self.cur_inverse = True
            elif code == 22:
                self.cur_bold = False
                self.cur_dim = False
            elif code == 23:
                self.cur_italic = False
            elif code == 24:
                self.cur_underline = False
            elif code == 27:
                self.cur_inverse = False
            elif code == 39:
                self.cur_fg = None
            elif code == 49:
                self.cur_bg = None
            elif code == 38: # Foreground extended color
                if i + 1 < len(params):
                    if params[i+1] == 2 and i + 4 < len(params): # 24-bit RGB
                        self.cur_fg = (params[i+2], params[i+3], params[i+4])
                        i += 4
                    elif params[i+1] == 5 and i + 2 < len(params): # 256 color
                        self.cur_fg = (params[i+2], params[i+2], params[i+2])
                        i += 2
            elif code == 48: # Background extended color
                if i + 1 < len(params):
                    if params[i+1] == 2 and i + 4 < len(params): # 24-bit RGB
                        self.cur_bg = (params[i+2], params[i+3], params[i+4])
                        i += 4
                    elif params[i+1] == 5 and i + 2 < len(params):
                        self.cur_bg = (params[i+2], params[i+2], params[i+2])
                        i += 2
            i += 1

    def feed(self, text: str):
        i = 0
        n = len(text)
        while i < n:
            char = text[i]

            if char == '\r':
                self.cursor_c = 0
                i += 1
                continue
            elif char == '\n':
                self.cursor_r = min(self.rows - 1, self.cursor_r + 1)
                i += 1
                continue
            elif char == '\x08': # Backspace
                self.cursor_c = max(0, self.cursor_c - 1)
                i += 1
                continue
            elif char == '\t':
                self.cursor_c = min(self.cols - 1, (self.cursor_c // 8 + 1) * 8)
                i += 1
                continue
            elif char == '\x1b': # Escape sequence
                if i + 1 < n and text[i+1] == '[':
                    # CSI sequence
                    m = re.match(r'^\x1b\[([\d;?]*)(\??)([a-zA-Z])', text[i:])
                    if m:
                        raw_seq = m.group(0)
                        params_str, qmark, cmd = m.group(1), m.group(2), m.group(3)
                        i += len(raw_seq)

                        # Parse numeric arguments
                        params = []
                        if params_str and not params_str.startswith('?'):
                            for p in params_str.split(';'):
                                if p.isdigit():
                                    params.append(int(p))

                        if cmd == 'm': # SGR Color / Style
                            self.handle_sgr(params if params else [0])
                        elif cmd == 'H' or cmd == 'f': # Cursor Position
                            r = (params[0] - 1) if len(params) > 0 and params[0] > 0 else 0
                            c = (params[1] - 1) if len(params) > 1 and params[1] > 0 else 0
                            self.cursor_r = max(0, min(self.rows - 1, r))
                            self.cursor_c = max(0, min(self.cols - 1, c))
                        elif cmd == 'A': # Cursor Up
                            dist = params[0] if params else 1
                            self.cursor_r = max(0, self.cursor_r - dist)
                        elif cmd == 'B': # Cursor Down
                            dist = params[0] if params else 1
                            self.cursor_r = min(self.rows - 1, self.cursor_r + dist)
                        elif cmd == 'C': # Cursor Forward
                            dist = params[0] if params else 1
                            self.cursor_c = min(self.cols - 1, self.cursor_c + dist)
                        elif cmd == 'D': # Cursor Back
                            dist = params[0] if params else 1
                            self.cursor_c = max(0, self.cursor_c - dist)
                        elif cmd == 'J': # Erase in Display
                            mode = params[0] if params else 0
                            if mode == 2:
                                self.clear_screen()
                        elif cmd == 'K': # Erase in Line
                            mode = params[0] if params else 0
                            self.clear_line(mode)
                        elif cmd == 'h': # Set Mode
                            if '1049' in params_str or '?1049' in params_str:
                                self.alt_screen_active = True
                            if '25' in params_str:
                                self.cursor_visible = True
                        elif cmd == 'l': # Reset Mode
                            if '1049' in params_str or '?1049' in params_str:
                                self.alt_screen_active = False
                            if '25' in params_str:
                                self.cursor_visible = False
                        continue
                    else:
                        i += 2
                        continue
                elif i + 1 < n and text[i+1] == ']': # OSC sequence
                    # Find BEL (\x07) or ST (\x1b\\)
                    end_idx = text.find('\x07', i)
                    if end_idx != -1:
                        i = end_idx + 1
                    else:
                        i += 2
                    continue
                else:
                    i += 1
                    continue
            else:
                # Printable character
                if ord(char) >= 32:
                    self.set_cell(self.cursor_r, self.cursor_c, char)
                    self.cursor_c += 1
                    if self.cursor_c >= self.cols:
                        self.cursor_c = 0
                        self.cursor_r = min(self.rows - 1, self.cursor_r + 1)
                i += 1

    def render_plain_text(self) -> str:
        lines = []
        for r in range(self.rows):
            line = "".join(self.grid[r][c].char for c in range(self.cols))
            lines.append(line.rstrip())
        return "\n".join(lines)
