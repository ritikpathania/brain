---
status: active
owner: tui
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Terminal User Interface (TUI) Design System

This document specifies the layout container rules, widget hierarchy, responsive breakpoints, and rendering invariants for the Brain TUI.

---

## 1. System Architecture & Rendering Loop
The interface is built using `Ratatui` and `Crossterm` in Rust. It utilizes an alternate screen buffer (alt-screen) and immediate-mode differential rendering to ensure zero visual tearing.

---

## 2. Layout Hierarchy & Responsive Breakpoints

```text
┌─────────────────────────────────────────────────────────────────────────┐
│ Status Bar / Header (Connected | Active Session | Memory Load)           │
├────────────────────────────────┬────────────────────────────────────────┤
│ Sidebar Panel (Compact <80col) │ Chat Viewport / Timeline               │
│ - Sessions List                │ - Markdown Render Stream               │
│ - Tool Status Cards            │ - Code Block Highlights                │
│                                │ - Search Result Cards                  │
├────────────────────────────────┴────────────────────────────────────────┤
│ Prompt Editor / Input Area (Multiline | History Nav | Status Hints)     │
└─────────────────────────────────────────────────────────────────────────┘
```

### Width Breakpoints:
- **Wide Mode (>= 80 columns)**: Displays sidebar, main timeline, and full header telemetry.
- **Compact Mode (< 80 columns)**: Automatically collapses sidebar and expands chat viewport to fill terminal width.

---

## 3. Theme & Color Resolution
Colors are resolved at render time via `Theme` maps defined in `crates/brain-tui/src/ui/theme/mod.rs`. Components must access semantic color tokens rather than hardcoding ANSI escape sequences.
