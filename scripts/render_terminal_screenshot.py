#!/usr/bin/env python3
import os
import sys
from PIL import Image, ImageDraw, ImageFont

def create_terminal_screenshot(output_path="docs/assets/brain_tui_preview.png"):
    cols = 97
    rows = 20

    font_path = "/System/Library/Fonts/Menlo.ttc"
    if not os.path.exists(font_path):
        font_path = "/System/Library/Fonts/Monaco.ttf"

    font_size = 14
    font = ImageFont.truetype(font_path, font_size)

    bbox = font.getbbox("A")
    char_width = bbox[2] - bbox[0]
    char_height = bbox[3] - bbox[1] + 8

    padding_x = 20
    padding_y = 18
    header_height = 36

    content_width = cols * char_width
    content_height = rows * char_height

    img_width = content_width + (padding_x * 2)
    img_height = content_height + header_height + (padding_y * 2)

    bg_color = (24, 24, 37)      # Catppuccin Base #181825
    term_bg_color = (30, 30, 46) # Catppuccin Surface #1e1e2e
    header_bg = (24, 24, 37)

    img = Image.new("RGBA", (img_width, img_height), bg_color)
    draw = ImageDraw.Draw(img)

    # Window Box
    corner_radius = 10
    draw.rounded_rectangle(
        [padding_x - 4, padding_y - 4, img_width - padding_x + 4, img_height - padding_y + 4],
        radius=corner_radius,
        fill=term_bg_color,
        outline=(69, 71, 90),
        width=1
    )

    # Titlebar
    draw.rounded_rectangle(
        [padding_x - 4, padding_y - 4, img_width - padding_x + 4, padding_y + header_height],
        radius=corner_radius,
        corners=(True, True, False, False),
        fill=header_bg
    )

    # Traffic Lights
    button_y = padding_y + 11
    draw.ellipse([padding_x + 8, button_y, padding_x + 20, button_y + 12], fill=(255, 95, 86))   # Red
    draw.ellipse([padding_x + 26, button_y, padding_x + 38, button_y + 12], fill=(255, 189, 46)) # Yellow
    draw.ellipse([padding_x + 44, button_y, padding_x + 56, button_y + 12], fill=(39, 201, 63))  # Green

    # Window Title
    title_text = "brain — Terminal User Interface [v1.0.0]"
    title_bbox = font.getbbox(title_text)
    title_w = title_bbox[2] - title_bbox[0]
    title_x = (img_width - title_w) // 2
    draw.text((title_x, padding_y + 8), title_text, fill=(166, 173, 200), font=font)

    # Theme Colors
    c_border   = (116, 199, 236) # Cyan
    c_text     = (205, 214, 244) # Main text
    c_dim      = (147, 153, 178) # Muted text
    c_green    = (166, 227, 161) # Green
    c_magenta  = (245, 194, 231) # Magenta
    c_yellow   = (249, 226, 175) # Yellow
    c_blue     = (137, 180, 250) # Blue
    c_highlight= (49, 50, 68)    # Highlight bg

    # 97-character formatted terminal screen
    lines = [
        [("┌─ Brain Relational Memory Engine [v1.0.0] ────────────────────────────────────────────────────────┐", c_border, False)],
        [("│ ", c_border, False), ("Query: ", c_yellow, False), ("search \"knowledge compiler reconciliation engine\"", c_text, False), (" " * 38, c_text, False), ("│", c_border, False)],
        [("├──────────────────┬──────────────────────────────────────────────────────────────────────────────┤", c_border, False)],
        [("│ ", c_border, False), ("SESSIONS        ", c_magenta, False), (" │ ", c_border, False), ("SEARCH PROJECTIONS & HYBRID MEMORY GRAPH RESULTS", c_blue, False), (" " * 28, c_text, False), ("│", c_border, False)],
        [("├──────────────────┼──────────────────────────────────────────────────────────────────────────────┤", c_border, False)],
        [("│ ", c_border, False), ("• main-session   ", c_green, True), ("│ ", c_border, False), ("1. ", c_yellow, False), ("[0.94] Knowledge Compiler (6-Pass Reconciliation Engine)", c_green, False), (" " * 17, c_text, False), ("│", c_border, False)],
        [("│   refactor-v2    │    ", c_border, False), ("Pass 1..6: Structural DTO validation & canonicalization", c_text, False), (" " * 20, c_text, False), ("│", c_border, False)],
        [("│   debug-tui      │    ", c_border, False), ("Graph edge strength: 1.00 | Entity ID: 8f4a-9b12", c_dim, False), (" " * 29, c_text, False), ("│", c_border, False)],
        [("│                  │                                                                              │", c_border, False)],
        [("│ ", c_border, False), ("PROPERTIES      ", c_magenta, False), ("│ ", c_border, False), ("2. ", c_yellow, False), ("[0.88] SearchProjector (SQLite FTS5 + Vector BLOB Fusion)", c_blue, False), (" " * 16, c_text, False), ("│", c_border, False)],
        [("│ State: Connected │    ", c_border, False), ("Fused lexical FTS5 BM25 match with dense embedding BLOBs.", c_text, False), (" " * 19, c_text, False), ("│", c_border, False)],
        [("│ Pool: 4/4        │    ", c_border, False), ("Latency: 0.14 ms | Mode: Hybrid Rank (BM25 + Cosine)", c_dim, False), (" " * 26, c_text, False), ("│", c_border, False)],
        [("│ WAL: Enabled     │                                                                              │", c_border, False)],
        [("│ UDS: Active      │ ", c_border, False), ("3. ", c_yellow, False), ("[0.82] SqliteStorage (ACID Memory Transaction Engine)", c_text, False), (" " * 19, c_text, False), ("│", c_border, False)],
        [("│                  │    ", c_border, False), ("Durable SQLite store managing facts, assertions, & checkpoints.", c_text, False), (" " * 15, c_text, False), ("│", c_border, False)],
        [("│                  │                                                                              │", c_border, False)],
        [("├──────────────────┴──────────────────────────────────────────────────────────────────────────────┤", c_border, False)],
        [("│ ", c_border, False), ("> ", c_green, False), ("brain query \"compiler\"", c_text, False), ("█", c_magenta, False), (" " * 69, c_text, False), ("│", c_border, False)],
        [("├─────────────────────────────────────────────────────────────────────────────────────────────────┤", c_border, False)],
        [("└─ ", c_border, False), ("[Ctrl+P] Palette  ", c_yellow, False), ("│ ", c_border, False), ("[Ctrl+C] Exit  ", c_magenta, False), ("│ ", c_border, False), ("[Tab] Focus Panel  ", c_blue, False), ("│ ", c_border, False), ("Daemon: Connected (UDS 0600)", c_green, False), (" ──┘", c_border, False)],
    ]

    start_x = padding_x
    start_y = padding_y + header_height + 6

    for row_idx, line in enumerate(lines):
        curr_x = start_x
        curr_y = start_y + (row_idx * char_height)

        for segment in line:
            text, color, is_bg_highlight = segment
            seg_w = len(text) * char_width

            if is_bg_highlight:
                draw.rectangle([curr_x, curr_y - 2, curr_x + seg_w, curr_y + char_height - 2], fill=c_highlight)

            draw.text((curr_x, curr_y), text, fill=color, font=font)
            curr_x += seg_w

    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    img.save(output_path, "PNG")
    print(f"Generated screenshot at: {output_path}")

if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "docs/assets/brain_tui_preview.png"
    create_terminal_screenshot(out)
