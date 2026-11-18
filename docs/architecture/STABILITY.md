---
status: active
owner: architecture
canonical: true
review_cycle: quarterly
last_reviewed: 2026-09-30
applies_to: v1.1+
---

# Architectural Stability Guide

This document defines the stability contracts and extension boundaries of the Standalone Relational Memory Engine (`brain`). It acts as a guide for contributors, clarifying which subsystems are frozen infrastructure versus which are extensible or experimental.

---

## Stability Levels

- **Frozen**: Public contracts and architectural boundaries are stable. Internal bug fixes, performance improvements, and refactoring that preserve behavior are allowed. Modifying public signatures or boundaries requires submitting an **Architectural Decision Record (ADR)** and undergoes rigorous regression and performance profile audits.
- **Extensible**: New implementations and variants may be added through existing extension points (such as Rust traits or typed adapter interfaces) without modifying frozen orchestration code.
- **Experimental**: APIs and algorithms may change between milestones until promoted to Extensible or Frozen.

---

## 1. Frozen Layers
* **Domain & Core (`brain-core`, `brain-domain`)**: Basic value types (`Node`, `Edge`, `MemoryDTO`), traits, and interface signatures are locked.
* **Storage (`brain-storage`)**: Transactional SQLite watermarks, CASCADE delete schemas, and migrations are frozen.
* **Retrieval & Ranking (`brain-services/src/retrieval`)**: The sequential candidate collection, RRF merge execution, and tie-breaking sort pipelines are frozen.
* **Plugins & Runtime (`brain-plugins`, `brain-python`)**: PyO3 GIL release boundaries, thread offloading, and context validity checking are frozen.
* **Streaming Runtime (`brain-services/src/agent/streaming.rs`)**: Monotonic sequence tracking, subscriber caught-up replay logic, and backpressure policies (ADR-007) are frozen.
* **Canonical frontend boundary**: The versioned UDS frontend protocol, handshake/capability contract, and authoritative Brain session/tool lifecycle are frozen. Presentation is owned by `packages/brain-frontend`, above typed state/projection; backend boundaries remain presentation-agnostic.

---

## 2. Extensible Layers
* **Memory Policies (`PromotionPolicy`, `SummaryPolicy`)**: Custom promotion algorithms (recency, goal-aware) and rolling summary rules are added by implementing these traits.
* **Ranking Strategies (`RankingStrategy`)**: Alternative rankers (semantic importance, BM25, graph centrality) can be registered with the pipeline builder.
* **Execution Stages (`ExecutionStage`)**: New lifecycle stages are integrated by appending them to the runner's sequence list.
* **Workflow Nodes**: Custom nodes (conditional switches, parallel steps, retry scopes) can be written as independent tool blocks.
* **Frontend adapters**: New backend capabilities may be exposed by explicit versioned DTOs and adapter view models without leaking storage or Rust domain structures into presentation code.
* **Presentation changes**: Belong to the owned frontend layer and require interaction/render/PTY regression evidence without changing backend protocol semantics.

---

## 3. Experimental Layers
* **Adaptive Memory**: Goal-aware promotion and importance scoring heuristics.
* **Reflection & Verification**: Execution loop self-correction and validation stages.
* **Workflow Scheduling**: Graph-based task execution and async parallel node resolvers.
