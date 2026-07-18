# Brain Architecture Philosophy

This document defines the foundational architectural guidelines and design principles for **Brain** as a stateful, relational **knowledge runtime**. 

---

## 1. Core Architectural Axioms

These three foundational axioms represent the core invariants of Brain's design:

### Axiom 1: Brain is a knowledge runtime, not a coding agent
Brain does not write code, manage workspaces, execute test suites, or perform automated refactoring. It serves as the context-aware memory substrate supporting those actions from the outside.

### Axiom 2: Brain owns knowledge, not workflows
Workflows consume knowledge; they do not define it. Brain manages retrieval heuristics, context compilation, associative edges, and cognitive reflection. Client applications own user interfaces, plan generation, permission gates, and execution control loops.
*   *Rule*: If a proposed feature is about an application's workflow rather than knowledge representation, it belongs outside the runtime.

### Axiom 3: Semantic knowledge is canonical
The runtime owns the authoritative semantic state. Persistence, caches, indexes, snapshots, projections, and transport-specific representations are all derived from that canonical semantic knowledge model. 

---

## 2. Derived Architectural Consequences

These principles follow naturally as direct structural consequences of our core axioms:

### Consequence 1: Presentation is a projection of runtime state
Presentation layers observe runtime state through stable application interfaces and never execute or own domain business logic. This separation guarantees UI stability and prevents visual components from leaking into the core model. Projections are disposable, transient presentations that can connect, disconnect, or terminate entirely without threatening the integrity of the underlying runtime state.

### Consequence 2: Protocol independence enables universal access
Any client or transport interacts with the runtime through a stable, unified application interface. The runtime does not care which transport layer is accessing it (such as TUI, CLI, ACP, MCP, A2A, or SDK adapters).

---

## 3. Candidate Architectural Principles (Emerging Direction)

As the runtime evolves, these principles represent candidate invariants currently under observation:

### Candidate 1: Knowledge evolves through deterministic transformations
Knowledge state changes should progress through deterministic, pipeline-oriented transformations rather than arbitrary state mutation. Data flows sequentially from raw observation, to extraction, to synthesis/compaction, reflection, and projection.

```
Raw Observation ──► Extraction ──► Synthesis ──► Reflection ──► Projection
```

### Candidate 2: Evolution preserves semantic identity
Brain is a long-lived knowledge runtime. While storage formats, index algorithms, and retrieval weights may migrate and adapt, the underlying semantic identity of established entities and relationships must remain stable over time.

---

## 4. Operational Review: Philosophy Compliance Checklist

To ensure these principles remain active, every new Pull Request, ADR, or RFC must proceed through the architectural verification cycle:

```
          RFC ──► Implementation ──► Fitness Tests ──► Verification
```

Every contribution must be evaluated against this compliance checklist during review:

```markdown
### Philosophy Compliance
- [ ] Respects Axiom 1: Brain is a knowledge runtime, not a coding agent.
- [ ] Respects Axiom 2: Brain owns knowledge, not workflows (Workflows consume knowledge; they do not define it).
- [ ] Respects Axiom 3: Semantic knowledge is canonical.
- [ ] Respects Consequence 1: Presentation is a projection (no business logic in projections).
- [ ] Respects Consequence 2: Preserves protocol independence.

### Stability & Roadmap Check
- [ ] Requires no ADR (pure implementation)
- [ ] Requires ADR (structural change)
- [ ] Requires RFC before implementation (Investigate item)
- [ ] Adopt item (High confidence change)
- [ ] Investigate item (Requires design validation first)
```

### A. Architectural Fitness Tests

To prevent gradual design erosion, the project implements automated fitness tests that enforce our invariants inside the test suite:

| Key Constraint | Verification Method |
| :--- | :--- |
| `brain-domain` has zero outgoing dependencies on presentation or service crates. | Automated static dependency analysis (`cargo tree` / AST checks). |
| Projections are strictly read-only and never mutate canonical state. | Unit & Property-based checks asserting mutation boundaries. |
| Once emitted, Domain Events are immutable. | Serialization invariants and immutability checks in event loop tests. |
| Reflection never mutates canonical knowledge facts. | Integration checks verifying Reflection only writes to derived tables. |
| Graph state evolution remains deterministic. | Replay tests asserting bit-wise identical SQLite databases for matching input logs. |
| Protocol adapters and transports contain no business logic. | Arch/lint tests checking separation of adapter facade layers. |

---
---

## Appendix: Actionable Research Roadmap & Priority Buckets

To ground our core philosophy, this appendix maps the lessons learned from analyzing systems like `grok-build` against Brain's actual roadmap.

### A. Stable Core Architecture (No Changes Required)
The following foundational abstractions in Brain are already more generalized and decoupled than external coding-assistant systems, and should remain unchanged:
*   **Domain Decoupling**: Hexagonal adapter architecture, protocol independence, and stable client/adapter interfaces.
*   **DDD Invariants**: Clear separation of domain logic (`brain-domain`), application services (`brain-services`), and event serialization envelopes.

### B. Actionable Roadmap: Adopt (High Confidence)
These patterns are directly supported by our architecture, represent low risk, and should be adopted during implementation:
1.  **Frictionless Keyboard Ergonomics**: Universal command palettes, fuzzy list navigation, consistent hotkey bindings, and fluid focus switching to minimize user modal fatigue.
2.  **Panel Management**: Dynamic resize boundaries (`SIGWINCH`), collapsible detail views, saved viewport layouts, and high-visibility focus indicators.
3.  **Async Rendering Boundaries**: Audit all database, file access, and Python FFI calls to guarantee they run on async worker threads (such as `tokio::task::spawn_blocking`), ensuring background processing never interrupts client UI responsiveness.
4.  **Semantic Progress Updates**: Expose structured, semantic events for long-running reflection or compilation pipelines (e.g. showing stage transitions like `[Extracting Entities] -> [Consolidating Edges]`) instead of generic loader animations.
5.  **Integrated Modal Workflows**: Investing specifically in layout modals for core Brain decisions: memory merging, semantic conflict resolution, and memory deletion.
6.  **Structured State Visualization**: Rendering visual representations of background jobs (ingestion, entity extraction, relational consolidation, active reflection, and client projection updates) rather than dumping unstructured debug logs.

### C. Actionable Roadmap: Investigate (Requires Design Work & RFCs)
These represent larger architectural investments or semantic capabilities. Before writing code, we must explore design options (e.g., dedicated subsystems vs. incremental traits) and draft RFCs:
1.  **First-Class Projection Architecture**: Designing how projections are structured (ensuring projections are first-class runtime assets rather than transport-specific serializers, while keeping the implementation details agnostic of concrete engines or crates).
2.  **Knowledge Transformation Pipeline**: Modeling the deterministic, pipeline-oriented flow of raw observations into finalized semantic knowledge.
3.  **Reflection UX**: Design loops for surfacing background compaction, memory decay, and semantic adjustments to client views.
4.  **Graph Visualization & Interactive Inspection**: Evaluating methods for graph neighborhood exploration, relationship visualization, and interactive node/edge cards in the terminal and protocols.

### D. Non-Goals (What We Will NOT Build)
To avoid architectural drift, we will explicitly ignore coding-assistant workflow features:
*   **No Code Review UIs** (no inline git diff displays, workspace/repository file explorer tabs, or coding approvals).
*   **No Git Worktree Orchestration** (workspace branching is owned entirely by the consuming developer tools).
*   **No Multi-Agent Dashboards** (orchestration metrics or Arena Mode simulations belong inside client agents).
*   **No Planning Interfaces** (no `plan.md` authorization loops or filesystem command execution gates).

### E. Reference Map for Specialized Research
We will direct all future engineering research toward systems that share Brain's core runtime domain:
*   **Knowledge & Note Systems** (*Obsidian, Logseq, Tana, Roam Research*): For temporal graphs, backlink indexes, memory decay modeling, and contextual retrieval windows.
*   **Graph Databases** (*Neo4j Browser, RedisInsight*): For neighborhood graph traversal visualization and interactive node/edge inspection.
*   **Terminal TUIs** (*Helix, Lazygit, Zellij, K9s*): For advanced layout systems, mouse/keyboard integration, and panel navigation workflows.
*   **Distributed Engines** (*Temporal, Dagster, Prefect*): For tracing state transitions, background compaction flows, and execution graphs.

### F. Phased Execution Roadmap
To prevent the user interface from driving the runtime's design, implementation work must be ordered such that runtime contracts and capabilities are defined before UI presentations consume them:

#### Phase 1: Runtime Contracts (Highest Priority)
These define the shared context and interfaces that unlock downstream client features:
1.  **RFC-008: Projection Architecture** — Define what a projection is, its lifecycle, metadata contracts, and compare implementation structures (services vs. traits vs. modules) without premature crate constraints.
2.  **RFC-009: Runtime Event & Progress Model** — Establish a standard, capability-neutral semantic event taxonomy and operation lifecycle (separating deep domain events from UI-oriented progress chunks).

#### Phase 2: Runtime Evolution
3.  **RFC-010: Knowledge Transformation Pipeline** — Model the sequence of Observations ──► Extractions ──► Compaction ──► Projections.
4.  **RFC-011: Reflection UX** — Design how background evolution, relationship decay, and semantic corrections are surfaced to clients.

#### Phase 3: Presentation & TUI (Consuming Stable Contracts)
5.  **RFC-012: Keyboard Navigation Model** — Mapped shortcuts and modal triggers.
6.  **RFC-013: Panel & Layout System** — Collapsible and resizable terminal structures.
7.  **RFC-014: Modal Interaction Framework** — Standard views for merges, deletions, and conflict resolution.
8.  **RFC-015: State & Progress UI** — Rendering visual progress graphs and job trackers.

#### Phase 4: Graph Exploration & Visualization
9.  **RFC-016: Graph Exploration** — Visualizing semantic associations and neighborhood nodes.
10. **RFC-017: Interactive Inspector** — Detailed entity and edge attribute cards.
