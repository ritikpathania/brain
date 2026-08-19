# Forensic Audit: Detection & Elimination of Legacy Brain UI/UX Design Language

**Audit Status**: `COMPLETE (Strictly Read-Only Forensic Analysis)`  
**Audit Scope**: Complete 442-Artifact Repository Corpus  
**Certified Baseline**: Certified Phase 0–10 Documentation Governance Baseline  
**Target Visual Authority**: [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md)

---

## 1. Executive Summary

Brain's certified production baseline established a robust, acyclic, and portable documentation system across all 442 tracked non-source files. However, the existing active specifications still contain a substantial body of **legacy Brain UI/UX design language**—including 8-bit pixel art mascot avatars ("clawd" / Memory Core), electric purple / neon cyan cyberpunk color tokens (`#6C5CE7`, `#00CEC9`), heavy box-in-a-box dashboard chrome, and telemetry-first home screens.

The product directive establishes a clean, modern design evolution:
> **The Brain frontend must reconstruct its visual presentation and interaction grammar around Claude's UI/UX baseline, while preserving 100% of Brain's backend capabilities, relational data model, and local-first architecture.**

This forensic audit identifies every document encoding legacy UI assumptions, classifies each under a strict 5-category taxonomy, and establishes the exact supersession and reconciliation path to [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md) as the sole canonical frontend visual authority.

---

## 2. Corpus Scanned

The audit evaluated the **entire 442-document repository corpus** indexed in [`docs/artifact-manifest.yaml`](../../docs/artifact-manifest.yaml):
- `docs/design/**` (19 canonical & supporting design specifications)
- `docs/architecture/**` (Constitution, Invariants, ADRs, RFCs, Subsystems)
- `docs/reference/**` (Storage contracts, wire protocols, API specs)
- `docs/archive/**` (74 historical frontend parity reports, 6 historical ADRs, 35 sprint logs)
- `crates/**/README.md` & `apps/**/README.md` (24 crate/app architecture contracts)
- `docs/ux/**` & `docs/research/**` (Reverse-engineered Claude research baselines)
- Root governance, configuration, and CI workflows

---

## 3. Detection Methodology

Detection was conducted via multi-pass semantic and lexical analysis scanning for:
1. **Visual Language Tokens**: Pixel art, mascot avatars, "clawd", "Memory Core", synthwave, cyberpunk, electric violet, neon cyan, `#6C5CE7`, `#00CEC9`, heavy double borders (`═`, `║`).
2. **Information Architecture Patterns**: Three-pane dashboard, telemetry inspector, fixed activity panels, static status dashboards, "Connected: Yes | Ready: Yes" home cards.
3. **Interaction Model Primacy**: Slash-command-first workflow requirements, `/session` as mandatory entry point, modal-heavy wizards.
4. **Component Model Constructs**: Static uncollapsed `[REASONING]` boxes, heavy tool parameter tables, single-line prompt editors.
5. **Reference Graph Tracing**: Tracking documents that import, reference, or declare legacy design specifications as canonical.

---

## 4. Complete Classification of Affected Documents

The 442 repository documents are classified into 5 mutually exclusive categories:

```
═══════════════════════════════════════════════════════════════════════════════════════════════
                             DOCUMENT CLASSIFICATION BREAKDOWN
═══════════════════════════════════════════════════════════════════════════════════════════════

• TARGET-CONFLICTING:        4 documents (Direct visual/palette contradiction; must be superseded)
• TARGET-RECONCILABLE:       8 documents (Architecturally valid; references/links must be reconciled)
• TARGET-COMPATIBLE:        25 documents (Fully aligned with Claude Visual Contract; keep supporting)
• NON-UI / MUST-PRESERVE:  262 documents (Pure backend, storage, domain, or CI contracts; untouched)
• HISTORICAL / IMMUTABLE:  143 documents (Immutable historical records and sprint logs; preserved verbatim)
```

---

## 5. Detailed Forensic Findings for Conflicting & Reconcilable Documents

### A. TARGET-CONFLICTING Documents (4 Canonical Specifications — To Be Superseded)

| Path | Authority / Role / Lifecycle | Exact Legacy Assumptions Found | Severity | Target Conflict | Recommended Action | Proposed Replacement Authority |
| :--- | :--- | :--- | :---: | :--- | :--- | :--- |
| [`docs/design/LANDING_PAGE.md`](./LANDING_PAGE.md) | `canonical` / `contract` / `active` | 8-bit pixel-art mascot ("clawd" / Memory Core), "Think once. Remember forever." promotional banner, "Connected: Yes \| Ready: Yes" telemetry boxes, 3 fixed button pills. | 🔴 **CRITICAL** | Directly mandates obsolete dashboard home screen instead of Claude's scrollable greeting header. | **SUPERSEDE** | [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md) |
| [`docs/design/VISUAL_LANGUAGE_V2.md`](./VISUAL_LANGUAGE_V2.md) | `canonical` / `contract` / `active` | Cyberpunk palette, electric violet (`#6C5CE7`), neon cyan (`#00CEC9`), ASCII pixel borders, high-contrast gaming visual language. | 🔴 **CRITICAL** | Direct aesthetic contradiction to Claude's warm neutral/terracotta visual identity. | **SUPERSEDE** | [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md) |
| [`docs/design/THEME_TOKENS.md`](./THEME_TOKENS.md) | `canonical` / `contract` / `active` | Raw hex values for cyberpunk and violet themes (`#6C5CE7`, `#A29BFE`, `#00CEC9`). | 🔴 **CRITICAL** | Encodes obsolete color tokens that conflict with Claude warm terracotta tokens. | **SUPERSEDE** | [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md) |
| [`docs/design/THEMING.md`](./THEMING.md) | `canonical` / `contract` / `active` | Preset definitions for `brain_dark`, `cyberpunk`, and `nord` color schemes. | 🔴 **CRITICAL** | Mandates legacy themes with neon violet accents. | **SUPERSEDE** | [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md) |

---

### B. TARGET-RECONCILABLE Documents (8 Documents — To Be Reconciled)

| Path | Authority / Role / Lifecycle | Exact Legacy Assumptions Found | Severity | Reconciliation Scope | Recommended Action | Replacement Authority Reference |
| :--- | :--- | :--- | :---: | :--- | :--- | :--- |
| [`docs/design/TUI_DESIGN_SYSTEM.md`](./TUI_DESIGN_SYSTEM.md) | `canonical` / `contract` / `active` | Mentions 3-region grid and legacy status chips. | 🟡 **MODERATE** | Reconcile canonical references and layout hierarchy to point to `CLAUDE_VISUAL_CONTRACT.md`. | **PARTIALLY_RECONCILE** | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/COMPONENT_LIBRARY.md`](./COMPONENT_LIBRARY.md) | `canonical` / `contract` / `active` | Boxed card containers, static telemetry panels. | 🟡 **MODERATE** | Reconcile component catalog to define Claude message stream, thinking blocks, and tool cards. | **PARTIALLY_RECONCILE** | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/INFORMATION_ARCHITECTURE.md`](./INFORMATION_ARCHITECTURE.md) | `canonical` / `contract` / `active` | Multi-screen dashboard flow and fixed sidebar navigation. | 🟡 **MODERATE** | Reconcile screen hierarchy to two-region vertical stack + floating overlays. | **PARTIALLY_RECONCILE** | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/INTERACTION_MODEL.md`](./INTERACTION_MODEL.md) | `canonical` / `contract` / `active` | Command-first primacy assumptions. | 🟡 **MODERATE** | Reconcile prompt-first interaction model and floating slash autocomplete popup. | **PARTIALLY_RECONCILE** | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/architecture/STABLE_UI_INVARIANTS.md`](../architecture/STABLE_UI_INVARIANTS.md) | `canonical` / `governance` / `active` | Mentions legacy component names in invariant descriptions. | 🟡 **MODERATE** | Maintain architectural invariants; update component references to `CLAUDE_VISUAL_CONTRACT.md`. | **PARTIALLY_RECONCILE** | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/subsystems/tui.md`](../../docs/subsystems/tui.md) | `supporting` / `operational` / `active` | Subsystem guide referencing legacy design specs. | 🟡 **MODERATE** | Update canonical specification reference link to `CLAUDE_VISUAL_CONTRACT.md`. | **PARTIALLY_RECONCILE** | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`docs/design/README.md`](./README.md) | `supporting` / `governance` / `active` | Design sitemap index. | 🟡 **MODERATE** | Reorganize sitemap to establish `CLAUDE_VISUAL_CONTRACT.md` as primary design authority. | **PARTIALLY_RECONCILE** | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |
| [`crates/brain-tui/README.md`](../../crates/brain-tui/README.md) | `supporting` / `operational` / `active` | References `TUI_DESIGN_SYSTEM.md` as canonical reference. | 🟡 **MODERATE** | Update canonical specification link to `CLAUDE_VISUAL_CONTRACT.md`. | **PARTIALLY_RECONCILE** | `docs/design/CLAUDE_VISUAL_CONTRACT.md` |

---

### C. TARGET-COMPATIBLE Documents (25 Supporting & Capability Documents — Preserved)

Key documents that are fully compatible with the Claude visual baseline:
- [`docs/design/BRAIN_PRODUCT_CAPABILITIES_ROADMAP.md`](./BRAIN_PRODUCT_CAPABILITIES_ROADMAP.md): **Remains Canonical Product Roadmap** (governs backend capability inventory independently of visual presentation).
- [`docs/design/KEYBINDINGS.md`](./KEYBINDINGS.md): Keyboard navigation grammar (`Ctrl+K`, `Ctrl+O`, Vim bindings).
- [`docs/design/RESPONSIVE_LAYOUTS.md`](./RESPONSIVE_LAYOUTS.md): Terminal geometry and resize math.
- [`docs/design/MOTION.md`](./MOTION.md): 60fps refresh budgets and typewriter timing.
- [`docs/design/ACCESSIBILITY.md`](./ACCESSIBILITY.md): WCAG AA contrast and ANSI 16-color fallbacks.
- [`docs/design/PERFORMANCE_BUDGET.md`](./PERFORMANCE_BUDGET.md): Frame latency budgets (<66ms).
- [`docs/reference/cli-ux-comparison.md`](../reference/cli-ux-comparison.md): Comparative UX analysis across modern AI CLI tools.
- `docs/ux/*` & `docs/research/*`: Reverse-engineered Claude research baselines.

---

### D. HISTORICAL / IMMUTABLE Documents (143 Documents — Verbatim Preserved)

All historical sprint reports, forensic audits, visual reconciliation logs, and migration reports under:
- `docs/archive/frontend-parity/*` (74 files)
- `docs/archive/historical-adrs/*` (6 files)
- `docs/archive/migration/*` (3 files)
- `docs/archive/sprint-reports/*` (35 files)
- `crates/brain-services/tests/evaluation/*` (12 files)
- `packages/brain-frontend/*` (2 files)

**Historical Invariant**: These documents are historical evidence and are **preserved verbatim**. They are non-authoritative for current system design.

---

## 6. Canonical Authority Analysis: Eliminating Competing Authorities

A critical vulnerability identified in the pre-audit repository state was the presence of multiple conflicting documents claiming canonical authority over the UI:
- `docs/design/TUI_DESIGN_SYSTEM.md` claimed canonical layout authority.
- `docs/design/VISUAL_LANGUAGE_V2.md` claimed canonical aesthetic authority.
- `docs/design/LANDING_PAGE.md` claimed canonical home screen authority.
- `docs/design/THEME_TOKENS.md` claimed canonical color authority.

### The Unified Canonical Target
Upon approved execution of the migration plan:
```text
═══════════════════════════════════════════════════════════════════════════════════════════════
                                UNIFIED AUTHORITY HIERARCHY
═══════════════════════════════════════════════════════════════════════════════════════════════

   Sole Canonical Visual Authority:     docs/design/CLAUDE_VISUAL_CONTRACT.md
   Sole Capability Surface Mapping:     docs/design/BRAIN_CLAUDE_SURFACE_MAPPING.md
   Sole Canonical Product Roadmap:      docs/design/BRAIN_PRODUCT_CAPABILITIES_ROADMAP.md
   Supporting Architectural Details:    docs/design/KEYBINDINGS.md, RESPONSIVE_LAYOUTS.md, etc.
   Superseded Historical Records:       docs/design/LANDING_PAGE.md, VISUAL_LANGUAGE_V2.md, etc.
```

---

## 7. Dependency & Reference Impact Analysis

Traced inbound and outbound references across the repository:
1. **Inbound References to Conflicting Documents**:
   - `docs/design/README.md` references `LANDING_PAGE.md` and `VISUAL_LANGUAGE_V2.md`.
   - `crates/brain-tui/README.md` references `TUI_DESIGN_SYSTEM.md`.
   - `docs/subsystems/tui.md` references `TUI_DESIGN_SYSTEM.md`.
2. **Reconciliation Impact**:
   - All inbound links in active documents will be cleanly re-pointed to `docs/design/CLAUDE_VISUAL_CONTRACT.md`.
   - Historical documents referencing old design files will remain untouched, preserving archival provenance.

---

## 8. Risk Assessment & Safety Invariants

| Risk Dimension | Risk Level | Mitigation Invariant |
| :--- | :---: | :--- |
| **Backend Capability Loss** | **ZERO** | Brain backend services (`RetrievalService`, `SessionService`, `ConsolidationService`, SQLite store) are strictly decoupled from visual presentation. |
| **Inventing Fake Cloud Features** | **ZERO** | `BRAIN_CLAUDE_SURFACE_MAPPING.md` explicitly forbids cloud model selectors, effort sliders, and subscription UI. |
| **Authority Graph Cycles** | **ZERO** | All supersession pointers flow unidirectionally to `CLAUDE_VISUAL_CONTRACT.md` (0 cycles). |
| **Source Code Regressions** | **ZERO** | Source code is strictly untouched during this documentation audit. |

---

## 9. Recommended Migration Ordering

When user approval is granted, execution should proceed in the following disciplined order:

```text
Step 1: Manifest Registration & Supersession
        Update docs/artifact-manifest.yaml:
        - Register docs/design/CLAUDE_VISUAL_CONTRACT.md as canonical / contract / active
        - Register docs/design/BRAIN_CLAUDE_SURFACE_MAPPING.md as canonical / contract / active
        - Mark LANDING_PAGE, VISUAL_LANGUAGE_V2, THEMING, THEME_TOKENS as historical / contract / superseded

Step 2: Deprecation Notices in Superseded Documents
        Add standardized non-destructive deprecation headers to the 4 superseded files pointing
        to docs/design/CLAUDE_VISUAL_CONTRACT.md.

Step 3: Reconcile Supporting Design Documents & Crate READMEs
        Update canonical reference links in TUI_DESIGN_SYSTEM, COMPONENT_LIBRARY, INFORMATION_ARCHITECTURE,
        INTERACTION_MODEL, docs/design/README.md, and crates/brain-tui/README.md.

Step 4: Automated CI Validation Sweep
        Re-verify 100% manifest parity, acyclicity (Invariant 7), classification consistency (Invariant 13),
        and link validity.
```

---

## 10. Stop Condition & Audit Certification

In accordance with strict instructions, **no files have been deleted, no files moved, no source code modified, and no migration performed**.

This forensic audit establishes the complete evidence base and roadmap required to execute the UI documentation migration cleanly upon explicit user approval.
