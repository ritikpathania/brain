# Forensic Audit: Obsolete Brain UI/UX Documentation vs. Claude Visual Contract

**Audit Date**: 2026-08-14  
**Audit Scope**: Complete Repository Non-Source Artifact Corpus (442 Files)  
**Baseline**: Certified Phase 0–10 Documentation Governance Baseline  
**Product Objective**: Forensic audit of documentation encoding the obsolete Brain-specific UI/UX/design language to establish a disciplined path toward the **Claude Visual Contract** as the sole frontend visual authority.

---

## 1. Executive Summary

Brain's certified production backend (hybrid BM25/vector search, relational knowledge graph, session tracking, UDS streaming protocol) is stable and frozen. However, the documentation baseline contains a significant corpus of legacy UI/UX specifications that encode the **obsolete Brain-specific visual language**:
- Pixel-art mascot ("clawd" / Memory Core)
- Electric purple / neon cyan cyberpunk color tokens (`#6C5CE7`, `#00CEC9`)
- "Think once. Remember forever." promotional banner framing
- "Connected: Yes | Ready: Yes" system telemetry status cards on the home screen
- Heavy box-in-a-box terminal chrome with double borders and nested cards
- Slash-command primacy rather than conversational prompt-first interaction

Under the new product directive:
> **Brain's frontend will faithfully reproduce Claude's UI/UX and visual language as closely as possible, while retaining Brain's own backend, capabilities, data model, and product semantics.**

This audit inventories all 442 artifacts, categorizes their UI dependencies, identifies conflicts, and provides explicit classification recommendations.

---

## 2. Complete Artifact Classification & Conflict Inventory

### A. Active Canonical Design Specifications (`docs/design/`)

| Path | Authority | Role | Lifecycle | Current Claim | Old UI Dependency | Claude Compatibility | Action | Recommended `superseded_by` |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| [`docs/design/LANDING_PAGE.md`](./LANDING_PAGE.md) | `canonical` | `contract` | `active` | Mandates pixel-art mascot, "Think once" tagline, status pills, and 3 suggestion cards. | **Direct** (Heavy pixel art & status boxes) | **CONFLICTING** | `SUPERSEDE` | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/VISUAL_LANGUAGE_V2.md`](./VISUAL_LANGUAGE_V2.md) | `canonical` | `contract` | `active` | Proposes electric purple palette, pixel art, and terminal neon aesthetic. | **Direct** (Cyberpunk/purple palette) | **CONFLICTING** | `SUPERSEDE` | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/THEME_TOKENS.md`](./THEME_TOKENS.md) | `canonical` | `contract` | `active` | Defines raw hex color values centered on violet `#6C5CE7` and cyber cyan. | **Direct** (Old brand color tokens) | **SUPERSEDE** | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/THEMING.md`](./THEMING.md) | `canonical` | `contract` | `active` | Defines theme engine with `brain_dark`, `cyberpunk`, and `nord` presets. | **Direct** (Old palette definitions) | **SUPERSEDE** | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/TUI_DESIGN_SYSTEM.md`](./TUI_DESIGN_SYSTEM.md) | `canonical` | `contract` | `active` | Outlines 3-region viewport grid, Ratatui rendering, and status footer chips. | **Direct** (Footer chrome & layout divisions) | **PARTIALLY_CONFLICTING** | `PARTIALLY_RECONCILE` | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/COMPONENT_LIBRARY.md`](./COMPONENT_LIBRARY.md) | `canonical` | `contract` | `active` | Defines terminal cards, memory inspector widgets, and box containers. | **Direct** (Card styling & box borders) | **PARTIALLY_CONFLICTING** | `PARTIALLY_RECONCILE` | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/INFORMATION_ARCHITECTURE.md`](./INFORMATION_ARCHITECTURE.md) | `canonical` | `contract` | `active` | Defines screen navigation, command modals, and workspace hierarchy. | **Indirect** (Screen flow & modal layering) | **PARTIALLY_CONFLICTING** | `PARTIALLY_RECONCILE` | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/INTERACTION_MODEL.md`](./INTERACTION_MODEL.md) | `canonical` | `contract` | `active` | Defines keyboard-first prompt editor, modal popups, and focus states. | **Indirect** (Focus traversal & modals) | **PARTIALLY_CONFLICTING** | `PARTIALLY_RECONCILE` | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/KEYBINDINGS.md`](./KEYBINDINGS.md) | `canonical` | `contract` | `active` | Defines Ctrl+K palette, Ctrl+O card expansion, and Vim navigation. | **Indirect** (Keyboard navigation grammar) | **COMPATIBLE** | `KEEP_SUPPORTING` | N/A |
| [`docs/design/RESPONSIVE_LAYOUTS.md`](./RESPONSIVE_LAYOUTS.md) | `canonical` | `contract` | `active` | Defines terminal resize math and compact (<80col) degradation. | **Indirect** (Geometry constraints) | **COMPATIBLE** | `KEEP_SUPPORTING` | N/A |
| [`docs/design/MOTION.md`](./MOTION.md) | `canonical` | `contract` | `active` | Defines spinner tick rates (80ms) and typewriter queue timing. | **Indirect** (Tick rates & smooth drain) | **COMPATIBLE** | `KEEP_SUPPORTING` | N/A |
| [`docs/design/ACCESSIBILITY.md`](./ACCESSIBILITY.md) | `canonical` | `contract` | `active` | Mandates WCAG AA contrast, high contrast mode, and ANSI fallbacks. | **Indirect** (Contrast & terminal fallbacks) | **COMPATIBLE** | `KEEP_SUPPORTING` | N/A |
| [`docs/design/PERFORMANCE_BUDGET.md`](./PERFORMANCE_BUDGET.md) | `canonical` | `contract` | `active` | Mandates <66ms draw latency and memory footprint constraints. | **None** (Runtime performance budget) | **COMPATIBLE** | `KEEP_SUPPORTING` | N/A |
| [`docs/design/DESIGN_INVARIANTS.md`](./DESIGN_INVARIANTS.md) | `canonical` | `contract` | `active` | Mandates whitespace before chrome, no decorative fluff, responsive flexbox. | **Indirect** (Axiomatic design principles) | **COMPATIBLE** | `KEEP_SUPPORTING` | N/A |
| [`docs/design/EXTENSION_PHILOSOPHY.md`](./EXTENSION_PHILOSOPHY.md) | `canonical` | `contract` | `active` | Governs third-party widget guidelines and custom theme boundaries. | **Indirect** (Extensibility rules) | **COMPATIBLE** | `KEEP_SUPPORTING` | N/A |
| [`docs/design/TERMINAL_CAPABILITIES.md`](./TERMINAL_CAPABILITIES.md) | `canonical` | `contract` | `active` | Defines TrueColor, 256-color, and ASCII terminal feature detection. | **None** (Terminal environment capabilities) | **COMPATIBLE** | `KEEP_SUPPORTING` | N/A |
| [`docs/design/UX_PRINCIPLES.md`](./UX_PRINCIPLES.md) | `canonical` | `contract` | `active` | Defines speed, low friction, and semantic clarity as design axioms. | **Indirect** (Axiomatic usability guidelines) | **COMPATIBLE** | `KEEP_SUPPORTING` | N/A |
| [`docs/design/BRAIN_PRODUCT_CAPABILITIES_ROADMAP.md`](./BRAIN_PRODUCT_CAPABILITIES_ROADMAP.md) | `canonical` | `governance` | `active` | Exhaustive capability matrix of Brain's backend and product features. | **None** (Pure backend capability inventory) | **COMPATIBLE** | `KEEP_CANONICAL` | N/A (Remains Canonical Product Roadmap) |
| [`docs/design/README.md`](./README.md) | `supporting` | `governance` | `active` | Navigational sitemap for the design directory. | **Indirect** (Directory index links) | **PARTIALLY_CONFLICTING** | `PARTIALLY_RECONCILE` | N/A |

---

### B. Architecture & Reference Specifications Touching UI

| Path | Authority | Role | Lifecycle | Current Claim | Old UI Dependency | Claude Compatibility | Action | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| [`docs/architecture/STABLE_UI_INVARIANTS.md`](../architecture/STABLE_UI_INVARIANTS.md) | `canonical` | `governance` | `active` | Architectural invariants governing state reducers, layout purity, and theme token usage. | **Indirect** (UI state isolation) | **PARTIALLY_CONFLICTING** | `PARTIALLY_RECONCILE` | Invariants are architecturally sound; UI component descriptions must align with Claude contract. |
| [`docs/subsystems/tui.md`](../../docs/subsystems/tui.md) | `supporting` | `operational` | `active` | Subsystem guide describing Ratatui architecture, event routing, and rendering loops. | **Direct** (Old component references) | **PARTIALLY_CONFLICTING** | `PARTIALLY_RECONCILE` | Technical architecture valid; component references should point to `CLAUDE_VISUAL_CONTRACT.md`. |
| [`docs/reference/cli-ux-comparison.md`](../reference/cli-ux-comparison.md) | `supporting` | `evidence` | `active` | Comparative UX analysis across modern AI CLI tools (Claude, Cursor, Codex). | **None** (Comparative research) | **COMPATIBLE** | `KEEP_SUPPORTING` | Valuable context documenting the empirical rationale for Claude UI convergence. |
| [`docs/architecture/rfc/RFC-004.md`](../architecture/rfc/RFC-004.md) | `canonical` | `contract` | `active` | RFC specifying Brain Terminal UX and Ratatui migration. | **Direct** (Historical UX proposal) | **PARTIALLY_CONFLICTING** | `KEEP_SUPPORTING` | Delegated RFC establishing Ratatui adoption; superseded in visual details by `CLAUDE_VISUAL_CONTRACT.md`. |
| [`docs/architecture/rfc/RFC-008.md`](../architecture/rfc/RFC-008.md) | `canonical` | `contract` | `active` | RFC specifying differential rendering and typewriter queue pipeline. | **None** (Low-level rendering engine) | **COMPATIBLE** | `KEEP_SUPPORTING` | Core technical rendering mechanism remains 100% active. |
| [`docs/architecture/rfc/RFC-009.md`](../architecture/rfc/RFC-009.md) | `canonical` | `contract` | `active` | RFC specifying modal dialog permissions and keyboard review. | **Indirect** (Modal interaction state) | **COMPATIBLE** | `KEEP_SUPPORTING` | Modal permission pattern matches Claude tool approval modals. |

---

### C. Active Crate & Application READMEs

| Path | Authority | Role | Lifecycle | Current Claim | Action |
| :--- | :--- | :--- | :--- | :--- | :--- |
| [`crates/brain-tui/README.md`](../../crates/brain-tui/README.md) | `supporting` | `operational` | `active` | References `TUI_DESIGN_SYSTEM.md` as canonical specification. | `PARTIALLY_RECONCILE` (Update canonical reference to `CLAUDE_VISUAL_CONTRACT.md`) |
| [`apps/brain/README.md`](../../apps/brain/README.md) | `supporting` | `operational` | `active` | References CLI application entry point and interactive UI launcher. | `KEEP_SUPPORTING` |
| [`daemon/README.md`](../../daemon/README.md) | `supporting` | `operational` | `active` | Headless background daemon service with UDS socket interface. | `KEEP_SUPPORTING` |

---

### D. Reverse-Engineered Research & Evidence (`docs/ux/` & `docs/research/`)

| Path | Role | Action | Description |
| :--- | :--- | :--- | :--- |
| `docs/ux/CLAUDE_VISUAL_CONTRACT.md` | `evidence` | `KEEP_SUPPORTING` | Detailed reverse-engineered specification of Claude Code's Ink/Yoga frontend. |
| `docs/ux/CLAUDE_COMPONENT_MODEL.md` | `evidence` | `KEEP_SUPPORTING` | Technical deconstruction of Claude's component tree. |
| `docs/ux/CLAUDE_BRAIN_VISUAL_GAP_MATRIX.md` | `evidence` | `KEEP_SUPPORTING` | Cell-level visual gap matrix between Claude Code and Brain. |
| `docs/research/CLAUDE_UX_BASELINE.md` | `evidence` | `KEEP_SUPPORTING` | Comprehensive reference baseline of Claude Code's CLI interaction. |
| `docs/research/CLAUDE_UX_DESIGN_ATLAS.md` | `evidence` | `KEEP_SUPPORTING` | Visual layout atlas and component dimensions. |
| `docs/research/CLAUDE_FEATURE_MATRIX.md` | `evidence` | `KEEP_SUPPORTING` | Feature-by-feature matrix comparing Claude and Brain capabilities. |

---

### E. Historical Archives (`docs/archive/frontend-parity/`, `docs/archive/historical-adrs/`, `docs/archive/migration/`)

All **74 files in `docs/archive/frontend-parity/`**, **6 files in `docs/archive/historical-adrs/`**, and **3 files in `docs/archive/migration/`** are classified as `HISTORICAL_PRESERVE`. Their substantive bodies are **verbatim preserved** as immutable point-in-time evidence and audit logs.

---

## 3. Summary of Document Actions

```yaml
audit_summary:
  total_documents_scanned: 442
  old_ui_documents_found: 246
  canonical_conflicts: 4
  supporting_conflicts: 12
  historical_documents: 141
  review_required: 0

action_breakdown:
  SUPERSEDE: 4
    - docs/design/LANDING_PAGE.md
    - docs/design/VISUAL_LANGUAGE_V2.md
    - docs/design/THEME_TOKENS.md
    - docs/design/THEMING.md
  PARTIALLY_RECONCILE: 12
    - docs/design/TUI_DESIGN_SYSTEM.md
    - docs/design/COMPONENT_LIBRARY.md
    - docs/design/INFORMATION_ARCHITECTURE.md
    - docs/design/INTERACTION_MODEL.md
    - docs/design/README.md
    - docs/architecture/STABLE_UI_INVARIANTS.md
    - docs/subsystems/tui.md
    - crates/brain-tui/README.md
    - (Supporting sitemaps & guides)
  KEEP_CANONICAL: 34
    - docs/design/BRAIN_PRODUCT_CAPABILITIES_ROADMAP.md
    - docs/architecture/CONSTITUTION.md
    - docs/architecture/ARCHITECTURE_INVARIANTS.md
    - docs/reference/storage.md
    - docs/reference/protocol.md
    - (Other backend/system specifications)
  KEEP_SUPPORTING: 251
    - docs/design/KEYBINDINGS.md
    - docs/design/RESPONSIVE_LAYOUTS.md
    - docs/design/MOTION.md
    - docs/design/ACCESSIBILITY.md
    - docs/design/PERFORMANCE_BUDGET.md
    - docs/design/DESIGN_INVARIANTS.md
    - docs/design/EXTENSION_PHILOSOPHY.md
    - docs/design/TERMINAL_CAPABILITIES.md
    - docs/design/UX_PRINCIPLES.md
    - docs/reference/cli-ux-comparison.md
    - docs/ux/* (Claude research evidence)
    - docs/research/* (Claude research baseline)
    - Crate READMEs
  HISTORICAL_PRESERVE: 141
    - docs/archive/frontend-parity/* (74 files)
    - docs/archive/historical-adrs/* (6 files)
    - docs/archive/migration/* (3 files)
    - docs/archive/sprint-reports/* (35 files)
    - crates/brain-services/tests/evaluation/* (12 files)
    - packages/brain-frontend/* (2 files)
    - Other immutable records
```
