# Accessibility

This document defines the accessibility specifications for the Brain TUI client, establishing rules for screen-reader compatibility, high-contrast displays, and reduced-motion environments.

---

## 1. Accessible Mode Specification

Accessible Mode is toggled via the CLI command flag `--ax-screen-reader` or a configuration parameter. When active, the client must apply these 8 structural changes:

```
┌──────────────────────────────────────┐
│          Standard Ratatui TUI        │
│  - Alternate Buffer Layout           │
│  - Rounded Box borders               │
│  - Braille spinners & progress bars   │
│  - Rich ANSI/RGB theme colors        │
└──────────────────┬───────────────────┘
                   │
                   │ Enable `--ax-screen-reader`
                   ▼
┌──────────────────────────────────────┐
│           Accessible Mode            │
│  - Linear Standard Output (Scroll)   │
│  - ASCII Borders (+, -, |)           │
│  - Static Progress Text ([Thinking]) │
│  - High Contrast ANSI-16 Palette     │
└──────────────────────────────────────┘
```

---

## 2. Invariants & Rules

### 2.1. Linear Reading Order (Screen Reader Compatibility)
* **Standard TUI**: Uses alternate screen buffers, splitting the viewport into sidebar list, chat pane, and footer panels. This is hard for screen readers to navigate linearly.
* **Accessible TUI**: Renders content as a standard terminal stream (similar to standard input/output). Newly loaded session history, messages, and command outputs are printed sequentially. The input cursor sits at the bottom prompt of the standard stream.

### 2.2. Borders & Box Drawing
* **Standard TUI**: Renders panels using Unicode line-drawing glyphs (`╭`, `╮`, `─`, `│`, etc.).
* **Accessible TUI**: Replaces all line-drawing characters with standard ASCII alternatives (`+`, `-`, `|`) or completely disables borders, utilizing simple blank space indentation to separate panes.

### 2.3. Progress & Spinners
* **Standard TUI**: Displays animated Braille-dot spinners (`⠋`, `⠙`, `⠹`) during generation and block-fill progress bars (`████░░░░`).
* **Accessible TUI**: Replaces all animations and progress symbols with static text updates (e.g. `[Thinking...]`, `[Executing: 50% completed]`). This prevents screen readers from repeatedly announcing rapidly changing unicode characters.

### 2.4. Reduced Motion
* **Standard TUI**: Simulates smooth typing flows via a typewriter rendering queue (progressive token output).
* **Accessible TUI**: Disables the typewriter delay. Incoming stream chunks are immediately flushed and printed to the terminal without interpolation.

### 2.5. Color Contrast & Theme Fallbacks
* **Standard TUI**: Renders rich 24-bit RGB palettes.
* **Accessible TUI**: Automatically overrides custom theme configurations, falling back to a strictly high-contrast black-and-white or standard ANSI-16 palette.

### 2.6. Keyboard & Status Guidance
* In menus and dialog overlays, the TUI must print visible action hints.
* *Example*: Instead of rendering only a highlighted bar over a session index, the TUI must render explicit indices and text guides:
  ```
  Select session (Type index number or press Up/Down arrows):
  [1] Refactor main loop (Active)
  [2] Fix memory cache leak
  ```
