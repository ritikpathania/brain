# Old Canonical Design Migration & Supersession Plan

**Document Status**: `PROPOSED GOVERNANCE MIGRATION PLAN`  
**Scope**: Transition of Obsolete Brain UI Specifications to the **Claude Visual Contract**  
**Governing Authority**: Dual-Pillar Model (Dual-Pillar Architecture & Invariant 13)

---

## 1. Executive Governance Strategy

To eliminate competing UI design authorities across the repository, all obsolete Brain visual documents are formally transitioned under the approved governance lifecycle:
1. **Establish Single Canonical Authority**: [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md) becomes the sole normative UI/UX authority for Brain frontend development.
2. **Establish Capability Surface Mapping**: [`docs/design/BRAIN_CLAUDE_SURFACE_MAPPING.md`](./BRAIN_CLAUDE_SURFACE_MAPPING.md) provides the binding mapping of Brain backend capabilities onto Claude visual surfaces.
3. **Supersede Obsolete Visual Specifications**: The 4 canonical documents that directly conflict with the Claude visual language are marked `lifecycle: superseded` with explicit `superseded_by` pointers to `docs/design/CLAUDE_VISUAL_CONTRACT.md`.
4. **Reconcile Supporting Specifications**: Update supporting design documents to reference the new visual contract while retaining their underlying technical constraints (e.g. keyboard grammar, motion budgets, accessibility rules).

```
═══════════════════════════════════════════════════════════════════════════════════════════════
                             CANONICAL UI AUTHORITY TRANSITION CHAIN
═══════════════════════════════════════════════════════════════════════════════════════════════

   [Obsolete Brain UI Specifications]
   (LANDING_PAGE, VISUAL_LANGUAGE_V2, THEMING, THEME_TOKENS)
                   │
                   ▼  (Marked lifecycle: superseded in manifest)
   ┌────────────────────────────────────────────────────────┐
   │       docs/design/CLAUDE_VISUAL_CONTRACT.md            │ ◄── Sole Canonical Visual Authority
   └─────────────────────────┬──────────────────────────────┘
                             │
                             ▼  (Governs capability projection)
   ┌────────────────────────────────────────────────────────┐
   │    docs/design/BRAIN_CLAUDE_SURFACE_MAPPING.md         │ ◄── Capability Surface Mapping
   └─────────────────────────┬──────────────────────────────┘
                             │
                             ▼  (Implements Ratatui rendering)
   ┌────────────────────────────────────────────────────────┐
   │         Frontend Implementation (crates/brain-tui)     │
   └────────────────────────────────────────────────────────┘
```

---

## 2. Document-by-Document Migration Matrix

| Document Path | Current Authority / Role / Lifecycle | Conflict with Claude Visual Direction | Target Authority / Role / Lifecycle | `superseded_by` Pointer | Migration Action |
| :--- | :--- | :--- | :--- | :--- | :--- |
| [`docs/design/LANDING_PAGE.md`](./LANDING_PAGE.md) | `canonical` / `contract` / `active` | Mandates pixel-art mascot, "Think once" banner, status boxes. | `historical` / `contract` / `superseded` | `docs/design/CLAUDE_VISUAL_CONTRACT.md` | Mark superseded; add deprecation notice pointing to `CLAUDE_VISUAL_CONTRACT.md`. |
| [`docs/design/VISUAL_LANGUAGE_V2.md`](./VISUAL_LANGUAGE_V2.md) | `canonical` / `contract` / `active` | Mandates electric purple / cyber cyan cyberpunk palette and pixel art. | `historical` / `contract` / `superseded` | `docs/design/CLAUDE_VISUAL_CONTRACT.md` | Mark superseded; add deprecation notice pointing to `CLAUDE_VISUAL_CONTRACT.md`. |
| [`docs/design/THEME_TOKENS.md`](./THEME_TOKENS.md) | `canonical` / `contract` / `active` | Encodes legacy raw hex values for cyberpunk and violet themes. | `historical` / `contract` / `superseded` | `docs/design/CLAUDE_VISUAL_CONTRACT.md` | Mark superseded; add deprecation notice pointing to `CLAUDE_VISUAL_CONTRACT.md`. |
| [`docs/design/THEMING.md`](./THEMING.md) | `canonical` / `contract` / `active` | Defines obsolete `brain_dark`, `cyberpunk`, and `nord` theme presets. | `historical` / `contract` / `superseded` | `docs/design/CLAUDE_VISUAL_CONTRACT.md` | Mark superseded; add deprecation notice pointing to `CLAUDE_VISUAL_CONTRACT.md`. |
| [`docs/design/TUI_DESIGN_SYSTEM.md`](./TUI_DESIGN_SYSTEM.md) | `canonical` / `contract` / `active` | Describes 3-region grid with legacy status footer chips. | `supporting` / `contract` / `active` | N/A (Aligns with `CLAUDE_VISUAL_CONTRACT.md`) | Reconcile canonical references to point to `CLAUDE_VISUAL_CONTRACT.md`. |
| [`docs/design/COMPONENT_LIBRARY.md`](./COMPONENT_LIBRARY.md) | `canonical` / `contract` / `active` | References old card and panel box widgets. | `supporting` / `contract` / `active` | N/A (Aligns with `CLAUDE_VISUAL_CONTRACT.md`) | Reconcile component definitions to Claude message, thinking, and tool cards. |
| [`docs/design/INFORMATION_ARCHITECTURE.md`](./INFORMATION_ARCHITECTURE.md) | `canonical` / `contract` / `active` | Outlines old multi-screen dashboard hierarchy. | `supporting` / `contract` / `active` | N/A (Aligns with `CLAUDE_VISUAL_CONTRACT.md`) | Reconcile screen hierarchy to conversation stream + floating overlays. |
| [`docs/design/INTERACTION_MODEL.md`](./INTERACTION_MODEL.md) | `canonical` / `contract` / `active` | Focus flow and modal dialog layering. | `supporting` / `contract` / `active` | N/A (Aligns with `CLAUDE_VISUAL_CONTRACT.md`) | Reconcile prompt-first interaction and floating slash autocomplete. |
| [`docs/design/BRAIN_PRODUCT_CAPABILITIES_ROADMAP.md`](./BRAIN_PRODUCT_CAPABILITIES_ROADMAP.md) | `canonical` / `governance` / `active` | None (pure backend/product capability matrix). | `canonical` / `governance` / `active` | N/A | **RETAIN AS CANONICAL PRODUCT ROADMAP**. |
| [`docs/design/README.md`](./README.md) | `supporting` / `governance` / `active` | Directory index sitemap. | `supporting` / `governance` / `active` | N/A | Update directory index to position `CLAUDE_VISUAL_CONTRACT.md` as primary authority. |
| [`docs/architecture/STABLE_UI_INVARIANTS.md`](../architecture/STABLE_UI_INVARIANTS.md) | `canonical` / `governance` / `active` | Architecture state invariants remain valid. | `canonical` / `governance` / `active` | N/A | Update canonical references to `CLAUDE_VISUAL_CONTRACT.md`. |
| [`docs/subsystems/tui.md`](../../docs/subsystems/tui.md) | `supporting` / `operational` / `active` | Subsystem overview guide. | `supporting` / `operational` / `active` | N/A | Update canonical reference link to `CLAUDE_VISUAL_CONTRACT.md`. |
| [`crates/brain-tui/README.md`](../../crates/brain-tui/README.md) | `supporting` / `operational` / `active` | References `TUI_DESIGN_SYSTEM.md`. | `supporting` / `operational` / `active` | N/A | Update canonical reference link to `CLAUDE_VISUAL_CONTRACT.md`. |

---

## 3. Governance Invariants & Acyclicity Guarantee

1. **Acyclicity (Invariant 7)**: All 4 new supersession edges (`LANDING_PAGE`, `VISUAL_LANGUAGE_V2`, `THEME_TOKENS`, `THEMING`) point strictly unidirectionally to `docs/design/CLAUDE_VISUAL_CONTRACT.md`. Zero back-references or cycles exist.
2. **Classification Consistency (Invariant 13)**:
   - `docs/design/CLAUDE_VISUAL_CONTRACT.md`: `authority: canonical`, `role: contract`, `lifecycle: active`, `claim_status: current`.
   - `docs/design/BRAIN_CLAUDE_SURFACE_MAPPING.md`: `authority: canonical`, `role: contract`, `lifecycle: active`, `claim_status: current`.
   - Superseded documents: `authority: historical`, `role: contract`, `lifecycle: superseded`, `claim_status: superseded`.
3. **No Intermediate Rewrite**: In accordance with governance constraints, this document represents the migration plan. Actual document updates will occur only upon explicit user approval.
