# ADR-028: Knowledge Exploration UI Architecture

## Status

Accepted

## Context

Following the completion of search retrieval orchestration (ADR-027) and result presentation (Milestone 2), the Brain Terminal User Interface (`brain-tui`) requires a dedicated exploration surface for understanding knowledge graph entities, match explainability, and event provenance.

To prevent exploration state from leaking into search retrieval or widgets, the system must enforce strict separation between selection navigation ("Where am I?") and loaded session context ("What have I loaded?").

## Decision

We establish the Knowledge Exploration UI Architecture:

```text
Search Result Selection
          │
          ▼
1. ExplorationSession         ("What have I loaded?" — entity DTOs & section states)
          │
          ▼
2. NavigationState<Id>        ("Where am I?" — identity selection, scroll, history stack)
          │
          ├──► List Projection
          └──► Graph Projection  (Shared single source of selection state)
```

### Exploration Invariants

1. **Selection Identity Invariant**: If search results refresh while an inspector or exploration session is active, the currently selected entity **MUST** remain loaded if it still exists in the updated collection.
2. **History Semantics Invariant**: The history stack (`history_stack: Vec<Id>`) **MUST ONLY** record entity navigation transitions (`Entity A` → `Entity B`). Local UI state changes (expanding section accordions) **MUST NOT** push history frames.
3. **Graph / List Synchronization Invariant**: List view and Graph view **MUST NOT** maintain independent selection models. Both views are pure projections of the same `NavigationState`.
4. **Explainability Ownership Invariant**: The explainability widget **MUST** render score breakdowns and confidence badges directly from pre-computed ViewModels. Renderers **MUST NOT** recompute ranking or query services.
5. **Provenance Ownership Invariant**: Provenance timelines **MUST** be pure projections of event logs. Provenance widgets **MUST NOT** execute database queries or perform network requests.

## Milestone 3 Acceptance Criteria

| User Interaction | Expected Observable Outcome |
|---|---|
| Open Search Result | Inspector panel opens with entity DTO details |
| Expand Provenance | Event origin timeline and timestamps displayed |
| View Explainability | Additive score breakdown and match reasons rendered |
| Traverse Relationship | Connected entity loads, pushing previous entity to history stack |
| Switch Graph / List | Active entity selection remains synchronized |
| Press Back (`Esc` / `Backspace`) | Previous entity in history stack restored |
| Refresh Search Stream | Active inspector selection remains stable |
| Resize Terminal Window | Navigation scroll offset and exploration state preserved |

## Consequences

- **Positive**: Complete separation between retrieval, presentation, and exploration prevents logic duplication and keeps widgets stateless.
- **Positive**: Shared `NavigationState` guarantees 100% synchronization between ASCII Graph and List presentation modes.
- **Negative**: Requires explicit DTO-to-ViewModel projection for explainability and provenance models.
