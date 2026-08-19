# Claude Code UX Reference & Principles

This document records the design principles, interaction patterns, layout rules, and visual hierarchy extracted from deep inspection of the local **Claude Code** TUI design system (`/Users/ritikpathania/Developer/src`).

---

## 1. Core UX Principles Extracted from Claude Code

### Principle 1: Prompt as Primary Focal Point
- **Observed Behavior**: The input prompt box is the central interaction anchor of the entire TUI. The focused prompt border uses the signature brand orange (`claude`: `rgb(215,119,87)`), drawing immediate optical focus.
- **Whitespace & Rhythm**: The screen layout uses generous vertical margins (`spacing.normal` / `relaxed`) around the prompt, making the interface feel calm rather than crowded.
- **Reference**: `components/PromptInput/PromptInput.tsx`, `components/design-system/ThemedBox.tsx`.

### Principle 2: Whitespace Before Borders
- **Observed Behavior**: Borders are reserved strictly for discrete interactive elements (Input Box, Modal Dialogs, Pickers, Diff containers). Section separation relies on whitespace and single-cell box-drawing dividers (`─` in `subtle` color `rgb(80,80,80)`).
- **Avoidance of Chrome Overhead**: The layout avoids nesting boxes inside boxes. Flat transparent backgrounds are used for the main viewport floor, reserving surface colors (`userMessageBackground`, `bashMessageBackgroundColor`) for message cards.
- **Reference**: `DESIGN.md` (lines 599–608), `components/design-system/Divider.tsx`.

### Principle 3: Single-Key & Keyboard-First Discovery
- **Observed Behavior**: Pressing `/` or `Ctrl+K` instantly opens the slash command picker overlay. Keyboard hints (`↑/↓ Navigate`, `Enter Select`, `Tab Complete`, `Esc Cancel`) are rendered right below the picker.
- **Overflow & Clipping Protection**: The picker dynamically calculates maximum visible items (`visibleCount = Math.max(MIN_VISIBLE, Math.min(requestedVisible, rows - CHROME_ROWS))`), guaranteeing that picker overlay opening never clips the terminal or triggers screen jumping on small viewports.
- **Compact Adaptation**: On terminals `< 120` columns, keyboard hints collapse to compact forms to prevent line wrapping.
- **Reference**: `components/design-system/FuzzyPicker.tsx` (lines 97–104).

### Principle 4: Subtle Shimmer & Breathing Progress
- **Observed Behavior**: Long-running operations use a Braille-dot spinner (`⠋⠙⠹...`) alternating between base accent colors (e.g. `claude`) and lighter shimmer companions (e.g. `claudeShimmer`) on alternating ~80ms frames.
- **Transient Lifecycle**: Verb labels ("Thinking…", "Reading…") rotate continuously. Once response tokens begin streaming, thinking progress collapses cleanly to prevent visual clutter.
- **Reference**: `DESIGN.md` (lines 481–512), `components/design-system/LoadingState.tsx`.

### Principle 5: Restrained Empty States
- **Observed Behavior**: Empty states use muted text (`inactive`), subtle iconography, and clean whitespace rather than heavy double-bordered cards or diagnostic boxes. Action hints provide explicit next steps.
- **Reference**: `components/design-system/FuzzyPicker.tsx` (line 85), `DESIGN.md` (lines 648–650).

---

## 2. Structural Comparison & Classification

| UX Feature / Element | Claude Code Behavior | Brain Current Behavior | Classification |
| :--- | :--- | :--- | :--- |
| **Landing Focal Point** | Logo + Tagline + Prompt invitation | Logo + Tagline + Try commands + Prompt | **Implemented in Brain** (`v1.0 Frozen`) |
| **Theme Color System** | TrueColor RGB + Shimmer + Semantic tokens | `ThemeToken` + precomputed palette | **Implemented in Brain** (`ThemeToken`) |
| **Command Palette Overlay** | Floating box, `/` / `Ctrl+K` shortcut, overflow protection | Bordered overlay, `/` / `Ctrl+K` pipeline, score ranking | **Partially Implemented** (Missing viewport overflow protection calculation) |
| **Keyboard Navigation Hints** | Contextual footer line (`Tab`, `Esc`, `Enter`) | Command hint footer line | **Implemented in Brain** |
| **Responsive Breakpoints** | `<80` (Compact), `80-120` (Standard), `>120` (Wide) | `<70` (Compact), `70-120` (Standard), `>=120` (Wide) | **Partially Implemented** (Adjust compact threshold to 80 cols) |
| **Reasoning Progress** | Braille spinner + rotating verb $\rightarrow$ collapse on token | Stage steps (`○`/`●`/`✓`) $\rightarrow$ collapse on token | **Implemented in Brain** (`Phase C`) |
| **Evidence Cards & Provenance** | Read-only cards, matched terms, score breakdown | Read-only `EvidenceCard` & `ConfidenceBadge` | **Implemented in Brain** (`Phase D`) |
| **Text Truncation & Middle Ellipsis** | `truncate-middle` for file paths, `wrap` for text | `truncate_middle` helper & cell slicing | **Implemented in Brain** |

---

## 3. Recommended Brain UX Improvements (Preserving Frozen Contracts)

1. **Palette Viewport Overflow Protection (P0)**:
   - Calculate maximum visible palette items dynamically based on available terminal height (`area.height - CHROME_ROWS`), preventing command palette overlay clipping on small terminals (e.g. 80x24).

2. **Responsive Terminal Breakpoint Synchronization (P1)**:
   - Align compact sidebar collapse threshold to `< 80` columns (standard TUI threshold) matching `COMPONENT_LIBRARY.md` and Claude Code responsive standards.

3. **Palette Keyboard Hint Formatting (P1)**:
   - Refine bottom hint line formatting in `PaletteWidget` (`↑↓ navigate  tab complete  enter select  esc close`) to be lightweight, matching `FuzzyPicker` UX.

4. **Empty State & Search Viewport Refinements (P2)**:
   - Refine search list empty states to use muted subtle typography and clean single-line hints without heavy card wrapping.
