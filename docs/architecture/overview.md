# Architecture Overview

This document provides a detailed breakdown of the active architecture of the Brain Relational Memory Engine, detailing the core components, their lifecycles, and transaction flows.

---

## 1. System Topology

The platform operates as a decoupled client-server system communicating over a low-overhead Unix Domain Socket (UDS) JSON-IPC wire protocol:

```text
  ┌─────────────────────────────────┐
  │           Ratatui TUI           │ (Interactive Console Client)
  └────────────────┬────────────────┘
                   │
                   │ Unix Domain Socket (UDS) JSON-IPC
                   ▼
  ┌─────────────────────────────────┐
  │        Transport Daemon         │ (Stateless socket listener)
  └────────────────┬────────────────┘
                   │
                   │ In-Memory Rust Calls
                   ▼
  ┌─────────────────────────────────┐
  │          BrainRuntime           │ (Authoritative Composition Root)
  │                                 │
  │  ┌───────────────────────────┐  │
  │  │      SearchProjector      │  │ (Lexical & Vector Projection)
  │  └───────────────────────────┘  │
  │  ┌───────────────────────────┐  │
  │  │   InMemoryDispatcher      │  │ (Synchronous Event Bus)
  │  └───────────────────────────┘  │
  │  ┌───────────────────────────┐  │
  │  │       SqliteStorage       │  │ (Durable Transaction Backend)
  │  └───────────────────────────┘  │
  └─────────────────────────────────┘
```

---

## 2. Core Components

### BrainRuntime
`BrainRuntime` is the authoritative composition root and lifecycle owner of the relational memory engine. It exposes clean boundaries:
- `ingest(obs)`: Authoritative write pathway. Coordinates validation, canonicalization, and relationship reflection transactionally.
- `query_projection(projector, query, correlation_id)`: Authoritative read pathway. Generates materializations of the knowledge graph on-demand.
- `subscribe()`: Allows external observers to listen to the runtime event stream.
- `shutdown()`: Flushes event queues, terminates background threads, and closes database connections.

### SqliteStorage
Located in `brain-storage` crate, `SqliteStorage` manages the durable transaction engine. It exposes the transaction boundaries using a pool of connections and ensures ACID compliance.

### SearchProjector & MemoryListProjection
Located in `brain-services`, these represent native read-model projections.
- `SearchProjector` implements lexical matching (BM25) and semantic vector similarity nearest-neighbors queries against SQLite, returning pure domain-level documents.

### InMemoryEventDispatcher
A synchronous event bus providing in-order event propagation. When the database transaction commits, events (such as `ObservationCanonicalized`) are published to the dispatcher, which routes them to synchronous and asynchronous observers (e.g. reflection engine).

---

## 3. Ingestion Lifecycle (Write Path)

Every ingested observation is processed synchronously inside a single SQLite transaction to preserve strict database invariants:

```mermaid
sequenceDiagram
    autonumber
    participant Client
    participant Daemon as Transport Daemon
    participant RT as BrainRuntime
    participant Canonicalizer as SqliteCanonicalizer
    participant Storage as SqliteStorage
    participant Dispatcher as InMemoryDispatcher
    participant Reflection as ReflectionEngine

    Client->>Daemon: Send ingest request (JSON payload)
    Daemon->>RT: Invoke RT::ingest(Observation)
    RT->>Canonicalizer: Delegate to canonicalizer
    activate Canonicalizer
    Canonicalizer->>Storage: Begin Transaction
    Canonicalizer->>Storage: Verify invariants and deduplicate nodes
    Canonicalizer->>Storage: Write entities to DB
    Canonicalizer->>Storage: Commit Transaction
    Canonicalizer->>Dispatcher: Publish Ingested Events
    activate Dispatcher
    Dispatcher->>Reflection: Propagate event to ReflectionEngine
    Reflection->>Storage: Run ontology reflection & write inferred edges
    deactivate Dispatcher
    deactivate Canonicalizer
    RT-->>Daemon: Return IngestionResult (epoch, affected nodes)
    Daemon-->>Client: Return JSON success response
```

---

## 4. Query Lifecycle (Read Path)

Read queries are routed on-demand through the `SearchProjector` projection:

```mermaid
sequenceDiagram
    autonumber
    participant Client
    participant Daemon as Transport Daemon
    participant RT as BrainRuntime
    participant Projector as SearchProjector
    participant Storage as SqliteStorage

    Client->>Daemon: Send query request (JSON payload)
    Daemon->>RT: Invoke RT::query_projection(SearchProjector, Query)
    RT->>Projector: Execute project(query)
    Projector->>Storage: Query node entries (BM25 lexical search)
    Projector->>Storage: Query vector embeddings (float similarity)
    Projector->>Storage: Query matching edges
    Projector-->>RT: Assemble SearchResult
    RT-->>Daemon: Return search document candidates
    Daemon-->>Client: Stream results (StreamEvent chunk updates)
```

---

## 5. Observability & Telemetry

Operational auditing is instrumented directly within the engine:
- **Metrics**: Runtime execution counts, average canonicalization/reflection/dispatch stage latencies, and total operations are tracked via lock-free atomics in `RuntimeMetrics`.
- **Diagnostics**: A FIFO ring-buffer retains the 50 most recent operational failures, inspectable via `RuntimeDiagnostics` at any time.
