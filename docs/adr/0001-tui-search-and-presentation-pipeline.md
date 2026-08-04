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

## Consequences

- **Testability**: Every stage is unit-tested in isolation without mocking ratatui frames or network sockets.
- **Extensibility**: Adding new search providers or presentation badges requires extending only the relevant pipeline stage without touching render loops.
- **Accessibility**: Theme contrast ratio checks (WCAG AA >= 4.5:1) are verified via automated tests.
