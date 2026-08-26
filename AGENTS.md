<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool | Use when |
|------|----------|
| `detect_changes` | Reviewing code changes — gives risk-scored analysis |
| `get_review_context` | Need source snippets for review — token-efficient |
| `get_impact_radius` | Understanding blast radius of a change |
| `get_affected_flows` | Finding which execution paths are impacted |
| `query_graph` | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes` | Finding functions/classes by name or keyword |
| `get_architecture_overview` | Understanding high-level codebase structure |
| `refactor_tool` | Planning renames, finding dead code |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.

## CLI TUI Design System

When writing or modifying Terminal User Interface (TUI) components in the `packages/brain-shell/` directory, always adhere to the design system specified in `packages/brain-shell/src/state/themeStore.ts`, `themeContext.tsx`, and `src/contracts/theme.ts`:

1. **Use Theme Tokens Everywhere**:
   - Never use hardcoded raw RGB, hex, or plain ANSI colors directly in layout components. Use semantic colors defined in `ThemeTokens` (`primary`, `secondary`, `dim`, `accent`, `error`, `warning`, `success`, `info`, `border`, `bg`, etc.).

2. **Precomputed Theme Maps**:
   - Access theme properties via the active React theme context (`useTheme()`) to ensure live updates without requiring process restarts.

3. **Standard Layout and Primitives**:
   - Favor themed layout containers (`ModalFrame`, `PromptInput`, `PaletteView`) over raw text or box primitives. This ensures semantic style encapsulation.
   - Design layouts to dynamically resize (`SIGWINCH`) using flexbox properties (`flexGrow`, `flexShrink`) and percentage widths instead of hardcoded column widths.
   - Handle compact terminal widths (< 80 columns) gracefully by simplifying or wrapping headers.
4. **Panel Borders**:
   - Standard interactive panels, popups, and transient modals use rounded borders (`╭`, `─`, `╮`, `│`, `╯`, `╰`) and semantic header/footer dividers. ASCII fallbacks (`+`, `-`, `|`) are preserved for compatibility.

## CLI TUI Testing & Verification

When modifying or adding components to the TUI client:

1. **Verify Using Integration Tests**: Run `bun test` in `packages/brain-shell` to check state projections, cell geometry, and keyboard interaction invariants.
2. **Run Performance Profiling**: Measure frame draw latency and memory footprint to check for regressions.

## UDS Streaming Protocol & Reactive State Projection

When working with queries or communication between the Rust Daemon and the TUI Client:

1. **Monotonic Tagged Stream Events**:
   - Communication uses tagged chunk types (`token`, `thinking`, `tool_use`, `tool_result`, `permission_request`, `error`, `finished`).
   - Sequence numbers must increment monotonically within a stream.

2. **Reactive State Projection**:
   - The TUI receives incoming stream chunks via `SessionController.handleChunk`, updating immutable reactive snapshots and transcript rows with atomic freeze on turn settlement.


## Domain-Driven Design (DDD) Invariants

When modifying or adding models, specifications, or operations to the `crates/brain-domain/` directory:

1. **Zero External Subsystem Dependencies**:
   - `brain-domain` must stay at the bottom of the dependency tree. It cannot depend on any async runtimes, logger setups, database engines, or FFI modules.

2. **Invariants Protected by Entities and Aggregates**:
   - Encapsulate validation rules and business logic directly within entities (e.g., `Edge::strengthen`, `Conversation::archive`) and aggregate roots (e.g., `KnowledgeGraph`, `Session`) rather than orchestrating workflows inside application services.

3. **Pure Domain Events**:
   - State-changing mutations on domain models must return pure, side-effect-free events (e.g. `RelationshipStrengthened`, `MemoryMerged`, `ConversationArchived`) instead of calling event publishers directly. Let calling services intercept and publish them.

4. **Pure Evaluation Specifications**:
   - Specifications (`Specification` implementations) must only handle in-memory domain validations (e.g., checking if a node is pinned or if an edge is expired) and must not be used for database query building or indexing.

### Architectural Scaling Guidelines

As the relational memory engine grows, maintain the following design disciplines:

1. **Strict Event Separation**:
   - **DomainEvent**: Captures immutable business facts.
   - **EventEnvelope**: Serves as the metadata/transport wrapper.
   - **StreamEvent**: Expresses TUI/UI/streaming concerns.
   - *Translation Boundary*: The service layer must act as the sole translation boundary between these layers.

2. **Collection-Free Aggregates**:
   - Do not let aggregates (like `KnowledgeGraph` or `Session`) grow indefinitely. For massive graphs, transition to referencing entity identifiers/repositories rather than materializing full collection records in memory.

3. **Internal Event Recording Pattern**:
   - For complex, multi-event mutations, shift from returning single events to internal event recording:
     ```rust
     entity.archive();
     let events = entity.take_events();
     ```

4. **Domain Event Versioning**:
   - Treat `DomainEvent` variants as append-only or explicitly versioned to ensure forward-compatibility with plugins and downstream consumers.

5. **Future Architectural Fitness Testing (Roadmap)**:
   - *Note: Treat these additions as optional roadmap items to introduce incrementally if/when complexity increases, not as mandatory milestones.*
   - **AST-Based Enforcement**: As text-processing code grows complex, transition the fitness test suite to parse Rust code semantically using AST tools like `syn` or `ra_ap_syntax` rather than simple line-by-line text/regex scans.
   - **Positive Assertions**: Incorporate structural validation checks verifying that required standards (e.g. traits derived, expected placement directories) are successfully satisfied.
   - **Dependency DAG Verification**: Assert full workspace dependency layout invariants (e.g. verifying that `brain-domain` lacks outgoing dependencies and `brain-services` handles facade orchestrations) using `cargo_metadata` or cargo tree analysis.

## Framework Evolution Guardrails

To prevent the framework from growing faster than the product, adhere to this core project rule:

> **Framework code exists only to enable capabilities. Capabilities should not require framework evolution by default.**

Before proposing or introducing any new foundational framework abstraction, enforce the following checklist:
1. **Reuse**: Can this capability be implemented cleanly using the existing aggregate, event, projection, and query models?
2. **Duplication Proof**: Is this the second or third separate feature showing a clear need for the proposed abstraction?
3. **Generalizability**: Will the new abstraction simplify multiple future capabilities, rather than just the immediate task at hand?

Unless the answer to all three is a clear "yes", implement the capability strictly on top of the existing architecture instead of extending it.
