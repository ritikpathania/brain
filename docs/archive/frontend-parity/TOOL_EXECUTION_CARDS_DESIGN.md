# Design Specification — Inline Tool Execution Cards & Collapsible Result Drawers

> **Document Status**: Approved Design Specification  
> **Target Subsystem**: `crates/brain-tui` (Tool Execution Presentation & Interaction Layer)  
> **Governing Forensic Audit**: [`docs/design/CLAUDE_TOOL_EXECUTION_FORENSIC_AUDIT.md`](CLAUDE_TOOL_EXECUTION_FORENSIC_AUDIT.md)  
> **Locked Systems Protection**: Two-Pass Layout Engine, Inline Collapsible Thinking Blocks, New Messages Pill, Multiline Prompt Cursor  
> **Final Recommendation Gate**: `APPROVED FOR IMPLEMENTATION`  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

## 1. Executive Summary & Design Scope

This design specification details the minimal native Rust/Ratatui implementation required to reproduce Claude Code's **Inline Tool Execution Cards & Collapsible Result Drawers** in `crates/brain-tui`.

### Core Capabilities Designed:
1. **Tool Execution Card Header**: Renders status icon (`⏺` running, `✔` success, `✖` error/rejection), bold tool name (e.g. `FileRead`, `Bash`), arguments in dim parentheses (e.g. `(crates/brain-tui/src/state.rs)`), and elapsed duration/status text.
2. **Collapsible Result Drawer**: Outputs default to collapsed (1 visual row with a `(ctrl+o to expand)` hint). Pressing `Ctrl+O` expands the output drawer to show up to 20 lines of formatted text.
3. **Deterministic `Ctrl+O` Routing**: Resolves multi-target expansion collisions between active `ThinkingBlock` and active `ToolExecutionCard` without mutating locked `ThinkingBlockWidget` contracts.
4. **Two-Pass Layout Engine Integration**: Collapsed height $= 1$ row; expanded height $= 1 + \min(\text{visual\_lines}, 20)$ rows.

---

## 2. Component Design & Type Contracts

All changes remain strictly inside `crates/brain-tui`. Zero backend, UDS, domain, or Cargo dependency changes.

```text
UiState
  ├── active_tool_calls: Vec<ToolExecution> (existing model in tool.rs)
  └── expanded_tool_calls: HashSet<ToolCallId> (NEW: tracks user-expanded cards)
        │
        ▼
ToolExecutionCardViewModel (NEW: pure visual presentation view model)
        │
        ▼
ToolExecutionCardWidget (NEW: Ratatui layout & rendering component in tool_card.rs)
```

### Proposed Types (`crates/brain-tui/src/ui/widgets/tool_card.rs`):

```rust
/// Pure view model for rendering an inline tool execution card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionCardViewModel {
    /// Unique identifier of the tool call.
    pub call_id: ToolCallId,
    /// User-facing tool display name (e.g. "FileRead", "Bash").
    pub tool_name: String,
    /// Formatted arguments string (e.g. "crates/brain-tui/src/state.rs").
    pub arguments: String,
    /// Status classification.
    pub status: ToolExecutionStatus,
    /// Whether the result drawer is expanded by user via Ctrl+O.
    pub is_expanded: bool,
    /// Full multiline result or error text.
    pub output_text: String,
}

/// Ratatui widget responsible for drawing tool execution cards.
pub struct ToolExecutionCardWidget<'a> {
    pub vm: &'a ToolExecutionCardViewModel,
}
```

---

## 3. Tool Lifecycle Mapping

Mapping the 6 source-confirmed Claude states to Brain's existing `ToolExecutionStatus` enum (`crates/brain-tui/src/ui/command/tool.rs`):

| Claude Lifecycle State | Brain `ToolExecutionStatus` | Status Symbol | Symbol Style | Output Drawer Available | Default Collapsed Summary |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **1. Queued** | `ToolExecutionStatus::Approved` | `⏺` (`\u25CF`) | `ThemeToken::TextDim` | No | `⏺ ToolName(args) queued...` |
| **2. InProgress** | `ToolExecutionStatus::Running` | `⏺` (`\u25CF`) | `ThemeToken::TextAccent` | Partial / Live Logs | `⏺ ToolName(args) running...` |
| **3. WaitingForPermission** | `ToolExecutionStatus::PendingApproval` | `⏺` (`\u25CF`) | `ThemeToken::TextWarning` | No | `⏺ ToolName(args) waiting for approval...` |
| **4. Completed / Success** | `ToolExecutionStatus::Completed` | `✔` (`\u2714`) | `ThemeToken::TextSuccess` | Yes | `✔ ToolName(args) (ctrl+o to expand)` |
| **5. Completed / Error** | `ToolExecutionStatus::Failed` | `✖` (`\u2716`) | `ThemeToken::TextError` | Yes (Error details) | `✖ ToolName(args) failed (ctrl+o to expand)` |
| **6. Rejected / Denied** | `ToolExecutionStatus::Denied` | `✖` (`\u2716`) | `ThemeToken::TextError` | Yes (Reason) | `✖ ToolName(args) permission denied` |

---

## 4. Visual Contract & Rendering Rules

### Header Line Format:
```text
<Symbol> <BoldToolName>(<DimArguments>) <OptionalExpansionHint>
```

- **Symbol**: 1-cell width status icon (`⏺`, `✔`, `✖`).
- **Bold Tool Name**: `Span::styled(tool_name, Style::default().add_modifier(Modifier::BOLD))`.
- **Dim Arguments**: `Span::styled(format!("({})", args), Style::default().fg(theme.text_dim))`.
- **Expansion Hint**: Rendered in `ThemeToken::TextDim` at right edge when collapsed: `(ctrl+o to expand)`.

### Collapsed Mode (Default):
- Height $= 1$ visual row.
- Renders header line only.

### Expanded Mode (`Ctrl+O` Toggled):
- Renders header line on line 0.
- Renders output text indented by 2 spaces starting on line 1.
- Max visible lines $= 20$. If `output_lines > 20`, renders first 20 visual lines followed by a `... (N lines truncated)` indicator line in `ThemeToken::TextDim`.

---

## 5. State Model & Ownership

1. **State Storage**: `expanded_tool_calls: HashSet<ToolCallId>` in `UiState` ([`crates/brain-tui/src/state.rs`](../../../crates/brain-tui/src/state.rs)).
2. **Default State**: Unexpanded (`is_expanded = false`).
3. **Action Dispatch**: `Action::ToggleToolCardExpansion(Option<ToolCallId>)` added to `Action` enum.
4. **Persistence**: Expansion state persists across UI repaints and screen resizes.

---

## 6. Deterministic Key Routing & `Ctrl+O` Precedence

To prevent routing collisions with the locked `ThinkingBlockWidget` and overlays, `InputRouter::route_key_event` will follow this strict priority matrix for `Ctrl+O` / `Alt+T`:

```text
Key Event (Ctrl+O / Alt+T)
    │
    ├── 1. Modals / Overlays Open? ──► Handled by Overlay (Do not toggle inline cards)
    │
    ├── 2. Focused/Latest Timeline Target:
    │     ├── Active Thinking Block present & running? ──► Action::ToggleThinkingBlock
    │     ├── Active Tool Execution present? ──► Action::ToggleToolCardExpansion(latest_tool_id)
    │     └── Fallback: If thinking block exists ──► Action::ToggleThinkingBlock
```

This guarantees **100% backward compatibility** with the locked `ThinkingBlockWidget` tests while cleanly supporting tool card expansion.

---

## 7. Two-Pass Layout Engine & Measurement Integration

Zero architectural changes to the Two-Pass layout engine (`LayoutEngine`).

### Intrinsic Height Calculation Contract:
```rust
pub fn measure_tool_card(
    vm: &ToolExecutionCardViewModel,
    usable_width: u16,
) -> u16 {
    if !vm.is_expanded || vm.output_text.is_empty() {
        return 1; // Collapsed height = 1 row
    }

    let indented_width = (usable_width as usize).saturating_sub(2).max(1);
    let mut visual_lines = 0;

    for line in vm.output_text.lines() {
        let line_len = line.chars().count();
        let rows = (line_len + indented_width - 1) / indented_width;
        visual_lines += rows.max(1);
    }

    let max_output_rows = 20;
    let rendered_output_rows = visual_lines.min(max_output_rows);
    let truncation_indicator_row = if visual_lines > max_output_rows { 1 } else { 0 };

    (1 + rendered_output_rows + truncation_indicator_row) as u16
}
```

---

## 8. Scroll Integration & Anchoring

- **Anchor Preservation**: When `ViewportState::follow_tail == false` (user scrolled up reading history), expanding or collapsing a tool execution card updates `timeline_height`, but `ScrollAnchor` recalculates `scroll_offset` so the visual lines currently visible to the user remain completely fixed (`BRAIN-CONFIRMED`).
- **Tail Following**: When `follow_tail == true`, expanding a tool card automatically adjusts `scroll_offset` to keep the newest tool output visible at the bottom.

---

## 9. Truncation & Width Semantics

- **Max Lines**: 20 lines maximum output height when expanded.
- **Visual vs. Logical Lines**: Truncation is computed on **visual wrapped lines** (`UNKNOWN` gap in Claude source resolved safely by using visual line bounds to prevent viewport overflow).
- **Control Characters**: Stripped or sanitized to prevent terminal breakage.

---

## 10. Existing Brain Code Base Integration Plan

Target files strictly inside `crates/brain-tui`:

1. [`crates/brain-tui/src/ui/widgets/tool_card.rs`](../../../crates/brain-tui/src/ui/widgets/tool_card.rs) (`[NEW]`): Create `ToolExecutionCardViewModel` and `ToolExecutionCardWidget`.
2. [`crates/brain-tui/src/ui/widgets/mod.rs`](../../../crates/brain-tui/src/ui/widgets/mod.rs) (`[MODIFY]`): Re-export `tool_card`.
3. [`crates/brain-tui/src/state.rs`](../../../crates/brain-tui/src/state.rs) (`[MODIFY]`): Add `expanded_tool_calls: HashSet<ToolCallId>` to `UiState`, add `Action::ToggleToolCardExpansion`, handle action in `UiState::update`.
4. [`crates/brain-tui/src/ui/interaction/router.rs`](../../../crates/brain-tui/src/ui/interaction/router.rs) (`[MODIFY]`): Update `Ctrl+O` routing precedence.
5. [`crates/brain-tui/src/ui/widgets/chat.rs`](../../../crates/brain-tui/src/ui/widgets/chat.rs) (`[MODIFY]`): Integrate `ToolExecutionCardWidget` into timeline rendering.
6. [`crates/brain-tui/tests/tool_card_tests.rs`](../../../crates/brain-tui/tests/tool_card_tests.rs) (`[NEW]`): Add unit & integration tests.

---

## 11. Testing & Verification Plan

### Proposed Tests (`tests/tool_card_tests.rs`):
1. `test_tool_card_lifecycle_status_symbols()`: Verifies `⏺`, `✔`, `✖` for all 6 states.
2. `test_tool_card_collapsed_height_is_one()`: Asserts intrinsic height $= 1$ when collapsed.
3. `test_tool_card_expanded_height_and_20_line_cap()`: Tests intrinsic height calculation and truncation indicator for $> 20$ lines.
4. `test_ctrl_o_tool_card_expansion_toggle()`: Tests `Ctrl+O` key routing and `expanded_tool_calls` state transition.
5. `test_thinking_block_ctrl_o_coexistence()`: Verifies that existing `ThinkingBlock` `Ctrl+O` expansion tests pass without regression.

---

## 12. Locked Subsystems Compatibility Matrix

| Locked Subsystem | Touched? | Risk Level | Safety Guarantee |
| :--- | :--- | :--- | :--- |
| **Two-Pass Layout Engine** | No | None | Uses standard `measure_chat` height contract |
| **Inline Collapsible Thinking Blocks** | No | None | Preserves `ToggleThinkingBlock` action & test suites |
| **New Messages Pill** | No | None | Floating overlay layer unaffected |
| **Multiline Prompt Cursor** | No | None | Editor state & key routing untouched |

---

## 13. Risk Analysis & Mitigation

- **Risk**: `Ctrl+O` key routing collision between Thinking Block and Tool Card.
  - **Mitigation**: Deterministic target resolution based on active timeline block.
- **Risk**: Timeline height changes causing scroll jump when reading history.
  - **Mitigation**: Locked `ScrollAnchor` automatically compensates for height delta.

---

## 14. Final Recommendation Gate

```text
APPROVED FOR IMPLEMENTATION
```
