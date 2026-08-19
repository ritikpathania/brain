# Brain Frontend Design Authority & Technical Specifications Index

This directory contains the canonical visual contracts, component models, capability surface mappings, and subordinate engineering specifications for the Brain Terminal User Interface (`crates/brain-tui`).

---

## 1. Primary Canonical Design Authorities

* **[`CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md)**: **Sole Canonical Visual & Interaction Authority**. Defines the normative visual grammar, typographic hierarchy, warm terracotta/neutral palette, borderless conversation floor, and two-region vertical stack.
* **[`CLAUDE_COMPONENT_MODEL.md`](./CLAUDE_COMPONENT_MODEL.md)**: **Canonical Component Architecture**. Defines the 18 reusable component primitives.
* **[`BRAIN_CLAUDE_SURFACE_MAPPING.md`](./BRAIN_CLAUDE_SURFACE_MAPPING.md)**: **Canonical Capability Projection Contract**. Governs how Brain's relational memory engine and search capabilities project onto Claude visual surfaces without inventing unbacked cloud features.
* **[`BRAIN_PRODUCT_CAPABILITIES_ROADMAP.md`](./BRAIN_PRODUCT_CAPABILITIES_ROADMAP.md)**: **Canonical Product Roadmap**. Exhaustive inventory of Brain's backend and product capabilities.

---

## 2. Subordinate Technical & Engineering Specifications

All specifications below are subordinate to and strictly governed by [`CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md):

* **[`TUI_DESIGN_SYSTEM.md`](./TUI_DESIGN_SYSTEM.md)**: Ratatui layout container implementation guide.
* **[`COMPONENT_LIBRARY.md`](./COMPONENT_LIBRARY.md)**: Widget implementation catalog for the 18 component primitives.
* **[`INFORMATION_ARCHITECTURE.md`](./INFORMATION_ARCHITECTURE.md)**: Screen hierarchy and progressive disclosure rules.
* **[`INTERACTION_MODEL.md`](./INTERACTION_MODEL.md)**: Interaction states, focus traversal, and prompt-first state machine.
* **[`KEYBINDINGS.md`](./KEYBINDINGS.md)**: Keyboard shortcuts and Emacs/Vim line editing bindings.
* **[`RESPONSIVE_LAYOUTS.md`](./RESPONSIVE_LAYOUTS.md)**: Terminal resize behavior, geometry math, and compact degradation.
* **[`MOTION.md`](./MOTION.md)**: 60fps refresh loop budgets, typewriter queue timing, and 80ms spinner cycling.
* **[`ACCESSIBILITY.md`](./ACCESSIBILITY.md)**: WCAG AA contrast tokens, screen-reader landmarks, and ANSI 16-color fallbacks.
* **[`PERFORMANCE_BUDGET.md`](./PERFORMANCE_BUDGET.md)**: Draw frame latency budgets (<66ms) and memory limits.
* **[`DESIGN_INVARIANTS.md`](./DESIGN_INVARIANTS.md)**: Whitespace before chrome, zero decorative fluff, responsive flow.
* **[`TERMINAL_CAPABILITIES.md`](./TERMINAL_CAPABILITIES.md)**: ANSI, 256-color, and TrueColor detection.
* **[`EXTENSION_PHILOSOPHY.md`](./EXTENSION_PHILOSOPHY.md)**: Plugin UI boundary guidelines.
* **[`UX_PRINCIPLES.md`](./UX_PRINCIPLES.md)**: Speed, cognitive clarity, and low-friction axioms.

---

## 3. Superseded Historical Specifications

The following legacy specifications are **SUPERSEDED** and retained strictly for historical reference:

* **[`LANDING_PAGE.md`](./LANDING_PAGE.md)**: *Superseded by [`CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md)*.
* **[`VISUAL_LANGUAGE_V2.md`](./VISUAL_LANGUAGE_V2.md)**: *Superseded by [`CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md)*.
* **[`THEME_TOKENS.md`](./THEME_TOKENS.md)**: *Superseded by [`CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md)*.
* **[`THEMING.md`](./THEMING.md)**: *Superseded by [`CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md)*.
