# Terminal Capabilities & Fallbacks

> **AUTHORITY NOTICE**: This document is a **supporting engineering specification** for `crates/brain-tui`, strictly subordinate to and governed by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).


This document outlines how the Brain TUI client detects, degrades, and handles varying terminal features, from high-end modern emulators to basic legacy remote SSH sessions.

---

## 1. Feature Support Matrix

| Feature | Best Experience | Fallback Level 1 | Fallback Level 2 (Minimum) |
| :--- | :--- | :--- | :--- |
| **Color Depth** | 24-bit Truecolor (RGB) | xterm-256 Color Map | standard ANSI-16 colors |
| **Borders & Boxes**| Unicode Box Drawings (`╭`, `─`) | ASCII lines (`+`, `-`, `|`) | No borders (spaces/indentation) |
| **Spinners / Glyphs**| Braille dots (`⠋`), Nerd Fonts | Rotating bar (`\|`, `/`, `-`) | Plain static text (`[Thinking]`) |
| **Progress Bars** | Solid block fills (`█`, `▌`) | Equal signs (`[====>    ]`) | Percentage labels (`50%`) |
| **File Links** | OSC-8 Interactive Hyperlinks | Raw paths (`file:///path`) | File name string only |
| **Mouse Interaction**| Clickable buttons & scroll | Keyboard scrolling only | Keyboard navigation only |

---

## 2. Fallback Rules Specifications

### 2.1. Color Depth Degradation
1. **Truecolor**: Rendered directly using `ratatui::style::Color::Rgb(r, g, b)`.
2. **256-Color**: Maps token RGB values to the nearest matching index in the 256-color map.
3. **ANSI-16**: Decays colors to basic terminal keywords. Redundant warnings and icons must be printed alongside text since colors may blend on certain dark/light terminal profiles:
   * `Success` -> `Color::Green`
   * `Danger` -> `Color::Red`
   * `Warning` -> `Color::Yellow`
   * `Primary` -> `Color::Red` (for orange replacement)
   * `Secondary` -> `Color::Magenta`

### 2.2. Line Drawing & Border Fallbacks
If the environment variables do not support UTF-8 (e.g., `LANG=C` or `LC_ALL=C`), the TUI must automatically replace Unicode box-drawing characters:
* `┌`, `┐`, `└`, `┘` -> `+`
* `├`, `┤` -> `+`
* `─` -> `-`
* `│` -> `|`

### 2.3. OSC-8 Hyperlinks
* **Supported**: File paths rendered inside the Chat Pane (such as modified file names in planning steps) are compiled as clickable hyperlinks using OSC-8 escape sequences: `\x1b]8;;file://[path]\x1b\[text]\x1b]8;;\x1b\`.
* **Unsupported**: Rendered as a plain, underline-styled file path string.

### 2.4. Nerd Fonts & Icons
* **Supported**: File system navigations render file type icons (e.g., rust logo for `.rs`, python logo for `.py`).
* **Unsupported**: File icons are skipped; file names are rendered as text strings.
