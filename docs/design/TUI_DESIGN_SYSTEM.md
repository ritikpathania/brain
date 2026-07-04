# TUI Design System Specification

**Version:** 1.0.0  
**Precedence:** Canonical Specification (If implementation and documentation disagree, the Design System is authoritative.)

Welcome to the canonical specification for the **Brain Terminal User Interface (TUI) Design System**. This system governs the visual presentation layer, screen layouts, motion behaviors, and accessibility invariants for all terminal-based Brain clients.

---

## 🚫 Non-Goals
This design system explicitly excludes the following from its scope:
* **Pixel-Perfect Consistency**: Monospace font families, cell ratios, and line-heights are set by the terminal emulator. Layout scaling is liquid and grid-based, not pixel-locked.
* **Mouse-First UX**: Mouse operations are convenient add-ons. No workflow, selection menu, or dialog submission may require mouse input.
* **Client-Specific Optimizations**: Design layouts must remain cross-platform and standard ANSI-compliant, avoiding proprietary terminal emulator layout hacks.
* **Complex UI Animations**: Movement is limited strictly to low-overhead progress interpolations and cursor/spinner sequences defined in [MOTION.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/MOTION.md).

---

## 🗺️ Design System Map

The design system specification is organized into modular guides:

```text
TUI_DESIGN_SYSTEM.md (You are here)
├── Foundations
│   ├── UX_PRINCIPLES.md ────────────► Core user experience philosophies
│   ├── DESIGN_INVARIANTS.md ────────► Strict visual & functional laws
│   └── INFORMATION_ARCHITECTURE.md ─► Content categorization & display
├── Styling & Assets
│   ├── THEMING.md ──────────────────► Semantic color token specifications
│   ├── THEME_TOKENS.md ─────────────► Physical RGB/ANSI color values map
│   └── MOTION.md ───────────────────► Spinners, typewriter & transitions
├── Interaction
│   ├── INTERACTION_MODEL.md ────────► Lifecycle state machine behaviors
│   └── KEYBINDINGS.md ──────────────► Terminal keyboard shortcuts
└── Layouts & Technical
    ├── COMPONENT_LIBRARY.md ────────► Stateless widget specifications
    ├── RESPONSIVE_LAYOUTS.md ───────► Width breakpoints & sizing rules
    ├── TERMINAL_CAPABILITIES.md ────► Fallbacks for remote/basic environments
    ├── PERFORMANCE_BUDGET.md ───────► CPU, startup, & rendering limits
    └── EXTENSION_PHILOSOPHY.md ─────► Rules for custom widgets & themes
```

---

## 🎨 Foundations & Vision
Brain's terminal client is designed to feel highly cohesive, premium, and human-centric. Rather than defaulting to black-and-white, it incorporates a warm brand theme, smooth typewriter streaming pacing, clear step-by-step progress timelines, and robust accessibility standards.

### Specifications Links
* **[UX Principles](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/UX_PRINCIPLES.md)**: The 7 core principles, starting with *The Chat is Primary* and *Typing is Sacred*.
* **[Design Invariants](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/DESIGN_INVARIANTS.md)**: Rules that must never be broken (e.g. typing never loses focus).
* **[Information Architecture](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/INFORMATION_ARCHITECTURE.md)**: Visibility tiers (Always Visible, Contextual, Hidden).
* **[Theming Specification](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/THEMING.md)**: Semantic token definitions and contrast ratios.
* **[Theme Tokens Mappings](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/THEME_TOKENS.md)**: Physical color maps for dark, light, daltonized, and ANSI terminals.
* **[Motion Specification](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/MOTION.md)**: Spinner arrays, typewriter queue buffers, and visual decays.
* **[Interaction Model](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/INTERACTION_MODEL.md)**: State matrix defining idle, typing, planning, and streaming.
* **[Keybindings Specification](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/KEYBINDINGS.md)**: Complete keyboard shortcut maps and routing rules.
* **[Component Library](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/COMPONENT_LIBRARY.md)**: Specifications for widgets (StatusBar, Sidebar, Input, Chat, Dialogs).
* **[Responsive Layouts](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/RESPONSIVE_LAYOUTS.md)**: Width breakpoints (Compact, Standard, Wide, Ultra-wide).
* **[Terminal Capabilities](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/TERMINAL_CAPABILITIES.md)**: Degraded fallback standards for color, borders, and OSC-8 links.
* **[Performance Budget](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/PERFORMANCE_BUDGET.md)**: Frame render budgets, startup limits, and memory bounds.
* **[Extension Philosophy](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/EXTENSION_PHILOSOPHY.md)**: Safe extension pathways for new themes, widgets, and transitions.
