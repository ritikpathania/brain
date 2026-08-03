# Architectural Invariants: Brain Framework & TUI

This document canonicalizes the architectural invariants and design principles governing the **Brain** system, Daemon IPC, and TUI Client.

---

## 1. Domain-Driven Design (DDD) Layering & Zero External Dependencies

### Rule
`brain-domain` sits at the very bottom of the workspace dependency tree. It MUST NOT depend on async runtimes (`tokio`), database drivers (`rusqlite`, `sqlx`), FFI modules, or logging frameworks.

### Layering Flow
```text
Domain Services  ──>  Domain Models  ──>  View Models  ──>  Interaction  ──>  Widgets  ──>  Renderer  ──>  Theme
```

### Rationale
Protects business rules and memory graph invariants from infrastructure churn.

---

## 2. Transport Command Parsing Isolation

### Rule
Transport implementations (`UdsClient`) MUST NOT inspect user command syntax or parse slash commands (e.g., checking `input.starts_with("/search")`).

### Rationale
Decouples IPC transport implementations from user-facing UI syntax. Transport layers serialize typed requests only (`RequestKind::Search`, `RequestKind::Query`). Command parsing belongs exclusively to the parser layer (`LocalSlashParser`).

---

## 3. Strongly-Typed, Symmetric IPC Protocol

### Rule
All IPC communication between Client and Daemon must use versioned, strongly-typed tagged request and response envelopes (`RequestKind`, `ResponseKind`).

### Rationale
Prevents stringly-typed protocol erosion (`action = "search"`) and ensures compile-time exhaustiveness across client and daemon codebases.

---

## 4. Presentation View Model Separation

### Rule
View models (such as `SearchResultsViewModel`) MUST live under `ui/view_models/` and act as **immutable projections** from domain data (`SearchResultItem`). View models MUST NOT perform data fetching, searching, or ranking logic.

### Rationale
Preserves the unidirectional data flow (`Domain` -> `ViewModel` -> `Widget` -> `Renderer`) and isolates UI state from storage logic.

---

## 5. Single Active Selection Focus

### Rule
Only one interactive collection (`CommandCompletion`, `SearchResultsViewModel`, `SessionPicker`) may own keyboard selection focus at a time.

### Rationale
Prevents ambiguous keyboard handling, double-triggers, and focus collisions.

---

## 6. One Command = One Deterministic Capability

### Rule
Every user-facing slash command MUST map to exactly one deterministic capability or service (`/help` -> `HelpService`, `/search` -> `SearchService`, `/session` -> `SessionService`).

### Rationale
Keeps slash commands composable, predictable, and simple to test.

---

## 7. Capability-Oriented Interfaces

### Rule
Public service and client APIs expose domain capabilities rather than implementation or storage details (`ExecutionClient::search(...)`, `ExecutionClient::inspect_entity(...)` instead of `inspect_node(...)` or `query_sql(...)`).

### Rationale
Keeps presentation independent of persistence and graph representation, allowing backend storage engines to evolve without breaking client contracts.

---

## 8. Domain Before Presentation

### Rule
Domain objects MUST NEVER contain presentation concerns. Presentation artifacts (highlight ranges, formatting, ANSI colors, UI titles) belong exclusively in ViewModel or Renderer layers.

### Rationale
Preserves clean separation between business logic and UI while allowing multiple frontends (TUI, CLI, SDK, Web) to consume the same domain model.
