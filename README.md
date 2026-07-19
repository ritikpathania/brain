# Brain: AI Coding Companion Relational Memory Engine

## 1. What Brain Is
Brain is a high-performance, stateful Relational Memory Engine designed to store, canonicalize, and query structured knowledge for autonomous coding agents and interactive console interfaces. It operates as a local background daemon that exposes a Unix Domain Socket (UDS) IPC interface, backing a native terminal user interface (TUI) and external agent adapters.

---

## 2. Why It Exists
In traditional AI coding systems, state is ephemeral, context is scattered, and there is no single source of truth for the project's memory. Brain solves this by acting as a **single authoritative runtime** context. It encapsulates:
*   **ACID Ingestion**: Validates, canonicalizes, and deduplicates incoming observations transactionally.
*   **Relationship Reflection**: Spawns ontology-driven reflections (e.g., links, associations) automatically upon ingestion.
*   **Unified Projections**: Exposes high-performance read-model projections (hybrid keyword + vector search) directly from storage, bypassing duplicate in-memory pipelines.
*   **Adapter Isolation**: Keeps transport protocols (MCP, HTTP, UDS) stateless, mapping wire payloads to a clean runtime boundary.

---

## 3. High-Level Architecture

```text
       ┌──────────────────┐
       │   Client / TUI   │ (Ratatui Console Client)
       └────────┬─────────┘
                │
                │ Unix Domain Socket (UDS) IPC
                ▼
       ┌──────────────────┐
       │  Transport Daemon│ (Stateless JSON-IPC Adapter)
       └────────┬─────────┘
                │
                │ Direct Memory Calls
                ▼
  ┌────────────────────────────┐
  │        BrainRuntime        │ (Authoritative Composition Root)
  │                            │
  │  ┌──────────────────────┐  │
  │  │   SearchProjector    │  │ (Lexical & Vector Projection)
  │  └──────────────────────┘  │
  │  ┌──────────────────────┐  │
  │  │    SqliteStorage     │  │ (Durable Transaction Engine)
  │  └──────────────────────┘  │
  └────────────────────────────┘
```

The system is split into three main layers:
1.  **Frontend / UI (`crates/brain-tui/`)**: A native Rust interactive terminal UI built with Ratatui.
2.  **Transport Adapter (`daemon/`)**: A lightweight background daemon (`brain-daemon`) that processes socket requests, validates them against the JSON Schema wire protocol, and routes them directly to the runtime.
3.  **Core Relational Engine (`crates/brain-services/`)**: The authoritative business logic (`BrainRuntime`), coordinating SQLite persistence (`crates/brain-storage/`) and event dispatching (`crates/brain-events/`).

---

## 4. Quick Start (5 Minutes)

### Prerequisites
Make sure you have Rust (Cargo) and `uv` (Python package manager) installed.

### Step 1: Sync Environment and Dependencies
Initialize the Python virtualenv and dependencies for Maturin compilation:
```bash
make setup
```

### Step 2: Build the Daemon and Client
Compile the PyO3 Rust extension modules and build the standalone binaries:
```bash
make build-daemon
```

### Step 3: Run the System
You can start the background daemon process and TUI interface using the standard Makefile workflow:
```bash
# Compile and start the background daemon
make dev
```

---

## 5. Repository Layout
```text
.
├── Cargo.toml                  # Workspace dependencies configuration
├── Makefile                    # Standard build and run shortcuts
├── apps/                       # Standalone binaries
│   └── brain/                  # Main entry point CLI
├── crates/                     # Core system crates
│   ├── brain-core/             # Core interfaces and common error types
│   ├── brain-domain/           # Entities, aggregates, and domain invariants
│   ├── brain-events/           # Events definitions and publishing
│   ├── brain-observability/    # Performance tracing and diagnostics
│   ├── brain-python/           # Python FFI bindings via PyO3
│   ├── brain-services/         # Composition root (BrainRuntime) & projections
│   ├── brain-storage/          # SQLite storage engine & transactions
│   └── brain-tui/              # Ratatui terminal user interface
├── daemon/                     # Stateless IPC socket listener daemon
└── docs/                       # System documentation
```

---

## 6. Deeper Documentation Links

*   **[Documentation Index](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/README.md)**: The central directory map outlining the active guides.
*   **[Architecture Overview](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/overview.md)**: Deep dive into the runtime lifecycle and components.
*   **[Architectural Principles](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/principles.md)**: The core invariants that guide all changes.
*   **[Technical Reference Index](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/)**: Specifications for UDS IPC sockets, schemas, and configurations.
