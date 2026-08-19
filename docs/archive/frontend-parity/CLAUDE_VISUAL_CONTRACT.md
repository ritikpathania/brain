# Claude Visual Reference & Cell-Level Architecture Contract

> **Document Classification:** Official Bit-Exact Specification & Visual Oracle Contract for Claude Code CLI TUI.
> **Reference Baseline Authority:** `/Users/ritikpathania/Developer/src/DESIGN.md` and TypeScript source codebase in `/Users/ritikpathania/Developer/src`.

---

## 1. Reference Fixture Authority & Deterministic State Lock

### Source of Truth Invariant
- **Reference Authority**: All expected `CellSpec` grids MUST originate from the local Claude Code reference implementation in `/Users/ritikpathania/Developer/src`.
- **Prohibition on Self-Generation**: Expected reference fixtures MUST NEVER be generated from Brain's own renderer output. Screenshots serve as human visual context; the local source code is the sole authority for cell-level specifications.
- **Fixture Artifact Location**: `crates/brain-tui/tests/fixtures/claude_reference/*.json`

### Deterministic State Locking Matrix
To guarantee 100% test reproducibility across cell comparisons, all dynamic values are locked to fixed reference constants:

| State Field | Canonical Reference Constant |
|---|---|
| **Claude Version** | `v2.1.226` |
| **Model / Context** | `Opus 5 (1M context) with xhigh · API Usage Billing` |
| **Working Directory** | `~/Developer/PyCharm/brain` |
| **Agent Counts** | `4 awaiting input · 0 working · 17 completed` |
| **Task / Session Timestamps** | `2s` (active session), `11h` (idle session) |
| **Effort State** | `● xhigh · /effort` |
| **Prompt Focus State** | Focused (`❯ ` prefix with cursor `█`) |
| **Animation Frame** | Frame 0 (Static base colors: `claude` `#D77757`, no shimmer tick offset) |

---

## 2. Cell Specification & Terminal-Cell Semantics

### 8-Property Cell Spec
Visual parity is defined as a bit-exact equality of **all 8 cell properties** across every `(x, y)` coordinate in the terminal buffer:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CellSpec {
    pub symbol: String,
    pub fg: Option<ratatui::style::Color>,
    pub bg: Option<ratatui::style::Color>,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub dim: bool,
    pub reversed: bool,
}
```

### Reset vs None Semantics
- `None`: Attribute not explicitly specified by style (inherits floor style).
- `Some(Color::Reset)`: Explicit terminal default/reset sequence emitted by layout.
- **Equality Rule**: `None ≠ Some(Color::Reset)`. Both participate strictly in cell equality.

### Wide Character & Continuation Cell Semantics
- Symbols `●`, `❯`, `│`, `─`, `▍`, `▄`, `▀`, `█` and emojis are evaluated on Ratatui `TestBackend` character cell grid after width resolution.
- Wide multi-column glyphs compare primary cell symbol and trailing continuation cells exactly as rendered by `TestBackend`.

### Full-Grid Bounded Failure Diagnostics
`assert_cell_grid_eq(actual, expected, viewport)` evaluates all `W × H` coordinates and emits structured diagnostic blocks for all mismatches (bounded to first 20 errors):

```text
Visual Oracle Failure at Viewport 80×24 [Mismatch Count: 1]
Coordinate: (47, 3)

Expected:
  symbol = "│"
  fg = Some(Rgb(80, 80, 80))
  bg = None
  bold = false
  italic = false
  underlined = false
  dim = false
  reversed = false

Actual:
  symbol = "|"
  fg = None
  bg = None
  bold = false
  italic = false
  underlined = false
  dim = false
  reversed = false
```

---

## 3. Allowed Implementation Scope Boundary

- **Presentation Layer**: `crates/brain-tui/src/ui/`
- **TUI State & Reducer Layer**: `crates/brain-tui/src/state.rs`, `crates/brain-tui/src/ui/interaction/dispatcher.rs`
- **TUI Test Suite & Fixtures**: `crates/brain-tui/tests/`
- **Forbidden Crates**: `brain-domain`, `brain-core`, `brain-storage`, `brain-services`, `brain-events`. ZERO changes permitted.

---

## 4. Canonical RGB Color Tokens

Source: `/Users/ritikpathania/Developer/src/DESIGN.md` (Lines 31–119)

| Token Name | RGB Value | Hex Value | Cell Style & Modifiers | Usage in Reference |
|---|---|---|---|---|
| `claude` | `rgb(215,119,87)` | `#D77757` | `fg: Rgb(215,119,87), bold: true` | Brand orange logo, top welcome border title, active headers. |
| `claudeShimmer` | `rgb(235,159,127)` | `#EB9F7F` | `fg: Rgb(235,159,127)` | Breathing animation frames on odd ticks (~80ms). |
| `text` | `rgb(255,255,255)` | `#FFFFFF` | `fg: Rgb(255,255,255)` | Primary text on dark background floor. |
| `inactive` | `rgb(153,153,153)` | `#999999` | `fg: Rgb(153,153,153)` | Dimmed metadata, version strings, secondary hints. |
| `subtle` | `rgb(80,80,80)` | `#505050` | `fg: Rgb(80,80,80)` | Horizontal line rules (`─`), column dividers (`│`). |
| `promptBorder` | `rgb(136,136,136)` | `#888888` | `fg: Rgb(136,136,136)` | Unfocused prompt input border rules. |
| `suggestion` / `permission` | `rgb(177,185,249)` | `#AFB9F9` | `fg: Rgb(177,185,249)` | Command palette categories (`command ·`, `skill ·`), search highlights. |
| `selectionBg` | `rgb(38,79,120)` | `#264F78` | `bg: Rgb(38,79,120), fg: Rgb(255,255,255)` | Selected row in workspace dashboard and command palette. |
| `userMessageBackground` | `rgb(55,55,55)` | `#373737` | `bg: Rgb(55,55,55)` | Container fill for user prompt bubbles. |

---

## 5. Precise Unambiguous Geometry Specifications

### Canonical Horizontal Layout Topography (`80×24` Viewport)
- **Outer Surface Bounds**: `x = 1..78` (width 78)
- **Left Welcome Pane Interior**: `x = 2..46` (width 45)
- **Vertical Divider Column**: `x = 47` (glyph `│` in `#505050`)
- **Right Information Rail Interior**: `x = 48..77` (width 30)
- **Right Border Edge Column**: `x = 78` (glyph `│` in `#D77757`)

### Exact Top-Border Title Cell Geometry (`80×24` Viewport, Row `y = 2`)
The integrated title string `" Claude Code v2.1.226 "` consists of exactly **22 terminal cells**.

```text
x = 1       : ┌  (fg: Rgb(215, 119, 87))
x = 2       : ─  (fg: Rgb(215, 119, 87))
x = 3..24   : " Claude Code v2.1.226 "
              x = 3..9   : " Claude " (fg: Rgb(215,119,87), bold: true)
              x = 10..14 : "Code "   (fg: Rgb(215,119,87), bold: true)
              x = 15..24 : "v2.1.226 " (fg: Rgb(153,153,153), bold: false)
x = 25..77  : ───────────────────────────────────────────────────── (fg: Rgb(215,119,87))
x = 78      : ┐  (fg: Rgb(215, 119, 87))
```

### Complete Side & Bottom Border Cell Specs (`y = 3..10`)
- `(1, y)` for `y = 3..9`: Left border `│` (`fg: Rgb(215,119,87)`)
- `(47, y)` for `y = 3..9`: Vertical divider `│` (`fg: Rgb(80,80,80)`)
- `(78, y)` for `y = 3..9`: Right border `│` (`fg: Rgb(215,119,87)`)
- `(1, 10)`: Corner `└` (`fg: Rgb(215,119,87)`)
- `(2..77, 10)`: Bottom border `─` (`fg: Rgb(215,119,87)`)
- `(78, 10)`: Corner `┘` (`fg: Rgb(215,119,87)`)

---

## 6. Certified Viewport Matrix

| Viewport | Surface Bounds | Left Pane | Vertical Divider | Right Rail | Right Edge | Prompt Rows | Footer Row |
|---|---|---|---|---|---|---|---|
| **80×24** | `x = 1..78`, `y = 2..10` | `x = 2..46` | `x = 47` | `x = 48..77` | `x = 78` | `y = 20..22` | `y = 23` |
| **127×24** | `x = 1..125`, `y = 2..10` | `x = 2..72` | `x = 73` | `x = 74..124` | `x = 125` | `y = 20..22` | `y = 23` |
| **80×34** | `x = 1..78`, `y = 2..10` | `x = 2..46` | `x = 47` | `x = 48..77` | `x = 78` | `y = 30..32` | `y = 33` |
| **182×53** | `x = 1..180`, `y = 2..10` | `x = 2..104` | `x = 105` | `x = 106..179` | `x = 180` | `y = 49..51` | `y = 52` |

---

## 7. State & Reducer Interaction Matrix

```text
[State::Home]
   │
   ├── User types "/" ──► [State::CommandPaletteOpen]
   │                           ├── Key "Down" ──► Selection moves to row 1 (bg = #264F78)
   │                           ├── Key "Up"   ──► Selection moves to row 0
   │                           ├── Key "Esc"  ──► Closes palette, returns focus to prompt
   │                           └── Key "Enter" ──► Executes command, resets state
   │
   └── User presses "Ctrl+K" ──► [State::CommandPaletteOpen]

[State::Workspace] (Opened from Home or Conversation)
   │
   ├── Key "Down" / "Up" ──► Move selection highlight (row bg = #264F78)
   ├── Key "Enter" ──► Opens selected session ──► [State::Conversation]
   └── Key "Esc"   ──► Returns to previous foreground screen
                        (Screen::Home if opened from Home;
                         Screen::Conversation if opened from Conversation)
```

---

## 8. Negative Assertion Invariants

1. `!buffer.contains("System Status")`
2. `!buffer.contains("Context")`
3. `!buffer.contains("Daemon: Connected | Latency:")`
4. `!buffer.contains("Sessions (Active)")`
5. Vertical sidebar divider `│` at column `x = 22` in Workspace mode.
