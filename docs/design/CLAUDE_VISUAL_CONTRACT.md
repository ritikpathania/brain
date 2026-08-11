# Claude Visual Reference & Cell-Level Architecture Contract

> **Document Classification:** Official Bit-Exact Specification & Visual Oracle Contract for Claude Code CLI TUI.
> **Reference Baseline Authority:** `/Users/ritikpathania/Developer/src/DESIGN.md` and TypeScript source codebase in `/Users/ritikpathania/Developer/src`.

---

## 1. Reference Fixture Authority

### Source of Truth Invariant
- **Reference Authority**: All expected `CellSpec` grids MUST originate from the local Claude Code reference implementation in `/Users/ritikpathania/Developer/src`.
- **Prohibition on Self-Generation**: Expected reference fixtures MUST NEVER be generated from Brain's own renderer output. Screenshots serve as human visual context; the local source code is the sole authority for cell-level specifications.

---

## 2. Cell Specification & Terminal-Cell Semantics

### 8-Property Cell Spec
Visual parity is defined as a bit-exact equality of **all 8 cell properties** across every `(x, y)` coordinate in the terminal buffer:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
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

### Wide Character & Unicode Cell Semantics
- Comparisons are performed on `TestBackend` character cells after Ratatui layout resolution.
- Multi-column Unicode characters (e.g. `●`, `❯`, `▍`, `│`, `─`) and emojis occupy their resolved cell grid coordinates.

### Full-Buffer Diagnostic Formatting
When a cell oracle comparison fails, the test harness MUST output structured diagnostics:

```text
Visual Oracle Failure at Viewport 80×24
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
- **TUI Test Suite**: `crates/brain-tui/tests/`
- **Forbidden Crates**: `brain-domain`, `brain-core`, `brain-storage`, `brain-services`, `brain-events`. ZERO changes permitted.

---

## 4. Complete Border & Geometry Specification

### Home Welcome Surface Border Grammar (`80×24` Viewport)
- **Top Border Line** (`y = 2`):
  - `(1, 2)`: Corner `┌` (`fg: Some(Rgb(215,119,87))`, `#D77757`)
  - `(2, 2)`: Border `─` (`fg: Some(Rgb(215,119,87))`)
  - `(3, 2)..(23, 2)`: Integrated Title `" Claude Code v2.1.226 "` (`fg: Some(Rgb(215,119,87))`, `bold: true` for "Claude Code")
  - `(24, 2)..(77, 2)`: Border `─` (`fg: Some(Rgb(215,119,87))`)
  - `(78, 2)`: Corner `┐` (`fg: Some(Rgb(215,119,87))`)
- **Side Borders & Inner Divider** (`y = 3..9`):
  - `(1, y)`: Left border `│` (`fg: Some(Rgb(215,119,87))`)
  - `(47, y)`: Inner vertical divider `│` (`fg: Some(Rgb(80,80,80))`, `#505050`)
  - `(78, y)`: Right border `│` (`fg: Some(Rgb(215,119,87))`)
- **Bottom Border Line** (`y = 10`):
  - `(1, 10)`: Corner `└` (`fg: Some(Rgb(215,119,87))`)
  - `(2, 10)..(77, 10)`: Border `─` (`fg: Some(Rgb(215,119,87))`)
  - `(78, 10)`: Corner `┘` (`fg: Some(Rgb(215,119,87))`)

---

## 5. State & Reducer Interaction Matrix

### Authoritative State Transitions
Source: `/Users/ritikpathania/Developer/src`

```text
[State::Home]
   │
   ├── User types "/" ──► [State::CommandPaletteOpen] (Floating 3-column dropdown above prompt)
   │                           │
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

## 6. Negative Assertion Invariants

The following legacy structures MUST emit ZERO matches across all cell buffers:
1. `!buffer.contains("System Status")`
2. `!buffer.contains("Context")` (telemetry block)
3. `!buffer.contains("Daemon: Connected | Latency:")` (raw latency footer)
4. `!buffer.contains("Sessions (Active)")`
5. Vertical sidebar divider `│` at column `x = 22` in Workspace mode.
