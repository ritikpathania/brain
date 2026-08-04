# ADR-027: Retrieval UI Projection Pipeline

## Status

Accepted

## Context

The Brain Terminal User Interface (`brain-tui`) requires a continuous, real-time retrieval surface that transforms user keyboard input into ranked, grouped, and interactive knowledge projections.

To prevent presentation logic from leaking into domain/retrieval layers or accumulating inside Ratatui widget render loops, the system must enforce strict stage separation along the retrieval pipeline.

## Decision

We establish a unidirectional 9-stage Retrieval UI Projection Pipeline:

```text
Keyboard Input / Prompt
          │
          ▼
1. Input / Controller        (Debounce 150ms + CancellationToken)
          │
          ▼
2. Search Providers          (Local, Remote, Candidates)
          │
          ▼
3. Search Aggregator         (Generation tracking & event fan-in)
          │
          ▼
4. Ranking Engine            (Score thresholding & additive boosts)
          │
          ▼
5. Grouping Engine           (Confidence tier partitioning: High / Med / Low)
          │
          ▼
6. Projection Layer          (DTO to UI domain mapping)
          │
          ▼
7. ViewModels                (Immutable display data models)
          │
          ▼
8. Widgets                   (Stateless layout & buffer rendering)
          │
          ▼
9. Renderer                  (Frame draw & terminal output)
```

### Stage Boundary Invariants

1. **Input / Controller**: Handles debounce timers and cancellation signals. Produces `SearchQuery` with monotonically increasing `SearchGeneration`. **Only the Input/Controller owns `CancellationToken`s**; downstream stages consume cancellation tokens but never create or manage them.
2. **Search Providers**: Pure data access strategies. Responds to `SearchQuery` and emits `SearchEvent` frames to `SearchEventSink`.
3. **Search Aggregator**: Tracks current generation ID. Discards events from stale generations.
4. **Ranking Engine**: Computes normalized match scores (`0` to `100`). Applies score cutoff thresholds.
5. **Grouping Engine**: Partitions ranked candidates into confidence tiers (`High`, `Medium`, `Low`). Preserves relative ranking within tiers.
6. **Projection Layer**: **Sole owner of presentation formatting** (score text, confidence badges, timestamps, string truncation, DTO-to-ViewModel transformations). Widgets and renderers **MUST NOT** perform string formatting or domain interpretation.
7. **ViewModels**: Immutable value objects containing only pre-formatted display strings and layout flags. **MUST NOT** hold mutable widget interaction state (e.g. selection indices, scroll offsets).
8. **Widgets**: Pure stateless functions consuming ViewModels and drawing into `ratatui::buffer::Buffer`.
9. **Renderer**: Orchestrates screen layout rects and invokes widget draw functions.

## UI State Machine Lifecycle

The pipeline operates as a deterministic finite state machine with explicit allowed transitions:

```text
Idle
  │
  ▼
Debouncing (150ms)
  │
  ▼
Searching
 ├──────────────► Empty
 ├──────────────► Results
 └──────────────► Error

Results   ──► Debouncing  ──► Idle
Empty     ──► Debouncing  ──► Idle
Error     ──► Debouncing  ──► Idle
```

State transitions are driven by input events and aggregator events; invalid state transitions are rejected by the controller reducer.

## Milestone 1 Acceptance Contract

### Supported User Interactions
- Typing in search input box or `/` command prompt.
- Rapid typing without UI blocking or frame drops.
- Pressing `Esc` to cancel search and clear active query.

### Supported UI States
- `Idle`: Default browsing view.
- `Debouncing`: Character entered, awaiting 150ms trigger window.
- `Searching`: Async providers active; progress/loading state rendered.
- `Results`: Grouped confidence tiers displayed.
- `Empty`: Zero results matched; helpful diagnostic guidance displayed.
- `Error`: Provider failure occurred; recoverable error banner displayed.

### M1 Success Criteria
- Typing never blocks frame rendering.
- Fast typing never displays stale search generation results.
- Search cancellation immediately halts pending provider execution.
- Empty queries cleanly restore the default browsing view.
- No-result searches render friendly guidance.
- Failed searches display actionable error messages.
- Loading indicators render within one frame of state transition.
- Repeated searches for identical input produce deterministic result ordering.

## Consequences

- **Positive**: Strict separation prevents widget logic duplication and eliminates selection drift bugs.
- **Positive**: Clear boundary isolation simplifies unit, integration, and snapshot testing.
- **Negative**: Requires explicit DTO-to-ViewModel mapping code across pipeline stages.
