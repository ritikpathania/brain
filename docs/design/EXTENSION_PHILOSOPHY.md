# Extension Philosophy

> **AUTHORITY NOTICE**: This document is a **supporting engineering specification** for `crates/brain-tui`, strictly subordinate to and governed by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).


This document outlines the rules for extending the TUI client codebase. These architectural constraints prevent code drift, keep the renderer decoupled from the state engine, and ensure that complexity does not accumulate in centralized drawing procedures.

---

## 1. Extension Rules

```
┌──────────────────────────────────────┐
│        Adding a New Widget           │
│  1. Create file in ui/widgets/       │
│  2. Define stateless draw() function │
│  3. Define custom ViewModel          │
│  * DO NOT touch AppRenderer          │
│    unless screen layout partition    │
│    must explicitly change.           │
└──────────────────────────────────────┘

┌──────────────────────────────────────┐
│        Adding a New Theme            │
│  1. Add constructor to Theme struct  │
│  2. Populate tokens in theme.rs      │
│  * DO NOT modify Theme struct fields │
│    or ThemeResolver logic.           │
└──────────────────────────────────────┘

┌──────────────────────────────────────┐
│       Adding an Animation            │
│  1. Define character frame arrays   │
│  2. Update State Reducer ticks       │
│  * DO NOT inject timers or loops     │
│    into RenderLoop.                  │
└──────────────────────────────────────┘
```

---

## 2. Guidelines for Developers

### 2.1. Adding a New Widget
To maintain the TUI's clean presentation layer:
1. **Purity**: Widgets must be stateless. They receive a read-only `ViewModel` and a bounding `Rect` coordinate, and render to the Crossterm backend frame.
2. **Decoupling**: Create the widget inside a dedicated file under `src/ui/widgets/`. Export a public `draw()` function.
3. **AppRenderer Boundary**: Do not modify `AppRenderer::draw` directly to manage widget properties. The `AppRenderer` layout organizer is responsible only for calculating window partitions and mapping views to coordinates.

### 2.2. Adding a Theme
To support new visual skins (e.g., retro green phosphor, high-contrast light):
1. **Constructor Instantiation**: Open [theme/mod.rs](../../crates/brain-tui/src/ui/theme/mod.rs) and write a new instantiation function (e.g., `pub fn classic_retro() -> Self`).
2. **Contract Boundaries**: Do not add new styling parameters to the `Theme` struct unless a brand-new component type is designed. If styling is missing, map it to an existing semantic token (e.g., map a warning background to `Warning`).

### 2.3. Adding Animations
To enrich the terminal experience with micro-animations:
1. **Ticks Drive Motion**: Animation state (active frame index, elapsed milliseconds) must be managed inside `state.rs` via standard clock tick actions.
2. **Pure Projection**: The widget simply draws the character matching `active_sequence_array[tick_index % length]`. Do not spawn separate asynchronous sleep loops or timing threads inside widgets.
