# Ratatui TUI Design System Specification

**Version**: 2.0 (Native Rust / Ratatui Edition)  
**Status**: Active  

This document details the layout, styling, and design token rules for the native Rust **Ratatui TUI Client** (`crates/brain-tui`). 

---

## 🎨 1. Theme and Color Tokens

All layout components must draw style rules from semantic theme fields defined in `theme.rs` rather than hardcoding colors.

### Core Color Palette

| Token | RGB / Color Value | Semantic Usage |
| :--- | :--- | :--- |
| `primary` | `Color::Rgb(240, 100, 45)` | Brand orange (Claude accent). Main headers, focused borders. |
| `accent` | `Color::Rgb(128, 90, 213)` | Purple. Special activity states or highlights. |
| `success` | `Color::Green` | Completed tasks, successful indexing. |
| `warning` | `Color::Yellow` | Cautionary warnings or rate-limiting events. |
| `error` | `Color::Red` | Crashes, parsing errors, cancelled streams. |
| `border` | `Color::DarkGray` | Default pane separation line style. |
| `border_active`| `Color::Rgb(240, 100, 45)` | Highlights the focused pane/input. |
| `inactive` | `Color::Gray` | Muted/dimmed secondary content. |
| `text` | `Color::White` | Primary conversation body text. |
| `cursor` | `White` bg, `Black` fg | User input pointer location. |

---

## 📏 2. Layout Grid and Partitioning

Instead of React/Ink flexbox models, the interface is rendered in immediate-mode chunks using Ratatui `Layout` splits:

1. **Vertical Main Split**:
   - Split 1: Title bar/Header (`Constraint::Length(1)`).
   - Split 2: Center Workspace panel (`Constraint::Min(0)`).
   - Split 3: Status/Help footer (`Constraint::Length(1)`).

2. **Horizontal Workspace Split**:
   - Split 1: Thread/Session Sidebar panel (`Constraint::Percentage(25)`).
   - Split 2: Chat Viewport panel (`Constraint::Percentage(75)`).

### 📱 Responsive Breakpoint
If terminal width drops **below 80 columns**:
- The TUI automatically switches to **Compact Mode**.
- The Thread/Session Sidebar is hidden, allocation shifts to full-width (`100%`) for the Chat Viewport.

---

## ⌨️ 3. Navigation and Focus

Visual cues must guide the user's active keyboard context:
* **Focus Switching**: Pressing `Tab` cycles focus between the **Sidebar panel** (left) and the **Prompt Input editor** (right).
* **Focused Border Highlight**: The active panel's borders are drawn using `border_active` style (brand orange); inactive borders use `border` style.
* **Arrow Keys**: Move row selection inside the active list pane.
* **Vim Bindings**: Support `j`/`k` list movements and scrolling offsets.

---

## 🏗️ 4. Immediate-Mode Widgets

* **Header**: Shows versioning details and host socket connectivity status.
* **Chat Area**: Handles paragraph wrapping and markdown highlighting. Follows viewport scroll locking (locks to bottom during streaming, unlocks on scroll-up).
* **Prompt Input**: Multiline text editor capturing keystrokes and handling cursors.
* **Status Line**: Footer row showcasing shortcut key help legends.
