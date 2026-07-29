# Brain Architecture Constitution

> **Status**: Frozen (Normative Baseline)  
> **Authority**: Timeless System Identity

---

## 1. System Identity

> **Brain is a deterministic, local-first knowledge runtime.**

Brain stores, canonicalizes, and projects structured knowledge for autonomous coding agents and developer interfaces. It is not an LLM application, a coding assistant, or a workflow executor; it is the context-aware relational memory engine supporting external execution.

---

## 2. Core Axioms

The three foundational axioms represent the core invariants of Brain's design:

* **Axiom 1 (Knowledge Runtime)**: Brain **MUST** operate as a knowledge runtime, not a coding agent. It does not write code, execute test suites, or manage user workflows directly.
* **Axiom 2 (Knowledge Ownership)**: Brain **MUST** own knowledge, not workflows. Workflows consume knowledge; they do not define it.
* **Axiom 3 (Canonical Knowledge)**: Semantic knowledge **MUST** be canonical. Persistence formats, indexes, snapshots, projections, and transport wrappers are derived views.

---

## 3. The 4-Layer Dependency Hierarchy

Dependencies flow **strictly downward**. No layer may skip intermediate layers or import modules from a higher layer:

```text
1. Application Layer    (Adapters: UDS, MCP, ACP, REST, SDK | TUI | CLI)
         │
         ▼
2. Runtime Layer        (BrainRuntime | KnowledgeCompiler | CapabilityRegistry)
         │
         ▼
3. Domain Layer         (Observations | Knowledge | Provenance | Identifiers | Rules)
         │
         ▼
4. Infrastructure Layer (SQLite | SQLCipher | Local FS | Hardware Tracing)
```

---

## 4. Timeless Runtime Invariants

Every subsystem and execution path must satisfy these eleven normative rules:

1. **Single Mutation Entry**: Every state change **MUST** enter strictly through `KnowledgeCompiler` via a `MutationRequest`.
2. **Compile-Time Read Projections**: `ReadProjection` implementations **MUST** accept only `&self` and `&CanonicalGraph`; they **MUST NOT** acquire write handles or mutate state.
3. **Reflection as Analysis**: Reflection **MUST** operate as an analysis phase emitting `MutationRequest`s; it **MUST NOT** mutate storage directly.
4. **Adapter Storage Isolation**: Stateless transport adapters **MUST NOT** import or invoke storage repositories directly.
5. **Orchestration Only Runtime**: `BrainRuntime` **MUST** coordinate capabilities and context; it **MUST NOT** own feature-specific business logic.
6. **Deterministic Compiler Passes**: Given identical inputs and context state, compiler passes **MUST** produce bit-wise identical `CompilerResult` outputs.
7. **Controlled Graph Mutability**: The Canonical Graph **MUST** be mutable only through `KnowledgeCompiler`. Outside the compiler, it **MUST** be observable as an immutable value.
8. **Strongly-Typed Provenance**: Every domain entity **MUST** carry queryable provenance tracing its origin, compiler pass, and timestamp.
9. **Idempotent Mutation Processing**: Processing the same `MutationRequest` multiple times **MUST** produce identical canonical state or be rejected via deterministic identity.
10. **Primitive Composition**: Every new feature **MUST** be expressed by composing existing architectural primitives.
11. **Primitive Evolution**: Introducing a new architectural primitive **MUST** require an ADR demonstrating that existing primitives cannot express the capability without violating another invariant.

---

## 5. Constitutional Governance Principles

* **Architectural Minimality Principle**: The architectural vocabulary **MUST** grow more slowly than the feature set. New capabilities **SHOULD** almost always be expressed through composition of existing primitives rather than creation of new ones.
* **Architecture First, Optimization Second**: No performance optimization **MAY** violate a constitutional invariant. If an optimization requires relaxing an invariant, it must first amend the Constitution through the defined governance process.
* **Constitutional Stability Principle**: The Constitution exists to preserve the architectural identity of Brain. The Constitution **MUST** change only when the architectural identity of Brain changes.
