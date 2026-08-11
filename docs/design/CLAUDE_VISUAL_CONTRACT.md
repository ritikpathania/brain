# Claude Visual Reference & Cell-Level Architecture Contract

> **Document Classification:** Official Bit-Exact Specification & Visual Oracle Contract for Claude Code CLI TUI.
> **Reference Baseline:** `/Users/ritikpathania/Developer/src/DESIGN.md` and TypeScript source codebase in `/Users/ritikpathania/Developer/src`.

---

## 1. Zero-Trust Cell-Level Oracle Infrastructure

### Cell Specification Contract
Visual parity MUST NOT be asserted using loose substring matching (`contains()`). Parity is defined as a bit-exact equality of **all 8 cell properties** across every `(x, y)` coordinate in the terminal buffer:

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

A cell mismatch in ANY of the 8 properties (`symbol`, `fg`, `bg`, `bold`, `italic`, `underlined`, `dim`, `reversed`) at any coordinate `(x, y)` is a **CRITICAL TEST FAILURE**.

### Full-Buffer Diagnostic Formatting
When a cell oracle comparison fails, the test harness MUST output a structured diff detailing:
- Viewport size (`W × H`)
- Precise coordinate `(x, y)`
- Expected `CellSpec` vs Actual `CellSpec`

```text
Visual Oracle Failure at Viewport 80×24
Coordinate: (47, 3)

Expected:
  symbol = "│"
  fg = Rgb(80, 80, 80)
  bg = None
  bold = false
  italic = false
  underlined = false
  dim = false
  reversed = false

Actual:
  symbol = "|"
  fg = Reset
  bg = None
  bold = false
  italic = false
  underlined = false
  dim = false
  reversed = false
```

---

## 2. Allowed Implementation Scope

To prevent architectural drift while enabling full state machine fidelity:
- **Presentation Layer**: `crates/brain-tui/src/ui/`
- **TUI State & Reducer Layer**: `crates/brain-tui/src/state.rs`, `crates/brain-tui/src/ui/interaction/dispatcher.rs`
- **TUI Test Suite**: `crates/brain-tui/tests/`
- **Forbidden Crates**: `brain-domain`, `brain-core`, `brain-storage`, `brain-services`, `brain-events`. ZERO changes permitted.

---

## 3. Canonical Theme Tokens & RGB Palette

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

## 4. Viewport Geometry & Component Allocations

### Certified Viewport Matrix

#### A. 80×24 Viewport Baseline
- **Home Welcome Box**: `Rect::new(1, 2, 78, 9)`
  - Top border: `y = 2`, integrated title `" Claude Code v2.1.226 "` starting at `x = 3`.
  - Inner left pane: `x = 2..46`, `y = 3..10` (width 45). Mascot `▄▀▀▀▄` / `█ █ █` at `x = 6`, tagline `Think once. Remember.` at `x = 2, y = 8`.
  - Vertical divider column: `x = 47`, `y = 3..9`, glyph `│` in `subtle` gray (`#505050`).
  - Inner right rail: `x = 48..78`, `y = 3..10` (width 30). Section header `Tips for getting started` at `y = 3`, section divider `───` at `y = 5`, `What's new` at `y = 6`.
- **Ambient Status Row**: `x = 57..78`, `y = 19`, right-aligned: `"● xhigh · /effort"` (`●` in green `#4EBA65`).
- **Prompt Composer**:
  - Top divider rule: `y = 20`, full width `x = 0..80`, glyph `─` in `promptBorder` gray (`#888888`).
  - Input line: `y = 21`, prefix `❯ ` at `x = 0..2` in bold `claude` orange (`#D77757`), input text / placeholder starting at `x = 2`.
  - Bottom divider rule: `y = 22`, full width `x = 0..80`, glyph `─` in `promptBorder` gray (`#888888`).
- **Quiet Status Footer**: `y = 23`, full width `x = 0..80`, borderless: `" ▍▍ manual mode on · ? for shortcuts · ⬅ 3 agents"`.

#### B. 127×24 Viewport Baseline
- **Welcome Box**: `Rect::new(1, 2, 125, 9)`
- **Vertical Divider Column**: `x = 73` (58% left pane width).
- **Prompt Composer**: `y = 20..22`, full width 127 cols.

#### C. 80×34 Viewport Baseline
- **Welcome Box**: `Rect::new(1, 2, 78, 9)`
- **Transcript Scroll Buffer**: `y = 11..28` (18 rows canvas space).
- **Ambient Status Row**: `y = 29`.
- **Prompt Composer**: `y = 30..32`.
- **Quiet Status Footer**: `y = 33`.

#### D. 182×53 Viewport Baseline
- **Welcome Box**: `Rect::new(1, 2, 180, 9)`
- **Vertical Divider Column**: `x = 105`.
- **Prompt Composer**: `y = 49..51`.
- **Quiet Status Footer**: `y = 52`.

---

## 5. State & Reducer Interaction Matrix

### Authoritative State Transitions (Verified from Claude Reference `/Users/ritikpathania/Developer/src`)

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
   ├── User presses "Ctrl+K" ──► [State::CommandPaletteOpen]
   │
   └── User types query + "Enter" ──► [State::Conversation]
                                           │
                                           ├── Scroll buffer retains HomeWelcomeSurface at head
                                           ├── User prompt bubble added (bg = #373737)
                                           └── Assistant response streams (prefix = Claude:)

[State::Workspace] (Triggered by /session or workspace command)
   │
   ├── Header: "▄▀▀ Claude Code v2.1.226", context path, active task counts
   ├── Banner: "Your conversation moved to the background..."
   ├── Needs Input Table: Full-width row selection
   │     ├── Key "Down" ──► Selection moves down (row bg = #264F78, prefix = "* ")
   │     ├── Key "Up"   ──► Selection moves up
   │     └── Key "Enter" ──► Opens selected session ──► [State::Conversation]
   │
   └── Key "Esc" ──► Returns from Workspace ──► [State::Home / State::Conversation]
```

---

## 6. Negative Assertion Invariants

The following legacy structures MUST emit ZERO matches across all cell buffers:
1. `!buffer.contains("System Status")`
2. `!buffer.contains("Context")` (telemetry block)
3. `!buffer.contains("Daemon: Connected | Latency:")` (raw latency footer)
4. `!buffer.contains("Sessions (Active)")`
5. Vertical sidebar divider `│` at column `x = 22` in Workspace mode.
