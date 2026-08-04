# ADR 0001: TUI Search & Presentation Pipeline Layering

## Context

Search results and memory retrieval in the terminal UI previously mixed chat messages, raw database entities, presentation styling, and retrieval filtering logic across widget boundaries. This caused several issues:

- Search results were mapped to `Message` DTOs, forcing unrelated domain concepts together.
- Presentation logic (e.g. interpreting confidence scores or generating fallback labels for missing titles) was calculated inside renderer widgets.
- Interaction state (selection, scrolling, focus) drifted from list data during refreshes.
- Silent error swallows obscured network/daemon failures from end users.

## Decision

We establish a strict, unidirectional 5-stage pipeline for search retrieval, presentation projection, grouping, and rendering:

```text
Backend / Daemon Retrieval
           │
           ▼ (SearchCandidate)
     Aggregation
           │
           ▼ (SearchResult)
  Presentation Projection
           │
           ▼ (MemoryResultViewModel)
        Grouping
           │
           ▼ (MemoryResultGroup)
        Rendering
```

### Stage Responsibilities & Invariants

1. **Retrieval Boundary (Daemon & `SearchProjector`)**:
   - Owns scoring, ranking algorithms, score thresholds, confidence classification (`Confidence::High`, `Medium`, `Low`), and candidate retrieval.
   - Outputs domain-neutral `SearchCandidate` types.
   - Does NOT depend on UI layout or ratatui primitives.

2. **Aggregation Boundary (`SearchAggregator`)**:
   - Collects results across immediate (commands, sessions, local messages) and async (remote knowledge graph) providers.
   - Performs borrowing-based deduplication by `entity_id`.
   - Distinguishes transport failures (`SearchFailure::BackendUnavailable`) from empty search results (`Ok(empty)`).
   - Outputs `SearchResult`.

3. **Presentation Projection Boundary (`MemoryResultViewModel`)**:
   - **Immutable value objects**: ViewModels contain NO retrieval logic, ranking algorithms, transport error state, or mutable UI interaction state (`selected`, `expanded`, `focused`, `scroll_offset`).
   - Resolves all `Option` placeholders at this boundary (e.g., `None` title → `"(untitled memory)"`).
   - Encodes detail view availability in a strongly-typed enum (`DetailAvailability::Available(EntityId)` or `None`) to prevent invalid states.

4. **Grouping Boundary (`MemoryGroupingEngine`)**:
   - Deterministic, stable grouping by confidence tier (`High match`, `Good match`, `Partial match`).
   - Pure presentation container logic. Does NOT re-rank, re-filter, or perform network requests.

5. **Rendering Boundary (`Renderer` & Widgets)**:
   - Stateless drawing over presentation groups and ViewModels.
   - Derives `ratatui::widgets::ListState` and `Block` styles dynamically at frame time.
   - Consumes styles via `theme.style(ThemeToken::...)` to enforce WCAG AA accessibility contrast.

### Navigation State Invariant

All scroll offsets and list selections are owned by dedicated state models (e.g., `SessionNavigator`), tracking identity via `SessionId` rather than list indices.

## Verification & End-to-End Strategy

End-to-end tests are structured by **behavioral capability** rather than individual widget units:

```text
tests/e2e/
├── search_flow.rs       # Concept search, placeholder resolution, confidence badges
├── command_palette.rs   # /slash navigation, Escape dismissal, parameter collection
├── navigation.rs        # Session list scrolling, identity preservation across updates
├── themes.rs            # 4-theme switching, WCAG AA contrast resolution
└── failure_modes.rs     # Backend unavailability, reconnection & recovery state machine
```

### Critical End-to-End Invariants Tested

1. **Presentation Boundary Regression (Black-Box Assertions)**:
   - Given a daemon response with missing title, missing summary, valid entity ID, and score.
   - Asserts observable UI behavior: no raw UUID strings or unresolved `Option` values appear in rendered output, default placeholder text (`"(untitled memory)"`) appears in place of missing fields, and detail navigation resolves to the correct entity ID.
   - Does NOT assert internal method calls, keeping tests resilient against internal refactoring.

2. **Ordering Stability Across State Operations & Repaints**:
   - Given a set of search results displayed in confidence groups (`High`, `Medium`, `Low`).
   - Triggering UI state changes (theme switches, sidebar navigation, screen resize, redrawing).
   - Asserts that group ordering and intra-group item ordering remain 100% identical across repaints; state mutations update presentation styles without triggering unexpected re-ranking or re-grouping.

3. **Daemon Recovery State Machine**:
   - Given a daemon transport failure (`BackendUnavailable`), followed by daemon reconnection and a successful query.
   - Asserts that the failure banner clears automatically, stale error states reset, and search results render without requiring an application restart.


## Consequences

- **Testability**: Every stage is unit-tested in isolation without mocking ratatui frames or network sockets, while end-to-end behavioral suites validate multi-layer user flows.
- **Extensibility**: Adding new search providers or presentation badges requires extending only the relevant pipeline stage without touching render loops.
- **Accessibility**: Theme contrast ratio checks (WCAG AA >= 4.5:1) are verified via automated tests.

