# Claude Code vs. Brain TUI — Visual & Structural Differential Analysis
> Research pass · Brain TUI Audit · 2026-08-10
> Detailed layout breakdown, color token mappings, and visual comparisons

---

## 1. Macro Layout & Geometry Comparison

```
CLAUDE CODE CLI (Full Screen Split)           BRAIN TUI (Proportional Anchor Layout)
┌────────────────────────────────────────┐   ┌────────────────────────────────────────┐
│ [Scrollable Transcript Area]           │   │ [Top Padding / Header]                 │
│                                        │   │ ┌────────────────────────────────────┐ │
│  Clawd ASCII Art + Feed                │   │ │ Mascot Art + System Status Widget  │ │
│  User & Assistant Message History      │   │ └────────────────────────────────────┘ │
│                                        │   │                                        │
│                                        │   │ ┌────────────────────────────────────┐ │
│                                        │   │ │ Prompt Input Box                   │ │
│                                        │   │ └────────────────────────────────────┘ │
│                                        │   │ [Dynamic Spacer - Clamped 67%]        │
│ ┌────────────────────────────────────┐ │   │                                        │
│ │ Prompt Input Field                 │ │   │                                        │
│ └────────────────────────────────────┘ │   │                                        │
│ [ Status Line Footer                 ] │   │ [ Status Bar Footer                  ] │
└────────────────────────────────────────┘   └────────────────────────────────────────┘
```

### Key Geometry Distinctions

1. **Home Screen Prompt Placement**:
   - **Claude Code**: The prompt is strictly bottom-anchored. Top transcript space fills dynamically as content is added. Breathing room exists above the prompt.
   - **Brain TUI**: Bounded proportional layout. On tall terminals (>24 rows), the prompt container clamps around ~67% height using a `Constraint::Min(0)` spacer beneath it, keeping the initial focus near the visual center-lower third.

2. **Responsive Column Adaptation**:
   - **Claude Code**: 
     - `< 70 cols`: Switches to vertical single-column condensed mode.
     - `≥ 70 cols`: Splits into horizontal 2-panel (Left Clawd mascot ≤50 cols, Right feed ≥30 cols).
     - `≥ 140 cols`: Global search previews move from bottom to right panel.
   - **Brain TUI**:
     - `< 70 cols`: Hides sidebar entirely.
     - `70-120 cols`: Sidebar fixed at 22 cols.
     - `> 120 cols`: Sidebar scales fluidly as 20% of width (clamped 22–28 cols).

---

## 2. Color System & Aesthetic Palette Mapping

Both products use warm, humanist color choices to contrast against harsh white-on-black terminal defaults.

| Semantic Role | Claude Code Token | Claude RGB Value | Brain TUI Token | Brain RGB / Style |
|---|---|---|---|---|
| Primary Accent / Brand | `claude` | `rgb(215,119,87)` | `ThemeToken::Accent` | Warm Orange / Coral |
| Brand Shimmer | `claudeShimmer` | `rgb(235,159,127)` | N/A | Static accent |
| Security / Permission | `permission` | `rgb(177,185,249)` | `ThemeToken::Warning` | Amber / Soft Blue |
| User Message Card | `userMessageBackground` | `rgb(55,55,55)` | `ThemeToken::Surface` | Dark Slate Box Fill |
| Input Border (Default)| `promptBorder` | `rgb(136,136,136)` | `ThemeToken::BorderSubtle` | Muted Gray |
| Input Border (Focus)  | `claude` | `rgb(215,119,87)` | `ThemeToken::BorderFocused` | Accent Highlight |
| Success State | `success` | `rgb(78,186,101)` | `ThemeToken::Success` | Green |
| Error State | `error` | `rgb(255,107,128)` | `ThemeToken::Error` | Bright Red |
| Selection Highlight | `selectionBg` | `rgb(38,79,120)` | `ThemeToken::Selection` | Muted Blue Background |

---

## 3. Component Visual Comparisons

### A. Home / Welcome Screen

- **Claude Code**: Features Clawd mascot (ASCII art), welcoming back the user by name, accompanied by a 2-column feed displaying recent release notes, activity, and onboarding guides.
- **Brain TUI**: Features a clean ASCII logo, a `SystemStatusWidget` showing daemon latency, and a `MemoryContextWidget` displaying active graph nodes and sessions.

### B. Command Palette vs. Quick Discovery

- **Claude Code**: Uses modal overlay dialogs centered on screen with rounded box borders (`┌─┐│└─┘`). Text inputs sit at the top of the modal, while results scroll cleanly below.
- **Brain TUI**: `PaletteWidget` floats in the top-center of the screen. Supports command search, entity search, and session switching with clean category icons.

### C. Status Footer

- **Claude Code**: Single-line horizontal footer at the absolute bottom margin. Uses `space-between` flex alignment to show mode badges, active model, token window %, cost in USD, and a rate-limit progress bar.
- **Brain TUI**: Single-line horizontal status bar (`status_footer.rs`). Displays connectivity status (Daemon vs. Embedded), active workspace state, daemon query latency (e.g., `12ms`), and execution spinners.

---

## 4. Animation & Motion Design

- **Claude Code**: Implements a dedicated **Shimmer System**. Spinner components alternate between base colors and `*Shimmer` companion tokens every 80ms, creating a pulsing breathing motion effect.
- **Brain TUI**: Uses smooth typewriter token streaming via `TypewriterQueue` and frame-rate capped spinners, ensuring zero screen tearing or stuttering during heavy data transfers.
