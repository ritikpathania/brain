---
status: active
owner: storage
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Storage Backend Subsystem

The storage subsystem manages durable SQLite state, event logging, and high-performance read-model projections to achieve sub-millisecond query latencies.

## 1. SQLite Storage Engine (`crates/brain-storage/src/`)

SQLite stores nodes, relationship edges, event logs, and search projection tables under Write-Ahead Logging (WAL) mode.

### Schema & Projection Models
* **`nodes`**: `id (TEXT PRIMARY KEY)`, `label (TEXT)`, `type (TEXT)`, `properties (TEXT JSON)`, `updated_at (INTEGER)`
* **`edges`**: `source (TEXT)`, `target (TEXT)`, `relation (TEXT)`, `weight (REAL DEFAULT 1.0)`, `updated_at (INTEGER)`, referencing nodes.
* **`search_projection`**: In-memory FTS5 and vector similarity index projections for hybrid keyword and embedding retrieval.
* **`sessions_projection`**: Session history and turn window projections.
* **`jobs_projection`**: Background async task and consolidation checkpoints.

### Mathematical Decay & Reinforcement
When edge updates occur, the edge weight is reinforced by adding $+0.5$ (capped at $2.0$). 
During background consolidation passes, relationship weights are dampened using half-life decay:

\[W_{\text{new}} = W_{\text{old}} \times e^{-\lambda \Delta t}\]

where \(\lambda = \frac{\ln(2)}{T_{1/2}}\). Edges dropping below the threshold weight (default: $0.1$) are pruned.
