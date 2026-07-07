# Global Search Omnibox & Pluggable Providers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a unified Global Search omnibox overlay (`Ctrl+P`) with immediate local providers (commands, sessions, active message cache) and debounced remote daemon queries, managed by an event aggregator and pure ranking engine.

**Architecture:** A decoupled event-driven pipeline where the controller manages lifetimes and cancellations, providers stream events, the aggregator handles incremental updates, and the ranking engine provides stable, pure scoring.

**Tech Stack:** Rust, Ratatui, Crossterm.

## Global Constraints
- Do not expose daemon database handles directly to the TUI client; all daemon communication must use UDS IPC streaming protocols.
- Avoid introducing external scoring/fuzzy-match crates.
- Maintain zero external subsystem dependencies on `brain-domain`.
- All public types, functions, and modules must be documented.

---

### Task 1: Core Search Types

**Files:**
- Create: `crates/brain-tui/src/ui/search/mod.rs`
- Create: `crates/brain-tui/src/ui/search/types.rs`
- Create: `crates/brain-tui/tests/search_types_tests.rs`

**Interfaces:**
- Consumes: None
- Produces: `ProviderId`, `SearchGeneration`, `SearchQuery`, `SearchFailure`, `SearchResultKind`, `SearchResultAction`, `SearchResult`, `SearchEvent`, `SearchEventSink`, `ProviderStatus`, `SearchViewState`

- [ ] **Step 1: Write the failing type construct test**

Create `crates/brain-tui/tests/search_types_tests.rs`:
```rust
use brain_tui::ui::search::types::{ProviderId, PROVIDER_COMMANDS, SearchGeneration, SearchQuery};

#[test]
fn test_type_construction() {
    let gen = SearchGeneration(1);
    let query = SearchQuery {
        generation: gen,
        text: "hello".to_string(),
    };
    assert_eq!(query.text, "hello");
    assert_eq!(PROVIDER_COMMANDS.as_str(), "commands");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test search_types_tests`
Expected: FAIL (modules and types do not exist)

- [ ] **Step 3: Core Types Scaffolding**

Create `crates/brain-tui/src/ui/search/types.rs` carrying definitions for all core data structures, values, enums, and read-only view state accessors, enforcing the crate-private constructor for `ProviderId`.
Create `crates/brain-tui/src/ui/search/mod.rs` declaring and re-exporting the `types` module.
Declare the `search` module in `crates/brain-tui/src/ui/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test search_types_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/search/mod.rs crates/brain-tui/src/ui/search/types.rs crates/brain-tui/tests/search_types_tests.rs && git commit -m "feat: add core global search pipeline types"`

---

### Task 2: Pure Ranking Engine

**Files:**
- Create: `crates/brain-tui/src/ui/search/ranking.rs`
- Create: `crates/brain-tui/tests/search_ranking_tests.rs`

**Interfaces:**
- Consumes: `SearchResult`, `SearchResultKind`
- Produces: `RankingEngine::rank(&self, query: &str, results: impl IntoIterator<Item = SearchResult>) -> Vec<SearchResult>`

- [ ] **Step 1: Write the deterministic sorting tests**

Create `crates/brain-tui/tests/search_ranking_tests.rs`:
```rust
use brain_tui::ui::search::types::{SearchResult, SearchResultKind, SearchResultAction};
use brain_tui::ui::search::ranking::RankingEngine;

#[test]
fn test_ranking_determinism_and_stable_sort() {
    let engine = RankingEngine;
    let results = vec![
        SearchResult {
            title: "B Session".to_string(),
            subtitle: "".to_string(),
            kind: SearchResultKind::Session,
            provider_score: 5,
            action: SearchResultAction::InvokeCommand(brain_tui::ui::command::CommandId("test".to_string())),
        },
        SearchResult {
            title: "A Session".to_string(),
            subtitle: "".to_string(),
            kind: SearchResultKind::Session,
            provider_score: 5,
            action: SearchResultAction::InvokeCommand(brain_tui::ui::command::CommandId("test".to_string())),
        },
    ];
    
    let ranked = engine.rank("session", results);
    assert_eq!(ranked[0].title, "A Session"); // Alphabetical fallback when scores match
    assert_eq!(ranked[1].title, "B Session");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test search_ranking_tests`
Expected: FAIL

- [ ] **Step 3: Ranking Engine Implementation**

Create `crates/brain-tui/src/ui/search/ranking.rs` defining the pure, side-effect free `RankingEngine`. Implement the ordered, additive scoring pipeline (ProviderScore -> PrefixBoost -> WordBoundaryBoost -> KindBoost) and sort stably descending by score, falling back to title alphabetically.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test search_ranking_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/search/ranking.rs crates/brain-tui/tests/search_ranking_tests.rs && git commit -m "feat: implement pure search result ranking engine"`

---

### Task 3: Search Aggregator

**Files:**
- Create: `crates/brain-tui/src/ui/search/aggregator.rs`
- Create: `crates/brain-tui/tests/search_aggregator_tests.rs`

**Interfaces:**
- Consumes: `SearchEvent`, `SearchViewState`, `RankingEngine`
- Produces: `SearchAggregator::handle_event(&mut self, event: SearchEvent)`, `SearchAggregator::view_state(&self) -> SearchViewState`, and `SearchAggregator::is_complete(&self) -> bool`

- [ ] **Step 1: Write the generation filtering test**

Create `crates/brain-tui/tests/search_aggregator_tests.rs`:
- Test that events from older generations are discarded.
- Test that events from the current generation update statuses and collected results, and trigger incremental view state updates.
- Test that `is_complete()` returns true only when all registered providers are `Completed` or `Failed`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test search_aggregator_tests`
Expected: FAIL

- [ ] **Step 3: Aggregator Implementation**

Create `crates/brain-tui/src/ui/search/aggregator.rs` defining `SearchAggregator` and `ProviderStatus`. Implement event handling and generation filtering.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test search_aggregator_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/search/aggregator.rs crates/brain-tui/tests/search_aggregator_tests.rs && git commit -m "feat: implement search aggregator and state collector"`

---

### Task 4: Immediate Providers

**Files:**
- Create: `crates/brain-tui/src/ui/search/providers.rs`
- Create: `crates/brain-tui/tests/immediate_providers_tests.rs`

**Interfaces:**
- Consumes: `SearchProvider`, `SearchQuery`, `SearchEventSink`
- Produces: `CommandsProvider`, `SessionsProvider`, `LocalMessagesProvider`

- [ ] **Step 1: Write failing immediate providers tests**

Create `crates/brain-tui/tests/immediate_providers_tests.rs`:
- Construct a mock `SearchEventSink` to collect emitted `SearchEvent`s.
- Assert `CommandsProvider` synchronous matches.
- Assert `SessionsProvider` matches.
- Assert `LocalMessagesProvider` matches on loaded messages in state.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test immediate_providers_tests`
Expected: FAIL

- [ ] **Step 3: Providers Implementation**

Create `crates/brain-tui/src/ui/search/providers.rs`. Implement `SearchProvider` for the three synchronous sources, querying the registry and loaded state.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test immediate_providers_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/search/providers.rs crates/brain-tui/tests/immediate_providers_tests.rs && git commit -m "feat: add immediate search providers for commands, sessions, and cache"`

---

### Task 5: Search Controller & Orchestrator

**Files:**
- Create: `crates/brain-tui/src/ui/search/controller.rs`
- Create: `crates/brain-tui/tests/search_controller_tests.rs`

**Interfaces:**
- Consumes: `SearchQuery`, `SearchSession`, `SearchProvider`, `SearchEventSink`
- Produces: `SearchController::new()`, `SearchController::search(&mut self, text: String)`, and `SearchController::cancel(&mut self)`

- [ ] **Step 1: Write cancellation and debouncing tests**

Create `crates/brain-tui/tests/search_controller_tests.rs`:
- Assert that starting a new search generation cancels in-flight providers of the previous generation.
- Assert that query debouncing occurs for remote sources.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test search_controller_tests`
Expected: FAIL

- [ ] **Step 3: Controller Implementation**

Create `crates/brain-tui/src/ui/search/controller.rs` implementing `SearchController` and lifecycle orchestration. Ensure provider registration is immutable.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test search_controller_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/search/controller.rs crates/brain-tui/tests/search_controller_tests.rs && git commit -m "feat: implement SearchController with generation lifecycle and cancellation"`

---

### Task 6: Remote Message Provider & Daemon IPC

**Files:**
- Modify: `crates/brain-tui/src/ui/search/providers.rs`
- Create: `crates/brain-tui/tests/remote_provider_tests.rs`

**Interfaces:**
- Consumes: `SearchProvider`, UDS daemon protocols
- Produces: `RemoteMessagesProvider`

- [ ] **Step 1: Write failing remote message provider tests**

Create `crates/brain-tui/tests/remote_provider_tests.rs` testing that `RemoteMessagesProvider` triggers IPC calls to the backend, handles result streams, and discards stale generations.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test remote_provider_tests`
Expected: FAIL

- [ ] **Step 3: Remote Provider Implementation**

Extend `crates/brain-tui/src/ui/search/providers.rs` to include `RemoteMessagesProvider`. Integrate UDS search query commands.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test remote_provider_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/search/providers.rs crates/brain-tui/tests/remote_provider_tests.rs && git commit -m "feat: implement RemoteMessagesProvider querying daemon SQLite history"`

---

### Task 7: Omnibox Overlay Integration

**Files:**
- Modify: `crates/brain-tui/src/ui/command/palette.rs`
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Create: `crates/brain-tui/tests/omnibox_integration_tests.rs`

**Interfaces:**
- Consumes: `SearchController`, `SearchAggregator`, `SearchViewState`
- Produces: Omnibox search overlay rendering, keyboard event routing, and result execution dispatcher mappings.

- [ ] **Step 1: Write visual search layout tests**

Create `crates/brain-tui/tests/omnibox_integration_tests.rs` asserting search result display lists and keyboard inputs (`Up`/`Down` cursor, `Enter` executes action).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test omnibox_integration_tests`
Expected: FAIL

- [ ] **Step 3: Omnibox Overlay Implementation**

Refactor `crates/brain-tui/src/ui/command/palette.rs` to integrate the `SearchController` and `SearchAggregator`. Update rendering in `renderer.rs` to draw the omnibox list under the unified layout modal.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test omnibox_integration_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/command/palette.rs crates/brain-tui/src/ui/renderer.rs crates/brain-tui/tests/omnibox_integration_tests.rs && git commit -m "feat: integrate unified omnibox search overlay and event router"`
